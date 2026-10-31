//! Standalone native Vulkan rendering for main and floating windows.
use std::{
    sync::{Arc, OnceLock},
    time::Instant,
};
use vesta_gpui::render::{ImageDiagnostic, ImageResolver, RenderOutput};
use vesta_gpui::{PaintList, TextInputContext, TextRasterizer, Viewport};
use vesta_gpui_vulkan::{
    RecoveryPoll, RecoveryTarget, RendererCreation, RendererError, RendererFault, RendererUpdates,
    TargetConfiguration, WindowRenderer,
};
use vesta_viewport::render::ViewportRect;
use vesta_viewport::{Camera, ViewportScene};
use winit::{event_loop::ActiveEventLoop, window::Window};

pub(super) fn recovery_fault() -> Option<bool> {
    if std::env::args().any(|a| a == "--smoke-device-loss") {
        Some(true)
    } else if std::env::args().any(|a| a == "--smoke-surface-loss") {
        Some(false)
    } else {
        None
    }
}
pub(super) fn recovery_retry_enabled() -> bool {
    std::env::args().any(|a| {
        matches!(
            a.as_str(),
            "--smoke-recovery-timeout"
                | "--smoke-recovery-cancel"
                | "--smoke-recovery-create-failure"
        )
    })
}
pub(super) fn recovery_cancel_enabled() -> bool {
    std::env::args().any(|a| a == "--smoke-recovery-cancel")
}
pub(super) fn recovery_close_enabled() -> bool {
    std::env::args().any(|a| a == "--smoke-recovery-close-floating")
}
fn render_error(message: &str) -> RendererError {
    RendererError {
        code: "standalone_render_failed",
        stage: "host",
        fault: RendererFault::Other,
        message: message.into(),
        element: None,
    }
}

pub(super) fn acquire_faults_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::args().any(|a| a == "--smoke-acquire-failures"))
}

pub(super) fn presentation_fault_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::args().any(|a| a == "--smoke-present-out-of-date"))
}

pub(super) fn recording_fault_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::args().any(|a| a == "--smoke-record-failure"))
}
fn completion_fault_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::args()
            .any(|a| a == "--smoke-completion-panic" || a == "--smoke-retirement-completion-panic")
    })
}
#[cfg(feature = "gpu-fault-injection")]
pub(super) fn completion_retirement_fault_enabled() -> bool {
    std::env::args().any(|a| a == "--smoke-retirement-completion-panic")
}
pub(super) fn retirement_probe_enabled() -> bool {
    std::env::args().any(|a| a == "--smoke-retirement")
}

fn initial_presentation_fault_enabled() -> bool {
    std::env::args().any(|a| {
        a == "--smoke-initial-surface-query-loss" || a == "--smoke-initial-surface-query-memory"
    })
}
pub(super) fn creation_fixture_enabled() -> bool {
    std::env::args().any(|a| {
        matches!(
            a.as_str(),
            "--smoke-create-delay"
                | "--smoke-create-failure"
                | "--smoke-create-timeout"
                | "--smoke-create-close"
        )
    })
}

pub(super) fn validate_args() -> Result<(), String> {
    let target_flags = std::env::args()
        .filter(|arg| arg.starts_with("--smoke-target-"))
        .collect::<Vec<_>>();
    if !target_flags.is_empty()
        && (target_flags.len() != 1
            || !matches!(
                target_flags[0].as_str(),
                "--smoke-target-delay"
                    | "--smoke-target-resize"
                    | "--smoke-target-close"
                    | "--smoke-target-timeout"
                    | "--smoke-target-cancel"
                    | "--smoke-target-failure"
                    | "--smoke-target-out-of-date"
                    | "--smoke-target-device-loss"
                    | "--smoke-target-surface-loss"
            )
            || !cfg!(feature = "gpu-fault-injection")
            || !std::env::args().any(|a| a == "--smoke")
            || creation_fixture_enabled()
            || initial_presentation_fault_enabled()
            || recovery_fault().is_some()
            || recovery_retry_enabled()
            || completion_fault_enabled()
            || recording_fault_enabled()
            || presentation_fault_enabled()
            || acquire_faults_enabled()
            || retirement_probe_enabled()
            || super::gpu_lifecycle::enabled()
            || super::demo_images::faults_enabled()
            || std::env::args().any(|a| a == "--smoke-mode-cycle" || a == "--smoke-resize"))
    {
        return Err("standalone_target_smoke_invalid: use one isolated target fixture with gpu-fault-injection and --smoke".into());
    }
    if creation_fixture_enabled()
        && (std::env::args()
            .filter(|a| {
                matches!(
                    a.as_str(),
                    "--smoke-create-delay"
                        | "--smoke-create-failure"
                        | "--smoke-create-timeout"
                        | "--smoke-create-close"
                )
            })
            .count()
            != 1
            || initial_presentation_fault_enabled()
            || recovery_fault().is_some()
            || recovery_retry_enabled()
            || completion_fault_enabled()
            || recording_fault_enabled()
            || presentation_fault_enabled()
            || acquire_faults_enabled())
    {
        return Err("standalone_creation_smoke_invalid: use one isolated creation fixture".into());
    }
    if initial_presentation_fault_enabled()
        && (std::env::args()
            .filter(|a| {
                a == "--smoke-initial-surface-query-loss"
                    || a == "--smoke-initial-surface-query-memory"
            })
            .count()
            != 1
            || recovery_fault().is_some()
            || recovery_retry_enabled()
            || recovery_close_enabled()
            || completion_fault_enabled()
            || recording_fault_enabled()
            || presentation_fault_enabled()
            || acquire_faults_enabled()
            || retirement_probe_enabled()
            || super::gpu_lifecycle::enabled()
            || super::demo_images::faults_enabled()
            || std::env::args().any(|a| {
                a == "--smoke-mode-cycle" || a == "--smoke-floating" || a == "--smoke-resize"
            }))
    {
        return Err("standalone_initial_query_smoke_invalid: use one isolated initial presentation query probe without other fault/lifecycle flags".into());
    }
    if completion_fault_enabled()
        && (std::env::args()
            .filter(|a| {
                a == "--smoke-completion-panic" || a == "--smoke-retirement-completion-panic"
            })
            .count()
            != 1
            || recovery_fault().is_some()
            || recording_fault_enabled()
            || presentation_fault_enabled()
            || acquire_faults_enabled()
            || retirement_probe_enabled()
            || super::gpu_lifecycle::enabled()
            || super::demo_images::faults_enabled()
            || std::env::args().any(|a| a == "--smoke-mode-cycle"))
    {
        return Err("standalone_completion_smoke_invalid: use the isolated borrowed completion panic probe without other fault/lifecycle flags".into());
    }
    if retirement_probe_enabled()
        && (recovery_fault().is_some()
            || recording_fault_enabled()
            || presentation_fault_enabled()
            || acquire_faults_enabled())
    {
        return Err("standalone_retirement_smoke_invalid: use a healthy GPU shutdown probe without fault/recovery flags".into());
    }
    if recovery_close_enabled()
        && (recovery_fault() != Some(true)
            || !std::env::args().any(|a| a == "--smoke-floating")
            || recovery_retry_enabled())
    {
        return Err("standalone_recovery_smoke_invalid: close fixture requires --smoke-device-loss --smoke-floating without a retry fixture".into());
    }
    if recovery_retry_enabled()
        && (recovery_fault().is_none()
            || std::env::args()
                .filter(|a| {
                    matches!(
                        a.as_str(),
                        "--smoke-recovery-timeout"
                            | "--smoke-recovery-cancel"
                            | "--smoke-recovery-create-failure"
                    )
                })
                .count()
                != 1)
    {
        return Err(
            "standalone_recovery_smoke_invalid: one recovery retry fixture requires a loss scope"
                .into(),
        );
    }
    if (acquire_faults_enabled()
        || presentation_fault_enabled()
        || recording_fault_enabled()
        || completion_fault_enabled()
        || initial_presentation_fault_enabled()
        || creation_fixture_enabled()
        || retirement_probe_enabled()
        || recovery_fault().is_some())
        && (!cfg!(feature = "gpu-fault-injection") || !std::env::args().any(|a| a == "--smoke"))
    {
        return Err(
            "standalone_acquire_smoke_invalid: build with gpu-fault-injection and use --smoke"
                .into(),
        );
    }
    if std::env::args().any(|a| a == "--renderer" || a.starts_with("--renderer=")) {
        return Err(
            "standalone_renderer_option_removed: Standalone uses native Vulkan; remove --renderer"
                .into(),
        );
    }
    let frame_args = std::env::args()
        .filter(|a| a.starts_with("--smoke-frames="))
        .collect::<Vec<_>>();
    if recovery_fault().is_some()
        && (super::gpu_lifecycle::enabled()
            || recording_fault_enabled()
            || presentation_fault_enabled()
            || acquire_faults_enabled()
            || (std::env::args().any(|a| a == "--smoke-device-loss")
                && std::env::args().any(|a| a == "--smoke-surface-loss"))
            || frame_args.iter().any(|a| {
                a.trim_start_matches("--smoke-frames=")
                    .parse::<u32>()
                    .map_or(true, |n| n < 12)
            }))
    {
        return Err("standalone_recovery_smoke_invalid: select one recovery scope, at least 12 frames, without other GPU fault/lifecycle probes".into());
    }
    if super::gpu_lifecycle::enabled()
        && (!std::env::args().any(|a| a == "--smoke")
            || std::env::args().any(|a| {
                a == "--mode=3d" || a == "--smoke-mode-cycle" || a == "--smoke-image-failures"
            })
            || frame_args.iter().any(|a| {
                a.trim_start_matches("--smoke-frames=")
                    .parse::<u32>()
                    .map_or(true, |n| n < 12)
            }))
    {
        return Err(
            "standalone_gpu_lifecycle_smoke_invalid: use --smoke in 2D with at least 12 frames"
                .into(),
        );
    }
    if super::demo_images::faults_enabled()
        && (!std::env::args().any(|a| a == "--smoke")
            || std::env::args().any(|a| a == "--mode=3d" || a == "--smoke-mode-cycle")
            || frame_args.iter().any(|a| {
                a.trim_start_matches("--smoke-frames=")
                    .parse::<u32>()
                    .map_or(true, |n| n < 7)
            }))
    {
        return Err(
            "standalone_image_smoke_invalid: use --smoke in 2D with at least 7 frames".into(),
        );
    }
    if frame_args.len() > 1
        || frame_args.iter().any(|a| {
            a.trim_start_matches("--smoke-frames=")
                .parse::<u32>()
                .map_or(true, |n| !(3..=10_000).contains(&n))
        })
    {
        return Err("standalone_smoke_frames_invalid: choose 3..10000 frames".into());
    }
    Ok(())
}
pub(super) fn smoke_frame_limit(gallery: bool) -> u64 {
    std::env::args()
        .find_map(|a| {
            a.strip_prefix("--smoke-frames=")
                .and_then(|n| n.parse().ok())
        })
        .unwrap_or(
            if super::gpu_lifecycle::enabled() || recovery_fault().is_some() {
                12
            } else if super::demo_images::faults_enabled() {
                7
            } else if gallery || std::env::args().any(|a| a == "--smoke-resize") {
                3
            } else {
                2
            },
        )
}
pub(super) fn smoke_mode_action(frame: u64) -> Option<super::AcceptanceMode> {
    if !std::env::args().any(|a| a == "--smoke-mode-cycle") {
        return None;
    }
    match frame {
        3 => Some(super::AcceptanceMode::Viewport3d),
        5 => Some(super::AcceptanceMode::Gallery2d),
        _ => None,
    }
}
#[derive(Clone, Copy)]
pub(super) struct RenderOptions {
    pub clear: Option<[f32; 4]>,
    pub selection: vesta_gpui::Color,
    pub caret: vesta_gpui::Color,
    pub caret_visible: bool,
}
pub(super) struct RenderFrame<'a> {
    pub viewport: Viewport,
    pub paint: &'a PaintList,
    pub context: &'a mut TextInputContext,
    pub rasterizer: &'a mut TextRasterizer,
    pub images: Option<&'a dyn ImageResolver>,
    pub options: RenderOptions,
    pub scene: Option<(Camera, &'a ViewportScene, [f32; 4])>,
    pub finish: bool,
    pub cpu_profile: Option<&'a mut Option<vesta_gpui_vulkan::FrameCpuProfile>>,
    pub gpu_profile: &'a mut super::gpu_profile::GpuProfiler,
    pub profile_context: super::frame_profile::FrameContext,
}
pub(super) struct GpuState {
    #[cfg(feature = "gpu-fault-injection")]
    retirement_capture: Option<vesta_gpui_vulkan::CaptureTicket>,
    renderer: Option<Box<vesta_gpui_vulkan::WindowRenderer>>,
    creation: Option<RendererCreation>,
    updates: RendererUpdates,
    #[cfg(feature = "gpu-fault-injection")]
    creation_primary: bool,
    awaiting_first_frame: bool,
    creation_resize_pending: bool,
    #[cfg(feature = "gpu-fault-injection")]
    creation_pending_ticks: u32,
    retirement: vesta_gpui_vulkan::RetirementMonitor,
    retired_recreations: u32,
    retired_scene_frames: u32,
    image_diagnostics: Vec<ImageDiagnostic>,
    acquire_retry_attempts: u32,
    acquire_retry_reasons: [bool; 2],
}

#[cfg(feature = "gpu-fault-injection")]
impl Drop for GpuState {
    fn drop(&mut self) {
        if completion_retirement_fault_enabled() {
            let armed = self
                .renderer
                .as_mut()
                .is_some_and(|renderer| renderer.inject_retirement_completion_panic());
            eprintln!("UI-R08 retirement completion probe: owned_submission_armed={armed}");
        }
    }
}

impl GpuState {
    pub(super) fn capture_renderer(&mut self) -> Result<&mut WindowRenderer, RendererError> {
        if self.updates.blocked() {
            return Err(render_error("capture_target_updating"));
        }
        self.renderer
            .as_deref_mut()
            .ok_or_else(|| render_error("capture_renderer_unavailable"))
    }
    pub fn report_smoke(&self, resize: bool, error: &mut Option<String>) {
        #[cfg(feature = "gpu-fault-injection")]
        if presentation_fault_enabled() {
            let recovered = self
                .renderer
                .as_ref()
                .is_some_and(|renderer| renderer.presentation_fault_recovered());
            eprintln!("UI-R04 presentation fault: recovered={recovered}");
            if !recovered {
                *error = Some(
                    "presentation fault did not retire its owned submission before recovery".into(),
                );
            }
        }
        if acquire_faults_enabled() {
            eprintln!(
                "UI-R03 acquire: {} deferrals; observed reasons {:?}; recovered",
                self.acquire_retry_attempts, self.acquire_retry_reasons
            );
            if self.acquire_retry_attempts < 3 || self.acquire_retry_reasons != [true, true] {
                *error = Some("acquire fault smoke did not exercise all injected branches".into());
            }
        }
        eprintln!(
            "Vesta native Vulkan Gallery: {} swapchain recreations",
            self.swapchain_recreations()
        );
        if std::env::args().any(|a| a == "--mode=3d" || a == "--smoke-mode-cycle") {
            eprintln!(
                "Vesta native Vulkan viewport: {} presented scene frames",
                self.scene_frames()
            );
            if self.scene_frames() < 2 {
                *error = Some("native smoke did not present two 3D scene frames".into());
            }
        }
        if resize && self.swapchain_recreations() == 0 {
            *error = Some("native smoke did not recreate swapchain".into());
        }
    }
    pub fn new(
        event_loop: &ActiveEventLoop,
        window: Arc<Window>,
        scene: bool,
    ) -> Result<Self, String> {
        #[cfg(feature = "gpu-fault-injection")]
        let presentation = if initial_presentation_fault_enabled() {
            Some(
                if std::env::args().any(|a| a == "--smoke-initial-surface-query-loss") {
                    vesta_gpui_vulkan::InitialPresentationFault::SurfaceLost
                } else {
                    vesta_gpui_vulkan::InitialPresentationFault::OutOfHostMemory
                },
            )
        } else {
            None
        };
        #[cfg(feature = "gpu-fault-injection")]
        let fixture = if std::env::args().any(|a| a == "--smoke-create-failure") {
            Some(vesta_gpui_vulkan::RendererCreationFixture::CreationFailure)
        } else if std::env::args().any(|a| a == "--smoke-create-timeout") {
            Some(vesta_gpui_vulkan::RendererCreationFixture::ObservationTimeout)
        } else if creation_fixture_enabled() {
            Some(vesta_gpui_vulkan::RendererCreationFixture::DelayedDelivery)
        } else {
            None
        };
        #[cfg(feature = "gpu-fault-injection")]
        let creation =
            RendererCreation::start_fixture(event_loop, window, scene, presentation, fixture);
        #[cfg(not(feature = "gpu-fault-injection"))]
        let creation = RendererCreation::start_with_scene(event_loop, window, scene);
        creation
            .map(|creation| Self::from_creation(creation, true))
            .map_err(|e| e.to_string())
    }
    fn prepare_renderer(renderer: WindowRenderer) -> WindowRenderer {
        #[cfg(feature = "gpu-fault-injection")]
        let mut renderer = renderer;
        #[cfg(feature = "gpu-fault-injection")]
        if acquire_faults_enabled() {
            renderer.inject_acquire_faults();
        }
        #[cfg(feature = "gpu-fault-injection")]
        if presentation_fault_enabled() {
            renderer.inject_present_out_of_date();
        }
        #[cfg(feature = "gpu-fault-injection")]
        if recording_fault_enabled() {
            renderer.inject_recording_failure();
        }
        #[cfg(feature = "gpu-fault-injection")]
        if completion_fault_enabled() && !completion_retirement_fault_enabled() {
            renderer.inject_completion_poll_panic();
        }
        renderer
    }
    fn from_creation(creation: RendererCreation, primary: bool) -> Self {
        #[cfg(not(feature = "gpu-fault-injection"))]
        let _ = primary;
        Self {
            retirement: creation.retirement_monitor(),
            #[cfg(feature = "gpu-fault-injection")]
            retirement_capture: None,
            renderer: None,
            creation: Some(creation),
            updates: RendererUpdates::default(),
            #[cfg(feature = "gpu-fault-injection")]
            creation_primary: primary,
            awaiting_first_frame: true,
            creation_resize_pending: false,
            #[cfg(feature = "gpu-fault-injection")]
            creation_pending_ticks: 0,
            retired_recreations: 0,
            retired_scene_frames: 0,
            image_diagnostics: Vec::new(),
            acquire_retry_attempts: 0,
            acquire_retry_reasons: [false; 2],
        }
    }
    pub fn poll_creation(
        &mut self,
        window: &Arc<Window>,
    ) -> Result<Option<Instant>, RendererError> {
        let Some(creation) = &mut self.creation else {
            return Ok(None);
        };
        match creation.poll() {
            RecoveryPoll::Pending { deadline } => {
                #[cfg(feature = "gpu-fault-injection")]
                if self.creation_primary && creation_fixture_enabled() {
                    self.creation_pending_ticks += 1;
                    if self.creation_pending_ticks == 1 {
                        let _ =
                            window.request_inner_size(winit::dpi::LogicalSize::new(1040.0, 740.0));
                        eprintln!(
                            "UI-R10 pending: input_blocked={} renderer_published=false resize_requested=true",
                            self.awaiting_first_frame
                        );
                    }
                }
                return Ok(Some(deadline));
            }
            RecoveryPoll::Ready(renderers) => {
                for renderer in renderers {
                    if renderer.window_id() != window.id() {
                        return Err(RendererError {
                            code: "native_vulkan_creation_identity_failed",
                            stage: "host",
                            fault: RendererFault::Other,
                            message: "renderer belongs to another window".into(),
                            element: None,
                        });
                    }
                    let renderer = Self::prepare_renderer(renderer);
                    #[cfg(feature = "gpu-fault-injection")]
                    let mut renderer = renderer;
                    #[cfg(feature = "gpu-fault-injection")]
                    if self.creation_primary
                        && let Some(device) = recovery_fault()
                    {
                        renderer.inject_recovery_notification(device);
                    }
                    let mut renderer = renderer;
                    if self.creation_resize_pending {
                        let size = window.inner_size();
                        renderer.resize(size.width, size.height);
                        self.creation_resize_pending = false;
                    }
                    self.renderer = Some(Box::new(renderer));
                }
                #[cfg(feature = "gpu-fault-injection")]
                if self.creation_primary && creation_fixture_enabled() {
                    eprintln!(
                        "UI-R10 accepted: window_preserved=true pending_ticks={} first_frame_gate={}",
                        self.creation_pending_ticks, self.awaiting_first_frame
                    );
                }
                window.request_redraw();
            }
            RecoveryPoll::Failed(error) => return Err(error),
            RecoveryPoll::Finished => (),
        }
        if creation.worker_finished() {
            self.creation = None;
            Ok(None)
        } else {
            Ok(Some(Instant::now() + std::time::Duration::from_millis(16)))
        }
    }
    pub fn display_blocked(&self) -> bool {
        self.awaiting_first_frame || self.updates.blocked() || !self.available()
    }
    #[cfg(feature = "gpu-fault-injection")]
    pub fn close_creation_fixture(&mut self) -> bool {
        if self.creation_primary
            && std::env::args().any(|a| a == "--smoke-create-close")
            && let Some(creation) = &mut self.creation
            && creation.fixture_prepared()
        {
            creation.cancel();
            eprintln!(
                "UI-R10 close during creation: gpu_prepared=true input_blocked={} renderer_published=false",
                self.awaiting_first_frame
            );
            return true;
        }
        false
    }
    pub fn frame_retry_deadline(&self) -> Option<Instant> {
        self.renderer
            .as_ref()
            .and_then(|renderer| renderer.frame_retry())
            .map(|retry| retry.deadline)
    }
    pub fn retirement_monitor(&self) -> vesta_gpui_vulkan::RetirementMonitor {
        self.retirement.clone()
    }
    pub fn gpu_context(&self) -> Option<Arc<vesta_gpui_vulkan::GpuContext>> {
        self.renderer
            .as_ref()
            .map(|renderer| renderer.gpu_context())
            .or_else(|| self.updates.context())
    }
    #[cfg(feature = "gpu-fault-injection")]
    pub fn take_retirement_capture(&mut self) -> Option<vesta_gpui_vulkan::CaptureTicket> {
        self.retirement_capture.take()
    }
    pub fn swapchain_recreations(&self) -> u32 {
        self.retired_recreations
            + self
                .renderer
                .as_ref()
                .map_or(self.updates.counts()[0], |renderer| {
                    renderer.swapchain_recreations()
                })
    }
    pub fn scene_frames(&self) -> u32 {
        self.retired_scene_frames
            + self
                .renderer
                .as_ref()
                .map_or(self.updates.counts()[1], |renderer| renderer.scene_frames())
    }
    pub fn available(&self) -> bool {
        self.renderer.is_some()
    }
    pub fn creating(&self) -> bool {
        self.creation.is_some() || self.updates.pending()
    }
    pub fn update_worker_finished(&self) -> bool {
        self.updates.worker_finished()
    }
    #[cfg(feature = "gpu-fault-injection")]
    pub fn inject_update_fixture(
        &mut self,
        fixture: vesta_gpui_vulkan::RecoveryFixture,
        short_budget: bool,
    ) {
        self.updates.inject_fixture(fixture, short_budget);
    }
    #[cfg(feature = "gpu-fault-injection")]
    pub fn update_prepared(&self) -> bool {
        self.updates.fixture_prepared()
    }
    #[cfg(feature = "gpu-fault-injection")]
    pub fn cancel_update(&mut self) {
        self.updates.cancel();
    }
    pub fn take_target(&mut self) -> Option<RecoveryTarget> {
        let counts = self.updates.reset_for_recovery();
        self.retired_recreations += counts[0];
        self.retired_scene_frames += counts[1];
        self.renderer.take().map(|renderer| {
            self.retired_recreations += renderer.swapchain_recreations();
            self.retired_scene_frames += renderer.scene_frames();
            RecoveryTarget::from_renderer(*renderer)
        })
    }
    pub fn restore(&mut self, renderer: WindowRenderer) {
        self.updates.reset_for_recovery();
        self.awaiting_first_frame = true;
        self.renderer = Some(Box::new(renderer));
    }
    pub fn with_gpu(
        window: Arc<Window>,
        gpu: Arc<vesta_gpui_vulkan::GpuContext>,
    ) -> Result<Self, String> {
        RendererCreation::with_gpu(window, gpu)
            .map(|creation| Self::from_creation(creation, false))
            .map_err(|e| e.to_string())
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        self.awaiting_first_frame = true;
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(width, height);
        } else if self.creation.is_some() {
            self.creation_resize_pending = true;
        }
    }
    pub fn poll_updates(
        &mut self,
        window: &Arc<Window>,
        scene: bool,
    ) -> Result<Option<Instant>, RendererError> {
        let size = window.inner_size();
        let desired = TargetConfiguration::ui([size.width, size.height]).with_scene(scene);
        let deadline = self.updates.poll(&mut self.renderer, desired)?;
        if self.renderer.is_some()
            && self.updates.blocked()
            && size.width != 0
            && size.height != 0
            && deadline.is_none_or(|at| at <= Instant::now())
        {
            window.request_redraw();
        }
        Ok(deadline)
    }
    pub fn render(
        &mut self,
        floating: Option<&mut Self>,
        mut frame: RenderFrame<'_>,
    ) -> Result<Option<RenderOutput>, RendererError> {
        let state = floating.unwrap_or(self);
        if let Some(profile) = &mut frame.cpu_profile {
            **profile = None;
        }
        let Some(renderer) = state.renderer.as_ref() else {
            return Ok(None);
        };
        let extent = renderer.window_extent();
        let desired = TargetConfiguration::ui(extent).with_scene(frame.scene.is_some());
        if !state.updates.prepare(&mut state.renderer, desired)? {
            return Ok(None);
        }
        let Some(gpu) = &mut state.renderer else {
            return Ok(None);
        };
        #[cfg(feature = "gpu-fault-injection")]
        if retirement_probe_enabled() && state.retirement_capture.is_none() {
            state.retirement_capture =
                Some(gpu.capture_next_frame(vesta_gpui_vulkan::CaptureRegion {
                    origin: [0, 0],
                    extent: [1, 1],
                })?);
        }
        gpu.enable_cpu_profiling(frame.cpu_profile.is_some());
        let previous_retry = gpu.frame_retry();
        let clear = frame
            .scene
            .map(|(_, _, clear)| clear)
            .or(frame.options.clear)
            .ok_or_else(|| render_error("native_vulkan_clear_required"))?;
        let output = {
            let gpu_profile = frame.gpu_profile;
            let profile_context = super::gpu_profile::GpuFrameContext {
                frame: frame.profile_context,
                viewport: frame.viewport,
            };
            let mut render = || {
                gpu_profile.render(gpu, profile_context, |gpu| {
                    gpu.render_with_scene(
                        frame.viewport,
                        frame.paint,
                        frame.context,
                        frame.rasterizer,
                        frame.images,
                        vesta_gpui_vulkan::RenderOptions {
                            clear,
                            selection: frame.options.selection,
                            caret: frame.options.caret,
                            caret_visible: frame.options.caret_visible,
                        },
                        frame.finish && !completion_fault_enabled(),
                        frame.scene.map(|(camera, scene, _)| {
                            (
                                camera,
                                scene,
                                ViewportRect::full(
                                    (frame.viewport.width * frame.viewport.scale_factor).round()
                                        as u32,
                                    (frame.viewport.height * frame.viewport.scale_factor).round()
                                        as u32,
                                ),
                            )
                        }),
                    )
                })
            };
            let output = render();
            #[cfg(feature = "gpu-fault-injection")]
            if output.as_ref().is_err_and(|error| {
                (recording_fault_enabled() && error.code == "native_vulkan_record_fixture_failed")
                    || (completion_fault_enabled()
                        && error.code == "native_vulkan_completion_dependency_panicked")
            }) {
                let repeated = render();
                if let (Err(first), Err(second)) = (&output, &repeated) {
                    if first.code != second.code
                        || first.stage != second.stage
                        || first.message != second.message
                    {
                        return Err(render_error(
                            "GPU fault changed its error on a repeated redraw",
                        ));
                    }
                } else {
                    return Err(render_error("GPU fault permitted a repeated redraw"));
                }
            }
            output
        };
        #[cfg(feature = "gpu-fault-injection")]
        if recording_fault_enabled() && output.is_err() {
            eprintln!(
                "UI-R04 recording fault: retained={}",
                gpu.recording_fault_retained()
            );
        }
        #[cfg(feature = "gpu-fault-injection")]
        if completion_fault_enabled()
            && output
                .as_ref()
                .is_err_and(|error| error.code == "native_vulkan_completion_dependency_panicked")
        {
            if !gpu.completion_fault_retained() {
                return Err(render_error(
                    "completion panic ownership probe did not finish",
                ));
            }
            eprintln!("UI-R08 completion fault: retained=true unknown_panic_is_other=true");
        }
        #[cfg(feature = "gpu-fault-injection")]
        if frame.cpu_profile.is_some()
            && (acquire_faults_enabled()
                || presentation_fault_enabled()
                || recovery_fault().is_some()
                || completion_fault_enabled())
        {
            // Check the public snapshot against the actual native result, including
            // transitions from a successful frame to a deferred/failed attempt.
            let completed = output.as_ref().is_ok_and(Option::is_some);
            if gpu.cpu_profile().is_some() != completed {
                return Err(render_error(
                    "native_cpu_profile_fixture_failed: stale or missing snapshot",
                ));
            }
            if !completed {
                eprintln!(
                    "UI-P01 native profile: skipped=true current_snapshot=false failed={}",
                    output.is_err()
                );
            }
        }
        let output = output?;
        #[cfg(feature = "gpu-fault-injection")]
        if state.updates.blocked()
            && std::env::args().any(|a| a.starts_with("--smoke-target-"))
            && let Some(output) = &output
        {
            eprintln!(
                "UI-R11 cache: bitmap_uploads={} bitmap_reuses={} glyph_uploads={} glyph_reuses={}",
                output.bitmap_uploads,
                output.bitmap_reuses,
                output.glyph_uploads,
                output.glyph_reuses
            );
            if std::env::args().any(|a| a == "--smoke-target-resize")
                && (output.bitmap_uploads != 0 || output.bitmap_reuses == 0)
            {
                return Err(render_error(
                    "normal target reconfiguration discarded warm image cache",
                ));
            }
        }
        if output.is_some() {
            state.awaiting_first_frame = false;
            state.updates.presented();
        }
        if let Some(profile) = frame.cpu_profile {
            *profile = gpu.cpu_profile();
        }
        if acquire_faults_enabled()
            && let Some(retry) = gpu.frame_retry()
            && Some(retry) != previous_retry
        {
            let index = match retry.reason {
                vesta_gpui_vulkan::FrameRetryReason::ImageUnavailable => Some(0),
                vesta_gpui_vulkan::FrameRetryReason::SwapchainOutOfDate => Some(1),
                vesta_gpui_vulkan::FrameRetryReason::GpuCompletion
                | vesta_gpui_vulkan::FrameRetryReason::TargetConfiguration => None,
            };
            if let Some(index) = index {
                state.acquire_retry_attempts += 1;
                state.acquire_retry_reasons[index] = true;
                eprintln!(
                    "UI-R03 frame deferred: {:?}, attempt {}",
                    retry.reason, retry.attempts
                );
            }
        }
        if let Some(output) = &output
            && state.image_diagnostics != output.image_diagnostics
        {
            for diagnostic in &output.image_diagnostics {
                eprintln!("{}: {diagnostic:?}", diagnostic.issue.code());
            }
            if output.image_diagnostics.is_empty() {
                eprintln!("ui_image_recovered");
            }
            state
                .image_diagnostics
                .clone_from(&output.image_diagnostics);
        }
        Ok(output)
    }
}
