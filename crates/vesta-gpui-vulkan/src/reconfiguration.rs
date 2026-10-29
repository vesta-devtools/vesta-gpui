//! Shared host ownership and current-configuration gate. ADR-0347.
use crate::{GpuContext, RecoveryJob, RecoveryPoll, RendererError, WindowRenderer};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetMode {
    Ui,
    #[cfg(feature = "viewport")]
    Scene,
}
impl TargetMode {
    pub(crate) fn is_scene(self) -> bool {
        #[cfg(feature = "viewport")]
        {
            self == Self::Scene
        }
        #[cfg(not(feature = "viewport"))]
        {
            false
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetConfiguration {
    pub extent: [u32; 2],
    pub mode: TargetMode,
}
impl TargetConfiguration {
    pub fn ui(extent: [u32; 2]) -> Self {
        Self {
            extent,
            mode: TargetMode::Ui,
        }
    }
    #[cfg(feature = "viewport")]
    pub fn with_scene(mut self, scene: bool) -> Self {
        self.mode = if scene {
            TargetMode::Scene
        } else {
            TargetMode::Ui
        };
        self
    }
    fn drawable(self) -> bool {
        self.extent.into_iter().all(|value| value != 0)
    }
}

/// Content admission is separate from an explicitly retained target capability.
/// Legacy callers infer from content; an explicit UI target cannot accept 3D.
#[cfg(feature = "viewport")]
pub(crate) fn frame_mode(
    scene_present: bool,
    prepared: Option<TargetMode>,
) -> Result<TargetMode, RendererError> {
    let mode = prepared.unwrap_or(if scene_present {
        TargetMode::Scene
    } else {
        TargetMode::Ui
    });
    if scene_present && mode == TargetMode::Ui {
        return Err(RendererError::at(
            "frame_scene_mode",
            "3D content requires a Scene-capable target",
        ));
    }
    Ok(mode)
}

/// Both native hosts use this coordinator with their existing renderer slot.
/// It owns no author/UI model. A late target can be reused as an update owner,
/// but only a current configuration may produce a frame or resume input.
#[derive(Default)]
pub struct RendererUpdates {
    job: Option<RecoveryJob>,
    context: Option<Arc<GpuContext>>,
    counts: [u32; 2],
    blocked: bool,
    retry_at: Option<Instant>,
    window: Option<winit::window::WindowId>,
    #[cfg(feature = "fault-injection")]
    fixture: Option<crate::RecoveryFixture>,
    #[cfg(feature = "fault-injection")]
    short_budget: bool,
}
impl RendererUpdates {
    pub fn pending(&self) -> bool {
        self.job.is_some()
    }
    pub fn worker_finished(&self) -> bool {
        self.job.as_ref().is_none_or(RecoveryJob::worker_finished)
    }
    #[cfg(feature = "fault-injection")]
    pub fn fixture_prepared(&self) -> bool {
        self.job
            .as_ref()
            .is_some_and(RecoveryJob::creation_prepared)
    }
    pub fn cancel(&mut self) {
        if let Some(job) = &mut self.job {
            job.cancel();
        }
    }
    pub fn blocked(&self) -> bool {
        self.blocked
    }
    pub fn context(&self) -> Option<Arc<GpuContext>> {
        self.context.clone()
    }
    pub fn counts(&self) -> [u32; 2] {
        self.counts
    }
    pub fn presented(&mut self) {
        self.blocked = false;
    }
    /// Caller first captures the context/seed and waits for the worker barrier.
    pub fn reset_for_recovery(&mut self) -> [u32; 2] {
        self.job = None;
        self.context = None;
        self.retry_at = None;
        self.blocked = true;
        std::mem::take(&mut self.counts)
    }
    #[cfg(feature = "fault-injection")]
    pub fn inject_fixture(&mut self, fixture: crate::RecoveryFixture, short_budget: bool) {
        self.fixture = Some(fixture);
        self.short_budget = short_budget;
    }
    pub fn poll(
        &mut self,
        renderer: &mut Option<Box<WindowRenderer>>,
        desired: TargetConfiguration,
    ) -> Result<Option<Instant>, RendererError> {
        let Some(job) = &mut self.job else {
            return Ok(self.retry_at);
        };
        match job.poll() {
            RecoveryPoll::Pending { deadline } => return Ok(Some(deadline)),
            RecoveryPoll::Ready(renderers) => {
                for restored in renderers {
                    if self.window != Some(restored.window_id()) {
                        return Err(RendererError {
                            code: "native_vulkan_reconfiguration_identity_failed",
                            stage: "host",
                            fault: crate::RendererFault::Other,
                            message: "target result belongs to another window".into(),
                            element: None,
                        });
                    }
                    let current = !restored.needs_configuration(desired);
                    #[cfg(feature = "fault-injection")]
                    eprintln!(
                        "UI-R11 result: window={:?} current_configuration={current} extent={:?} mode={:?}",
                        restored.window_id(),
                        restored.swapchain.image_extent(),
                        desired.mode
                    );
                    self.blocked = true;
                    self.retry_at = (!current && desired.drawable())
                        .then(|| Instant::now() + Duration::from_millis(16));
                    self.counts = [0; 2];
                    *renderer = Some(Box::new(restored));
                }
            }
            RecoveryPoll::Failed(error) => return Err(error),
            RecoveryPoll::Finished => (),
        }
        if job.worker_finished() {
            self.job = None;
            if renderer.is_some() {
                self.context = None;
            }
        }
        Ok(self
            .job
            .as_ref()
            .map(|_| Instant::now() + Duration::from_millis(16))
            .or(self.retry_at))
    }
    pub fn prepare(
        &mut self,
        renderer: &mut Option<Box<WindowRenderer>>,
        desired: TargetConfiguration,
    ) -> Result<bool, RendererError> {
        self.poll(renderer, desired)?;
        if self.job.is_some() || !desired.drawable() {
            return Ok(false);
        }
        let Some(current) = renderer.as_mut() else {
            return Ok(false);
        };
        if let Some(error) = current.gpu.failure().or(current.failed.as_ref()) {
            return Err(error.clone());
        }
        if !current.needs_configuration(desired) {
            self.retry_at = None;
            return Ok(true);
        }
        current.cpu_profile = None;
        self.blocked = true;
        if self.retry_at.is_some_and(|at| Instant::now() < at) {
            return Ok(false);
        }
        let mut current = renderer.take().expect("renderer checked above");
        self.context = Some(current.gpu_context());
        self.window = Some(current.window_id());
        self.counts[0] = current.swapchain_recreations();
        #[cfg(feature = "viewport")]
        {
            self.counts[1] = current.scene_frames();
        }
        let configuration = desired;
        current.cpu_profile = None;
        #[cfg(feature = "fault-injection")]
        let fixture = self.fixture.take();
        match RecoveryJob::reconfigure(
            *current,
            configuration,
            #[cfg(feature = "fault-injection")]
            fixture,
            #[cfg(feature = "fault-injection")]
            self.short_budget,
        ) {
            Ok(job) => {
                self.job = Some(job);
                self.retry_at = None;
                #[cfg(feature = "fault-injection")]
                {
                    self.short_budget = false;
                }
            }
            Err(rejected) => {
                for target in rejected.targets {
                    if let Some(owned) = target.into_renderer() {
                        *renderer = Some(Box::new(owned));
                    }
                }
                self.counts = [0; 2];
                self.context = None;
                if rejected.error.code != "native_vulkan_worker_busy" {
                    return Err(rejected.error);
                }
                #[cfg(feature = "fault-injection")]
                {
                    self.fixture = fixture;
                }
                self.retry_at = Some(Instant::now() + Duration::from_millis(16));
            }
        }
        Ok(false)
    }
}

#[cfg(all(test, feature = "viewport"))]
mod frame_mode_tests {
    use super::*;
    // Protects the prepare/submit boundary that otherwise retries forever with
    // no main scene, and rejects the inverse mismatch before depth is needed.
    #[test]
    fn explicit_target_admits_ui_on_scene_capacity_without_weakening_legacy_or_depth_gates() {
        assert_eq!(frame_mode(false, None).expect("legacy UI"), TargetMode::Ui);
        assert_eq!(
            frame_mode(true, None).expect("legacy scene"),
            TargetMode::Scene
        );
        assert_eq!(
            frame_mode(false, Some(TargetMode::Scene)).expect("UI with capture capacity"),
            TargetMode::Scene
        );
        assert_eq!(
            frame_mode(true, Some(TargetMode::Scene)).expect("actual scene"),
            TargetMode::Scene
        );
        assert_eq!(
            frame_mode(false, Some(TargetMode::Ui)).expect("explicit UI"),
            TargetMode::Ui
        );
        assert_eq!(
            frame_mode(true, Some(TargetMode::Ui))
                .expect_err("no depth-capability fallback")
                .stage,
            "frame_scene_mode"
        );
    }
}
