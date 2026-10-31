//! Caller-owned radio choices with one enabled Tab stop and shared key policy.

use crate::{
    ChoiceBoundary, ChoiceKeyAction, ControlState, RadioIds, Widget, WidgetIdentity, WidgetTheme,
    choice_key_action, radio,
};
use std::{collections::HashSet, hash::Hash};
use vesta_gpui::{Element, ElementId, KeyChord, KeyStroke, Semantic, SemanticRole, Style};

pub struct RadioGroupOption<K, A> {
    pub value: K,
    pub ids: RadioIds,
    pub label: String,
    pub action: A,
    pub state: ControlState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadioGroupError {
    DuplicateValue(ElementId),
}

impl std::fmt::Display for RadioGroupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateValue(id) => write!(f, "duplicate radio-group value {id:?}"),
        }
    }
}
impl std::error::Error for RadioGroupError {}

/// The selected enabled choice is the Tab stop; otherwise use the first enabled
/// choice without silently changing the caller's selection. Empty groups are valid.
pub fn radio_group<K: Eq + Hash, A>(
    root: WidgetIdentity,
    name: impl Into<String>,
    selected: Option<&K>,
    choices: Vec<RadioGroupOption<K, A>>,
    theme: WidgetTheme,
) -> Result<Widget<A>, RadioGroupError> {
    let mut seen = HashSet::with_capacity(choices.len());
    for choice in &choices {
        if !seen.insert(&choice.value) {
            return Err(RadioGroupError::DuplicateValue(choice.ids.root.element));
        }
    }
    let stop = choices
        .iter()
        .find(|choice| selected == Some(&choice.value) && choice.state != ControlState::Disabled)
        .or_else(|| {
            choices
                .iter()
                .find(|choice| choice.state != ControlState::Disabled)
        })
        .map(|choice| choice.ids.root.element);
    let mut group = Widget::from_element(
        Element::container(root.element, root.key)
            .semantic(Semantic::new(SemanticRole::Group, name))
            .styled(Style::default().gap(theme.metrics.spacing)),
    );
    for choice in choices {
        let selected = selected == Some(&choice.value);
        let mut widget = radio(
            choice.ids,
            choice.label,
            selected,
            choice.action,
            choice.state,
            theme,
        );
        widget.element.style.focusable &= Some(widget.element.id) == stop;
        group = group.child(widget);
    }
    Ok(group)
}

/// Supply the current enabled stable values, in visual order. Arrows select
/// immediately through Highlight; Space/Enter confirm. Tab/Escape remain host keys.
#[must_use]
pub fn radio_group_key_action<K: Clone + Eq>(
    enabled: &[K],
    focused: Option<&K>,
    mut chord: KeyChord,
) -> Option<ChoiceKeyAction<K>> {
    chord.key = match chord.key {
        KeyStroke::ArrowLeft => KeyStroke::ArrowUp,
        KeyStroke::ArrowRight => KeyStroke::ArrowDown,
        KeyStroke::Tab | KeyStroke::Escape => return None,
        key => key,
    };
    choice_key_action(enabled, focused, chord, ChoiceBoundary::Wrap)
}
