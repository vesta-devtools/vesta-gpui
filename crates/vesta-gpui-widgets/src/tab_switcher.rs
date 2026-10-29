//! Controlled page/scope navigation composed from the ordinary tab projection.
use crate::{ControlState, Widget, WidgetIdentity, WidgetTheme, tab};
use vesta_gpui::{Align, Axis, Color, Dimension, Element, Insets, Semantic, SemanticRole, Style};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabSwitcherStyle {
    #[default]
    Underline,
    Segmented,
}

/// Identities and actions belong to the host; no selection is retained here.
pub struct TabSwitcherEntry<A> {
    pub root: WidgetIdentity,
    pub label: WidgetIdentity,
    pub indicator: WidgetIdentity,
    pub text: String,
    pub selected: bool,
    pub state: ControlState,
    pub action: A,
}

/// Each enabled tab retains ordinary Tab/Enter/Space activation. Navigation and
/// selection are the host's typed actions, independent of the presentation.
#[must_use]
pub fn tab_switcher<A>(
    root: WidgetIdentity,
    name: impl Into<String>,
    entries: Vec<TabSwitcherEntry<A>>,
    presentation: TabSwitcherStyle,
    theme: WidgetTheme,
) -> Widget<A> {
    let segmented = presentation == TabSwitcherStyle::Segmented;
    let mut group = Widget::from_element(
        Element::container(root.element, root.key)
            .semantic(Semantic::new(SemanticRole::Group, name))
            .styled(
                Style::default()
                    .axis(Axis::Row)
                    .align(Align::Center)
                    .align_self(Align::Start)
                    .size(Dimension::Auto, Dimension::Px(32.0))
                    .flex(0.0, 0.0)
                    .gap(if segmented { 2.0 } else { 4.0 })
                    .padding(Insets::all(if segmented { 3.0 } else { 0.0 }))
                    .background(if segmented {
                        theme.palette.field
                    } else {
                        Color::srgba(0.0, 0.0, 0.0, 0.0)
                    })
                    .radius(if segmented { 5.0 } else { 0.0 }),
            ),
    );
    for entry in entries {
        let enabled = entry.state != ControlState::Disabled;
        let mut item = tab(
            entry.root,
            entry.label,
            entry.text,
            entry.selected,
            entry.action,
            entry.state,
            theme,
        );
        let active = enabled && entry.selected;
        let fill = if entry.state == ControlState::Hovered {
            theme.palette.surface_hovered
        } else if segmented && active {
            theme.palette.surface
        } else {
            Color::srgba(0.0, 0.0, 0.0, 0.0)
        };
        item.element.style = item
            .element
            .style
            .size(
                Dimension::Auto,
                Dimension::Px(if segmented { 26.0 } else { 32.0 }),
            )
            .flex(0.0, 0.0)
            .padding(Insets::symmetric(14.0, 0.0))
            .background(fill)
            .radius(if segmented { 3.0 } else { 0.0 });
        if let Some(label) = item.element.children.first_mut() {
            label.style.visual.text_color = if !enabled {
                theme.palette.text_disabled
            } else if active || entry.state == ControlState::Hovered {
                theme.palette.text
            } else {
                theme.palette.text_muted
            };
        }
        if !segmented && active {
            item.element.children.push(
                Element::container(entry.indicator.element, entry.indicator.key).styled(
                    Style::default()
                        .absolute(0.0, 30.0)
                        .size(Dimension::Percent(1.0), Dimension::Px(2.0))
                        .background(theme.palette.accent)
                        .pointer_events(false),
                ),
            );
        }
        group = group.child(item);
    }
    group
}

#[cfg(test)]
mod tests {
    use super::*;
    use vesta_gpui::{ActionRoute, ElementIdAllocator, LayoutEngine, Viewport};

    #[test]
    fn presentation_keeps_stable_actions_and_disabled_boundaries() {
        let scope = ElementIdAllocator.allocate_scope().expect("switcher scope");
        let id = |local| WidgetIdentity::new(scope.element(local), format!("switcher.{local}"));
        for theme in [WidgetTheme::dark(), WidgetTheme::light()] {
            for presentation in [TabSwitcherStyle::Underline, TabSwitcherStyle::Segmented] {
                for selected in [false, true] {
                    let entries = [ControlState::Idle, ControlState::Disabled]
                        .into_iter()
                        .enumerate()
                        .map(|(index, state)| {
                            let local = 10 + index as u32 * 3;
                            TabSwitcherEntry {
                                root: id(local),
                                label: id(local + 1),
                                indicator: id(local + 2),
                                text: "快捷键 / Shortcuts".into(),
                                selected,
                                state,
                                action: index,
                            }
                        })
                        .collect();
                    let widget =
                        tab_switcher(id(1), "Settings", entries, presentation, theme).on_click(99);
                    let layout = LayoutEngine
                        .compute(&widget.element, Viewport::new(600.0, 40.0, 1.25))
                        .expect("switcher layout");
                    let route = ActionRoute::build(&widget.element, widget.actions)
                        .expect("switcher route");
                    assert_eq!(route.action_for_hit(id(11).element), Some(&0));
                    assert_eq!(route.action_for_hit(id(14).element), None);
                    assert!(route.focus_order().contains(&id(10).element));
                    assert!(!route.focus_order().contains(&id(13).element));
                    assert!(layout.node(id(10).element).is_some());
                    assert_eq!(
                        widget.element.children[0]
                            .semantic
                            .as_ref()
                            .expect("tab semantics")
                            .selected,
                        Some(selected)
                    );
                    if selected && presentation == TabSwitcherStyle::Underline {
                        assert_eq!(route.action_for_hit(id(12).element), Some(&0));
                    }
                }
            }
        }
    }
}
