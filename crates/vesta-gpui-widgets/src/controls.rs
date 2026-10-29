use vesta_gpui::{
    Align, Axis, Dimension, Element, ElementId, FocusIndicator, Insets, Justify, Point, Semantic,
    SemanticRole, Style, TextHAlign, TextInputSnapshot, TextVAlign,
};

use crate::{
    CompactSign, ControlState, ICON_LINE_SIZE, ICON_LINE_STROKE, TreeRowContentIds, Widget,
    WidgetContent, WidgetIdentity, WidgetTheme, compact_sign,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComboBoxIds {
    pub root: WidgetIdentity,
    pub label: WidgetIdentity,
    pub arrow: WidgetIdentity,
}

#[must_use]
pub fn combo_box<A>(
    ids: ComboBoxIds,
    name: impl Into<String>,
    value: impl Into<String>,
    open: bool,
    action: A,
    state: ControlState,
    theme: WidgetTheme,
) -> Widget<A> {
    let name = name.into();
    let value = value.into();
    let ComboBoxIds { root, label, arrow } = ids;
    let enabled = state != ControlState::Disabled;
    let root_id = root.element;
    let border = if open {
        theme.palette.focus
    } else {
        theme.palette.border
    };
    let element = Element::container(root.element, root.key)
        .semantic(
            Semantic::new(SemanticRole::ComboBox, name)
                .value(&value)
                .expanded(open),
        )
        .styled(
            Style::default()
                .size(
                    Dimension::Px(220.0),
                    Dimension::Px(theme.metrics.control_height),
                )
                .axis(Axis::Row)
                .align(Align::Center)
                .padding(Insets::symmetric(10.0, 0.0))
                .background(if enabled {
                    theme.palette.field
                } else {
                    theme.palette.surface_disabled
                })
                .border(if open { 1.5 } else { 1.0 }, border)
                .radius(theme.metrics.radius)
                .pointer_events(enabled)
                .focusable(enabled)
                .action_boundary(!enabled),
        )
        .child(
            Element::text(label.element, label.key, value).styled(
                Style::default()
                    .size(Dimension::Auto, Dimension::Percent(1.0))
                    .flex(1.0, 1.0)
                    .text(
                        theme.typography.font_size,
                        if enabled {
                            theme.palette.text
                        } else {
                            theme.palette.text_disabled
                        },
                    )
                    .pointer_events(false),
            ),
        )
        .child(
            Element::container(arrow.element, arrow.key).styled(
                Style::default()
                    .size(Dimension::Px(ICON_LINE_SIZE), Dimension::Px(ICON_LINE_SIZE))
                    .polyline(
                        crate::disclosure_chevron_points(true)
                            .into_iter()
                            .map(|p| {
                                if open {
                                    Point::new(p.x, ICON_LINE_SIZE - p.y)
                                } else {
                                    p
                                }
                            })
                            .collect(),
                        ICON_LINE_STROKE,
                    )
                    .background(if enabled {
                        theme.palette.text_muted
                    } else {
                        theme.palette.text_disabled
                    })
                    .pointer_events(false),
            ),
        );
    Widget {
        element,
        actions: enabled.then_some((root_id, action)).into_iter().collect(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumberFieldIds {
    pub root: WidgetIdentity,
    pub value_label: WidgetIdentity,
    pub decrement: WidgetIdentity,
    pub decrement_label: WidgetIdentity,
    pub increment: WidgetIdentity,
    pub increment_label: WidgetIdentity,
}

#[must_use]
pub fn number_field<A>(
    ids: NumberFieldIds,
    value: impl Into<String>,
    decrement_action: A,
    increment_action: A,
    state: ControlState,
    theme: WidgetTheme,
) -> Widget<A> {
    let NumberFieldIds {
        root,
        value_label,
        decrement,
        decrement_label,
        increment,
        increment_label,
    } = ids;
    let value = value.into();
    let enabled = state != ControlState::Disabled;
    let button = |identity: WidgetIdentity, label: WidgetIdentity, sign: CompactSign| {
        Element::container(identity.element, identity.key)
            .semantic(Semantic::new(
                SemanticRole::Button,
                if sign == CompactSign::Minus {
                    "Decrease"
                } else {
                    "Increase"
                },
            ))
            .styled(
                Style::default()
                    .size(Dimension::Px(28.0), Dimension::Percent(1.0))
                    .axis(Axis::Row)
                    .align(Align::Center)
                    .justify(Justify::Center)
                    .background(theme.palette.surface)
                    .pointer_events(enabled)
                    .focusable(enabled)
                    .action_boundary(!enabled),
            )
            .child(compact_sign(
                label,
                sign,
                if enabled {
                    theme.palette.text
                } else {
                    theme.palette.text_disabled
                },
            ))
    };
    let decrement_id = decrement.element;
    let increment_id = increment.element;
    let element = Element::container(root.element, root.key)
        .semantic(Semantic::new(SemanticRole::Group, "Number").value(&value))
        .styled(
            Style::default()
                .size(
                    Dimension::Px(180.0),
                    Dimension::Px(theme.metrics.control_height),
                )
                .axis(Axis::Row)
                .align(Align::Center)
                .background(theme.palette.field)
                .border(1.0, theme.palette.border)
                .radius(theme.metrics.radius)
                .pointer_events(false)
                .action_boundary(!enabled),
        )
        .child(button(decrement, decrement_label, CompactSign::Minus))
        .child(
            Element::text(value_label.element, value_label.key, value).styled(
                Style::default()
                    .size(Dimension::Auto, Dimension::Percent(1.0))
                    .flex(1.0, 1.0)
                    .text(theme.typography.font_size, theme.palette.text)
                    .text_align(TextHAlign::Center, TextVAlign::Center)
                    .pointer_events(false),
            ),
        )
        .child(button(increment, increment_label, CompactSign::Plus));
    Widget {
        element,
        actions: if enabled {
            vec![
                (decrement_id, decrement_action),
                (increment_id, increment_action),
            ]
        } else {
            Vec::new()
        },
    }
}

/// A host-controlled number editor with direct text input and typed step actions.
///
/// The host owns the draft [`vesta_gpui::TextInputState`], parsing, range validation, and commit or
/// cancel policy. The widget projects the immutable edit snapshot and numeric accessibility
/// contract without mutating the authoritative value.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn editable_number_field<A>(
    ids: NumberFieldIds,
    snapshot: TextInputSnapshot,
    focused: bool,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    decrement_action: A,
    increment_action: A,
    state: ControlState,
    theme: WidgetTheme,
) -> Widget<A> {
    let NumberFieldIds {
        root,
        value_label,
        decrement,
        decrement_label,
        increment,
        increment_label,
    } = ids;
    let enabled = state != ControlState::Disabled;
    let button = |identity: WidgetIdentity, label: WidgetIdentity, sign: CompactSign| {
        Element::container(identity.element, identity.key)
            .semantic(Semantic::new(
                SemanticRole::Button,
                if sign == CompactSign::Minus {
                    "Decrease"
                } else {
                    "Increase"
                },
            ))
            .styled(
                Style::default()
                    .size(Dimension::Px(28.0), Dimension::Percent(1.0))
                    .axis(Axis::Row)
                    .align(Align::Center)
                    .justify(Justify::Center)
                    .background(theme.palette.surface)
                    .pointer_events(enabled)
                    .focusable(enabled)
                    .action_boundary(!enabled),
            )
            .child(compact_sign(
                label,
                sign,
                if enabled {
                    theme.palette.text
                } else {
                    theme.palette.text_disabled
                },
            ))
    };
    let decrement_id = decrement.element;
    let increment_id = increment.element;
    let input = Element::text_input(value_label.element, value_label.key, snapshot, focused)
        .semantic(
            Semantic::new(SemanticRole::SpinButton, "Number")
                .value(value.to_string())
                .description("Type a number or drag horizontally to adjust")
                .numeric(value, min, max, Some(step)),
        )
        .styled(
            Style::default()
                .size(Dimension::Auto, Dimension::Percent(1.0))
                .flex(1.0, 1.0)
                .border(
                    if focused { 1.5 } else { 0.0 },
                    if focused {
                        theme.palette.accent
                    } else {
                        theme.palette.border
                    },
                )
                .focusable(enabled)
                .focus_indicator(FocusIndicator::Control)
                .pointer_events(enabled)
                .action_boundary(!enabled)
                .text(theme.typography.font_size, theme.palette.text)
                .text_align(TextHAlign::Center, TextVAlign::Center),
        );
    let element = Element::container(root.element, root.key)
        .semantic(Semantic::new(SemanticRole::Group, "Number field"))
        .styled(
            Style::default()
                .size(
                    Dimension::Px(180.0),
                    Dimension::Px(theme.metrics.control_height),
                )
                .axis(Axis::Row)
                .align(Align::Center)
                .background(theme.palette.field)
                .border(1.0, theme.palette.border)
                .radius(theme.metrics.radius)
                .pointer_events(false)
                .action_boundary(!enabled),
        )
        .child(button(decrement, decrement_label, CompactSign::Minus))
        .child(input)
        .child(button(increment, increment_label, CompactSign::Plus));
    Widget {
        element,
        actions: if enabled {
            vec![
                (decrement_id, decrement_action),
                (increment_id, increment_action),
            ]
        } else {
            Vec::new()
        },
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeItemIds {
    pub root: WidgetIdentity,
    pub disclosure: WidgetIdentity,
    pub label: WidgetIdentity,
}

#[must_use]
pub fn list_item<A>(
    root: WidgetIdentity,
    label: WidgetIdentity,
    text: impl Into<String>,
    selected: bool,
    action: A,
    state: ControlState,
    theme: WidgetTheme,
) -> Widget<A> {
    let text = text.into();
    let content = Widget::from_element(
        Element::text(label.element, label.key, &text).styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                .text(
                    theme.typography.font_size,
                    if state == ControlState::Disabled {
                        theme.palette.text_disabled
                    } else {
                        theme.palette.text
                    },
                )
                .pointer_events(false),
        ),
    );
    list_item_with_content(
        root,
        WidgetContent::new(text, content),
        selected,
        action,
        state,
        theme,
    )
}

/// Selectable shell around arbitrary content. Descendant controls retain their own actions.
/// Use [`list_item_from`] when the caller supplies typed data and an item factory.
#[must_use]
pub fn list_item_with_content<A>(
    root: WidgetIdentity,
    content: WidgetContent<A>,
    selected: bool,
    action: A,
    state: ControlState,
    theme: WidgetTheme,
) -> Widget<A> {
    let text = content.label;
    let enabled = state != ControlState::Disabled;
    let root_id = root.element;
    let background = theme.palette.selectable_background(selected, state);
    let element = Element::container(root.element, root.key)
        .semantic(Semantic::new(SemanticRole::ListItem, &text).selected(selected))
        .styled(
            Style::default()
                .size(
                    Dimension::Px(150.0),
                    Dimension::Px(theme.metrics.control_height),
                )
                .padding(Insets::symmetric(10.0, 0.0))
                .align(Align::Center)
                .background(background)
                .radius(theme.metrics.radius)
                .pointer_events(enabled)
                .focusable(enabled)
                .action_boundary(!enabled),
        );
    Widget {
        element,
        actions: enabled.then_some((root_id, action)).into_iter().collect(),
    }
    .child(content.widget)
}

/// Build one item's content from typed host data, preserving the shared selectable shell.
/// The factory returns an explicit accessible label and a complete subtree with its actions.
#[must_use]
pub fn list_item_from<T, A>(
    root: WidgetIdentity,
    item: &T,
    selected: bool,
    action: A,
    state: ControlState,
    theme: WidgetTheme,
    build: impl FnOnce(&T) -> WidgetContent<A>,
) -> Widget<A> {
    list_item_with_content(root, build(item), selected, action, state, theme)
}

/// A selectable option for a ComboBox popup. The host owns option order,
/// selected value, stable identities, and the typed selection action.
#[must_use]
pub fn combo_option<A>(
    root: WidgetIdentity,
    label: WidgetIdentity,
    text: impl Into<String>,
    selected: bool,
    action: A,
    state: ControlState,
    theme: WidgetTheme,
) -> Widget<A> {
    let mut option = list_item(root, label, text, selected, action, state, theme);
    option.element.style.width = Dimension::Percent(1.0);
    if let Some(semantic) = &mut option.element.semantic {
        semantic.role = SemanticRole::ListBoxOption;
    }
    option
}

#[must_use]
pub fn combo_option_with_content<A>(
    root: WidgetIdentity,
    content: WidgetContent<A>,
    selected: bool,
    action: A,
    state: ControlState,
    theme: WidgetTheme,
) -> Widget<A> {
    let mut option = list_item_with_content(root, content, selected, action, state, theme);
    option.element.style.width = Dimension::Percent(1.0);
    if let Some(semantic) = &mut option.element.semantic {
        semantic.role = SemanticRole::ListBoxOption;
    }
    option
}

#[must_use]
pub fn tree_item<A>(
    ids: TreeItemIds,
    text: impl Into<String>,
    depth: u8,
    expanded: bool,
    action: A,
    state: ControlState,
    theme: WidgetTheme,
) -> Widget<A> {
    let text = text.into();
    let TreeItemIds {
        root,
        disclosure,
        label,
    } = ids;
    let content = Widget::from_element(
        Element::text(label.element, label.key, &text).styled(
            Style::default()
                .size(Dimension::Auto, Dimension::Percent(1.0))
                .text(theme.typography.font_size, theme.palette.text)
                .pointer_events(false),
        ),
    );
    tree_item_with_content(
        TreeRowContentIds { root, disclosure },
        WidgetContent::new(text, content),
        depth,
        expanded,
        action,
        state,
        theme,
    )
}

#[must_use]
pub fn tree_item_with_content<A>(
    ids: TreeRowContentIds,
    mut content: WidgetContent<A>,
    depth: u8,
    expanded: bool,
    action: A,
    state: ControlState,
    theme: WidgetTheme,
) -> Widget<A> {
    content.widget.element.style.min_width = Dimension::Px(0.0);
    content.widget.element.style.flex_grow = 1.0;
    content.widget.element.style.flex_shrink = 1.0;
    let text = content.label;
    let TreeRowContentIds { root, disclosure } = ids;
    let enabled = state != ControlState::Disabled;
    let root_id = root.element;
    let element = Element::container(root.element, root.key)
        .semantic(Semantic::new(SemanticRole::TreeItem, &text).expanded(expanded))
        .styled(
            Style::default()
                .size(
                    Dimension::Px(190.0),
                    Dimension::Px(theme.metrics.control_height),
                )
                .axis(Axis::Row)
                .align(Align::Center)
                .gap(6.0)
                .padding(Insets {
                    left: 8.0 + f32::from(depth) * 14.0,
                    right: 8.0,
                    top: 0.0,
                    bottom: 0.0,
                })
                .background(theme.palette.control_background(state))
                .radius(theme.metrics.radius)
                .pointer_events(enabled)
                .focusable(enabled)
                .action_boundary(!enabled),
        )
        .child(compact_sign(
            disclosure,
            if expanded {
                CompactSign::Minus
            } else {
                CompactSign::Plus
            },
            theme.palette.text_muted,
        ));
    Widget {
        element,
        actions: enabled.then_some((root_id, action)).into_iter().collect(),
    }
    .child(content.widget)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItemIds {
    pub root: WidgetIdentity,
    pub check: WidgetIdentity,
    pub label: WidgetIdentity,
    pub shortcut: WidgetIdentity,
    pub arrow: WidgetIdentity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuItemView<'a> {
    pub label: &'a str,
    pub shortcut: Option<&'a str>,
    pub checked: Option<bool>,
    pub has_submenu: bool,
}

#[must_use]
pub fn menu_item<A>(
    ids: MenuItemIds,
    view: MenuItemView<'_>,
    action: A,
    state: ControlState,
    theme: WidgetTheme,
) -> Widget<A> {
    let MenuItemIds {
        root,
        check,
        label,
        shortcut,
        arrow,
    } = ids;
    let enabled = state != ControlState::Disabled;
    let root_id = root.element;
    let foreground = if enabled {
        theme.palette.text
    } else {
        theme.palette.text_disabled
    };
    let mut check_style = Style::default()
        .size(Dimension::Px(18.0), Dimension::Px(ICON_LINE_SIZE))
        .flex(0.0, 0.0)
        .pointer_events(false);
    if view.checked == Some(true) {
        check_style = check_style
            .polyline(
                vec![
                    Point::new(3.0, 7.0),
                    Point::new(6.0, 10.0),
                    Point::new(12.0, 4.0),
                ],
                ICON_LINE_STROKE,
            )
            .background(if enabled {
                theme.palette.accent
            } else {
                foreground
            });
    }
    let element = Element::container(root.element, root.key)
        .semantic(view.checked.map_or_else(
            || Semantic::new(SemanticRole::MenuItem, view.label),
            |checked| Semantic::new(SemanticRole::MenuItemCheckBox, view.label).checked(checked),
        ))
        .styled(
            Style::default()
                .size(
                    Dimension::Percent(1.0),
                    Dimension::Px(theme.metrics.control_height),
                )
                .axis(Axis::Row)
                .align(Align::Center)
                .gap(8.0)
                .padding(Insets::symmetric(10.0, 0.0))
                .background(
                    if matches!(state, ControlState::Hovered | ControlState::Pressed) {
                        theme.palette.menu_selected
                    } else {
                        theme.palette.menu_surface
                    },
                )
                .radius(theme.metrics.radius.max(4.0))
                .focusable(enabled)
                .pointer_events(enabled)
                .action_boundary(!enabled),
        )
        .child(Element::container(check.element, check.key).styled(check_style))
        .child(
            Element::text(label.element, label.key, view.label).styled(
                Style::default()
                    .size(Dimension::Auto, Dimension::Percent(1.0))
                    .flex(1.0, 1.0)
                    .text(theme.typography.font_size, foreground)
                    .pointer_events(false),
            ),
        )
        .child(
            Element::text(
                shortcut.element,
                shortcut.key,
                view.shortcut.unwrap_or_default(),
            )
            .styled(
                Style::default()
                    .size(Dimension::Auto, Dimension::Percent(1.0))
                    .text(theme.typography.small_font_size, theme.palette.text_muted)
                    .text_align(TextHAlign::End, TextVAlign::Center)
                    .pointer_events(false),
            ),
        )
        .child(
            Element::container(arrow.element, arrow.key).styled(
                Style::default()
                    .size(Dimension::Px(ICON_LINE_SIZE), Dimension::Px(ICON_LINE_SIZE))
                    .flex(0.0, 0.0)
                    .polyline(
                        if view.has_submenu {
                            crate::disclosure_chevron_points(false).to_vec()
                        } else {
                            Vec::new()
                        },
                        ICON_LINE_STROKE,
                    )
                    .background(if enabled {
                        theme.palette.text_muted
                    } else {
                        theme.palette.text_disabled
                    })
                    .pointer_events(false),
            ),
        );
    Widget {
        element,
        actions: enabled.then_some((root_id, action)).into_iter().collect(),
    }
}

#[must_use]
pub fn property_row(
    root: WidgetIdentity,
    name: WidgetIdentity,
    value: WidgetIdentity,
    name_text: impl Into<String>,
    value_text: impl Into<String>,
    theme: WidgetTheme,
) -> Element {
    Element::container(root.element, root.key)
        .styled(
            Style::default()
                .size(
                    Dimension::Px(260.0),
                    Dimension::Px(theme.metrics.control_height),
                )
                .axis(Axis::Row)
                .align(Align::Center)
                .background(theme.palette.field)
                .border(1.0, theme.palette.border)
                .radius(theme.metrics.radius)
                .pointer_events(false),
        )
        .child(
            Element::text(name.element, name.key, name_text).styled(
                Style::default()
                    .size(Dimension::Px(120.0), Dimension::Percent(1.0))
                    .padding(Insets::symmetric(8.0, 0.0))
                    .text(theme.typography.font_size, theme.palette.text_muted)
                    .pointer_events(false),
            ),
        )
        .child(
            Element::text(value.element, value.key, value_text).styled(
                Style::default()
                    .size(Dimension::Auto, Dimension::Percent(1.0))
                    .flex(1.0, 1.0)
                    .padding(Insets::symmetric(8.0, 0.0))
                    .background(theme.palette.surface)
                    .text(theme.typography.font_size, theme.palette.text)
                    .pointer_events(false),
            ),
        )
}

pub const SPLITTER_HIT_SIZE: f32 = 6.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitterIds {
    pub root: WidgetIdentity,
    pub line: WidgetIdentity,
}

#[must_use]
pub fn splitter<A>(
    ids: SplitterIds,
    action: A,
    hovered: bool,
    vertical: bool,
    theme: WidgetTheme,
) -> Widget<A> {
    let root_id = ids.root.element;
    let line_color = if hovered {
        theme.palette.accent
    } else {
        theme.palette.border
    };
    let element = Element::container(ids.root.element, ids.root.key)
        .styled(
            Style::default()
                .size(
                    if vertical {
                        Dimension::Px(SPLITTER_HIT_SIZE)
                    } else {
                        Dimension::Percent(1.0)
                    },
                    if vertical {
                        Dimension::Percent(1.0)
                    } else {
                        Dimension::Px(SPLITTER_HIT_SIZE)
                    },
                )
                .align(Align::Center)
                .justify(Justify::Center)
                .background(theme.palette.panel)
                .focusable(true)
                .flex(0.0, 0.0),
        )
        .semantic(Semantic::new(SemanticRole::Splitter, "Resize panes"))
        .child(
            Element::container(ids.line.element, ids.line.key).styled(
                Style::default()
                    .size(
                        if vertical {
                            Dimension::Px(1.0)
                        } else {
                            Dimension::Percent(1.0)
                        },
                        if vertical {
                            Dimension::Percent(1.0)
                        } else {
                            Dimension::Px(1.0)
                        },
                    )
                    .background(line_color)
                    .pointer_events(false),
            ),
        );
    Widget {
        element,
        actions: vec![(root_id, action)],
    }
}

#[must_use]
pub fn action_for<A>(actions: &[(ElementId, A)], element: ElementId) -> Option<&A> {
    actions
        .iter()
        .find_map(|(candidate, action)| (*candidate == element).then_some(action))
}
