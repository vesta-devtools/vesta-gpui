//! Typed keyboard decisions for host-supplied enabled choices (ADR-0306).

use vesta_gpui::{KeyChord, KeyStroke};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceBoundary {
    Wrap,
    Clamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChoiceKeyAction<K> {
    Highlight(K),
    Confirm(K),
    Close,
    TraverseFocus { backwards: bool },
    Consumed,
}

/// The caller supplies enabled stable identities in the current visual order and decides
/// whether this control owns the key (text composition and shortcut precedence stay explicit).
/// Empty choices still consume navigation/confirmation, rather than activating a parent.
#[must_use]
pub fn choice_key_action<K: Clone + Eq>(
    choices: &[K],
    current: Option<&K>,
    chord: KeyChord,
    boundary: ChoiceBoundary,
) -> Option<ChoiceKeyAction<K>> {
    if chord.modifiers.control || chord.modifiers.alt || chord.modifiers.super_key {
        return None;
    }
    match chord.key {
        KeyStroke::Escape => return Some(ChoiceKeyAction::Close),
        KeyStroke::Tab => {
            return Some(ChoiceKeyAction::TraverseFocus {
                backwards: chord.modifiers.shift,
            });
        }
        KeyStroke::ArrowDown
        | KeyStroke::ArrowUp
        | KeyStroke::Home
        | KeyStroke::End
        | KeyStroke::Enter
        | KeyStroke::Space => {}
        _ => return None,
    }
    if choices.is_empty() {
        return Some(ChoiceKeyAction::Consumed);
    }
    let current = current
        .and_then(|value| choices.iter().position(|item| item == value))
        .unwrap_or(0);
    let index = match chord.key {
        KeyStroke::ArrowDown if current + 1 < choices.len() => current + 1,
        KeyStroke::ArrowDown => match boundary {
            ChoiceBoundary::Wrap => 0,
            ChoiceBoundary::Clamp => current,
        },
        KeyStroke::ArrowUp if current > 0 => current - 1,
        KeyStroke::ArrowUp => match boundary {
            ChoiceBoundary::Wrap => choices.len() - 1,
            ChoiceBoundary::Clamp => 0,
        },
        KeyStroke::Home => 0,
        KeyStroke::End => choices.len() - 1,
        KeyStroke::Enter | KeyStroke::Space => {
            return Some(ChoiceKeyAction::Confirm(choices[current].clone()));
        }
        _ => return None,
    };
    Some(ChoiceKeyAction::Highlight(choices[index].clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vesta_gpui::KeyModifiers;

    #[test]
    fn navigation_uses_enabled_stable_choices_after_reorder_and_handles_empty_popups() {
        let choices = [50_u64, 20, 90, 10, 80];
        let key = |key| KeyChord::new(key, KeyModifiers::default());
        assert_eq!(
            choice_key_action(
                &choices,
                Some(&90),
                key(KeyStroke::ArrowDown),
                ChoiceBoundary::Wrap
            ),
            Some(ChoiceKeyAction::Highlight(10))
        );
        assert_eq!(
            choice_key_action(
                &choices,
                Some(&80),
                key(KeyStroke::ArrowDown),
                ChoiceBoundary::Wrap
            ),
            Some(ChoiceKeyAction::Highlight(50))
        );
        assert_eq!(
            choice_key_action(
                &choices,
                Some(&80),
                key(KeyStroke::ArrowDown),
                ChoiceBoundary::Clamp
            ),
            Some(ChoiceKeyAction::Highlight(80))
        );
        assert_eq!(
            choice_key_action(
                &[80, 90, 20],
                Some(&90),
                key(KeyStroke::Enter),
                ChoiceBoundary::Wrap
            ),
            Some(ChoiceKeyAction::Confirm(90))
        );
        assert_eq!(
            choice_key_action(
                &[80, 20],
                Some(&90),
                key(KeyStroke::Enter),
                ChoiceBoundary::Wrap
            ),
            Some(ChoiceKeyAction::Confirm(80))
        );
        assert_eq!(
            choice_key_action::<u64>(&[], None, key(KeyStroke::Enter), ChoiceBoundary::Wrap),
            Some(ChoiceKeyAction::Consumed)
        );
        let backwards = KeyChord::new(
            KeyStroke::Tab,
            KeyModifiers {
                shift: true,
                ..KeyModifiers::default()
            },
        );
        assert_eq!(
            choice_key_action::<u64>(&[], None, backwards, ChoiceBoundary::Wrap),
            Some(ChoiceKeyAction::TraverseFocus { backwards: true })
        );
        assert_eq!(
            choice_key_action(
                &choices,
                None,
                key(KeyStroke::Character('x')),
                ChoiceBoundary::Wrap
            ),
            None
        );
        for modifiers in [
            KeyModifiers {
                control: true,
                ..Default::default()
            },
            KeyModifiers {
                alt: true,
                ..Default::default()
            },
            KeyModifiers {
                super_key: true,
                ..Default::default()
            },
        ] {
            assert_eq!(
                choice_key_action(
                    &choices,
                    None,
                    KeyChord::new(KeyStroke::Tab, modifiers),
                    ChoiceBoundary::Wrap
                ),
                None
            );
        }
    }
}
