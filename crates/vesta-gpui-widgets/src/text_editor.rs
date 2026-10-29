//! Shared single-line and multiline editor projections.
use crate::{WidgetIdentity, WidgetTheme};
use vesta_gpui::{
    Dimension, Element, FocusIndicator, Semantic, SemanticRole, Style, TextInputSnapshot,
    TextVAlign,
};

#[must_use]
pub fn text_field(
    identity: WidgetIdentity,
    snapshot: TextInputSnapshot,
    focused: bool,
    theme: WidgetTheme,
) -> Element {
    editor_field(
        identity,
        snapshot,
        focused,
        SemanticRole::TextInput,
        "Text",
        theme.metrics.control_height,
        theme,
    )
}

/// A multiline editor projection. The host owns the text, selection, scroll and undo state.
#[must_use]
pub fn text_area(
    identity: WidgetIdentity,
    snapshot: TextInputSnapshot,
    focused: bool,
    theme: WidgetTheme,
) -> Element {
    editor_field(
        identity,
        snapshot.with_vertical_alignment(TextVAlign::Start),
        focused,
        SemanticRole::MultilineTextInput,
        "Text area",
        120.0,
        theme,
    )
}

fn editor_field(
    identity: WidgetIdentity,
    snapshot: TextInputSnapshot,
    focused: bool,
    role: SemanticRole,
    label: &str,
    height: f32,
    theme: WidgetTheme,
) -> Element {
    Element::text_input(identity.element, identity.key, snapshot, focused)
        .semantic(Semantic::new(role, label))
        .styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Px(height))
                .background(theme.palette.field)
                .border(
                    if focused { 1.5 } else { 1.0 },
                    if focused {
                        theme.palette.accent
                    } else {
                        theme.palette.border
                    },
                )
                .radius(theme.metrics.radius)
                .focusable(true)
                .focus_indicator(FocusIndicator::Control)
                .text(theme.typography.font_size, theme.palette.text),
        )
}
