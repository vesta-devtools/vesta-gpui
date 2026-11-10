use super::*;

impl View {
    pub(crate) fn commit_focus(
        &mut self,
        gallery: ElementScope,
        window: UiWindowId,
        scope: &FocusScope<'_, DemoAction>,
        input: &mut InputOwnership,
    ) {
        let Some(action) = self.pending_focus.take() else {
            return;
        };
        let element = match action {
            Action::Cell(row, column) => table::cell_element(gallery, row, column),
            Action::Message(id) => gallery.element(MESSAGE_BASE + id * 8 + 2),
            _ => return,
        };
        let target = InputTarget { window, element };
        if scope.accepts(target) {
            input.request_focus(target);
        }
    }
}

impl App {
    pub(crate) fn sample_focus(&self, window: UiWindowId) -> Option<Action> {
        match self
            .window_actions(window)?
            .action_at(self.window_focus(window)?)?
        {
            DemoAction::Sample(action) => Some(*action),
            _ => None,
        }
    }

    pub(crate) fn apply_sample_action(&mut self, window: UiWindowId, action: Action) {
        let shift = self.window_modifiers(window).shift_key();
        let view = if window == STANDALONE_WINDOW {
            Some(&mut self.collection_samples)
        } else {
            self.native_windows
                .values_mut()
                .find(|host| host.ui_window == window)
                .map(|host| &mut host.collection_samples)
        };
        let Some(view) = view else { return };
        let source = &mut self.demo.collection_source;
        if !source.change(action, view.selection.map(|(_, end)| end.1)) {
            match action {
                Action::Latest => view.messages.latest(),
                Action::Message(id) if source.messages.iter().any(|message| message.id == id) => {
                    view.notice = format!("Selected message {id}")
                }
                Action::Cell(row, column)
                    if source.rows.contains(&row) && source.columns.contains(&column) =>
                {
                    let end = (row, column);
                    let anchor = if shift {
                        view.selection.map_or(end, |(anchor, _)| anchor)
                    } else {
                        end
                    };
                    view.selection = Some((anchor, end));
                    view.notice = format!("Selected Asset {row}, field {column}");
                }
                Action::ToggleDirection => view.rtl = !view.rtl,
                Action::ToggleFreeze => view.freeze = !view.freeze,
                Action::Copy => {
                    view.notice = match view.copy(source) {
                        Ok(text) => match self.clipboard.write_text(&text) {
                            Ok(()) => "Selection copied".into(),
                            Err(error) => format!("Copy failed: {error}"),
                        },
                        Err(error) => error.into(),
                    }
                }
                _ => {}
            }
        }
        self.request_all_redraw();
    }

    pub(crate) fn handle_sample_key(&mut self, window: UiWindowId, key: &Key) -> bool {
        if self.text_session_field(window).is_some() || self.owns_overlay(window) {
            return false;
        }
        let Some(focus @ (Action::Cell(..) | Action::Message(..))) = self.sample_focus(window)
        else {
            return false;
        };
        let modifiers = self.window_modifiers(window);
        if matches!(focus, Action::Cell(..))
            && modifiers.control_key()
            && matches!(key, Key::Character(text) if text.eq_ignore_ascii_case("c"))
        {
            self.apply_sample_action(window, Action::Copy);
            return true;
        }
        let view = if window == STANDALONE_WINDOW {
            Some(&self.collection_samples)
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .map(|host| &host.collection_samples)
        };
        let Some(view) = view else { return false };
        let source = &self.demo.collection_source;
        let next = match focus {
            Action::Message(id) => {
                let Some(index) = source.messages.iter().position(|message| message.id == id)
                else {
                    return false;
                };
                let target = match key {
                    Key::Named(NamedKey::ArrowUp) => index.saturating_sub(1),
                    Key::Named(NamedKey::ArrowDown) => (index + 1).min(source.messages.len() - 1),
                    Key::Named(NamedKey::Home) => 0,
                    Key::Named(NamedKey::End) => source.messages.len() - 1,
                    _ => return false,
                };
                Action::Message(source.messages[target].id)
            }
            Action::Cell(row, column) => {
                let Some(mut ri) = source.rows.iter().position(|&id| id == row) else {
                    return false;
                };
                let Some(mut ci) = source.columns.iter().position(|&id| id == column) else {
                    return false;
                };
                let backward = if view.rtl {
                    NamedKey::ArrowRight
                } else {
                    NamedKey::ArrowLeft
                };
                let forward = if view.rtl {
                    NamedKey::ArrowLeft
                } else {
                    NamedKey::ArrowRight
                };
                match key {
                    Key::Named(key) if *key == backward => ci = ci.saturating_sub(1),
                    Key::Named(key) if *key == forward => {
                        ci = (ci + 1).min(source.columns.len() - 1)
                    }
                    Key::Named(NamedKey::ArrowUp) => ri = ri.saturating_sub(1),
                    Key::Named(NamedKey::ArrowDown) => ri = (ri + 1).min(source.rows.len() - 1),
                    Key::Named(NamedKey::PageUp) => ri = ri.saturating_sub(6),
                    Key::Named(NamedKey::PageDown) => ri = (ri + 6).min(source.rows.len() - 1),
                    Key::Named(NamedKey::Home) => {
                        ci = 0;
                        if modifiers.control_key() {
                            ri = 0;
                        }
                    }
                    Key::Named(NamedKey::End) => {
                        ci = source.columns.len() - 1;
                        if modifiers.control_key() {
                            ri = source.rows.len() - 1;
                        }
                    }
                    _ => return false,
                }
                Action::Cell(source.rows[ri], source.columns[ci])
            }
            _ => return false,
        };
        self.apply_sample_action(window, next);
        let view = if window == STANDALONE_WINDOW {
            Some(&mut self.collection_samples)
        } else {
            self.native_windows
                .values_mut()
                .find(|host| host.ui_window == window)
                .map(|host| &mut host.collection_samples)
        };
        if let Some(view) = view {
            view.pending_focus = Some(next);
        }
        true
    }
}
