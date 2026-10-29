//! Preview spike: not a Graph result channel. Six-study display slice, ADR-0524.
use super::{geometry::*, *};

fn palette(model: &Model, t: f32) -> [f32; 4] {
    match model.params().palette {
        1 => [0.12 + t * 0.44, 0.36 + t * 0.55, 0.33 + t * 0.48, 1.0],
        2 => [0.34 + t * 0.48, 0.25 + t * 0.49, 0.55 + t * 0.45, 1.0],
        _ => [0.48 + t * 0.70, 0.20 + t * 0.56, 0.08 + t * 0.34, 1.0],
    }
}
fn item(scene: &mut ViewportScene, id: u64, mesh: Mesh, color: [f32; 4]) {
    if !mesh.indices.is_empty() {
        scene.renderables.push(Renderable {
            id: RenderableId::new(id),
            mesh,
            translation: Vec3::new(0.0, STAGE_Y, 0.0),
            color,
            selected: false,
        });
    }
}
fn batches(scene: &mut ViewportScene, model: &Model, meshes: Vec<Mesh>) {
    let length = meshes.len();
    for (i, mesh) in meshes.into_iter().enumerate() {
        item(
            scene,
            100 + i as u64,
            mesh,
            palette(model, i as f32 / (length - 1).max(1) as f32),
        );
    }
}
pub fn scene(model: &Model) -> ViewportScene {
    let mut scene = ViewportScene::default();
    scene.gizmo.mode = TransformMode::Select;
    if model.visible(Layer::Ribbon) {
        match model.study {
            Study::Wave => wave(&mut scene, model),
            Study::Sphere => sphere(&mut scene, model),
            Study::Helix => helix(&mut scene, model),
            Study::Bloom => bloom(&mut scene, model),
            Study::Spiral => spiral(&mut scene, model),
            Study::Orbit => return super::scene(model),
        }
    }
    if model.visible(Layer::Trails) {
        match model.study {
            Study::Wave => wave_grid(&mut scene, model),
            Study::Helix => helix_links(&mut scene, model),
            Study::Spiral => {
                let points = (0..=160)
                    .map(|i| spiral_point(model, i as f32 / 160.0))
                    .collect::<Vec<_>>();
                item(
                    &mut scene,
                    300,
                    tube(&points, 0.012, 5, false),
                    [0.25, 0.65, 0.59, 1.0],
                );
            }
            _ => {
                for (i, tilt) in [-0.42, 0.64].into_iter().enumerate() {
                    item(
                        &mut scene,
                        300 + i as u64,
                        ring(model.params().radius + 1.03, tilt, model.time),
                        [0.24, 0.62, 0.55, 1.0],
                    );
                }
            }
        }
    }
    scene
}

fn wave(scene: &mut ViewportScene, model: &Model) {
    let p = model.params();
    let columns = (p.count as f32).sqrt().ceil() as u32;
    let rows = p.count.div_ceil(columns);
    let spacing = p.radius * 0.29;
    let phase = rotation(model.time);
    let mut meshes = (0..12).map(|_| empty()).collect::<Vec<_>>();
    for i in 0..p.count {
        let x = ((i % columns) as f32 - (columns - 1) as f32 * 0.5) * spacing;
        let z = ((i / columns) as f32 - (rows - 1) as f32 * 0.5) * spacing;
        let wave = 0.5 + 0.5 * (x * 1.35 + phase).sin() * (z * 0.85 - phase).cos();
        let height = 0.22 + (p.twist * 0.55 + 0.35) * wave * 1.65;
        let group = ((wave * 11.0).floor() as usize).min(11);
        append(
            &mut meshes[group],
            cube(
                Vec3::new(x, height * 0.5 - 0.55, z),
                Vec3::new(spacing * 0.38, height * 0.5, spacing * 0.38),
                0.0,
            ),
        );
    }
    batches(scene, model, meshes);
}
fn wave_grid(scene: &mut ViewportScene, model: &Model) {
    let mut mesh = empty();
    let size = model.params().radius * 1.5;
    for i in 0..=10 {
        let at = -size + i as f32 * size / 5.0;
        append(
            &mut mesh,
            cube(
                Vec3::new(at, -0.58, 0.0),
                Vec3::new(0.008, 0.008, size),
                0.0,
            ),
        );
        append(
            &mut mesh,
            cube(
                Vec3::new(0.0, -0.58, at),
                Vec3::new(size, 0.008, 0.008),
                0.0,
            ),
        );
    }
    item(scene, 300, mesh, [0.12, 0.26, 0.25, 1.0]);
}
fn sphere(scene: &mut ViewportScene, model: &Model) {
    let p = model.params();
    let meridians = p.count / 2;
    let latitudes = p.count - meridians;
    let phase = rotation(model.time);
    let mut meshes = (0..6).map(|_| empty()).collect::<Vec<_>>();
    for i in 0..p.count {
        let points = (0..=96)
            .map(|sample| {
                let t = TAU * sample as f32 / 96.0;
                let point = if i < meridians {
                    let angle = std::f32::consts::PI * i as f32 / meridians as f32;
                    Vec3::new(
                        p.radius * t.sin() * angle.cos(),
                        p.radius * t.cos(),
                        p.radius * t.sin() * angle.sin(),
                    )
                } else {
                    let latitude = ((i - meridians + 1) as f32 / (latitudes + 1) as f32 - 0.5)
                        * std::f32::consts::PI;
                    Vec3::new(
                        p.radius * latitude.cos() * t.cos(),
                        p.radius * latitude.sin(),
                        p.radius * latitude.cos() * t.sin(),
                    )
                };
                rotate_z(
                    rotate_x(rotate_y(point, phase), 0.4 + p.twist * 0.2 * phase.sin()),
                    0.12 * phase.sin(),
                )
            })
            .collect::<Vec<_>>();
        append(&mut meshes[i as usize % 6], tube(&points, 0.018, 6, true));
    }
    batches(scene, model, meshes);
}
fn helix_point(model: &Model, u: f32, strand: usize) -> Vec3 {
    let p = model.params();
    let angle = TAU * (1.0 + p.twist * 2.0) * u
        + rotation(model.time)
        + strand as f32 * std::f32::consts::PI;
    Vec3::new(
        p.radius * 0.45 * angle.cos(),
        (u - 0.5) * 4.8,
        p.radius * 0.45 * angle.sin(),
    )
}
fn helix(scene: &mut ViewportScene, model: &Model) {
    for strand in 0..2 {
        let points = (0..=160)
            .map(|i| helix_point(model, i as f32 / 160.0, strand))
            .collect::<Vec<_>>();
        let mut mesh = tube(&points, 0.075, 10, false);
        for i in 0..model.params().count {
            append(
                &mut mesh,
                cube(
                    helix_point(model, i as f32 / (model.params().count - 1) as f32, strand),
                    Vec3::new(0.075, 0.075, 0.075),
                    rotation(model.time),
                ),
            );
        }
        item(
            scene,
            100 + strand as u64,
            mesh,
            palette(model, if strand == 0 { 0.85 } else { 0.25 }),
        );
    }
}
fn helix_links(scene: &mut ViewportScene, model: &Model) {
    let mut mesh = empty();
    for i in 0..model.params().count {
        let u = i as f32 / (model.params().count - 1) as f32;
        append(
            &mut mesh,
            tube(
                &[helix_point(model, u, 0), helix_point(model, u, 1)],
                0.025,
                6,
                false,
            ),
        );
    }
    item(scene, 300, mesh, [0.72, 0.53, 0.27, 1.0]);
}
fn bloom(scene: &mut ViewportScene, model: &Model) {
    let p = model.params();
    let phase = rotation(model.time);
    let mut meshes = (0..8).map(|_| empty()).collect::<Vec<_>>();
    for i in 0..p.count {
        let theta = TAU * i as f32 / p.count as f32;
        let mesh = surface(24, 4, |u, v| {
            let angle = theta + phase + p.twist * (1.0 - u) * 0.9;
            let r = 0.30 + u * p.radius * 1.23;
            let width = (std::f32::consts::PI * u).sin() * (0.25 + 14.0 / p.count as f32);
            Vec3::new(
                r * angle.cos() - v * width * angle.sin(),
                r * angle.sin() + v * width * angle.cos(),
                (std::f32::consts::PI * u).sin() * (0.25 + 0.5 * phase.sin())
                    + u * u * p.twist * 0.48
                    + v * v * 0.13,
            )
        });
        append(&mut meshes[i as usize % 8], mesh);
    }
    batches(scene, model, meshes);
}
fn spiral_point(model: &Model, u: f32) -> Vec3 {
    let p = model.params();
    let angle = TAU * (1.0 + p.twist * 1.7) * u + rotation(model.time);
    let radius = p.radius * (0.65 + 0.25 * (u * std::f32::consts::PI).sin());
    Vec3::new(radius * angle.cos(), (u - 0.5) * 4.8, radius * angle.sin())
}
fn spiral(scene: &mut ViewportScene, model: &Model) {
    let p = model.params();
    let phase = rotation(model.time);
    let mut meshes = (0..8).map(|_| empty()).collect::<Vec<_>>();
    for i in 0..p.count {
        let u = i as f32 / (p.count - 1) as f32;
        let size = 0.11 + 0.055 * (u * TAU * 2.0 + phase).sin().abs();
        let group = (u * 7.0).round() as usize;
        append(
            &mut meshes[group],
            cube(
                spiral_point(model, u),
                Vec3::new(size, size * 1.7, size),
                phase + u * TAU,
            ),
        );
    }
    batches(scene, model, meshes);
}
