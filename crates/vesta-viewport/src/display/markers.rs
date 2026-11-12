//! Neutral, constant-screen-size point billboards. Size is physical pixels.
use crate::{
    Camera, ViewportScene,
    render::{ViewportRect, view_projection},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkerStyle {
    pub diameter: f32,
    pub edge_width: f32,
    pub color: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkerVertex {
    pub clip_position: [f32; 4],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub softness: f32,
}

pub fn marker_triangles(
    scene: &ViewportScene,
    camera: Camera,
    rect: ViewportRect,
) -> Vec<MarkerVertex> {
    let Some(sources) = &scene.sources else {
        return Vec::new();
    };
    if rect.width == 0 || rect.height == 0 {
        return Vec::new();
    }
    let matrix = view_projection(camera, rect.width as f32 / rect.height as f32);
    let mut result = Vec::new();
    let content = sources.content_points.iter().map(|point| {
        (
            &point.position,
            MarkerStyle {
                diameter: 2.0,
                edge_width: 0.5,
                color: point.color,
            },
        )
    });
    let edit_style = sources.marker_style.filter(|_| scene.display.points);
    let markers = sources
        .points
        .iter()
        .filter_map(|point| edit_style.map(|style| (point, style)));
    for (point, style) in content.chain(markers) {
        if !style.diameter.is_finite()
            || style.diameter <= 0.0
            || !style.edge_width.is_finite()
            || style.edge_width <= 0.0
            || !style.color.into_iter().all(f32::is_finite)
        {
            continue;
        }
        let softness = 2.0 * style.edge_width / style.diameter;
        if !softness.is_finite() {
            continue;
        }
        if !point.iter().all(|v| v.is_finite()) {
            continue;
        }
        let p = [point[0], point[1], point[2], 1.0];
        let clip = std::array::from_fn::<_, 4, _>(|row| {
            (0..4).map(|col| matrix[col][row] * p[col]).sum::<f32>()
        });
        if !clip.into_iter().all(f32::is_finite) || clip[3] <= 0.0 {
            continue;
        }
        let start = result.len();
        for uv in [
            [-1.0, -1.0],
            [1.0, -1.0],
            [1.0, 1.0],
            [-1.0, -1.0],
            [1.0, 1.0],
            [-1.0, 1.0],
        ] {
            result.push(MarkerVertex {
                clip_position: [
                    clip[0] + uv[0] * style.diameter / rect.width as f32 * clip[3],
                    clip[1] + uv[1] * style.diameter / rect.height as f32 * clip[3],
                    clip[2],
                    clip[3],
                ],
                uv,
                color: style.color,
                softness,
            });
        }
        if result[start..]
            .iter()
            .any(|v| !v.clip_position.into_iter().all(f32::is_finite))
        {
            result.truncate(start);
        }
    }
    result
}
