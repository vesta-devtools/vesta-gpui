//! Window-owned keyboard precedence and text/modal barriers (ADR-0306).

use vesta_gpui::{InputTarget, KeyChord, KeyStroke, UiWindowId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyRouteLayer {
    Capture,
    Menu,
    Popup,
    Gesture,
    TextCommand,
    FocusedControl,
    FocusTraversal,
    Activation,
    Shortcut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyRouteContext {
    pub window: UiWindowId,
    pub text_owner: Option<InputTarget>,
    pub composing: bool,
    pub modal: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyRouteOutcome {
    Consumed(KeyRouteLayer),
    TextInput,
    ModalBlocked,
    InvalidTextOwner,
    Unhandled,
}

/// Runs the first applicable consumer only. The host supplies live control state and
/// performs typed actions; OS events, document writes and business keymaps stay outside.
/// A composition key never reaches a non-text consumer, including an unknown key.
#[must_use]
pub fn route_key(
    context: KeyRouteContext,
    chord: Option<KeyChord>,
    mut consume: impl FnMut(KeyRouteLayer) -> bool,
) -> KeyRouteOutcome {
    if context
        .text_owner
        .is_some_and(|owner| owner.window != context.window)
        || (context.composing && context.text_owner.is_none())
    {
        return KeyRouteOutcome::InvalidTextOwner;
    }
    if context.composing {
        return KeyRouteOutcome::TextInput;
    }
    for layer in [
        KeyRouteLayer::Capture,
        KeyRouteLayer::Menu,
        KeyRouteLayer::Popup,
        KeyRouteLayer::Gesture,
        KeyRouteLayer::TextCommand,
        KeyRouteLayer::FocusedControl,
    ] {
        if consume(layer) {
            return KeyRouteOutcome::Consumed(layer);
        }
    }
    if context.text_owner.is_some() {
        return KeyRouteOutcome::TextInput;
    }
    if let Some(chord) = chord
        && !chord.modifiers.control
        && !chord.modifiers.alt
        && !chord.modifiers.super_key
    {
        let layer = match chord.key {
            KeyStroke::Tab => Some(KeyRouteLayer::FocusTraversal),
            KeyStroke::Enter | KeyStroke::Space => Some(KeyRouteLayer::Activation),
            _ => None,
        };
        if let Some(layer) = layer
            && consume(layer)
        {
            return KeyRouteOutcome::Consumed(layer);
        }
    }
    if context.modal {
        return KeyRouteOutcome::ModalBlocked;
    }
    if consume(KeyRouteLayer::Shortcut) {
        KeyRouteOutcome::Consumed(KeyRouteLayer::Shortcut)
    } else {
        KeyRouteOutcome::Unhandled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vesta_gpui::{ElementId, KeyModifiers};

    #[test]
    fn consumption_text_composition_and_modal_barriers_prevent_duplicate_actions() {
        let context = KeyRouteContext {
            window: UiWindowId(1),
            text_owner: None,
            composing: false,
            modal: false,
        };
        let tab = Some(KeyChord::new(KeyStroke::Tab, KeyModifiers::default()));
        let mut visited = Vec::new();
        assert_eq!(
            route_key(context, tab, |layer| {
                visited.push(layer);
                layer == KeyRouteLayer::Popup
            }),
            KeyRouteOutcome::Consumed(KeyRouteLayer::Popup)
        );
        assert!(!visited.contains(&KeyRouteLayer::FocusTraversal));
        assert!(!visited.contains(&KeyRouteLayer::Shortcut));
        let text = KeyRouteContext {
            text_owner: Some(InputTarget {
                window: context.window,
                element: ElementId::new(12),
            }),
            ..context
        };
        visited.clear();
        assert_eq!(
            route_key(text, tab, |layer| {
                visited.push(layer);
                false
            }),
            KeyRouteOutcome::TextInput
        );
        assert!(!visited.contains(&KeyRouteLayer::Activation));
        assert!(!visited.contains(&KeyRouteLayer::FocusTraversal));
        assert!(!visited.contains(&KeyRouteLayer::Shortcut));
        for chord in [tab, None] {
            assert_eq!(
                route_key(
                    KeyRouteContext {
                        composing: true,
                        ..text
                    },
                    chord,
                    |_| { panic!("IME composition is exclusively text-owned") }
                ),
                KeyRouteOutcome::TextInput
            );
        }
        assert_eq!(
            route_key(
                KeyRouteContext {
                    window: UiWindowId(2),
                    ..text
                },
                tab,
                |_| { panic!("foreign owner is rejected before dispatch") }
            ),
            KeyRouteOutcome::InvalidTextOwner
        );
        visited.clear();
        assert_eq!(
            route_key(
                KeyRouteContext {
                    modal: true,
                    ..context
                },
                tab,
                |layer| {
                    visited.push(layer);
                    false
                }
            ),
            KeyRouteOutcome::ModalBlocked
        );
        assert!(visited.contains(&KeyRouteLayer::FocusTraversal));
        assert!(!visited.contains(&KeyRouteLayer::Shortcut));
        visited.clear();
        let modified_tab = Some(KeyChord::new(
            KeyStroke::Tab,
            KeyModifiers {
                control: true,
                ..KeyModifiers::default()
            },
        ));
        assert_eq!(
            route_key(context, modified_tab, |layer| {
                visited.push(layer);
                layer == KeyRouteLayer::Shortcut
            }),
            KeyRouteOutcome::Consumed(KeyRouteLayer::Shortcut)
        );
        assert!(!visited.contains(&KeyRouteLayer::FocusTraversal));
    }
}
