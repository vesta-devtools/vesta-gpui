//! Shared numeric admission; no model, window or scheduling state.
use crate::MIN_DOCK_PANE_EXTENT;
use vesta_gpui::{FlowDirection, Point, Rect};
use vesta_workspace::SplitAxis;

pub(crate) fn valid_rect(rect: Rect) -> bool {
    rect.width >= 0.0
        && rect.height >= 0.0
        && [
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            rect.right(),
            rect.bottom(),
        ]
        .into_iter()
        .all(f32::is_finite)
}
pub(crate) fn hittable_rect(rect: Rect) -> bool {
    valid_rect(rect) && rect.width > 0.0 && rect.height > 0.0
}
pub(crate) fn finite_point(point: Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

/// Resolve a split against its own bounds. Invalid geometry, pointer or minimum
/// returns 0.5; a negative finite minimum means zero. Valid output is bounded.
#[must_use]
pub fn split_ratio_at(
    rect: Rect,
    axis: SplitAxis,
    direction: FlowDirection,
    pointer: Point,
    minimum_pane_extent: f32,
) -> f32 {
    if !hittable_rect(rect) || !finite_point(pointer) || !minimum_pane_extent.is_finite() {
        return 0.5;
    }
    let (position, origin, extent) = match axis {
        SplitAxis::Row => (pointer.x, rect.x, rect.width),
        SplitAxis::Column => (pointer.y, rect.y, rect.height),
    };
    let minimum = (minimum_pane_extent.max(0.0) / extent).clamp(0.05, 0.5);
    if minimum >= 0.5 {
        0.5
    } else {
        let fraction = (position - origin) / extent;
        let fraction = if axis == SplitAxis::Row && direction == FlowDirection::RightToLeft {
            1.0 - fraction
        } else {
            fraction
        };
        fraction.clamp(minimum, 1.0 - minimum)
    }
}

/// Move by a pixel delta; infinite deltas are explicit Home/End-style jumps.
#[must_use]
pub fn split_ratio_from_delta(
    rect: Rect,
    axis: SplitAxis,
    direction: FlowDirection,
    current: f32,
    delta: f32,
) -> f32 {
    let extent = match axis {
        SplitAxis::Row => rect.width,
        SplitAxis::Column => rect.height,
    };
    let position = if delta == f32::NEG_INFINITY {
        0.0
    } else if delta == f32::INFINITY {
        extent
    } else {
        let physical = if axis == SplitAxis::Row && direction == FlowDirection::RightToLeft {
            1.0 - current
        } else {
            current
        };
        physical * extent + delta
    };
    let pointer = match axis {
        SplitAxis::Row => Point::new(rect.x + position, rect.y),
        SplitAxis::Column => Point::new(rect.x, rect.y + position),
    };
    split_ratio_at(rect, axis, direction, pointer, MIN_DOCK_PANE_EXTENT)
}
