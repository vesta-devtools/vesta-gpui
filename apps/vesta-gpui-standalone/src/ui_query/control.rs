//! Current-route commands. Never deserializes DemoAction or bypasses its handler.
use super::*;
use serde_json::json;
use vesta_gpui::{UiCommand, UiTargetRef};
fn semantic_activation(action: DemoAction) -> bool {
    super::admission::action_kind(action).0 == "semantic"
}

impl App {
    pub(super) fn answer_ui_control(
        &mut self,
        operation: &Operation,
    ) -> Result<serde_json::Value, &'static str> {
        if !self.ui_query.control_enabled {
            return Err("control_disabled");
        }
        match operation {
            Operation::Commands { instance, window } => {
                if *instance != self.ui_query.instance {
                    return Err("foreign_instance");
                }
                let snapshot = self
                    .ui_query
                    .latest
                    .get(window)
                    .cloned()
                    .ok_or("not_ready")?;
                self.ui_query.pin(snapshot.clone());
                let route = self.window_actions(*window).ok_or("window_not_found")?;
                let modal = self.focus_root_for_window(*window);
                let commands = snapshot
                    .nodes()
                    .iter()
                    .filter_map(|node| {
                        let target = self.ui_query.owners.reference(*window, node.id)?;
                        let mut verbs = Vec::new();
                        for command in [UiCommand::Activate, UiCommand::Focus, UiCommand::Reveal, UiCommand::SetValue(0.0)] {
                            if matches!(command,UiCommand::Activate) && !route.action_at(node.id).copied().is_some_and(semantic_activation) {continue;}
                            if command.preflight(target, &snapshot, route, modal).is_ok() {
                                verbs.push(command.name());
                            }
                        }
let input_target=verbs.contains(&"reveal")&&!node.value.redacted&&!node.name.redacted&&node.visible_bounds.is_some();
                        if node.expanded.is_some()
                            && route.action_at(node.id).is_some()
                            && UiCommand::SetExpanded(false).preflight(target, &snapshot, route, modal).is_ok()
                        {
                            verbs.push("set_expanded");
                        }
                        if self.text_field_at(*window, Some(node.id)).is_some()
                            && !node.value.redacted
                        {
                            for command in [
                                UiCommand::SelectAll,
                                UiCommand::Undo,
                                UiCommand::Redo,
                                UiCommand::ReplaceText {
                                    expected: String::new(),
                                    text: String::new(),
                                },
                            ] {
                                if command.preflight(target, &snapshot, route, modal).is_ok() {
                                    verbs.push(command.name());
                                }
                            }
                        }
                        (!verbs.is_empty()||input_target||node.parent.is_none())
                            .then(|| json!({"element":node.id.get().to_string(),"owner":target.owner.to_string(),"commands":verbs,"input_target":input_target,"window_target":node.parent.is_none()}))
                    })
                    .collect::<Vec<_>>();
                let coverage=snapshot.nodes().iter().filter_map(|node|{
                    let action=route.action_at(node.id).copied()?;let (kind,reason)=super::admission::action_kind(action);
                    Some(json!({"element":node.id.get().to_string(),"kind":kind,"reason":reason}))
                }).collect::<Vec<_>>();
                Ok(
                    json!({"revision":snapshot.id.revision.to_string(),"commands":commands,
                    "coverage":{"scope":"current_realized_route","exhaustive_action_classification":true,"actions":coverage},
                    "presentation_pending":self.ui_query.dirty.contains(window),"input":vesta_gpui_winit::ui_input_schema(),
                    "schemas":{"set_expanded":{"expanded":"boolean"},"set_value":{"value":"finite number; existing control clamping applies"},"replace_text":{"expected":"string (exact current source)","text":"string (bounded editor input)"}}}),
                )
            }
            Operation::Invoke {
                request_id,
                target,
                command,
            } => {
                let intent = ControlIntent::Semantic(*target, command.clone());
                if let Some((_, prior, result)) = self
                    .ui_query
                    .receipts
                    .iter()
                    .find(|(id, ..)| id == request_id)
                {
                    return if prior == &intent {
                        result.clone()
                    } else {
                        Err("request_id_conflict")
                    };
                }
                let result = self.invoke_ui_command(*target, command);
                if self.ui_query.receipts.len() == 128 {
                    self.ui_query.receipts.pop_front();
                }
                self.ui_query
                    .receipts
                    .push_back((request_id.clone(), intent, result.clone()));
                result
            }
            _ => Err("invalid_request"),
        }
    }

    fn invoke_ui_command(
        &mut self,
        target: UiTargetRef,
        command: &UiCommand,
    ) -> Result<serde_json::Value, &'static str> {
        if target.instance != self.ui_query.instance {
            return Err("foreign_instance");
        }
        if !self.ui_query.owners.contains(target) {
            return Err("stale_owner");
        }
        if [
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
        if self.ui_query.dirty.contains(&target.window) {
            return Err("presentation_pending");
        }
        let snapshot = self
            .ui_query
            .latest
            .get(&target.window)
            .ok_or("not_ready")?
            .clone();
        let route = self
            .window_actions(target.window)
            .ok_or("window_not_found")?;
        command.preflight(
            target,
            &snapshot,
            route,
            self.focus_root_for_window(target.window),
        )?;
        let before = snapshot.id.revision;
        let old_action_window = self.action_window;
        self.action_window = target.window;
        let result = self.apply_ui_command(target, command);
        self.action_window = old_action_window;
        result?;
        self.ui_query.invalidate_shared();
        self.request_all_redraw();
        Ok(
            json!({"status":"completed","observed_revision":before.to_string(),"presentation_pending":true}),
        )
    }

    fn apply_ui_command(
        &mut self,
        target: UiTargetRef,
        command: &UiCommand,
    ) -> Result<(), &'static str> {
        match command {
            UiCommand::SetValue(value) => {
                self.set_window_numeric_value(target.window, target.element, *value)?
            }
            UiCommand::Activate | UiCommand::SetExpanded(_) => {
                if let UiCommand::SetExpanded(expanded) = command {
                    let node = self
                        .ui_query
                        .latest
                        .get(&target.window)
                        .and_then(|s| s.nodes().iter().find(|n| n.id == target.element))
                        .ok_or("target_not_captured")?;
                    if node.expanded == Some(*expanded) {
                        return Ok(());
                    }
                }
                let action = *self
                    .window_actions(target.window)
                    .and_then(|r| r.action_at(target.element))
                    .ok_or("action_unavailable")?;
                if !semantic_activation(action) {
                    return Err("semantic_action_required");
                }
                self.ui_query.input_origin = vesta_gpui_winit::UiInputOrigin::Command;
                self.ui_query.input_error = None;
                self.apply_window_action(target.window, action);
                self.ui_query.input_origin = vesta_gpui_winit::UiInputOrigin::Native;
                if let Some(error) = self.ui_query.input_error.take() {
                    return Err(error);
                }
            }
            UiCommand::Focus => {
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
            UiCommand::Reveal => {
                // Reveal is offered for ordinary content as well as keyboard stops.
                let rect = self
                    .ui_query
                    .latest
                    .get(&target.window)
                    .and_then(|snapshot| {
                        snapshot
                            .nodes()
                            .iter()
                            .find(|node| node.id == target.element)
                    })
                    .and_then(|node| node.bounds)
                    .ok_or("target_geometry_unavailable")?;
                self.reveal_window_rect(target.window, target.element, rect);
            }
            UiCommand::ReplaceText { expected, text } => {
                let field = self
                    .text_field_at(target.window, Some(target.element))
                    .ok_or("not_editor")?;
                // Check before changing focus: field transitions may reset a transaction.
                {
                    let (state, _) = self.text_state_and_context_mut(field);
                    if state.is_password() {
                        return Err("sensitive_target");
                    }
                    if state.text() != expected {
                        return Err("stale_content");
                    }
                    if state.is_composing() {
                        return Err("composing");
                    }
                }
                self.set_window_focus(target.window, Some(target.element));
                let (state, context) = self.text_state_and_context_mut(field);
                if state.text() != expected {
                    return Err("stale_content");
                }
                state
                    .replace_accessibility(context, text)
                    .map_err(|_| "edit_rejected")?;
            }
            UiCommand::SelectAll | UiCommand::Undo | UiCommand::Redo => {
                let field = self
                    .text_field_at(target.window, Some(target.element))
                    .ok_or("not_editor")?;
                self.set_window_focus(target.window, Some(target.element));
                let (state, context) = self.text_state_and_context_mut(field);
                if state.is_password() {
                    return Err("sensitive_target");
                }
                if state.is_composing() {
                    return Err("composing");
                }
                state.act(
                    context,
                    match command {
                        UiCommand::SelectAll => vesta_gpui::TextInputAction::SelectAll,
                        UiCommand::Undo => vesta_gpui::TextInputAction::Undo,
                        _ => vesta_gpui::TextInputAction::Redo,
                    },
                );
            }
        }
        Ok(())
    }
}
