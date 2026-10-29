//! Conditional control composition and model-availability input retirement.

use super::*;

fn availability(demo: &DemoState) -> WidgetAvailability {
    WidgetAvailability {
        visible: demo.radio == 0,
        enabled: demo.toggle,
    }
}

pub(super) fn widget(ids: &mut Ids, demo: &DemoState, theme: WidgetTheme) -> Widget<DemoAction> {
    let condition_root = ids.identity("conditional.parent");
    let condition_label = ids.identity("conditional.label");
    let mut compound = Widget::from_element(
        Element::container(condition_root.element, condition_root.key)
            .styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Px(40.0))
                    .max_width(Dimension::Px(430.0))
                    .min_width(Dimension::Px(0.0))
                    .axis(Axis::Row)
                    .align(Align::Center)
                    .gap(10.0)
                    .padding(Insets::symmetric(8.0, 0.0))
                    .background(theme.palette.surface)
                    .border(1.0, theme.palette.border)
                    .radius(theme.metrics.radius)
                    .focusable(true),
            )
            .child(
                Element::text(
                    condition_label.element,
                    condition_label.key,
                    "Parent: toggle",
                )
                .styled(
                    Style::default()
                        .size(Dimension::Px(132.0), Dimension::Percent(1.0))
                        .text(theme.typography.font_size, theme.palette.text)
                        .pointer_events(false),
                ),
            ),
    )
    .on_click(DemoAction::ToggleSwitch);
    let available = availability(demo);
    let child = button(
        ids.identity("conditional.child"),
        ids.identity("conditional.child.label"),
        format!("Child: step {}", demo.number),
        DemoAction::StepNumber(1),
        if available.enabled {
            ControlState::Idle
        } else {
            ControlState::Disabled
        },
        theme,
    )
    .conditioned(available);
    if let Some(child) = child {
        compound = compound.child(child);
    }
    compound
}

impl App {
    pub(super) fn apply_conditional_control_action(&mut self, action: DemoAction) {
        match action {
            DemoAction::ToggleSwitch => self.demo.toggle = !self.demo.toggle,
            DemoAction::SelectRadio(value) => self.demo.radio = value,
            _ => return,
        }
        let available = availability(&self.demo);
        if !available.visible || !available.enabled {
            let owners = std::iter::once(STANDALONE_WINDOW)
                .chain(self.native_windows.values().map(|host| host.ui_window))
                .collect::<Vec<_>>();
            for owner in owners {
                let child = self.gallery_scopes(owner).and_then(|scope| {
                    let route = self.window_actions(owner)?;
                    let root = GallerySectionId::Conditional.element(scope, 0);
                    route.focus_order().iter().copied().find(|id| {
                        route.is_descendant_of(*id, root)
                            && route.action_at(*id) == Some(&DemoAction::StepNumber(1))
                    })
                });
                if let Some(child) = child {
                    self.retire_window_subtree_input(owner, child);
                }
            }
        }
        if let DemoAction::SelectRadio(value) = action
            && let Some(scope) = self.gallery_scopes(self.action_window)
        {
            self.restore_after_rebuild(
                self.action_window,
                Some(scope.element(RADIO_GROUP_ROOT + 1 + u32::from(value) * 4)),
            );
        }
    }
}
