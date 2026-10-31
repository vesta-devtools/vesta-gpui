//! Shared display geometry and camera projection for viewport backends. ADR-0312.
use crate::{
    GizmoPart, Renderable, SurfaceLighting, SurfaceMaterial, TransformMode, Vec3, ViewportScene,
};

pub const SURFACE_LIGHTING_SHADER: &str = include_str!("surface_lighting.wgsl");
pub const VIEWPORT_SHADER: &str = concat!(
    include_str!("surface_lighting.wgsl"),
    "\n",
    include_str!("viewport.wgsl")
);
pub const NUMERIC_LABEL_SHADER: &str = include_str!("numeric_labels.wgsl");

pub use crate::camera::{camera_uniform, view_projection};

pub const MESH_VERTEX_STRIDE: u32 = 9 * std::mem::size_of::<f32>() as u32;

/// Position, normal and optional RGB multiplier, shared with the shader layout.
pub fn mesh_vertices(
    item: &Renderable,
    colors: Option<&[[f32; 3]]>,
) -> Result<Vec<[f32; 9]>, &'static str> {
    if item.mesh.vertices.iter().any(|vertex| {
        vertex
            .position
            .into_iter()
            .chain(vertex.normal)
            .any(|v| !v.is_finite())
    }) {
        return Err("Mesh vertices require finite positions and normals");
    }
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
    if ![
        object.translation.x,
        object.translation.y,
        object.translation.z,
    ]
    .into_iter()
    .chain(object.color)
    .chain(range)
    .all(f32::is_finite)
    {
        return Err("Object translation, color and terrain range require finite values");
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl LineVertex {
    fn is_finite(self) -> bool {
        self.position
            .into_iter()
            .chain(self.color)
            .all(f32::is_finite)
    }
}

/// Compact an existing derived buffer; invalid whole primitives never shift
/// vertex grouping and valid buffers require no allocation or copies.
pub(crate) fn finite_primitives<const N: usize>(mut vertices: Vec<LineVertex>) -> Vec<LineVertex> {
    let mut written = 0;
    for start in (0..vertices.len() / N * N).step_by(N) {
        if vertices[start..start + N].iter().all(|v| v.is_finite()) {
            if written != start {
                vertices.copy_within(start..start + N, written);
            }
            written += N;
        }
    }
    vertices.truncate(written);
    vertices
}

/// Resolve a host-supplied grid without changing the generic default recipe.
pub fn scene_grid_vertices(scene: &ViewportScene) -> Result<Vec<LineVertex>, &'static str> {
    let Some(lines) = &scene.grid_lines else {
        return Ok(grid_vertices());
    };
    if lines.len() % 2 != 0 || lines.iter().any(|v| !v.is_finite()) {
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
                        Vec3::new(crate::GIZMO_AXIS_LENGTH, 0.0, 0.0),
                        [1.0, 0.18, 0.12, 1.0],
                    ),
                    (
                        GizmoPart::AxisY,
                        Vec3::new(0.0, crate::GIZMO_AXIS_LENGTH, 0.0),
                        [0.18, 1.0, 0.32, 1.0],
                    ),
                    (
                        GizmoPart::AxisZ,
                        Vec3::new(0.0, 0.0, crate::GIZMO_AXIS_LENGTH),
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
    finite_primitives::<2>(vertices)
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

#[cfg(test)]
mod grid_tests {
    use super::*;
    use crate::Camera;

    #[test]
    fn shader_payloads_reject_nonfinite_values_without_restricting_finite_materials() {
        use crate::{LightingNormals, Mesh, RenderableId, Vertex};
        let object = Renderable {
            id: RenderableId::new(1),
            mesh: Mesh {
                vertices: vec![Vertex {
                    position: [0.0; 3],
                    normal: [0.0; 3],
                }],
                indices: Vec::new(),
            },
            translation: Vec3::default(),
            color: [-1.0, 2.0, 0.0, 1.0],
            selected: false,
        };
        assert!(mesh_vertices(&object, None).is_ok());
        assert!(object_uniform(&object, None, true).is_ok());
        let mut failures = Vec::new();
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            for normal in [false, true] {
                let mut changed = object.clone();
                if normal {
                    changed.mesh.vertices[0].normal[1] = value;
                } else {
                    changed.mesh.vertices[0].position[1] = value;
                }
                if mesh_vertices(&changed, None).is_ok() {
                    failures.push("nonfinite mesh shader data accepted");
                }
            }
            for color in [false, true] {
                let mut changed = object.clone();
                if color {
                    changed.color[1] = value;
                } else {
                    changed.translation.y = value;
                }
                if object_uniform(&changed, None, false).is_ok() {
                    failures.push("nonfinite object shader data accepted");
                }
            }
            let material = SurfaceMaterial::Terrain {
                height_range: [0.0, value],
                lighting: SurfaceLighting::default(),
                normals: LightingNormals::Source,
            };
            if object_uniform(&object, Some(material), true).is_ok() {
                failures.push("nonfinite terrain shader data accepted");
            }
        }
        for height_range in [[1.0, 1.0], [2.0, -1.0]] {
            assert!(
                object_uniform(
                    &object,
                    Some(SurfaceMaterial::Terrain {
                        height_range,
                        lighting: SurfaceLighting::default(),
                        normals: LightingNormals::FaceViewer,
                    }),
                    false
                )
                .is_ok(),
                "finite flat/inverted ranges retain their existing semantics"
            );
        }
        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn expanded_display_geometry_omits_invalid_complete_primitives_in_stable_order() {
        use crate::display::{
            DisplaySources, MarkerStyle, NormalSource, NormalStyle, display_lines,
            marker_triangles, normal_triangles,
        };
        let mut scene = ViewportScene::default();
        scene.display.wireframe = true;
        let valid_edge = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]];
        scene.sources = Some(DisplaySources {
            wire_edges: Some(vec![valid_edge, [[0.0; 3], [f32::NAN; 3]]]),
            ..DisplaySources::default()
        });
        let lines = display_lines(&scene);
        let mut failures = Vec::new();
        if lines.len() != 2
            || lines
                .iter()
                .flat_map(|v| v.position)
                .any(|v| !v.is_finite())
        {
            failures.push("invalid wire pair must be omitted without shifting valid endpoints");
        }
        assert_eq!(lines[0].position, valid_edge[0]);
        assert_eq!(lines[1].position, valid_edge[1]);
        let camera = Camera {
            target: Vec3::new(0.0, 0.0, 1.0),
            distance: 1.0,
            yaw: 0.0,
            pitch: 0.0,
            ..Camera::default()
        };
        scene.display.normals = true;
        scene.sources = Some(DisplaySources {
            normals: vec![
                NormalSource {
                    position: [0.0; 3],
                    direction: [0.0, 1.0, 0.0],
                },
                NormalSource {
                    position: [0.0, 0.0, 10000.0],
                    direction: [0.0, 1.0, 0.0],
                },
            ],
            normal_style: Some(NormalStyle {
                length: f32::MAX,
                half_width: 0.01,
                tip_length: 0.1,
                reference_distance: 1.0,
            }),
            ..DisplaySources::default()
        });
        let normals = normal_triangles(&scene, camera);
        if normals.len() != 9
            || normals
                .iter()
                .flat_map(|v| v.position)
                .any(|v| !v.is_finite())
        {
            failures.push("overflowing normal arrow must be omitted as a complete primitive");
        }
        scene.display.points = true;
        scene.sources = Some(DisplaySources {
            points: vec![[0.0, 0.0, 0.25], [0.0, 0.0, 2.0]],
            marker_style: Some(MarkerStyle {
                diameter: f32::MAX,
                edge_width: 1.0,
                color: [1.0; 4],
            }),
            ..DisplaySources::default()
        });
        let markers = marker_triangles(&scene, camera, ViewportRect::full(1, 1));
        if markers.len() != 6
            || markers
                .iter()
                .flat_map(|v| v.clip_position)
                .any(|v| !v.is_finite())
        {
            failures.push("overflowing marker must be omitted as a complete billboard");
        }
        assert!(failures.is_empty(), "{failures:?}");
    }

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
