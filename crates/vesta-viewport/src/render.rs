//! Shared display geometry and camera projection for viewport backends. ADR-0312.
use crate::{
    Camera, GizmoPart, Projection, Renderable, SurfaceLighting, SurfaceMaterial, TransformMode,
    Vec3, ViewportScene,
};

pub const SURFACE_LIGHTING_SHADER: &str = include_str!("surface_lighting.wgsl");
pub const VIEWPORT_SHADER: &str = concat!(
    include_str!("surface_lighting.wgsl"),
    "\n",
    include_str!("viewport.wgsl")
);
pub const NUMERIC_LABEL_SHADER: &str = include_str!("numeric_labels.wgsl");

pub const MESH_VERTEX_STRIDE: u32 = 9 * std::mem::size_of::<f32>() as u32;

/// Position, normal and optional RGB multiplier, shared with the shader layout.
pub fn mesh_vertices(
    item: &Renderable,
    colors: Option<&[[f32; 3]]>,
) -> Result<Vec<[f32; 9]>, &'static str> {
    if colors.is_some_and(|colors| {
        colors.len() != item.mesh.vertices.len() || colors.iter().flatten().any(|v| !v.is_finite())
    }) {
        return Err("Vertex colors require one finite RGB row per Mesh vertex");
    }
    Ok(item
        .mesh
        .vertices
        .iter()
        .enumerate()
        .map(|(index, vertex)| {
            let color = colors.map_or([1.0; 3], |colors| colors[index]);
            [
                vertex.position[0],
                vertex.position[1],
                vertex.position[2],
                vertex.normal[0],
                vertex.normal[1],
                vertex.normal[2],
                color[0],
                color[1],
                color[2],
            ]
        })
        .collect())
}

/// Exact seven-vec4 ObjectUniform layout consumed by the shared viewport shader.
pub fn object_uniform(
    object: &Renderable,
    material: Option<SurfaceMaterial>,
    cull_back_faces: bool,
) -> Result<[[f32; 4]; 7], &'static str> {
    let (range, terrain, lighting, normals) = match material {
        Some(SurfaceMaterial::Terrain {
            height_range,
            lighting,
            normals,
        }) => (height_range, 1.0, lighting, normals),
        Some(SurfaceMaterial::Lit { lighting, normals }) => ([0.0; 2], 0.0, lighting, normals),
        None => (
            [0.0; 2],
            0.0,
            SurfaceLighting::default(),
            if cull_back_faces {
                crate::LightingNormals::Source
            } else {
                crate::LightingNormals::FaceViewer
            },
        ),
    };
    if !lighting.is_valid() {
        return Err("Surface lighting requires finite terms and nonzero finite directions");
    }
    let mut uniform = [[0.0; 4]; 7];
    uniform[0] = [
        object.translation.x,
        object.translation.y,
        object.translation.z,
        0.0,
    ];
    uniform[1] = object.color;
    uniform[2] = [
        range[0],
        range[1],
        terrain,
        if normals == crate::LightingNormals::FaceViewer {
            1.0
        } else {
            0.0
        },
    ];
    uniform[3..].copy_from_slice(&lighting.uniform());
    Ok(uniform)
}

/// Shader camera block: view/projection columns, eye + perspective flag, forward.
/// Orthographic lighting uses parallel sight lines rather than a finite eye.
#[must_use]
pub fn camera_uniform(camera: Camera, aspect: f32) -> [[f32; 4]; 6] {
    let matrix = view_projection(camera, aspect);
    let eye = camera.eye();
    let forward = camera.forward();
    [
        matrix[0],
        matrix[1],
        matrix[2],
        matrix[3],
        [
            eye.x,
            eye.y,
            eye.z,
            if matches!(camera.projection, Projection::Perspective) {
                1.0
            } else {
                0.0
            },
        ],
        [forward.x, forward.y, forward.z, 0.0],
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewportRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl ViewportRect {
    #[must_use]
    pub const fn full(width: u32, height: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStats {
    pub meshes: usize,
    pub grid_lines: usize,
    pub overlay_lines: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

/// Resolve a host-supplied grid without changing the generic default recipe.
pub fn scene_grid_vertices(scene: &ViewportScene) -> Result<Vec<LineVertex>, &'static str> {
    let Some(lines) = &scene.grid_lines else {
        return Ok(grid_vertices());
    };
    if lines.len() % 2 != 0
        || lines.iter().any(|v| {
            v.position
                .iter()
                .chain(v.color.iter())
                .any(|v| !v.is_finite())
        })
    {
        return Err("Grid requires complete finite world-space line pairs");
    }
    Ok(lines.clone())
}

pub fn grid_vertices() -> Vec<LineVertex> {
    let mut vertices = Vec::new();
    for index in -10..=10 {
        let at = index as f32;
        let color = if index == 0 {
            [0.38, 0.42, 0.48, 1.0]
        } else {
            [0.19, 0.22, 0.27, 1.0]
        };
        vertices.extend([
            LineVertex {
                position: [at, 0.0, -10.0],
                color,
            },
            LineVertex {
                position: [at, 0.0, 10.0],
                color,
            },
            LineVertex {
                position: [-10.0, 0.0, at],
                color,
            },
            LineVertex {
                position: [10.0, 0.0, at],
                color,
            },
        ]);
    }
    vertices
}

pub fn overlay_vertices(scene: &ViewportScene) -> Vec<LineVertex> {
    let mut vertices = Vec::new();
    for renderable in scene.renderables.iter().filter(|item| item.selected) {
        let center = renderable.translation;
        append_box(&mut vertices, center, 0.58, [1.0, 0.72, 0.12, 1.0]);
        match scene.gizmo.mode {
            TransformMode::Select => {}
            TransformMode::Translate | TransformMode::Scale => {
                let axes = [
                    (
                        GizmoPart::AxisX,
                        Vec3::new(1.3, 0.0, 0.0),
                        [1.0, 0.18, 0.12, 1.0],
                    ),
                    (
                        GizmoPart::AxisY,
                        Vec3::new(0.0, 1.3, 0.0),
                        [0.18, 1.0, 0.32, 1.0],
                    ),
                    (
                        GizmoPart::AxisZ,
                        Vec3::new(0.0, 0.0, 1.3),
                        [0.20, 0.48, 1.0, 1.0],
                    ),
                ];
                for (part, axis, normal_color) in axes {
                    let color = if scene.gizmo.active == Some(part) {
                        [1.0, 0.92, 0.55, 1.0]
                    } else {
                        normal_color
                    };
                    vertices.extend([
                        LineVertex {
                            position: [center.x, center.y, center.z],
                            color,
                        },
                        LineVertex {
                            position: [center.x + axis.x, center.y + axis.y, center.z + axis.z],
                            color,
                        },
                    ]);
                }
            }
            TransformMode::Rotate => {
                append_circle(&mut vertices, center, 1.0, 0, [1.0, 0.18, 0.12, 1.0]);
                append_circle(&mut vertices, center, 1.0, 1, [0.18, 1.0, 0.32, 1.0]);
                append_circle(&mut vertices, center, 1.0, 2, [0.20, 0.48, 1.0, 1.0]);
            }
        }
    }
    vertices
}

fn append_circle(
    output: &mut Vec<LineVertex>,
    center: Vec3,
    radius: f32,
    normal_axis: u8,
    color: [f32; 4],
) {
    const SEGMENTS: usize = 48;
    for index in 0..SEGMENTS {
        let first = index as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let second = (index + 1) as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        for angle in [first, second] {
            let (sine, cosine) = angle.sin_cos();
            let offset = match normal_axis {
                0 => Vec3::new(0.0, cosine * radius, sine * radius),
                1 => Vec3::new(cosine * radius, 0.0, sine * radius),
                _ => Vec3::new(cosine * radius, sine * radius, 0.0),
            };
            output.push(LineVertex {
                position: [
                    center.x + offset.x,
                    center.y + offset.y,
                    center.z + offset.z,
                ],
                color,
            });
        }
    }
}

fn append_box(output: &mut Vec<LineVertex>, center: Vec3, half: f32, color: [f32; 4]) {
    let corners = [
        [-half, -half, -half],
        [half, -half, -half],
        [half, half, -half],
        [-half, half, -half],
        [-half, -half, half],
        [half, -half, half],
        [half, half, half],
        [-half, half, half],
    ];
    for (a, b) in [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ] {
        for corner in [corners[a], corners[b]] {
            output.push(LineVertex {
                position: [
                    center.x + corner[0],
                    center.y + corner[1],
                    center.z + corner[2],
                ],
                color,
            });
        }
    }
}

pub fn view_projection(camera: Camera, aspect: f32) -> [[f32; 4]; 4] {
    let projection = match camera.projection {
        Projection::Perspective => {
            perspective(camera.vertical_fov_radians, aspect, camera.near, camera.far)
        }
        Projection::Orthographic { vertical_size } => {
            orthographic(vertical_size, aspect, camera.near, camera.far)
        }
    };
    multiply(projection, look_at(camera))
}

fn look_at(camera: Camera) -> [[f32; 4]; 4] {
    let eye = camera.eye();
    let forward = camera.forward();
    let (right, up) = camera.screen_axes();
    [
        [right.x, up.x, -forward.x, 0.0],
        [right.y, up.y, -forward.y, 0.0],
        [right.z, up.z, -forward.z, 0.0],
        [-right.dot(eye), -up.dot(eye), forward.dot(eye), 1.0],
    ]
}

fn perspective(fov: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let factor = 1.0 / (fov * 0.5).tan();
    [
        [factor / aspect.max(0.001), 0.0, 0.0, 0.0],
        [0.0, factor, 0.0, 0.0],
        [0.0, 0.0, far / (near - far), -1.0],
        [0.0, 0.0, near * far / (near - far), 0.0],
    ]
}

fn orthographic(vertical_size: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let height = vertical_size.max(0.01);
    let width = (height * aspect.max(0.001)).max(0.001);
    let depth = (far - near).max(0.001);
    [
        [2.0 / width, 0.0, 0.0, 0.0],
        [0.0, 2.0 / height, 0.0, 0.0],
        [0.0, 0.0, -1.0 / depth, 0.0],
        [0.0, 0.0, -near / depth, 1.0],
    ]
}

fn multiply(left: [[f32; 4]; 4], right: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut output = [[0.0; 4]; 4];
    for column in 0..4 {
        for row in 0..4 {
            output[column][row] = (0..4)
                .map(|index| left[index][row] * right[column][index])
                .sum();
        }
    }
    output
}

#[cfg(test)]
mod grid_tests {
    use super::*;

    #[test]
    fn scene_grid_override_preserves_fallback_and_rejects_invalid_line_pairs() {
        let mut scene = ViewportScene::default();
        assert_eq!(scene_grid_vertices(&scene), Ok(grid_vertices()));
        scene.grid_lines = Some(Vec::new());
        assert_eq!(scene_grid_vertices(&scene), Ok(Vec::new()));
        let first = LineVertex {
            position: [2.0, 3.0, 4.0],
            color: [0.1, 0.2, 0.3, 1.0],
        };
        let second = LineVertex {
            position: [5.0, 6.0, 7.0],
            ..first
        };
        let valid = vec![first, second];
        scene.grid_lines = Some(valid.clone());
        assert_eq!(scene_grid_vertices(&scene), Ok(valid));
        scene.grid_lines = Some(vec![first]);
        assert!(scene_grid_vertices(&scene).is_err());
        for (position, color) in [(true, false), (false, true)] {
            let mut invalid = second;
            if position {
                invalid.position[0] = f32::NAN;
            }
            if color {
                invalid.color[3] = f32::INFINITY;
            }
            scene.grid_lines = Some(vec![first, invalid]);
            assert!(scene_grid_vertices(&scene).is_err());
        }
    }
}
