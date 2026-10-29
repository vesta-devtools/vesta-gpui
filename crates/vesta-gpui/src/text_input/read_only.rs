//! Read-only source mode and grapheme-safe selection over the same Parley editor.
use super::*;

impl TextInputSnapshot {
    /// Closest Parley cluster edge in layout coordinates. Hosts apply viewport
    /// clipping first; document owners additionally validate grapheme anchors.
    pub fn byte_index_at(&self, point: Point) -> Option<usize> {
        if !point.x.is_finite() || !point.y.is_finite() || self.0.layout.lines().next().is_none() {
            return None;
        }
        Some(parley::editing::Cursor::from_point(&self.0.layout, point.x, point.y).index())
    }

    /// Logical rectangles of an exact cluster-edge byte range. No coordinate
    /// approximation or independent shaping is used for wrapped/bidi runs.
    pub fn range_rectangles(
        &self,
        range: std::ops::Range<usize>,
    ) -> Result<Vec<Rect>, StyledTextError> {
        let start = parley::editing::Cursor::from_byte_index(
            &self.0.layout,
            range.start,
            parley::Affinity::Downstream,
        );
        let end = parley::editing::Cursor::from_byte_index(
            &self.0.layout,
            range.end,
            parley::Affinity::Upstream,
        );
        if range.start > range.end || start.index() != range.start || end.index() != range.end {
            return Err(StyledTextError::InvalidRange);
        }
        Ok(parley::editing::Selection::new(start, end)
            .geometry(&self.0.layout)
            .into_iter()
            .map(|(bounds, _)| rect(bounds))
            .collect())
    }

    /// Decorate static text without copying its glyph layout or source. Editor
    /// caret/composition and its owning buffer must stay on the editor path.
    pub fn with_read_only_selection(
        mut self,
        range: std::ops::Range<usize>,
    ) -> Result<Self, StyledTextError> {
        if self.0.caret.is_some() || !self.0.compose.is_empty() {
            return Err(StyledTextError::InteractiveSnapshot);
        }
        if self
            .3
            .as_ref()
            .is_some_and(|current| current.range == range)
        {
            return Ok(self);
        }
        self.3 = Some(Arc::new(SnapshotSelection {
            rectangles: self.range_rectangles(range.clone())?,
            range,
        }));
        Ok(self)
    }

    pub(crate) fn selection_rectangles(&self) -> &[Rect] {
        self.3
            .as_ref()
            .map_or(&self.0.selection, |selection| &selection.rectangles)
    }

    pub(crate) fn selection_range(&self) -> Option<[usize; 2]> {
        self.3
            .as_ref()
            .map(|selection| [selection.range.start, selection.range.end])
    }
}

impl TextInputState {
    /// Owner reset supplies the source; user edits cannot mutate this buffer.
    pub fn new_read_only(font_size: f32, font_stack: &str, wrap_width: Option<f32>) -> Self {
        let mut state = match wrap_width {
            Some(width) => Self::new_multiline(font_size, font_stack, width),
            None => Self::new(font_size, font_stack),
        };
        state.read_only = true;
        state
    }

    #[must_use]
    pub const fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub(super) fn read_only_boundary(&self, index: usize) -> bool {
        index == self.text().len()
            || self
                .text()
                .grapheme_indices(true)
                .any(|(start, _)| start == index)
    }
}
