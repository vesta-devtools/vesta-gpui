//! Caller-owned measured heights with one validated range/projection authority.

use std::ops::Range;

use super::{
    Dimension, MAX_CONTENT_HEIGHT, MAX_OVERSCAN, MAX_VISIBLE_ROWS, VirtualListError,
    VirtualListIds, Widget, project_list,
};

/// Immutable logical-pixel geometry in the caller's current item order. Rebuild
/// when that order or its measured heights change; item identities stay caller-owned.
#[derive(Clone, Debug, PartialEq)]
pub struct VariableListMetrics {
    positions: Vec<f32>,
}

impl VariableListMetrics {
    pub fn new(heights: &[f32]) -> Result<Self, VirtualListError> {
        if heights.len() > MAX_CONTENT_HEIGHT as usize {
            return Err(VirtualListError::ContentTooLarge);
        }
        let mut positions = Vec::with_capacity(heights.len() + 1);
        positions.push(0.0);
        let mut total = 0.0_f64;
        for height in heights {
            if !height.is_finite() || *height < 1.0 {
                return Err(VirtualListError::InvalidGeometry);
            }
            total += f64::from(*height);
            if total > MAX_CONTENT_HEIGHT {
                return Err(VirtualListError::ContentTooLarge);
            }
            positions.push(total as f32);
        }
        Ok(Self { positions })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.positions.len() - 1
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn content_height(&self) -> f32 {
        self.positions[self.len()]
    }

    /// Geometry used by both projection and the caller's reveal policy.
    #[must_use]
    pub fn row_bounds(&self, index: usize) -> Option<(f32, f32)> {
        let top = *self.positions.get(index)?;
        let bottom = *self.positions.get(index.checked_add(1)?)?;
        Some((top, bottom - top))
    }

    pub fn range(
        &self,
        viewport_height: f32,
        offset: f32,
        overscan: usize,
    ) -> Result<VariableListRange<'_>, VirtualListError> {
        if !viewport_height.is_finite() || viewport_height <= 0.0 || !offset.is_finite() {
            return Err(VirtualListError::InvalidGeometry);
        }
        let content_height = self.content_height();
        let mut offset = offset.clamp(0.0, (content_height - viewport_height).max(0.0));
        // At large coordinates a positive fractional viewport can be smaller than
        // one f32 step. Round the maximum offset inward, keeping its last row live.
        if offset > 0.0
            && (offset >= content_height
                || f64::from(offset) + f64::from(viewport_height) > f64::from(content_height))
        {
            offset = offset.next_down().max(0.0);
        }
        let first = self
            .positions
            .partition_point(|top| *top <= offset)
            .saturating_sub(1)
            .min(self.len());
        let bottom = f64::from(offset) + f64::from(viewport_height);
        let end = self
            .positions
            .partition_point(|top| f64::from(*top) < bottom)
            .min(self.len())
            .max(first.saturating_add(1).min(self.len()));
        if end.saturating_sub(first) > MAX_VISIBLE_ROWS {
            return Err(VirtualListError::ViewportTooDense);
        }
        let padding = overscan.min(MAX_OVERSCAN);
        Ok(VariableListRange {
            metrics: self,
            visible: first.saturating_sub(padding)..end.saturating_add(padding).min(self.len()),
            offset,
            viewport_height,
        })
    }
}

/// A viewport borrowed from its immutable metrics. Callers cannot forge indices
/// or substitute another metrics instance between validation and projection.
#[derive(Clone, Debug, PartialEq)]
pub struct VariableListRange<'a> {
    metrics: &'a VariableListMetrics,
    visible: Range<usize>,
    offset: f32,
    viewport_height: f32,
}

impl VariableListRange<'_> {
    #[must_use]
    pub fn visible_rows(&self) -> Range<usize> {
        self.visible.clone()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.visible.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.visible.is_empty()
    }

    #[must_use]
    pub const fn offset(&self) -> f32 {
        self.offset
    }
}

/// Build only visible/overscan rows and at most one retained offscreen item.
/// The caller maps stable item identities to the current order, just as for the
/// fixed-height entry point. A count mismatch rejects before any row callback.
pub fn variable_list_with_items<T, A>(
    ids: VirtualListIds,
    width: Dimension,
    items: &[T],
    range: VariableListRange<'_>,
    retained_index: Option<usize>,
    mut row: impl FnMut(&T) -> Widget<A>,
) -> Result<Widget<A>, VirtualListError> {
    if items.len() != range.metrics.len() {
        return Err(VirtualListError::ItemCountMismatch);
    }
    let retained =
        retained_index.filter(|index| *index < items.len() && !range.visible.contains(index));
    let rows = range.visible.clone().chain(retained).map(|index| {
        let top = range.metrics.positions[index];
        let height = range.metrics.positions[index + 1] - top;
        (index, top, height)
    });
    Ok(project_list(
        ids,
        width,
        range.offset,
        range.viewport_height,
        range.metrics.content_height(),
        rows,
        |index| row(&items[index]),
    ))
}
