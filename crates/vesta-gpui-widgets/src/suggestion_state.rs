//! Caller-driven query lifecycle. Text, providers and accepted values stay with the host.
use std::collections::BTreeSet;

use crate::{ChoiceBoundary, ChoiceKeyAction, ScrollAreaState, choice_key_action};
use vesta_gpui::{InputTarget, KeyChord, KeyStroke};

pub const SUGGESTION_ROW_HEIGHT: f32 = 28.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SuggestionRequestId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuggestionRequest {
    pub owner: InputTarget,
    pub id: SuggestionRequestId,
    pub query: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuggestionOption<K> {
    pub id: K,
    pub label: String,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SuggestionPhase {
    #[default]
    Closed,
    Loading,
    Ready,
    Empty,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuggestionError {
    Composing,
    ReusedRequestId,
    DuplicateCandidate,
}

impl std::fmt::Display for SuggestionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Composing => "suggestion request is unavailable during text composition",
            Self::ReusedRequestId => "suggestion request identity was already used",
            Self::DuplicateCandidate => "duplicate suggestion candidate identity",
        })
    }
}
impl std::error::Error for SuggestionError {}

#[derive(Clone, Debug, PartialEq)]
pub struct SuggestionState<K> {
    request: Option<SuggestionRequest>,
    last_id: u64,
    phase: SuggestionPhase,
    options: Vec<SuggestionOption<K>>,
    highlighted: Option<K>,
    failure: Option<String>,
    scroll: ScrollAreaState,
    viewport_height: f32,
}

impl<K> Default for SuggestionState<K> {
    fn default() -> Self {
        Self {
            request: None,
            last_id: 0,
            phase: SuggestionPhase::Closed,
            options: Vec::new(),
            highlighted: None,
            failure: None,
            scroll: ScrollAreaState::default(),
            viewport_height: 140.0,
        }
    }
}

impl<K: Clone + Ord> SuggestionState<K> {
    #[must_use]
    pub fn request(&self) -> Option<&SuggestionRequest> {
        self.request.as_ref()
    }
    #[must_use]
    pub fn phase(&self) -> SuggestionPhase {
        self.phase
    }
    #[must_use]
    pub fn options(&self) -> &[SuggestionOption<K>] {
        &self.options
    }
    #[must_use]
    pub fn highlighted(&self) -> Option<&K> {
        self.highlighted.as_ref()
    }
    #[must_use]
    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }
    #[must_use]
    pub fn scroll(&self) -> ScrollAreaState {
        self.scroll
    }

    /// IDs increase across cancellation and owner changes. Provider I/O is external.
    pub fn begin(
        &mut self,
        owner: InputTarget,
        id: SuggestionRequestId,
        query: String,
        composing: bool,
    ) -> Result<SuggestionRequest, SuggestionError> {
        if composing {
            return Err(SuggestionError::Composing);
        }
        if id.0 <= self.last_id {
            return Err(SuggestionError::ReusedRequestId);
        }
        if self.request.as_ref().is_none_or(|r| r.owner != owner) {
            self.highlighted = None;
        }
        let request = SuggestionRequest { owner, id, query };
        self.last_id = id.0;
        self.request = Some(request.clone());
        self.phase = SuggestionPhase::Loading;
        self.options.clear();
        self.failure = None;
        self.scroll = ScrollAreaState::default();
        Ok(request)
    }

    /// A stale/duplicate-terminal response returns false. Invalid data never partially applies.
    pub fn resolve(
        &mut self,
        request: &SuggestionRequest,
        result: Result<Vec<SuggestionOption<K>>, String>,
    ) -> Result<bool, SuggestionError> {
        if self.request.as_ref() != Some(request) || self.phase != SuggestionPhase::Loading {
            return Ok(false);
        }
        match result {
            Ok(options) => {
                let mut seen = BTreeSet::new();
                if options.iter().any(|item| !seen.insert(&item.id)) {
                    return Err(SuggestionError::DuplicateCandidate);
                }
                self.phase = if options.is_empty() {
                    SuggestionPhase::Empty
                } else {
                    SuggestionPhase::Ready
                };
                self.options = options;
                self.highlighted = self
                    .highlighted
                    .as_ref()
                    .filter(|id| self.options.iter().any(|o| &o.id == *id && o.enabled))
                    .cloned()
                    .or_else(|| {
                        self.options
                            .iter()
                            .find(|o| o.enabled)
                            .map(|o| o.id.clone())
                    });
                self.sync_viewport(self.viewport_height);
                self.reveal();
            }
            Err(message) => {
                self.phase = SuggestionPhase::Failed;
                self.failure = Some(message);
                self.highlighted = None;
            }
        }
        Ok(true)
    }

    pub fn cancel(&mut self, owner: InputTarget) -> bool {
        if self.request.as_ref().is_none_or(|r| r.owner != owner) {
            return false;
        }
        self.request = None;
        self.phase = SuggestionPhase::Closed;
        self.options.clear();
        self.highlighted = None;
        self.failure = None;
        self.scroll = ScrollAreaState::default();
        true
    }

    /// Fresh pointer and keyboard acceptance use the same exact ticket and retire it once.
    pub fn accept(
        &mut self,
        owner: InputTarget,
        request: &SuggestionRequest,
        id: &K,
        composing: bool,
    ) -> Option<SuggestionOption<K>> {
        if composing
            || self.phase != SuggestionPhase::Ready
            || request.owner != owner
            || self.request.as_ref() != Some(request)
        {
            return None;
        }
        let value = self
            .options
            .iter()
            .find(|o| &o.id == id && o.enabled)?
            .clone();
        self.cancel(owner);
        Some(value)
    }

    pub fn sync_viewport(&mut self, height: f32) {
        let next = if height.is_finite() {
            height.clamp(1.0, 4096.0)
        } else {
            140.0
        };
        let changed = next != self.viewport_height;
        self.viewport_height = next;
        self.scroll.sync_content_height(
            self.options.len() as f32 * SUGGESTION_ROW_HEIGHT,
            self.viewport_height,
        );
        if changed {
            self.reveal();
        }
    }

    fn reveal(&mut self) {
        if let Some(index) = self
            .options
            .iter()
            .position(|o| Some(&o.id) == self.highlighted.as_ref())
        {
            crate::reveal_collection_row(
                &mut self.scroll,
                index,
                SUGGESTION_ROW_HEIGHT,
                self.viewport_height,
            );
        }
    }

    pub fn wheel(&mut self, delta: f32) {
        self.scroll.scroll_by(delta);
    }

    pub fn drag_scroll(&mut self, drag: crate::ScrollbarDrag, point: vesta_gpui::Point) -> bool {
        drag.update(&mut self.scroll, point)
    }

    /// Home/End remain ordinary caret movement; Alt+Home/End select first/last candidate.
    /// Tab traverses without accepting. Preedit and editing shortcuts are never intercepted.
    pub fn key_action(
        &mut self,
        owner: InputTarget,
        mut chord: KeyChord,
        composing: bool,
    ) -> Option<ChoiceKeyAction<K>> {
        if composing
            || self.request.as_ref().is_none_or(|r| r.owner != owner)
            || chord.modifiers.control
            || chord.modifiers.super_key
        {
            return None;
        }
        if matches!(chord.key, KeyStroke::Home | KeyStroke::End) {
            if !chord.modifiers.alt {
                return None;
            }
            chord.modifiers.alt = false;
        } else if chord.modifiers.alt {
            return None;
        }
        if matches!(
            chord.key,
            KeyStroke::Space
                | KeyStroke::Character(_)
                | KeyStroke::ArrowLeft
                | KeyStroke::ArrowRight
        ) {
            return None;
        }
        let choices = self
            .options
            .iter()
            .filter(|o| o.enabled)
            .map(|o| o.id.clone())
            .collect::<Vec<_>>();
        let action = if matches!(chord.key, KeyStroke::PageUp | KeyStroke::PageDown) {
            if choices.is_empty() {
                ChoiceKeyAction::Consumed
            } else {
                let index = self
                    .highlighted
                    .as_ref()
                    .and_then(|id| choices.iter().position(|k| k == id))
                    .unwrap_or(0);
                let step = (self.viewport_height / SUGGESTION_ROW_HEIGHT)
                    .floor()
                    .max(1.0) as usize;
                let next = if chord.key == KeyStroke::PageDown {
                    index.saturating_add(step).min(choices.len() - 1)
                } else {
                    index.saturating_sub(step)
                };
                ChoiceKeyAction::Highlight(choices[next].clone())
            }
        } else {
            choice_key_action(
                &choices,
                self.highlighted.as_ref(),
                chord,
                ChoiceBoundary::Wrap,
            )?
        };
        if let ChoiceKeyAction::Highlight(id) = &action {
            self.highlighted = Some(id.clone());
            self.reveal();
        }
        Some(action)
    }
}
