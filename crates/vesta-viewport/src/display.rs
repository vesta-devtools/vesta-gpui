//! Backend-neutral display switches; never part of an author document or cache key.
use crate::{Vec3, ViewportScene, render::LineVertex};
mod markers;
pub use markers::{MarkerStyle, MarkerVertex, marker_triangles};
mod labels;
pub use labels::{NumericAtlas, NumericLabel, NumericLabelStyle, NumericLabels};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplaySources {
    /// Already transformed into display world space by the host.
    pub points: Vec<[f32; 3]>,
    /// Original world-space wire segments. Some(empty) deliberately draws no edges.
    pub wire_edges: Option<Vec<[[f32; 3]; 2]>>,
    pub normals: Vec<NormalSource>,
    pub primitive_normals: Vec<NormalSource>,
    /// Optional host-supplied ribbon dimensions, in display world units.
    pub normal_style: Option<NormalStyle>,
    pub marker_style: Option<MarkerStyle>,
    pub numeric_labels: Option<NumericLabels>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalStyle {
    pub length: f32,
    pub half_width: f32,
    pub tip_length: f32,
    pub reference_distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalSource {
    pub position: [f32; 3],
    pub direction: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayOptions {
    pub cull_back_faces: bool,
    pub grid: bool,
    pub surfaces: bool,
    pub wireframe: bool,
    pub points: bool,
    pub normals: bool,
    pub primitive_normals: bool,
    pub point_numbers: bool,
    pub primitive_numbers: bool,
}
impl Default for DisplayOptions {
    fn default() -> Self {
        Self {
            cull_back_faces: true,
            grid: true,
            surfaces: true,
            wireframe: false,
            points: false,
            normals: false,
            primitive_normals: false,
            point_numbers: false,
            primitive_numbers: false,
        }
    }
}
pub fn display_lines(scene: &ViewportScene) -> Vec<LineVertex> {
    let mut lines = Vec::new();
    for item in &scene.renderables {
        let point = |p: [f32; 3]| {
            [
                p[0] + item.translation.x,
                p[1] + item.translation.y,
                p[2] + item.translation.z,
            ]
        };
        if scene.display.wireframe
            && scene
                .sources
                .as_ref()
                .is_none_or(|s| s.wire_edges.is_none())
        {
            for t in item.mesh.indices.chunks_exact(3) {
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    if let (Some(a), Some(b)) = (
                        item.mesh.vertices.get(a as usize),
                        item.mesh.vertices.get(b as usize),
                    ) {
                        for p in [a.position, b.position] {
                            lines.push(LineVertex {
                                position: point(p),
                                color: [0.08, 0.12, 0.15, 1.0],
                            });
                        }
                    }
                }
            }
        }
        if scene.sources.is_none() && (scene.display.points || scene.display.normals) {
            for v in &item.mesh.vertices {
                let p = Vec3::new(v.position[0], v.position[1], v.position[2]);
                let mut segment = |a: Vec3, b: Vec3, color| {
                    for p in [a, b] {
                        lines.push(LineVertex {
                            position: point([p.x, p.y, p.z]),
                            color,
                        });
                    }
                };
                if scene.display.points {
                    for axis in [
                        Vec3::new(0.015, 0.0, 0.0),
                        Vec3::new(0.0, 0.015, 0.0),
                        Vec3::new(0.0, 0.0, 0.015),
                    ] {
                        segment(p.minus(axis), p.plus(axis), [1.0, 0.65, 0.12, 1.0]);
                    }
                }
                if scene.display.normals {
                    segment(
                        p,
                        p.plus(Vec3::new(v.normal[0], v.normal[1], v.normal[2]).scaled(0.12)),
                        [0.2, 0.75, 1.0, 1.0],
                    );
                }
            }
        }
    }
    if let Some(sources) = &scene.sources {
        if scene.display.wireframe
            && let Some(edges) = &sources.wire_edges
        {
            for edge in edges {
                for position in edge {
                    lines.push(LineVertex {
                        position: *position,
                        color: [0.08, 0.12, 0.15, 1.0],
                    });
                }
            }
        }
        if scene.display.points && sources.marker_style.is_none() {
            for p in &sources.points {
                for axis in [[0.015, 0.0, 0.0], [0.0, 0.015, 0.0], [0.0, 0.0, 0.015]] {
                    for sign in [-1.0, 1.0] {
                        lines.push(LineVertex {
                            position: [
                                p[0] + axis[0] * sign,
                                p[1] + axis[1] * sign,
                                p[2] + axis[2] * sign,
                            ],
                            color: [1.0, 0.65, 0.12, 1.0],
                        });
                    }
                }
            }
        }
        for (enabled, normals, color) in [
            (
                scene.display.normals,
                &sources.normals,
                [0.30, 0.86, 0.42, 1.0],
            ),
            (
                scene.display.primitive_normals,
                &sources.primitive_normals,
                [0.95, 0.72, 0.18, 1.0],
            ),
        ] {
            if !enabled || sources.normal_style.is_some() {
                continue;
            }
            for normal in normals {
                let p = normal.position;
                let n = normal.direction;
                for position in [
                    p,
                    [p[0] + n[0] * 0.12, p[1] + n[1] * 0.12, p[2] + n[2] * 0.12],
                ] {
                    lines.push(LineVertex { position, color });
                }
            }
        }
    }
    crate::render::finite_primitives::<2>(lines)
}

/// Camera-facing, unlit triangles. The renderer must depth-test without writing depth.
pub fn normal_triangles(scene: &ViewportScene, camera: crate::Camera) -> Vec<LineVertex> {
    let Some(sources) = &scene.sources else {
        return Vec::new();
    };
    let Some(style) = sources.normal_style else {
        return Vec::new();
    };
    if ![
        style.length,
        style.half_width,
        style.tip_length,
        style.reference_distance,
    ]
    .into_iter()
    .all(|value| value.is_finite() && value > 0.0)
    {
        return Vec::new();
    }
    let eye = camera.eye();
    let mut vertices = Vec::new();
    for (enabled, normals, color) in [
        (
            scene.display.normals,
            &sources.normals,
            [0.30, 0.86, 0.42, 1.0],
        ),
        (
            scene.display.primitive_normals,
            &sources.primitive_normals,
            [0.95, 0.72, 0.18, 1.0],
        ),
    ] {
        if !enabled {
            continue;
        }
        for source in normals {
            if !source
                .position
                .into_iter()
                .chain(source.direction)
                .all(f32::is_finite)
            {
                continue;
            }
            let base = Vec3::new(source.position[0], source.position[1], source.position[2]);
            let dir = Vec3::new(
                source.direction[0],
                source.direction[1],
                source.direction[2],
            )
            .normalized();
            if dir.dot(dir) < 1e-6 {
                continue;
            }
            let offset = base.minus(eye);
            let raw_width = dir.cross(offset.normalized());
            let width = if raw_width.dot(raw_width) > 1e-12 {
                raw_width.normalized()
            } else {
                Vec3::new(1.0, 0.0, 0.0)
            };
            let scale = (offset.dot(offset).sqrt() / style.reference_distance).clamp(0.2, 5.0);
            let length = style.length * scale;
            let half_width = width.scaled(style.half_width * scale);
            let tip_begin = base.plus(dir.scaled((length - style.tip_length * scale).max(0.0)));
            let a = base.minus(half_width);
            let b = base.plus(half_width);
            let c = tip_begin.plus(half_width);
            let d = tip_begin.minus(half_width);
            let tip = base.plus(dir.scaled(length));
            for p in [a, b, c, a, c, d, d, tip, c] {
                vertices.push(LineVertex {
                    position: [p.x, p.y, p.z],
                    color,
                });
            }
        }
    }
    crate::render::finite_primitives::<9>(vertices)
}
