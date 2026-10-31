//! Pre-registered ownership transfer; no GPU destructor runs on the producer.
use crate::{GpuContext, RendererError, RendererFault, recovery, submission::Submission};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use vulkano::{
    command_buffer::allocator::StandardCommandBufferAllocator,
    device::{Device, Queue},
    instance::Instance,
    memory::allocator::StandardMemoryAllocator,
    swapchain::SwapchainAcquireFuture,
};

pub(crate) enum Owner {
    Frame {
        gpu: Arc<GpuContext>,
        submission: Option<Submission>,
        acquired: Option<SwapchainAcquireFuture>,
    },
    Context {
        // Keep the last Queue (whose Drop waits idle) on the worker, even when
        // there was no renderer or no submitted frame.
        _instance: Arc<Instance>,
        _device: Arc<Device>,
        _queue: Arc<Queue>,
        _commands: Arc<StandardCommandBufferAllocator>,
        _memory: Arc<StandardMemoryAllocator>,
        released: Arc<AtomicBool>,
    },
}
impl Owner {
    pub fn context(gpu: &GpuContext) -> Self {
        Self::Context {
            _instance: gpu.instance.clone(),
            _device: gpu.device.clone(),
            _queue: gpu.queue.clone(),
            _commands: gpu.commands.clone(),
            _memory: gpu.memory.clone(),
            released: gpu.release.0.clone(),
        }
    }
    fn retire(self) -> Result<(), RendererError> {
        match self {
            Self::Frame {
                gpu,
                submission,
                acquired,
            } => recovery::retire_owned_futures(&gpu, submission, acquired),
            context @ Self::Context { .. } => {
                if let Self::Context { released, .. } = &context {
                    // The cloned queue must not be released before the
                    // producer's original fields finish dropping.
                    while !released.load(Ordering::Acquire) {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
                drop(context);
                Ok(())
            }
        }
    }
}

pub(crate) struct ReleaseMarker(pub Arc<AtomicBool>);
impl Default for ReleaseMarker {
    fn default() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
}
impl Drop for ReleaseMarker {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

pub(crate) struct Slot<T>(Mutex<Option<T>>);
pub(crate) struct Registry<T> {
    slots: Mutex<Vec<Arc<Slot<T>>>>,
    pub(crate) status: Arc<RetirementStatus>,
}
#[derive(Default)]
pub(crate) struct RetirementStatus {
    pending: AtomicUsize,
    error: OnceLock<RendererError>,
}
impl<T> Default for Registry<T> {
    fn default() -> Self {
        Self {
            slots: Mutex::default(),
            status: Arc::new(RetirementStatus::default()),
        }
    }
}
impl<T> Registry<T> {
    pub fn register(&self) -> Arc<Slot<T>> {
        let slot = Arc::new(Slot(Mutex::new(None)));
        self.slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(slot.clone());
        slot
    }
    // Only the registered owner's Drop calls this, exactly once. The payload
    // has a reserved destination regardless of request-channel backpressure.
    pub fn handoff(&self, slot: &Slot<T>, owner: T) {
        let mut place = slot.0.lock().unwrap_or_else(|e| e.into_inner());
        assert!(place.is_none(), "retirement slot published twice");
        self.status.pending.fetch_add(1, Ordering::AcqRel);
        *place = Some(owner);
    }
    #[cfg(test)]
    pub fn pending(&self) -> usize {
        self.status.pending.load(Ordering::Acquire)
    }
    pub fn record_error(&self, error: RendererError) {
        eprintln!("{error}");
        let _ = self.status.error.set(error);
    }
    pub fn drain(&self, mut retire: impl FnMut(T) -> Result<(), RendererError>) {
        loop {
            // Never hold the registry or slot mutex across driver calls/destructors.
            let slots = self.slots.lock().unwrap_or_else(|e| e.into_inner()).clone();
            for slot in &slots {
                let owner = slot.0.lock().unwrap_or_else(|e| e.into_inner()).take();
                if let Some(owner) = owner {
                    let result =
                        catch_unwind(AssertUnwindSafe(|| retire(owner))).unwrap_or_else(|_| {
                            Err(RendererError {
                                code: "native_vulkan_retirement_dependency_panicked",
                                stage: "retirement",
                                fault: RendererFault::Other,
                                message:
                                    "GPU owner retirement panicked; DeviceLost is not inferred"
                                        .into(),
                                element: None,
                            })
                        });
                    if let Err(error) = result {
                        self.record_error(error);
                    }
                    self.status.pending.fetch_sub(1, Ordering::AcqRel);
                }
            }
            drop(slots);
            self.slots
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|slot| {
                    Arc::strong_count(slot) > 1
                        || slot.0.lock().unwrap_or_else(|e| e.into_inner()).is_some()
                });
            // A producer may publish into an earlier slot while another owner is
            // being retired. Drain it before accepting a replacement request.
            if self.status.pending.load(Ordering::Acquire) == 0 {
                break;
            }
        }
    }
}
impl Registry<Owner> {
    pub fn drain_gpu(&self) {
        self.drain(|owner| {
            #[cfg(feature = "fault-injection")]
            let kind = match &owner {
                Owner::Frame {
                    submission,
                    acquired,
                    ..
                } => format!(
                    "frame submission={} acquired={}",
                    submission.is_some(),
                    acquired.is_some()
                ),
                Owner::Context { .. } => "context".into(),
            };
            let result = owner.retire();
            #[cfg(feature = "fault-injection")]
            eprintln!(
                "UI-R07 retirement: thread={} kind={kind} success={}",
                std::thread::current().name().unwrap_or("unnamed"),
                result.is_ok()
            );
            result
        });
    }
}

/// CPU-only observation. It owns no worker sender, window or GPU generation.
#[derive(Clone)]
pub struct RetirementMonitor {
    pub(crate) status: Arc<RetirementStatus>,
    pub(crate) busy: Arc<AtomicBool>,
}
impl RetirementMonitor {
    pub fn pending(&self) -> bool {
        self.status.pending.load(Ordering::Acquire) != 0 || self.busy.load(Ordering::Acquire)
    }
    /// Call after handing off all host GPU owners. This bounds observation, not
    /// a driver call, and never grants permission to release an unfinished owner.
    pub fn observe(&self, budget: Duration) -> Result<(), RendererError> {
        observe_pending(budget, || self.pending())?;
        match self.status.error.get() {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}
fn observe_pending(budget: Duration, pending: impl Fn() -> bool) -> Result<(), RendererError> {
    let deadline = Instant::now().checked_add(budget).ok_or_else(|| {
        RendererError::at(
            "retirement_budget",
            "observation deadline is not representable",
        )
    })?;
    while pending() {
        if Instant::now() >= deadline {
            return Err(RendererError {
                code: "native_vulkan_retirement_observation_timeout",
                stage: "retirement",
                fault: RendererFault::CompletionTimeout,
                message:
                    "GPU worker still owns pending resources after the host observation budget"
                        .into(),
                element: None,
            });
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reserved_slots_survive_busy_worker_and_are_pruned_only_after_retirement() {
        struct Owner(Arc<AtomicUsize>);
        impl Drop for Owner {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        let registry = Arc::new(Registry::default());
        let dropped = Arc::new(AtomicUsize::new(0));
        let late = registry.register();
        let first = registry.register();
        let second = registry.register();
        let live = registry.register();
        registry.handoff(&first, Owner(dropped.clone()));
        registry.handoff(&second, Owner(dropped.clone()));
        drop(first);
        drop(second);
        assert_eq!(
            dropped.load(Ordering::Relaxed),
            0,
            "producer never drops owner"
        );
        assert_eq!(registry.pending(), 2);
        let monitor = RetirementMonitor {
            status: registry.status.clone(),
            busy: Arc::new(AtomicBool::new(false)),
        };
        assert_eq!(
            monitor
                .observe(Duration::MAX)
                .expect_err(
                    "an unrepresentable observation budget is rejected without unwinding owners"
                )
                .stage,
            "retirement_budget"
        );
        assert_eq!(registry.pending(), 2);
        assert_eq!(dropped.load(Ordering::Relaxed), 0);
        let timeout = monitor
            .observe(Duration::from_millis(2))
            .expect_err("host timeout does not release queued resources");
        assert_eq!(timeout.code, "native_vulkan_retirement_observation_timeout");
        assert_eq!(dropped.load(Ordering::Relaxed), 0);
        let producer = std::thread::current().id();
        let worker_registry = registry.clone();
        let late_counter = dropped.clone();
        std::thread::spawn(move || {
            let mut late = Some(late);
            worker_registry.drain(|owner| {
                if let Some(slot) = late.take() {
                    worker_registry.handoff(&slot, Owner(late_counter.clone()));
                }
                assert_ne!(std::thread::current().id(), producer);
                assert_ne!(
                    worker_registry.pending(),
                    0,
                    "pending until destructor finishes"
                );
                drop(owner);
                Ok(())
            })
        })
        .join()
        .expect("retirement worker fixture");
        assert_eq!(
            dropped.load(Ordering::Relaxed),
            3,
            "late earlier slot drained before returning"
        );
        assert_eq!(registry.pending(), 0);
        monitor
            .observe(Duration::from_millis(2))
            .expect("completed ownership transfer");
        monitor.busy.store(true, Ordering::Release);
        assert!(
            monitor.observe(Duration::from_millis(2)).is_err(),
            "recovery still running"
        );
        monitor.busy.store(false, Ordering::Release);
        assert_eq!(
            registry.slots.lock().expect("slot registry").len(),
            1,
            "live reservation retained"
        );
        registry.handoff(&live, Owner(dropped.clone()));
        registry.drain(|owner| {
            drop(owner);
            panic!("unknown retirement fixture")
        });
        assert_eq!(
            registry
                .status
                .error
                .get()
                .expect("unknown retirement error")
                .code,
            "native_vulkan_retirement_dependency_panicked"
        );
        assert_eq!(
            registry
                .status
                .error
                .get()
                .expect("unknown retirement error")
                .fault,
            RendererFault::Other
        );
        assert_eq!(registry.pending(), 0);
        assert_eq!(
            monitor
                .observe(Duration::from_millis(2))
                .expect_err("unknown cleanup fault stays observable")
                .fault,
            RendererFault::Other
        );
        drop(live);
        registry.drain(|_| Ok(()));
        assert!(registry.slots.lock().expect("slot registry").is_empty());
        let weak = Arc::downgrade(&registry);
        drop(registry);
        assert!(
            weak.upgrade().is_none(),
            "observation does not retain the ownership registry"
        );
    }
}
