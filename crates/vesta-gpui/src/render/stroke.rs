//! Connected stroke topology for the existing backend-neutral shape stream.
use super::{ShapeVertex, linear_color};
use crate::{Color, Point, Viewport};

type Coordinate = [f64; 2];

struct Segment {
    start: [Coordinate; 2],
    end: [Coordinate; 2],
    direction: Coordinate,
    length: f64,
    to: Coordinate,
}

/// Butt endpoints and bevel joins; an exact repeated first point closes the join.
/// Ordinary bends share their inner edge. Short runs, reversals and self-crossing
/// strokes may overlap: this is not a global path-union tessellator.
/// Invalid input or unrepresentable output produces an empty mesh.
#[must_use]
pub fn polyline_vertices(
    points: &[Point],
    width: f32,
    color: Color,
    viewport: Viewport,
) -> Vec<ShapeVertex> {
    if !viewport.is_valid()
        || !width.is_finite()
        || width <= 0.0
        || points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite())
        || ![color.red, color.green, color.blue, color.alpha]
            .into_iter()
            .all(f32::is_finite)
    {
        return Vec::new();
    }
    let half = f64::from(width) * 0.5;
    let mut segments = Vec::new();
    for pair in points.windows(2) {
        let from = [f64::from(pair[0].x), f64::from(pair[0].y)];
        let to = [f64::from(pair[1].x), f64::from(pair[1].y)];
        let delta = [to[0] - from[0], to[1] - from[1]];
        let length = delta[0].hypot(delta[1]);
        if length == 0.0 {
            continue;
        }
        let direction = [delta[0] / length, delta[1] / length];
        let normal = [-direction[1] * half, direction[0] * half];
        let edges = |p: Coordinate| {
            [
                [p[0] + normal[0], p[1] + normal[1]],
                [p[0] - normal[0], p[1] - normal[1]],
            ]
        };
        segments.push(Segment {
            start: edges(from),
            end: edges(to),
            direction,
            length,
            to,
        });
    }
    let mut vertices = Vec::new();
    let color = linear_color(color);
    for i in 1..segments.len() {
        let (before, after) = segments.split_at_mut(i);
        if let Some(points) = join(&mut before[i - 1], &mut after[0], half)
            && !triangle(&mut vertices, points, color, viewport)
        {
            return Vec::new();
        }
    }
    if segments.len() > 1 && points.first() == points.last() {
        let (first, rest) = segments.split_at_mut(1);
        if let Some(points) = join(
            rest.last_mut().expect("closed stroke has two segments"),
            &mut first[0],
            half,
        ) && !triangle(&mut vertices, points, color, viewport)
        {
            return Vec::new();
        }
    }
    for segment in segments {
        let [a, b] = segment.start;
        let [d, c] = segment.end;
        if !triangle(&mut vertices, [a, b, c], color, viewport)
            || !triangle(&mut vertices, [a, c, d], color, viewport)
        {
            return Vec::new();
        }
    }
    vertices
}

fn join(previous: &mut Segment, next: &mut Segment, half: f64) -> Option<[Coordinate; 3]> {
    let [a, b] = [previous.direction, next.direction];
    let turn = a[0] * b[1] - a[1] * b[0];
    let dot = a[0] * b[0] + a[1] * b[1];
    if turn == 0.0 {
        if dot > 0.0 {
            next.start = previous.end;
        }
        return None;
    }
    let point = previous.to;
    let inner_side = usize::from(turn < 0.0);
    let outer_side = 1 - inner_side;
    let outer = [previous.end[outer_side], next.start[outer_side]];
    let denominator = 1.0 + dot;
    let distance = half * turn.abs() / denominator;
    let mut inner = point;
    if denominator > 0.0 && distance <= previous.length && distance <= next.length {
        let sign = if turn > 0.0 { 1.0 } else { -1.0 };
        inner = [
            point[0] - sign * (a[1] + b[1]) * half / denominator,
            point[1] + sign * (a[0] + b[0]) * half / denominator,
        ];
        previous.end[inner_side] = inner;
        next.start[inner_side] = inner;
    }
    // An intersection beyond a short segment would fold or spike its mesh.
    // Retain those bounded segment ends and fill the outer wedge from the bend.
    Some([outer[0], outer[1], inner])
}

fn triangle(
    vertices: &mut Vec<ShapeVertex>,
    mut points: [Coordinate; 3],
    color: [f32; 4],
    viewport: Viewport,
) -> bool {
    let [a, b, c] = points;
    let cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if cross == 0.0 {
        return true;
    }
    if cross < 0.0 {
        points.swap(1, 2);
    }
    let positions = points.map(|point| {
        [
            (point[0] / f64::from(viewport.width) * 2.0 - 1.0) as f32,
            (1.0 - point[1] / f64::from(viewport.height) * 2.0) as f32,
        ]
    });
    if positions.iter().flatten().any(|p| !p.is_finite()) {
        return false;
    }
    vertices.extend(positions.map(|position| ShapeVertex {
        position,
        local: [0.0; 2],
        size: [0.0; 2],
        radius_border: [-1.0, 0.0],
        color,
    }));
    true
}
