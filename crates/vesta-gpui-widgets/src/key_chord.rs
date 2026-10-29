//! Host-controlled shortcut capture trigger.

use crate::{ControlState, Widget, WidgetIdentity, WidgetTheme, button};
use vesta_gpui::{InputTarget, KeyChord, KeyStroke, Semantic, SemanticRole, UiWindowId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyCaptureAction {
    Assigned(KeyChord),
    Conflict,
    Cancelled,
    Consumed,
}

/// Capture precedes normal shortcuts. Reservation policy stays in the host keymap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyChordCapture {
    target: Option<InputTarget>,
    conflict: bool,
}

impl KeyChordCapture {
    pub fn begin(&mut self, target: InputTarget) {
        self.target = Some(target);
        self.conflict = false;
    }

    #[must_use]
    pub const fn target(&self) -> Option<InputTarget> {
        self.target
    }

    #[must_use]
    pub const fn conflict(&self) -> bool {
        self.conflict
    }

    pub fn cancel_for(&mut self, source: UiWindowId) -> bool {
        if self.target.is_some_and(|target| target.window == source) {
            self.target = None;
            self.conflict = false;
            true
        } else {
            false
        }
    }

    pub fn key_action(
        &mut self,
        source: UiWindowId,
        live_focus: bool,
        chord: Option<KeyChord>,
        reserved: impl FnOnce(KeyChord) -> bool,
    ) -> Option<KeyCaptureAction> {
        let target = self.target.filter(|target| target.window == source)?;
        if !live_focus {
            self.cancel_for(target.window);
            return None;
        }
        let Some(chord) = chord else {
            return Some(KeyCaptureAction::Consumed);
        };
        Some(if chord.key == KeyStroke::Escape {
            self.cancel_for(source);
            KeyCaptureAction::Cancelled
        } else if reserved(chord) {
            self.conflict = true;
            KeyCaptureAction::Conflict
        } else {
            self.cancel_for(source);
            KeyCaptureAction::Assigned(chord)
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyChordSelectorView<'a> {
    pub label: &'a str,
    pub value: Option<&'a str>,
    pub capturing: bool,
    pub conflict: bool,
    pub enabled: bool,
}

/// The host captures and validates keys; this widget only projects its state.
#[must_use]
pub fn key_chord_selector<A>(
    root: WidgetIdentity,
    label: WidgetIdentity,
    view: KeyChordSelectorView<'_>,
    begin_action: A,
    theme: WidgetTheme,
) -> Widget<A> {
    let caption = if view.conflict {
        "Shortcut in use; press another key".to_owned()
    } else if view.capturing {
        "Press a shortcut · Esc cancels".to_owned()
    } else {
        view.value.unwrap_or("Set shortcut").to_owned()
    };
    let mut control = button(
        root,
        label,
        &caption,
        begin_action,
        if view.enabled {
            if view.capturing {
                ControlState::Pressed
            } else {
                ControlState::Idle
            }
        } else {
            ControlState::Disabled
        },
        theme,
    );
    control.element.semantic = Some(
        Semantic::new(SemanticRole::Button, view.label)
            .value(&caption)
            .description("Activate, then press a shortcut; Escape cancels"),
    );
    if view.conflict {
        control.element.style.visual.border_color = Some(theme.palette.warning);
    }
    control
}
