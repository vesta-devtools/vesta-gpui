//! Safe Vulkan projection of the shared immutable viewport snapshot. ADR-0312.
use crate::{RendererError, checked_pipeline};
mod labels;
use std::sync::Arc;
use vesta_viewport::{
    Camera, Renderable, Vec3, ViewportScene,
    render::{
        ViewportRect, camera_uniform, object_uniform, overlay_vertices, scene_grid_vertices,
        view_projection,
    },
};

fn mesh_depth(camera: Camera, item: &Renderable) -> f32 {
    let mut minimum = [f32::INFINITY; 3];
    let mut maximum = [f32::NEG_INFINITY; 3];
    for vertex in &item.mesh.vertices {
        for axis in 0..3 {
            minimum[axis] = minimum[axis].min(vertex.position[axis]);
            maximum[axis] = maximum[axis].max(vertex.position[axis]);
        }
    }
    let center = if item.mesh.vertices.is_empty() {
        Vec3::default()
    } else {
        Vec3::new(
            minimum[0] * 0.5 + maximum[0] * 0.5,
            minimum[1] * 0.5 + maximum[1] * 0.5,
            minimum[2] * 0.5 + maximum[2] * 0.5,
        )
    };
    center
        .plus(item.translation)
        .minus(camera.eye())
        .dot(camera.forward())
}

// Called only after the draw boundary validates complete triangles and indices.
fn transparent_indices(camera: Camera, item: &Renderable) -> Vec<u32> {
    let forward = camera.forward();
    let mut triangles = item
        .mesh
        .indices
        .chunks_exact(3)
        .map(|triangle| {
            let depth = triangle
                .iter()
                .map(|&index| {
                    let position = item.mesh.vertices[index as usize].position;
                    Vec3::new(position[0], position[1], position[2]).dot(forward) / 3.0
                })
                .sum::<f32>();
            (depth, triangle)
        })
        .collect::<Vec<_>>();
    // Translation and eye depth are shared by all triangles in this mesh, so
    // omitting them preserves the order and avoids subtractive cancellation.
    triangles.sort_by(|first, second| second.0.total_cmp(&first.0));
    triangles
        .into_iter()
        .flat_map(|(_, triangle)| triangle.iter().copied())
        .collect()
}

use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer},
    descriptor_set::{
        DescriptorSet, WriteDescriptorSet, allocator::StandardDescriptorSetAllocator,
    },
    device::Device,
    image::SampleCount,
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::{
        GraphicsPipeline, Pipeline, PipelineBindPoint,
        graphics::viewport::{Scissor, Viewport},
    },
    render_pass::RenderPass,
};

pub(super) struct ScenePainter {
    labels: labels::LabelPainter,
    memory: Arc<StandardMemoryAllocator>,
    descriptors: Arc<StandardDescriptorSetAllocator>,
    mesh: Arc<GraphicsPipeline>,
    translucent_mesh: Arc<GraphicsPipeline>,
    two_sided_mesh: Arc<GraphicsPipeline>,
    two_sided_translucent_mesh: Arc<GraphicsPipeline>,
    normal_ribbons: Arc<GraphicsPipeline>,
    markers: Arc<GraphicsPipeline>,
    grid: Arc<GraphicsPipeline>,
    overlay: Arc<GraphicsPipeline>,
}
impl ScenePainter {
    pub fn new(
        device: Arc<Device>,
        pass: Arc<RenderPass>,
        overlay_pass: Arc<RenderPass>,
        samples: SampleCount,
    ) -> Result<Self, RendererError> {
        let memory = Arc::new(StandardMemoryAllocator::new_default(device.clone()));
        let descriptors = Arc::new(StandardDescriptorSetAllocator::new(
            device.clone(),
            Default::default(),
        ));
        Ok(Self {
            labels: labels::LabelPainter::new(
                device.clone(),
                overlay_pass,
                samples,
                memory.clone(),
                descriptors.clone(),
            )?,
            markers: checked_pipeline::create_viewport_markers(
                device.clone(),
                pass.clone(),
                samples,
            )?,
            two_sided_mesh: checked_pipeline::create_viewport_two_sided(
                device.clone(),
                pass.clone(),
                checked_pipeline::ViewportDepth::ReadWrite,
                samples,
            )?,
            two_sided_translucent_mesh: checked_pipeline::create_viewport_two_sided(
                device.clone(),
                pass.clone(),
                checked_pipeline::ViewportDepth::ReadOnly,
                samples,
            )?,
            normal_ribbons: checked_pipeline::create_viewport_unlit_triangles(
                device.clone(),
                pass.clone(),
                samples,
            )?,
            memory,
            descriptors,
            mesh: checked_pipeline::create_viewport(
                device.clone(),
                pass.clone(),
                true,
                checked_pipeline::ViewportDepth::ReadWrite,
                samples,
            )?,
            translucent_mesh: checked_pipeline::create_viewport(
                device.clone(),
                pass.clone(),
                true,
                checked_pipeline::ViewportDepth::ReadOnly,
                samples,
            )?,
            grid: checked_pipeline::create_viewport(
                device.clone(),
                pass.clone(),
                false,
                checked_pipeline::ViewportDepth::ReadOnly,
                samples,
            )?,
            overlay: checked_pipeline::create_viewport(
                device,
                pass,
                false,
                checked_pipeline::ViewportDepth::Disabled,
                samples,
            )?,
        })
    }
    pub(super) fn prepare_labels(
        &mut self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        camera: Camera,
        scene: &ViewportScene,
        rect: ViewportRect,
        clip: ViewportRect,
        depth: Arc<vulkano::image::view::ImageView>,
    ) -> Result<Option<labels::PreparedLabels>, RendererError> {
        self.labels
            .prepare(builder, camera, scene, rect, clip, depth)
    }
    pub(super) fn record_labels(
        &self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        labels: labels::PreparedLabels,
    ) -> Result<(), RendererError> {
        self.labels.record(builder, labels)
    }
    pub(super) fn invalidate_labels(&mut self) {
        self.labels.invalidate();
    }
    pub fn record(
        &self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        camera: Camera,
        scene: &ViewportScene,
        rect: ViewportRect,
        clip: ViewportRect,
    ) -> Result<(), RendererError> {
        if rect.width == 0 || rect.height == 0 {
            return Ok(());
        }
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
            camera_uniform(camera, rect.width as f32 / rect.height as f32),
        )
        .map_err(|e| RendererError::caused_by("viewport_camera", e))?;
        let camera_set = DescriptorSet::new(
            self.descriptors.clone(),
            self.mesh.layout().set_layouts()[0].clone(),
            [WriteDescriptorSet::buffer(0, camera_buffer)],
            [],
        )
        .map_err(|e| RendererError::caused_by("viewport_camera_set", e))?;
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
            .map_err(|e| RendererError::caused_by("scene_viewport", e))?;
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
            .map_err(|e| RendererError::caused_by("scene_scissor", e))?;
        if scene.display.grid {
            let grid = scene_grid_vertices(scene)
                .map_err(|message| RendererError::at("viewport_grid", message))?;
            self.lines(builder, &self.grid, grid, camera_set.clone())?;
        }
        // Establish opaque depth before blending translucent surfaces. Transparent
        // surfaces test that depth without hiding later transparent surfaces.
        let opaque = scene
            .renderables
            .iter()
            .filter(|item| item.color[3] >= 1.0 || item.color[3].is_nan());
        let mut translucent = scene
            .renderables
            .iter()
            .filter(|item| item.color[3] < 1.0)
            .map(|item| (mesh_depth(camera, item), item))
            .collect::<Vec<_>>();
        translucent.sort_by(|(first_depth, first), (second_depth, second)| {
            second_depth
                .total_cmp(first_depth)
                .then_with(|| first.id.get().cmp(&second.id.get()))
        });
        for item in opaque.chain(translucent.into_iter().map(|(_, item)| item)) {
            let pipeline = match (scene.display.cull_back_faces, item.color[3] < 1.0) {
                (true, false) => &self.mesh,
                (true, true) => &self.translucent_mesh,
                (false, false) => &self.two_sided_mesh,
                (false, true) => &self.two_sided_translucent_mesh,
            };
            if item.mesh.indices.is_empty() {
                continue;
            }
            if item.mesh.indices.len() % 3 != 0 {
                return Err(RendererError::at(
                    "viewport_indices",
                    "triangle index count is not divisible by three",
                ));
            }
            let count = u32::try_from(item.mesh.indices.len())
                .map_err(|e| RendererError::caused_by("viewport_count", e))?;
            if item
                .mesh
                .indices
                .iter()
                .any(|&index| index as usize >= item.mesh.vertices.len())
            {
                return Err(RendererError::at(
                    "viewport_indices",
                    "index exceeds vertex buffer",
                ));
            }
            if !scene.display.surfaces {
                continue;
            }
            let vertices = vesta_viewport::render::mesh_vertices(
                item,
                scene.vertex_colors.get(&item.id).map(Vec::as_slice),
            )
            .map_err(|e| RendererError::at("viewport_vertex_colors", e))?;
            let indices = Buffer::from_iter(
                self.memory.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::INDEX_BUFFER,
                    ..Default::default()
                },
                allocation.clone(),
                if item.color[3] < 1.0 {
                    transparent_indices(camera, item)
                } else {
                    item.mesh.indices.clone()
                },
            )
            .map_err(|e| RendererError::caused_by("viewport_index_buffer", e))?;
            let buffer = Buffer::from_iter(
                self.memory.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::VERTEX_BUFFER,
                    ..Default::default()
                },
                allocation.clone(),
                vertices,
            )
            .map_err(|e| RendererError::caused_by("viewport_mesh", e))?;
            let uniform = Buffer::from_data(
                self.memory.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::UNIFORM_BUFFER,
                    ..Default::default()
                },
                allocation.clone(),
                object_uniform(
                    item,
                    scene.materials.get(&item.id).copied(),
                    scene.display.cull_back_faces,
                )
                .map_err(|e| RendererError::at("viewport_material", e))?,
            )
            .map_err(|e| RendererError::caused_by("viewport_object", e))?;
            let object_set = DescriptorSet::new(
                self.descriptors.clone(),
                pipeline.layout().set_layouts()[1].clone(),
                [WriteDescriptorSet::buffer(0, uniform)],
                [],
            )
            .map_err(|e| RendererError::caused_by("viewport_object_set", e))?;
            builder
                .bind_pipeline_graphics(pipeline.clone())
                .map_err(|e| RendererError::caused_by("viewport_mesh_pipeline", e))?;
            builder
                .bind_descriptor_sets(
                    PipelineBindPoint::Graphics,
                    pipeline.layout().clone(),
                    0,
                    (camera_set.clone(), object_set),
                )
                .map_err(|e| RendererError::caused_by("viewport_mesh_sets", e))?;
            builder
                .bind_vertex_buffers(0, buffer)
                .map_err(|e| RendererError::caused_by("viewport_mesh_bind", e))?;
            builder
                .bind_index_buffer(indices)
                .map_err(|e| RendererError::caused_by("viewport_index_bind", e))?;
            checked_pipeline::draw_viewport_indexed(builder, count)?;
        }
        self.lines(
            builder,
            &self.grid,
            vesta_viewport::display::display_lines(scene),
            camera_set.clone(),
        )?;
        self.lines(
            builder,
            &self.normal_ribbons,
            vesta_viewport::display::normal_triangles(scene, camera),
            camera_set.clone(),
        )?;
        self.markers(
            builder,
            vesta_viewport::display::marker_triangles(scene, camera, rect),
        )?;
        self.lines(builder, &self.overlay, overlay_vertices(scene), camera_set)?;
        Ok(())
    }
    fn markers(
        &self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        vertices: Vec<vesta_viewport::display::MarkerVertex>,
    ) -> Result<(), RendererError> {
        if vertices.is_empty() {
            return Ok(());
        }
        let count = u32::try_from(vertices.len())
            .map_err(|e| RendererError::caused_by("marker_count", e))?;
        let buffer = Buffer::from_iter(
            self.memory.clone(),
            BufferCreateInfo {
                usage: BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            vertices.into_iter().map(|v| {
                [
                    v.clip_position[0],
                    v.clip_position[1],
                    v.clip_position[2],
                    v.clip_position[3],
                    v.uv[0],
                    v.uv[1],
                    v.color[0],
                    v.color[1],
                    v.color[2],
                    v.color[3],
                    v.softness,
                ]
            }),
        )
        .map_err(|e| RendererError::caused_by("marker_buffer", e))?;
        builder
            .bind_pipeline_graphics(self.markers.clone())
            .map_err(|e| RendererError::caused_by("marker_pipeline", e))?;
        builder
            .bind_vertex_buffers(0, buffer)
            .map_err(|e| RendererError::caused_by("marker_bind", e))?;
        checked_pipeline::draw_viewport(builder, count)
    }
    fn lines(
        &self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        pipeline: &Arc<GraphicsPipeline>,
        lines: Vec<vesta_viewport::render::LineVertex>,
        camera: Arc<DescriptorSet>,
    ) -> Result<(), RendererError> {
        if lines.is_empty() {
            return Ok(());
        }
        let count = u32::try_from(lines.len())
            .map_err(|e| RendererError::caused_by("viewport_line_count", e))?;
        let buffer = Buffer::from_iter(
            self.memory.clone(),
            BufferCreateInfo {
                usage: BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            lines.into_iter().map(|v| {
                [
                    v.position[0],
                    v.position[1],
                    v.position[2],
                    v.color[0],
                    v.color[1],
                    v.color[2],
                    v.color[3],
                ]
            }),
        )
        .map_err(|e| RendererError::caused_by("viewport_lines", e))?;
        builder
            .bind_pipeline_graphics(pipeline.clone())
            .map_err(|e| RendererError::caused_by("viewport_line_pipeline", e))?;
        builder
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                pipeline.layout().clone(),
                0,
                camera,
            )
            .map_err(|e| RendererError::caused_by("viewport_line_camera", e))?;
        builder
            .bind_vertex_buffers(0, buffer)
            .map_err(|e| RendererError::caused_by("viewport_line_bind", e))?;
        checked_pipeline::draw_viewport(builder, count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_depth_tracks_camera_and_local_geometry() {
        let scene = crate::viewport_fixture::scene();
        let mut near = scene.renderables[0].clone();
        let mut far = near.clone();
        near.translation = Vec3::default();
        far.translation = Vec3::new(0.0, 0.0, 4.0);
        let mut camera = Camera {
            yaw: 0.0,
            pitch: 0.0,
            ..Camera::default()
        };
        assert!(mesh_depth(camera, &far) > mesh_depth(camera, &near));
        camera.yaw = std::f32::consts::PI;
        assert!(mesh_depth(camera, &far) < mesh_depth(camera, &near));
        camera.projection = vesta_viewport::Projection::Orthographic { vertical_size: 8.0 };
        assert!(mesh_depth(camera, &far) < mesh_depth(camera, &near));
        far.translation = near.translation;
        for vertex in &mut far.mesh.vertices {
            vertex.position[2] += 4.0;
        }
        assert!(mesh_depth(camera, &far) < mesh_depth(camera, &near));

        // A transparent mesh can contain separated layers in arbitrary index
        // order. Reordering must reverse with the camera, preserve winding and
        // retain the original order of coplanar triangles.
        let mut layered = near.clone();
        layered.mesh.vertices = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 4.0],
            [1.0, 0.0, 4.0],
            [0.0, 1.0, 4.0],
        ]
        .into_iter()
        .map(|position| vesta_viewport::Vertex {
            position,
            normal: [0.0, 0.0, 1.0],
        })
        .collect();
        layered.mesh.indices = vec![0, 1, 2, 3, 4, 5, 2, 1, 0];
        camera.yaw = 0.0;
        let unchanged = layered.mesh.clone();
        assert_eq!(
            transparent_indices(camera, &layered),
            [3, 4, 5, 0, 1, 2, 2, 1, 0]
        );
        camera.yaw = std::f32::consts::PI;
        assert_eq!(
            transparent_indices(camera, &layered),
            [0, 1, 2, 2, 1, 0, 3, 4, 5]
        );
        assert_eq!(layered.mesh, unchanged);
    }
}
