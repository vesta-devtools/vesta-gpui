//! Event-thread Surface preparation, worker-owned GPU factories. ADR-0343.
use crate::{
    GpuContext, RecoveryJob, RecoveryPoll, RecoveryTarget, RendererError, RetirementMonitor,
};
use std::{sync::Arc, time::Duration};
use winit::{event_loop::ActiveEventLoop, window::Window};

/// Nonblocking GPU creation ticket. Dropping it cancels delivery; the worker
/// retains and retires late resources. `Ready` contains exactly one renderer.
pub struct RendererCreation(RecoveryJob);

#[cfg(feature = "fault-injection")]
#[derive(Clone, Copy)]
pub enum RendererCreationFixture {
    DelayedDelivery,
    CreationFailure,
    ObservationTimeout,
}

impl RendererCreation {
    pub fn start(event_loop: &ActiveEventLoop, window: Arc<Window>) -> Result<Self, RendererError> {
        Self::initial(
            event_loop,
            window,
            false,
            #[cfg(feature = "fault-injection")]
            None,
            #[cfg(feature = "fault-injection")]
            None,
        )
    }
    #[cfg(feature = "viewport")]
    pub fn start_with_scene(
        event_loop: &ActiveEventLoop,
        window: Arc<Window>,
        scene: bool,
    ) -> Result<Self, RendererError> {
        Self::initial(
            event_loop,
            window,
            scene,
            #[cfg(feature = "fault-injection")]
            None,
            #[cfg(feature = "fault-injection")]
            None,
        )
    }
    #[cfg(feature = "fault-injection")]
    pub fn start_fixture(
        event_loop: &ActiveEventLoop,
        window: Arc<Window>,
        scene: bool,
        presentation: Option<crate::InitialPresentationFault>,
        fixture: Option<RendererCreationFixture>,
    ) -> Result<Self, RendererError> {
        Self::initial(event_loop, window, scene, presentation, fixture)
    }
    fn initial(
        event_loop: &ActiveEventLoop,
        window: Arc<Window>,
        scene: bool,
        #[cfg(feature = "fault-injection")] presentation: Option<crate::InitialPresentationFault>,
        #[cfg(feature = "fault-injection")] fixture: Option<RendererCreationFixture>,
    ) -> Result<Self, RendererError> {
        let (seed, surface) = GpuContext::prepare(
            event_loop,
            window.clone(),
            #[cfg(feature = "fault-injection")]
            presentation,
        )?;
        let budget = Duration::from_secs(10);
        #[cfg(feature = "fault-injection")]
        let budget = if matches!(fixture, Some(RendererCreationFixture::ObservationTimeout)) {
            Duration::from_millis(25)
        } else {
            budget
        };
        RecoveryJob::create(
            seed,
            None,
            RecoveryTarget::prepared(window, surface, scene),
            budget,
            #[cfg(feature = "fault-injection")]
            fixture.map(|fixture| match fixture {
                RendererCreationFixture::CreationFailure => crate::RecoveryFixture::CreationFailure,
                RendererCreationFixture::DelayedDelivery
                | RendererCreationFixture::ObservationTimeout => {
                    crate::RecoveryFixture::DelayedCreation
                }
            }),
        )
        .map(Self)
        .map_err(|rejected| rejected.error)
    }
    pub fn with_gpu(window: Arc<Window>, gpu: Arc<GpuContext>) -> Result<Self, RendererError> {
        if !gpu.can_create_renderer() {
            return Err(RendererError::at(
                "creation",
                "GPU worker or retirement is still pending",
            ));
        }
        if let Some(error) = gpu.failure() {
            return Err(error.clone());
        }
        RecoveryJob::create(
            gpu.recovery_seed(),
            Some(gpu),
            RecoveryTarget::without_renderer(window, false),
            Duration::from_secs(10),
            #[cfg(feature = "fault-injection")]
            None,
        )
        .map(Self)
        .map_err(|rejected| rejected.error)
    }
    pub fn retirement_monitor(&self) -> RetirementMonitor {
        self.0.seed().worker.retirement_monitor()
    }
    pub fn poll(&mut self) -> RecoveryPoll {
        self.0.poll()
    }
    pub fn cancel(&mut self) {
        self.0.cancel();
    }
    pub fn worker_finished(&self) -> bool {
        self.0.worker_finished()
    }
    #[cfg(feature = "fault-injection")]
    pub fn fixture_prepared(&self) -> bool {
        self.0.creation_prepared()
    }
}
