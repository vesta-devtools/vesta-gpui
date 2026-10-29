//! An isolated target uses the same preparation, pipelines and record path as a window.
use super::*;
use crate::{
    PaintList, RenderOptions, TextInputContext, TextRasterizer, Viewport, frame_profile, painter,
    submission, targets,
};
use vesta_gpui::render::ImageResolver;
use vulkano::image::{ImageCreateInfo, ImageType, ImageUsage};

impl WindowRenderer {
    #[allow(clippy::too_many_arguments)]
    pub fn capture_offscreen(
        &mut self,
        viewport: Viewport,
        paint: &PaintList,
        context: &mut TextInputContext,
        rasterizer: &mut TextRasterizer,
        images: Option<&dyn ImageResolver>,
        options: RenderOptions,
        region: CaptureRegion,
    ) -> Result<CaptureTicket, RendererError> {
        self.capture_offscreen_frame(
            viewport,
            paint,
            context,
            rasterizer,
            images,
            options,
            region,
            #[cfg(feature = "viewport")]
            None,
        )
    }
    #[cfg(feature = "viewport")]
    #[allow(clippy::too_many_arguments)]
    pub fn capture_offscreen_with_scene(
        &mut self,
        viewport: Viewport,
        paint: &PaintList,
        context: &mut TextInputContext,
        rasterizer: &mut TextRasterizer,
        images: Option<&dyn ImageResolver>,
        options: RenderOptions,
        region: CaptureRegion,
        scene: crate::recording::SceneFrame<'_>,
    ) -> Result<CaptureTicket, RendererError> {
        self.capture_offscreen_frame(
            viewport, paint, context, rasterizer, images, options, region, scene,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn capture_offscreen_frame(
        &mut self,
        viewport: Viewport,
        paint: &PaintList,
        context: &mut TextInputContext,
        rasterizer: &mut TextRasterizer,
        images: Option<&dyn ImageResolver>,
        options: RenderOptions,
        region: CaptureRegion,
        #[cfg(feature = "viewport")] scene: crate::recording::SceneFrame<'_>,
    ) -> Result<CaptureTicket, RendererError> {
        if self.capture.pending() {
            return Err(RendererError::at(
                "capture_busy",
                "another capture owns the renderer",
            ));
        }
        let result = self.capture_offscreen_inner(
            viewport,
            paint,
            context,
            rasterizer,
            images,
            options,
            region,
            #[cfg(feature = "viewport")]
            scene,
        );
        if let Err(error) = &result {
            #[cfg(feature = "viewport")]
            if let Some(scene) = &mut self.scene {
                scene.invalidate_labels();
            }
            self.capture.fail(error.clone());
            if self.submission.is_some() && error.stage == "capture_submit" {
                self.record_failure(error);
            }
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn capture_offscreen_inner(
        &mut self,
        viewport: Viewport,
        paint: &PaintList,
        context: &mut TextInputContext,
        rasterizer: &mut TextRasterizer,
        images: Option<&dyn ImageResolver>,
        options: RenderOptions,
        region: CaptureRegion,
        #[cfg(feature = "viewport")] scene: crate::recording::SceneFrame<'_>,
    ) -> Result<CaptureTicket, RendererError> {
        if self.failed.is_some() || self.acquired.is_some() || self.gpu.failure().is_some() {
            return Err(RendererError::at(
                "capture_unavailable",
                "renderer unavailable",
            ));
        }
        if let Some(submission) = &mut self.submission {
            let complete = match submission.poll() {
                Ok(complete) => complete,
                Err(error) => {
                    self.record_failure(&error);
                    return Err(error);
                }
            };
            if !complete {
                return Err(RendererError::at(
                    "capture_busy",
                    "previous GPU work is pending",
                ));
            }
            self.capture.complete();
            self.submission = None;
        }
        if !viewport.width.is_finite()
            || !viewport.height.is_finite()
            || !viewport.scale_factor.is_finite()
            || viewport.width <= 0.0
            || viewport.height <= 0.0
            || viewport.scale_factor <= 0.0
        {
            return Err(RendererError::at("capture_viewport", "invalid viewport"));
        }
        let extent = [
            (viewport.width * viewport.scale_factor).ceil() as u32,
            (viewport.height * viewport.scale_factor).ceil() as u32,
        ];
        CaptureRegion {
            origin: [0, 0],
            extent,
        }
        .validate(extent)?;
        region.validate(extent)?;
        let ticket = self.begin_capture(region)?;
        let memory = Arc::new(StandardMemoryAllocator::new_default(
            self.gpu.device.clone(),
        ));
        let image = Image::new(
            memory,
            ImageCreateInfo {
                image_type: ImageType::Dim2d,
                format: self.swapchain.image_format(),
                extent: [extent[0], extent[1], 1],
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_SRC,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
        )
        .map_err(|e| RendererError::caused_by("capture_target", e))?;
        let frames = targets::framebuffers(
            std::slice::from_ref(&image),
            self.render_pass.clone(),
            self.samples,
            self.has_scene(),
        )?;
        let (prepared, mut output) = painter::prepare(
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
        )?;
        let mut trace = frame_profile::Trace::new(false);
        let (mut builder, _) = self.record_commands(
            frames[0].clone(),
            extent,
            prepared,
            &mut output,
            options,
            &mut trace,
            false,
            #[cfg(feature = "viewport")]
            scene,
        )?;
        self.record_capture(&mut builder, image, self.presented_frames, true)?;
        let command = builder
            .build()
            .map_err(|e| RendererError::caused_by("capture_commands", e))?;
        self.submission = Some(submission::Submission::offscreen(
            command,
            self.gpu.queue.clone(),
        )?);
        self.submission
            .as_mut()
            .ok_or_else(|| RendererError::at("capture_submit", "missing submission"))?
            .submit_offscreen()?;
        self.retry.defer(
            std::time::Instant::now(),
            crate::FrameRetryReason::GpuCompletion,
        );
        Ok(ticket)
    }
}
