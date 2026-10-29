//! Shared flat application chrome controls. Window actions remain host-owned.

use crate::{
    CompactSign, ControlState, ICON_LINE_SIZE, ICON_LINE_STROKE, Widget, WidgetIdentity,
    WidgetTheme, button, compact_sign,
};
use vesta_gpui::{
    Align, Axis, Dimension, Element, ElementKey, ElementScope, Insets, Point, Semantic,
    SemanticRole, Style, TextHAlign, TextVAlign,
};

pub const CHROME_TITLE_HEIGHT: f32 = 32.0;
pub const CHROME_TAB_HEIGHT: f32 = 32.0;
pub const CHROME_STATUS_HEIGHT: f32 = 28.0;
pub const CHROME_STATUS_CONTROL_WIDTH: f32 = 26.0;
pub const CHROME_WINDOW_CONTROL_WIDTH: f32 = 36.0;

pub struct StatusItem<A> {
    pub widget: Widget<A>,
    pub width: f32,
    pub priority: u8,
    pub overflow_label: String,
    pub overflow_action: Option<A>,
}

pub struct StatusOverflowEntry<A> {
    pub label: String,
    pub action: Option<A>,
}

pub struct StatusPartition<A> {
    pub visible: Vec<Widget<A>>,
    pub overflow: Vec<StatusOverflowEntry<A>>,
}

/// Preserve source order while hiding lower-priority items first.
#[must_use]
pub fn partition_status_items<A>(
    items: Vec<StatusItem<A>>,
    available_width: f32,
    overflow_button_width: f32,
) -> StatusPartition<A> {
    let available_width = if available_width.is_finite() {
        available_width.max(0.0)
    } else {
        0.0
    };
    let mut retained = vec![true; items.len()];
    let mut total = items.iter().map(|item| item.width.max(0.0)).sum::<f32>();
    let mut has_overflow = false;
    while total > available_width {
        let Some(index) = items
            .iter()
            .enumerate()
            .filter(|(index, _)| retained[*index])
            .min_by_key(|(index, item)| (item.priority, std::cmp::Reverse(*index)))
            .map(|(index, _)| index)
        else {
            break;
        };
        retained[index] = false;
        total -= items[index].width.max(0.0);
        if !has_overflow {
            total += overflow_button_width.max(0.0);
            has_overflow = true;
        }
    }
    let mut partition = StatusPartition {
        visible: Vec::new(),
        overflow: Vec::new(),
    };
    for (index, item) in items.into_iter().enumerate() {
        if retained[index] {
            partition.visible.push(item.widget);
        } else {
            partition.overflow.push(StatusOverflowEntry {
                label: item.overflow_label,
                action: item.overflow_action,
            });
        }
    }
    partition
}

#[must_use]
pub fn status_progress<A>(
    root: WidgetIdentity,
    fill: WidgetIdentity,
    value: f32,
    label: &str,
    width: f32,
    theme: WidgetTheme,
) -> Widget<A> {
    let value = if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    };
    Widget::from_element(
        Element::container(root.element, root.key)
            .semantic(
                Semantic::new(SemanticRole::ProgressIndicator, label)
                    .value(format!("{:.0}%", value * 100.0)),
            )
            .styled(
                Style::default()
                    .size(Dimension::Px(width.max(0.0)), Dimension::Px(8.0))
                    .background(theme.palette.field)
                    .pointer_events(false),
            )
            .child(
                Element::container(fill.element, fill.key).styled(
                    Style::default()
                        .size(Dimension::Percent(value), Dimension::Percent(1.0))
                        .background(theme.palette.accent)
                        .pointer_events(false),
                ),
            ),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChromeButtonRole {
    Tab,
    Command,
    Window,
    Status,
}

#[derive(Clone, Copy, Debug)]
pub struct ChromeButtonStyle {
    pub role: ChromeButtonRole,
    pub width: f32,
    pub selected: bool,
    pub hovered: bool,
    pub destructive: bool,
}

fn window_control_points(caption: &str) -> Option<Vec<Point>> {
    match caption {
        "−" => Some(vec![Point::new(1.5, 7.0), Point::new(12.5, 7.0)]),
        "□" => Some(vec![
            Point::new(2.0, 2.0),
            Point::new(12.0, 2.0),
            Point::new(12.0, 12.0),
            Point::new(2.0, 12.0),
            Point::new(2.0, 2.0),
        ]),
        "❐" => Some(vec![
            Point::new(2.0, 4.0),
            Point::new(2.0, 12.0),
            Point::new(10.0, 12.0),
            Point::new(10.0, 4.0),
            Point::new(2.0, 4.0),
            Point::new(5.0, 4.0),
            Point::new(5.0, 2.0),
            Point::new(12.0, 2.0),
            Point::new(12.0, 10.0),
            Point::new(10.0, 10.0),
        ]),
        // The short return to the center retraces the first stroke; the path
        // then draws the second diagonal without adding a visible connector.
        "×" => Some(vec![
            Point::new(2.0, 2.0),
            Point::new(12.0, 12.0),
            Point::new(7.0, 7.0),
            Point::new(12.0, 2.0),
            Point::new(2.0, 12.0),
        ]),
        _ => None,
    }
}

#[must_use]
pub fn chrome_button<A>(
    root: WidgetIdentity,
    label: WidgetIdentity,
    caption: &str,
    accessible_name: &str,
    action: A,
    visual: ChromeButtonStyle,
    theme: WidgetTheme,
) -> Widget<A> {
    let mut control = button(
        root,
        label,
        caption,
        action,
        if visual.selected {
            ControlState::Pressed
        } else if visual.hovered {
            ControlState::Hovered
        } else {
            ControlState::Idle
        },
        theme,
    );
    control.element.semantic = Some(Semantic::new(SemanticRole::Button, accessible_name));
    if matches!(
        visual.role,
        ChromeButtonRole::Status | ChromeButtonRole::Window
    ) {
        control.element.tooltip = Some(accessible_name.to_owned());
    }
    control.element.style.width = Dimension::Px(visual.width);
    control.element.style.height = Dimension::Px(match visual.role {
        ChromeButtonRole::Tab => CHROME_TAB_HEIGHT,
        ChromeButtonRole::Window => CHROME_TITLE_HEIGHT,
        ChromeButtonRole::Command => 30.0,
        ChromeButtonRole::Status => CHROME_STATUS_HEIGHT,
    });
    control.element.style.padding = Insets::all(0.0);
    if let Some(label) = control.element.children.first_mut() {
        let sign = match caption {
            "+" => Some(CompactSign::Plus),
            "−" => Some(CompactSign::Minus),
            _ => None,
        };
        if visual.role == ChromeButtonRole::Window
            && let Some(points) = window_control_points(caption)
        {
            *label = Element::container(label.id, label.key.clone()).styled(
                Style::default()
                    .size(Dimension::Px(ICON_LINE_SIZE), Dimension::Px(ICON_LINE_SIZE))
                    .polyline(points, ICON_LINE_STROKE)
                    .background(theme.palette.text)
                    .pointer_events(false),
            );
        } else if let Some(sign) = sign {
            *label = compact_sign(
                WidgetIdentity {
                    element: label.id,
                    key: label.key.clone(),
                },
                sign,
                theme.palette.text,
            );
        } else {
            if matches!(
                visual.role,
                ChromeButtonRole::Tab | ChromeButtonRole::Command
            ) && caption.chars().count() > 1
            {
                label.style.text_v_align = TextVAlign::Baseline;
            }
            label.style.visual.font_size = match visual.role {
                ChromeButtonRole::Window => 21.0,
                ChromeButtonRole::Status => 18.0,
                _ if caption.chars().count() == 1 => 16.0,
                _ => theme.typography.font_size,
            };
        }
    }
    control.element.style.visual.background = Some(if visual.destructive && visual.hovered {
        theme.palette.danger
    } else if visual.role == ChromeButtonRole::Status && (visual.selected || visual.hovered) {
        theme.palette.chrome_divider
    } else if visual.selected {
        theme.palette.field
    } else if visual.hovered {
        theme.palette.surface_hovered
    } else {
        match visual.role {
            ChromeButtonRole::Status => theme.palette.chrome_status,
            _ => theme.palette.chrome_title,
        }
    });
    control.element.style.visual.border_width = 0.0;
    control.element.style.visual.corner_radius = 0.0;
    control
}

/// Compose host supplied status controls without taking ownership of their state or actions.
#[must_use]
pub fn status_bar<A>(
    scope: ElementScope,
    left: Vec<Widget<A>>,
    right: Vec<Widget<A>>,
    theme: WidgetTheme,
) -> Widget<A> {
    let mut actions = Vec::new();
    let mut root = Element::container(scope.element(1), ElementKey::new("chrome.status")).styled(
        Style::default()
            .size(Dimension::Percent(1.0), Dimension::Px(CHROME_STATUS_HEIGHT))
            .axis(Axis::Row)
            .align(Align::Center)
            .padding(Insets {
                left: 4.0,
                right: 6.0,
                top: 0.0,
                bottom: 0.0,
            })
            .background(theme.palette.chrome_status)
            .pointer_events(true),
    );
    for widget in left {
        actions.extend(widget.actions);
        root = root.child(widget.element);
    }
    root = root.child(
        Element::container(scope.element(3), ElementKey::new("chrome.status.spacer"))
            .styled(Style::default().flex(1.0, 1.0).pointer_events(false)),
    );
    for widget in right {
        actions.extend(widget.actions);
        root = root.child(widget.element);
    }
    root = root.child(
        Element::container(scope.element(2), ElementKey::new("chrome.status.divider")).styled(
            Style::default()
                .absolute(0.0, 0.0)
                .size(Dimension::Percent(1.0), Dimension::Px(1.0))
                .background(theme.palette.chrome_divider)
                .pointer_events(false),
        ),
    );
    Widget {
        element: root,
        actions,
    }
}

/// A flat divider between host-defined groups of status controls.
#[must_use]
pub fn status_separator<A>(
    scope: ElementScope,
    id: u32,
    key: &str,
    theme: WidgetTheme,
) -> Widget<A> {
    Widget::from_element(
        Element::container(scope.element(id), ElementKey::new(key)).styled(
            Style::default()
                .size(Dimension::Px(1.0), Dimension::Px(16.0))
                .background(theme.palette.chrome_divider)
                .pointer_events(false),
        ),
    )
}

#[must_use]
pub fn status_label<A>(
    scope: ElementScope,
    id: u32,
    key: &str,
    caption: &str,
    theme: WidgetTheme,
) -> Widget<A> {
    Widget::from_element(
        Element::text(scope.element(id), ElementKey::new(key), caption)
            .semantic(Semantic::new(SemanticRole::Label, caption))
            .styled(
                Style::default()
                    .size(Dimension::Auto, Dimension::Px(CHROME_STATUS_HEIGHT))
                    .padding(Insets::symmetric(9.0, 0.0))
                    .text(theme.typography.font_size, theme.palette.text_muted)
                    .text_align(TextHAlign::Start, TextVAlign::Center)
                    .pointer_events(false),
            ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vesta_gpui::ElementIdAllocator;

    #[test]
    fn status_partition_keeps_priority_and_overflow_actions_in_source_order() {
        let scope = ElementIdAllocator.allocate_scope().expect("scope");
        let item = |id, width, priority, action| StatusItem {
            widget: status_label(
                scope,
                id,
                "status.test",
                &format!("Item {id}"),
                WidgetTheme::dark(),
            ),
            width,
            priority,
            overflow_label: format!("Item {id}"),
            overflow_action: Some(action),
        };
        let partition = partition_status_items(
            vec![
                item(10, 60.0, 2, 10_u8),
                item(11, 60.0, 0, 11),
                item(12, 60.0, 1, 12),
            ],
            130.0,
            30.0,
        );
        assert_eq!(partition.visible.len(), 1);
        assert_eq!(partition.visible[0].element.id, scope.element(10));
        assert_eq!(
            partition
                .overflow
                .iter()
                .map(|entry| entry.action)
                .collect::<Vec<_>>(),
            vec![Some(11), Some(12)]
        );
    }
}
