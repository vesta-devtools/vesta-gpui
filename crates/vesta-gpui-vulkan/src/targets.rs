//! Swapchain-sized depth/MSAA attachments and render-pass compatibility.
use crate::RendererError;
use std::sync::Arc;
use vulkano::{
    device::{Device, DeviceOwned},
    format::Format,
    image::{
        Image, ImageCreateInfo, ImageLayout, ImageType, ImageUsage, SampleCount, view::ImageView,
    },
    memory::allocator::{AllocationCreateInfo, StandardMemoryAllocator},
    render_pass::{
        AttachmentDescription, AttachmentLoadOp, AttachmentReference, AttachmentStoreOp,
        Framebuffer, FramebufferCreateInfo, RenderPass, RenderPassCreateInfo, SubpassDescription,
    },
};
pub(super) fn render_pass(
    device: Arc<Device>,
    format: Format,
    samples: SampleCount,
    with_depth: bool,
) -> Result<Arc<RenderPass>, RendererError> {
    let mut attachments = vec![
        AttachmentDescription {
            format,
            samples,
            load_op: AttachmentLoadOp::Clear,
            store_op: if with_depth || samples == SampleCount::Sample1 {
                AttachmentStoreOp::Store
            } else {
                AttachmentStoreOp::DontCare
            },
            initial_layout: ImageLayout::Undefined,
            final_layout: ImageLayout::ColorAttachmentOptimal,
            ..Default::default()
        },
        AttachmentDescription {
            format: Format::D32_SFLOAT,
            samples,
            load_op: AttachmentLoadOp::Clear,
            store_op: AttachmentStoreOp::Store,
            initial_layout: ImageLayout::Undefined,
            final_layout: ImageLayout::DepthStencilReadOnlyOptimal,
            ..Default::default()
        },
    ];
    if !with_depth {
        attachments.pop();
    }
    let mut subpass = SubpassDescription {
        color_attachments: vec![Some(AttachmentReference {
            attachment: 0,
            layout: ImageLayout::ColorAttachmentOptimal,
            ..Default::default()
        })],
        depth_stencil_attachment: with_depth.then_some(AttachmentReference {
            attachment: 1,
            layout: ImageLayout::DepthStencilAttachmentOptimal,
            ..Default::default()
        }),
        ..Default::default()
    };
    if samples != SampleCount::Sample1 {
        attachments.push(AttachmentDescription {
            format,
            samples: SampleCount::Sample1,
            load_op: AttachmentLoadOp::DontCare,
            store_op: AttachmentStoreOp::Store,
            initial_layout: ImageLayout::Undefined,
            final_layout: ImageLayout::ColorAttachmentOptimal,
            ..Default::default()
        });
        subpass.color_resolve_attachments = vec![Some(AttachmentReference {
            attachment: 2,
            layout: ImageLayout::ColorAttachmentOptimal,
            ..Default::default()
        })];
    }
    RenderPass::new(
        device,
        RenderPassCreateInfo {
            attachments,
            subpasses: vec![subpass],
            ..Default::default()
        },
    )
    .map_err(|e| RendererError::caused_by("render_pass", e))
}

#[cfg(feature = "viewport")]
pub(super) fn overlay_pass(
    device: Arc<Device>,
    format: Format,
    samples: SampleCount,
) -> Result<Arc<RenderPass>, RendererError> {
    let mut attachments = vec![AttachmentDescription {
        format,
        samples,
        load_op: AttachmentLoadOp::Load,
        store_op: AttachmentStoreOp::Store,
        initial_layout: ImageLayout::ColorAttachmentOptimal,
        final_layout: ImageLayout::ColorAttachmentOptimal,
        ..Default::default()
    }];
    let mut subpass = SubpassDescription {
        color_attachments: vec![Some(AttachmentReference {
            attachment: 0,
            layout: ImageLayout::ColorAttachmentOptimal,
            ..Default::default()
        })],
        ..Default::default()
    };
    if samples != SampleCount::Sample1 {
        attachments.push(AttachmentDescription {
            format,
            samples: SampleCount::Sample1,
            load_op: AttachmentLoadOp::DontCare,
            store_op: AttachmentStoreOp::Store,
            initial_layout: ImageLayout::Undefined,
            final_layout: ImageLayout::ColorAttachmentOptimal,
            ..Default::default()
        });
        subpass.color_resolve_attachments = vec![Some(AttachmentReference {
            attachment: 1,
            layout: ImageLayout::ColorAttachmentOptimal,
            ..Default::default()
        })];
    }
    RenderPass::new(
        device,
        RenderPassCreateInfo {
            attachments,
            subpasses: vec![subpass],
            ..Default::default()
        },
    )
    .map_err(|e| RendererError::caused_by("overlay_pass", e))
}

pub(super) fn overlay_framebuffer(
    frame: &Arc<Framebuffer>,
    pass: Arc<RenderPass>,
) -> Result<Arc<Framebuffer>, RendererError> {
    let attachments = frame
        .attachments()
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 1)
        .map(|(_, view)| view.clone())
        .collect();
    Framebuffer::new(
        pass,
        FramebufferCreateInfo {
            attachments,
            ..Default::default()
        },
    )
    .map_err(|e| RendererError::caused_by("overlay_framebuffer", e))
}

#[cfg(feature = "viewport")]
pub(super) fn sample_count(device: &Device, color: Format) -> Result<SampleCount, RendererError> {
    let physical = device.physical_device();
    let supported = |format, usage| {
        physical
            .image_format_properties(vulkano::image::ImageFormatInfo {
                format,
                usage,
                ..Default::default()
            })
            .map_err(|e| RendererError::caused_by("attachment_capabilities", e))?
            .ok_or_else(|| {
                RendererError::at(
                    "attachment_capabilities",
                    "required attachment format is unsupported",
                )
            })
    };
    let color = supported(color, ImageUsage::COLOR_ATTACHMENT)?;
    let depth = supported(
        Format::D32_SFLOAT,
        ImageUsage::DEPTH_STENCIL_ATTACHMENT | ImageUsage::SAMPLED,
    )?;
    let counts = color.sample_counts
        & depth.sample_counts
        & physical.properties().framebuffer_color_sample_counts
        & physical.properties().framebuffer_depth_sample_counts;
    Ok(if counts.contains(SampleCount::Sample4.into()) {
        SampleCount::Sample4
    } else {
        SampleCount::Sample1
    })
}

pub(super) fn framebuffers(
    images: &[Arc<Image>],
    pass: Arc<RenderPass>,
    samples: SampleCount,
    with_depth: bool,
) -> Result<Vec<Arc<Framebuffer>>, RendererError> {
    if !with_depth {
        return images
            .iter()
            .map(|image| {
                let view = ImageView::new_default(image.clone())
                    .map_err(|e| RendererError::caused_by("image_view", e))?;
                Framebuffer::new(
                    pass.clone(),
                    FramebufferCreateInfo {
                        attachments: vec![view],
                        ..Default::default()
                    },
                )
                .map_err(|e| RendererError::caused_by("framebuffer", e))
            })
            .collect();
    }
    let image = images
        .first()
        .ok_or_else(|| RendererError::at("framebuffers", "empty swapchain"))?;
    let memory = Arc::new(StandardMemoryAllocator::new_default(image.device().clone()));
    let attachment = |format, usage| -> Result<Arc<ImageView>, RendererError> {
        let image = Image::new(
            memory.clone(),
            ImageCreateInfo {
                image_type: ImageType::Dim2d,
                format,
                extent: image.extent(),
                samples,
                usage,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
        )
        .map_err(|e| RendererError::caused_by("attachment_image", e))?;
        ImageView::new_default(image).map_err(|e| RendererError::caused_by("attachment_view", e))
    };
    // Each window submits through one queue and one predecessor future. Sharing
    // the transient targets keeps their exclusive writes tracked in every frame,
    // including when acquire alternates swapchain images. Queue ordering protects
    // reuse; command buffers retain old targets during swapchain reconstruction.
    let depth = attachment(
        Format::D32_SFLOAT,
        ImageUsage::DEPTH_STENCIL_ATTACHMENT | ImageUsage::SAMPLED,
    )?;
    let color = if samples == SampleCount::Sample1 {
        None
    } else {
        Some(attachment(image.format(), ImageUsage::COLOR_ATTACHMENT)?)
    };
    images
        .iter()
        .map(|image| {
            let view = ImageView::new_default(image.clone())
                .map_err(|e| RendererError::caused_by("image_view", e))?;
            Framebuffer::new(
                pass.clone(),
                FramebufferCreateInfo {
                    attachments: match &color {
                        None => vec![view, depth.clone()],
                        Some(color) => vec![color.clone(), depth.clone(), view],
                    },
                    ..Default::default()
                },
            )
            .map_err(|e| RendererError::caused_by("framebuffer", e))
        })
        .collect()
}
