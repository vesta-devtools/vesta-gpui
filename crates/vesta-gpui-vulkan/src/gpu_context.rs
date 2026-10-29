//! Application GPU ownership. Window surfaces and in-flight paint remain separate.
#![forbid(unsafe_code)]
use crate::RendererError;
use std::sync::Arc;
use vulkano::{
    Validated, VulkanError, VulkanLibrary,
    command_buffer::allocator::StandardCommandBufferAllocator,
    device::{
        Device, DeviceCreateInfo, DeviceExtensions, Queue, QueueCreateInfo, QueueFlags,
        physical::PhysicalDeviceType,
    },
    instance::{Instance, InstanceCreateInfo},
    swapchain::Surface,
};
use winit::{event_loop::ActiveEventLoop, window::Window};

// The fallible driver query must distinguish unsupported from failed.
fn presentation_family(
    families: impl IntoIterator<Item = (u32, QueueFlags)>,
    mut supports: impl FnMut(u32) -> Result<bool, Validated<VulkanError>>,
) -> Result<Option<u32>, RendererError> {
    for (family, flags) in families {
        if flags.intersects(QueueFlags::GRAPHICS)
            && supports(family)
                .map_err(|error| RendererError::caused_by("presentation_support", error))?
        {
            return Ok(Some(family));
        }
    }
    Ok(None)
}

/// Explicit initial-query failure on a healthy GPU; absent from normal builds.
#[cfg(feature = "fault-injection")]
#[derive(Clone, Copy)]
pub enum InitialPresentationFault {
    SurfaceLost,
    OutOfHostMemory,
}

/// Opaque shared native context; it contains no first-window handle or surface.
pub struct GpuContext {
    pub(crate) instance: Arc<Instance>,
    pub(crate) device: Arc<Device>,
    pub(crate) queue: Arc<Queue>,
    pub(crate) commands: Arc<StandardCommandBufferAllocator>,
    pub(crate) failure: crate::gpu_failure::GpuFailure,
    pub(crate) worker: Arc<crate::recovery::RecoveryWorker>,
    retirement: Arc<crate::retirement::Slot<crate::retirement::Owner>>,
    // Must be last: the worker cannot release its Queue clone until the
    // producer's original Queue/allocator/device fields are gone.
    pub(crate) release: crate::retirement::ReleaseMarker,
}

impl Drop for GpuContext {
    fn drop(&mut self) {
        if !self.worker.on_worker() {
            self.worker
                .retire(&self.retirement, crate::retirement::Owner::context(self));
        }
    }
}

impl GpuContext {
    pub fn retirement_monitor(&self) -> crate::RetirementMonitor {
        self.worker.retirement_monitor()
    }
    pub fn can_create_renderer(&self) -> bool {
        !self.retirement_monitor().pending()
    }
    pub fn recovery_seed(&self) -> crate::recovery::RecoverySeed {
        crate::recovery::RecoverySeed {
            instance: self.instance.clone(),
            physical: self.device.physical_device().clone(),
            family: self.queue.queue_family_index(),
            worker: self.worker.clone(),
        }
    }
    pub(crate) fn replacement(
        seed: &crate::recovery::RecoverySeed,
    ) -> Result<Arc<Self>, RendererError> {
        Self::create(seed, "recovery_device")
    }
    pub(crate) fn create(
        seed: &crate::recovery::RecoverySeed,
        stage: &'static str,
    ) -> Result<Arc<Self>, RendererError> {
        let (device, mut queues) = Device::new(
            seed.physical.clone(),
            DeviceCreateInfo {
                enabled_extensions: DeviceExtensions {
                    khr_swapchain: true,
                    ..DeviceExtensions::empty()
                },
                queue_create_infos: vec![QueueCreateInfo {
                    queue_family_index: seed.family,
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .map_err(|e| RendererError::caused_by(stage, e))?;
        let queue = queues
            .next()
            .ok_or_else(|| RendererError::at(stage, "missing GPU queue"))?;
        let commands = Arc::new(StandardCommandBufferAllocator::new(
            device.clone(),
            Default::default(),
        ));
        Ok(Arc::new(Self {
            retirement: seed.worker.register_retirement(),
            release: Default::default(),
            instance: seed.instance.clone(),
            device,
            queue,
            commands,
            failure: Default::default(),
            worker: seed.worker.clone(),
        }))
    }
    pub fn failure(&self) -> Option<&RendererError> {
        self.failure.error()
    }
    /// Blocking compatibility factory. Event-loop hosts use `RendererCreation`
    /// so Device and pipeline construction run on the application GPU worker.
    pub fn new(
        event_loop: &ActiveEventLoop,
        window: Arc<Window>,
    ) -> Result<Arc<Self>, RendererError> {
        Self::for_window(event_loop, window).map(|(context, _)| context)
    }

    pub(crate) fn for_window(
        event_loop: &ActiveEventLoop,
        window: Arc<Window>,
    ) -> Result<(Arc<Self>, Arc<Surface>), RendererError> {
        let (seed, surface) = Self::prepare(
            event_loop,
            window,
            #[cfg(feature = "fault-injection")]
            None,
        )?;
        Self::create(&seed, "device").map(|gpu| (gpu, surface))
    }

    #[cfg(feature = "fault-injection")]
    pub(crate) fn for_window_with_fault(
        event_loop: &ActiveEventLoop,
        window: Arc<Window>,
        fault: InitialPresentationFault,
    ) -> Result<(Arc<Self>, Arc<Surface>), RendererError> {
        let (seed, surface) = Self::prepare(event_loop, window, Some(fault))?;
        Self::create(&seed, "device").map(|gpu| (gpu, surface))
    }

    pub(crate) fn prepare(
        event_loop: &ActiveEventLoop,
        window: Arc<Window>,
        #[cfg(feature = "fault-injection")] fault: Option<InitialPresentationFault>,
    ) -> Result<(crate::recovery::RecoverySeed, Arc<Surface>), RendererError> {
        let library = VulkanLibrary::new().map_err(|e| RendererError::caused_by("loader", e))?;
        let extensions = Surface::required_extensions(event_loop)
            .map_err(|e| RendererError::caused_by("surface_extensions", e))?;
        let instance = Instance::new(
            library,
            InstanceCreateInfo {
                enabled_extensions: extensions,
                ..Default::default()
            },
        )
        .map_err(|e| RendererError::caused_by("instance", e))?;
        let surface = Surface::from_window(instance.clone(), window)
            .map_err(|e| RendererError::caused_by("surface", e))?;
        let extensions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::empty()
        };
        let mut candidates = Vec::new();
        for physical in instance
            .enumerate_physical_devices()
            .map_err(|e| RendererError::caused_by("enumeration", e))?
            .filter(|p| p.supported_extensions().contains(&extensions))
        {
            let query = presentation_family(
                physical
                    .queue_family_properties()
                    .iter()
                    .enumerate()
                    .map(|(i, queue)| (i as u32, queue.queue_flags)),
                |family| {
                    #[cfg(feature = "fault-injection")]
                    if let Some(fault) = fault {
                        return Err(Validated::Error(match fault {
                            InitialPresentationFault::SurfaceLost => VulkanError::SurfaceLost,
                            InitialPresentationFault::OutOfHostMemory => {
                                VulkanError::OutOfHostMemory
                            }
                        }));
                    }
                    physical.surface_support(family, &surface)
                },
            );
            #[cfg(feature = "fault-injection")]
            if fault.is_some()
                && let Err(error) = &query
            {
                eprintln!(
                    "UI-R09 presentation query: code={} stage={} fault={:?} device_created=false worker_started=false",
                    error.code, error.stage, error.fault,
                );
            }
            if let Some(family) = query? {
                candidates.push((physical, family));
            }
        }
        let (physical, family) = candidates
            .into_iter()
            .min_by_key(|(p, _)| {
                let rank = match p.properties().device_type {
                    PhysicalDeviceType::DiscreteGpu => 0,
                    PhysicalDeviceType::IntegratedGpu => 1,
                    PhysicalDeviceType::VirtualGpu => 2,
                    PhysicalDeviceType::Cpu => 3,
                    _ => 4,
                };
                (rank, p.properties().device_name.clone())
            })
            .ok_or_else(|| RendererError::at("adapter", "no graphics/presentation queue"))?;
        eprintln!(
            "Vesta native Vulkan GPU context: {} / {:?}",
            physical.properties().device_name,
            physical.api_version()
        );
        // Failed capability queries own no Device/Queue and need no worker.
        let worker = crate::recovery::RecoveryWorker::new()?;
        Ok((
            crate::recovery::RecoverySeed {
                instance,
                physical,
                family,
                worker,
            },
            surface,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RendererFault;

    // Query failure must not become unsupported or reach a later family.
    // Also protects graphics filtering and the first supported family's order.
    #[test]
    fn presentation_selection_preserves_query_errors_and_family_order() {
        let families = [
            (0, QueueFlags::TRANSFER),
            (1, QueueFlags::GRAPHICS),
            (2, QueueFlags::GRAPHICS),
            (3, QueueFlags::GRAPHICS),
        ];
        let mut queried = Vec::new();
        let selected = presentation_family(families, |family| {
            queried.push(family);
            Ok(family >= 2)
        })
        .expect("successful supported query");
        assert_eq!(selected, Some(2));
        assert_eq!(queried, [1, 2]);
        assert_eq!(
            presentation_family(families, |_| Ok(false)).expect("unsupported query is successful"),
            None
        );

        for (cause, fault, code) in [
            (
                VulkanError::SurfaceLost,
                RendererFault::SurfaceLost,
                "native_vulkan_surface_lost",
            ),
            (
                VulkanError::OutOfHostMemory,
                RendererFault::Other,
                "native_vulkan_failed",
            ),
        ] {
            queried.clear();
            let message = cause.to_string();
            let error = presentation_family(families, |family| {
                queried.push(family);
                Err(Validated::Error(cause))
            })
            .expect_err("driver query failures must not be swallowed");
            assert_eq!(queried, [1]);
            assert_eq!(error.stage, "presentation_support");
            assert_eq!(error.code, code);
            assert_eq!(error.fault, fault);
            assert!(error.message.contains(&message));
        }
        let validation = presentation_family(families, |_| {
            Err(Validated::ValidationError(Box::new(
                vulkano::ValidationError {
                    context: "queue_family_index".into(),
                    problem: "fixture validation cause".into(),
                    ..Default::default()
                },
            )))
        })
        .expect_err("validation failures must not be swallowed");
        assert_eq!(validation.fault, RendererFault::Other);
        assert_eq!(validation.stage, "presentation_support");
        assert!(validation.message.contains("fixture validation cause"));
    }
}
