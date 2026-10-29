use std::sync::atomic::{AtomicU32, Ordering};

use unicode_segmentation::UnicodeSegmentation;
use vesta_gpui::{ElementId, ElementIdAllocator, ElementScope, UiWindowId};

use super::{MarkdownError, MarkdownView};

// Resolved block order and byte offsets within the current document revision.
type SelectionBounds = ((usize, usize), (usize, usize));

// Local paragraph slots are globally minted, never inferred from array order.
// A host scope can project the same document in another window independently.
static NEXT_BLOCK: AtomicU32 = AtomicU32::new(10_000);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MarkdownDocumentId(ElementId);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkdownRevision(pub(super) u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MarkdownBlockId(pub(super) u32);

impl MarkdownDocumentId {
    pub(super) fn allocate() -> Result<Self, MarkdownError> {
        ElementIdAllocator
            .allocate_scope()
            .map(|scope| Self(scope.element(0)))
            .map_err(|_| MarkdownError::IdentityExhausted)
    }

    pub(super) fn key(self) -> u64 {
        self.0.get()
    }
}

impl MarkdownBlockId {
    pub(super) fn allocate() -> Result<Self, MarkdownError> {
        // Block, marker, text, quote rule, table row and table group.
        allocate_slots(6).map(Self)
    }
}

pub(super) fn allocate_slots(count: u32) -> Result<u32, MarkdownError> {
    NEXT_BLOCK
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
            next.checked_add(count)
        })
        .map_err(|_| MarkdownError::IdentityExhausted)
}

/// Byte offsets are valid only inside this document revision and stable block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkdownAnchor {
    pub(super) document: MarkdownDocumentId,
    pub(super) revision: MarkdownRevision,
    pub(super) block: MarkdownBlockId,
    pub(super) byte: usize,
}

impl MarkdownAnchor {
    pub const fn block(self) -> MarkdownBlockId {
        self.block
    }
    pub const fn byte(self) -> usize {
        self.byte
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkdownSelection {
    pub(super) window: UiWindowId,
    pub(super) anchor: MarkdownAnchor,
    pub(super) focus: MarkdownAnchor,
}

/// An inert host intent. Resolving it never opens a URL or reads a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkdownLinkIntent {
    pub(super) window: UiWindowId,
    pub(super) document: MarkdownDocumentId,
    pub(super) revision: MarkdownRevision,
    pub(super) block: MarkdownBlockId,
    pub(super) start: usize,
    pub(super) end: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownAnchorError {
    DocumentUnavailable,
    WrongWindow,
    WrongDocument,
    StaleRevision,
    UnknownBlock,
    InvalidBoundary,
    UnknownLink,
    CopyLimit { limit: usize },
}

impl std::fmt::Display for MarkdownAnchorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Markdown anchor: {self:?}")
    }
}

impl std::error::Error for MarkdownAnchorError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkdownLink<'a> {
    pub label: &'a str,
    pub destination: &'a str,
}

impl MarkdownView {
    pub fn document_id(&self) -> Option<MarkdownDocumentId> {
        self.identity
    }
    pub fn revision(&self) -> MarkdownRevision {
        MarkdownRevision(self.revision)
    }
    pub fn block_id_at(&self, index: usize) -> Option<MarkdownBlockId> {
        self.blocks_in(index..index.saturating_add(1))
            .next()
            .map(|block| block.id)
    }

    /// Resolve stable block identity, rather than a page's local row position.
    pub fn paragraph_id(
        &self,
        scope: ElementScope,
        block: MarkdownBlockId,
    ) -> Result<ElementId, MarkdownAnchorError> {
        self.block_index(block)?;
        Ok(scope.element(block.0 + 2))
    }

    pub fn anchor(
        &self,
        block: MarkdownBlockId,
        byte: usize,
    ) -> Result<MarkdownAnchor, MarkdownAnchorError> {
        let document = self
            .identity
            .ok_or(MarkdownAnchorError::DocumentUnavailable)?;
        let text = &self.blocks[self.block_index(block)?].content.text;
        if byte != text.len() && !text.grapheme_indices(true).any(|(start, _)| start == byte) {
            return Err(MarkdownAnchorError::InvalidBoundary);
        }
        Ok(MarkdownAnchor {
            document,
            revision: self.revision(),
            block,
            byte,
        })
    }

    pub fn selection(
        &self,
        window: UiWindowId,
        anchor: MarkdownAnchor,
        focus: MarkdownAnchor,
    ) -> Result<MarkdownSelection, MarkdownAnchorError> {
        self.validate_anchor(anchor)?;
        self.validate_anchor(focus)?;
        Ok(MarkdownSelection {
            window,
            anchor,
            focus,
        })
    }

    /// Current logical display row containing the validated selection's focus.
    /// A table's cells share one row. Hosts can page to an off-page selection
    /// before querying its shaped geometry; this index is not a stable identity.
    pub fn selection_focus_row(
        &self,
        window: UiWindowId,
        selection: MarkdownSelection,
    ) -> Result<usize, MarkdownAnchorError> {
        self.selection_bounds(window, selection)?;
        self.row_index(selection.focus.block)
    }

    /// Entire rendered document, including blocks outside the current page.
    /// The empty, initialized document has no selectable range.
    pub fn select_all(
        &self,
        window: UiWindowId,
    ) -> Result<Option<MarkdownSelection>, MarkdownAnchorError> {
        if self.identity.is_none() {
            return Err(MarkdownAnchorError::DocumentUnavailable);
        }
        let Some(first) = self.blocks.first() else {
            return Ok(None);
        };
        let last = self
            .blocks
            .last()
            .ok_or(MarkdownAnchorError::UnknownBlock)?;
        self.selection(
            window,
            self.anchor(first.id, 0)?,
            self.anchor(last.id, last.content.text.len())?,
        )
        .map(Some)
    }

    /// Plain rendered text in document order, bounded before any allocation.
    /// Empty selections copy an empty string; blocks are separated by two LF.
    pub fn copy_selection(
        &self,
        window: UiWindowId,
        selection: MarkdownSelection,
        limit: usize,
    ) -> Result<String, MarkdownAnchorError> {
        let (start, end) = self.selection_bounds(window, selection)?;
        let slices = || {
            (start.0..=end.0).map(|index| {
                let text = &self.blocks[index].content.text;
                &text[if index == start.0 { start.1 } else { 0 }..if index == end.0 {
                    end.1
                } else {
                    text.len()
                }]
            })
        };
        let mut bytes = 0_usize;
        for (index, slice) in slices().enumerate() {
            bytes = bytes
                .checked_add(if index > 0 {
                    self.separator_after(start.0 + index - 1).len()
                } else {
                    0
                })
                .and_then(|bytes| bytes.checked_add(slice.len()))
                .filter(|bytes| *bytes <= limit)
                .ok_or(MarkdownAnchorError::CopyLimit { limit })?;
        }
        let mut copy = String::with_capacity(bytes);
        for (index, slice) in slices().enumerate() {
            if index > 0 {
                copy.push_str(self.separator_after(start.0 + index - 1));
            }
            copy.push_str(slice);
        }
        Ok(copy)
    }

    pub fn links_in(
        &self,
        window: UiWindowId,
        range: std::ops::Range<usize>,
    ) -> impl Iterator<Item = MarkdownLinkIntent> + '_ {
        self.identity.into_iter().flat_map(move |document| {
            self.blocks_in(range.clone()).flat_map(move |block| {
                block
                    .content
                    .links
                    .iter()
                    .map(move |link| MarkdownLinkIntent {
                        window,
                        document,
                        revision: self.revision(),
                        block: block.id,
                        start: link.range.start,
                        end: link.range.end,
                    })
            })
        })
    }

    pub fn resolve_link(
        &self,
        window: UiWindowId,
        intent: MarkdownLinkIntent,
    ) -> Result<MarkdownLink<'_>, MarkdownAnchorError> {
        if intent.window != window {
            return Err(MarkdownAnchorError::WrongWindow);
        }
        self.validate_revision(intent.document, intent.revision)?;
        let block = &self.blocks[self.block_index(intent.block)?];
        let link = block
            .content
            .links
            .iter()
            .find(|link| link.range.start == intent.start && link.range.end == intent.end)
            .ok_or(MarkdownAnchorError::UnknownLink)?;
        Ok(MarkdownLink {
            label: &block.content.text[link.range.clone()],
            destination: &link.destination,
        })
    }

    pub(super) fn block_index(&self, id: MarkdownBlockId) -> Result<usize, MarkdownAnchorError> {
        self.blocks
            .iter()
            .position(|block| block.id == id)
            .ok_or(MarkdownAnchorError::UnknownBlock)
    }

    fn validate_revision(
        &self,
        document: MarkdownDocumentId,
        revision: MarkdownRevision,
    ) -> Result<(), MarkdownAnchorError> {
        if self.identity.is_none() {
            return Err(MarkdownAnchorError::DocumentUnavailable);
        }
        if self.identity != Some(document) {
            return Err(MarkdownAnchorError::WrongDocument);
        }
        if self.revision() != revision {
            return Err(MarkdownAnchorError::StaleRevision);
        }
        Ok(())
    }

    fn validate_anchor(&self, anchor: MarkdownAnchor) -> Result<usize, MarkdownAnchorError> {
        self.validate_revision(anchor.document, anchor.revision)?;
        self.anchor(anchor.block, anchor.byte)?;
        self.block_index(anchor.block)
    }

    pub(super) fn selection_bounds(
        &self,
        window: UiWindowId,
        selection: MarkdownSelection,
    ) -> Result<SelectionBounds, MarkdownAnchorError> {
        if selection.window != window {
            return Err(MarkdownAnchorError::WrongWindow);
        }
        let anchor = (
            self.validate_anchor(selection.anchor)?,
            selection.anchor.byte,
        );
        let focus = (self.validate_anchor(selection.focus)?, selection.focus.byte);
        Ok(if anchor <= focus {
            (anchor, focus)
        } else {
            (focus, anchor)
        })
    }
}
