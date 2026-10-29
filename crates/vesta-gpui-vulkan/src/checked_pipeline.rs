//! ADR-0304/0577: private checked draw/shader and owned timestamp recording.
pub(crate) mod recording;
use crate::{RendererError, painter::Uploaded};
use std::sync::Arc;
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer},
    device::Device,
    format::Format,
    pipeline::{
        DynamicState, GraphicsPipeline, Pipeline, PipelineBindPoint, PipelineLayout,
        PipelineShaderStageCreateInfo,
        graphics::{
            GraphicsPipelineCreateInfo,
            color_blend::{
                AttachmentBlend, BlendFactor, ColorBlendAttachmentState, ColorBlendState,
            },
            input_assembly::InputAssemblyState,
            multisample::MultisampleState,
            rasterization::RasterizationState,
            vertex_input::{
                VertexInputAttributeDescription, VertexInputBindingDescription, VertexInputRate,
                VertexInputState,
            },
            viewport::{Scissor, Viewport, ViewportState},
        },
        layout::PipelineDescriptorSetLayoutCreateInfo,
    },
    render_pass::{RenderPass, Subpass},
    shader::{ShaderModule, ShaderModuleCreateInfo},
};

fn spirv(textured: bool, stage: naga::ShaderStage) -> Result<Vec<u32>, RendererError> {
    let source = if textured {
        vesta_gpui::render::TEXTURE_SHADER
    } else {
        vesta_gpui::render::SHAPE_SHADER
    };
    let module = naga::front::wgsl::parse_str(source)
        .map_err(|e| RendererError::caused_by("shader_parse", e))?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map_err(|e| RendererError::caused_by("shader_validate", e))?;
    let entry_point = match stage {
        naga::ShaderStage::Vertex => "vertex",
        _ => "fragment",
    }
    .to_owned();
    let options = naga::back::spv::Options::default();
    let pipeline_options = naga::back::spv::PipelineOptions {
        shader_stage: stage,
        entry_point,
    };
    naga::back::spv::write_vec(&module, &info, &options, Some(&pipeline_options))
        .map_err(|e| RendererError::caused_by("spirv", e))
}

fn shader(
    device: Arc<Device>,
    textured: bool,
    stage: naga::ShaderStage,
) -> Result<Arc<ShaderModule>, RendererError> {
    let words = spirv(textured, stage)?;
    // SAFETY: only embedded, Naga-parsed and fully validated WGSL is compiled here.
    // Naga's SPIR-V writer emits valid code for the specified entry point. No caller
    // can supply SPIR-V, shaders, specialization constants or pointer-bearing data.
    unsafe { ShaderModule::new(device, ShaderModuleCreateInfo::new(&words)) }
        .map_err(|e| RendererError::caused_by("shader_module", e))
}

#[cfg(feature = "viewport")]
#[derive(Clone, Copy)]
enum ViewportGeometry {
    Mesh(bool),
    Lines,
    Triangles,
    Markers,
    Labels { multisampled: bool },
}
#[cfg(feature = "viewport")]
impl ViewportGeometry {
    fn entry(self, stage: naga::ShaderStage) -> &'static str {
        match (self, stage) {
            (Self::Mesh(_), naga::ShaderStage::Vertex) => "mesh_vertex",
            (Self::Mesh(_), _) => "mesh_fragment",
            (Self::Markers, naga::ShaderStage::Vertex) => "marker_vertex",
            (Self::Markers, _) => "marker_fragment",
            (Self::Labels { .. }, naga::ShaderStage::Vertex) => "label_vertex",
            (Self::Labels { .. }, _) => "label_fragment",
            (_, naga::ShaderStage::Vertex) => "line_vertex",
            _ => "line_fragment",
        }
    }
}
#[cfg(feature = "viewport")]
fn viewport_spirv(
    kind: ViewportGeometry,
    stage: naga::ShaderStage,
) -> Result<Vec<u32>, RendererError> {
    let label_source;
    let source = if let ViewportGeometry::Labels { multisampled } = kind {
        label_source = if multisampled {
            vesta_viewport::render::NUMERIC_LABEL_SHADER.to_owned()
        } else {
            vesta_viewport::render::NUMERIC_LABEL_SHADER
                .replace("texture_depth_multisampled_2d", "texture_depth_2d")
                .replace(
                    "textureLoad(label_depth, pixel, sample)",
                    "textureLoad(label_depth, pixel, 0)",
                )
        };
        label_source.as_str()
    } else {
        vesta_viewport::render::VIEWPORT_SHADER
    };
    let module = naga::front::wgsl::parse_str(source)
        .map_err(|e| RendererError::caused_by("viewport_shader_parse", e))?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map_err(|e| RendererError::caused_by("viewport_shader_validate", e))?;
    let entry_point = kind.entry(stage);
    naga::back::spv::write_vec(
        &module,
        &info,
        &naga::back::spv::Options::default(),
        Some(&naga::back::spv::PipelineOptions {
            shader_stage: stage,
            entry_point: entry_point.into(),
        }),
    )
    .map_err(|e| RendererError::caused_by("viewport_spirv", e))
}
#[cfg(feature = "viewport")]
fn viewport_shader(
    device: Arc<Device>,
    kind: ViewportGeometry,
    stage: naga::ShaderStage,
) -> Result<Arc<ShaderModule>, RendererError> {
    let words = viewport_spirv(kind, stage)?;
    // SAFETY: only the embedded shared Viewport WGSL, fully validated by Naga,
    // reaches this call. The shader reads typed uniforms and vertices only.
    unsafe { ShaderModule::new(device, ShaderModuleCreateInfo::new(&words)) }
        .map_err(|e| RendererError::caused_by("viewport_shader_module", e))
}
#[cfg(feature = "viewport")]
pub(super) fn draw_viewport(
    builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    count: u32,
) -> Result<(), RendererError> {
    // SAFETY: callers bind only create_viewport pipelines with initialized typed
    // buffers of exactly count vertices. Mesh indices are checked
    // before upload. Safe binding and checked draw validate all pipeline state;
    // Labels instead read one bounded storage record per max_digits*6 vertices;
    // their caller validates capacity and derives count from that record length.
    // Embedded shaders have no storage writes or externally supplied code.
    unsafe { builder.draw(count, 1, 0, 0) }
        .map(|_| ())
        .map_err(|e| RendererError::caused_by("viewport_draw", e))
}
#[cfg(feature = "viewport")]
pub(super) fn draw_viewport_indexed(
    builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    count: u32,
) -> Result<(), RendererError> {
    // SAFETY: ScenePainter checks every index against its initialized vertex buffer
    // before upload and binds exactly count u32 indices. Vertex offset is zero and
    // there is one instance. Embedded validated shaders read uniforms/vertices only;
    // checked draw also validates compatible pipeline, descriptors and dynamic state.
    unsafe { builder.draw_indexed(count, 1, 0, 0, 0) }
        .map(|_| ())
        .map_err(|e| RendererError::caused_by("viewport_indexed_draw", e))
}
#[cfg(feature = "viewport")]
pub(super) enum ViewportDepth {
    Disabled,
    ReadOnly,
    ReadWrite,
}
#[cfg(feature = "viewport")]
pub(super) fn create_viewport(
    device: Arc<Device>,
    pass: Arc<RenderPass>,
    mesh: bool,
    depth: ViewportDepth,
    samples: vulkano::image::SampleCount,
) -> Result<Arc<GraphicsPipeline>, RendererError> {
    create_viewport_geometry(
        device,
        pass,
        if mesh {
            ViewportGeometry::Mesh(true)
        } else {
            ViewportGeometry::Lines
        },
        depth,
        samples,
    )
}
#[cfg(feature = "viewport")]
pub(super) fn create_viewport_two_sided(
    device: Arc<Device>,
    pass: Arc<RenderPass>,
    depth: ViewportDepth,
    samples: vulkano::image::SampleCount,
) -> Result<Arc<GraphicsPipeline>, RendererError> {
    create_viewport_geometry(device, pass, ViewportGeometry::Mesh(false), depth, samples)
}
#[cfg(feature = "viewport")]
pub(super) fn create_viewport_unlit_triangles(
    device: Arc<Device>,
    pass: Arc<RenderPass>,
    samples: vulkano::image::SampleCount,
) -> Result<Arc<GraphicsPipeline>, RendererError> {
    create_viewport_geometry(
        device,
        pass,
        ViewportGeometry::Triangles,
        ViewportDepth::ReadOnly,
        samples,
    )
}
#[cfg(feature = "viewport")]
pub(super) fn create_viewport_labels(
    device: Arc<Device>,
    pass: Arc<RenderPass>,
    samples: vulkano::image::SampleCount,
) -> Result<Arc<GraphicsPipeline>, RendererError> {
    create_viewport_geometry(
        device,
        pass,
        ViewportGeometry::Labels {
            multisampled: samples != vulkano::image::SampleCount::Sample1,
        },
        ViewportDepth::Disabled,
        samples,
    )
}
#[cfg(feature = "viewport")]
pub(super) fn create_viewport_markers(
    device: Arc<Device>,
    pass: Arc<RenderPass>,
    samples: vulkano::image::SampleCount,
) -> Result<Arc<GraphicsPipeline>, RendererError> {
    create_viewport_geometry(
        device,
        pass,
        ViewportGeometry::Markers,
        ViewportDepth::ReadOnly,
        samples,
    )
}
#[cfg(feature = "viewport")]
fn create_viewport_geometry(
    device: Arc<Device>,
    pass: Arc<RenderPass>,
    kind: ViewportGeometry,
    depth: ViewportDepth,
    samples: vulkano::image::SampleCount,
) -> Result<Arc<GraphicsPipeline>, RendererError> {
    let vertex = viewport_shader(device.clone(), kind, naga::ShaderStage::Vertex)?
        .entry_point(kind.entry(naga::ShaderStage::Vertex))
        .ok_or_else(|| RendererError::at("vertex_entry", "missing vertex"))?;
    let fragment = viewport_shader(device.clone(), kind, naga::ShaderStage::Fragment)?
        .entry_point(kind.entry(naga::ShaderStage::Fragment))
        .ok_or_else(|| RendererError::at("fragment_entry", "missing fragment"))?;
    let stages = [
        PipelineShaderStageCreateInfo::new(vertex),
        PipelineShaderStageCreateInfo::new(fragment),
    ];
    let mut descriptor_info = PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages);
    if matches!(
        kind,
        ViewportGeometry::Mesh(_) | ViewportGeometry::Lines | ViewportGeometry::Triangles
    ) {
        // The same camera set is bound by mesh, grid and normal-ribbon passes.
        // Mesh lighting reads it in fragment as well; normalize visibility so
        // independently reflected entry points retain compatible set layouts.
        let camera = descriptor_info
            .set_layouts
            .first_mut()
            .and_then(|set| set.bindings.get_mut(&0))
            .ok_or_else(|| RendererError::at("viewport_camera_layout", "missing camera binding"))?;
        camera.stages =
            vulkano::shader::ShaderStages::VERTEX | vulkano::shader::ShaderStages::FRAGMENT;
    }
    let layout_info = descriptor_info
        .into_pipeline_layout_create_info(device.clone())
        .map_err(|e| RendererError::caused_by("layout_info", e))?;
    let layout = PipelineLayout::new(device.clone(), layout_info)
        .map_err(|e| RendererError::caused_by("pipeline_layout", e))?;
    let attributes = if matches!(kind, ViewportGeometry::Markers) {
        vec![
            (Format::R32G32B32A32_SFLOAT, 0),
            (Format::R32G32_SFLOAT, 16),
            (Format::R32G32B32A32_SFLOAT, 24),
            (Format::R32_SFLOAT, 40),
        ]
    } else if matches!(kind, ViewportGeometry::Mesh(_)) {
        vec![
            (Format::R32G32B32_SFLOAT, 0),
            (Format::R32G32B32_SFLOAT, 12),
            (Format::R32G32B32_SFLOAT, 24),
        ]
    } else {
        vec![
            (Format::R32G32B32_SFLOAT, 0),
            (Format::R32G32B32A32_SFLOAT, 12),
        ]
    };
    let input = if matches!(kind, ViewportGeometry::Labels { .. }) {
        VertexInputState::default()
    } else {
        VertexInputState {
            bindings: [(
                0,
                VertexInputBindingDescription {
                    stride: match kind {
                        ViewportGeometry::Mesh(_) => vesta_viewport::render::MESH_VERTEX_STRIDE,
                        ViewportGeometry::Markers => 44,
                        _ => 28,
                    },
                    input_rate: VertexInputRate::Vertex,
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
            attributes: attributes
                .into_iter()
                .enumerate()
                .map(|(i, (format, offset))| {
                    (
                        i as u32,
                        VertexInputAttributeDescription {
                            binding: 0,
                            format,
                            offset,
                            ..Default::default()
                        },
                    )
                })
                .collect(),
            ..Default::default()
        }
    };
    let subpass =
        Subpass::from(pass, 0).ok_or_else(|| RendererError::at("subpass", "missing subpass"))?;
    GraphicsPipeline::new(
        device,
        None,
        GraphicsPipelineCreateInfo {
            stages: stages.into_iter().collect(),
            vertex_input_state: Some(input),
            input_assembly_state: Some(InputAssemblyState {
                topology: if !matches!(kind, ViewportGeometry::Lines) {
                    vulkano::pipeline::graphics::input_assembly::PrimitiveTopology::TriangleList
                } else {
                    vulkano::pipeline::graphics::input_assembly::PrimitiveTopology::LineList
                },
                ..Default::default()
            }),
            viewport_state: Some(ViewportState::default()),
            rasterization_state: Some(RasterizationState {
                cull_mode: if matches!(kind, ViewportGeometry::Mesh(true)) {
                    vulkano::pipeline::graphics::rasterization::CullMode::Back
                } else {
                    vulkano::pipeline::graphics::rasterization::CullMode::None
                },
                ..Default::default()
            }),
            multisample_state: Some(MultisampleState {
                rasterization_samples: samples,
                ..Default::default()
            }),
            depth_stencil_state: (!matches!(kind, ViewportGeometry::Labels { .. })).then_some(
                vulkano::pipeline::graphics::depth_stencil::DepthStencilState {
                    depth: (!matches!(depth, ViewportDepth::Disabled)).then_some(
                        vulkano::pipeline::graphics::depth_stencil::DepthState {
                            write_enable: matches!(depth, ViewportDepth::ReadWrite),
                            compare_op:
                                vulkano::pipeline::graphics::depth_stencil::CompareOp::LessOrEqual,
                        },
                    ),
                    ..Default::default()
                },
            ),
            color_blend_state: Some(ColorBlendState::with_attachment_states(
                1,
                ColorBlendAttachmentState {
                    blend: Some(AttachmentBlend::alpha()),
                    ..Default::default()
                },
            )),
            dynamic_state: [DynamicState::Viewport, DynamicState::Scissor]
                .into_iter()
                .collect(),
            subpass: Some(subpass.into()),
            ..GraphicsPipelineCreateInfo::layout(layout)
        },
    )
    .map_err(|e| RendererError::caused_by("pipeline", e))
}

pub(super) fn create(
    device: Arc<Device>,
    pass: Arc<RenderPass>,
    textured: bool,
) -> Result<Arc<GraphicsPipeline>, RendererError> {
    let vertex = shader(device.clone(), textured, naga::ShaderStage::Vertex)?
        .entry_point("vertex")
        .ok_or_else(|| RendererError::at("vertex_entry", "missing vertex"))?;
    let fragment = shader(device.clone(), textured, naga::ShaderStage::Fragment)?
        .entry_point("fragment")
        .ok_or_else(|| RendererError::at("fragment_entry", "missing fragment"))?;
    let stages = [
        PipelineShaderStageCreateInfo::new(vertex),
        PipelineShaderStageCreateInfo::new(fragment),
    ];
    let layout_info = PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
        .into_pipeline_layout_create_info(device.clone())
        .map_err(|e| RendererError::caused_by("layout_info", e))?;
    let layout = PipelineLayout::new(device.clone(), layout_info)
        .map_err(|e| RendererError::caused_by("pipeline_layout", e))?;
    let attributes = if textured {
        vec![(Format::R32G32_SFLOAT, 0), (Format::R32G32_SFLOAT, 8)]
    } else {
        vec![
            (Format::R32G32_SFLOAT, 0),
            (Format::R32G32_SFLOAT, 8),
            (Format::R32G32_SFLOAT, 16),
            (Format::R32G32_SFLOAT, 24),
            (Format::R32G32B32A32_SFLOAT, 32),
        ]
    };
    let input = VertexInputState {
        bindings: [(
            0,
            VertexInputBindingDescription {
                stride: if textured { 16 } else { 48 },
                input_rate: VertexInputRate::Vertex,
                ..Default::default()
            },
        )]
        .into_iter()
        .collect(),
        attributes: attributes
            .into_iter()
            .enumerate()
            .map(|(i, (format, offset))| {
                (
                    i as u32,
                    VertexInputAttributeDescription {
                        binding: 0,
                        format,
                        offset,
                        ..Default::default()
                    },
                )
            })
            .collect(),
        ..Default::default()
    };
    let samples = pass.attachments()[0].samples;
    let index = pass.subpasses().len().saturating_sub(1);
    let depth = pass.subpasses()[index].depth_stencil_attachment.is_some();
    let subpass = Subpass::from(pass, index as u32)
        .ok_or_else(|| RendererError::at("subpass", "missing subpass"))?;
    GraphicsPipeline::new(
        device,
        None,
        GraphicsPipelineCreateInfo {
            stages: stages.into_iter().collect(),
            vertex_input_state: Some(input),
            input_assembly_state: Some(InputAssemblyState::default()),
            viewport_state: Some(ViewportState::default()),
            rasterization_state: Some(RasterizationState::default()),
            multisample_state: Some(MultisampleState {
                rasterization_samples: samples,
                ..Default::default()
            }),
            depth_stencil_state: depth
                .then(vulkano::pipeline::graphics::depth_stencil::DepthStencilState::default),
            color_blend_state: Some(ColorBlendState::with_attachment_states(
                1,
                ColorBlendAttachmentState {
                    // Straight RGB uses SrcAlpha; coverage itself is source-over.
                    // Multiplying source alpha again makes opaque targets translucent.
                    blend: Some(AttachmentBlend {
                        src_alpha_blend_factor: BlendFactor::One,
                        ..AttachmentBlend::alpha()
                    }),
                    ..Default::default()
                },
            )),
            dynamic_state: [DynamicState::Viewport, DynamicState::Scissor]
                .into_iter()
                .collect(),
            subpass: Some(subpass.into()),
            ..GraphicsPipelineCreateInfo::layout(layout)
        },
    )
    .map_err(|e| RendererError::caused_by("pipeline", e))
}

#[derive(Default)]
pub(super) struct UiBindings {
    pub pipeline_binds: usize,
    pub scissor_updates: usize,
}

pub(super) fn record(
    builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    shape: &Arc<GraphicsPipeline>,
    texture: &Arc<GraphicsPipeline>,
    draws: Vec<Uploaded>,
    extent: [u32; 2],
) -> Result<UiBindings, RendererError> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum PipelineKind {
        Shape,
        Texture,
    }
    // ADR-0363: a fresh builder/pass always establishes its own UI state.
    let mut pipeline_bound = None;
    let mut scissor_bound = None;
    let mut bindings = UiBindings::default();
    builder
        .set_viewport(
            0,
            [Viewport {
                offset: [0.0, 0.0],
                extent: [extent[0] as f32, extent[1] as f32],
                depth_range: 0.0..=1.0,
            }]
            .into_iter()
            .collect(),
        )
        .map_err(|e| RendererError::caused_by("viewport", e))?;
    for draw in draws {
        let (count, scissor) = match draw {
            Uploaded::Shape { vertices, scissor } => {
                let count = u32::try_from(vertices.len())
                    .map_err(|e| RendererError::caused_by("draw_count", e))?;
                if pipeline_bound != Some(PipelineKind::Shape) {
                    builder
                        .bind_pipeline_graphics(shape.clone())
                        .map_err(|e| RendererError::caused_by("shape_pipeline", e))?;
                    pipeline_bound = Some(PipelineKind::Shape);
                    bindings.pipeline_binds += 1;
                }
                builder
                    .bind_vertex_buffers(0, vertices)
                    .map_err(|e| RendererError::caused_by("shape_vertices", e))?;
                (count, scissor)
            }
            Uploaded::Bitmap {
                vertices,
                set,
                scissor,
            } => {
                let count = u32::try_from(vertices.len())
                    .map_err(|e| RendererError::caused_by("draw_count", e))?;
                if pipeline_bound != Some(PipelineKind::Texture) {
                    builder
                        .bind_pipeline_graphics(texture.clone())
                        .map_err(|e| RendererError::caused_by("texture_pipeline", e))?;
                    pipeline_bound = Some(PipelineKind::Texture);
                    bindings.pipeline_binds += 1;
                }
                builder
                    .bind_descriptor_sets(
                        PipelineBindPoint::Graphics,
                        texture.layout().clone(),
                        0,
                        set,
                    )
                    .map_err(|e| RendererError::caused_by("texture_set", e))?;
                builder
                    .bind_vertex_buffers(0, vertices)
                    .map_err(|e| RendererError::caused_by("texture_vertices", e))?;
                (count, scissor)
            }
        };
        if scissor_bound != Some(scissor) {
            builder
                .set_scissor(
                    0,
                    [Scissor {
                        offset: [scissor.x, scissor.y],
                        extent: [scissor.width, scissor.height],
                    }]
                    .into_iter()
                    .collect(),
                )
                .map_err(|e| RendererError::caused_by("scissor", e))?;
            scissor_bound = Some(scissor);
            bindings.scissor_updates += 1;
        }
        // SAFETY: the only pipelines use the embedded validated shaders above: they
        // read typed vertex buffers and sampled 2D images and perform no shader writes.
        // Bound buffers contain exactly count initialized vertices; ranges start at 0
        // with one instance. Safe binding calls and draw's validation check pipeline,
        // render-pass, descriptor, vertex-input and dynamic-state compatibility.
        unsafe { builder.draw(count, 1, 0, 0) }.map_err(|e| RendererError::caused_by("draw", e))?;
    }
    Ok(bindings)
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_wgsl_validates_for_every_native_pipeline_stage() {
        #[cfg(feature = "viewport")]
        {
            use vesta_viewport::{SurfaceLighting, SurfaceMaterial, render};
            let shader = naga::front::wgsl::parse_str(render::VIEWPORT_SHADER)
                .expect("complete shared viewport source");
            let scene = vesta_viewport::ViewportScene::acceptance_scene();
            let item = &scene.renderables[0];
            let packed = render::mesh_vertices(item, None).expect("legacy vertices");
            assert_eq!(
                std::mem::size_of_val(&packed[0]),
                render::MESH_VERTEX_STRIDE as usize
            );
            assert!(
                packed.iter().all(|v| v[6..] == [1.0; 3]),
                "legacy tint is preserved"
            );
            let colors = (0..item.mesh.vertices.len())
                .map(|i| [i as f32, 0.25, 0.75])
                .collect::<Vec<_>>();
            let colored = render::mesh_vertices(item, Some(&colors)).expect("RGB vertices");
            for ((base, colored), color) in packed.iter().zip(&colored).zip(&colors) {
                assert_eq!(
                    base[..6],
                    colored[..6],
                    "color preserves position and normal"
                );
                assert_eq!(colored[6..], *color);
            }
            assert!(render::mesh_vertices(item, Some(&colors[1..])).is_err());
            let mut invalid_colors = colors.clone();
            invalid_colors[0][0] = f32::NAN;
            assert!(render::mesh_vertices(item, Some(&invalid_colors)).is_err());
            let (_, mesh_in) = shader
                .types
                .iter()
                .find(|(_, t)| t.name.as_deref() == Some("MeshIn"))
                .expect("mesh vertex input");
            let naga::TypeInner::Struct { members, .. } = &mesh_in.inner else {
                panic!("vertex struct")
            };
            for (location, member) in members.iter().enumerate() {
                assert!(
                    matches!(member.binding,Some(naga::Binding::Location {location:l,..}) if l==location as u32)
                );
                assert!(matches!(
                    shader.types[member.ty].inner,
                    naga::TypeInner::Vector {
                        size: naga::VectorSize::Tri,
                        scalar: naga::Scalar {
                            kind: naga::ScalarKind::Float,
                            width: 4
                        }
                    }
                ));
            }
            assert_eq!(members.len(), 3);
            let legacy = render::object_uniform(item, None, true).expect("default lighting");
            let rig = SurfaceLighting {
                key: [1.0, 2.0, 3.0, 0.5],
                ..SurfaceLighting::default()
            };
            for normals in [
                vesta_viewport::LightingNormals::Source,
                vesta_viewport::LightingNormals::FaceViewer,
            ] {
                let oriented = if normals == vesta_viewport::LightingNormals::FaceViewer {
                    1.0
                } else {
                    0.0
                };
                for (material, flags) in [
                    (
                        SurfaceMaterial::Lit {
                            lighting: rig,
                            normals,
                        },
                        [0.0, 0.0, 0.0, oriented],
                    ),
                    (
                        SurfaceMaterial::Terrain {
                            height_range: [-4.0, 9.0],
                            lighting: rig,
                            normals,
                        },
                        [-4.0, 9.0, 1.0, oriented],
                    ),
                ] {
                    let uniform = render::object_uniform(item, Some(material), false)
                        .expect("explicit lighting");
                    assert_eq!(
                        uniform[..2],
                        legacy[..2],
                        "lighting preserves position and base color"
                    );
                    assert_eq!(uniform[2], flags);
                    assert_eq!(uniform[3..], rig.uniform());
                    assert_eq!(
                        render::object_uniform(item, Some(material), true)
                            .expect("culled material"),
                        uniform,
                        "explicit lighting orientation is independent of culling"
                    );
                }
            }
            assert_eq!(legacy[2][3], 0.0);
            assert_eq!(
                render::object_uniform(item, None, false).expect("legacy two-sided")[2][3],
                1.0
            );
            for (name, bytes) in [
                ("ObjectUniform", std::mem::size_of_val(&legacy)),
                ("SurfaceLighting", std::mem::size_of_val(&rig.uniform())),
            ] {
                let (_, ty) = shader
                    .types
                    .iter()
                    .find(|(_, ty)| ty.name.as_deref() == Some(name))
                    .expect("shader uniform type");
                let naga::TypeInner::Struct { span, .. } = &ty.inner else {
                    panic!("uniform struct")
                };
                assert_eq!(*span as usize, bytes, "CPU and WGSL material layouts agree");
            }
            for direction in [[0.0; 4], [f32::NAN, 1.0, 0.0, 1.0]] {
                let invalid = SurfaceLighting {
                    key: direction,
                    ..rig
                };
                assert!(
                    render::object_uniform(
                        item,
                        Some(SurfaceMaterial::Lit {
                            lighting: invalid,
                            normals: vesta_viewport::LightingNormals::Source
                        }),
                        true
                    )
                    .is_err()
                );
            }
        }
        #[cfg(feature = "viewport")]
        for multisampled in [false, true] {
            for stage in [naga::ShaderStage::Vertex, naga::ShaderStage::Fragment] {
                super::viewport_spirv(super::ViewportGeometry::Labels { multisampled }, stage)
                    .expect("sampled anchor label shader validates");
            }
        }
        #[cfg(feature = "viewport")]
        for stage in [naga::ShaderStage::Vertex, naga::ShaderStage::Fragment] {
            super::viewport_spirv(super::ViewportGeometry::Markers, stage)
                .expect("screen-space marker shader validates");
        }
        for textured in [false, true] {
            for stage in [naga::ShaderStage::Vertex, naga::ShaderStage::Fragment] {
                super::spirv(textured, stage)
                    .expect("shared WGSL must remain valid for native Vulkan");
                #[cfg(feature = "viewport")]
                super::viewport_spirv(
                    if textured {
                        super::ViewportGeometry::Mesh(true)
                    } else {
                        super::ViewportGeometry::Lines
                    },
                    stage,
                )
                .expect("shared viewport shader validates");
            }
        }
    }
}
