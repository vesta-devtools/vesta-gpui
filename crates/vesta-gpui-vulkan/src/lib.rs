//! Native Vulkan rendering of the shared UI PaintList, with an optional viewport. ADR-0316.
#![deny(unsafe_code)]
mod capture;
#[allow(unsafe_code)]
mod checked_pipeline;
pub use capture::{CaptureReadback, CaptureRegion, CaptureTicket};
mod creation;
mod frame_profile;
mod frame_retry;
mod glyph_atlas;
mod gpu_context;
mod gpu_failure;
mod gpu_profile;
mod image;
mod painter;
mod preparation;
pub use preparation::{PreparedPaint, prepare_paint};
mod reconfiguration;
mod recording;
mod recovery;
mod renderer_error;
mod retirement;
#[cfg(feature = "viewport")]
mod scene_views;
mod submission;
mod targets;
#[cfg(feature = "viewport")]
mod viewport;
#[cfg(feature = "viewport")]
pub use scene_views::{MAX_SCENE_VIEWS, SceneView, validate_scene_views};

pub use creation::RendererCreation;
#[cfg(feature = "fault-injection")]
pub use creation::RendererCreationFixture;
pub use frame_profile::FrameCpuProfile;
pub use frame_retry::{FrameRetry, FrameRetryReason};
pub use gpu_context::GpuContext;
#[cfg(feature = "fault-injection")]
pub use gpu_context::InitialPresentationFault;
pub use gpu_profile::{
    FrameGpuProfile, GpuProfileKey, GpuProfileObservation, GpuProfileStream,
    GpuTimestampCapabilities, GpuTimingUnavailable,
};
pub use reconfiguration::{RendererUpdates, TargetConfiguration, TargetMode};
#[cfg(feature = "fault-injection")]
pub use recovery::RecoveryFixture;
pub use recovery::{
    RecoveryJob, RecoveryPoll, RecoveryRejected, RecoveryScope, RecoverySeed, RecoveryTarget,
};
pub use renderer_error::{RendererError, RendererFault};
pub use retirement::RetirementMonitor;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use vesta_gpui::render::{ImageResolver, RenderOutput};
use vesta_gpui::{PaintList, TextInputContext, TextRasterizer, Viewport};
use vulkano::{
    Validated, VulkanError,
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassBeginInfo,
        SubpassEndInfo,
    },
    format::Format,
    image::{Image, ImageUsage, SampleCount},
    render_pass::{Framebuffer, RenderPass},
    swapchain::{
        ColorSpace, PresentMode, Surface, Swapchain, SwapchainCreateInfo, SwapchainPresentInfo,
        acquire_next_image,
    },
};
use winit::{event_loop::ActiveEventLoop, window::Window};
pub struct WindowRenderer {
    capture: capture::CaptureState,
    presented_frames: u64,
    window: Arc<Window>,
    gpu: Arc<GpuContext>,
    retirement: Arc<retirement::Slot<retirement::Owner>>,
    swapchain: Arc<Swapchain>,
    render_pass: Arc<RenderPass>,
    overlay_pass: Option<Arc<RenderPass>>,
    frames: Vec<Arc<Framebuffer>>,
    images: Vec<Arc<Image>>,
    submission: Option<submission::Submission>,
    /// Keep an acquired image owned if upload/recording fails before submission.
    acquired: Option<vulkano::swapchain::SwapchainAcquireFuture>,
    failed: Option<RendererError>,
    recreate: bool,
    recreations: u32,
    cpu_profiling: bool,
    cpu_profile: Option<FrameCpuProfile>,
    gpu_profile: Option<gpu_profile::GpuTimestampState>,
    retry: frame_retry::AcquireRetry,
    #[cfg(feature = "fault-injection")]
    acquire_faults: std::collections::VecDeque<VulkanError>,
    #[cfg(feature = "fault-injection")]
    presentation_probe: PresentationProbe,
    #[cfg(feature = "fault-injection")]
    recording_probe: RecordingProbe,
    #[cfg(feature = "fault-injection")]
    completion_probe: CompletionProbe,
    #[cfg(feature = "fault-injection")]
    recovery_probe: Option<(RendererFault, u32)>,
    painter: painter::Painter,
    #[cfg(feature = "viewport")]
    scene: Option<viewport::ScenePainter>,
    samples: SampleCount,
    #[cfg(feature = "viewport")]
    scene_frames: u32,
}

impl Drop for WindowRenderer {
    fn drop(&mut self) {
        self.capture.fail(RendererError::at(
            "capture_closed",
            "window renderer closed",
        ));
        let submission = self.submission.take();
        let acquired = self.acquired.take();
        if self.gpu.worker.on_worker() {
            if let Err(error) = recovery::retire_owned_futures(&self.gpu, submission, acquired) {
                self.gpu.worker.record_retirement_error(error);
            }
        } else {
            self.gpu.worker.retire(
                &self.retirement,
                retirement::Owner::Frame {
                    gpu: self.gpu.clone(),
                    submission,
                    acquired,
                },
            );
        }
    }
}

#[cfg(feature = "fault-injection")]
#[derive(Default, PartialEq, Eq)]
enum PresentationProbe {
    #[default]
    Disabled,
    Inject,
    AwaitCompletion,
    Complete,
}

#[cfg(feature = "fault-injection")]
#[derive(Default, PartialEq, Eq)]
enum RecordingProbe {
    #[default]
    Disabled,
    Inject,
    AwaitRedraw,
    Verified,
}

#[cfg(feature = "fault-injection")]
#[derive(Default, PartialEq, Eq)]
enum CompletionProbe {
    #[default]
    Disabled,
    Inject,
    AwaitRedraw,
    Verified,
}

impl WindowRenderer {
    /// Enable optional CPU observations; the default path reads no profiling clock.
    pub fn enable_cpu_profiling(&mut self, enabled: bool) {
        self.cpu_profiling = enabled;
        if !enabled {
            self.cpu_profile = None;
        }
    }

    /// Snapshot of this renderer's latest successful attempt, cleared by every
    /// subsequent attempt, including deferred or failed frames. Not GPU timing.
    pub fn cpu_profile(&self) -> Option<FrameCpuProfile> {
        self.cpu_profile
    }

    /// Explicit opt-in. Default consumers allocate no queries or telemetry.
    /// Keep the returned stream through retirement to collect a final submission.
    pub fn enable_gpu_profiling(
        &mut self,
        enabled: bool,
    ) -> Result<Option<GpuProfileStream>, RendererError> {
        if !enabled {
            self.gpu_profile = None;
        } else if self.gpu_profile.is_none() {
            self.gpu_profile = Some(gpu_profile::GpuTimestampState::new(&self.gpu)?);
        }
        Ok(self.gpu_profile.as_ref().map(|state| state.stream.clone()))
    }

    /// Latest profiled recording attempt, including attempts whose present failed.
    /// Compare before/after a call; this is not a successful presentation count.
    pub fn gpu_profile_key(&self) -> Option<GpuProfileKey> {
        self.gpu_profile
            .as_ref()
            .and_then(gpu_profile::GpuTimestampState::key)
    }

    pub fn window_id(&self) -> winit::window::WindowId {
        self.window.id()
    }
    fn record_failure(&mut self, error: &RendererError) {
        self.capture.fail(error.clone());
        self.gpu.failure.record(error);
        self.failed = Some(error.clone());
    }
    #[cfg(feature = "viewport")]
    pub fn scene_frames(&self) -> u32 {
        self.scene_frames
    }
    pub fn swapchain_recreations(&self) -> u32 {
        self.recreations
    }
    pub fn frame_retry(&self) -> Option<FrameRetry> {
        self.retry.pending()
    }

    /// Explicit finite pre-acquire failures for the standalone fault harness.
    /// This API is absent from normal builds and never fabricates GPU completion.
    #[cfg(feature = "fault-injection")]
    pub fn inject_acquire_faults(&mut self) {
        self.acquire_faults = [
            VulkanError::Timeout,
            VulkanError::NotReady,
            VulkanError::OutOfDate,
        ]
        .into();
    }
    #[cfg(feature = "fault-injection")]
    pub fn inject_present_out_of_date(&mut self) {
        self.presentation_probe = PresentationProbe::Inject;
    }
    #[cfg(feature = "fault-injection")]
    pub fn presentation_fault_recovered(&self) -> bool {
        self.presentation_probe == PresentationProbe::Complete
    }
    #[cfg(feature = "fault-injection")]
    pub fn inject_recording_failure(&mut self) {
        self.recording_probe = RecordingProbe::Inject;
    }
    #[cfg(feature = "fault-injection")]
    pub fn recording_fault_retained(&self) -> bool {
        self.recording_probe == RecordingProbe::Verified
    }
    #[cfg(feature = "fault-injection")]
    pub fn inject_completion_poll_panic(&mut self) {
        self.completion_probe = CompletionProbe::Inject;
    }
    #[cfg(feature = "fault-injection")]
    pub fn completion_fault_retained(&self) -> bool {
        self.completion_probe == CompletionProbe::Verified
    }
    #[cfg(feature = "fault-injection")]
    pub fn inject_retirement_completion_panic(&mut self) -> bool {
        if let Some(submission) = &mut self.submission {
            submission.inject_poll_panic();
            true
        } else {
            false
        }
    }
    /// Notify loss after actual submissions on a healthy GPU. This never fakes
    /// completion or resets the physical device; retirement still polls fences.
    #[cfg(feature = "fault-injection")]
    pub fn inject_recovery_notification(&mut self, device: bool) {
        self.recovery_probe = Some((
            if device {
                RendererFault::DeviceLost
            } else {
                RendererFault::SurfaceLost
            },
            4,
        ));
    }
    /// Fail after the first real submission completes, before returning its
    /// output to the host. Does not fabricate a fence or accepted UI frame.
    #[cfg(feature = "fault-injection")]
    pub fn inject_initial_recovery_notification(&mut self, device: bool) {
        self.inject_recovery_notification(device);
        if let Some((_, remaining)) = &mut self.recovery_probe {
            *remaining = 1;
        }
    }
    pub fn new(event_loop: &ActiveEventLoop, window: Arc<Window>) -> Result<Self, RendererError> {
        let (gpu, surface) = GpuContext::for_window(event_loop, window.clone())?;
        Self::from_surface(window, gpu.clone(), surface)
            .inspect_err(|error| gpu.failure.record(error))
    }

    /// Inject a typed failure at initial presentation selection, before Device
    /// creation. This creates a real instance/surface, never a fake submission.
    #[cfg(feature = "fault-injection")]
    pub fn new_with_initial_presentation_fault(
        event_loop: &ActiveEventLoop,
        window: Arc<Window>,
        fault: InitialPresentationFault,
    ) -> Result<Self, RendererError> {
        let (gpu, surface) = GpuContext::for_window_with_fault(event_loop, window.clone(), fault)?;
        Self::from_surface(window, gpu.clone(), surface)
            .inspect_err(|error| gpu.failure.record(error))
    }

    /// Create another surface without creating another Device or Queue.
    pub fn with_gpu(window: Arc<Window>, gpu: Arc<GpuContext>) -> Result<Self, RendererError> {
        if !gpu.can_create_renderer() {
            return Err(RendererError::at(
                "retirement",
                "GPU worker is retiring prior owners; defer new renderer creation",
            ));
        }
        if let Some(error) = gpu.failure() {
            return Err(error.clone());
        }
        let surface = Surface::from_window(gpu.instance.clone(), window.clone())
            .map_err(|e| RendererError::caused_by("surface", e))?;
        Self::from_surface(window, gpu.clone(), surface)
            .inspect_err(|error| gpu.failure.record(error))
    }

    pub fn gpu_context(&self) -> Arc<GpuContext> {
        Arc::clone(&self.gpu)
    }

    fn from_surface(
        window: Arc<Window>,
        gpu: Arc<GpuContext>,
        surface: Arc<Surface>,
    ) -> Result<Self, RendererError> {
        let device = &gpu.device;
        let physical = device.physical_device();
        if !physical
            .surface_support(gpu.queue.queue_family_index(), &surface)
            .map_err(|e| RendererError::caused_by("presentation_support", e))?
        {
            return Err(RendererError {
                code: "native_vulkan_surface_unsupported",
                stage: "presentation_support",
                fault: RendererFault::Other,
                message: "application queue cannot present to this window surface".into(),
                element: None,
            });
        }
        let capabilities = physical
            .surface_capabilities(&surface, Default::default())
            .map_err(|e| RendererError::caused_by("capabilities", e))?;
        if !capabilities
            .supported_usage_flags
            .contains(ImageUsage::COLOR_ATTACHMENT)
        {
            return Err(RendererError::at(
                "capabilities",
                "color attachments are unsupported",
            ));
        }
        let (format, color_space) = physical
            .surface_formats(&surface, Default::default())
            .map_err(|e| RendererError::caused_by("formats", e))?
            .into_iter()
            .find(|(f, space)| {
                matches!(f, Format::B8G8R8A8_SRGB | Format::R8G8B8A8_SRGB)
                    && *space == ColorSpace::SrgbNonLinear
            })
            .ok_or_else(|| RendererError::at("formats", "no supported sRGB surface format"))?;
        let composite_alpha = capabilities
            .supported_composite_alpha
            .into_iter()
            .next()
            .ok_or_else(|| RendererError::at("capabilities", "no composite alpha mode"))?;
        let modes = physical
            .surface_present_modes(&surface, Default::default())
            .map_err(|e| RendererError::caused_by("present_modes", e))?;
        let present_mode = [
            PresentMode::Immediate,
            PresentMode::Mailbox,
            PresentMode::Fifo,
        ]
        .into_iter()
        .find(|mode| modes.contains(mode))
        .ok_or_else(|| RendererError::at("present_modes", "no supported presentation mode"))?;
        let count = capabilities
            .min_image_count
            .max(2)
            .min(capabilities.max_image_count.unwrap_or(u32::MAX));
        let size = window.inner_size();
        let extent = capabilities.current_extent.unwrap_or([
            size.width.clamp(
                capabilities.min_image_extent[0],
                capabilities.max_image_extent[0],
            ),
            size.height.clamp(
                capabilities.min_image_extent[1],
                capabilities.max_image_extent[1],
            ),
        ]);
        let (swapchain, images) = Swapchain::new(
            device.clone(),
            surface,
            SwapchainCreateInfo {
                min_image_count: count,
                image_format: format,
                image_color_space: color_space,
                image_extent: extent,
                image_usage: ImageUsage::COLOR_ATTACHMENT
                    | (capabilities.supported_usage_flags & ImageUsage::TRANSFER_SRC),
                composite_alpha,
                pre_transform: capabilities.current_transform,
                present_mode,
                ..Default::default()
            },
        )
        .map_err(|e| RendererError::caused_by("swapchain", e))?;
        let samples = SampleCount::Sample1;
        let render_pass = targets::render_pass(device.clone(), format, samples, false)?;
        let frames = targets::framebuffers(&images, render_pass.clone(), samples, false)?;
        eprintln!(
            "Vesta native Vulkan renderer: {format:?}, {present_mode:?}, {count} swapchain images"
        );
        let painter = painter::Painter::new(device.clone(), render_pass.clone())?;
        Ok(Self {
            retirement: gpu.worker.register_retirement(),
            window,
            painter,
            #[cfg(feature = "viewport")]
            scene: None,
            samples,
            #[cfg(feature = "viewport")]
            scene_frames: 0,
            swapchain,
            render_pass,
            overlay_pass: None,
            frames,
            images,
            submission: None,
            capture: Default::default(),
            presented_frames: 0,
            acquired: None,
            failed: None,
            gpu,
            recreate: false,
            recreations: 0,
            cpu_profiling: false,
            cpu_profile: None,
            gpu_profile: None,
            retry: Default::default(),
            #[cfg(feature = "fault-injection")]
            acquire_faults: Default::default(),
            #[cfg(feature = "fault-injection")]
            presentation_probe: Default::default(),
            #[cfg(feature = "fault-injection")]
            recording_probe: Default::default(),
            #[cfg(feature = "fault-injection")]
            completion_probe: Default::default(),
            #[cfg(feature = "fault-injection")]
            recovery_probe: None,
        })
    }

    #[cfg(feature = "viewport")]
    fn configure_scene(&mut self, enabled: bool) -> Result<(), RendererError> {
        if self.scene.is_some() == enabled {
            return Ok(());
        }
        let samples = if enabled {
            targets::sample_count(&self.gpu.device, self.swapchain.image_format())?
        } else {
            SampleCount::Sample1
        };
        let pass = targets::render_pass(
            self.gpu.device.clone(),
            self.swapchain.image_format(),
            samples,
            enabled,
        )?;
        let frames = targets::framebuffers(&self.images, pass.clone(), samples, enabled)?;
        let overlay = enabled
            .then(|| {
                targets::overlay_pass(
                    self.gpu.device.clone(),
                    self.swapchain.image_format(),
                    samples,
                )
            })
            .transpose()?;
        let scene = if enabled {
            Some(viewport::ScenePainter::new(
                self.gpu.device.clone(),
                pass.clone(),
                overlay.as_ref().expect("enabled overlay pass").clone(),
                samples,
            )?)
        } else {
            None
        };
        self.painter.set_pass(
            self.gpu.device.clone(),
            overlay.as_ref().unwrap_or(&pass).clone(),
        )?;
        self.render_pass = pass;
        self.overlay_pass = overlay;
        self.frames = frames;
        self.samples = samples;
        self.scene = scene;
        eprintln!("Vesta native Vulkan targets: 3D={enabled}, {samples:?}");
        Ok(())
    }
    fn rebuild(&mut self, requested_extent: [u32; 2]) -> Result<(), RendererError> {
        let caps = self
            .gpu
            .device
            .physical_device()
            .surface_capabilities(self.swapchain.surface(), Default::default())
            .map_err(|e| RendererError::caused_by("resize_capabilities", e))?;
        let extent = caps.current_extent.unwrap_or([
            requested_extent[0].clamp(caps.min_image_extent[0], caps.max_image_extent[0]),
            requested_extent[1].clamp(caps.min_image_extent[1], caps.max_image_extent[1]),
        ]);
        if extent.contains(&0) {
            return Err(RendererError {
                code: "native_vulkan_target_unavailable",
                stage: "resize_capabilities",
                fault: RendererFault::SwapchainOutOfDate,
                message: "surface has zero extent; the unretired target remains owned".into(),
                element: None,
            });
        }
        let (swapchain, images) = self
            .swapchain
            .recreate(SwapchainCreateInfo {
                image_extent: extent,
                pre_transform: caps.current_transform,
                ..self.swapchain.create_info()
            })
            .map_err(|e| RendererError::caused_by("resize_swapchain", e))?;
        self.frames = targets::framebuffers(
            &images,
            self.render_pass.clone(),
            self.samples,
            self.has_scene(),
        )?;
        self.images = images;
        self.swapchain = swapchain;
        self.recreate = false;
        self.recreations += 1;
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        // A pending creation already reads the latest window size. Replaying
        // its resize intent must not rebuild an identical target on the UI.
        if self.swapchain.image_extent() != [width, height] {
            self.recreate = true;
        }
        self.retry.reset();
    }
    pub fn needs_configuration(&self, desired: TargetConfiguration) -> bool {
        self.recreate
            || self.swapchain.image_extent() != desired.extent
            || self.has_scene() != desired.mode.is_scene()
    }
    pub fn window_extent(&self) -> [u32; 2] {
        let size = self.window.inner_size();
        [size.width, size.height]
    }

    fn has_scene(&self) -> bool {
        #[cfg(feature = "viewport")]
        {
            self.scene.is_some()
        }
        #[cfg(not(feature = "viewport"))]
        {
            false
        }
    }

    /// Render shared UI without requiring the optional 3D dependency.
    /// Present a UI frame. The signature is unchanged when `viewport` is enabled.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        viewport: Viewport,
        paint: &PaintList,
        context: &mut TextInputContext,
        rasterizer: &mut TextRasterizer,
        images: Option<&dyn ImageResolver>,
        options: RenderOptions,
        finish: bool,
    ) -> Result<Option<RenderOutput>, RendererError> {
        self.render_frame(
            viewport,
            paint,
            context,
            rasterizer,
            images,
            options,
            finish,
            #[cfg(feature = "viewport")]
            scene_views::SceneInput::Single(None),
            #[cfg(feature = "viewport")]
            None,
        )
        .inspect_err(|error| self.record_failure(error))
    }

    /// Present the same UI frame over an optional immutable viewport scene.
    #[cfg(feature = "viewport")]
    #[allow(clippy::too_many_arguments)]
    pub fn render_with_scene(
        &mut self,
        viewport: Viewport,
        paint: &PaintList,
        context: &mut TextInputContext,
        rasterizer: &mut TextRasterizer,
        images: Option<&dyn ImageResolver>,
        options: RenderOptions,
        finish: bool,
        scene: Option<(
            vesta_viewport::Camera,
            &vesta_viewport::ViewportScene,
            vesta_viewport::render::ViewportRect,
        )>,
    ) -> Result<Option<RenderOutput>, RendererError> {
        self.render_frame(
            viewport,
            paint,
            context,
            rasterizer,
            images,
            options,
            finish,
            scene_views::SceneInput::Single(scene),
            None,
        )
        .inspect_err(|error| self.record_failure(error))
    }

    /// Present on the exact mode prepared by the Host. Scene-capable targets
    /// may carry UI-only content while remaining available for offscreen 3D.
    /// The legacy render/render_with_scene inference remains unchanged.
    #[cfg(feature = "viewport")]
    #[allow(clippy::too_many_arguments)]
    pub fn render_with_target_mode(
        &mut self,
        viewport: Viewport,
        paint: &PaintList,
        context: &mut TextInputContext,
        rasterizer: &mut TextRasterizer,
        images: Option<&dyn ImageResolver>,
        options: RenderOptions,
        finish: bool,
        mode: TargetMode,
        scene: Option<(
            vesta_viewport::Camera,
            &vesta_viewport::ViewportScene,
            vesta_viewport::render::ViewportRect,
        )>,
    ) -> Result<Option<RenderOutput>, RendererError> {
        self.render_frame(
            viewport,
            paint,
            context,
            rasterizer,
            images,
            options,
            finish,
            scene_views::SceneInput::Single(scene),
            Some(mode),
        )
        .inspect_err(|error| self.record_failure(error))
    }

    /// Present a bounded batch of independent scene views, followed by UI once.
    /// Visible clips must be disjoint; invalid batches do not retire the renderer.
    #[cfg(feature = "viewport")]
    #[allow(clippy::too_many_arguments)]
    pub fn render_with_scene_views(
        &mut self,
        viewport: Viewport,
        paint: &PaintList,
        context: &mut TextInputContext,
        rasterizer: &mut TextRasterizer,
        images: Option<&dyn ImageResolver>,
        options: RenderOptions,
        finish: bool,
        views: &[SceneView<'_>],
    ) -> Result<Option<RenderOutput>, RendererError> {
        self.render_frame(
            viewport,
            paint,
            context,
            rasterizer,
            images,
            options,
            finish,
            scene_views::SceneInput::Batch(views),
            None,
        )
        .inspect_err(|error| {
            if !scene_views::input_rejected(error) {
                self.record_failure(error);
            }
        })
    }

    /// Use the explicitly prepared target mode with one ordinary batch submission.
    #[cfg(feature = "viewport")]
    #[allow(clippy::too_many_arguments)]
    pub fn render_with_scene_views_and_target_mode(
        &mut self,
        viewport: Viewport,
        paint: &PaintList,
        context: &mut TextInputContext,
        rasterizer: &mut TextRasterizer,
        images: Option<&dyn ImageResolver>,
        options: RenderOptions,
        finish: bool,
        mode: TargetMode,
        views: &[SceneView<'_>],
    ) -> Result<Option<RenderOutput>, RendererError> {
        self.render_frame(
            viewport,
            paint,
            context,
            rasterizer,
            images,
            options,
            finish,
            scene_views::SceneInput::Batch(views),
            Some(mode),
        )
        .inspect_err(|error| {
            if !scene_views::input_rejected(error) {
                self.record_failure(error);
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn render_frame(
        &mut self,
        viewport: Viewport,
        paint: &PaintList,
        context: &mut TextInputContext,
        rasterizer: &mut TextRasterizer,
        images: Option<&dyn ImageResolver>,
        options: RenderOptions,
        finish: bool,
        #[cfg(feature = "viewport")] scene: scene_views::SceneInput<'_>,
        #[cfg(feature = "viewport")] target_mode: Option<TargetMode>,
    ) -> Result<Option<RenderOutput>, RendererError> {
        let _trace = vesta_gpui::trace::span("gpui.vulkan.cpu", "renderer.attempt");
        self.cpu_profile = None;
        let mut trace = frame_profile::Trace::new(self.cpu_profiling);
        if let Some(error) = self.gpu.failure() {
            return Err(error.clone());
        }
        // A failed recording/flush keeps its owner. Repeated host redraws may
        // inspect the same error, but must not replace an acquired/submitted
        // frame or silently reuse its device generation before recovery.
        if let Some(error) = &self.failed {
            #[cfg(feature = "fault-injection")]
            if self.recording_probe == RecordingProbe::AwaitRedraw {
                if self.acquired.is_none() || self.submission.is_some() {
                    return Err(RendererError::at(
                        "record_probe",
                        "failed frame ownership was not retained",
                    ));
                }
                self.recording_probe = RecordingProbe::Verified;
                eprintln!(
                    "UI-R04 recording owner: same_error=true acquired_retained=true repeated_redraw_rejected=true"
                );
            }
            #[cfg(feature = "fault-injection")]
            if self.completion_probe == CompletionProbe::AwaitRedraw {
                if self.submission.is_none()
                    || self.acquired.is_some()
                    || self.gpu.failure().is_some()
                    || error.fault != RendererFault::Other
                    || error.code != "native_vulkan_completion_dependency_panicked"
                {
                    return Err(RendererError::at(
                        "completion_probe",
                        "completion panic did not preserve its owner and unknown failure scope",
                    ));
                }
                self.completion_probe = CompletionProbe::Verified;
                eprintln!(
                    "UI-R08 completion owner: same_error=true submission_retained=true device_failure=false repeated_redraw_rejected=true"
                );
            }
            return Err(error.clone());
        }
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            self.retry.reset();
            return Ok(None);
        }
        if !self.retry.ready(Instant::now()) {
            return Ok(None);
        }
        if let Some(submission) = &mut self.submission {
            #[cfg(feature = "fault-injection")]
            if self.completion_probe == CompletionProbe::Inject {
                submission.inject_poll_panic();
                self.completion_probe = CompletionProbe::AwaitRedraw;
            }
            if !submission.poll()? {
                self.retry
                    .defer(Instant::now(), FrameRetryReason::GpuCompletion);
                return Ok(None);
            }
            self.capture.complete();
            self.submission = None;
            #[cfg(feature = "fault-injection")]
            if self.presentation_probe == PresentationProbe::AwaitCompletion {
                self.presentation_probe = PresentationProbe::Complete;
                eprintln!("UI-R04 presentation owner: real_completion_before_rebuild=true");
            }
        }
        trace.mark(frame_profile::Stage::Readiness);
        let desired = TargetConfiguration::ui([size.width, size.height]);
        #[cfg(feature = "viewport")]
        let desired = TargetConfiguration {
            extent: desired.extent,
            mode: reconfiguration::frame_mode(scene.is_present(), target_mode)?,
        };
        if self.needs_configuration(desired) {
            self.retry
                .defer(Instant::now(), FrameRetryReason::TargetConfiguration);
            return Ok(None);
        }
        #[cfg(feature = "viewport")]
        let scenes = scene.prepare(desired.extent)?;
        trace.mark(frame_profile::Stage::Targets);
        let (prepared, mut output) = prepare_paint(
            viewport,
            paint,
            context,
            rasterizer,
            images,
            self.gpu
                .device
                .physical_device()
                .properties()
                .max_image_dimension2_d,
            options,
        )?
        .into_parts();
        trace.prepared(prepared.len());
        trace.mark(frame_profile::Stage::Prepare);
        #[cfg(feature = "fault-injection")]
        let injected = self.acquire_faults.pop_front();
        #[cfg(not(feature = "fault-injection"))]
        let injected: Option<VulkanError> = None;
        let acquisition = match injected {
            Some(error) => Err(Validated::Error(error)),
            None => acquire_next_image(self.swapchain.clone(), Some(Duration::ZERO)),
        };
        let (index, suboptimal, acquired) = match acquisition {
            Ok(value) => value,
            Err(Validated::Error(VulkanError::Timeout | VulkanError::NotReady)) => {
                self.retry
                    .defer(Instant::now(), FrameRetryReason::ImageUnavailable);
                return Ok(None);
            }
            Err(Validated::Error(VulkanError::OutOfDate)) => {
                self.recreate = true;
                self.retry
                    .defer(Instant::now(), FrameRetryReason::SwapchainOutOfDate);
                return Ok(None);
            }
            Err(error) => return Err(RendererError::caused_by("acquire", error)),
        };
        self.acquired = Some(acquired);
        trace.mark(frame_profile::Stage::Acquire);
        #[cfg(feature = "fault-injection")]
        if self.recording_probe == RecordingProbe::Inject {
            self.recording_probe = RecordingProbe::AwaitRedraw;
            return Err(RendererError {
                code: "native_vulkan_record_fixture_failed",
                stage: "record_probe",
                fault: RendererFault::Other,
                message: "intentional pre-submit failure after acquiring an image; owner retained"
                    .into(),
                element: None,
            });
        }
        self.recreate |= suboptimal;
        let (mut builder, gpu_timestamp) = self.record_commands(
            self.frames[index as usize].clone(),
            self.swapchain.image_extent(),
            prepared,
            &mut output,
            options,
            &mut trace,
            true,
            #[cfg(feature = "viewport")]
            &scenes,
        )?;
        self.record_capture(
            &mut builder,
            self.images[index as usize].clone(),
            self.presented_frames + 1,
            false,
        )?;
        trace.mark(frame_profile::Stage::Record);
        let command = builder
            .build()
            .map_err(|e| RendererError::caused_by("build_commands", e))?;
        trace.mark(frame_profile::Stage::Commands);
        self.submission = Some(submission::Submission::new(
            &mut self.acquired,
            command,
            self.gpu.queue.clone(),
            gpu_timestamp,
        )?);
        let submission = self
            .submission
            .as_mut()
            .ok_or_else(|| RendererError::at("submit", "missing retained submission"))?;
        #[cfg(feature = "fault-injection")]
        let inject = self.presentation_probe == PresentationProbe::Inject;
        #[cfg(not(feature = "fault-injection"))]
        let inject = false;
        let result = submission.submit(
            self.gpu.queue.clone(),
            SwapchainPresentInfo::swapchain_image_index(self.swapchain.clone(), index),
            inject,
        );
        trace.mark(frame_profile::Stage::Submit);
        #[cfg(feature = "fault-injection")]
        if inject
            && result
                .as_ref()
                .is_err_and(|error| error.fault == RendererFault::SwapchainOutOfDate)
        {
            if !submission.has_completion_fence() {
                return Err(RendererError::at(
                    "present",
                    "injected failure did not retain a completion fence",
                ));
            }
            self.presentation_probe = PresentationProbe::AwaitCompletion;
            eprintln!("UI-R04 presentation owner: retained_after_error=true completion_fence=true");
        }
        match result {
            Ok(()) => {
                vesta_gpui::trace::instant("gpui.vulkan.cpu", "present.submitted");
                self.presented_frames += 1;
                if finish {
                    submission.finish()?;
                    self.capture.complete();
                }
                trace.mark(frame_profile::Stage::Finish);
                self.retry.reset();
                if self.capture.pending() {
                    self.retry
                        .defer(Instant::now(), FrameRetryReason::GpuCompletion);
                }
                #[cfg(feature = "fault-injection")]
                if let Some((fault, remaining)) = &mut self.recovery_probe {
                    *remaining -= 1;
                    if *remaining == 0 {
                        let cause = if *fault == RendererFault::DeviceLost {
                            VulkanError::DeviceLost
                        } else {
                            VulkanError::SurfaceLost
                        };
                        self.recovery_probe = None;
                        return Err(RendererError::caused_by(
                            "recovery_notification_fixture",
                            cause,
                        ));
                    }
                }
                #[cfg(feature = "viewport")]
                if scene.is_present() {
                    self.scene_frames = self.scene_frames.saturating_add(1);
                }
                self.cpu_profile = trace.completed(&output, finish);
                Ok(Some(output))
            }
            Err(error) if error.fault == RendererFault::SwapchainOutOfDate => {
                self.painter.invalidate_cache();
                #[cfg(feature = "viewport")]
                if let Some(scene) = &mut self.scene {
                    scene.invalidate_labels();
                }
                self.recreate = true;
                self.retry
                    .defer(Instant::now(), FrameRetryReason::SwapchainOutOfDate);
                Ok(None)
            }
            Err(error) => {
                #[cfg(feature = "viewport")]
                if let Some(scene) = &mut self.scene {
                    scene.invalidate_labels();
                }
                Err(error)
            }
        }
    }
}
#[derive(Clone, Copy)]
pub struct RenderOptions {
    /// Linear RGBA for the sRGB target; theme tokens use `vesta_gpui::render::linear_color`.
    pub clear: [f32; 4],
    pub selection: vesta_gpui::Color,
    pub caret: vesta_gpui::Color,
    pub caret_visible: bool,
}
