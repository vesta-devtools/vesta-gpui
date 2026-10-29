//! Masked host-owned text projection with explicit reveal and clipboard policy.

use crate::{ControlState, Widget, WidgetIdentity, WidgetTheme, button};
use vesta_gpui::{
    Align, Axis, Dimension, Element, FocusIndicator, Semantic, SemanticRole, Style,
    TextInputSnapshot,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasswordFieldIds {
    pub root: WidgetIdentity,
    pub display: WidgetIdentity,
    pub controls: WidgetIdentity,
    pub reveal: WidgetIdentity,
    pub reveal_label: WidgetIdentity,
    pub clear: WidgetIdentity,
    pub clear_label: WidgetIdentity,
}

pub struct PasswordFieldView<'a> {
    pub mask: &'a str,
    /// Derived from password-mode TextInputState; reveal requires the explicit revealed snapshot.
    pub snapshot: TextInputSnapshot,
    pub revealed: bool,
    pub focused: bool,
}

pub struct PasswordFieldActions<A> {
    pub focus: A,
    pub reveal: A,
    pub clear: A,
}

/// The host owns the secret, focus, typing, and clipboard policy.
#[must_use]
pub fn password_field<A>(
    ids: PasswordFieldIds,
    view: PasswordFieldView<'_>,
    actions: PasswordFieldActions<A>,
    theme: WidgetTheme,
) -> Widget<A> {
    let PasswordFieldView {
        mask,
        snapshot,
        revealed,
        focused,
    } = view;
    let mut root = Widget::from_element(
        Element::container(ids.root.element, ids.root.key)
            .semantic(
                Semantic::new(SemanticRole::PasswordInput, "Password")
                    .value(mask)
                    .description("Copy and cut are disabled"),
            )
            .styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Auto)
                    .max_width(Dimension::Px(320.0))
                    .min_width(Dimension::Px(0.0))
                    .axis(Axis::Column)
                    .gap(4.0)
                    .background(theme.palette.field)
                    .border(
                        1.0,
                        if focused {
                            theme.palette.focus
                        } else {
                            theme.palette.border
                        },
                    )
                    .focusable(true)
                    .focus_indicator(FocusIndicator::Control),
            )
            .child(
                Element::text_input(ids.display.element, ids.display.key, snapshot, focused)
                    .styled(
                        Style::default()
                            .size(
                                Dimension::Percent(1.0),
                                Dimension::Px(theme.metrics.control_height),
                            )
                            .min_width(Dimension::Px(0.0))
                            .text(theme.typography.font_size, theme.palette.text)
                            .pointer_events(false),
                    ),
            ),
    )
    .on_click(actions.focus);
    let mut controls = Widget::from_element(
        Element::container(ids.controls.element, ids.controls.key).styled(
            Style::default()
                .axis(Axis::Row)
                .align(Align::Center)
                .gap(4.0),
        ),
    )
    .child(button(
        ids.reveal,
        ids.reveal_label,
        if revealed { "Hide" } else { "Show" },
        actions.reveal,
        ControlState::Idle,
        theme,
    ));
    if !mask.is_empty() {
        controls = controls.child(button(
            ids.clear,
            ids.clear_label,
            "Clear",
            actions.clear,
            ControlState::Idle,
            theme,
        ));
    }
    root = root.child(controls);
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use vesta_gpui::{ElementId, ElementKind};

    fn id(value: u64) -> WidgetIdentity {
        WidgetIdentity::new(ElementId::new(value), format!("password.{value}"))
    }

    #[test]
    fn hidden_projection_never_contains_secret_and_actions_keep_identity() {
        let mut context = vesta_gpui::TextInputContext::default();
        let mut editor = vesta_gpui::TextInputState::new_password(13.0, "sans-serif", 256);
        editor.reset(&mut context, "s3cret").expect("secret");
        let ids = PasswordFieldIds {
            root: id(1),
            display: id(2),
            controls: id(7),
            reveal: id(3),
            reveal_label: id(4),
            clear: id(5),
            clear_label: id(6),
        };
        let hidden = password_field(
            ids.clone(),
            PasswordFieldView {
                mask: &editor.masked_value(),
                snapshot: editor.snapshot(),
                revealed: false,
                focused: false,
            },
            PasswordFieldActions {
                focus: 1_u8,
                reveal: 2,
                clear: 3,
            },
            WidgetTheme::dark(),
        );
        let display = hidden.element.find(ElementId::new(2)).expect("display");
        assert!(matches!(&display.kind, ElementKind::TextInput { .. }));
        assert_eq!(
            hidden.element.semantic.as_ref().expect("semantics").role,
            SemanticRole::PasswordInput
        );
        assert_eq!(
            hidden
                .element
                .semantic
                .as_ref()
                .expect("semantics")
                .value
                .as_deref(),
            Some("••••••")
        );
        assert_eq!(hidden.action_at(ElementId::new(1)), Some(&1));
        assert_eq!(hidden.action_at(ElementId::new(3)), Some(&2));
        assert_eq!(hidden.action_at(ElementId::new(5)), Some(&3));
        let revealed = password_field(
            ids,
            PasswordFieldView {
                mask: &editor.masked_value(),
                snapshot: editor.revealed_snapshot(&mut context),
                revealed: true,
                focused: true,
            },
            PasswordFieldActions {
                focus: 1_u8,
                reveal: 2,
                clear: 3,
            },
            WidgetTheme::dark(),
        );
        assert!(matches!(
            &revealed
                .element
                .find(ElementId::new(2))
                .expect("display")
                .kind,
            ElementKind::TextInput { .. }
        ));
        assert_eq!(
            revealed
                .element
                .semantic
                .as_ref()
                .expect("semantics")
                .value
                .as_deref(),
            Some("••••••")
        );
    }
}
