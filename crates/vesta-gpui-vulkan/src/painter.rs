//! Paint preparation and bounded GPU texture ownership; no unsafe operations.
use crate::{
    RenderOptions, RendererError, checked_pipeline,
    glyph_atlas::{GlyphAtlas, SIDE, glyph_pixels},
    image,
};
use std::{
    collections::VecDeque,
    hash::{Hash, Hasher},
    sync::Arc,
};
use vesta_gpui::render::{
    self, ImageData, ImageResolver, RenderOutput, Scissor, ShapeVertex, TextureVertex,
};
use vesta_gpui::{
    Color, PaintCommand, PaintList, Point, RasterGlyph, Rect, TextInputContext, TextInputSnapshot,
    TextInputViewport, TextRasterizer, Viewport,
};
#[cfg(feature = "viewport")]
use vulkano::device::Device;
use vulkano::{
    buffer::{
        Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer,
        allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo},
    },
    command_buffer::{AutoCommandBufferBuilder, CopyBufferToImageInfo, PrimaryAutoCommandBuffer},
    descriptor_set::{
        DescriptorSet, WriteDescriptorSet, allocator::StandardDescriptorSetAllocator,
    },
    format::Format,
    image::{
        Image, ImageCreateInfo, ImageType, ImageUsage,
        sampler::{Filter, Sampler, SamplerCreateInfo},
        view::ImageView,
    },
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::{GraphicsPipeline, Pipeline},
    render_pass::RenderPass,
};

pub(super) enum Prepared {
    Shape {
        vertices: Vec<[f32; 12]>,
        scissor: Scissor,
    },
    Bitmap {
        vertices: [[f32; 4]; 6],
        scissor: Scissor,
        image: Arc<ImageData>,
    },
    Glyph {
        vertices: [[f32; 4]; 6],
        scissor: Scissor,
        glyph: RasterGlyph,
        color: Color,
    },
}

pub(super) enum Uploaded {
    Shape {
        vertices: Subbuffer<[[f32; 12]]>,
        scissor: Scissor,
    },
    Bitmap {
        vertices: Subbuffer<[[f32; 4]]>,
        set: Arc<DescriptorSet>,
        scissor: Scissor,
    },
}

fn shape(vertex: ShapeVertex) -> [f32; 12] {
    [
        vertex.position[0],
        vertex.position[1],
        vertex.local[0],
        vertex.local[1],
        vertex.size[0],
        vertex.size[1],
        vertex.radius_border[0],
        vertex.radius_border[1],
        vertex.color[0],
        vertex.color[1],
        vertex.color[2],
        vertex.color[3],
    ]
}
fn texture(vertex: TextureVertex) -> [f32; 4] {
    [
        vertex.position[0],
        vertex.position[1],
        vertex.uv[0],
        vertex.uv[1],
    ]
}
fn push_shape(
    out: &mut Vec<Prepared>,
    vertices: impl IntoIterator<Item = ShapeVertex>,
    scissor: Scissor,
) {
    if let Some(Prepared::Shape {
        vertices: last,
        scissor: previous,
    }) = out.last_mut()
        && *previous == scissor
    {
        last.extend(vertices.into_iter().map(shape));
    } else {
        out.push(Prepared::Shape {
            vertices: vertices.into_iter().map(shape).collect(),
            scissor,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn rectangle(
    out: &mut Vec<Prepared>,
    viewport: Viewport,
    rect: Rect,
    clip: Rect,
    radius: f32,
    border: f32,
    rotation: f32,
    color: Color,
) -> Result<(), RendererError> {
    if ![
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        radius,
        border,
        rotation,
        color.red,
        color.green,
        color.blue,
        color.alpha,
    ]
    .iter()
    .all(|v| v.is_finite())
    {
        return Err(RendererError::at(
            "paint_geometry",
            "non-finite rectangle or color",
        ));
    }
    if rect.is_empty() || rect.intersection(clip).is_none() {
        return Ok(());
    }
    if let Some(scissor) = render::scissor(clip, viewport) {
        push_shape(
            out,
            render::shape_vertices(rect, radius, border, rotation, color, viewport),
            scissor,
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn glyphs(
    out: &mut Vec<Prepared>,
    viewport: Viewport,
    bounds: Rect,
    view: TextInputViewport,
    snapshot: &TextInputSnapshot,
    color: Color,
    rasterizer: &mut TextRasterizer,
) -> Result<(), RendererError> {
    let Some(scissor) = render::scissor(view.clip, viewport) else {
        return Ok(());
    };
    rasterizer
        .visit_glyph_refs(snapshot, view, viewport.scale_factor, |glyph| {
            if glyph.width == 0 || glyph.height == 0 {
                return;
            }
            let scale = viewport.scale_factor;
            let rect = Rect::new(
                bounds.x + glyph.left as f32 / scale,
                bounds.y + glyph.top as f32 / scale,
                glyph.width as f32 / scale,
                glyph.height as f32 / scale,
            );
            // Match the wgpu atlas: filtering at a glyph edge samples a transparent gutter.
            let width = glyph.width + 2;
            let height = glyph.height + 2;
            let left = 1.0 / width as f32;
            let top = 1.0 / height as f32;
            let right = (glyph.width + 1) as f32 / width as f32;
            let bottom = (glyph.height + 1) as f32 / height as f32;
            out.push(Prepared::Glyph {
                vertices: render::texture_vertices_uv(
                    rect,
                    viewport,
                    [[left, top], [right, top], [right, bottom], [left, bottom]],
                )
                .map(texture),
                scissor,
                glyph: glyph.clone(),
                color,
            });
        })
        .map_err(|e| RendererError::at("raster", format!("{e:?}")))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare(
    viewport: Viewport,
    paint: &PaintList,
    context: &mut TextInputContext,
    rasterizer: &mut TextRasterizer,
    images: Option<&dyn ImageResolver>,
    max_image_dimension: u32,
    options: RenderOptions,
) -> Result<(Vec<Prepared>, RenderOutput), RendererError> {
    if !viewport.is_valid() {
        return Err(RendererError::at("viewport", "invalid viewport"));
    }
    let mut out = Vec::new();
    let mut output = RenderOutput::default();
    let mut clips = Vec::<Rect>::new();
    for command in paint.commands() {
        let clip = clips
            .last()
            .copied()
            .unwrap_or_else(|| viewport.logical_rect());
        let result = match command {
            PaintCommand::PushClip { rect, no_clip } => {
                if ![rect.x, rect.y, rect.width, rect.height]
                    .iter()
                    .all(|v| v.is_finite())
                {
                    return Err(RendererError::at("clip", "non-finite clip"));
                }
                clips.push(if *no_clip {
                    *rect
                } else {
                    clip.intersection(*rect).unwrap_or_default()
                });
                Ok(())
            }
            PaintCommand::PopClip => {
                if clips.pop().is_none() {
                    return Err(RendererError::at("clip", "unbalanced pop"));
                }
                Ok(())
            }
            PaintCommand::FillRoundedRect {
                rect,
                radius,
                rotation,
                color,
                ..
            } => rectangle(
                &mut out, viewport, *rect, clip, *radius, 0.0, *rotation, *color,
            ),
            PaintCommand::StrokeRoundedRect {
                rect,
                radius,
                width,
                rotation,
                color,
                ..
            } => rectangle(
                &mut out, viewport, *rect, clip, *radius, *width, *rotation, *color,
            ),
            PaintCommand::FillQuad { points, color, .. } => {
                if let Some(scissor) = render::scissor(clip, viewport) {
                    let vertices = render::quad_vertices(points, *color, viewport);
                    if !vertices.is_empty() {
                        push_shape(&mut out, vertices, scissor);
                    }
                }
                Ok(())
            }
            PaintCommand::FillPolyline {
                points,
                width,
                color,
                ..
            } => {
                if !width.is_finite() || points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite())
                {
                    return Err(RendererError::at("polyline", "non-finite polyline"));
                }
                if let Some(scissor) = render::scissor(clip, viewport)
                    && points.len() >= 2
                    && *width > 0.0
                {
                    let vertices = render::polyline_vertices(points, *width, *color, viewport);
                    if !vertices.is_empty() {
                        push_shape(&mut out, vertices, scissor);
                    }
                }
                Ok(())
            }
            PaintCommand::Text {
                bounds,
                clip,
                text,
                font_size,
                font_role,
                color,
                h_align,
                v_align,
                wrap,
                ..
            } => {
                if bounds
                    .intersection(*clip)
                    .and_then(|r| r.intersection(viewport.logical_rect()))
                    .is_none()
                {
                    continue;
                }
                let (snapshot, view) = render::static_text_view(
                    context, *bounds, *clip, text, *font_size, *font_role, *wrap, *h_align,
                    *v_align,
                )
                .ok_or_else(|| RendererError::at("text", "text layout failed"))?;
                glyphs(
                    &mut out, viewport, *bounds, view, &snapshot, *color, rasterizer,
                )
            }
            PaintCommand::TextInput {
                element,
                bounds,
                clip,
                snapshot,
                focused,
                color,
            } => {
                let view = snapshot.viewport(*bounds, *clip, Point::default());
                output.text_inputs.push((*element, view));
                let decorations = snapshot.decorations(
                    view,
                    *focused,
                    options.caret_visible,
                    viewport.scale_factor,
                );
                let mut selection = options.selection;
                if !focused {
                    selection.alpha *= 0.45;
                }
                for rect in decorations.selection {
                    rectangle(
                        &mut out, viewport, rect, view.clip, 0.0, 0.0, 0.0, selection,
                    )?;
                }
                glyphs(
                    &mut out, viewport, *bounds, view, snapshot, *color, rasterizer,
                )?;
                for rect in decorations.compose {
                    rectangle(&mut out, viewport, rect, view.clip, 0.0, 0.0, 0.0, *color)?;
                }
                if let Some(rect) = decorations.caret {
                    rectangle(
                        &mut out,
                        viewport,
                        rect,
                        view.clip,
                        0.0,
                        0.0,
                        0.0,
                        options.caret,
                    )?;
                }
                Ok(())
            }
            PaintCommand::Image {
                element,
                bounds,
                clip: image_clip,
                source,
            }
            | PaintCommand::FittedImage {
                element,
                bounds,
                clip: image_clip,
                source,
                ..
            } => {
                let Some(visible) = bounds
                    .intersection(*image_clip)
                    .and_then(|r| r.intersection(clip))
                else {
                    continue;
                };
                let Some(scissor) = render::scissor(visible, viewport) else {
                    continue;
                };
                if let Some(image) =
                    image::resolve(*element, source, images, max_image_dimension, &mut output)
                {
                    let presentation = match command {
                        PaintCommand::FittedImage { presentation, .. } => *presentation,
                        _ => vesta_gpui::ImagePresentation::default(),
                    };
                    let Some(placement) = presentation
                        .place(
                            *bounds,
                            vesta_gpui::Size::new(image.width as f32, image.height as f32),
                        )
                        .and_then(|placement| placement.clipped(visible))
                    else {
                        continue;
                    };
                    out.push(Prepared::Bitmap {
                        vertices: render::texture_vertices_uv(
                            placement.bounds,
                            viewport,
                            placement.texture_uv(),
                        )
                        .map(texture),
                        scissor,
                        image,
                    });
                    Ok(())
                } else {
                    let mut tint = options.selection;
                    tint.alpha *= 0.2;
                    rectangle(&mut out, viewport, *bounds, visible, 0.0, 0.0, 0.0, tint)
                }
            }
        };
        result.map_err(|mut e| {
            e.element = command.element_id();
            e
        })?;
    }
    if !clips.is_empty() {
        return Err(RendererError::at("clip", "unbalanced push"));
    }
    output.draw_calls = out.len();
    Ok((out, output))
}

use image::MAX_TEXTURE_BYTES;
const MAX_TEXTURES: usize = 4096;
struct CachedTexture {
    hash: u64,
    image: Arc<ImageData>,
    set: Arc<DescriptorSet>,
}

#[cfg(test)]
mod tests;
pub(super) struct Painter {
    memory: Arc<StandardMemoryAllocator>,
    vertices: SubbufferAllocator,
    atlas: GlyphAtlas,
    descriptors: Arc<StandardDescriptorSetAllocator>,
    sampler: Arc<Sampler>,
    shape: Arc<GraphicsPipeline>,
    texture: Arc<GraphicsPipeline>,
    cache: VecDeque<CachedTexture>,
    bytes: usize,
}
type Builder = AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>;

impl Painter {
    #[cfg(feature = "viewport")]
    pub fn set_pass(
        &mut self,
        device: Arc<Device>,
        pass: Arc<RenderPass>,
    ) -> Result<(), RendererError> {
        let shape = checked_pipeline::create(device.clone(), pass.clone(), false)?;
        let texture = checked_pipeline::create(device, pass, true)?;
        self.shape = shape;
        self.texture = texture;
        Ok(())
    }
    pub fn invalidate_cache(&mut self) {
        self.atlas.clear();
        self.cache.clear();
        self.bytes = 0;
    }
    pub fn new(gpu: &crate::GpuContext, pass: Arc<RenderPass>) -> Result<Self, RendererError> {
        let device = gpu.device.clone();
        let shape = checked_pipeline::create(device.clone(), pass.clone(), false)?;
        let texture = checked_pipeline::create(device.clone(), pass, true)?;
        let sampler = Sampler::new(
            device.clone(),
            SamplerCreateInfo {
                mag_filter: Filter::Linear,
                min_filter: Filter::Linear,
                address_mode: [vulkano::image::sampler::SamplerAddressMode::ClampToEdge; 3],
                ..Default::default()
            },
        )
        .map_err(|e| RendererError::caused_by("sampler", e))?;
        let memory = gpu.memory.clone();
        let vertices = SubbufferAllocator::new(
            memory.clone(),
            SubbufferAllocatorCreateInfo {
                arena_size: 1024 * 1024,
                buffer_usage: BufferUsage::VERTEX_BUFFER,
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
        );
        Ok(Self {
            memory,
            vertices,
            atlas: GlyphAtlas::default(),
            descriptors: Arc::new(StandardDescriptorSetAllocator::new(
                device,
                Default::default(),
            )),
            sampler,
            shape,
            texture,
            cache: VecDeque::new(),
            bytes: 0,
        })
    }
    fn bitmap(
        &mut self,
        image: Arc<ImageData>,
        builder: &mut Builder,
    ) -> Result<(Arc<DescriptorSet>, bool), RendererError> {
        if let Some(index) = self
            .cache
            .iter()
            .position(|e| Arc::ptr_eq(&e.image, &image))
        {
            let entry = self
                .cache
                .remove(index)
                .ok_or_else(|| RendererError::at("cache", "missing entry"))?;
            let set = entry.set.clone();
            self.cache.push_back(entry);
            return Ok((set, true));
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        image.width.hash(&mut hasher);
        image.height.hash(&mut hasher);
        image.rgba.hash(&mut hasher);
        let hash = hasher.finish();
        if let Some(index) = self
            .cache
            .iter()
            .position(|e| e.hash == hash && *e.image == *image)
        {
            let mut entry = self
                .cache
                .remove(index)
                .ok_or_else(|| RendererError::at("cache", "missing entry"))?;
            // Keep the host's current immutable allocation for future pointer hits.
            entry.image = image;
            let set = entry.set.clone();
            self.cache.push_back(entry);
            return Ok((set, true));
        }
        if image.rgba.len() > MAX_TEXTURE_BYTES {
            return Err(RendererError::at(
                "image_budget",
                "single texture exceeds cache budget",
            ));
        }
        let set = self.upload_texture(image.width, image.height, &image.rgba, builder)?;
        while self.cache.len() >= MAX_TEXTURES || self.bytes + image.rgba.len() > MAX_TEXTURE_BYTES
        {
            if let Some(old) = self.cache.pop_front() {
                self.bytes -= old.image.rgba.len();
            } else {
                break;
            }
        }
        self.bytes += image.rgba.len();
        self.cache.push_back(CachedTexture {
            hash,
            image,
            set: set.clone(),
        });
        Ok((set, false))
    }
    fn upload_texture(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
        builder: &mut Builder,
    ) -> Result<Arc<DescriptorSet>, RendererError> {
        let image = Image::new(
            self.memory.clone(),
            ImageCreateInfo {
                image_type: ImageType::Dim2d,
                format: Format::R8G8B8A8_SRGB,
                extent: [width, height, 1],
                usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                ..Default::default()
            },
        )
        .map_err(|e| RendererError::caused_by("texture", e))?;
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
            rgba.iter().copied(),
        )
        .map_err(|e| RendererError::caused_by("upload_buffer", e))?;
        builder
            .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(upload, image.clone()))
            .map_err(|e| RendererError::caused_by("upload_texture", e))?;
        let view = ImageView::new_default(image)
            .map_err(|e| RendererError::caused_by("texture_view", e))?;
        let set = DescriptorSet::new(
            self.descriptors.clone(),
            self.texture.layout().set_layouts()[0].clone(),
            [
                WriteDescriptorSet::image_view(0, view),
                WriteDescriptorSet::sampler(1, self.sampler.clone()),
            ],
            [],
        )
        .map_err(|e| RendererError::caused_by("texture_descriptor", e))?;
        Ok(set)
    }
    fn vertex_buffer<'a, T: BufferContents + Copy + 'a>(
        &self,
        slices: impl Iterator<Item = &'a [T]> + Clone,
    ) -> Result<Option<Subbuffer<[T]>>, RendererError> {
        let count = slices.clone().try_fold(0u64, |total, vertices| {
            total
                .checked_add(vertices.len() as u64)
                .ok_or_else(|| RendererError::at("vertex_count", "vertex count overflow"))
        })?;
        if count == 0 {
            return Ok(None);
        }
        let buffer = self
            .vertices
            .allocate_slice::<T>(count)
            .map_err(|e| RendererError::caused_by("vertex_arena", e))?;
        {
            let mut mapped = buffer
                .write()
                .map_err(|e| RendererError::caused_by("vertex_write", e))?;
            let mut offset = 0;
            for vertices in slices {
                let end = offset + vertices.len();
                mapped[offset..end].copy_from_slice(vertices);
                offset = end;
            }
        }
        Ok(Some(buffer))
    }
    pub fn upload(
        &mut self,
        prepared: Vec<Prepared>,
        builder: &mut Builder,
        output: &mut RenderOutput,
    ) -> Result<Vec<Uploaded>, RendererError> {
        enum Batch {
            Shape {
                vertices: Vec<[f32; 12]>,
                scissor: Scissor,
            },
            Bitmap {
                vertices: Vec<[f32; 4]>,
                set: Arc<DescriptorSet>,
                scissor: Scissor,
            },
        }
        let mut atlas = std::mem::take(&mut self.atlas);
        atlas.begin_frame();
        let placements = prepared
            .iter()
            .map(|draw| match draw {
                Prepared::Glyph { glyph, color, .. } => atlas.get_or_insert(glyph, *color),
                _ => None,
            })
            .collect::<Vec<_>>();
        for page in &mut atlas.pages {
            if page.dirty {
                // Never update an image sampled by an earlier submission. Publish
                // a new immutable page after all insertions, then batch its glyphs.
                page.set = Some(self.upload_texture(SIDE, SIDE, &page.rgba, builder)?);
                page.dirty = false;
            }
        }
        let mut batches = Vec::<Batch>::new();
        for (draw, placement) in prepared.into_iter().zip(placements) {
            let (vertices, set, scissor) = match draw {
                Prepared::Shape { vertices, scissor } => {
                    batches.push(Batch::Shape { vertices, scissor });
                    continue;
                }
                Prepared::Bitmap {
                    vertices,
                    scissor,
                    image,
                } => {
                    let (set, reused) = self.bitmap(image, builder)?;
                    if reused {
                        output.bitmap_reuses += 1;
                    } else {
                        output.bitmap_uploads += 1;
                    }
                    (vertices, set, scissor)
                }
                Prepared::Glyph {
                    mut vertices,
                    scissor,
                    glyph,
                    color,
                } => {
                    let (set, reused) = if let Some(at) = placement {
                        for vertex in &mut vertices {
                            vertex[2] = (at.x as f32 + vertex[2] * at.width as f32) / SIDE as f32;
                            vertex[3] = (at.y as f32 + vertex[3] * at.height as f32) / SIDE as f32;
                        }
                        let set = atlas.pages[at.page].set.clone().ok_or_else(|| {
                            RendererError::at("glyph_atlas", "page was not uploaded")
                        })?;
                        (set, at.reused)
                    } else {
                        self.bitmap(
                            Arc::new(ImageData {
                                width: glyph.width + 2,
                                height: glyph.height + 2,
                                rgba: glyph_pixels(&glyph, color),
                            }),
                            builder,
                        )?
                    };
                    if reused {
                        output.glyph_reuses += 1;
                    } else {
                        output.glyph_uploads += 1;
                    }
                    (vertices, set, scissor)
                }
            };
            if let Some(Batch::Bitmap {
                vertices: previous,
                set: previous_set,
                scissor: previous_scissor,
            }) = batches.last_mut()
                && *previous_scissor == scissor
                && Arc::ptr_eq(previous_set, &set)
            {
                previous.extend(vertices);
            } else {
                batches.push(Batch::Bitmap {
                    vertices: vertices.into(),
                    set,
                    scissor,
                });
            }
        }
        self.atlas = atlas;
        let shapes = self.vertex_buffer(batches.iter().filter_map(|batch| match batch {
            Batch::Shape { vertices, .. } => Some(vertices.as_slice()),
            _ => None,
        }))?;
        let bitmaps = self.vertex_buffer(batches.iter().filter_map(|batch| match batch {
            Batch::Bitmap { vertices, .. } => Some(vertices.as_slice()),
            _ => None,
        }))?;
        let mut shape_offset = 0u64;
        let mut bitmap_offset = 0u64;
        let result = batches
            .into_iter()
            .map(|batch| match batch {
                Batch::Shape { vertices, scissor } => {
                    let end = shape_offset + vertices.len() as u64;
                    let buffer = shapes
                        .as_ref()
                        .ok_or_else(|| RendererError::at("vertex_count", "empty shape batch"))?
                        .clone()
                        .slice(shape_offset..end);
                    shape_offset = end;
                    Ok(Uploaded::Shape {
                        vertices: buffer,
                        scissor,
                    })
                }
                Batch::Bitmap {
                    vertices,
                    set,
                    scissor,
                } => {
                    let end = bitmap_offset + vertices.len() as u64;
                    let buffer = bitmaps
                        .as_ref()
                        .ok_or_else(|| RendererError::at("vertex_count", "empty bitmap batch"))?
                        .clone()
                        .slice(bitmap_offset..end);
                    bitmap_offset = end;
                    Ok(Uploaded::Bitmap {
                        vertices: buffer,
                        set,
                        scissor,
                    })
                }
            })
            .collect::<Result<Vec<_>, RendererError>>()?;
        output.draw_calls = result.len();
        Ok(result)
    }
    pub fn record(
        &self,
        builder: &mut Builder,
        uploaded: Vec<Uploaded>,
        extent: [u32; 2],
    ) -> Result<checked_pipeline::UiBindings, RendererError> {
        checked_pipeline::record(builder, &self.shape, &self.texture, uploaded, extent)
    }
}
