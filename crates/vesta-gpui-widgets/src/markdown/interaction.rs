//! Read-only gestures over revision-scoped anchors and the host's input owner.

use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;
use vesta_gpui::{
    Element, ElementScope, InputOwnership, InputTarget, KeyChord, KeyStroke, LayoutTree, Point,
    PointerButton, Rect, Semantic, SemanticRole, TextLayoutCursor, UiWindowId,
};

use super::{
    MarkdownAnchor, MarkdownBlockId, MarkdownDocumentId, MarkdownLink, MarkdownLinkIntent,
    MarkdownProjectionError, MarkdownRevision, MarkdownSelection, MarkdownView,
};
use crate::WidgetTheme;
mod accessibility;
mod drag_scroll;
#[path = "interaction/navigation.rs"]
mod navigation;
use navigation::VerticalNavigation;

pub struct MarkdownSurface<'a> {
    pub window: UiWindowId,
    pub document: &'a MarkdownView,
    pub layout: &'a LayoutTree,
    pub range: Range<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownPointerResult {
    Ignored,
    Selection,
    Activate(MarkdownLinkIntent),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkdownKeyIntent {
    Handled,
    /// Navigation changed the logical focus; reveal its current unclipped geometry.
    RevealSelection,
    Copy(String),
    Activate(MarkdownLinkIntent),
}

struct Gesture {
    origin: Point,
    pointer: Point,
    before: Option<MarkdownSelection>,
    anchor: MarkdownAnchor,
    pressed_link: Option<MarkdownLinkIntent>,
    dragged: bool,
    scroll: Option<drag_scroll::DragScroll>,
    scroll_stopped: bool,
}

/// One projection in one window; text and glyphs remain in MarkdownView.
pub struct MarkdownInteraction {
    target: InputTarget,
    scope: ElementScope,
    binding: Option<(MarkdownDocumentId, MarkdownRevision, Range<usize>)>,
    selection: Option<MarkdownSelection>,
    gesture: Option<Gesture>,
    navigation: Option<VerticalNavigation>,
}

impl MarkdownInteraction {
    pub fn new(window: UiWindowId, scope: ElementScope) -> Self {
        Self {
            target: InputTarget {
                window,
                element: scope.element(0),
            },
            scope,
            binding: None,
            selection: None,
            gesture: None,
            navigation: None,
        }
    }

    pub fn target(&self) -> InputTarget {
        self.target
    }
    pub fn selection(&self) -> Option<MarkdownSelection> {
        self.selection
    }
    pub fn is_dragging(&self) -> bool {
        self.gesture.is_some()
    }

    /// Opt in to the read-only document and independent link focus targets.
    /// Ordinary MarkdownView::view stays nonfocusable.
    pub fn view(
        &self,
        document: &MarkdownView,
        range: Range<usize>,
        theme: WidgetTheme,
    ) -> Element {
        let base = || document.view(self.scope, range.clone(), theme);
        let mut root = self
            .selection
            .and_then(|selection| {
                document
                    .selected_view(
                        self.scope,
                        self.target.window,
                        range.clone(),
                        selection,
                        theme,
                    )
                    .ok()
            })
            .unwrap_or_else(base);
        root.style.focusable = true;
        root.semantic = Some(Semantic::new(SemanticRole::Document, "Markdown").read_only(true));
        document.attach_link_semantics(&mut root, self.scope, range);
        root
    }

    /// Rebuilt pages retain completed selection in the same revision, but never
    /// retain a pressed link or gesture across page, focus or capture changes.
    pub fn sync(&mut self, surface: &MarkdownSurface<'_>, input: &mut InputOwnership) -> bool {
        if surface.window != self.target.window {
            return false;
        }
        let next = surface
            .document
            .document_id()
            .map(|id| (id, surface.document.revision(), surface.range.clone()));
        let changed_document = match (&self.binding, &next) {
            (Some(old), Some(new)) => old.0 != new.0 || old.1 != new.1,
            (None, None) => false,
            _ => true,
        };
        let changed_page = self.binding != next;
        let missing = surface.layout.node(self.target.element).is_none();
        let lost_owner = input.focus() != Some(self.target)
            || self.gesture.is_some() && !self.owns_capture(input);
        let mut changed = false;
        if missing || lost_owner || changed_document || changed_page && self.gesture.is_some() {
            changed |= self.cancel(input);
        }
        if changed_document {
            changed |= self.selection.take().is_some();
        }
        self.binding = next;
        changed
    }

    /// Restore only this transient drag; never clear another control's capture.
    pub fn cancel(&mut self, input: &mut InputOwnership) -> bool {
        self.navigation = None;
        let Some(gesture) = self.gesture.take() else {
            return false;
        };
        self.selection = gesture.before;
        if self.owns_capture(input) {
            input.release_pointer(self.target.window, PointerButton::Primary);
        }
        true
    }

    pub fn press(
        &mut self,
        surface: &MarkdownSurface<'_>,
        point: Point,
        extend: bool,
        input: &mut InputOwnership,
    ) -> Result<MarkdownPointerResult, MarkdownProjectionError> {
        if surface.window != self.target.window {
            return Ok(MarkdownPointerResult::Ignored);
        }
        self.sync(surface, input);
        if input.capture().is_some() {
            return Ok(MarkdownPointerResult::Ignored);
        }
        let Some((focus, cursor)) = self.point_cursor(surface, point, false)? else {
            return Ok(MarkdownPointerResult::Ignored);
        };
        let before = self.selection;
        let anchor = if extend {
            before.map_or(focus, |selection| selection.anchor)
        } else {
            focus
        };
        let selection = surface
            .document
            .selection(self.target.window, anchor, focus)?;
        let pressed_link = if extend {
            None
        } else {
            surface.document.link_at_point(
                self.scope,
                surface.window,
                surface.layout,
                surface.range.clone(),
                point,
            )?
        };
        input.set_focus(Some(self.target));
        input.capture_pointer(self.target, PointerButton::Primary);
        self.selection = Some(selection);
        self.navigation = Some(VerticalNavigation::from_pointer(focus, cursor));
        self.gesture = Some(Gesture {
            origin: point,
            pointer: point,
            before,
            anchor,
            pressed_link,
            dragged: extend,
            scroll: None,
            scroll_stopped: false,
        });
        Ok(MarkdownPointerResult::Selection)
    }

    pub fn pointer_move(
        &mut self,
        surface: &MarkdownSurface<'_>,
        point: Point,
        input: &mut InputOwnership,
    ) -> Result<MarkdownPointerResult, MarkdownProjectionError> {
        if surface.window != self.target.window {
            return Ok(MarkdownPointerResult::Ignored);
        }
        self.sync(surface, input);
        let Some(anchor) = self.gesture.as_ref().map(|gesture| gesture.anchor) else {
            return Ok(MarkdownPointerResult::Ignored);
        };
        let (focus, cursor) = match self.point_cursor(surface, point, true) {
            Ok(Some(position)) => position,
            Ok(None) => return Ok(MarkdownPointerResult::Selection),
            Err(error) => {
                self.cancel(input);
                return Err(error);
            }
        };
        self.selection = Some(surface.document.selection(surface.window, anchor, focus)?);
        self.navigation = Some(VerticalNavigation::from_pointer(focus, cursor));
        if let Some(gesture) = &mut self.gesture {
            if gesture.pointer != point {
                gesture.scroll_stopped = false;
            }
            gesture.pointer = point;
            let dx = point.x - gesture.origin.x;
            let dy = point.y - gesture.origin.y;
            gesture.dragged |= dx * dx + dy * dy >= 9.0 || focus != anchor;
        }
        Ok(MarkdownPointerResult::Selection)
    }

    pub fn release(
        &mut self,
        surface: &MarkdownSurface<'_>,
        point: Point,
        button: PointerButton,
        input: &mut InputOwnership,
    ) -> Result<MarkdownPointerResult, MarkdownProjectionError> {
        if surface.window != self.target.window || button != PointerButton::Primary {
            return Ok(MarkdownPointerResult::Ignored);
        }
        self.pointer_move(surface, point, input)?;
        let Some(gesture) = self.gesture.take() else {
            return Ok(MarkdownPointerResult::Ignored);
        };
        input.release_pointer(surface.window, button);
        if !gesture.dragged
            && let Some(link) = gesture.pressed_link
            && surface.document.link_at_point(
                self.scope,
                surface.window,
                surface.layout,
                surface.range.clone(),
                point,
            )? == Some(link)
        {
            return Ok(MarkdownPointerResult::Activate(link));
        }
        Ok(MarkdownPointerResult::Selection)
    }

    pub fn key(
        &mut self,
        surface: &MarkdownSurface<'_>,
        chord: KeyChord,
        copy_limit: usize,
        input: &mut InputOwnership,
    ) -> Result<Option<MarkdownKeyIntent>, MarkdownProjectionError> {
        if surface.window != self.target.window {
            return Ok(None);
        }
        self.sync(surface, input);
        if surface.layout.node(self.target.element).is_none() || chord.modifiers.alt {
            return Ok(None);
        }
        let command = chord.modifiers.control || chord.modifiers.super_key;
        if input.focus() != Some(self.target) {
            if !command
                && !chord.modifiers.shift
                && matches!(chord.key, KeyStroke::Enter | KeyStroke::Space)
                && let Some(target) = input.focus()
                && let Some(intent) = self.activate_target(surface, target)?
            {
                return Ok(Some(MarkdownKeyIntent::Activate(intent)));
            }
            return Ok(None);
        }
        match chord.key {
            KeyStroke::Character('a') if command && !chord.modifiers.shift => {
                self.cancel(input);
                self.selection = surface.document.select_all(surface.window)?;
            }
            KeyStroke::Character('c' | 'x') if command && !chord.modifiers.shift => {
                if let Some(selection) = self.selection {
                    let text =
                        surface
                            .document
                            .copy_selection(surface.window, selection, copy_limit)?;
                    if !text.is_empty() {
                        return Ok(Some(MarkdownKeyIntent::Copy(text)));
                    }
                }
            }
            KeyStroke::Escape if !command && !chord.modifiers.shift => {
                if !self.cancel(input) && self.selection.take().is_none() {
                    return Ok(None);
                }
            }
            KeyStroke::ArrowLeft | KeyStroke::ArrowRight if !command => {
                self.cancel(input);
                self.move_selection(surface.document, chord)?;
                return Ok(Some(MarkdownKeyIntent::RevealSelection));
            }
            KeyStroke::ArrowUp | KeyStroke::ArrowDown | KeyStroke::PageUp | KeyStroke::PageDown
                if !command =>
            {
                if self.gesture.is_some() {
                    self.cancel(input);
                }
                return Ok(self
                    .move_vertical(surface, chord)?
                    .then_some(MarkdownKeyIntent::RevealSelection));
            }
            KeyStroke::Home | KeyStroke::End => {
                self.cancel(input);
                self.move_selection(surface.document, chord)?;
                return Ok(Some(MarkdownKeyIntent::RevealSelection));
            }
            _ => return Ok(None),
        }
        Ok(Some(MarkdownKeyIntent::Handled))
    }

    /// Revalidate a current projected link for keyboard or platform activation.
    pub fn activate_target(
        &self,
        surface: &MarkdownSurface<'_>,
        target: InputTarget,
    ) -> Result<Option<MarkdownLinkIntent>, MarkdownProjectionError> {
        if surface.window != self.target.window
            || target.window != surface.window
            || surface.layout.node(self.target.element).is_none()
            || surface.layout.node(target.element).is_none()
        {
            return Ok(None);
        }
        let Some(intent) = surface.document.intent_for_target(
            self.scope,
            surface.window,
            surface.range.clone(),
            target,
        ) else {
            return Ok(None);
        };
        Ok((!surface
            .document
            .link_rectangles(self.scope, surface.window, surface.layout, intent)?
            .is_empty())
        .then_some(intent))
    }

    pub fn resolve_activation<'a>(
        &self,
        surface: &MarkdownSurface<'a>,
        intent: MarkdownLinkIntent,
    ) -> Result<Option<MarkdownLink<'a>>, MarkdownProjectionError> {
        let target = surface
            .document
            .link_target(self.scope, surface.window, intent)?;
        if self.activate_target(surface, target)? != Some(intent) {
            return Ok(None);
        }
        Ok(Some(surface.document.resolve_link(surface.window, intent)?))
    }

    fn move_selection(
        &mut self,
        document: &MarkdownView,
        chord: KeyChord,
    ) -> Result<(), MarkdownProjectionError> {
        let Some(all) = document.select_all(self.target.window)? else {
            return Ok(());
        };
        let current = self.selection.unwrap_or(document.selection(
            self.target.window,
            all.anchor,
            all.anchor,
        )?);
        let (start, end) = document.selection_bounds(self.target.window, current)?;
        let focus_index = document.block_index(current.focus.block)?;
        let forward = matches!(chord.key, KeyStroke::ArrowRight | KeyStroke::End);
        let command = chord.modifiers.control || chord.modifiers.super_key;
        let (index, byte) = match chord.key {
            KeyStroke::Home if command => (0, 0),
            KeyStroke::End if command => (
                document.blocks.len() - 1,
                document
                    .blocks
                    .last()
                    .map_or(0, |block| block.content.text.len()),
            ),
            KeyStroke::Home => (focus_index, 0),
            KeyStroke::End => (focus_index, document.blocks[focus_index].content.text.len()),
            _ if !chord.modifiers.shift && start != end => {
                if forward {
                    end
                } else {
                    start
                }
            }
            _ => {
                let text = &document.blocks[focus_index].content.text;
                if forward {
                    if let Some((byte, grapheme)) = text
                        .grapheme_indices(true)
                        .find(|(byte, _)| *byte == current.focus.byte)
                    {
                        (focus_index, byte + grapheme.len())
                    } else if focus_index + 1 < document.blocks.len() {
                        (focus_index + 1, 0)
                    } else {
                        (focus_index, text.len())
                    }
                } else if let Some((byte, _)) = text
                    .grapheme_indices(true)
                    .take_while(|(byte, _)| *byte < current.focus.byte)
                    .last()
                {
                    (focus_index, byte)
                } else if focus_index > 0 {
                    (
                        focus_index - 1,
                        document.blocks[focus_index - 1].content.text.len(),
                    )
                } else {
                    (0, 0)
                }
            }
        };
        let focus = document.anchor(document.blocks[index].id, byte)?;
        self.selection = Some(document.selection(
            self.target.window,
            if chord.modifiers.shift {
                current.anchor
            } else {
                focus
            },
            focus,
        )?);
        Ok(())
    }

    fn owns_capture(&self, input: &InputOwnership) -> bool {
        input.capture().is_some_and(|capture| {
            capture.target == self.target && capture.button == PointerButton::Primary
        })
    }

    fn point_cursor(
        &self,
        surface: &MarkdownSurface<'_>,
        point: Point,
        clamp: bool,
    ) -> Result<Option<(MarkdownAnchor, TextLayoutCursor)>, MarkdownProjectionError> {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Ok(None);
        }
        let mut nearest = None;
        for block in surface.document.blocks_in(surface.range.clone()) {
            let (_, viewport) =
                surface
                    .document
                    .block_viewport(self.scope, surface.layout, block.id)?;
            let clip = viewport.clip;
            if clip.width <= 0.0 || clip.height <= 0.0 {
                continue;
            }
            if clip.contains(point) {
                return surface.document.cursor_at_point(
                    self.scope,
                    surface.layout,
                    block.id,
                    point,
                );
            }
            if clamp {
                // Stay inside the half-open visible rectangle for point hit testing.
                let inset = 0.01_f32.min(clip.width / 2.0).min(clip.height / 2.0);
                let clipped = Point::new(
                    point.x.clamp(clip.x, clip.x + clip.width - inset),
                    point.y.clamp(clip.y, clip.y + clip.height - inset),
                );
                let dx = point.x - clipped.x;
                let dy = point.y - clipped.y;
                let distance = dx * dx + dy * dy;
                if nearest.is_none_or(|(best, _, _)| distance < best) {
                    nearest = Some((distance, block.id, clipped));
                }
            }
        }
        match nearest {
            Some((_, block, point)) => {
                surface
                    .document
                    .cursor_at_point(self.scope, surface.layout, block, point)
            }
            None => Ok(None),
        }
    }
}
