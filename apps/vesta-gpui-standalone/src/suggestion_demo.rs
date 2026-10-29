//! The existing Search editor consumes the shared request lifecycle; no provider runtime.
use super::*;
use vesta_gpui_widgets::{
    SuggestionOption, SuggestionRequestId, SuggestionState, SuggestionViewIds,
    editable_combo_controlled,
};

pub(super) fn filtered_suggestions(query: &str) -> Vec<SuggestionOption<u16>> {
    (0..128_u16)
        .filter_map(|id| {
            let label = match id {
                0 => "Cube".to_owned(),
                1 => "Sphere".to_owned(),
                2 => "Terrain".to_owned(),
                _ => format!("Terrain preset 地形 {id:03}"),
            };
            let searchable = label.to_lowercase();
            query
                .split_whitespace()
                .all(|word| searchable.contains(&word.to_lowercase()))
                .then_some(SuggestionOption {
                    id,
                    label,
                    enabled: id % 13 != 5,
                })
        })
        .collect()
}

pub(super) fn project(input: &UiBuildState<'_>, editor: Widget<DemoAction>) -> Widget<DemoAction> {
    let scope = input.scopes.gallery;
    let id = |part, name| WidgetIdentity::new(scope.element(SUGGESTION_ROOT + part), name);
    let closed = SuggestionState::default();
    let state = if input
        .demo
        .suggestions
        .request()
        .is_some_and(|r| r.owner.window == input.ui_window)
    {
        &input.demo.suggestions
    } else {
        &closed
    };
    let ticket = state.request().map(|r| r.id);
    let mut widget = editable_combo_controlled(
        SuggestionViewIds {
            combo: EditableComboIds {
                root: id(0, "gallery.suggestions.root"),
                list: id(1, "gallery.suggestions.list"),
                empty: id(2, "gallery.suggestions.empty"),
            },
            content: id(3, "gallery.suggestions.content"),
        },
        editor,
        state,
        suggestion_height(input.viewport_size.height),
        input.theme,
        |option| {
            (
                WidgetIdentity::new(
                    scope.element(SUGGESTION_ROOT + 10 + u32::from(option.id)),
                    format!("gallery.suggestion.{}", option.id),
                ),
                ticket.map(|ticket| DemoAction::ChooseSuggestion(ticket, option.id)),
            )
        },
    )
    .expect("finite bounded gallery suggestion viewport");
    widget.element.style.width = Dimension::Px(300.0);
    widget.element.style.max_width = Dimension::Percent(1.0);
    widget
}

fn suggestion_height(height: f32) -> f32 {
    (height * 0.25).clamp(56.0, 140.0)
}

impl App {
    fn suggestion_owner(&self, window: UiWindowId) -> Option<InputTarget> {
        Some(InputTarget {
            window,
            element: self.gallery_scopes(window)?.element(SEARCH_FIELD),
        })
    }

    pub(super) fn prepare_suggestions(&mut self, window: UiWindowId) {
        let Some(owner) = self.suggestion_owner(window) else {
            return;
        };
        let live = self.text_session_field(window) == Some(TextFieldKind::Search)
            && !self.demo.inline_session.is_open()
            && !self.demo.palette_session.is_open()
            && self.focus_root_for_window(window).is_none()
            && !self.search_state.is_composing();
        if !live {
            if self
                .demo
                .suggestions
                .request()
                .is_some_and(|r| r.owner == owner)
            {
                self.retire_suggestion_drag(window);
                self.demo.suggestions.cancel(owner);
            }
            return;
        }
        let query = self.search_state.text().to_owned();
        if self.demo.suggestion_dismissed.as_ref() == Some(&(owner, query.clone())) {
            return;
        }
        if self
            .demo
            .suggestions
            .request()
            .is_some_and(|r| r.owner == owner && r.query == query)
        {
            return;
        }
        let Some(next) = self.demo.suggestion_serial.checked_add(1) else {
            return;
        };
        self.retire_suggestion_drag(window);
        self.demo.suggestion_serial = next;
        if let Ok(request) =
            self.demo
                .suggestions
                .begin(owner, SuggestionRequestId(next), query.clone(), false)
        {
            // A real synchronous host catalog. Delayed response ordering is qualified in
            // the existing independent consumer, not invented here as fake async I/O.
            let _ = self
                .demo
                .suggestions
                .resolve(&request, Ok(filtered_suggestions(&query)));
        }
    }

    pub(super) fn dismiss_suggestions(&mut self, window: UiWindowId) {
        if let Some(owner) = self.suggestion_owner(window) {
            if self
                .demo
                .suggestions
                .request()
                .is_none_or(|r| r.owner != owner)
            {
                return;
            }
            self.retire_suggestion_drag(window);
            self.demo.suggestions.cancel(owner);
            self.demo.suggestion_dismissed = Some((owner, self.search_state.text().to_owned()));
            self.demo.suggestion_scroll_node = None;
        }
    }

    fn retire_suggestion_drag(&mut self, window: UiWindowId) {
        if self
            .demo
            .suggestions
            .request()
            .is_none_or(|r| r.owner.window != window)
            || self.demo.suggestion_drag.take().is_none()
        {
            return;
        }
        let Some(scope) = self.gallery_scopes(window) else {
            return;
        };
        let target = InputTarget {
            window,
            element: scope.element(SUGGESTION_ROOT + 1),
        };
        let input = if window == STANDALONE_WINDOW {
            &mut self.input_ownership
        } else if let Some(host) = self
            .native_windows
            .values_mut()
            .find(|h| h.ui_window == window)
        {
            &mut host.input
        } else {
            return;
        };
        if input
            .capture()
            .is_some_and(|c| c.target == target && c.button == PointerButton::Primary)
        {
            input.cancel_capture();
        }
    }

    pub(super) fn choose_suggestion(&mut self, ticket: SuggestionRequestId, id: u16) {
        let window = self.action_window;
        let Some(owner) = self.suggestion_owner(window) else {
            return;
        };
        if self.text_session_field(window) != Some(TextFieldKind::Search)
            || self.demo.palette_session.is_open()
            || self.demo.inline_session.is_open()
            || self.focus_root_for_window(window).is_some()
        {
            return;
        }
        let Some(request) = self
            .demo
            .suggestions
            .request()
            .filter(|r| r.id == ticket && r.query == self.search_state.text())
            .cloned()
        else {
            return;
        };
        if let Some(option) =
            self.demo
                .suggestions
                .accept(owner, &request, &id, self.search_state.is_composing())
        {
            // The ticket is already retired. The editor handoff releases this
            // source's capture; retire its drag and geometry before another query.
            self.demo.suggestion_drag = None;
            self.demo.suggestion_scroll_node = None;
            // Reset would discard the editor's entire Undo/Redo history. This
            // existing bounded replacement path remains one ordinary text edit.
            if self
                .search_state
                .replace_accessibility(&mut self.text_context, &option.label)
                .is_err()
            {
                return;
            }
            self.demo.suggestion_dismissed = Some((owner, option.label));
            self.begin_text_selection(window, TextFieldKind::Search, None);
        }
    }

    pub(super) fn handle_suggestion_key_for(&mut self, window: UiWindowId, key: &Key) -> bool {
        if self.text_session_field(window) != Some(TextFieldKind::Search)
            || self.demo.inline_session.is_open()
            || self.demo.palette_session.is_open()
        {
            return false;
        }
        let Some(chord) = shortcut_chord(key.as_ref(), self.window_modifiers(window)) else {
            return false;
        };
        if matches!(
            chord.key,
            vesta_gpui::KeyStroke::ArrowDown | vesta_gpui::KeyStroke::ArrowUp
        ) && chord.modifiers == vesta_gpui::KeyModifiers::default()
            && !self.search_state.is_composing()
            && self.demo.suggestions.request().is_none()
        {
            self.demo.suggestion_dismissed = None;
        }
        self.prepare_suggestions(window);
        let Some(owner) = self.suggestion_owner(window) else {
            return false;
        };
        let action =
            self.demo
                .suggestions
                .key_action(owner, chord, self.search_state.is_composing());
        match action {
            Some(ChoiceKeyAction::Confirm(id)) => {
                if let Some(ticket) = self.demo.suggestions.request().map(|r| r.id) {
                    self.action_window = window;
                    self.apply_demo_action(DemoAction::ChooseSuggestion(ticket, id));
                }
            }
            Some(ChoiceKeyAction::Close) => self.dismiss_suggestions(window),
            Some(ChoiceKeyAction::TraverseFocus { backwards }) => {
                self.dismiss_suggestions(window);
                self.traverse_window_focus(window, backwards);
            }
            Some(ChoiceKeyAction::Highlight(_) | ChoiceKeyAction::Consumed) => {}
            None => return false,
        }
        true
    }

    pub(super) fn sync_suggestion_layout(
        &mut self,
        window: UiWindowId,
        scope: ElementScope,
        layout: &LayoutTree,
    ) {
        if self
            .demo
            .suggestions
            .request()
            .is_some_and(|r| r.owner.window == window)
            && let Some(node) = layout.node(scope.element(SUGGESTION_ROOT + 1))
        {
            self.demo
                .suggestions
                .sync_viewport(node.scroll_viewport().height);
            self.demo.suggestion_scroll_node = Some(node.clone());
        }
    }

    /// Nested list wheels consume only their own viewport. Outer page behavior is unchanged.
    pub(super) fn handle_suggestion_scroll_event(
        &mut self,
        window: UiWindowId,
        event: &WindowEvent,
    ) -> bool {
        if self
            .demo
            .suggestions
            .request()
            .is_none_or(|r| r.owner.window != window)
        {
            return false;
        }
        if matches!(event, WindowEvent::Focused(false) | WindowEvent::Destroyed) {
            self.dismiss_suggestions(window);
            return false;
        }
        let (point, scale) = if window == STANDALONE_WINDOW {
            (
                self.cursor,
                self.window.as_ref().map_or(1.0, |w| w.scale_factor()),
            )
        } else if let Some(host) = self.native_windows.values().find(|h| h.ui_window == window) {
            (host.cursor, host.window.scale_factor())
        } else {
            return false;
        };
        let Some(node) = self.demo.suggestion_scroll_node.as_ref() else {
            return false;
        };
        if self.demo.suggestion_drag.is_some() {
            match event {
                WindowEvent::CursorMoved { position, .. } => {
                    let next = logical_point(*position, scale);
                    if window == STANDALONE_WINDOW {
                        self.cursor = next;
                    } else if let Some(host) = self
                        .native_windows
                        .values_mut()
                        .find(|h| h.ui_window == window)
                    {
                        host.cursor = next;
                    }
                    if let Some(drag) = self.demo.suggestion_drag {
                        self.demo.suggestions.drag_scroll(drag, next);
                    }
                    return true;
                }
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    button: MouseButton::Left,
                    ..
                } => {
                    self.retire_suggestion_drag(window);
                    return true;
                }
                _ => {}
            }
        }
        if !node.rect.contains(point) || !node.inherited_clip.contains(point) {
            return false;
        }
        if matches!(
            event,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            }
        ) && let Some(drag) = self
            .demo
            .suggestions
            .scroll()
            .begin_thumb_drag_in_layout(node, point)
        {
            self.demo.suggestion_drag = Some(drag);
            if let Some(scope) = self.gallery_scopes(window) {
                let target = InputTarget {
                    window,
                    element: scope.element(SUGGESTION_ROOT + 1),
                };
                if window == STANDALONE_WINDOW {
                    self.input_ownership
                        .capture_pointer(target, PointerButton::Primary);
                } else if let Some(host) = self
                    .native_windows
                    .values_mut()
                    .find(|h| h.ui_window == window)
                {
                    host.input.capture_pointer(target, PointerButton::Primary);
                }
            }
            return true;
        }
        if let WindowEvent::MouseWheel { delta, .. } = event {
            let y = match delta {
                MouseScrollDelta::LineDelta(_, y) => -y * 28.0,
                MouseScrollDelta::PixelDelta(p) => -logical_point(*p, scale).y,
            };
            self.demo.suggestions.wheel(y);
            return true;
        }
        if matches!(
            event,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            }
        ) {
            let hit = if window == STANDALONE_WINDOW {
                self.hit_test.hit(point)
            } else {
                self.native_windows
                    .values()
                    .find(|h| h.ui_window == window)
                    .and_then(|h| h.hit_test.hit(point))
            };
            if self
                .window_actions(window)
                .zip(self.gallery_scopes(window))
                .is_some_and(|(route, scope)| {
                    hit.is_some_and(|hit| {
                        route.is_descendant_of(hit, scope.element(SUGGESTION_ROOT + 1))
                            && route.action_for_hit(hit).is_none()
                    })
                })
            {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{activate_gallery_page, install_action_route};
    use vesta_gpui::TextInputAction;

    #[test]
    fn suggestion_acceptance_revalidates_owner_query_ime_and_terminal_lifetime() {
        let mut app = App::new();
        activate_gallery_page(&mut app, "gallery.inputs");
        install_action_route(&mut app);
        app.search_state
            .reset(&mut app.text_context, "Terrain")
            .expect("query");
        app.action_window = STANDALONE_WINDOW;
        app.apply_demo_action(DemoAction::FocusSearch);
        app.prepare_suggestions(STANDALONE_WINDOW);
        let first = app
            .demo
            .suggestions
            .request()
            .expect("current query")
            .clone();
        app.action_window = UiWindowId(999);
        app.choose_suggestion(first.id, 2);
        assert_eq!(app.search_state.text(), "Terrain");
        assert_eq!(app.demo.suggestions.request(), Some(&first));
        app.action_window = STANDALONE_WINDOW;
        app.search_state
            .preedit(&mut app.text_context, "未确认", None)
            .expect("preedit");
        app.choose_suggestion(first.id, 3);
        assert!(app.search_state.is_composing());
        app.search_state
            .reset(&mut app.text_context, "Cu")
            .expect("replacement query");
        app.choose_suggestion(first.id, 2);
        assert_eq!(
            app.search_state.text(),
            "Cu",
            "stale row changed replacement text"
        );
        app.prepare_suggestions(STANDALONE_WINDOW);
        let second = app.demo.suggestions.request().expect("new request").clone();
        assert_ne!(first.id, second.id);
        app.choose_suggestion(second.id, 0);
        assert_eq!(app.search_state.text(), "Cube");
        assert!(
            app.search_state
                .act(&mut app.text_context, TextInputAction::Undo),
            "accepting a candidate must remain one ordinary undoable text edit"
        );
        assert_eq!(app.search_state.text(), "Cu");
        assert!(
            app.search_state
                .act(&mut app.text_context, TextInputAction::Redo)
        );
        assert_eq!(app.search_state.text(), "Cube");
        assert!(app.demo.suggestions.request().is_none());
        app.choose_suggestion(second.id, 1);
        assert_eq!(app.search_state.text(), "Cube", "old action accepted twice");
        app.prepare_suggestions(STANDALONE_WINDOW);
        assert!(
            app.demo.suggestions.request().is_none(),
            "accept reopened candidates"
        );
        app.apply_demo_action(DemoAction::FocusSearch);
        app.prepare_suggestions(STANDALONE_WINDOW);
        assert!(app.handle_suggestion_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::Escape)));
        assert_eq!(app.search_state.text(), "Cube");
        app.apply_demo_action(DemoAction::FocusSearch);
        app.prepare_suggestions(STANDALONE_WINDOW);
        let cancelled = app.demo.suggestions.request().expect("reopened").clone();
        app.open_demo_dialog(1);
        assert!(
            app.demo.suggestions.request().is_none(),
            "modal retained candidates"
        );
        app.choose_suggestion(cancelled.id, 1);
        assert_eq!(app.search_state.text(), "Cube");

        let mut held = App::new();
        activate_gallery_page(&mut held, "gallery.inputs");
        install_action_route(&mut held);
        held.search_state
            .reset(&mut held.text_context, "")
            .expect("long candidate query");
        held.action_window = STANDALONE_WINDOW;
        held.apply_demo_action(DemoAction::FocusSearch);
        held.prepare_suggestions(STANDALONE_WINDOW);
        let drag = held
            .demo
            .suggestions
            .scroll()
            .begin_thumb_drag(Rect::new(0.0, 0.0, 300.0, 140.0), Point::new(296.0, 8.0))
            .expect("actual long-list thumb");
        held.demo.suggestion_drag = Some(drag);
        let list_target = InputTarget {
            window: STANDALONE_WINDOW,
            element: held.scopes.gallery.element(SUGGESTION_ROOT + 1),
        };
        held.input_ownership
            .capture_pointer(list_target, PointerButton::Primary);
        assert!(held.handle_window_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Enter)));
        assert_eq!(held.search_state.text(), "Cube");
        assert!(held.demo.suggestions.request().is_none());
        assert!(
            held.demo.suggestion_drag.is_none(),
            "accepted request must retire the still-held candidate thumb"
        );
        assert!(held.demo.suggestion_scroll_node.is_none());
        assert!(held.input_ownership.capture().is_none());
    }
}
