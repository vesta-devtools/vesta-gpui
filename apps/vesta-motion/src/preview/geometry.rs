//! Bounded display mesh helpers for the preview study gallery (ADR-0524).
use super::*;

pub fn empty() -> Mesh {
    Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
    }
}
pub fn append(target: &mut Mesh, mesh: Mesh) {
    let offset = target.vertices.len() as u32;
    target.vertices.extend(mesh.vertices);
    target
        .indices
        .extend(mesh.indices.into_iter().map(|i| i + offset));
}
pub fn rotate_y(p: Vec3, angle: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    Vec3::new(p.x * c + p.z * s, p.y, -p.x * s + p.z * c)
}
pub fn rotate_x(p: Vec3, angle: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    Vec3::new(p.x, p.y * c - p.z * s, p.y * s + p.z * c)
}
pub fn rotate_z(p: Vec3, angle: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    Vec3::new(p.x * c - p.y * s, p.x * s + p.y * c, p.z)
}
fn vertex(p: Vec3, normal: Vec3) -> Vertex {
    Vertex {
        position: [p.x, p.y, p.z],
        normal: [normal.x, normal.y, normal.z],
    }
}
pub fn cube(center: Vec3, half: Vec3, angle: f32) -> Mesh {
    let mut mesh = empty();
    let (x, y, z) = (half.x, half.y, half.z);
    let faces = [
        (
            [[-x, -y, z], [x, -y, z], [x, y, z], [-x, y, z]],
            Vec3::new(0.0, 0.0, 1.0),
        ),
        (
            [[x, -y, -z], [-x, -y, -z], [-x, y, -z], [x, y, -z]],
            Vec3::new(0.0, 0.0, -1.0),
        ),
        (
            [[x, -y, z], [x, -y, -z], [x, y, -z], [x, y, z]],
            Vec3::new(1.0, 0.0, 0.0),
        ),
        (
            [[-x, -y, -z], [-x, -y, z], [-x, y, z], [-x, y, -z]],
            Vec3::new(-1.0, 0.0, 0.0),
        ),
        (
            [[-x, y, z], [x, y, z], [x, y, -z], [-x, y, -z]],
            Vec3::new(0.0, 1.0, 0.0),
        ),
        (
            [[-x, -y, -z], [x, -y, -z], [x, -y, z], [-x, -y, z]],
            Vec3::new(0.0, -1.0, 0.0),
        ),
    ];
    for (points, normal) in faces {
        let offset = mesh.vertices.len() as u32;
        for p in points {
            mesh.vertices.push(vertex(
                rotate_y(Vec3::new(p[0], p[1], p[2]), angle).plus(center),
                rotate_y(normal, angle),
            ));
        }
        mesh.indices.extend([
            offset,
            offset + 1,
            offset + 2,
            offset,
            offset + 2,
            offset + 3,
        ]);
    }
    mesh
}
pub fn tube(points: &[Vec3], radius: f32, sides: u32, closed: bool) -> Mesh {
    let mut mesh = empty();
    for (i, &p) in points.iter().enumerate() {
        let before = if i == 0 {
            if closed { points[points.len() - 2] } else { p }
        } else {
            points[i - 1]
        };
        let after = if i + 1 == points.len() {
            if closed { points[1] } else { p }
        } else {
            points[i + 1]
        };
        let tangent = after.minus(before).normalized();
        let reference = if tangent.z.abs() > 0.95 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        };
        let n = tangent.cross(reference).normalized();
        let b = tangent.cross(n).normalized();
        for side in 0..=sides {
            let angle = TAU * side as f32 / sides as f32;
            let normal = n.scaled(angle.cos()).plus(b.scaled(angle.sin()));
            mesh.vertices
                .push(vertex(p.plus(normal.scaled(radius)), normal));
        }
        if i + 1 < points.len() {
            for side in 0..sides {
                let a = i as u32 * (sides + 1) + side;
                let next = a + sides + 1;
                // Angular derivative crossed with path tangent faces outwards.
                mesh.indices.extend([a, a + 1, next, a + 1, next + 1, next]);
            }
        }
    }
    mesh
}
pub fn surface(rows: u32, columns: u32, sample: impl Fn(f32, f32) -> Vec3) -> Mesh {
    let mut mesh = empty();
    for row in 0..=rows {
        for column in 0..=columns {
            let u = row as f32 / rows as f32;
            let v = column as f32 / columns as f32 * 2.0 - 1.0;
            let p = sample(u, v);
            let du = sample(u + 0.0005, v).minus(sample(u - 0.0005, v));
            let dv = sample(u, v + 0.0005).minus(sample(u, v - 0.0005));
            let n = du.cross(dv);
            let normal = if n.dot(n) < 1e-12 {
                Vec3::new(0.0, 0.0, 1.0)
            } else {
                n.normalized()
            };
            mesh.vertices.push(vertex(p, normal));
            if row < rows && column < columns {
                let a = row * (columns + 1) + column;
                let next = a + columns + 1;
                mesh.indices.extend([a, next, a + 1, a + 1, next, next + 1]);
            }
        }
    }
    let offset = mesh.vertices.len() as u32;
    let backs = mesh
        .vertices
        .iter()
        .map(|v| Vertex {
            position: v.position,
            normal: [-v.normal[0], -v.normal[1], -v.normal[2]],
        })
        .collect::<Vec<_>>();
    let indices = mesh
        .indices
        .chunks_exact(3)
        .flat_map(|t| [t[0] + offset, t[2] + offset, t[1] + offset])
        .collect::<Vec<_>>();
    mesh.vertices.extend(backs);
    mesh.indices.extend(indices);
    mesh
}
