//! Prepared numeric annotations. All uploads precede the render pass.
use super::*;
use vesta_viewport::display::{NumericAtlas, NumericLabelStyle};
use vulkano::{
    command_buffer::CopyBufferToImageInfo,
    descriptor_set::DescriptorImageViewInfo,
    device::DeviceOwned,
    format::Format,
    image::{
        Image, ImageCreateInfo, ImageLayout, ImageType, ImageUsage, SampleCount,
        sampler::{Filter, Sampler, SamplerCreateInfo},
        view::ImageView,
    },
};

struct CachedAtlas {
    source: Arc<NumericAtlas>,
    view: Arc<ImageView>,
    sampler: Arc<Sampler>,
}
pub(super) struct LabelPainter {
    pipeline: Arc<GraphicsPipeline>,
    memory: Arc<StandardMemoryAllocator>,
    descriptors: Arc<StandardDescriptorSetAllocator>,
    samples: u32,
    atlas: Option<CachedAtlas>,
}
pub(crate) struct PreparedLabels {
    camera: Arc<DescriptorSet>,
    sources: Arc<DescriptorSet>,
    depth: Arc<DescriptorSet>,
    count: u32,
    rect: ViewportRect,
    clip: ViewportRect,
}

fn valid(style: NumericLabelStyle) -> bool {
    style
        .cell_size
        .into_iter()
        .chain([
            style.glyph_scale,
            style.advance_fraction,
            style.scale_factor,
        ])
        .all(|v| v.is_finite() && v > 0.0)
        && style.glyph_scale <= 1.0
        && (1..=10).contains(&style.max_digits)
        && style.atlas_cells == 10
        && style
            .offset
            .into_iter()
            .chain(style.color)
            .all(f32::is_finite)
        && style.depth_tolerance.is_finite()
        && style.depth_tolerance >= 0.0
        && style.occlusion_fraction.is_finite()
        && style.occlusion_fraction > 0.0
        && style.occlusion_fraction <= 1.0
}

impl LabelPainter {
    pub(super) fn new(
        device: Arc<Device>,
        pass: Arc<RenderPass>,
        samples: SampleCount,
        memory: Arc<StandardMemoryAllocator>,
        descriptors: Arc<StandardDescriptorSetAllocator>,
    ) -> Result<Self, RendererError> {
        Ok(Self {
            pipeline: checked_pipeline::create_viewport_labels(device, pass, samples)?,
            memory,
            descriptors,
            samples: samples as u32,
            atlas: None,
        })
    }
    pub(super) fn invalidate(&mut self) {
        self.atlas = None;
    }
    fn atlas(
        &mut self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        source: &Arc<NumericAtlas>,
    ) -> Result<(), RendererError> {
        if self
            .atlas
            .as_ref()
            .is_some_and(|a| Arc::ptr_eq(&a.source, source))
        {
            return Ok(());
        }
        if source.width == 0
            || source.height == 0
            || source.width > 8192
            || source.height > 8192
            || source.width % 10 != 0
            || source.rgba.len() != source.width as usize * source.height as usize * 4
        {
            return Err(RendererError::at(
                "label_atlas",
                "invalid numeric atlas pixels",
            ));
        }
        let image = Image::new(
            self.memory.clone(),
            ImageCreateInfo {
                image_type: ImageType::Dim2d,
                format: Format::R8G8B8A8_SRGB,
                extent: [source.width, source.height, 1],
                usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
        )
        .map_err(|e| RendererError::caused_by("label_atlas_image", e))?;
        let upload = Buffer::from_iter(
            self.memory.clone(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_SRC,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            source.rgba.iter().copied(),
        )
        .map_err(|e| RendererError::caused_by("label_atlas_upload", e))?;
        builder
            .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(upload, image.clone()))
            .map_err(|e| RendererError::caused_by("label_atlas_copy", e))?;
        let view = ImageView::new_default(image)
            .map_err(|e| RendererError::caused_by("label_atlas_view", e))?;
        let sampler = Sampler::new(
            self.pipeline.device().clone(),
            SamplerCreateInfo {
                mag_filter: Filter::Linear,
                min_filter: Filter::Linear,
                ..Default::default()
            },
        )
        .map_err(|e| RendererError::caused_by("label_sampler", e))?;
        self.atlas = Some(CachedAtlas {
            source: source.clone(),
            view,
            sampler,
        });
        Ok(())
    }
    pub(super) fn prepare(
        &mut self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        camera: Camera,
        scene: &ViewportScene,
        rect: ViewportRect,
        clip: ViewportRect,
        depth_view: Arc<ImageView>,
    ) -> Result<Option<PreparedLabels>, RendererError> {
        let Some(labels) = scene
            .sources
            .as_ref()
            .and_then(|s| s.numeric_labels.as_ref())
        else {
            return Ok(None);
        };
        let Some(atlas) = &labels.atlas else {
            return Ok(None);
        };
        if rect.width == 0 || rect.height == 0 {
            return Ok(None);
        }
        let selected = labels
            .points
            .iter()
            .filter(|_| scene.display.point_numbers)
            .chain(
                labels
                    .primitives
                    .iter()
                    .filter(|_| scene.display.primitive_numbers),
            )
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Ok(None);
        }
        let style = labels.style;
        if !valid(style)
            || selected.iter().any(|label| {
                !label.position.into_iter().all(f32::is_finite)
                    || label.number.checked_ilog10().map_or(1, |n| n + 1) > style.max_digits
            })
        {
            return Err(RendererError::at(
                "label_style",
                "invalid numeric source or capacity",
            ));
        }
        let count = u32::try_from(selected.len())
            .ok()
            .and_then(|n| n.checked_mul(style.max_digits * 6))
            .ok_or_else(|| RendererError::at("label_count", "numeric label count overflow"))?;
        let policy = [
            [
                rect.x as f32,
                rect.y as f32,
                rect.width as f32,
                rect.height as f32,
            ],
            [
                style.cell_size[0] * style.scale_factor,
                style.cell_size[1] * style.scale_factor,
                style.glyph_scale,
                style.advance_fraction,
            ],
            style.color,
            [
                style.offset[0] * style.scale_factor,
                style.offset[1] * style.scale_factor,
                style.max_digits as f32,
                style.atlas_cells as f32,
            ],
            [
                style.depth_tolerance,
                self.samples as f32,
                (self.samples as f32 * style.occlusion_fraction).ceil(),
                0.0,
            ],
            [
                clip.x as f32,
                clip.y as f32,
                clip.width as f32,
                clip.height as f32,
            ],
        ];
        if !policy.iter().flatten().all(|v| v.is_finite()) {
            return Err(RendererError::at(
                "label_style",
                "nonfinite physical label policy",
            ));
        }
        self.atlas(builder, atlas)?;
        let atlas = self
            .atlas
            .as_ref()
            .ok_or_else(|| RendererError::at("label_atlas", "missing uploaded atlas"))?;
        let allocation = AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_HOST
                | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ..Default::default()
        };
        let camera_buffer = Buffer::from_data(
            self.memory.clone(),
            BufferCreateInfo {
                usage: BufferUsage::UNIFORM_BUFFER,
                ..Default::default()
            },
            allocation.clone(),
            view_projection(camera, rect.width as f32 / rect.height as f32),
        )
        .map_err(|e| RendererError::caused_by("label_camera", e))?;
        let policy_buffer = Buffer::from_data(
            self.memory.clone(),
            BufferCreateInfo {
                usage: BufferUsage::UNIFORM_BUFFER,
                ..Default::default()
            },
            allocation.clone(),
            policy,
        )
        .map_err(|e| RendererError::caused_by("label_policy", e))?;
        // Explicit bits preserve the complete u32 scalar alongside f32 positions.
        let sources_buffer = Buffer::from_iter(
            self.memory.clone(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER,
                ..Default::default()
            },
            allocation,
            selected.into_iter().map(|s| {
                [
                    s.position[0].to_bits(),
                    s.position[1].to_bits(),
                    s.position[2].to_bits(),
                    0,
                    s.number,
                    0,
                    0,
                    0,
                ]
            }),
        )
        .map_err(|e| RendererError::caused_by("label_sources", e))?;
        let layouts = self.pipeline.layout().set_layouts();
        let camera = DescriptorSet::new(
            self.descriptors.clone(),
            layouts[0].clone(),
            [WriteDescriptorSet::buffer(0, camera_buffer)],
            [],
        )
        .map_err(|e| RendererError::caused_by("label_camera_set", e))?;
        let sources = DescriptorSet::new(
            self.descriptors.clone(),
            layouts[1].clone(),
            [
                WriteDescriptorSet::buffer(0, sources_buffer),
                WriteDescriptorSet::buffer(1, policy_buffer),
                WriteDescriptorSet::image_view(2, atlas.view.clone()),
                WriteDescriptorSet::sampler(3, atlas.sampler.clone()),
            ],
            [],
        )
        .map_err(|e| RendererError::caused_by("label_source_set", e))?;
        let depth = DescriptorSet::new(
            self.descriptors.clone(),
            layouts[2].clone(),
            [WriteDescriptorSet::image_view_with_layout(
                0,
                DescriptorImageViewInfo {
                    image_view: depth_view,
                    image_layout: ImageLayout::DepthStencilReadOnlyOptimal,
                },
            )],
            [],
        )
        .map_err(|e| RendererError::caused_by("label_depth_set", e))?;
        Ok(Some(PreparedLabels {
            camera,
            sources,
            depth,
            count,
            rect,
            clip,
        }))
    }
    pub(super) fn record(
        &self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        labels: PreparedLabels,
    ) -> Result<(), RendererError> {
        let rect = labels.rect;
        let clip = labels.clip;
        builder
            .set_viewport(
                0,
                [Viewport {
                    offset: [rect.x as f32, rect.y as f32],
                    extent: [rect.width as f32, rect.height as f32],
                    depth_range: 0.0..=1.0,
                }]
                .into_iter()
                .collect(),
            )
            .map_err(|e| RendererError::caused_by("label_viewport", e))?;
        builder
            .set_scissor(
                0,
                [Scissor {
                    offset: [clip.x, clip.y],
                    extent: [clip.width, clip.height],
                }]
                .into_iter()
                .collect(),
            )
            .map_err(|e| RendererError::caused_by("label_scissor", e))?;
        builder
            .bind_pipeline_graphics(self.pipeline.clone())
            .map_err(|e| RendererError::caused_by("label_pipeline", e))?;
        builder
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.pipeline.layout().clone(),
                0,
                (labels.camera, labels.sources, labels.depth),
            )
            .map_err(|e| RendererError::caused_by("label_bindings", e))?;
        checked_pipeline::draw_viewport(builder, labels.count)
    }
}
