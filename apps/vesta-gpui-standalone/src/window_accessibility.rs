//! Per-window platform projection and shared action policy (ADR-0293).

use super::*;

pub(super) struct TextFieldStates<'a> {
    pub main: &'a mut TextInputState,
    pub search: &'a mut TextInputState,
    pub area: &'a mut TextInputState,
    pub number: &'a mut TextInputState,
    pub vector: &'a mut [TextInputState; 3],
    pub color: &'a mut TextInputState,
    pub password: &'a mut TextInputState,
    pub table_note: &'a mut TextInputState,
    pub readonly_label: &'a mut TextInputState,
}

impl TextFieldStates<'_> {
    fn get(&mut self, field: TextFieldKind) -> &mut TextInputState {
        match field {
            TextFieldKind::Main => self.main,
            TextFieldKind::Search => self.search,
            TextFieldKind::Area => self.area,
            TextFieldKind::Number => self.number,
            TextFieldKind::Vector(axis) => &mut self.vector[usize::from(axis)],
            TextFieldKind::ColorHex => self.color,
            TextFieldKind::Password => self.password,
            TextFieldKind::TableNote => self.table_note,
            TextFieldKind::Label => self.readonly_label,
        }
    }
}

pub(super) fn enrich_window_text(
    semantics: &mut AccessibilityTree,
    gallery: ElementScope,
    views: &TextViews,
    mut states: TextFieldStates<'_>,
    context: &mut TextInputContext,
    update: &mut accesskit::TreeUpdate,
) {
    for field in TextFieldKind::ALL {
        if let Some(view) = views.get(field) {
            semantics.enrich_text_input(
                update,
                field.element(gallery),
                states.get(field),
                context,
                view.origin,
            );
        }
    }
}

pub(super) struct WindowAccessibilityFrame<'a> {
    pub window: UiWindowId,
    pub semantics: &'a AccessibilityTree,
    pub actions: &'a ActionRoute<DemoAction>,
    pub focus_root: Option<ElementId>,
}

impl WindowAccessibilityFrame<'_> {
    pub fn resolve(
        &self,
        node: accesskit::NodeId,
        action: AccessibilityAction,
    ) -> Option<InputTarget> {
        FocusScope::new(self.window, self.actions, self.focus_root).accessible_target(
            self.semantics,
            node,
            action,
        )
    }
}

impl App {
    pub(super) fn set_window_numeric_value(
        &mut self,
        window: UiWindowId,
        element: ElementId,
        value: f64,
    ) -> Result<(), &'static str> {
        if !value.is_finite() {
            return Err("invalid_value");
        }
        let actions = self.window_actions(window).ok_or("window_not_found")?;
        if !FocusScope::new(window, actions, self.focus_root_for_window(window))
            .accepts(InputTarget { window, element })
        {
            return Err("target_unavailable");
        }
        let action = actions.action_at(element).copied();
        let ramp = ramp_accessibility_action(
            actions,
            element,
            Some(&AccessibilityActionData::NumericValue(value)),
        );
        let field = self.text_field_at(window, Some(element));
        if let Some(action) = ramp {
            self.apply_window_action(window, action);
        } else if field == Some(TextFieldKind::Number) {
            self.demo.number = (value.round() as i32).clamp(NUMBER_MIN, NUMBER_MAX);
            self.reset_number_draft();
        } else if let Some(TextFieldKind::Vector(axis)) = field {
            self.demo.vector[usize::from(axis)] =
                Some((value as f32).clamp(VECTOR_MIN, VECTOR_MAX));
            self.reset_vector_draft(axis);
        } else if action == Some(DemoAction::SetSlider) {
            self.apply_window_action(
                window,
                DemoAction::SetSliderValue(value.clamp(0.0, 1.0) as f32),
            );
        } else {
            return Err("not_settable");
        }
        self.set_window_focus(window, Some(element));
        Ok(())
    }
    pub(super) fn window_accessibility_frame(
        &self,
        window: UiWindowId,
    ) -> Option<WindowAccessibilityFrame<'_>> {
        let focus_root = self.focus_root_for_window(window);
        if window == STANDALONE_WINDOW {
            Some(WindowAccessibilityFrame {
                window,
                semantics: &self.semantics,
                actions: &self.ui_actions,
                focus_root,
            })
        } else {
            let host = self
                .native_windows
                .values()
                .find(|host| host.ui_window == window)?;
            Some(WindowAccessibilityFrame {
                window,
                semantics: &host.semantics,
                actions: &host.actions,
                focus_root,
            })
        }
    }

    pub(super) fn window_accessibility_action(
        &mut self,
        window: UiWindowId,
        request: accesskit::ActionRequest,
    ) {
        let Some(frame) = self.window_accessibility_frame(window) else {
            return;
        };
        let Some(target) = frame.resolve(request.target, request.action) else {
            self.frame_profile
                .observe_native(|| NativeObservation::Action {
                    window,
                    node: request.target,
                    action: request.action,
                    resolved_owner: None,
                });
            return;
        };
        let action = frame.actions.action_at(target.element).copied();
        let field = self.text_field_at(window, Some(target.element));
        let document_selection = match &request.data {
            Some(AccessibilityActionData::SetTextSelection(selection)) => frame
                .semantics
                .resolve_read_only_selection(target.element, selection),
            _ => None,
        };
        self.frame_profile
            .observe_native(|| NativeObservation::Action {
                window,
                node: request.target,
                action: request.action,
                resolved_owner: Some(target.element),
            });
        self.action_window = window;
        self.observe_window_focus_input(window, vesta_gpui::FocusInput::Keyboard);
        match (request.action, request.data) {
            (AccessibilityAction::Focus, _) => {
                self.set_window_focus(window, Some(target.element));
            }
            (AccessibilityAction::Click, _) => {
                if field.is_some() {
                    self.set_window_focus(window, Some(target.element));
                } else if !self.activate_window_document_target(window, target) {
                    if let Some(action) = action.filter(|action| !action.starts_pointer_drag()) {
                        self.apply_window_action(window, action);
                    } else {
                        return;
                    }
                }
            }
            (AccessibilityAction::SetValue, Some(AccessibilityActionData::NumericValue(value))) => {
                if self
                    .set_window_numeric_value(window, target.element, value)
                    .is_err()
                {
                    return;
                }
            }
            (AccessibilityAction::SetValue, Some(AccessibilityActionData::Value(value))) => {
                if let Some(field) = field {
                    self.set_window_focus(window, Some(target.element));
                    let (state, context) = self.text_state_and_context_mut(field);
                    if let Err(error) = state.replace_accessibility(context, &value) {
                        eprintln!("accessibility text edit: {error:?}");
                        return;
                    }
                } else if action == Some(DemoAction::SetSlider) {
                    let Ok(value) = value.parse::<f64>() else {
                        return;
                    };
                    if !value.is_finite() {
                        return;
                    }
                    self.apply_window_action(
                        window,
                        DemoAction::SetSliderValue(value.clamp(0.0, 1.0) as f32),
                    );
                    self.set_window_focus(window, Some(target.element));
                } else {
                    return;
                }
            }
            (
                AccessibilityAction::SetTextSelection,
                Some(AccessibilityActionData::SetTextSelection(selection)),
            ) => {
                if let Some(selection) = document_selection {
                    self.select_window_document_accessibility(window, selection);
                    return;
                }
                let Some(field) = field else { return };
                let Some(selection) = self.resolve_window_accessibility_text(
                    window,
                    target.element,
                    field,
                    &selection,
                ) else {
                    return;
                };
                self.set_window_focus(window, Some(target.element));
                let (state, context) = self.text_state_and_context_mut(field);
                state.apply_accessibility_selection(context, selection);
            }
            _ => return,
        }
        self.reveal_window_focus(window, target.element);
        self.request_all_redraw();
    }

    fn resolve_window_accessibility_text(
        &mut self,
        window: UiWindowId,
        element: ElementId,
        field: TextFieldKind,
        selection: &accesskit::TextSelection,
    ) -> Option<vesta_gpui::TextInputSelection> {
        let mut states = TextFieldStates {
            main: &mut self.text_state,
            search: &mut self.search_state,
            area: &mut self.area_state,
            number: &mut self.number_state,
            vector: &mut self.vector_states,
            color: &mut self.color_hex_state,
            password: &mut self.password_state,
            table_note: &mut self.table_note_state,
            readonly_label: &mut self.readonly_label_state,
        };
        let semantics = if window == STANDALONE_WINDOW {
            &self.semantics
        } else {
            let host = self
                .native_windows
                .values()
                .find(|host| host.ui_window == window)?;
            &host.semantics
        };
        semantics.resolve_text_input_selection(
            element,
            states.get(field),
            &mut self.text_context,
            selection,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_target_requires_a_live_window_route_and_modal_scope() {
        let window = UiWindowId(77);
        let root_id = ElementId::new(990);
        let field = ElementId::new(991);
        let root = Element::container(root_id, ElementKey::new("accessible.root"))
            .styled(Style::default().size(Dimension::Px(200.0), Dimension::Px(100.0)))
            .child(
                Element::container(field, ElementKey::new("accessible.field"))
                    .styled(
                        Style::default()
                            .size(Dimension::Px(180.0), Dimension::Px(32.0))
                            .focusable(true),
                    )
                    .semantic(Semantic::new(SemanticRole::TextInput, "Text")),
            );
        let layout = LayoutEngine
            .compute(&root, Viewport::new(200.0, 100.0, 1.0))
            .expect("layout");
        let actions = ActionRoute::build(&root, [(field, DemoAction::ToggleTheme)]).expect("route");
        let mut semantics = AccessibilityTree::default();
        semantics.build(&root, &layout, Some(field), "Source window");
        let node = semantics.node_for_element(field).expect("field node");
        let frame = WindowAccessibilityFrame {
            window,
            semantics: &semantics,
            actions: &actions,
            focus_root: None,
        };
        assert_eq!(
            frame.resolve(node, AccessibilityAction::SetValue),
            Some(InputTarget {
                window,
                element: field
            })
        );
        assert_eq!(
            frame.resolve(accesskit::NodeId(9999), AccessibilityAction::Focus),
            None
        );
        let hidden = ActionRoute::default();
        let stale = WindowAccessibilityFrame {
            actions: &hidden,
            ..frame
        };
        assert_eq!(
            stale.resolve(node, AccessibilityAction::SetValue),
            None,
            "queued action cannot reach removed controls"
        );
        let modal = WindowAccessibilityFrame {
            focus_root: Some(ElementId::new(992)),
            ..frame
        };
        assert_eq!(
            modal.resolve(node, AccessibilityAction::Focus),
            None,
            "background actions stay outside the modal"
        );
        let disabled_root = root.clone().styled(Style::default().action_boundary(true));
        let disabled_actions =
            ActionRoute::build(&disabled_root, [(field, DemoAction::ToggleTheme)])
                .expect("disabled route");
        let disabled = WindowAccessibilityFrame {
            actions: &disabled_actions,
            ..frame
        };
        assert_eq!(disabled.resolve(node, AccessibilityAction::Click), None);
    }
}
