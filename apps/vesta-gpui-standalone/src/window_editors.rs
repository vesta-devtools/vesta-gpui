//! Window-owned transient editor controls and keyboard dispatch (ADR-0270).

use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum NumericDraftExit {
    CancelSourceLoss,
    CommitWorkspaceReset,
}
use vesta_gpui_widgets::{PopupCloseReason, PopupDismissal};
use winit::keyboard::ModifiersState;

impl App {
    pub(super) fn window_modifiers(&self, window: UiWindowId) -> ModifiersState {
        if window == STANDALONE_WINDOW {
            self.text_bridge.modifiers()
        } else {
            self.native_text_bridges
                .get(window)
                .map_or(ModifiersState::empty(), TextInputBridge::modifiers)
        }
    }

    pub(super) fn editor_target(&self, slot: u32) -> Option<InputTarget> {
        Some(InputTarget {
            window: self.action_window,
            element: self.gallery_scopes(self.action_window)?.element(slot),
        })
    }

    pub(super) fn editor_focus_is_live(&self, window: UiWindowId, element: ElementId) -> bool {
        self.window_focus(window) == Some(element)
            && self.window_actions(window).is_some_and(|actions| {
                FocusScope::new(window, actions, self.focus_root_for_window(window))
                    .accepts(InputTarget { window, element })
            })
    }

    pub(super) fn restore_editor_opener(&mut self, opener: InputTarget) {
        self.restore_popup_dismissal(PopupDismissal {
            source_window: opener.window,
            reason: PopupCloseReason::Explicit,
            opener: Some(opener),
        });
    }

    pub(super) fn restore_popup_dismissal(&mut self, dismissal: PopupDismissal) {
        let window = dismissal.source_window;
        let restored = self.window_actions(window).and_then(|actions| {
            dismissal.restore_target(&FocusScope::new(
                window,
                actions,
                self.focus_root_for_window(window),
            ))
        });
        self.set_window_focus(window, restored.map(|target| target.element));
        if let Some(target) = restored {
            self.reveal_window_focus(window, target.element);
        }
    }

    pub(super) fn restore_after_rebuild(&mut self, window: UiWindowId, element: Option<ElementId>) {
        self.set_window_focus(window, None);
        let Some(element) = element else {
            return;
        };
        let target = InputTarget { window, element };
        if window == STANDALONE_WINDOW {
            self.input_ownership.request_focus(target);
        } else if let Some(host) = self
            .native_windows
            .values_mut()
            .find(|host| host.ui_window == window)
        {
            host.input.request_focus(target);
        }
    }

    pub(super) fn close_popover(&mut self, reason: PopupCloseReason) {
        if let Some(opener) = self.demo.popover.opener() {
            self.close_popover_for(opener.window, reason);
        }
    }
    pub(super) fn close_combo(&mut self, reason: PopupCloseReason) {
        if let Some(opener) = self.demo.combo.opener() {
            self.close_combo_for(opener.window, reason);
        }
    }
    pub(super) fn close_combo_for(&mut self, window: UiWindowId, reason: PopupCloseReason) {
        if let Some(dismissal) = self.demo.combo.dismiss(window, reason) {
            self.restore_popup_dismissal(dismissal);
        }
    }
    pub(super) fn close_popover_for(&mut self, window: UiWindowId, reason: PopupCloseReason) {
        if let Some(dismissal) = self.demo.popover.dismiss(window, reason) {
            self.restore_popup_dismissal(dismissal);
        }
    }

    pub(super) fn toggle_combo(&mut self) {
        if self
            .demo
            .combo
            .opener()
            .is_some_and(|opener| opener.window == self.action_window)
        {
            self.close_combo(PopupCloseReason::Explicit);
            return;
        }
        let Some(opener) = self.editor_target(COMBO_TRIGGER) else {
            return;
        };
        self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
        self.close_popover(PopupCloseReason::FocusTransfer);
        self.demo.combo.open_for(opener);
        let Some(gallery) = self.gallery_scopes(opener.window) else {
            return;
        };
        self.set_window_focus(
            opener.window,
            Some(gallery.element(
                COMBO_OPTION_BASE
                    + usize::from(self.demo.combo_selected).min(COMBO_VALUES.len() - 1) as u32,
            )),
        );
    }

    pub(super) fn toggle_popover(&mut self) {
        if self
            .demo
            .popover
            .opener()
            .is_some_and(|opener| opener.window == self.action_window)
        {
            self.close_popover(PopupCloseReason::Explicit);
            return;
        }
        let Some(opener) = self.editor_target(POPOVER_TRIGGER) else {
            return;
        };
        let Some(scopes) = self.window_scopes(opener.window) else {
            return;
        };
        self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
        self.close_combo(PopupCloseReason::FocusTransfer);
        self.demo.popover.open_for(opener);
        self.set_window_focus(opener.window, Some(scopes.popover.element(POPOVER_BUTTON)));
    }

    pub(super) fn dismiss_combo_for_hit(&mut self, hit: Option<ElementId>) {
        if self.demo.combo.opener().is_some_and(|opener| {
            popup_dismiss_for_hit(
                opener,
                self.scopes.gallery.element(COMBO_POPUP),
                STANDALONE_WINDOW,
                hit,
                &self.ui_actions,
            )
        }) {
            self.close_combo(PopupCloseReason::FocusTransfer);
        }
    }

    pub(super) fn dismiss_popover_for_hit(&mut self, hit: Option<ElementId>) {
        if self.demo.popover.opener().is_some_and(|opener| {
            popup_dismiss_for_hit(
                opener,
                self.scopes.popover.element(POPOVER_ROOT),
                STANDALONE_WINDOW,
                hit,
                &self.ui_actions,
            )
        }) {
            let _ = self
                .demo
                .popover
                .dismiss(STANDALONE_WINDOW, PopupCloseReason::OutsidePress);
        }
    }

    pub(super) fn handle_combo_key_for(&mut self, window: UiWindowId, key: &Key) -> bool {
        let Some(opener) = self
            .demo
            .combo
            .opener()
            .filter(|opener| opener.window == window)
        else {
            return false;
        };
        let Some(gallery) = self.gallery_scopes(window) else {
            return false;
        };
        if self.focus_root_for_window(window).is_some() {
            return false;
        }
        let choices = (0..COMBO_VALUES.len())
            .map(|index| gallery.element(COMBO_OPTION_BASE + index as u32))
            .filter(|element| {
                self.window_actions(window)
                    .is_some_and(|route| route.focus_order().contains(element))
            })
            .collect::<Vec<_>>();
        let focused = self
            .window_focus(window)
            .filter(|element| choices.contains(element))
            .or_else(|| {
                let selected =
                    gallery.element(COMBO_OPTION_BASE + u32::from(self.demo.combo_selected));
                choices.contains(&selected).then_some(selected)
            });
        let decision =
            shortcut_chord(key.as_ref(), self.window_modifiers(window)).and_then(|chord| {
                choice_key_action(&choices, focused.as_ref(), chord, ChoiceBoundary::Wrap)
            });
        match decision {
            Some(ChoiceKeyAction::Highlight(element)) => {
                self.set_window_focus(window, Some(element))
            }
            Some(ChoiceKeyAction::Confirm(element)) => {
                if let Some(index) = (0..COMBO_VALUES.len())
                    .position(|index| gallery.element(COMBO_OPTION_BASE + index as u32) == element)
                {
                    self.demo.combo_selected = index as u8;
                }
                self.close_combo(PopupCloseReason::Confirm);
            }
            Some(ChoiceKeyAction::Close) => self.close_combo(PopupCloseReason::Explicit),
            Some(ChoiceKeyAction::TraverseFocus { backwards }) => {
                self.close_combo(PopupCloseReason::TraverseFocus);
                self.traverse_window_focus(window, backwards);
            }
            _ => {}
        }
        if self.demo.combo.opener() == Some(opener)
            && let Some(element) = self.window_focus(window)
        {
            self.reveal_window_focus(window, element);
        }
        true
    }

    pub(super) fn password_display_snapshot(&mut self) -> vesta_gpui::TextInputSnapshot {
        if self.demo.password_revealed {
            self.password_state
                .revealed_snapshot(&mut self.text_context)
        } else {
            self.password_state.snapshot()
        }
    }

    pub(super) fn capture_key_chord_for(&mut self, window: UiWindowId, key: &Key) -> bool {
        let live_focus = self
            .demo
            .key_capture
            .target()
            .is_some_and(|opener| self.editor_focus_is_live(window, opener.element));
        let chord = shortcut_chord(key.as_ref(), self.window_modifiers(window));
        let decision = self
            .demo
            .key_capture
            .key_action(window, live_focus, chord, |chord| {
                self.keymap.resolve(chord).is_some()
            });
        if let Some(KeyCaptureAction::Assigned(chord)) = decision {
            self.demo.key_chord = Some(chord);
        }
        decision.is_some()
    }

    pub(super) fn handle_palette_shortcut_for(&mut self, window: UiWindowId, key: &Key) -> bool {
        let composing = self.window_text_composing(window);
        let menu_open = if window == STANDALONE_WINDOW {
            self.demo.menu.is_open()
        } else {
            self.native_windows
                .values()
                .any(|host| host.ui_window == window && host.pane_menu.is_some())
        };
        if composing
            || menu_open
            || self.demo.palette_session.is_open()
            || self.owns_overlay(window)
            || shortcut_chord(key.as_ref(), self.window_modifiers(window))
                .and_then(|chord| self.keymap.resolve(chord))
                .copied()
                != Some(DemoAction::OpenCommandPalette)
        {
            return false;
        }
        self.action_window = window;
        self.open_command_palette();
        true
    }

    pub(super) fn restore_native_window(&mut self, window_id: WindowId) {
        let Some((logical_id, return_pane)) = self
            .native_windows
            .get(&window_id)
            .map(|host| (host.logical_id, host.return_pane))
        else {
            return;
        };
        match self
            .demo
            .workspace
            .restore_floating_window(logical_id, return_pane)
        {
            Ok(_) => {
                let owned_overlay = self
                    .native_windows
                    .get(&window_id)
                    .is_some_and(|host| self.overlay_owner == Some(host.ui_window));
                if let Some(host) = self.native_windows.get(&window_id) {
                    self.cancel_window_editors(host.ui_window);
                }
                if let Some(host) = self.native_windows.remove(&window_id) {
                    self.ui_query.forget(host.ui_window);
                    self.toast_focus.forget_window(host.ui_window);
                    host.window.set_visible(false);
                    self.native_text_bridges.remove(host.ui_window);
                }
                self.native_window_order.retain(|id| *id != window_id);
                if owned_overlay {
                    self.close_owned_overlays();
                }
                self.demo.sync_workspace_ui();
                self.request_all_redraw();
            }
            Err(error) => {
                eprintln!("restore native floating window {}: {error}", logical_id.0);
                if let Some(host) = self.native_windows.get(&window_id) {
                    host.window.request_redraw();
                }
            }
        }
    }

    pub(super) fn cancel_window_editors(&mut self, window: UiWindowId) {
        self.cancel_window_editors_for(window, PopupCloseReason::WindowUnavailable);
    }
    pub(super) fn cancel_window_editors_for(
        &mut self,
        window: UiWindowId,
        reason: PopupCloseReason,
    ) {
        self.finish_window_editors_for(window, reason, NumericDraftExit::CancelSourceLoss);
    }

    pub(super) fn reset_window_editors(&mut self, window: UiWindowId) {
        self.finish_window_editors_for(
            window,
            PopupCloseReason::OwnerRemoved,
            NumericDraftExit::CommitWorkspaceReset,
        );
    }

    fn finish_window_editors_for(
        &mut self,
        window: UiWindowId,
        reason: PopupCloseReason,
        numeric_exit: NumericDraftExit,
    ) {
        use vesta_gpui::{InteractionCleanupStage as Stage, finish_window_interaction};
        finish_window_interaction(|stage| match stage {
            Stage::CancelPreviews => {
                self.cancel_window_workspace_drag(window);
                self.cancel_window_document_input(window);
                self.cancel_window_value_edits(window);
            }
            Stage::EndTextSession => {
                let reset_numeric = numeric_exit == NumericDraftExit::CommitWorkspaceReset
                    && matches!(
                        self.text_session_field(window),
                        Some(TextFieldKind::Number | TextFieldKind::Vector(_))
                    );
                if !reset_numeric {
                    self.cancel_removed_value_draft(window, reason);
                }
                self.finish_window_text_session(window);
            }
            Stage::DismissTransient => {
                self.dismiss_suggestions(window);
                self.close_window_menu(window, reason);
                self.close_combo_for(window, reason);
                self.close_popover_for(window, reason);
                self.finish_inline_edit_for(window, reason);
                self.finish_command_palette(window, reason);
                // A live window's modal survives activation of another workspace surface.
                // Blur still clears live input ownership in the next cleanup stage.
                if reason != PopupCloseReason::FocusTransfer && self.overlay_owner == Some(window) {
                    while let Some(id) = self.demo.dialog_stack.top() {
                        if self.demo.dialog_stack.dismiss(window, id, reason).is_none() {
                            break;
                        }
                    }
                    self.overlay_owner = None;
                }
                self.demo.key_capture.cancel_for(window);
            }
            Stage::ClearOwnership => {
                if window == STANDALONE_WINDOW {
                    self.input_ownership.cancel_capture();
                    self.input_ownership.set_focus(None);
                    self.input_ownership.cancel_focus_request();
                    self.pointer_click.cancel();
                } else if let Some(host) = self
                    .native_windows
                    .values_mut()
                    .find(|host| host.ui_window == window)
                {
                    host.input.cancel_capture();
                    host.input.set_focus(None);
                    host.input.cancel_focus_request();
                    host.pointer_click.cancel();
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{activate_gallery_page, install_action_route};

    /// The command center must use the live owner, query and availability rather than an
    /// earlier projected row; a repeated activation cannot dispatch the command twice.
    #[test]
    fn palette_revalidates_execution_and_preserves_borrowed_text_and_modal_ownership() {
        let mut app = App::new();
        activate_gallery_page(&mut app, "gallery.inputs");
        install_action_route(&mut app);
        let opener = app.scopes.header.element(MODE_2D);
        app.set_focus_target(Some(opener));
        app.search_state
            .reset(&mut app.text_context, "saved gallery query")
            .expect("saved text");
        app.search_state.act(
            &mut app.text_context,
            vesta_gpui::TextInputAction::SelectAll,
        );
        app.search_state
            .insert(
                &mut app.text_context,
                "redo 中文",
                vesta_gpui::TextEditKind::Paste,
            )
            .expect("history edit");
        app.search_state
            .act(&mut app.text_context, vesta_gpui::TextInputAction::Undo);
        app.open_command_palette();
        let before = app.demo.number;
        app.search_state
            .reset(&mut app.text_context, "increment number")
            .expect("query");
        app.action_window = UiWindowId(999);
        app.execute_palette_command(16);
        assert_eq!(app.demo.number, before, "foreign window executed a command");
        assert!(app.demo.palette_session.is_open());
        app.action_window = STANDALONE_WINDOW;
        app.search_state
            .preedit(&mut app.text_context, "未确认", None)
            .expect("preedit");
        assert!(!app.handle_window_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Escape)));
        app.execute_palette_command(16);
        assert_eq!(app.demo.number, before, "preedit executed a command");
        assert!(app.demo.palette_session.is_open());
        app.search_state
            .reset(&mut app.text_context, "missing command")
            .expect("empty result");
        app.execute_palette_command(16);
        assert_eq!(app.demo.number, before, "stale filtered row executed");
        app.search_state
            .reset(&mut app.text_context, "increment number")
            .expect("query");
        app.execute_palette_command(16);
        app.execute_palette_command(16);
        assert_eq!(app.demo.number, before + 1);
        assert!(!app.demo.palette_session.is_open());
        assert_eq!(app.search_state.text(), "saved gallery query");
        assert_eq!(
            app.search_state.selected_text(),
            Some("saved gallery query")
        );
        app.search_state
            .act(&mut app.text_context, vesta_gpui::TextInputAction::Redo);
        assert_eq!(
            app.search_state.text(),
            "redo 中文",
            "borrowed editor lost Redo"
        );
        app.search_state
            .act(&mut app.text_context, vesta_gpui::TextInputAction::Undo);
        assert_eq!(app.search_state.text(), "saved gallery query");
        install_action_route(&mut app);
        assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(opener));

        app.open_command_palette();
        app.demo.busy = false;
        app.search_state
            .reset(&mut app.text_context, "cancel activity")
            .expect("query");
        app.execute_palette_command(4);
        assert!(!app.demo.busy);
        assert!(
            app.demo.palette_session.is_open(),
            "unavailable command closed the palette"
        );
        app.finish_command_palette(STANDALONE_WINDOW, PopupCloseReason::OutsidePress);
        assert_eq!(app.search_state.text(), "saved gallery query");
        app.open_demo_dialog(1);
        app.open_command_palette();
        assert!(!app.demo.palette_session.is_open());
        assert_eq!(app.demo.dialog_stack.top(), Some(1));
    }

    /// ADR-0254: the palette and modal dialogs belong to the window that opened them, so an
    /// overlay is projected only there and never leaks into another window's tree.
    #[test]
    fn overlays_belong_to_the_window_that_opened_them() {
        let mut app = App::new();
        activate_gallery_page(&mut app, "gallery.inputs");
        install_action_route(&mut app);
        let floating = UiWindowId(0x2000_0007);
        let palette_root = app.scopes.root.element(PALETTE_ROOT);
        let main_focus = app.scopes.header.element(MODE_2D);
        app.set_focus_target(Some(main_focus));

        app.action_window = floating;
        app.open_command_palette();
        assert!(app.demo.palette_session.is_open());
        assert_eq!(app.overlay_owner, Some(floating));
        assert!(app.owns_overlay(floating));
        assert!(!app.owns_overlay(STANDALONE_WINDOW));
        assert_eq!(
            app.focus_root_for_window(STANDALONE_WINDOW),
            None,
            "an overlay in another window cannot trap the main window's Tab order"
        );
        let root = install_action_route(&mut app);
        assert!(
            root.find(palette_root).is_none(),
            "the main window must not project an overlay owned by a floating window"
        );
        app.text_session = Some(TextSession {
            window: floating,
            field: TextFieldKind::Search,
        });
        let before = app.demo.palette.clone();
        assert!(!app.handle_palette_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowDown)));
        assert_eq!(
            app.demo.palette, before,
            "another window cannot navigate this palette"
        );
        assert!(app.handle_palette_key_for(floating, &Key::Named(NamedKey::ArrowDown)));
        app.demo.palette_session = Default::default();
        app.demo.palette_session.open_in(
            floating,
            Some(InputTarget {
                window: floating,
                element: ElementId::new(u64::MAX),
            }),
            TextInputState::default(),
        );
        assert!(app.handle_palette_key_for(floating, &Key::Named(NamedKey::Escape)));
        assert!(!app.demo.palette_session.is_open());
        assert_eq!(
            app.window_focus(STANDALONE_WINDOW),
            Some(main_focus),
            "closing a foreign palette must not clear or assign foreign IDs to main focus"
        );

        app.close_command_palette();
        assert_eq!(app.overlay_owner, None);
        app.action_window = STANDALONE_WINDOW;
        app.open_command_palette();
        assert_eq!(app.overlay_owner, Some(STANDALONE_WINDOW));
        assert_eq!(
            app.focus_root_for_window(STANDALONE_WINDOW),
            Some(palette_root)
        );
        let root = install_action_route(&mut app);
        assert!(
            root.find(palette_root).is_some(),
            "the main window keeps projecting the palette it opened"
        );
        let layout = LayoutEngine
            .compute(&root, Viewport::new(1280.0, 760.0, 1.25))
            .expect("palette layout");
        let header = layout.node(main_focus).expect("background command").rect;
        let hit = HitTestTree::build(&root, &layout).hit(Point::new(
            header.x + header.width / 2.0,
            header.y + header.height / 2.0,
        ));
        assert!(
            hit.is_some_and(|hit| app.ui_actions.is_descendant_of(hit, palette_root)),
            "palette backdrop must cover background chrome hit targets"
        );
        assert!(
            hit.and_then(|hit| app.ui_actions.action_for_hit(hit))
                .is_none(),
            "chrome action must not escape through the command-center backdrop"
        );
        app.close_command_palette();

        app.action_window = floating;
        app.open_demo_dialog(1);
        assert_eq!(
            app.demo.dialog_stack.top(),
            None,
            "a dialog cannot open in a window that does not exist"
        );
        assert_eq!(app.overlay_owner, None);
        app.action_window = STANDALONE_WINDOW;
        app.open_demo_dialog(1);
        assert_eq!(app.overlay_owner, Some(STANDALONE_WINDOW));
        app.open_demo_dialog(2);
        assert_eq!(app.overlay_owner, Some(STANDALONE_WINDOW));
        app.cancel_window_editors_for(floating, PopupCloseReason::FocusTransfer);
        assert_eq!(
            app.demo.dialog_stack.top(),
            Some(2),
            "foreign cleanup cannot cancel this modal"
        );
        app.cancel_window_editors_for(STANDALONE_WINDOW, PopupCloseReason::FocusTransfer);
        assert_eq!(
            app.demo.dialog_stack.top(),
            Some(2),
            "owner blur must retain the live nested modal"
        );
        assert_eq!(app.overlay_owner, Some(STANDALONE_WINDOW));
        assert_eq!(
            app.window_focus(STANDALONE_WINDOW),
            None,
            "blur still clears live input ownership"
        );
        let root = install_action_route(&mut app);
        assert!(root.find(dialog_element(app.scopes.root, 2, 0)).is_some());
        app.close_demo_dialog(2, false);
        assert_eq!(app.overlay_owner, Some(STANDALONE_WINDOW));
        app.close_demo_dialog(1, false);
        assert_eq!(app.overlay_owner, None);
        for reason in [
            PopupCloseReason::OwnerRemoved,
            PopupCloseReason::WindowUnavailable,
        ] {
            app.open_demo_dialog(1);
            app.open_demo_dialog(2);
            app.cancel_window_editors_for(STANDALONE_WINDOW, reason);
            assert!(
                app.demo.dialog_stack.is_empty(),
                "removed owner/window must retire all modals"
            );
            assert_eq!(app.overlay_owner, None);
            assert_eq!(app.window_focus(STANDALONE_WINDOW), None);
        }
    }
}
