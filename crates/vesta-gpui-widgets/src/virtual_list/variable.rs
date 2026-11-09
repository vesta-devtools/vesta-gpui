//! Caller-owned measured heights with one validated range/projection authority.

use std::ops::Range;

use super::{
    Dimension, MAX_CONTENT_HEIGHT, MAX_OVERSCAN, MAX_VISIBLE_ROWS, VirtualListError,
    VirtualListIds, Widget, project_list,
};

mod measured;
#[cfg(test)]
mod tests;
pub use measured::{LazyListMetrics, ListMeasurement};

// A bounded local scan keeps the height storage near the old prefix vector's
// size. Only block totals need f64 Fenwick entries; item identities stay in Host.
const BLOCK_ROWS: usize = 64;

/// Measured logical-pixel geometry in the caller's current item order. Update
/// individual heights or splice visual positions; Host retains record identity.
#[derive(Clone, Debug, PartialEq)]
pub struct VariableListMetrics {
    heights: Vec<f32>,
    blocks: Vec<f64>,
    total: f64,
}

impl VariableListMetrics {
    pub fn new(heights: &[f32]) -> Result<Self, VirtualListError> {
        if heights.len() > MAX_CONTENT_HEIGHT as usize {
            return Err(VirtualListError::ContentTooLarge);
        }
        let mut total = 0.0_f64;
        for height in heights {
            if !height.is_finite() || *height < 1.0 {
                return Err(VirtualListError::InvalidGeometry);
            }
            total += f64::from(*height);
            if total > MAX_CONTENT_HEIGHT {
                return Err(VirtualListError::ContentTooLarge);
            }
        }
        Ok(Self {
            heights: heights.to_vec(),
            blocks: Self::build_blocks(heights),
            total,
        })
    }

    fn filled(count: usize, height: f32) -> Result<Self, VirtualListError> {
        if !height.is_finite() || height < 1.0 {
            return Err(VirtualListError::InvalidGeometry);
        }
        let total = count as f64 * f64::from(height);
        if count > MAX_CONTENT_HEIGHT as usize || total > MAX_CONTENT_HEIGHT {
            return Err(VirtualListError::ContentTooLarge);
        }
        let heights = vec![height; count];
        let blocks = Self::build_blocks(&heights);
        Ok(Self {
            heights,
            blocks,
            total,
        })
    }

    fn build_blocks(heights: &[f32]) -> Vec<f64> {
        let mut blocks = vec![0.0; heights.len().div_ceil(BLOCK_ROWS) + 1];
        for (index, chunk) in heights.chunks(BLOCK_ROWS).enumerate() {
            blocks[index + 1] = chunk.iter().map(|height| f64::from(*height)).sum();
        }
        for index in 1..blocks.len() {
            let parent = index + (index & index.wrapping_neg());
            if parent < blocks.len() {
                blocks[parent] += blocks[index];
            }
        }
        blocks
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.heights.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.heights.is_empty()
    }

    #[must_use]
    pub fn content_height(&self) -> f32 {
        self.total as f32
    }

    /// Replace one measured height without rebuilding other rows. Invalid
    /// indices/heights or an excessive total leave all geometry unchanged.
    /// Returns false for an unchanged value. The caller owns measurement work.
    pub fn update_height(&mut self, index: usize, height: f32) -> Result<bool, VirtualListError> {
        let old = *self
            .heights
            .get(index)
            .ok_or(VirtualListError::ItemIndexOutOfBounds)?;
        if !height.is_finite() || height < 1.0 {
            return Err(VirtualListError::InvalidGeometry);
        }
        let delta = f64::from(height) - f64::from(old);
        let total = self.total + delta;
        if total > MAX_CONTENT_HEIGHT {
            return Err(VirtualListError::ContentTooLarge);
        }
        if old == height {
            return Ok(false);
        }
        self.replace_height(index, height);
        Ok(true)
    }

    fn replace_height(&mut self, index: usize, height: f32) {
        if self.heights[index] == height {
            return;
        }
        let delta = f64::from(height) - f64::from(self.heights[index]);
        self.heights[index] = height;
        self.total += delta;
        let mut block = index / BLOCK_ROWS + 1;
        while block < self.blocks.len() {
            self.blocks[block] += delta;
            block += block & block.wrapping_neg();
        }
    }

    /// Replace a range of visual positions with new measured heights. The Host
    /// changes its records separately and remaps any captured stable-item anchor.
    /// Admission is atomic. Append and equal-length replacement are incremental;
    /// ordinary Vec growth can reallocate existing height storage on append.
    /// general insert/remove (including prepend) rebuilds the flat index in O(n).
    pub fn splice(
        &mut self,
        range: Range<usize>,
        heights: &[f32],
    ) -> Result<bool, VirtualListError> {
        if range.start > range.end || range.end > self.len() {
            return Err(VirtualListError::ItemIndexOutOfBounds);
        }
        let count = (self.len() - range.len())
            .checked_add(heights.len())
            .filter(|count| *count <= MAX_CONTENT_HEIGHT as usize)
            .ok_or(VirtualListError::ContentTooLarge)?;
        let removed = self.heights[range.clone()]
            .iter()
            .map(|h| f64::from(*h))
            .sum::<f64>();
        let mut total = self.total - removed;
        for height in heights {
            if !height.is_finite() || *height < 1.0 {
                return Err(VirtualListError::InvalidGeometry);
            }
            total += f64::from(*height);
            if total > MAX_CONTENT_HEIGHT {
                return Err(VirtualListError::ContentTooLarge);
            }
        }
        if self.heights[range.clone()] == *heights {
            return Ok(false);
        }
        if range.start == self.len() {
            for height in heights {
                let block = self.heights.len() / BLOCK_ROWS + 1;
                if block == self.blocks.len() {
                    // A growing Fenwick node contains all existing blocks in
                    // its low-bit range, not just the newly appended block.
                    let prior = self.prefix_blocks(block - 1)
                        - self.prefix_blocks(block - (block & block.wrapping_neg()));
                    self.blocks.push(prior);
                }
                self.heights.push(*height);
                let mut node = block;
                while node < self.blocks.len() {
                    self.blocks[node] += f64::from(*height);
                    node += node & node.wrapping_neg();
                }
            }
        } else if range.len() == heights.len() {
            for (index, height) in range.zip(heights) {
                self.replace_height(index, *height);
            }
        } else {
            self.heights.splice(range, heights.iter().copied());
            self.blocks = Self::build_blocks(&self.heights);
        }
        debug_assert_eq!(self.len(), count);
        self.total = total;
        Ok(true)
    }

    fn prefix_blocks(&self, mut block: usize) -> f64 {
        let mut height = 0.0;
        while block > 0 {
            height += self.blocks[block];
            block &= block - 1;
        }
        height
    }

    fn prefix_height(&self, index: usize) -> f64 {
        let block = index / BLOCK_ROWS;
        self.heights[block * BLOCK_ROWS..index]
            .iter()
            .map(|height| f64::from(*height))
            .sum::<f64>()
            + self.prefix_blocks(block)
    }

    // Partition cumulative *f32 edges*, preserving the original projection's
    // rounding. A tree search plus at most one block replaces a full prefix vec.
    fn partition_edges(&self, predicate: impl Fn(f32) -> bool) -> usize {
        if !predicate(0.0) {
            return 0;
        }
        let mut block = 0;
        let mut top = 0.0;
        let mut step = (self.blocks.len() - 1).next_power_of_two();
        while step > 0 {
            let next = block + step;
            if next < self.blocks.len() && predicate((top + self.blocks[next]) as f32) {
                top += self.blocks[next];
                block = next;
            }
            step /= 2;
        }
        let first = (block * BLOCK_ROWS).min(self.len());
        for index in first..(first + BLOCK_ROWS).min(self.len()) {
            top += f64::from(self.heights[index]);
            if !predicate(top as f32) {
                return index + 1;
            }
        }
        self.len() + 1
    }

    /// Geometry used by both projection and the caller's reveal policy.
    #[must_use]
    pub fn row_bounds(&self, index: usize) -> Option<(f32, f32)> {
        let height = *self.heights.get(index)?;
        let top = self.prefix_height(index);
        Some((top as f32, (top + f64::from(height)) as f32 - top as f32))
    }

    /// Restore an anchor after a height/order change. `index` is the caller's
    /// current position for the captured stable item, not necessarily its old
    /// index. The caller decides what to do when that item has been removed.
    /// Shrinking a row keeps the offset inside it; content edges take precedence.
    pub fn offset_for_anchor(
        &self,
        anchor: ListScrollAnchor,
        index: usize,
        viewport_height: f32,
    ) -> Result<f32, VirtualListError> {
        let (top, height) = self
            .row_bounds(index)
            .ok_or(VirtualListError::ItemIndexOutOfBounds)?;
        let bottom = top + height;
        let mut offset = (f64::from(top) + f64::from(anchor.offset_in_row)) as f32;
        if offset >= bottom {
            offset = bottom.next_down().max(top);
        }
        self.clamp_offset(viewport_height, offset)
    }

    fn clamp_offset(&self, viewport_height: f32, offset: f32) -> Result<f32, VirtualListError> {
        if !viewport_height.is_finite() || viewport_height <= 0.0 || !offset.is_finite() {
            return Err(VirtualListError::InvalidGeometry);
        }
        let content_height = self.content_height();
        let mut offset = offset.clamp(0.0, (content_height - viewport_height).max(0.0));
        // A fractional viewport may be smaller than one f32 step near the limit.
        if offset > 0.0
            && (offset >= content_height
                || f64::from(offset) + f64::from(viewport_height) > f64::from(content_height))
        {
            offset = offset.next_down().max(0.0);
        }
        Ok(offset)
    }

    pub fn range(
        &self,
        viewport_height: f32,
        offset: f32,
        overscan: usize,
    ) -> Result<VariableListRange<'_>, VirtualListError> {
        let offset = self.clamp_offset(viewport_height, offset)?;
        let first = self
            .partition_edges(|top| top <= offset)
            .saturating_sub(1)
            .min(self.len());
        let bottom = f64::from(offset) + f64::from(viewport_height);
        let end = self
            .partition_edges(|top| f64::from(top) < bottom)
            .min(self.len())
            .max(first.saturating_add(1).min(self.len()));
        if end.saturating_sub(first) > MAX_VISIBLE_ROWS {
            return Err(VirtualListError::ViewportTooDense);
        }
        let padding = overscan.min(MAX_OVERSCAN);
        Ok(VariableListRange {
            metrics: self,
            visible: first.saturating_sub(padding)..end.saturating_add(padding).min(self.len()),
            first,
            offset,
            viewport_height,
        })
    }
}

/// A scroll observation in the current item order, not a stable data identity.
/// Keep the corresponding item ID in Host and remap it before restoration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ListScrollAnchor {
    index: usize,
    offset_in_row: f32,
}

impl ListScrollAnchor {
    pub(super) const fn at(index: usize, offset_in_row: f32) -> Self {
        Self {
            index,
            offset_in_row,
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }

    #[must_use]
    pub const fn offset_in_row(self) -> f32 {
        self.offset_in_row
    }
}

/// A viewport borrowed from its immutable metrics. Callers cannot forge indices
/// or substitute another metrics instance between validation and projection.
#[derive(Clone, Debug, PartialEq)]
pub struct VariableListRange<'a> {
    metrics: &'a VariableListMetrics,
    visible: Range<usize>,
    first: usize,
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

    /// Capture the first actually visible row, excluding overscan. Empty lists
    /// have no anchor. The snapshot does not retain or lock its metrics.
    #[must_use]
    pub fn anchor(&self) -> Option<ListScrollAnchor> {
        let (top, _) = self.metrics.row_bounds(self.first)?;
        Some(ListScrollAnchor::at(self.first, self.offset - top))
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
    let mut edge = range.metrics.prefix_height(range.visible.start);
    let rows = range
        .visible
        .clone()
        .map(|index| {
            let top = edge as f32;
            edge += f64::from(range.metrics.heights[index]);
            (index, top, edge as f32 - top)
        })
        .chain(retained.map(|index| {
            let (top, height) = range
                .metrics
                .row_bounds(index)
                .expect("validated retained index");
            (index, top, height)
        }));
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
