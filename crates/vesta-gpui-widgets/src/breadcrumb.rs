//! Host-owned location path with stable ancestor actions and width overflow.

use crate::{Widget, WidgetIdentity, WidgetTheme};
use vesta_gpui::{Align, Axis, Dimension, Element, Insets, Semantic, SemanticRole, Style};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BreadcrumbIds {
    pub root: WidgetIdentity,
    pub row: WidgetIdentity,
    pub overflow: WidgetIdentity,
    pub overflow_label: WidgetIdentity,
    pub hidden: WidgetIdentity,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BreadcrumbSegment<A> {
    pub root: WidgetIdentity,
    pub separator: WidgetIdentity,
    pub text: WidgetIdentity,
    pub label: String,
    pub action: Option<A>,
}

/// Indices retained in the first row. The first and current locations stay visible.
#[must_use]
pub fn breadcrumb_visible_indices(labels: &[&str], available_width: f32) -> Vec<usize> {
    if labels.is_empty() {
        return Vec::new();
    }
    let width = labels
        .iter()
        .map(|label| label.chars().count() as f32 * 8.0 + 20.0)
        .sum::<f32>()
        + labels.len().saturating_sub(1) as f32 * 14.0;
    if labels.len() <= 2 || width <= available_width {
        return (0..labels.len()).collect();
    }
    vec![0, labels.len() - 1]
}

/// The host supplies the current path, its actions, and overflow expansion state.
#[must_use]
pub fn breadcrumb<A>(
    ids: BreadcrumbIds,
    segments: Vec<BreadcrumbSegment<A>>,
    available_width: f32,
    overflow_expanded: bool,
    overflow_action: A,
    theme: WidgetTheme,
) -> Widget<A> {
    let visible = breadcrumb_visible_indices(
        &segments
            .iter()
            .map(|part| part.label.as_str())
            .collect::<Vec<_>>(),
        available_width,
    );
    let collapsed = visible.len() != segments.len();
    let current_index = segments.len().saturating_sub(1);
    let mut row = Widget::from_element(
        Element::container(ids.row.element, ids.row.key).styled(
            Style::default()
                .axis(Axis::Row)
                .align(Align::Center)
                .gap(3.0),
        ),
    );
    let mut hidden = Widget::from_element(
        Element::container(ids.hidden.element, ids.hidden.key)
            .styled(Style::default().axis(Axis::Column).gap(2.0)),
    );
    let mut overflow = collapsed.then(|| {
        Widget::from_element(
            Element::container(ids.overflow.element, ids.overflow.key)
                .semantic(
                    Semantic::new(SemanticRole::Button, "Path ancestors")
                        .expanded(overflow_expanded),
                )
                .styled(
                    Style::default()
                        .size(
                            Dimension::Px(28.0),
                            Dimension::Px(theme.metrics.control_height),
                        )
                        .align(Align::Center)
                        .padding(Insets::symmetric(4.0, 0.0))
                        .background(theme.palette.surface)
                        .focusable(true),
                )
                .child(
                    Element::text(ids.overflow_label.element, ids.overflow_label.key, "›  …")
                        .styled(
                            Style::default()
                                .text(theme.typography.font_size, theme.palette.text)
                                .pointer_events(false),
                        ),
                ),
        )
        .on_click(overflow_action)
    });
    for (index, segment) in segments.into_iter().enumerate() {
        if index == 1
            && let Some(overflow) = overflow.take()
        {
            row = row.child(overflow);
        }
        let is_hidden = collapsed && !visible.contains(&index);
        if is_hidden && !overflow_expanded {
            continue;
        }
        let is_current = index == current_index;
        let enabled = segment.action.is_some() && !is_current;
        let mut part = Widget::from_element(
            Element::container(segment.root.element, segment.root.key)
                .semantic(Semantic::new(
                    if enabled {
                        SemanticRole::Button
                    } else {
                        SemanticRole::Label
                    },
                    &segment.label,
                ))
                .styled(
                    Style::default()
                        .size(Dimension::Auto, Dimension::Px(theme.metrics.control_height))
                        .axis(Axis::Row)
                        .align(Align::Center)
                        .padding(Insets::symmetric(6.0, 0.0))
                        .background(if enabled {
                            theme.palette.surface
                        } else {
                            theme.palette.panel
                        })
                        .focusable(enabled)
                        .pointer_events(enabled)
                        .action_boundary(!enabled),
                )
                .children(
                    (index > 0 && !is_hidden)
                        .then(|| {
                            Element::text(segment.separator.element, segment.separator.key, "›")
                                .styled(
                                    Style::default()
                                        .text(theme.typography.font_size, theme.palette.text_muted)
                                        .pointer_events(false),
                                )
                        })
                        .into_iter(),
                )
                .child(
                    Element::text(segment.text.element, segment.text.key, segment.label).styled(
                        Style::default()
                            .text(theme.typography.font_size, theme.palette.text)
                            .pointer_events(false),
                    ),
                ),
        );
        if let Some(action) = segment.action.filter(|_| enabled) {
            part = part.on_click(action);
        }
        if is_hidden {
            hidden = hidden.child(part);
        } else {
            row = row.child(part);
        }
    }
    Widget::from_element(
        Element::container(ids.root.element, ids.root.key)
            .styled(Style::default().axis(Axis::Column).gap(3.0)),
    )
    .child(row)
    .child(hidden)
}
