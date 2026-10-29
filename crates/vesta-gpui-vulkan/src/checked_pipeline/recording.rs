//! Owns the complete one-time recording before exposing its timestamped builder.
use crate::*;
use vulkano::{
    command_buffer::PrimaryAutoCommandBuffer,
    query::{QueryPool, QueryPoolCreateInfo, QueryType},
    sync::PipelineStage,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn record(
    renderer: &mut WindowRenderer,
    framebuffer: Arc<Framebuffer>,
    extent: [u32; 2],
    prepared: Vec<painter::Prepared>,
    output: &mut RenderOutput,
    options: RenderOptions,
    trace: &mut frame_profile::Trace,
    presented: bool,
    #[cfg(feature = "viewport")] scene: crate::recording::SceneFrame<'_>,
) -> Result<
    (
        AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        Option<gpu_profile::PendingGpuTimestamp>,
    ),
    RendererError,
> {
    let mut builder = AutoCommandBufferBuilder::primary(
        renderer.gpu.commands.clone(),
        renderer.gpu.queue.queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )
    .map_err(|error| RendererError::caused_by("command_buffer", error))?;
    let ticket = if presented {
        renderer
            .gpu_profile
            .as_mut()
            .map(gpu_profile::GpuTimestampState::next)
            .transpose()?
            .flatten()
    } else {
        None
    };
    let timing = ticket
        .map(|ticket| {
            QueryPool::new(
                renderer.gpu.device.clone(),
                QueryPoolCreateInfo {
                    query_count: 2,
                    ..QueryPoolCreateInfo::query_type(QueryType::Timestamp)
                },
            )
            .map(|pool| (ticket, pool))
            .map_err(|error| RendererError::caused_by("gpu_profile_pool", error))
        })
        .transpose()?;
    if let Some((_, pool)) = &timing {
        // SAFETY: this fresh, private two-slot pool has no other command-buffer
        // users. Reset and both writes stay in this one-time builder; neither
        // pool nor builder is exposed until the whole recording is complete.
        unsafe {
            builder
                .reset_query_pool(pool.clone(), 0..2)
                .map_err(|error| RendererError::caused_by("gpu_profile_reset", error))?;
            builder
                .write_timestamp(pool.clone(), 0, PipelineStage::TopOfPipe)
                .map_err(|error| RendererError::caused_by("gpu_profile_begin", error))?;
        }
    }
    let uploaded = renderer.painter.upload(prepared, &mut builder, output)?;
    let overlay_frame = renderer
        .overlay_pass
        .as_ref()
        .map(|pass| targets::overlay_framebuffer(&framebuffer, pass.clone()))
        .transpose()?;
    #[cfg(feature = "viewport")]
    let numeric_labels = if let Some((camera, data, rect)) = scene {
        let rect = vesta_viewport::render::ViewportRect {
            x: rect.x.min(extent[0]),
            y: rect.y.min(extent[1]),
            width: rect.width.min(extent[0].saturating_sub(rect.x)),
            height: rect.height.min(extent[1].saturating_sub(rect.y)),
        };
        let depth = framebuffer
            .attachments()
            .get(1)
            .cloned()
            .ok_or_else(|| RendererError::at("label_depth", "missing depth attachment"))?;
        if let Some(painter) = &mut renderer.scene {
            painter.prepare_labels(&mut builder, camera, data, rect, depth)?
        } else {
            None
        }
    } else {
        None
    };
    trace.mark(frame_profile::Stage::Upload);
    builder
        .begin_render_pass(
            RenderPassBeginInfo {
                clear_values: if !renderer.has_scene() {
                    vec![Some(options.clear.into())]
                } else if renderer.samples == SampleCount::Sample1 {
                    vec![Some(options.clear.into()), Some(1.0f32.into())]
                } else {
                    vec![Some(options.clear.into()), Some(1.0f32.into()), None]
                },
                ..RenderPassBeginInfo::framebuffer(framebuffer)
            },
            SubpassBeginInfo::default(),
        )
        .map_err(|error| RendererError::caused_by("begin_pass", error))?;
    #[cfg(feature = "viewport")]
    if let Some((camera, scene, rect)) = scene {
        let rect = vesta_viewport::render::ViewportRect {
            x: rect.x.min(extent[0]),
            y: rect.y.min(extent[1]),
            width: rect.width.min(extent[0].saturating_sub(rect.x)),
            height: rect.height.min(extent[1].saturating_sub(rect.y)),
        };
        if let Some(painter) = &renderer.scene {
            painter.record(&mut builder, camera, scene, rect)?;
        }
    }
    if let Some(frame) = overlay_frame {
        builder
            .end_render_pass(SubpassEndInfo::default())
            .map_err(|error| RendererError::caused_by("scene_end_pass", error))?;
        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![None; frame.attachments().len()],
                    ..RenderPassBeginInfo::framebuffer(frame)
                },
                SubpassBeginInfo::default(),
            )
            .map_err(|error| RendererError::caused_by("overlay_begin_pass", error))?;
    }
    #[cfg(feature = "viewport")]
    if let Some(labels) = numeric_labels
        && let Some(painter) = &renderer.scene
    {
        painter.record_labels(&mut builder, labels)?;
    }
    let bindings = renderer.painter.record(&mut builder, uploaded, extent)?;
    trace.recorded(bindings);
    builder
        .end_render_pass(SubpassEndInfo::default())
        .map_err(|error| RendererError::caused_by("end_pass", error))?;
    let pending = if let Some((ticket, pool)) = timing {
        // SAFETY: slot1 was reset in this builder, has not been written, and
        // this pool has never escaped to any other command buffer. The pending
        // owner and Vulkano keep-alives retain it through Submission retirement.
        unsafe { builder.write_timestamp(pool.clone(), 1, PipelineStage::BottomOfPipe) }
            .map_err(|error| RendererError::caused_by("gpu_profile_end", error))?;
        Some(ticket.submitted(pool))
    } else {
        None
    };
    Ok((builder, pending))
}
