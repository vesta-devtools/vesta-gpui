//! Controlled display/edit switch for a host-owned text value.

use crate::{ControlState, Widget, WidgetIdentity, WidgetTheme, button};
use vesta_gpui::{
    Axis, Dimension, Element, InputTarget, KeyChord, KeyStroke, Semantic, SemanticRole, Style,
    UiWindowId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InlineEditKeyAction {
    Commit,
    Cancel,
}

/// Composition owns confirmation and cancellation until the platform ends preedit.
#[must_use]
pub fn inline_edit_key_action(
    owner: InputTarget,
    source: UiWindowId,
    editing: bool,
    composing: bool,
    chord: KeyChord,
) -> Option<InlineEditKeyAction> {
    if owner.window != source
        || !editing
        || composing
        || chord.modifiers.control
        || chord.modifiers.alt
        || chord.modifiers.super_key
        || chord.modifiers.shift
    {
        return None;
    }
    match chord.key {
        KeyStroke::Enter => Some(InlineEditKeyAction::Commit),
        KeyStroke::Escape => Some(InlineEditKeyAction::Cancel),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineEditIds {
    pub root: WidgetIdentity,
    pub display: WidgetIdentity,
    pub display_label: WidgetIdentity,
    pub controls: WidgetIdentity,
    pub commit: WidgetIdentity,
    pub commit_label: WidgetIdentity,
    pub cancel: WidgetIdentity,
    pub cancel_label: WidgetIdentity,
}

/// `editor` is supplied only while editing and retains the host's text-input identity.
#[must_use]
pub fn inline_editable_label<A>(
    ids: InlineEditIds,
    value: &str,
    editor: Option<Element>,
    begin: A,
    commit: A,
    cancel: A,
    theme: WidgetTheme,
) -> Widget<A> {
    let mut root = Widget::from_element(
        Element::container(ids.root.element, ids.root.key)
            .semantic(Semantic::new(SemanticRole::Group, "Editable label"))
            .styled(
                Style::default()
                    .size(Dimension::Px(300.0), Dimension::Auto)
                    .max_width(Dimension::Percent(1.0))
                    .axis(Axis::Column)
                    .gap(4.0),
            ),
    );
    if let Some(editor) = editor {
        let controls = Widget::from_element(
            Element::container(ids.controls.element, ids.controls.key)
                .styled(Style::default().axis(Axis::Row).gap(4.0)),
        )
        .child(button(
            ids.commit,
            ids.commit_label,
            "Save",
            commit,
            ControlState::Idle,
            theme,
        ))
        .child(button(
            ids.cancel,
            ids.cancel_label,
            "Cancel",
            cancel,
            ControlState::Idle,
            theme,
        ));
        root = root.child(Widget::from_element(editor)).child(controls);
    } else {
        let mut display = button(
            ids.display,
            ids.display_label,
            value,
            begin,
            ControlState::Idle,
            theme,
        );
        display.element.semantic = Some(Semantic::new(
            SemanticRole::Button,
            format!("Edit label: {value}"),
        ));
        root = root.child(display);
    }
    root
}
