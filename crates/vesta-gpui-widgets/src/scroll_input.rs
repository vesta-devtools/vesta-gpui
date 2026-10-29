//! A validated input path, shared by wheel propagation and thumb capture.
use crate::ScrollbarDrag;
use vesta_gpui::{ElementId, FocusScope, InputTarget, Point};

pub struct ScrollInputPath(Vec<ElementId>);
impl ScrollInputPath {
    pub fn new<A>(scope: &FocusScope<'_, A>, target: InputTarget) -> Option<Self> {
        scope.path_to(target).map(Self)
    }
    pub fn begin_drag(
        &self,
        pointer: Point,
        mut begin: impl FnMut(ElementId, Point) -> Option<ScrollbarDrag>,
    ) -> Option<(ElementId, ScrollbarDrag)> {
        if !pointer.x.is_finite() || !pointer.y.is_finite() {
            return None;
        }
        self.0
            .iter()
            .rev()
            .find_map(|&element| begin(element, pointer).map(|drag| (element, drag)))
    }
    pub fn scroll(
        &self,
        movement: Point,
        mut scroll: impl FnMut(ElementId, Point) -> Point,
    ) -> Point {
        if !movement.x.is_finite() || !movement.y.is_finite() {
            return movement;
        }
        let mut remaining = movement;
        for &element in self.0.iter().rev() {
            if remaining.x.abs() <= f32::EPSILON && remaining.y.abs() <= f32::EPSILON {
                break;
            }
            remaining = scroll(element, remaining);
        }
        remaining
    }
}
