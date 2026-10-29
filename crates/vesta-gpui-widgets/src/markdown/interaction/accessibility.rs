//! Typed native document projection over the existing revision and snapshots.
use super::super::{MarkdownAnchorError, MarkdownSelection};
use super::{MarkdownAnchor, MarkdownInteraction, MarkdownProjectionError, MarkdownSurface};
use vesta_gpui::{ReadOnlyDocument, ReadOnlyDocumentSelection, ReadOnlyText, ReadOnlyTextPosition};

impl MarkdownInteraction {
    pub fn accessibility<'a>(
        &self,
        surface: &MarkdownSurface<'a>,
    ) -> Result<ReadOnlyDocument<'a>, MarkdownProjectionError> {
        if surface.window != self.target.window {
            return Err(MarkdownAnchorError::WrongWindow.into());
        }
        let document = surface.document;
        let mut paragraphs = Vec::new();
        let visible = document.block_range(surface.range.clone());
        for (index, block) in document
            .blocks
            .iter()
            .enumerate()
            .skip(visible.start)
            .take(visible.end.saturating_sub(visible.start))
        {
            let (snapshot, viewport) =
                document.block_viewport(self.scope, surface.layout, block.id)?;
            paragraphs.push(ReadOnlyText {
                element: document.paragraph_id(self.scope, block.id)?,
                source: &block.content.text,
                snapshot,
                origin: viewport.origin,
                separator_after: Some(document.separator_after(index)),
            });
        }
        let selection = self.selection.and_then(|selection| {
            document.selection_bounds(surface.window, selection).ok()?;
            let position = |anchor: MarkdownAnchor| {
                let element = document.paragraph_id(self.scope, anchor.block).ok()?;
                paragraphs
                    .iter()
                    .any(|text| text.element == element)
                    .then_some(ReadOnlyTextPosition {
                        element,
                        byte: anchor.byte,
                    })
            };
            Some(ReadOnlyDocumentSelection {
                root: self.target.element,
                revision: document.revision().0,
                anchor: position(selection.anchor)?,
                focus: position(selection.focus)?,
            })
        });
        Ok(ReadOnlyDocument {
            root: self.target.element,
            revision: document.revision().0,
            paragraphs,
            selection,
            paragraph_separator: "\n\n",
        })
    }

    /// Validate a tree-resolved range before the Host's focus transition.
    pub fn accessible_selection(
        &self,
        surface: &MarkdownSurface<'_>,
        selection: ReadOnlyDocumentSelection,
    ) -> Result<MarkdownSelection, MarkdownProjectionError> {
        if surface.window != self.target.window {
            return Err(MarkdownAnchorError::WrongWindow.into());
        }
        if selection.root != self.target.element {
            return Err(MarkdownAnchorError::WrongDocument.into());
        }
        if selection.revision != surface.document.revision().0 {
            return Err(MarkdownAnchorError::StaleRevision.into());
        }
        self.accessibility(surface)?;
        let position = |position: ReadOnlyTextPosition| {
            let block = surface
                .document
                .blocks_in(surface.range.clone())
                .find(|block| self.scope.element(block.id.0 + 2) == position.element)
                .ok_or(MarkdownAnchorError::UnknownBlock)?;
            surface
                .document
                .block_viewport(self.scope, surface.layout, block.id)?;
            Ok::<_, MarkdownProjectionError>(surface.document.anchor(block.id, position.byte)?)
        };
        Ok(surface.document.selection(
            surface.window,
            position(selection.anchor)?,
            position(selection.focus)?,
        )?)
    }

    pub fn accept_accessible_selection(
        &mut self,
        surface: &MarkdownSurface<'_>,
        selection: ReadOnlyDocumentSelection,
        input: &mut vesta_gpui::InputOwnership,
    ) -> Result<(), MarkdownProjectionError> {
        let accepted = self.accessible_selection(surface, selection)?;
        self.cancel(input);
        self.selection = Some(accepted);
        self.navigation = None;
        input.set_focus(Some(self.target));
        Ok(())
    }
}
