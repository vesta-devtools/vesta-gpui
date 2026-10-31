//! Current-report-only cancellation observations using real native window owners.
use super::*;

pub(super) struct Armed {
    target: InputTarget,
    other: vesta_gpui::UiTargetRef,
    before_other: Option<vesta_gpui::UiSnapshotId>,
    number: i32,
    at: Instant,
    own_point: Point,
    other_point: Point,
}

impl Armed {
    pub(super) fn new(
        app: &mut App,
        event_loop: &ActiveEventLoop,
        floating: WindowId,
        target: vesta_gpui::UiTargetRef,
    ) -> Result<Self, String> {
        let host = app
            .native_windows
            .get(&floating)
            .ok_or("frame_cancel_float_lost")?;
        let (other_window, other_field) = if target.window == STANDALONE_WINDOW {
            (host.ui_window, host.scopes.gallery.element(NUMBER_FIELD))
        } else {
            (STANDALONE_WINDOW, app.scopes.gallery.element(NUMBER_FIELD))
        };
        let point = |view: Option<vesta_gpui::TextInputViewport>| -> Result<Point, String> {
            let clip = view.ok_or("frame_cancel_number_view_missing")?.clip;
            if clip.width <= 0.0 || clip.height <= 0.0 {
                return Err("frame_cancel_number_not_visible".into());
            }
            Ok(Point::new(
                clip.x + clip.width * 0.5,
                clip.y + clip.height * 0.5,
            ))
        };
        let own_point = point(if target.window == STANDALONE_WINDOW {
            app.number_view
        } else {
            host.text_views.get(TextFieldKind::Number)
        })?;
        let other_point = point(if other_window == STANDALONE_WINDOW {
            app.number_view
        } else {
            host.text_views.get(TextFieldKind::Number)
        })?;
        let commands = control(
            app,
            event_loop,
            ui_query::Operation::Commands {
                instance: target.instance,
                window: other_window,
            },
        )
        .map_err(str::to_owned)?;
        let other = command_target(&commands, target.instance, other_window, other_field)
            .map_err(str::to_owned)?;
        let before_other = app.ui_query.next(other_window);
        let number = app.demo.number;
        let target = InputTarget {
            window: target.window,
            element: target.element,
        };
        let mut scrub = NumberScrub::new(target, own_point.x, number, 4.0)
            .map_err(|e| format!("frame_cancel_scrub: {e:?}"))?;
        let preview = number_scrub_value(&mut scrub, target.window, own_point.x + 24.0, number)
            .ok_or("frame_cancel_number_preview_missing")?;
        if target.window == STANDALONE_WINDOW {
            app.number_scrub = Some(scrub);
        } else {
            app.native_windows
                .get_mut(&floating)
                .ok_or("frame_cancel_float_lost")?
                .number_scrub = Some(scrub);
        }
        app.demo.number = preview;
        app.reset_number_draft();
        let at = Instant::now();
        for (target, point) in [
            (target, own_point),
            (
                InputTarget {
                    window: other.window,
                    element: other.element,
                },
                other_point,
            ),
        ] {
            let bridge = app
                .text_bridge_for(target.window)
                .ok_or("frame_cancel_bridge_missing")?;
            bridge.clear_text_pointer();
            let first = bridge.text_pointer_press(target, point, at, false);
            if first != Some(vesta_gpui_winit::TextPointerAction::Caret) {
                return Err("frame_cancel_first_press_not_caret".into());
            }
        }
        Ok(Self {
            target,
            other,
            before_other,
            number,
            at,
            own_point,
            other_point,
        })
    }

    pub(super) fn verify(
        self,
        app: &mut App,
        event_loop: &ActiveEventLoop,
        source: usize,
        step: usize,
    ) -> Result<(), String> {
        let now = Instant::now();
        if now.duration_since(self.at) > Duration::from_millis(450) {
            return Err("frame_cancel_click_interval_expired".into());
        }
        let own = app
            .text_bridge_for(self.target.window)
            .ok_or("frame_cancel_bridge_missing")?
            .text_pointer_press(self.target, self.own_point, now, false);
        let foreign = app
            .text_bridge_for(self.other.window)
            .ok_or("frame_cancel_bridge_missing")?
            .text_pointer_press(
                InputTarget {
                    window: self.other.window,
                    element: self.other.element,
                },
                self.other_point,
                now,
                false,
            );
        let scoped = own == Some(vesta_gpui_winit::TextPointerAction::Caret)
            && foreign == Some(vesta_gpui_winit::TextPointerAction::Word);
        let restored =
            app.demo.number == self.number && app.number_state.text() == self.number.to_string();
        let commands = control(
            app,
            event_loop,
            ui_query::Operation::Commands {
                instance: self.other.instance,
                window: self.other.window,
            },
        )
        .map_err(str::to_owned)?;
        let command = control(
            app,
            event_loop,
            ui_query::Operation::Invoke {
                request_id: format!("cancel-other-{source}-{step}"),
                target: self.other,
                command: vesta_gpui::UiCommand::Focus,
            },
        );
        let pending = commands["presentation_pending"] == true
            && app.ui_query.next(self.other.window) == self.before_other
            && matches!(command, Err("presentation_pending"));
        for (name, accepted) in [
            ("reflow.cancel_click_scoped", scoped),
            ("reflow.cancel_value_restored", restored),
            ("reflow.cancel_other_pending", pending),
        ] {
            vesta_gpui::trace::counter("fixture", name, if accepted { 1.0 } else { 0.0 });
        }
        if !(scoped && restored && pending) {
            return Err(format!(
                "frame_cancel_mismatch: source={source} step={step} scoped={scoped} restored={restored} pending={pending} own={own:?} foreign={foreign:?} command={command:?}"
            ));
        }
        Ok(())
    }
}
