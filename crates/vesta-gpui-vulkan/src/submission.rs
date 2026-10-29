//! One owned submission per window; flush failures retain its resources. ADR-0327.
use crate::{RendererError, RendererFault};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
    time::{Duration, Instant},
};
use vulkano::{
    VulkanError,
    command_buffer::PrimaryAutoCommandBuffer,
    device::{DeviceOwned, Queue},
    swapchain::{PresentFuture, SwapchainAcquireFuture, SwapchainPresentInfo},
    sync::{GpuFuture, future::FenceSignalFuture},
};

type RetainedFuture = Box<dyn GpuFuture + Send + Sync>;
type Fence = Arc<FenceSignalFuture<RetainedFuture>>;

pub(crate) struct Submission {
    command: Fence,
    presentation: Option<PresentFuture<Fence>>,
    completion: Option<Fence>,
    error: Option<RendererError>,
    submitted_at: Instant,
    gpu_timestamp: Option<Box<crate::gpu_profile::PendingGpuTimestamp>>,
    #[cfg(feature = "fault-injection")]
    poll_panic: bool,
}

impl Submission {
    pub fn offscreen(
        command: Arc<PrimaryAutoCommandBuffer>,
        queue: Arc<Queue>,
    ) -> Result<Self, RendererError> {
        let previous = vulkano::sync::now(command.device().clone());
        let command: RetainedFuture = Box::new(
            previous
                .then_execute(queue, command)
                .map_err(|e| RendererError::caused_by("capture_submit", e))?,
        );
        let command = Arc::new(command.then_signal_fence());
        Ok(Self {
            completion: Some(command.clone()),
            command,
            presentation: None,
            error: None,
            submitted_at: Instant::now(),
            gpu_timestamp: None,
            #[cfg(feature = "fault-injection")]
            poll_panic: false,
        })
    }
    pub fn submit_offscreen(&mut self) -> Result<(), RendererError> {
        let result = self
            .command
            .flush()
            .map_err(|e| RendererError::caused_by("capture_submit", e));
        if let Err(error) = &result {
            self.error = Some(error.clone());
        }
        result
    }
    /// No GPU flush occurs here. The caller installs this owner before submit.
    pub fn new(
        acquired: &mut Option<SwapchainAcquireFuture>,
        command: Arc<PrimaryAutoCommandBuffer>,
        queue: Arc<Queue>,
        gpu_timestamp: Option<crate::gpu_profile::PendingGpuTimestamp>,
    ) -> Result<Self, RendererError> {
        // Vulkano 0.35.2 execute_after only asserts these preconditions before
        // returning Ok. Check while the renderer still owns the acquire.
        let previous = take_validated(acquired, |previous| {
            if !Arc::ptr_eq(command.device(), previous.device())
                || !Arc::ptr_eq(command.device(), queue.device())
                || (!previous.queue_change_allowed()
                    && previous
                        .queue()
                        .is_none_or(|prior| !Arc::ptr_eq(&prior, &queue)))
            {
                return Err(RendererError::at(
                    "submit",
                    "acquire/command/queue device or queue mismatch; acquired owner retained",
                ));
            }
            Ok(())
        })?;
        let command: RetainedFuture = Box::new(
            previous
                .then_execute(queue, command)
                .map_err(|e| RendererError::caused_by("submit", e))?,
        );
        Ok(Self {
            command: Arc::new(command.then_signal_fence()),
            presentation: None,
            completion: None,
            error: None,
            submitted_at: Instant::now(),
            gpu_timestamp: gpu_timestamp.map(Box::new),
            #[cfg(feature = "fault-injection")]
            poll_panic: false,
        })
    }

    pub fn submit(
        &mut self,
        queue: Arc<Queue>,
        info: SwapchainPresentInfo,
        inject_present_out_of_date: bool,
    ) -> Result<(), RendererError> {
        let result = self.submit_stages(queue, info, inject_present_out_of_date);
        if let Err(error) = &result {
            self.error = Some(error.clone());
        }
        result
    }

    fn submit_stages(
        &mut self,
        queue: Arc<Queue>,
        info: SwapchainPresentInfo,
        inject_present_out_of_date: bool,
    ) -> Result<(), RendererError> {
        // Wrapping commands first avoids PresentFuture calling the command
        // future's internal unwrap flush. All owners survive returned errors.
        self.command
            .flush()
            .map_err(|e| RendererError::caused_by("submit", e))?;
        self.presentation = Some(self.command.clone().then_swapchain_present(queue, info));
        let presentation = self
            .presentation
            .as_ref()
            .ok_or_else(|| RendererError::at("present", "missing owned presentation"))?;
        let result = presentation
            .flush()
            .map_err(|e| RendererError::caused_by("present", e));
        // An actual Device Lost must not attempt another submission. Leave the
        // command and attempted presentation owned for the recovery boundary.
        if result
            .as_ref()
            .is_err_and(|error| error.fault == RendererFault::DeviceLost)
        {
            return result;
        }
        let result = if inject_present_out_of_date && result.is_ok() {
            Err(RendererError::caused_by("present", VulkanError::OutOfDate))
        } else {
            result
        };
        // PresentFuture marks the attempt flushed even on OutOfDate/SurfaceLost.
        // Its fence then submits an empty completion marker, without retrying
        // presentation or prematurely dropping the command resources.
        let presentation: RetainedFuture = Box::new(
            self.presentation
                .take()
                .ok_or_else(|| RendererError::at("completion", "missing owned presentation"))?,
        );
        self.completion = Some(Arc::new(presentation.then_signal_fence()));
        self.completion
            .as_ref()
            .ok_or_else(|| RendererError::at("completion", "missing owned completion fence"))?
            .flush()
            .map_err(|e| RendererError::caused_by("completion_submit", e))?;
        result
    }

    /// No wait on an unsignaled fence; false means this owner must be retained.
    pub fn poll(&mut self) -> Result<bool, RendererError> {
        let complete = self.poll_retirement()?;
        observe_completion(complete, self.submitted_at, Instant::now())
    }
    /// Retirement remains an owner after the host watchdog expires. A timeout
    /// does not make a still-running submission safe to destroy or reuse.
    pub fn poll_retirement(&mut self) -> Result<bool, RendererError> {
        completion_dependency(|| self.poll_retirement_inner())
    }

    fn poll_retirement_inner(&mut self) -> Result<bool, RendererError> {
        #[cfg(feature = "fault-injection")]
        if std::mem::take(&mut self.poll_panic) {
            panic!("intentional borrowed completion query panic");
        }
        let Some(completion) = &mut self.completion else {
            return Err(self.error.clone().unwrap_or_else(|| {
                RendererError::at("completion", "submission has no completion fence")
            }));
        };
        if !completion
            .is_signaled()
            .map_err(|e| RendererError::caused_by("completion", e))?
        {
            return Ok(false);
        }
        completion.cleanup_finished();
        self.command.cleanup_finished();
        // A loss racing cleanup is a typed error, rather than permission to
        // retire the owner based on a previous observation of the fence.
        let complete = completion
            .is_signaled()
            .map_err(|e| RendererError::caused_by("completion", e))?;
        if complete && let Some(timestamp) = self.gpu_timestamp.take() {
            (*timestamp).complete(self.submitted_at.elapsed())?;
        }
        Ok(complete)
    }

    /// Existing explicit smoke finish; normal consumers pass finish=false.
    /// Timeout leaves this Submission owned, and never waits/drops an unsignaled
    /// future as Vulkano's FenceSignalFuture::wait(timeout) would do.
    pub fn finish(&mut self) -> Result<(), RendererError> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if self.poll()? {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(RendererError {
                    code: "native_vulkan_completion_timeout",
                    stage: "completion",
                    fault: RendererFault::CompletionTimeout,
                    message: "GPU completion was not observed within the explicit smoke budget; submission retained".into(),
                    element: None,
                });
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[cfg(feature = "fault-injection")]
    pub fn has_completion_fence(&self) -> bool {
        self.completion.is_some()
    }

    #[cfg(feature = "fault-injection")]
    pub fn inject_poll_panic(&mut self) {
        self.poll_panic = true;
    }
}
fn completion_dependency(
    operation: impl FnOnce() -> Result<bool, RendererError>,
) -> Result<bool, RendererError> {
    // The caller borrows its owner. A panic cannot start dropping the owned
    // future while this dependency call is unwinding.
    catch_unwind(AssertUnwindSafe(operation)).unwrap_or_else(|_| {
        Err(RendererError {
            code: "native_vulkan_completion_dependency_panicked",
            stage: "completion",
            fault: RendererFault::Other,
            message: "GPU dependency panicked during borrowed completion observation; submission retained and DeviceLost is not inferred".into(),
            element: None,
        })
    })
}

fn take_validated<T>(
    owned: &mut Option<T>,
    validate: impl FnOnce(&T) -> Result<(), RendererError>,
) -> Result<T, RendererError> {
    validate(
        owned
            .as_ref()
            .ok_or_else(|| RendererError::at("submit", "missing owned acquired image"))?,
    )?;
    owned
        .take()
        .ok_or_else(|| RendererError::at("submit", "missing validated acquired image"))
}
fn observe_completion(
    complete: bool,
    submitted: Instant,
    now: Instant,
) -> Result<bool, RendererError> {
    if !complete && now.saturating_duration_since(submitted) >= Duration::from_secs(5) {
        return Err(RendererError { code: "native_vulkan_completion_timeout", stage: "completion", fault: RendererFault::CompletionTimeout, message: "GPU completion was not observed within the frame observation budget; submission retained".into(), element: None });
    }
    Ok(complete)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejected_submit_validation_keeps_acquired_owner_until_successful_transfer() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct Owner(Arc<AtomicUsize>);
        impl Drop for Owner {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        let dropped = Arc::new(AtomicUsize::new(0));
        let mut acquired = Some(Owner(dropped.clone()));
        for _ in 0..2 {
            assert!(
                take_validated(&mut acquired, |_| Err(RendererError::at(
                    "submit",
                    "mismatched device fixture"
                )))
                .is_err()
            );
            assert!(acquired.is_some());
            assert_eq!(dropped.load(Ordering::Relaxed), 0);
        }
        let submission = take_validated(&mut acquired, |_| Ok(())).expect("validated transfer");
        assert!(acquired.is_none());
        assert!(
            take_validated(&mut acquired, |_| Ok(())).is_err(),
            "no duplicate acquisition"
        );
        assert_eq!(dropped.load(Ordering::Relaxed), 0);
        drop(submission);
        assert_eq!(dropped.load(Ordering::Relaxed), 1);
    }
    #[test]
    fn watchdog_rejects_stalled_work_but_accepts_observed_completion_after_host_delay() {
        let submitted = Instant::now();
        assert!(
            !observe_completion(false, submitted, submitted + Duration::from_millis(4999))
                .expect("within budget")
        );
        let error = observe_completion(false, submitted, submitted + Duration::from_secs(5))
            .expect_err("stall must stop new frames");
        assert_eq!(error.fault, RendererFault::CompletionTimeout);
        assert!(
            observe_completion(true, submitted, submitted + Duration::from_secs(20))
                .expect("actual completion wins over elapsed time")
        );
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct Owner(Arc<AtomicUsize>);
        impl Drop for Owner {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        let dropped = Arc::new(AtomicUsize::new(0));
        let submission = Owner(dropped.clone());
        let acquired = Owner(dropped.clone());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            completion_dependency(|| {
                let _borrowed = (&submission, &acquired);
                panic!("DeviceLost in an untyped dependency payload");
            })
        }))
        .expect("borrowed dependency panic must become a diagnostic without unwinding owners")
        .expect_err("unknown panic must not become completed work");
        assert_eq!(result.code, "native_vulkan_completion_dependency_panicked");
        assert_eq!(result.fault, RendererFault::Other);
        assert_eq!(dropped.load(Ordering::Relaxed), 0);
        let typed = completion_dependency(|| {
            Err(RendererError::caused_by(
                "completion",
                VulkanError::DeviceLost,
            ))
        })
        .expect_err("typed driver errors remain typed");
        assert_eq!(typed.fault, RendererFault::DeviceLost);
        drop((submission, acquired));
        assert_eq!(dropped.load(Ordering::Relaxed), 2);
    }
}
