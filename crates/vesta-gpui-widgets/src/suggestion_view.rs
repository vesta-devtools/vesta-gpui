//! Bounded controlled EditableCombo projection over the existing virtual list.
use crate::{
    EditableComboIds, SUGGESTION_ROW_HEIGHT, SuggestionOption, SuggestionPhase, SuggestionState,
    VirtualListError, VirtualListIds, VirtualListRange, Widget, WidgetIdentity, WidgetTheme,
    virtual_list_with_items,
};
use vesta_gpui::{Axis, Dimension, Element, Semantic, SemanticRole, Style};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuggestionViewIds {
    pub combo: EditableComboIds,
    pub content: WidgetIdentity,
}

/// The caller supplies stable row identities and an action tied to the current request.
/// `None` action also makes an otherwise enabled candidate inert. The editor is the tab stop.
pub fn editable_combo_controlled<K: Clone + Ord, A>(
    ids: SuggestionViewIds,
    input: Widget<A>,
    state: &SuggestionState<K>,
    height: f32,
    theme: WidgetTheme,
    mut row: impl FnMut(&SuggestionOption<K>) -> (WidgetIdentity, Option<A>),
) -> Result<Widget<A>, VirtualListError> {
    let expanded = state.phase() != SuggestionPhase::Closed;
    let root = Widget::from_element(
        Element::container(ids.combo.root.element, ids.combo.root.key)
            .semantic(
                Semantic::new(SemanticRole::ComboBox, "Search suggestions")
                    .expanded(expanded)
                    .busy(state.phase() == SuggestionPhase::Loading),
            )
            .styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Auto)
                    .axis(Axis::Column)
                    .gap(4.0),
            ),
    )
    .child(input);
    if !expanded {
        return Ok(root);
    }
    let mut range = VirtualListRange::compute(
        state.options().len(),
        SUGGESTION_ROW_HEIGHT,
        height,
        state.scroll().offset().y,
        1,
    )?;
    let message = match state.phase() {
        SuggestionPhase::Loading => Some("Loading suggestions"),
        SuggestionPhase::Empty => Some("No suggestions"),
        SuggestionPhase::Failed => Some(state.failure().unwrap_or("Suggestions unavailable")),
        _ => None,
    };
    if let Some(message) = message {
        let list = Widget::from_element(
            Element::container(ids.combo.list.element, ids.combo.list.key)
                .semantic(
                    Semantic::new(SemanticRole::ListBox, "Suggestions")
                        .busy(state.phase() == SuggestionPhase::Loading),
                )
                .styled(
                    Style::default()
                        .size(
                            Dimension::Percent(1.0),
                            Dimension::Px(SUGGESTION_ROW_HEIGHT),
                        )
                        .axis(Axis::Column)
                        .clipped()
                        .flex(0.0, 0.0)
                        .pointer_events(true)
                        .action_boundary(true)
                        .background(theme.palette.surface),
                ),
        )
        .child(Widget::from_element(
            Element::container(ids.content.element, ids.content.key).styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Px(0.0))
                    .flex(0.0, 0.0)
                    .pointer_events(false),
            ),
        ))
        .child(Widget::from_element(
            Element::text(ids.combo.empty.element, ids.combo.empty.key, message)
                .semantic(Semantic::new(SemanticRole::Label, message))
                .styled(
                    Style::default()
                        .size(
                            Dimension::Percent(1.0),
                            Dimension::Px(SUGGESTION_ROW_HEIGHT),
                        )
                        .clipped()
                        .text(theme.typography.small_font_size, theme.palette.text_muted)
                        .pointer_events(false),
                ),
        ));
        return Ok(root.child(list));
    }
    range.viewport_height = range.viewport_height.min(range.content_height);
    let mut list = virtual_list_with_items(
        VirtualListIds {
            root: ids.combo.list,
            content: ids.content,
        },
        Dimension::Percent(1.0),
        state.options(),
        range,
        None,
        |option| {
            let (id, action) = row(option);
            let enabled = option.enabled && action.is_some();
            let element = Element::text(id.element, id.key, &option.label)
                .semantic(
                    Semantic::new(SemanticRole::ListBoxOption, &option.label)
                        .selected(state.highlighted() == Some(&option.id))
                        .activatable(enabled),
                )
                .styled(
                    Style::default()
                        .clipped()
                        .focusable(false)
                        .action_boundary(!enabled)
                        .pointer_events(true)
                        .text(
                            theme.typography.font_size,
                            if enabled {
                                theme.palette.text
                            } else {
                                theme.palette.text_disabled
                            },
                        )
                        .background(if state.highlighted() == Some(&option.id) {
                            theme.palette.selection
                        } else {
                            theme.palette.surface
                        }),
                );
            let widget = Widget::from_element(element);
            if let Some(action) = action.filter(|_| enabled) {
                widget.on_click(action)
            } else {
                widget
            }
        },
    )?;
    list.element.semantic = Some(
        Semantic::new(SemanticRole::ListBox, "Suggestions")
            .busy(state.phase() == SuggestionPhase::Loading),
    );
    list.element.style = list
        .element
        .style
        .scrollbar(theme.scrollbar_style())
        .flex(0.0, 0.0)
        .action_boundary(state.phase() != SuggestionPhase::Ready);
    Ok(root.child(list))
}
