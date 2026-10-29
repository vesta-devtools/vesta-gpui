//! Host-only layered transparency fixture for ADR-0312; no author data.
use vesta_viewport::{Mesh, Renderable, RenderableId, Vec3, Vertex, ViewportScene};

pub(super) fn scene() -> ViewportScene {
    let plane = |id, translation, half_width: f32, color| Renderable {
        id: RenderableId::new(id),
        mesh: Mesh {
            vertices: [
                [-half_width, -1.2, 0.0],
                [half_width, -1.2, 0.0],
                [half_width, 1.2, 0.0],
                [-half_width, 1.2, 0.0],
            ]
            .into_iter()
            .map(|position| Vertex {
                position,
                normal: [0.0, 0.0, -1.0],
            })
            .collect(),
            // Both faces remain visible when the existing orbit crosses the planes.
            indices: vec![0, 2, 1, 0, 3, 2, 0, 1, 2, 0, 2, 3],
        },
        translation,
        color,
        selected: false,
    };
    ViewportScene {
        renderables: vec![
            plane(101, Vec3::new(0.0, 1.4, -1.2), 1.8, [1.0, 0.1, 0.1, 0.45]),
            plane(102, Vec3::new(0.0, 1.4, 1.2), 1.8, [0.1, 0.2, 1.0, 0.45]),
            plane(103, Vec3::new(0.0, 1.4, 0.0), 0.45, [0.1, 0.9, 0.2, 1.0]),
        ],
        ..ViewportScene::default()
    }
}
