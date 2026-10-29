//! Search and selection policy; command meaning and text editing remain with the host.
use crate::{ChoiceBoundary, ChoiceKeyAction, ScrollAreaState, choice_key_action};
use vesta_gpui::{KeyChord, KeyStroke};

pub const COMMAND_PALETTE_ROW_HEIGHT: f32 = 34.0;

/// All whitespace-delimited query words must occur in the supplied search metadata.
/// The caller keeps the actual command catalog; this never creates an executable registry.
#[must_use]
pub fn command_palette_matches(query: &str, label: &str, shortcut: &str, keywords: &str) -> bool {
    let text = format!("{label}\n{shortcut}\n{keywords}").to_lowercase();
    query
        .split_whitespace()
        .all(|word| text.contains(&word.to_lowercase()))
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommandPaletteState<K> {
    highlighted: Option<K>,
    scroll: ScrollAreaState,
    viewport_height: f32,
    selected_index: Option<usize>,
}

impl<K> Default for CommandPaletteState<K> {
    fn default() -> Self {
        Self {
            highlighted: None,
            scroll: ScrollAreaState::default(),
            viewport_height: 0.0,
            selected_index: None,
        }
    }
}

impl<K: Clone + Eq> CommandPaletteState<K> {
    #[must_use]
    pub fn highlighted(&self) -> Option<&K> {
        self.highlighted.as_ref()
    }

    #[must_use]
    pub fn scroll(&self) -> ScrollAreaState {
        self.scroll
    }

    /// Keep a surviving enabled identity. A removed/disabled selection resolves to the first
    /// enabled result, never to the old array position. Wheel movement alone does not reveal.
    pub fn reconcile(&mut self, entries: &[(K, bool)], viewport_height: f32) {
        let viewport_height = if viewport_height.is_finite() {
            viewport_height.max(0.0)
        } else {
            0.0
        };
        let next = self
            .highlighted
            .as_ref()
            .filter(|id| entries.iter().any(|(key, enabled)| key == *id && *enabled))
            .cloned()
            .or_else(|| {
                entries
                    .iter()
                    .find(|(_, enabled)| *enabled)
                    .map(|(key, _)| key.clone())
            });
        let index = entries.iter().position(|(id, _)| Some(id) == next.as_ref());
        let changed = self.highlighted != next
            || self.viewport_height != viewport_height
            || self.selected_index != index;
        self.highlighted = next;
        self.viewport_height = viewport_height;
        self.selected_index = index;
        self.scroll.sync_content_height(
            entries.len() as f32 * COMMAND_PALETTE_ROW_HEIGHT,
            viewport_height,
        );
        if changed {
            self.reveal(entries);
        }
    }

    fn reveal(&mut self, entries: &[(K, bool)]) {
        if let Some(index) = entries
            .iter()
            .position(|(id, _)| Some(id) == self.highlighted.as_ref())
        {
            crate::reveal_collection_row(
                &mut self.scroll,
                index,
                COMMAND_PALETTE_ROW_HEIGHT,
                self.viewport_height,
            );
        }
    }

    /// A palette owns its wheel, including movement left over at either boundary.
    pub fn wheel(&mut self, delta: f32) {
        self.scroll.scroll_by(delta);
    }

    pub fn drag_scroll(&mut self, drag: crate::ScrollbarDrag, pointer: vesta_gpui::Point) -> bool {
        drag.update(&mut self.scroll, pointer)
    }

    /// The owner host calls this only after checking the window and text session. Preedit,
    /// modified shortcuts, Space and ordinary text remain with the shared text bridge.
    pub fn key_action(
        &mut self,
        entries: &[(K, bool)],
        chord: KeyChord,
        composing: bool,
        viewport_height: f32,
    ) -> Option<ChoiceKeyAction<K>> {
        if composing || chord.modifiers.control || chord.modifiers.alt || chord.modifiers.super_key
        {
            return None;
        }
        self.reconcile(entries, viewport_height);
        if chord.key == KeyStroke::Tab {
            return Some(ChoiceKeyAction::Consumed);
        }
        if chord.key == KeyStroke::Space
            || matches!(
                chord.key,
                KeyStroke::Character(_) | KeyStroke::ArrowLeft | KeyStroke::ArrowRight
            )
        {
            return None;
        }
        let enabled = entries
            .iter()
            .filter(|(_, yes)| *yes)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        let action = if matches!(chord.key, KeyStroke::PageUp | KeyStroke::PageDown) {
            if enabled.is_empty() {
                ChoiceKeyAction::Consumed
            } else {
                let index = self
                    .highlighted
                    .as_ref()
                    .and_then(|id| enabled.iter().position(|key| key == id))
                    .unwrap_or(0);
                let step = (self.viewport_height / COMMAND_PALETTE_ROW_HEIGHT)
                    .floor()
                    .max(1.0) as usize;
                let index = if chord.key == KeyStroke::PageDown {
                    index.saturating_add(step).min(enabled.len() - 1)
                } else {
                    index.saturating_sub(step)
                };
                ChoiceKeyAction::Highlight(enabled[index].clone())
            }
        } else {
            choice_key_action(
                &enabled,
                self.highlighted.as_ref(),
                chord,
                ChoiceBoundary::Wrap,
            )?
        };
        if let ChoiceKeyAction::Highlight(id) = &action {
            self.highlighted = Some(id.clone());
            self.selected_index = entries.iter().position(|(key, _)| key == id);
            self.reveal(entries);
        }
        Some(action)
    }
}
