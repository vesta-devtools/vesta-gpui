//! Ordered host actions with group separators and a width-driven overflow list.

use crate::{
    CompactButtonKind, ControlState, IconButtonSpec, Widget, WidgetIdentity, WidgetTheme, button,
    icon_button,
};
use vesta_gpui::{Align, Axis, Dimension, Element, Semantic, SemanticRole, Style};

const ROW_GAP: f32 = 2.0;
const SEPARATOR_WIDTH: f32 = 1.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolbarIds {
    pub root: WidgetIdentity,
    pub row: WidgetIdentity,
    pub overflow: WidgetIdentity,
    pub overflow_label: WidgetIdentity,
    pub hidden: WidgetIdentity,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolbarItem<A> {
    pub root: WidgetIdentity,
    pub glyph: WidgetIdentity,
    pub label: WidgetIdentity,
    pub separator: WidgetIdentity,
    pub icon: String,
    pub name: String,
    pub shortcut: Option<String>,
    pub group: u8,
    pub state: ControlState,
    pub action: A,
}

/// Keep the longest ordered prefix that fits the actual row, including gaps.
/// Below one button's extent, only the minimum-width overflow entry is projected.
/// Invalid widths select no prefix; positive infinity admits all valid items.
#[must_use]
pub fn toolbar_visible_count(groups: &[u8], available_width: f32, item_extent: f32) -> usize {
    if groups.is_empty()
        || !item_extent.is_finite()
        || item_extent <= 0.0
        || available_width.is_nan()
        || available_width < 0.0
    {
        return 0;
    }
    let extent = f64::from(item_extent);
    let gap = f64::from(ROW_GAP);
    let item_width = |index: usize| {
        extent
            + if index == 0 { 0.0 } else { gap }
            + if index > 0 && groups[index - 1] != groups[index] {
                f64::from(SEPARATOR_WIDTH) + gap
            } else {
                0.0
            }
    };
    let total = (0..groups.len()).map(item_width).sum::<f64>();
    if total <= f64::from(available_width) {
        return groups.len();
    }
    let mut occupied = 0.0;
    let mut count = 0;
    for index in 0..groups.len() {
        let width = item_width(index);
        if occupied + width + gap + extent > f64::from(available_width) {
            break;
        }
        occupied += width;
        count += 1;
    }
    count
}

#[must_use]
pub fn toolbar<A>(
    ids: ToolbarIds,
    items: Vec<ToolbarItem<A>>,
    available_width: f32,
    overflow_expanded: bool,
    overflow_action: A,
    theme: WidgetTheme,
) -> Widget<A> {
    let visible = toolbar_visible_count(
        &items.iter().map(|item| item.group).collect::<Vec<_>>(),
        available_width,
        theme.metrics.control_height + 6.0,
    );
    let has_overflow = visible < items.len();
    let mut row = Widget::from_element(
        Element::container(ids.row.element, ids.row.key).styled(
            Style::default()
                .axis(Axis::Row)
                .align(Align::Center)
                .gap(ROW_GAP),
        ),
    );
    let mut hidden = Widget::from_element(
        Element::container(ids.hidden.element, ids.hidden.key)
            .styled(Style::default().axis(Axis::Column).gap(2.0)),
    );
    let mut previous_group = None;
    for (index, item) in items.into_iter().enumerate() {
        if index >= visible && !overflow_expanded {
            continue;
        }
        if index < visible {
            if previous_group.is_some_and(|group| group != item.group) {
                row = row.child(Widget::from_element(
                    Element::container(item.separator.element, item.separator.key).styled(
                        Style::default()
                            .size(Dimension::Px(SEPARATOR_WIDTH), Dimension::Px(18.0))
                            .flex(0.0, 0.0)
                            .background(theme.palette.border)
                            .pointer_events(false),
                    ),
                ));
            }
            previous_group = Some(item.group);
            let mut button = icon_button(
                item.root,
                item.glyph,
                IconButtonSpec {
                    icon: &item.icon,
                    name: &item.name,
                    shortcut: item.shortcut.as_deref(),
                    kind: CompactButtonKind::Toolbar,
                    state: item.state,
                },
                item.action,
                theme,
            );
            button.element.style.flex_shrink = 0.0;
            row = row.child(button);
        } else {
            let mut button = button(
                item.root,
                item.label,
                &item.name,
                item.action,
                item.state,
                theme,
            );
            button.element.tooltip = Some(match item.shortcut {
                Some(shortcut) => format!("{} ({shortcut})", item.name),
                None => item.name,
            });
            hidden = hidden.child(button);
        }
    }
    if has_overflow {
        row = row.child(
            Widget::from_element(
                Element::container(ids.overflow.element, ids.overflow.key)
                    .semantic(
                        Semantic::new(SemanticRole::Button, "More toolbar actions")
                            .expanded(overflow_expanded),
                    )
                    .styled(
                        Style::default()
                            .size(
                                Dimension::Px(theme.metrics.control_height + 6.0),
                                Dimension::Px(theme.metrics.control_height),
                            )
                            .flex(0.0, 0.0)
                            .align(Align::Center)
                            .background(theme.palette.surface)
                            .focusable(true),
                    )
                    .child(
                        Element::text(ids.overflow_label.element, ids.overflow_label.key, "…")
                            .styled(
                                Style::default()
                                    .text(theme.typography.font_size, theme.palette.text)
                                    .pointer_events(false),
                            ),
                    ),
            )
            .on_click(overflow_action),
        );
    }
    Widget::from_element(
        Element::container(ids.root.element, ids.root.key)
            .styled(Style::default().axis(Axis::Column).gap(2.0)),
    )
    .child(row)
    .child(hidden)
}
