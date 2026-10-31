use unicode_segmentation::UnicodeSegmentation;
use vesta_gpui::{
    Element, ElementKind, ElementScope, LayoutTree, Point, Rect, StyledTextError,
    TextInputSnapshot, TextInputViewport, TextLayoutCursor, UiWindowId, text_input_wrap_width,
};

use super::{
    MarkdownAnchor, MarkdownAnchorError, MarkdownBlockId, MarkdownLinkIntent, MarkdownSelection,
    MarkdownView,
};
use crate::WidgetTheme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownProjectionError {
    Anchor(MarkdownAnchorError),
    UnshapedBlock(MarkdownBlockId),
    MissingLayout(MarkdownBlockId),
    StaleWidth(MarkdownBlockId),
    Geometry(StyledTextError),
    Accessibility(vesta_gpui::ReadOnlyTextProjectionError),
}

impl std::fmt::Display for MarkdownProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Markdown projection: {self:?}")
    }
}

impl std::error::Error for MarkdownProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Anchor(error) => Some(error),
            Self::Geometry(error) => Some(error),
            Self::Accessibility(error) => Some(error),
            Self::UnshapedBlock(_) | Self::MissingLayout(_) | Self::StaleWidth(_) => None,
        }
    }
}
impl From<MarkdownAnchorError> for MarkdownProjectionError {
    fn from(error: MarkdownAnchorError) -> Self {
        Self::Anchor(error)
    }
}

impl MarkdownView {
    /// Explicit caller-owned selection over the same glyph snapshots as view.
    /// Input, focus, gestures, keyboard policy and clipboard remain host-owned.
    pub fn selected_view(
        &self,
        scope: ElementScope,
        window: UiWindowId,
        range: std::ops::Range<usize>,
        selection: MarkdownSelection,
        theme: WidgetTheme,
    ) -> Result<Element, MarkdownProjectionError> {
        let (start, end) = self.selection_bounds(window, selection)?;
        let mut root = self.view(scope, range.clone(), theme);
        let visible = self.block_range(range);
        for (index, block) in self
            .blocks
            .iter()
            .enumerate()
            .skip(visible.start)
            .take(visible.end.saturating_sub(visible.start))
        {
            if index < start.0 || index > end.0 {
                continue;
            }
            let text = &block.content.text;
            let from = if index == start.0 { start.1 } else { 0 };
            let to = if index == end.0 { end.1 } else { text.len() };
            let snapshot = block
                .shaped
                .as_ref()
                .ok_or(MarkdownProjectionError::UnshapedBlock(block.id))?
                .3
                .clone()
                .with_read_only_selection(from..to)
                .map_err(MarkdownProjectionError::Geometry)?;
            let body = super::table::find_mut(&mut root, scope.element(block.id.0 + 2))
                .ok_or(MarkdownProjectionError::MissingLayout(block.id))?;
            body.kind = ElementKind::TextInput {
                snapshot,
                focused: false,
            };
        }
        Ok(root)
    }

    /// A clipped host point maps to a complete grapheme in the current block.
    pub fn anchor_at_point(
        &self,
        scope: ElementScope,
        layout: &LayoutTree,
        block: MarkdownBlockId,
        point: Point,
    ) -> Result<Option<MarkdownAnchor>, MarkdownProjectionError> {
        Ok(self
            .cursor_at_point(scope, layout, block, point)?
            .map(|(anchor, _)| anchor))
    }

    pub(super) fn cursor_at_point(
        &self,
        scope: ElementScope,
        layout: &LayoutTree,
        block: MarkdownBlockId,
        point: Point,
    ) -> Result<Option<(MarkdownAnchor, TextLayoutCursor)>, MarkdownProjectionError> {
        let (snapshot, viewport) = self.block_viewport(scope, layout, block)?;
        if !viewport.clip.contains(point) {
            return Ok(None);
        }
        let Some(cursor) = snapshot.layout_cursor_at(viewport.to_layout(point)) else {
            return Ok(None);
        };
        Ok(Some(self.snap_cursor(block, cursor)?))
    }

    pub(super) fn snap_cursor(
        &self,
        block: MarkdownBlockId,
        cursor: TextLayoutCursor,
    ) -> Result<(MarkdownAnchor, TextLayoutCursor), MarkdownProjectionError> {
        let byte = cursor.byte_index();
        let text = &self.blocks[self.block_index(block)?].content.text;
        let snapped = text
            .grapheme_indices(true)
            .find_map(|(start, grapheme)| {
                let end = start + grapheme.len();
                (start < byte && byte < end).then(|| {
                    if byte - start <= end - byte {
                        start
                    } else {
                        end
                    }
                })
            })
            .unwrap_or(byte);
        Ok((
            self.anchor(block, snapped)?,
            cursor
                .at_byte_index(snapped)
                .map_err(MarkdownProjectionError::Geometry)?,
        ))
    }

    /// Exact visible link fragments, including wrapping and bidirectional runs.
    /// No rectangular union may make unrelated text between fragments clickable.
    pub fn link_rectangles(
        &self,
        scope: ElementScope,
        window: UiWindowId,
        layout: &LayoutTree,
        intent: MarkdownLinkIntent,
    ) -> Result<Vec<Rect>, MarkdownProjectionError> {
        self.resolve_link(window, intent)?;
        let (snapshot, viewport) = self.block_viewport(scope, layout, intent.block)?;
        Ok(snapshot
            .range_rectangles(intent.start..intent.end)
            .map_err(MarkdownProjectionError::Geometry)?
            .into_iter()
            .filter_map(|rect| {
                Rect::new(
                    rect.x + viewport.origin.x,
                    rect.y + viewport.origin.y,
                    rect.width,
                    rect.height,
                )
                .intersection(viewport.clip)
            })
            .collect())
    }

    pub fn link_at_point(
        &self,
        scope: ElementScope,
        window: UiWindowId,
        layout: &LayoutTree,
        range: std::ops::Range<usize>,
        point: Point,
    ) -> Result<Option<MarkdownLinkIntent>, MarkdownProjectionError> {
        for intent in self.links_in(window, range) {
            if self
                .link_rectangles(scope, window, layout, intent)?
                .iter()
                .any(|rect| rect.contains(point))
            {
                return Ok(Some(intent));
            }
        }
        Ok(None)
    }

    pub(super) fn block_viewport(
        &self,
        scope: ElementScope,
        layout: &LayoutTree,
        block: MarkdownBlockId,
    ) -> Result<(&TextInputSnapshot, TextInputViewport), MarkdownProjectionError> {
        let index = self.block_index(block)?;
        let node = layout
            .node(scope.element(block.0 + 2))
            .ok_or(MarkdownProjectionError::MissingLayout(block))?;
        let (width, _, _, snapshot) = self.blocks[index]
            .shaped
            .as_ref()
            .ok_or(MarkdownProjectionError::UnshapedBlock(block))?;
        if *width != text_input_wrap_width(node.rect) {
            return Err(MarkdownProjectionError::StaleWidth(block));
        }
        Ok((
            snapshot,
            snapshot.viewport(node.rect, node.inherited_clip, Point::default()),
        ))
    }
}
