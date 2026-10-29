//! Controlled side region composed from host-owned content.

use crate::{ControlState, Widget, WidgetIdentity, WidgetTheme, button};
use vesta_gpui::{Align, Axis, Dimension, Element, Insets, Semantic, SemanticRole, Style};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidebarDrawerIds {
    pub root: WidgetIdentity,
    pub toggle: WidgetIdentity,
    pub toggle_label: WidgetIdentity,
    pub body: WidgetIdentity,
    pub resize: WidgetIdentity,
    pub main: WidgetIdentity,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SidebarDrawerState<'a> {
    pub title: &'a str,
    pub open: bool,
    pub width: f32,
    pub min_width: f32,
    pub max_width: f32,
    pub keyboard_step: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarDrawerResizeKey {
    Decrease,
    Increase,
    PageDecrease,
    PageIncrease,
    Minimum,
    Maximum,
}

fn resize_bounds(state: SidebarDrawerState<'_>) -> (f32, f32, f32) {
    if state.min_width.is_finite()
        && state.max_width.is_finite()
        && state.keyboard_step.is_finite()
        && state.min_width > 0.0
        && state.max_width >= state.min_width
        && state.keyboard_step > 0.0
    {
        (state.min_width, state.max_width, state.keyboard_step)
    } else {
        (100.0, 320.0, 8.0)
    }
}

#[must_use]
pub fn sidebar_drawer_width_at(
    start_width: f32,
    pointer_delta: f32,
    state: SidebarDrawerState<'_>,
) -> f32 {
    let (min_width, max_width, _) = resize_bounds(state);
    (start_width + pointer_delta).clamp(min_width, max_width)
}

#[must_use]
pub fn sidebar_drawer_width_for_key(
    width: f32,
    key: SidebarDrawerResizeKey,
    state: SidebarDrawerState<'_>,
) -> f32 {
    let (min_width, max_width, step) = resize_bounds(state);
    let delta = match key {
        SidebarDrawerResizeKey::Decrease => -step,
        SidebarDrawerResizeKey::Increase => step,
        SidebarDrawerResizeKey::PageDecrease => -step * 4.0,
        SidebarDrawerResizeKey::PageIncrease => step * 4.0,
        SidebarDrawerResizeKey::Minimum => return min_width,
        SidebarDrawerResizeKey::Maximum => return max_width,
    };
    (width + delta).clamp(min_width, max_width)
}

/// Compose a side region with a stable toggle and an absent body when closed.
#[must_use]
pub fn sidebar_drawer<A>(
    ids: SidebarDrawerIds,
    state: SidebarDrawerState<'_>,
    body_content: Widget<A>,
    main_content: Widget<A>,
    toggle_action: A,
    theme: WidgetTheme,
) -> Widget<A> {
    let (min_width, max_width, keyboard_step) = resize_bounds(state);
    let width = if state.width.is_finite() {
        state.width.clamp(min_width, max_width)
    } else {
        min_width
    };
    let mut toggle = button(
        ids.toggle,
        ids.toggle_label,
        if state.open { "‹" } else { "›" },
        toggle_action,
        ControlState::Idle,
        theme,
    );
    toggle.element.semantic =
        Some(Semantic::new(SemanticRole::Button, state.title).expanded(state.open));
    toggle.element.style.width = Dimension::Px(30.0);
    toggle.element.style.padding = Insets::all(0.0);
    let mut root = Widget::from_element(
        Element::container(ids.root.element, ids.root.key)
            .semantic(Semantic::new(SemanticRole::Group, state.title))
            .styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Px(150.0))
                    .max_width(Dimension::Px(440.0))
                    .axis(Axis::Row)
                    .align(Align::Start)
                    .background(theme.palette.field)
                    .border(1.0, theme.palette.border),
            ),
    )
    .child(toggle);
    if state.open {
        root = root.child(
            Widget::from_element(
                Element::container(ids.body.element, ids.body.key).styled(
                    Style::default()
                        .size(Dimension::Px(width), Dimension::Percent(1.0))
                        .axis(Axis::Column)
                        .gap(6.0)
                        .padding(Insets::all(8.0))
                        .background(theme.palette.panel),
                ),
            )
            .child(body_content),
        );
        root = root.child(Widget::from_element(
            Element::container(ids.resize.element, ids.resize.key)
                .semantic(
                    Semantic::new(SemanticRole::Splitter, format!("Resize {}", state.title))
                        .numeric(
                            f64::from(width),
                            f64::from(min_width),
                            f64::from(max_width),
                            Some(f64::from(keyboard_step)),
                        ),
                )
                .styled(
                    Style::default()
                        .size(Dimension::Px(6.0), Dimension::Percent(1.0))
                        .background(theme.palette.border)
                        .focusable(true),
                ),
        ));
    }
    root.child(
        Widget::from_element(
            Element::container(ids.main.element, ids.main.key).styled(
                Style::default()
                    .size(Dimension::Auto, Dimension::Percent(1.0))
                    .min_width(Dimension::Px(0.0))
                    .flex(1.0, 1.0)
                    .padding(Insets::all(8.0)),
            ),
        )
        .child(main_content),
    )
}
