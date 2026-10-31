//! Retained scroll geometry and captured arithmetic; Host owns input and scheduling.

#[cfg(test)]
use vesta_gpui::scrollbar_geometry;
use vesta_gpui::{
    ElementId, FlowDirection, FocusScope, InputTarget, LayoutNode, Point, Rect, Size,
    scrollbar_geometry_in_direction,
};

/// Host-provided visible scroll geometry; margins remain a presentation choice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollRevealViewport {
    pub rect: Rect,
    pub margin: f32,
}

/// One inner-to-outer reveal request for an existing retained scroll state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollRevealStep {
    pub element: ElementId,
    pub target: Rect,
    pub viewport: ScrollRevealViewport,
}

/// Reveal a focused descendant through its current root-to-target action path.
/// After revealing an inner viewport, its outer ancestor reveals the viewport itself.
/// Invalid geometry rejects the whole plan before any returned steps are applied.
#[must_use]
pub fn scroll_reveal_path(
    path: &[ElementId],
    mut target: Rect,
    mut viewport: impl FnMut(ElementId) -> Option<ScrollRevealViewport>,
) -> Vec<ScrollRevealStep> {
    if !target.is_valid() {
        return Vec::new();
    }
    let mut steps = Vec::new();
    for &element in path.iter().rev() {
        if let Some(viewport) = viewport(element) {
            if !viewport.rect.is_valid() || !viewport.margin.is_finite() {
                return Vec::new();
            }
            steps.push(ScrollRevealStep {
                element,
                target,
                viewport,
            });
            target = viewport.rect;
        }
    }
    steps
}

/// Offer each axis's remainder to live scroll owners, innermost first. The host
/// supplies retained states by stable element identity; this function owns only
/// propagation. A foreign, disabled, removed or modal-excluded hit receives no update.
pub fn scroll_wheel_path<A>(
    scope: &FocusScope<'_, A>,
    target: InputTarget,
    movement: Point,
    scroll: impl FnMut(ElementId, Point) -> Point,
) -> Point {
    let Some(path) = crate::ScrollInputPath::new(scope, target) else {
        return movement;
    };
    path.scroll(movement, scroll)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollAxis {
    Horizontal,
    Vertical,
}

/// A thumb drag keeps its starting geometry so pointer movement maps to content offset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarDrag {
    axis: ScrollAxis,
    flow: FlowDirection,
    pointer_start: f32,
    offset_start: f32,
    travel: f32,
}

impl ScrollbarDrag {
    #[must_use]
    pub const fn axis(self) -> ScrollAxis {
        self.axis
    }

    pub fn update(self, state: &mut ScrollAreaState, pointer: Point) -> bool {
        if !pointer.x.is_finite() || !pointer.y.is_finite() {
            return false;
        }
        let max = match self.axis {
            ScrollAxis::Horizontal => state.max.x,
            ScrollAxis::Vertical => state.max.y,
        };
        if max == 0.0 {
            return false;
        }
        let old = state.offset;
        match self.axis {
            ScrollAxis::Horizontal => {
                state.offset.x = (self.offset_start
                    + self.flow.horizontal(pointer.x - self.pointer_start) * state.max.x
                        / self.travel)
                    .clamp(0.0, state.max.x);
            }
            ScrollAxis::Vertical => {
                state.offset.y = (self.offset_start
                    + (pointer.y - self.pointer_start) * state.max.y / self.travel)
                    .clamp(0.0, state.max.y);
            }
        }
        state.offset != old
    }
}

/// Retained scroll offset for a declarative `ScrollBox`. Positive deltas move
/// content upward; unconsumed delta can be offered to an ancestor scroll area.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollAreaState {
    offset: Point,
    max: Point,
    flow: FlowDirection,
}

impl ScrollAreaState {
    #[must_use]
    pub fn begin_thumb_drag(self, viewport: Rect, pointer: Point) -> Option<ScrollbarDrag> {
        let content = Size::new(viewport.width + self.max.x, viewport.height + self.max.y);
        if !viewport.is_valid() || !content.is_valid() {
            return None;
        }
        let (vertical_track, vertical_thumb, horizontal_track, horizontal_thumb) =
            scrollbar_geometry_in_direction(viewport, content, self.offset, self.flow);
        self.begin_drag_from_geometry(
            pointer,
            vertical_track,
            vertical_thumb,
            horizontal_track,
            horizontal_thumb,
            self.flow,
        )
    }

    /// Use the same reserved gutters and tracks as the window's painter.
    #[must_use]
    pub fn begin_thumb_drag_in_layout(
        self,
        node: &LayoutNode,
        pointer: Point,
    ) -> Option<ScrollbarDrag> {
        let (width, height) = node.scroll_content.unwrap_or_default();
        if !node.rect.is_valid()
            || !node.scroll_viewport().is_valid()
            || !Size::new(width, height).is_valid()
        {
            return None;
        }
        let (vertical_track, vertical_thumb, horizontal_track, horizontal_thumb) =
            node.scrollbar_geometry(self.offset);
        self.begin_drag_from_geometry(
            pointer,
            vertical_track,
            vertical_thumb,
            horizontal_track,
            horizontal_thumb,
            node.flow_direction,
        )
    }

    fn begin_drag_from_geometry(
        self,
        pointer: Point,
        vertical_track: Option<Rect>,
        vertical_thumb: Option<Rect>,
        horizontal_track: Option<Rect>,
        horizontal_thumb: Option<Rect>,
        flow: FlowDirection,
    ) -> Option<ScrollbarDrag> {
        if !pointer.x.is_finite() || !pointer.y.is_finite() {
            return None;
        }
        let candidates = [
            (ScrollAxis::Vertical, vertical_track, vertical_thumb),
            (ScrollAxis::Horizontal, horizontal_track, horizontal_thumb),
        ];
        candidates.into_iter().find_map(|(axis, track, thumb)| {
            let (Some(track), Some(thumb)) = (track, thumb) else {
                return None;
            };
            if !track.is_valid() || !thumb.is_valid() || !thumb.contains(pointer) {
                return None;
            }
            let (pointer_start, offset_start, travel) = match axis {
                ScrollAxis::Horizontal => (
                    pointer.x,
                    self.offset.x,
                    (track.width - thumb.width).max(0.0),
                ),
                ScrollAxis::Vertical => (
                    pointer.y,
                    self.offset.y,
                    (track.height - thumb.height).max(0.0),
                ),
            };
            (travel > 0.0).then_some(ScrollbarDrag {
                axis,
                flow,
                pointer_start,
                offset_start,
                travel,
            })
        })
    }

    #[must_use]
    pub fn new(initial_y: f32) -> Self {
        Self::new_2d(Point::new(0.0, initial_y))
    }

    #[must_use]
    pub fn new_2d(initial: Point) -> Self {
        let component = |value: f32| {
            if value.is_finite() {
                value.max(0.0)
            } else {
                0.0
            }
        };
        let initial = Point::new(component(initial.x), component(initial.y));
        Self {
            offset: initial,
            max: initial,
            flow: FlowDirection::LeftToRight,
        }
    }

    #[must_use]
    pub const fn offset(self) -> Point {
        self.offset
    }

    #[must_use]
    pub const fn offset_y(self) -> f32 {
        self.offset.y
    }

    #[must_use]
    pub const fn max(self) -> Point {
        self.max
    }

    #[must_use]
    pub const fn max_y(self) -> f32 {
        self.max.y
    }

    /// Synchronize direction and both axes from the window's current projection.
    pub fn sync_layout(&mut self, node: &LayoutNode) -> bool {
        let (width, height) = node.scroll_content.unwrap_or_default();
        let viewport = node.scroll_viewport();
        if !Size::new(width, height).is_valid()
            || !Size::new(viewport.width, viewport.height).is_valid()
        {
            return false;
        }
        let direction_changed = self.flow != node.flow_direction;
        self.flow = node.flow_direction;
        self.sync_content_size(
            Size::new(width, height),
            Size::new(viewport.width, viewport.height),
        ) || direction_changed
    }

    /// Reconcile both axes after a declarative layout rebuild.
    pub fn sync_content_size(&mut self, content: Size, visible: Size) -> bool {
        if !content.is_valid() || !visible.is_valid() {
            return false;
        }
        self.max = Point::new(
            (content.width - visible.width).max(0.0),
            (content.height - visible.height).max(0.0),
        );
        let old = self.offset;
        self.offset.x = self.offset.x.min(self.max.x);
        self.offset.y = self.offset.y.min(self.max.y);
        self.offset != old
    }

    /// Reconcile a rebuilt layout with retained scroll state.
    pub fn sync_content_height(&mut self, content_height: f32, visible_height: f32) -> bool {
        if !Size::new(content_height, visible_height).is_valid() {
            return false;
        }
        self.max.y = (content_height - visible_height).max(0.0);
        let old = self.offset.y;
        self.offset.y = self.offset.y.min(self.max.y);
        self.offset.y != old
    }

    /// Returns the unconsumed movement for each axis so an ancestor may scroll it.
    pub fn scroll_by_point(&mut self, delta: Point) -> Point {
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return delta;
        }
        let logical_x = self.flow.horizontal(delta.x);
        let old = self.offset;
        self.offset.x = (self.offset.x + logical_x).clamp(0.0, self.max.x);
        self.offset.y = (self.offset.y + delta.y).clamp(0.0, self.max.y);
        Point::new(
            self.flow.horizontal(logical_x - (self.offset.x - old.x)),
            delta.y - (self.offset.y - old.y),
        )
    }

    /// Returns the portion not consumed at this scroll boundary.
    pub fn scroll_by(&mut self, delta_y: f32) -> f32 {
        self.scroll_by_point(Point::new(0.0, delta_y)).y
    }

    pub fn reveal(&mut self, target: Rect, visible: Rect, margin: f32) -> bool {
        if !target.is_valid() || !visible.is_valid() || !margin.is_finite() {
            return false;
        }
        let margin = margin.max(0.0);
        // A requested margin must not push a nearly full target out of view.
        let margin_x = margin.min(((visible.width - target.width) * 0.5).max(0.0));
        let margin_y = margin.min(((visible.height - target.height) * 0.5).max(0.0));
        let delta_x = if target.x < visible.x + margin_x {
            target.x - visible.x - margin_x
        } else if target.right() > visible.right() - margin_x {
            target.right() - visible.right() + margin_x
        } else {
            0.0
        };
        let delta_y = if target.y < visible.y + margin_y {
            target.y - visible.y - margin_y
        } else if target.bottom() > visible.bottom() - margin_y {
            target.bottom() - visible.bottom() + margin_y
        } else {
            0.0
        };
        let old = self.offset;
        // A revealed edge can return a few f32 rounding units beyond its margin
        // after layout subtracts the retained offset. Do not schedule another
        // frame solely to chase that cancellation noise. Real wheel/drag deltas
        // are not subject to this reveal-only arithmetic tolerance.
        let stable_delta = |delta: f32, start: f32, end: f32, low: f32, high: f32| {
            let precision = [start, end, low, high]
                .into_iter()
                .map(f32::abs)
                .fold(1.0_f32, f32::max)
                * f32::EPSILON
                * 4.0;
            if delta.abs() <= precision { 0.0 } else { delta }
        };
        self.scroll_by_point(Point::new(
            stable_delta(
                delta_x,
                target.x,
                target.right(),
                visible.x,
                visible.right(),
            ),
            stable_delta(
                delta_y,
                target.y,
                target.bottom(),
                visible.y,
                visible.bottom(),
            ),
        ));
        self.offset != old
    }
}

#[cfg(test)]
mod tests;
