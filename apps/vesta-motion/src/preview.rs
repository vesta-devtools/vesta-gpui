//! Preview spike: not a Graph result channel. Display-only study, ADR-0522.
use crate::model::{DURATION, Layer, Model};
use crate::study::Study;
mod gallery;
mod geometry;
use std::f32::consts::TAU;
use vesta_viewport::{
    Camera, Mesh, Renderable, RenderableId, TransformMode, Vec3, Vertex, ViewportScene,
};

const STAGE_Y: f32 = 30.0;
const STEPS: u32 = 96;

pub fn camera(model: &Model) -> Camera {
    let mut camera = Camera {
        target: Vec3::new(0.0, STAGE_Y, 0.0),
        yaw: std::f32::consts::PI,
        pitch: -0.03,
        distance: 9.3,
        vertical_fov_radians: 43.0_f32.to_radians(),
        ..Camera::default()
    };
    if model.study == Study::Wave {
        camera.yaw = -2.45;
        camera.pitch = -0.55;
        camera.distance = 10.4;
    }
    if model.study == Study::Spiral {
        camera.yaw = 2.8;
        camera.pitch = -0.30;
        camera.distance = 10.1;
    }
    camera
}

pub fn rotation(time: f32) -> f32 {
    TAU * time.rem_euclid(DURATION) / DURATION
}
pub fn twist(time: f32) -> f32 {
    (TAU * time / DURATION).sin() * 0.16
}

fn orient(point: Vec3, angle: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    let p = Vec3::new(
        point.x * c - point.y * s,
        point.x * s + point.y * c,
        point.z,
    );
    let (s, c) = 0.55_f32.sin_cos();
    Vec3::new(p.x, p.y * c - p.z * s, p.y * s + p.z * c)
}
fn ribbon(theta: f32, u: f32, radius: f32, torsion: f32, angle: f32) -> Vec3 {
    let phi = theta + torsion * u.sin() * 0.66;
    let tube = 0.76 + 0.13 * (theta * 3.0 + 0.8).sin();
    let r = radius + tube * u.cos();
    orient(
        Vec3::new(
            r * phi.cos(),
            r * phi.sin(),
            tube * u.sin() + 0.24 * (phi * 2.0).sin(),
        ),
        angle,
    )
}
fn strip(theta: f32, width: f32, radius: f32, torsion: f32, angle: f32) -> Mesh {
    let mut vertices = Vec::with_capacity((STEPS as usize + 1) * 4);
    let mut indices = Vec::with_capacity(STEPS as usize * 18);
    for step in 0..=STEPS {
        let u = TAU * step as f32 / STEPS as f32;
        for side in 0..4 {
            let v = side as f32 / 3.0;
            let theta = theta + (v - 0.5) * width;
            let p = ribbon(theta, u, radius, torsion, angle);
            let a = ribbon(theta, u + 0.001, radius, torsion, angle).minus(p);
            let b = ribbon(theta + 0.001, u, radius, torsion, angle).minus(p);
            let normal = b.cross(a).normalized();
            let p = p.plus(normal.scaled((v * std::f32::consts::PI).sin() * 0.022));
            vertices.push(Vertex {
                position: [p.x, p.y, p.z],
                normal: [normal.x, normal.y, normal.z],
            });
        }
        if step < STEPS {
            for side in 0..3 {
                let a = step * 4 + side;
                indices.extend([a, a + 1, a + 4, a + 1, a + 5, a + 4]);
            }
        }
    }
    // Ribbons are thin surfaces: both sides have their own outward normals.
    // The shared renderer correctly culls backfaces, so retain both surfaces.
    let offset = vertices.len() as u32;
    let backs = vertices
        .iter()
        .map(|vertex| Vertex {
            position: vertex.position,
            normal: [-vertex.normal[0], -vertex.normal[1], -vertex.normal[2]],
        })
        .collect::<Vec<_>>();
    let back_indices = indices
        .chunks_exact(3)
        .flat_map(|t| [t[0] + offset, t[2] + offset, t[1] + offset])
        .collect::<Vec<_>>();
    vertices.extend(backs);
    indices.extend(back_indices);
    Mesh { vertices, indices }
}

fn ring(radius: f32, tilt: f32, time: f32) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for i in 0..=160 {
        let t = TAU * i as f32 / 160.0;
        for offset in [-0.008, 0.008] {
            let r = radius + offset;
            let p = orient(
                Vec3::new(r * t.cos(), r * t.sin() * 0.34, r * t.sin() * 0.78),
                tilt + rotation(time).sin() * 0.07,
            );
            vertices.push(Vertex {
                position: [p.x, p.y, p.z],
                normal: [0.3, 0.8, -0.4],
            });
        }
        if i < 160 {
            let a = i * 2;
            indices.extend([
                a,
                a + 2,
                a + 1,
                a + 1,
                a + 2,
                a + 3,
                a,
                a + 1,
                a + 2,
                a + 1,
                a + 3,
                a + 2,
            ]);
        }
    }
    Mesh { vertices, indices }
}

pub fn scene(model: &Model) -> ViewportScene {
    if model.study != Study::Orbit {
        return gallery::scene(model);
    }
    let mut scene = ViewportScene::default();
    scene.gizmo.mode = TransformMode::Select;
    let angle = rotation(model.time) - 0.5;
    if model.visible(Layer::Ribbon) {
        for i in 0..model.params().count {
            let theta = TAU * i as f32 / model.params().count as f32;
            let blend = 0.5 + 0.5 * (theta + 0.5).sin();
            // Art-directed color bands for this display study; no PBR material
            // contract or new shader is implied by the host preview palette.
            let highlight = (theta + angle + 0.65).cos().max(0.0).powi(18) * 1.3
                + (theta + angle - 2.4).cos().max(0.0).powi(26) * 0.65;
            let color = match model.params().palette {
                1 => [
                    0.38 + blend * 0.24,
                    0.69 + blend * 0.14,
                    0.62 + blend * 0.23,
                    1.0,
                ],
                2 => [
                    0.66 + blend * 0.22,
                    0.60 + blend * 0.22,
                    0.73 + blend * 0.24,
                    1.0,
                ],
                _ => [
                    0.45 + highlight,
                    0.18 + blend * 0.17 + highlight * 0.86,
                    0.07 + blend * 0.10 + highlight * 0.58,
                    1.0,
                ],
            };
            scene.renderables.push(Renderable {
                id: RenderableId::new(100 + u64::from(i)),
                mesh: strip(
                    theta,
                    TAU / model.params().count as f32 * 0.72,
                    model.params().radius,
                    model.params().twist + twist(model.time),
                    angle,
                ),
                translation: Vec3::new(0.0, STAGE_Y, 0.0),
                color,
                selected: false,
            });
        }
    }
    if model.visible(Layer::Trails) {
        for (i, tilt) in [-0.38, 0.35, 0.92].into_iter().enumerate() {
            scene.renderables.push(Renderable {
                id: RenderableId::new(300 + i as u64),
                mesh: ring(model.params().radius + 1.15, tilt, model.time),
                translation: Vec3::new(0.0, STAGE_Y, 0.0),
                color: if i == 1 {
                    [0.85, 0.61, 0.30, 1.0]
                } else {
                    [0.20, 0.52, 0.47, 1.0]
                },
                selected: false,
            });
        }
    }
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Action, Parameter};

    fn valid(scene: &ViewportScene) {
        assert!(!scene.renderables.is_empty());
        for item in &scene.renderables {
            assert_eq!(item.mesh.indices.len() % 3, 0);
            assert!(
                item.mesh
                    .indices
                    .iter()
                    .all(|&i| (i as usize) < item.mesh.vertices.len())
            );
            assert!(
                item.mesh.vertices.iter().all(|v| v
                    .position
                    .iter()
                    .chain(&v.normal)
                    .all(|n| n.is_finite()))
            );
        }
    }

    #[test]
    fn explicit_time_produces_valid_repeatable_preview_meshes() {
        let mut model = Model::new().expect("identity");
        for study in Study::ALL {
            model.apply(Action::Study(study));
            model.seek(0.37);
            let a = scene(&model);
            assert_eq!(a, scene(&model), "explicit time must be repeatable");
            valid(&a);
            model.seek(0.73);
            assert_ne!(
                a.renderables[0].mesh,
                scene(&model).renderables[0].mesh,
                "study must actually animate"
            );
            model.seek(0.0);
            let start = scene(&model);
            model.seek(1.0);
            let end = scene(&model);
            assert_eq!(
                start.renderables.len(),
                end.renderables.len(),
                "loop identity"
            );
            for (first, last) in start.renderables.iter().zip(&end.renderables) {
                assert_eq!(first.id, last.id);
                assert_eq!(first.mesh.vertices.len(), last.mesh.vertices.len());
                assert_eq!(first.mesh.indices, last.mesh.indices);
                for (a, b) in first.mesh.vertices.iter().zip(&last.mesh.vertices) {
                    assert!(
                        a.position
                            .iter()
                            .zip(b.position)
                            .all(|(a, b)| (a - b).abs() < 0.0001),
                        "loop seam: {}",
                        study.key()
                    );
                }
            }
            for limit in [0.0, 1.0] {
                for parameter in Parameter::ALL {
                    model.set(parameter, limit);
                }
                valid(&scene(&model));
            }
            model.apply(Action::Reset);
            model.apply(Action::Visibility(Layer::Ribbon));
            let auxiliary = scene(&model);
            assert!(
                auxiliary
                    .renderables
                    .iter()
                    .all(|item| item.id.get() >= 300)
            );
            model.apply(Action::Visibility(Layer::Trails));
            assert!(
                scene(&model).renderables.is_empty(),
                "layer visibility controls actual meshes"
            );
            model.apply(Action::Reset);
        }
    }
}
