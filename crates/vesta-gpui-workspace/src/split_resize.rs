//! Window-owned splitter preview and one-shot completion, independent of a host.

use crate::{MIN_DOCK_PANE_EXTENT, split_ratio_at};
use vesta_gpui::{InputTarget, Point, Rect, UiWindowId};
use vesta_gpui_widgets::{PointerEditOutcome, ValueEditSession};
use vesta_workspace::{ContainerRef, SplitAxis, SplitId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitResizeError {
    InvalidGeometry,
    InvalidRatio,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitResizeSession {
    container: ContainerRef,
    split: SplitId,
    axis: SplitAxis,
    bounds: Rect,
    edit: ValueEditSession<f32>,
}

impl SplitResizeSession {
    pub fn new(
        target: InputTarget,
        container: ContainerRef,
        split: SplitId,
        axis: SplitAxis,
        bounds: Rect,
        ratio: f32,
    ) -> Result<Self, SplitResizeError> {
        if !bounds.x.is_finite()
            || !bounds.y.is_finite()
            || !bounds.width.is_finite()
            || !bounds.height.is_finite()
            || bounds.width <= 0.0
            || bounds.height <= 0.0
        {
            return Err(SplitResizeError::InvalidGeometry);
        }
        if !ratio.is_finite() || !(0.0..=1.0).contains(&ratio) {
            return Err(SplitResizeError::InvalidRatio);
        }
        Ok(Self {
            container,
            split,
            axis,
            bounds,
            edit: ValueEditSession::new(target, ratio),
        })
    }

    #[must_use]
    pub const fn target(&self) -> InputTarget {
        self.edit.target()
    }
    #[must_use]
    pub const fn container(&self) -> ContainerRef {
        self.container
    }
    #[must_use]
    pub const fn split(&self) -> SplitId {
        self.split
    }
    #[must_use]
    pub const fn axis(&self) -> SplitAxis {
        self.axis
    }

    #[must_use]
    pub fn ratio_for_pointer(&self, window: UiWindowId, pointer: Point) -> Option<f32> {
        (self.edit.accepts(window) && pointer.x.is_finite() && pointer.y.is_finite())
            .then(|| split_ratio_at(self.bounds, self.axis, pointer, MIN_DOCK_PANE_EXTENT))
    }

    pub fn finish(
        &mut self,
        window: UiWindowId,
        current: f32,
        commit: bool,
    ) -> Option<PointerEditOutcome<f32>> {
        self.edit.finish(window, &current, commit)
    }
}
