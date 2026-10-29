//! Complete logical input converges on Main/Float's existing dispatch.
use super::*;
use serde_json::json;
use vesta_gpui_winit::{UiInputOrigin, UiInputRequest, UiInputStep};
impl App {
    pub(super) fn ui_native_window(
        &self,
        instance: u128,
        window: UiWindowId,
    ) -> Result<Arc<Window>, &'static str> {
        if instance != self.ui_query.instance {
            return Err("foreign_instance");
        }
        if window == STANDALONE_WINDOW {
            return self.window.clone().ok_or("window_not_found");
        }
        self.native_windows
            .values()
            .find(|host| host.ui_window == window)
            .map(|host| host.window.clone())
            .ok_or("window_not_found")
    }
    pub(super) fn answer_ui_input(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: &str,
        request: &UiInputRequest,
    ) -> Result<serde_json::Value, &'static str> {
        if !self.ui_query.control_enabled {
            return Err("control_disabled");
        }
        let intent = ControlIntent::Input(request.clone());
        if let Some((_, prior, result)) = self.ui_query.receipts.iter().find(|(key, ..)| key == id)
        {
            return if prior == &intent {
                result.clone()
            } else {
                Err("request_id_conflict")
            };
        }
        let result = self.ui_input_once(event_loop, request);
        if self.ui_query.receipts.len() == 128 {
            self.ui_query.receipts.pop_front();
        }
        self.ui_query
            .receipts
            .push_back((id.into(), intent, result.clone()));
        result
    }
    fn ui_input_once(
        &mut self,
        event_loop: &ActiveEventLoop,
        request: &UiInputRequest,
    ) -> Result<serde_json::Value, &'static str> {
        let target = request.target;
        let window = self.ui_native_window(target.instance, target.window)?;
        if !self.ui_query.owners.contains(target) {
            return Err("stale_owner");
        }
        let window_command = match request.steps.as_slice() {
            [UiInputStep::Window(command)] => Some(command),
            _ => None,
        };
        let cancel = matches!(request.steps.as_slice(), [UiInputStep::Cancel]);
        if self.ui_query.dirty.contains(&target.window) && window_command.is_none() && !cancel {
            return Err("presentation_pending");
        }
        if window_command.is_none() && self.display_blocked(window.id()) {
            return Err("display_unavailable");
        }
        if !cancel
            && (self.camera_drag
                || self.camera_pan
                || self.transform_drag.is_some()
                || self.input_ownership.capture().is_some()
                || self
                    .native_windows
                    .values()
                    .any(|host| host.input.capture().is_some()))
        {
            return Err("interaction_busy");
        }
        if window_command.is_none()
            && [
                &self.text_state,
                &self.search_state,
                &self.area_state,
                &self.number_state,
                &self.vector_states[0],
                &self.vector_states[1],
                &self.vector_states[2],
                &self.color_hex_state,
                &self.password_state,
                &self.table_note_state,
                &self.readonly_label_state,
            ]
            .iter()
            .any(|state| state.is_composing())
        {
            return Err("composing");
        }
        let current = self
            .ui_query
            .latest
            .get(&target.window)
            .cloned()
            .ok_or("not_ready")?;
        let snapshot = if request.revision == current.id.revision {
            current.clone()
        } else {
            self.ui_query
                .pinned
                .iter()
                .find(|snapshot| {
                    snapshot.id.window == target.window && snapshot.id.revision == request.revision
                })
                .cloned()
                .ok_or("stale_snapshot")?
        };
        let now = self
            .ui_query
            .roots
            .get(&target.window)
            .cloned()
            .ok_or("not_ready")?;
        let source = if request.revision == current.id.revision {
            now.clone()
        } else {
            self.ui_query
                .pinned_roots
                .get(&(target.window, request.revision))
                .cloned()
                .ok_or("stale_snapshot")?
        };
        source
            .input
            .validate(&snapshot, &current, &now.input, request)?;
        let route = self
            .window_actions(target.window)
            .ok_or("window_not_found")?;
        request.validate(
            &current,
            route,
            if window_command.is_some() {
                None
            } else {
                self.focus_root_for_window(target.window)
            },
        )?;
        if window_command.is_none() {
            request.validate_hits(&current, route, |point| {
                if target.window == STANDALONE_WINDOW {
                    self.hit_test.hit(point)
                } else {
                    self.native_windows
                        .get(&window.id())
                        .and_then(|host| host.hit_test.hit(point))
                }
            })?;
        }
        let events = if let Some(command) = window_command {
            vesta_gpui_winit::apply_ui_window_command(&window, command)
                .into_iter()
                .collect()
        } else {
            request.events(&snapshot, self.window_modifiers(target.window))?
        };
        if request
            .steps
            .iter()
            .any(|step| matches!(step, UiInputStep::Key { .. } | UiInputStep::Text(_)))
            && route.focus_order().contains(&target.element)
        {
            self.set_window_focus(target.window, Some(target.element));
            if self
                .window_actions(target.window)
                .and_then(|route| route.action_at(target.element))
                .is_some_and(|action| {
                    matches!(
                        action,
                        DemoAction::MenuCommand(_) | DemoAction::OpenSubmenu(_)
                    )
                })
            {
                self.hover_window_menu_item(target.window, Some(target.element));
            }
        }
        let count = events.len();
        let original_modifiers = self.window_modifiers(target.window);
        self.ui_query.input_origin = UiInputOrigin::Command;
        self.ui_query.input_error = None;
        for event in events {
            self.dispatch_window_event(event_loop, window.id(), event);
            if self.ui_query.input_error.is_some() {
                break;
            }
        }
        let captured = if target.window == STANDALONE_WINDOW {
            self.input_ownership.capture().is_some()
        } else {
            self.native_windows
                .get(&window.id())
                .is_some_and(|host| host.input.capture().is_some())
        };
        if captured {
            self.dispatch_window_event(event_loop, window.id(), WindowEvent::Focused(false));
        }
        if captured || self.ui_query.input_error.is_some() {
            self.dispatch_window_event(
                event_loop,
                window.id(),
                WindowEvent::ModifiersChanged(winit::event::Modifiers::from(original_modifiers)),
            );
        }
        self.ui_query.input_origin = UiInputOrigin::Native;
        self.ui_query.invalidate_shared();
        self.request_all_redraw();
        if let Some(error) = self.ui_query.input_error.take() {
            return Err(error);
        }
        if captured {
            return Err("gesture_cancelled");
        }
        Ok(
            json!({"status":"dispatched","completed":false,"origin":"command","events":count,"source_revision":snapshot.id.revision.to_string(),"observed_revision":current.id.revision.to_string(),"presentation_pending":true}),
        )
    }
}
