//! Search editing stays in the host's `TextInputState`; the widget only projects it.

use crate::{Widget, WidgetIdentity, WidgetTheme};
use vesta_gpui::{
    Align, Axis, Dimension, Element, FocusIndicator, Semantic, SemanticRole, Style, TextHAlign,
    TextInputSnapshot, TextVAlign,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchFieldIds {
    pub root: WidgetIdentity,
    pub icon: WidgetIdentity,
    pub input: WidgetIdentity,
    pub placeholder: WidgetIdentity,
    pub clear: WidgetIdentity,
}

#[must_use]
pub fn search_field<A>(
    ids: SearchFieldIds,
    snapshot: TextInputSnapshot,
    empty: bool,
    focused: bool,
    focus_action: A,
    clear_action: A,
    theme: WidgetTheme,
) -> Widget<A> {
    let SearchFieldIds {
        root,
        icon,
        input,
        placeholder,
        clear,
    } = ids;
    let mut element = Element::container(root.element, root.key).styled(
        Style::default()
            .size(
                Dimension::Px(240.0),
                Dimension::Px(theme.metrics.control_height),
            )
            .axis(Axis::Row)
            .align(Align::Center)
            .background(theme.palette.field)
            .border(
                if focused { 1.5 } else { 1.0 },
                if focused {
                    theme.palette.focus
                } else {
                    theme.palette.border
                },
            )
            .radius(theme.metrics.radius),
    );
    element = element.child(
        Element::text(icon.element, icon.key, "⌕").styled(
            Style::default()
                .size(Dimension::Px(28.0), Dimension::Percent(1.0))
                .text(theme.typography.font_size, theme.palette.text_muted)
                .text_align(TextHAlign::Center, TextVAlign::Center)
                .pointer_events(false),
        ),
    );
    let mut input_element = Element::text_input(input.element, input.key, snapshot, focused)
        .semantic(Semantic::new(SemanticRole::SearchInput, "Search objects"))
        .styled(
            Style::default()
                .size(Dimension::Auto, Dimension::Percent(1.0))
                .flex(1.0, 1.0)
                .focusable(true)
                .focus_indicator(FocusIndicator::Control)
                .text(theme.typography.font_size, theme.palette.text),
        );
    if empty && !focused {
        input_element = input_element.child(
            Element::text(placeholder.element, placeholder.key, "Search objects…").styled(
                Style::default()
                    .absolute(8.0, 0.0)
                    .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                    .text(theme.typography.font_size, theme.palette.text_muted)
                    .pointer_events(false),
            ),
        );
    }
    element = element.child(input_element);
    let mut widget = Widget::from_element(element).on_click(focus_action);
    if !empty {
        widget = widget.child(
            Widget::from_element(
                Element::text(clear.element, clear.key, "×")
                    .styled(
                        Style::default()
                            .size(Dimension::Px(26.0), Dimension::Percent(1.0))
                            .focusable(true)
                            .text(theme.typography.font_size, theme.palette.text_muted)
                            .text_align(TextHAlign::Center, TextVAlign::Center),
                    )
                    .semantic(Semantic::new(SemanticRole::Button, "Clear search")),
            )
            .on_click(clear_action),
        );
    }
    widget
}
