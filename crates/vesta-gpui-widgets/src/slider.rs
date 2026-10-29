//! Shared slider presentation and geometry for fixed or measured Host widths.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SliderIds {
    pub root: WidgetIdentity,
    pub track: WidgetIdentity,
    pub fill: WidgetIdentity,
    pub thumb: WidgetIdentity,
    pub value_label: WidgetIdentity,
}

const SLIDER_THUMB_SIZE: f32 = 14.0;

/// Fit the standard slider to a measured track width. Keep pointer conversion
/// and painted thumb travel under the same widget authority.
pub fn resize_slider(root: &mut Element, ids: &SliderIds, value: f32, track_width: f32) {
    if !track_width.is_finite() || !value.is_finite() {
        return;
    }
    let width = track_width.max(SLIDER_THUMB_SIZE);
    fn fit(root: &mut Element, ids: &SliderIds, value: f32, width: f32) {
        if root.id == ids.root.element {
            root.style.width = Dimension::Px(width + 70.0);
        }
        if root.id == ids.track.element {
            root.style.width = Dimension::Px(width);
            root.style.flex_shrink = 0.0;
        }
        if root.id == ids.thumb.element {
            root.style = root
                .style
                .clone()
                .absolute((width - SLIDER_THUMB_SIZE) * value.clamp(0.0, 1.0), -4.5);
        }
        for child in &mut root.children {
            fit(child, ids, value, width);
        }
    }
    fit(root, ids, value, width);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliderKey {
    Decrease,
    Increase,
    PageDecrease,
    PageIncrease,
    Start,
    End,
}

/// Convert a pointer over the painted track to the slider's normalized value.
#[must_use]
pub fn slider_value_at(track: Rect, pointer: Point) -> f32 {
    let travel = (track.width - SLIDER_THUMB_SIZE).max(1.0);
    ((pointer.x - track.x - SLIDER_THUMB_SIZE * 0.5) / travel).clamp(0.0, 1.0)
}

#[must_use]
pub fn slider_value_for_key(value: f32, key: SliderKey, step: f32) -> f32 {
    let step = if step.is_finite() && step > 0.0 {
        step.min(1.0)
    } else {
        0.05
    };
    match key {
        SliderKey::Decrease => (value - step).clamp(0.0, 1.0),
        SliderKey::Increase => (value + step).clamp(0.0, 1.0),
        SliderKey::PageDecrease => (value - step * 10.0).clamp(0.0, 1.0),
        SliderKey::PageIncrease => (value + step * 10.0).clamp(0.0, 1.0),
        SliderKey::Start => 0.0,
        SliderKey::End => 1.0,
    }
}

#[must_use]
pub fn slider<A>(
    ids: SliderIds,
    value: f32,
    action: A,
    state: ControlState,
    theme: WidgetTheme,
) -> Widget<A> {
    let SliderIds {
        root,
        track,
        fill,
        thumb,
        value_label,
    } = ids;
    let enabled = state != ControlState::Disabled;
    let value = value.clamp(0.0, 1.0);
    let root_id = root.element;
    let track_width = 150.0;
    let element = Element::container(root.element, root.key)
        .semantic(
            Semantic::new(SemanticRole::Slider, "Value")
                .value(format!("{value:.2}"))
                .numeric(f64::from(value), 0.0, 1.0, Some(0.01)),
        )
        .styled(
            Style::default()
                .size(
                    Dimension::Px(220.0),
                    Dimension::Px(theme.metrics.control_height),
                )
                .axis(vesta_gpui::Axis::Row)
                .align(Align::Center)
                .gap(10.0)
                .pointer_events(enabled)
                .focusable(enabled)
                .action_boundary(!enabled),
        )
        .child(
            Element::container(track.element, track.key)
                .styled(
                    Style::default()
                        .size(Dimension::Px(track_width), Dimension::Px(5.0))
                        .background(if enabled {
                            theme.palette.surface_pressed
                        } else {
                            theme.palette.surface_disabled
                        })
                        .radius(2.5)
                        .pointer_events(false),
                )
                .child(
                    Element::container(fill.element, fill.key).styled(
                        Style::default()
                            .absolute(0.0, 0.0)
                            .size(Dimension::Percent(value), Dimension::Percent(1.0))
                            .background(if enabled {
                                theme.palette.accent
                            } else {
                                theme.palette.text_disabled
                            })
                            .radius(2.5)
                            .pointer_events(false),
                    ),
                )
                .child(
                    Element::container(thumb.element, thumb.key).styled(
                        Style::default()
                            .absolute((track_width - SLIDER_THUMB_SIZE) * value, -4.5)
                            .size(
                                Dimension::Px(SLIDER_THUMB_SIZE),
                                Dimension::Px(SLIDER_THUMB_SIZE),
                            )
                            .background(if enabled {
                                theme.palette.focus
                            } else {
                                theme.palette.text_disabled
                            })
                            .border(2.0, theme.palette.panel)
                            .radius(7.0)
                            .pointer_events(false),
                    ),
                ),
        )
        .child(
            Element::text(value_label.element, value_label.key, format!("{value:.2}")).styled(
                Style::default()
                    .size(
                        Dimension::Px(52.0),
                        Dimension::Px(theme.metrics.control_height),
                    )
                    .text(
                        theme.typography.small_font_size,
                        if enabled {
                            theme.palette.text_muted
                        } else {
                            theme.palette.text_disabled
                        },
                    )
                    .text_align(TextHAlign::End, TextVAlign::Center)
                    .pointer_events(false),
            ),
        );
    Widget {
        element,
        actions: enabled.then_some((root_id, action)).into_iter().collect(),
    }
}
