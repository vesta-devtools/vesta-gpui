//! One ColorPicker projection shared by the sample swatch and ramp points.
use super::*;

impl DemoState {
    fn color_channel_values(&self) -> [String; 4] {
        match self.color_mode {
            ColorPickerMode::Rgb => self.color_draft.map(|value| value.to_string()),
            ColorPickerMode::Hsv => rgba_to_hsva(self.color_draft).map(|value| value.to_string()),
        }
    }
}

pub(super) fn build_color_picker(
    input: &UiBuildState<'_>,
    target: ColorEditTarget,
    actions: &mut Vec<(ElementId, DemoAction)>,
) -> Element {
    let scope = input.scopes.gallery;
    let demo = input.demo;
    let theme = input.theme;
    let base = match target {
        ColorEditTarget::Swatch => COLOR_PICKER_ROOT,
        ColorEditTarget::RampPoint(_) => RAMP_ROOT + 10,
    };
    let expanded = demo.color_open
        && demo.color_target == target
        && demo
            .color_session
            .as_ref()
            .is_some_and(|session| session.accepts(input.ui_window));
    let rgba = if expanded {
        demo.color_draft
    } else {
        match target {
            ColorEditTarget::Swatch => demo.color_committed,
            ColorEditTarget::RampPoint(id) => {
                demo.ramp.point(id).map_or([0; 4], |point| point.rgba)
            }
        }
    };
    let color_identity = |slot, part: &str| {
        WidgetIdentity::new(
            scope.element(base + slot),
            match target {
                ColorEditTarget::Swatch => format!("gallery.color.{part}"),
                ColorEditTarget::RampPoint(_) => format!("gallery.ramp.color.{part}"),
            },
        )
    };
    let color_channels = std::array::from_fn(|index| {
        let base = 20 + index as u32 * 10;
        NumberFieldIds {
            root: color_identity(base, &format!("channel.{index}")),
            value_label: color_identity(base + 1, &format!("channel.{index}.value")),
            decrement: color_identity(base + 2, &format!("channel.{index}.decrement")),
            decrement_label: color_identity(base + 3, &format!("channel.{index}.decrement.label")),
            increment: color_identity(base + 4, &format!("channel.{index}.increment")),
            increment_label: color_identity(base + 5, &format!("channel.{index}.increment.label")),
        }
    });
    widget_element(
        color_picker(
            ColorPickerIds {
                root: color_identity(0, "root"),
                swatch: color_identity(1, "swatch"),
                swatch_label: color_identity(2, "swatch.label"),
                panel: color_identity(3, "panel"),
                hex: WidgetIdentity::new(scope.element(COLOR_HEX_FIELD), "gallery.color.hex"),
                modes: color_identity(60, "modes"),
                mode_choices: [
                    RadioIds {
                        root: color_identity(61, "mode.rgb"),
                        ring: color_identity(62, "mode.rgb.ring"),
                        dot: color_identity(63, "mode.rgb.dot"),
                        label: color_identity(64, "mode.rgb.label"),
                    },
                    RadioIds {
                        root: color_identity(65, "mode.hsv"),
                        ring: color_identity(66, "mode.hsv.ring"),
                        dot: color_identity(67, "mode.hsv.dot"),
                        label: color_identity(68, "mode.hsv.label"),
                    },
                ],
                channel_rows: std::array::from_fn(|index| {
                    let slot = if index < 3 { 10 + index as u32 } else { 16 };
                    color_identity(slot, &format!("channel.{index}.row"))
                }),
                channel_labels: std::array::from_fn(|index| {
                    let slot = if index < 3 { 13 + index as u32 } else { 17 };
                    color_identity(slot, &format!("channel.{index}.label"))
                }),
                channels: color_channels,
                eyedropper: color_identity(69, "eyedropper"),
                eyedropper_label: color_identity(70, "eyedropper.label"),
                buttons: color_identity(5, "buttons"),
                confirm: color_identity(6, "apply"),
                confirm_label: color_identity(7, "apply.label"),
                cancel: color_identity(8, "cancel"),
                cancel_label: color_identity(9, "cancel.label"),
            },
            ColorPickerView {
                draft: rgba,
                mode: demo.color_mode,
                channel_values: demo.color_channel_values(),
                hex_snapshot: input.color_hex_snapshot.clone(),
                hex_focused: input.color_hex_focused,
                expanded,
            },
            ColorPickerActions {
                toggle: match target {
                    ColorEditTarget::Swatch => DemoAction::ToggleColorPicker,
                    ColorEditTarget::RampPoint(_) => DemoAction::RampEditColor,
                },
                select_mode: [
                    DemoAction::SelectColorMode(ColorPickerMode::Rgb),
                    DemoAction::SelectColorMode(ColorPickerMode::Hsv),
                ],
                decrease: std::array::from_fn(|index| DemoAction::StepColor(index as u8, -8)),
                increase: std::array::from_fn(|index| DemoAction::StepColor(index as u8, 8)),
                eyedropper: DemoAction::PickColor,
                confirm: DemoAction::ApplyColor,
                cancel: DemoAction::CancelColor,
            },
            theme,
        ),
        actions,
    )
}

impl App {
    pub(super) fn accepts_color_action(&self, action: DemoAction) -> bool {
        !matches!(
            action,
            DemoAction::ApplyColor
                | DemoAction::CancelColor
                | DemoAction::StepColor(..)
                | DemoAction::SelectColorMode(_)
                | DemoAction::PickColor
        ) || self.can_edit_color(self.action_window)
    }

    pub(super) fn can_edit_color(&self, window: UiWindowId) -> bool {
        self.demo.color_open
            && self
                .demo
                .color_session
                .as_ref()
                .is_some_and(|session| session.accepts(window))
    }

    pub(super) fn open_ramp_color(&mut self) {
        if self.demo.color_open && matches!(self.demo.color_target, ColorEditTarget::RampPoint(_)) {
            self.close_color_editor();
            return;
        }
        if !self.demo.ramp.selection.contains(&self.demo.ramp.primary) {
            return;
        }
        let Some(point) = self.demo.ramp.point(self.demo.ramp.primary) else {
            return;
        };
        let target = ColorEditTarget::RampPoint(point.id);
        let rgba = point.rgba;
        self.close_color_editor();
        self.demo.color_target = target;
        self.demo.color_draft = rgba;
        self.demo.color_open = true;
        self.begin_color_session(target);
        self.reset_color_hex_draft();
    }

    pub(super) fn toggle_swatch_color(&mut self) {
        let open = !(self.demo.color_open && self.demo.color_target == ColorEditTarget::Swatch);
        self.close_color_editor();
        self.demo.color_open = open;
        if open {
            self.begin_color_session(ColorEditTarget::Swatch);
        }
    }

    pub(super) fn apply_color_draft(&mut self) {
        if !self.can_edit_color(self.action_window) {
            return;
        }
        if self
            .text_session
            .is_some_and(|session| session.field == TextFieldKind::ColorHex)
        {
            self.commit_color_hex_draft();
        }
        if let Some(session) = self.demo.color_session.as_mut()
            && let Some(PointerEditOutcome::Commit { after, .. }) =
                session.finish(self.action_window, &self.demo.color_draft, true)
        {
            match self.demo.color_target {
                ColorEditTarget::Swatch => self.demo.color_committed = after,
                ColorEditTarget::RampPoint(id) => {
                    self.demo.ramp.set_color(id, after);
                }
            }
        }
        self.close_color_editor();
    }

    pub(super) fn close_color_editor(&mut self) {
        self.close_color_editor_with_restore(true);
    }

    pub(super) fn cancel_color_draft(&mut self) {
        if self.can_edit_color(self.action_window) {
            self.close_color_editor();
        }
    }

    pub(super) fn cancel_color_for(&mut self, window: UiWindowId) {
        if self.can_edit_color(window) {
            self.close_color_editor_with_restore(false);
        }
    }

    fn close_color_editor_with_restore(&mut self, restore_focus: bool) {
        let opener = self
            .demo
            .color_session
            .as_ref()
            .map(ValueEditSession::target);
        if self.text_session.is_some_and(|session| {
            session.field == TextFieldKind::ColorHex
                && opener.is_some_and(|opener| opener.window == session.window)
        }) {
            self.clear_text_session();
        }
        if let Some(mut session) = self.demo.color_session.take() {
            session.finish(session.target().window, &self.demo.color_draft, false);
        }
        self.demo.color_open = false;
        self.demo.color_target = ColorEditTarget::Swatch;
        self.demo.color_draft = self.demo.color_committed;
        self.reset_color_hex_draft();
        if let Some(opener) = opener.filter(|_| restore_focus) {
            self.restore_after_rebuild(opener.window, Some(opener.element));
        }
    }

    fn begin_color_session(&mut self, target: ColorEditTarget) {
        let Some(scopes) = self.window_scopes(self.action_window) else {
            return;
        };
        self.transition_text_field(self.text_session, None);
        self.clear_text_session();
        let slot = match target {
            ColorEditTarget::Swatch => COLOR_PICKER_ROOT + 1,
            ColorEditTarget::RampPoint(_) => RAMP_ROOT + 2,
        };
        let opener = InputTarget {
            window: self.action_window,
            element: scopes.gallery.element(slot),
        };
        self.set_window_focus(opener.window, Some(opener.element));
        self.demo.color_session = Some(ValueEditSession::new(opener, self.demo.color_draft));
    }

    pub(super) fn cancel_ramp_color(&mut self) {
        if matches!(self.demo.color_target, ColorEditTarget::RampPoint(_)) {
            self.close_color_editor();
        }
    }
}

impl App {
    pub(super) fn reset_color_hex_draft(&mut self) {
        let value = format_rgba_hex(self.demo.color_draft);
        let _ = self.color_hex_state.reset(&mut self.text_context, &value);
    }

    pub(super) fn commit_color_hex_draft(&mut self) -> bool {
        let resolution = resolve_value_draft(
            self.color_hex_state.text(),
            &self.demo.color_draft,
            parse_rgba_hex,
        );
        let accepted = match resolution {
            DraftResolution::Accepted { value, .. } => {
                self.demo.color_draft = value;
                true
            }
            DraftResolution::Rejected { .. } => false,
        };
        self.reset_color_hex_draft();
        accepted
    }

    pub(super) fn step_color_draft(&mut self, channel: u8, step: i8) {
        if !self.can_edit_color(self.action_window) {
            return;
        }
        if let Some(value) = vesta_gpui_widgets::color_channel_step(
            self.demo.color_draft,
            self.demo.color_mode,
            channel,
            step,
        ) {
            self.demo.color_draft = value;
        }
        self.reset_color_hex_draft();
    }
}
