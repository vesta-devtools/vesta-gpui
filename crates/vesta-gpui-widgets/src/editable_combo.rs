//! Text-input composition with host-filtered suggestions.

use crate::{Widget, WidgetIdentity, WidgetTheme};
use vesta_gpui::{Axis, Dimension, Element, Semantic, SemanticRole, Style};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditableComboIds {
    pub root: WidgetIdentity,
    pub list: WidgetIdentity,
    pub empty: WidgetIdentity,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditableSuggestion<A> {
    pub root: WidgetIdentity,
    pub label: String,
    pub selected: bool,
    pub action: A,
}

/// The input widget retains text/IME ownership; the host supplies filtered candidates.
#[must_use]
pub fn editable_combo<A>(
    ids: EditableComboIds,
    input: Widget<A>,
    suggestions: Vec<EditableSuggestion<A>>,
    theme: WidgetTheme,
) -> Widget<A> {
    let mut list = Widget::from_element(
        Element::container(ids.list.element, ids.list.key)
            .semantic(Semantic::new(SemanticRole::ListBox, "Suggestions"))
            .styled(Style::default().axis(Axis::Column).gap(2.0)),
    );
    if suggestions.is_empty() {
        list = list.child(Widget::from_element(
            Element::text(ids.empty.element, ids.empty.key, "No suggestions")
                .semantic(Semantic::new(SemanticRole::Label, "No suggestions"))
                .styled(
                    Style::default()
                        .size(Dimension::Percent(1.0), Dimension::Px(26.0))
                        .text(theme.typography.small_font_size, theme.palette.text_muted)
                        .pointer_events(false),
                ),
        ));
    }
    for suggestion in suggestions {
        let element = Element::text(
            suggestion.root.element,
            suggestion.root.key,
            &suggestion.label,
        )
        .semantic(
            Semantic::new(SemanticRole::ListBoxOption, &suggestion.label)
                .selected(suggestion.selected),
        )
        .styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Px(28.0))
                .text(theme.typography.font_size, theme.palette.text)
                .background(if suggestion.selected {
                    theme.palette.selection
                } else {
                    theme.palette.surface
                }),
        );
        list = list.child(Widget::from_element(element).on_click(suggestion.action));
    }
    Widget::from_element(
        Element::container(ids.root.element, ids.root.key)
            .semantic(Semantic::new(SemanticRole::ComboBox, "Search suggestions").expanded(true))
            .styled(
                Style::default()
                    .size(Dimension::Px(300.0), Dimension::Auto)
                    .max_width(Dimension::Percent(1.0))
                    .axis(Axis::Column)
                    .gap(4.0),
            ),
    )
    .child(input)
    .child(list)
}
