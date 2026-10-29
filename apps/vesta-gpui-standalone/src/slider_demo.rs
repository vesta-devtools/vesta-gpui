//! Slider composition, availability and window-owned value edit intents.

use super::*;
use vesta_gpui::FlexWrap;

const ENABLE_ROOT: u32 = 210_000;

pub(super) fn enable_target(scope: ElementScope) -> ElementId {
    scope.element(ENABLE_ROOT)
}

pub(super) fn row(
    ids: SliderIds,
    demo: &DemoState,
    scope: ElementScope,
    theme: WidgetTheme,
) -> Widget<DemoAction> {
    let identity = |slot, part| {
        WidgetIdentity::new(
            scope.element(ENABLE_ROOT + slot),
            format!("gallery.slider.enabled.{part}"),
        )
    };
    let root = identity(4, "row");
    Widget::from_element(
        Element::container(root.element, root.key).styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Auto)
                .min_width(Dimension::Px(0.0))
                .axis(Axis::Row)
                .flex_wrap(FlexWrap::Wrap)
                .align(Align::Center)
                .gap(8.0),
        ),
    )
    .child(slider(
        ids,
        demo.slider,
        DemoAction::SetSlider,
        if demo.slider_enabled {
            ControlState::Idle
        } else {
            ControlState::Disabled
        },
        theme,
    ))
    .child(toggle(
        ToggleIds {
            root: identity(0, "root"),
            track: identity(1, "track"),
            knob: identity(2, "knob"),
            label: identity(3, "label"),
        },
        "Enable slider",
        demo.slider_enabled,
        DemoAction::ToggleSliderEnabled,
        ControlState::Idle,
        theme,
    ))
}

impl App {
    pub(super) fn toggle_slider_enabled(&mut self) {
        self.demo.slider_enabled = !self.demo.slider_enabled;
        if self.demo.slider_enabled {
            return;
        }
        let owners = std::iter::once(STANDALONE_WINDOW)
            .chain(self.native_windows.values().map(|host| host.ui_window))
            .collect::<Vec<_>>();
        for owner in owners {
            let Some(scope) = self.gallery_scopes(owner) else {
                continue;
            };
            let target = self.window_actions(owner).and_then(|route| {
                let body = GallerySectionId::Feedback.element(scope, 3);
                route.focus_order().iter().copied().find(|element| {
                    route.is_descendant_of(*element, body)
                        && route.action_at(*element) == Some(&DemoAction::SetSlider)
                })
            });
            let Some(target) = target else {
                continue;
            };
            let lost_focus = self.window_focus(owner) == Some(target);
            self.retire_window_subtree_input(owner, target);
            let enable = enable_target(scope);
            if lost_focus
                && self.window_actions(owner).is_some_and(|route| {
                    route.action_at(enable) == Some(&DemoAction::ToggleSliderEnabled)
                })
            {
                self.set_window_focus(owner, Some(enable));
                self.reveal_window_focus(owner, enable);
            }
        }
    }

    pub(super) fn set_slider_value(&mut self, value: f32) {
        if self.demo.slider_enabled {
            self.demo.slider = value.clamp(0.0, 1.0);
        }
    }

    pub(super) fn begin_slider_edit(&mut self) {
        if !self.demo.slider_enabled {
            return;
        }
        let initial = self.demo.slider;
        let preview = if self.action_window == STANDALONE_WINDOW {
            self.input_ownership
                .focus()
                .zip(self.slider_rect)
                .map(|(target, rect)| {
                    self.slider_drag = Some(ValueEditSession::new(target, initial));
                    self.input_ownership
                        .capture_pointer(target, PointerButton::Primary);
                    slider_value_at(rect, self.cursor)
                })
        } else {
            self.native_windows
                .values_mut()
                .find(|host| host.ui_window == self.action_window)
                .and_then(|host| {
                    host.input
                        .focus()
                        .zip(host.slider_rect)
                        .map(|(target, rect)| {
                            host.slider_drag = Some(ValueEditSession::new(target, initial));
                            host.input.capture_pointer(target, PointerButton::Primary);
                            slider_value_at(rect, host.cursor)
                        })
                })
        };
        if let Some(value) = preview {
            self.apply_demo_action(DemoAction::SetSliderValue(value));
        }
    }
}
