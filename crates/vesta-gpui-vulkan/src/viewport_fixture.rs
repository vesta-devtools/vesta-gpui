//! Private input for renderer geometry/shader/reconfiguration tests only.
use vesta_viewport::{
    GizmoState, Mesh, Renderable, RenderableId, TransformMode, TransformSpace, Vec3, Vertex,
    ViewportScene,
};

pub(crate) fn scene() -> ViewportScene {
    ViewportScene {
        display: Default::default(),
        grid_lines: None,
        sources: None,
        renderables: vec![
            Renderable {
                id: RenderableId::new(1),
                mesh: cube_mesh(),
                translation: Vec3::new(-0.75, 0.7, 0.0),
                color: [0.18, 0.50, 0.95, 1.0],
                selected: true,
            },
            Renderable {
                id: RenderableId::new(2),
                mesh: cube_mesh(),
                translation: Vec3::new(0.85, 0.45, 0.45),
                color: [0.93, 0.42, 0.20, 1.0],
                selected: false,
            },
        ],
        hit_proxies: Vec::new(),
        materials: Default::default(),
        vertex_colors: Default::default(),
        gizmo: GizmoState {
            mode: TransformMode::Translate,
            space: TransformSpace::World,
            active: None,
        },
    }
}

fn cube_mesh() -> Mesh {
    let positions = [
        [-0.5, -0.5, 0.5],
        [0.5, -0.5, 0.5],
        [0.5, 0.5, 0.5],
        [-0.5, 0.5, 0.5],
        [0.5, -0.5, -0.5],
        [-0.5, -0.5, -0.5],
        [-0.5, 0.5, -0.5],
        [0.5, 0.5, -0.5],
    ];
    let vertices = positions
        .into_iter()
        .map(|position| Vertex {
            position,
            normal: {
                let normal = Vec3::new(position[0], position[1], position[2]).normalized();
                [normal.x, normal.y, normal.z]
            },
        })
        .collect();
    Mesh {
        vertices,
        indices: vec![
            0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7, 5, 0, 3, 5, 3, 6, 1, 4, 7, 1, 7, 2, 3, 2, 7, 3, 7,
            6, 5, 4, 1, 5, 1, 0,
        ],
    }
}
