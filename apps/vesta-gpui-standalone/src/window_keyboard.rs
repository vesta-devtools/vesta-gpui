//! Platform-key adapter over shared precedence for every Standalone window.

use super::*;
use vesta_gpui_widgets::{
    ChoiceKeyAction, KeyRouteContext, KeyRouteLayer, KeyRouteOutcome, radio_group_key_action,
    route_key,
};

impl App {
    pub(super) fn handle_window_keyboard_event(
        &mut self,
        window: UiWindowId,
        event: &WindowEvent,
    ) -> bool {
        let WindowEvent::KeyboardInput {
            event,
            is_synthetic,
            ..
        } = event
        else {
            return false;
        };
        self.handle_window_key_input(window, &event.logical_key, event.state, *is_synthetic)
    }

    pub(super) fn handle_window_key_input(
        &mut self,
        window: UiWindowId,
        key: &Key,
        state: ElementState,
        is_synthetic: bool,
    ) -> bool {
        vesta_gpui_winit::is_user_key_press(state, is_synthetic)
            && self.handle_window_key(window, key)
    }

    pub(super) fn window_text_composing(&self, window: UiWindowId) -> bool {
        self.text_session_field(window)
            .is_some_and(|field| match field {
                TextFieldKind::Main => self.text_state.is_composing(),
                TextFieldKind::Search => self.search_state.is_composing(),
                TextFieldKind::Area => self.area_state.is_composing(),
                TextFieldKind::Number => self.number_state.is_composing(),
                TextFieldKind::Vector(axis) => self.vector_states[usize::from(axis)].is_composing(),
                TextFieldKind::ColorHex => self.color_hex_state.is_composing(),
                TextFieldKind::Password => self.password_state.is_composing(),
                TextFieldKind::TableNote => self.table_note_state.is_composing(),
                TextFieldKind::Label => self.readonly_label_state.is_composing(),
            })
    }

    pub(super) fn handle_window_key(&mut self, window: UiWindowId, key: &Key) -> bool {
        if self.window_actions(window).is_none() {
            return false;
        }
        self.action_window = window;
        let text_owner = self.text_session_field(window).and_then(|_| {
            self.window_focus(window)
                .map(|element| InputTarget { window, element })
        });
        let context = KeyRouteContext {
            window,
            text_owner,
            composing: self.window_text_composing(window),
            modal: self.focus_root_for_window(window).is_some(),
        };
        let chord = shortcut_chord(key.as_ref(), self.window_modifiers(window));
        let result = route_key(context, chord, |layer| match layer {
            KeyRouteLayer::Capture => self.capture_key_chord_for(window, key),
            KeyRouteLayer::Menu => self.handle_window_menu_key(window, key),
            KeyRouteLayer::Popup => {
                self.handle_combo_key_for(window, key) || self.dismiss_window_popup_key(window, key)
            }
            KeyRouteLayer::Gesture => {
                key == &Key::Named(NamedKey::Escape) && self.cancel_window_key_gesture(window)
            }
            KeyRouteLayer::TextCommand => {
                self.handle_palette_shortcut_for(window, key)
                    || self.handle_palette_key_for(window, key)
                    || self.handle_inline_edit_key_for(window, key)
                    || self.handle_suggestion_key_for(window, key)
            }
            KeyRouteLayer::FocusedControl => {
                self.handle_window_document_key(window, key)
                    || self.handle_radio_group_key(window, key)
                    || self.handle_scalar_control_key(window, key)
                    || self.handle_window_ramp_key(window, key)
                    || self.handle_collection_key_for(window, key)
                    || self.resize_window_split_with_key(window, key)
            }
            KeyRouteLayer::FocusTraversal => {
                self.traverse_window_focus(window, chord.is_some_and(|c| c.modifiers.shift));
                true
            }
            KeyRouteLayer::Activation => self.activate_window_focus(window),
            KeyRouteLayer::Shortcut => {
                let action = chord.and_then(|chord| self.keymap.resolve(chord)).copied();
                if let Some(action) = action {
                    self.apply_window_action(window, action);
                }
                action.is_some()
            }
        });
        matches!(
            result,
            KeyRouteOutcome::Consumed(_)
                | KeyRouteOutcome::ModalBlocked
                | KeyRouteOutcome::InvalidTextOwner
        )
    }

    fn dismiss_window_popup_key(&mut self, window: UiWindowId, key: &Key) -> bool {
        if self
            .demo
            .popover
            .opener()
            .is_some_and(|opener| opener.window == window)
            && let Some(decision) = shortcut_chord(key.as_ref(), self.window_modifiers(window))
                .and_then(popup_key_action)
        {
            self.close_popover_for(window, PopupCloseReason::Explicit);
            if let PopupKeyAction::TraverseFocus { backwards } = decision {
                self.traverse_window_focus(window, backwards);
            }
            return true;
        }
        if key != &Key::Named(NamedKey::Escape) {
            return false;
        }
        if self.owns_overlay(window) {
            if self.demo.palette_session.is_open() {
                self.close_command_palette();
                return true;
            }
            if let Some(dialog) = self.demo.dialog_stack.top() {
                self.close_demo_dialog(dialog, false);
                return true;
            }
        }
        if window == STANDALONE_WINDOW && self.demo.status_overflow_open {
            self.demo.status_overflow_open = false;
            self.set_window_focus(window, Some(self.scopes.status.element(30)));
            return true;
        }
        self.dismiss_toolbar_overflow(window) || self.dismiss_breadcrumb_overflow(window)
    }

    pub(super) fn handle_window_ramp_key(&mut self, window: UiWindowId, key: &Key) -> bool {
        if self.text_session_field(window).is_some() {
            return false;
        }
        let action = self.window_actions(window).and_then(|route| {
            let element = self.window_focus(window)?;
            if !FocusScope::new(window, route, self.focus_root_for_window(window))
                .accepts(InputTarget { window, element })
            {
                return None;
            }
            ramp_key_action(&self.demo.ramp, route, element, key)
        });
        if let Some(action) = action {
            self.apply_demo_action(action);
        }
        action.is_some()
    }

    pub(super) fn activate_window_focus(&mut self, window: UiWindowId) -> bool {
        let action = self.window_actions(window).and_then(|route| {
            let element = self.window_focus(window)?;
            FocusScope::new(window, route, self.focus_root_for_window(window))
                .accepts(InputTarget { window, element })
                .then(|| route.action_at(element).copied())
                .flatten()
        });
        let Some(action) = action else {
            return false;
        };
        if matches!(
            action,
            DemoAction::Workspace(WorkspaceViewAction::BeginResize { .. })
                | DemoAction::Workspace(WorkspaceViewAction::BeginMoveFloating(_))
                | DemoAction::SetSlider
        ) {
            return false;
        }
        self.apply_window_action(window, action);
        true
    }

    fn resize_window_split_with_key(&mut self, window: UiWindowId, key: &Key) -> bool {
        let action = self.window_actions(window).and_then(|route| {
            let element = self.window_focus(window)?;
            FocusScope::new(window, route, self.focus_root_for_window(window))
                .accepts(InputTarget { window, element })
                .then(|| route.action_at(element).copied())
                .flatten()
        });
        let Some(DemoAction::Workspace(WorkspaceViewAction::BeginResize {
            container,
            split,
            axis,
        })) = action
        else {
            return false;
        };
        let frame = if window == STANDALONE_WINDOW {
            self.dock_frame.split_frame(container, split, axis)
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .and_then(|host| host.dock_frame.split_frame(container, split, axis))
        };
        let (Some(frame), Some(current)) = (
            frame,
            dock_split_ratio(&self.demo.workspace, container, split),
        ) else {
            return false;
        };
        let Some(ratio) = split_ratio_for_key(frame, current, key) else {
            return false;
        };
        self.demo
            .workspace
            .set_split_ratio(container, split, ratio)
            .is_ok()
    }

    fn handle_radio_group_key(&mut self, window: UiWindowId, key: &Key) -> bool {
        let Some(scope) = self.gallery_scopes(window) else {
            return false;
        };
        let decision = self.window_actions(window).and_then(|route| {
            let focused = self.window_focus(window)?;
            if !FocusScope::new(window, route, self.focus_root_for_window(window)).accepts(
                InputTarget {
                    window,
                    element: focused,
                },
            ) {
                return None;
            }
            let DemoAction::SelectRadio(current) = route.action_at(focused)? else {
                return None;
            };
            let enabled = [0_u8, 1]
                .into_iter()
                .filter(|value| {
                    route
                        .action_at(scope.element(RADIO_GROUP_ROOT + 1 + u32::from(*value) * 4))
                        .is_some()
                })
                .collect::<Vec<_>>();
            radio_group_key_action(
                &enabled,
                Some(current),
                shortcut_chord(key.as_ref(), self.window_modifiers(window))?,
            )
        });
        match decision {
            Some(ChoiceKeyAction::Highlight(value) | ChoiceKeyAction::Confirm(value)) => {
                self.action_window = window;
                self.apply_demo_action(DemoAction::SelectRadio(value));
                true
            }
            Some(ChoiceKeyAction::Consumed) => true,
            _ => false,
        }
    }

    fn cancel_window_key_gesture(&mut self, window: UiWindowId) -> bool {
        let values_active = self
            .workspace_resize
            .is_some_and(|drag| drag.target().window == window)
            || if window == STANDALONE_WINDOW {
                self.number_scrub.is_some()
                    || self.vector_scrub.is_some()
                    || self.drawer_resize.is_some()
                    || self.table_resize.is_some()
                    || self.ramp_drag.is_some()
                    || self.slider_drag.is_some()
                    || self.scrollbar_drag.is_some()
            } else {
                self.native_windows
                    .values()
                    .find(|host| host.ui_window == window)
                    .is_some_and(|host| {
                        host.number_scrub.is_some()
                            || host.vector_scrub.is_some()
                            || host.drawer_resize.is_some()
                            || host.table_resize.is_some()
                            || host.ramp_drag.is_some()
                            || host.slider_drag.is_some()
                            || host.scrollbar_drag.is_some()
                    })
            };
        if values_active {
            self.cancel_window_value_edits(window);
            return true;
        }
        let container = if window == STANDALONE_WINDOW {
            Some(ContainerRef::Main)
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .map(|host| ContainerRef::Floating(host.logical_id))
        };
        if self
            .workspace_drag
            .is_some_and(|drag| Some(drag.origin_container) == container)
        {
            self.workspace_drag = None;
            return true;
        }
        if window == STANDALONE_WINDOW
            && (self.floating_move.is_some()
                || self.transform_drag.is_some()
                || self.camera_drag
                || self.camera_pan)
        {
            self.cancel_floating_move();
            self.cancel_transform_drag();
            self.camera_drag = false;
            self.camera_pan = false;
            return true;
        }
        false
    }
}

pub(super) fn demo_keymap() -> Keymap<DemoAction> {
    let mut keymap = Keymap::default();
    for (key, action) in palette_demo::SHORTCUT_COMMANDS {
        keymap
            .bind(KeyChord::character(key, KeyModifiers::default()), action)
            .expect("unique demo shortcut");
        if key.is_ascii_alphabetic() {
            keymap
                .bind(
                    KeyChord::character(
                        key,
                        KeyModifiers {
                            shift: true,
                            ..KeyModifiers::default()
                        },
                    ),
                    action,
                )
                .expect("unique shifted demo shortcut");
        }
    }
    keymap
        .bind(
            KeyChord::character(
                'p',
                KeyModifiers {
                    control: true,
                    shift: true,
                    ..KeyModifiers::default()
                },
            ),
            DemoAction::OpenCommandPalette,
        )
        .expect("unique command palette shortcut");
    keymap
}

pub(super) fn key_chord_caption(chord: KeyChord) -> String {
    let mut parts = Vec::new();
    if chord.modifiers.control {
        parts.push("Ctrl".to_owned());
    }
    if chord.modifiers.alt {
        parts.push("Alt".to_owned());
    }
    if chord.modifiers.shift {
        parts.push("Shift".to_owned());
    }
    if chord.modifiers.super_key {
        parts.push("Super".to_owned());
    }
    parts.push(match chord.key {
        KeyStroke::Character(key) => key.to_uppercase().to_string(),
        other => format!("{other:?}"),
    });
    parts.join("+")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{
        activate_gallery_page, install_action_route, install_action_route_with_viewport,
    };
    #[test]
    fn tab_order_and_keyboard_action_include_the_header_and_compound_widgets() {
        let mut app = App::new();
        activate_gallery_page(&mut app, "gallery.inputs");
        for width in [400.0, 799.0, 800.0, 1280.0] {
            for expanded in [false, true] {
                if expanded {
                    app.open_application_menu(MenuId(0));
                } else {
                    app.close_menu(false);
                }
                let root = install_action_route_with_viewport(&mut app, Size::new(width, 600.0));
                let layout = LayoutEngine
                    .compute(&root, Viewport::new(width, 600.0, 1.25))
                    .expect("header layout");
                let gallery = layout
                    .node(app.scopes.header.element(MODE_2D))
                    .expect("2D button")
                    .rect;
                let viewport = layout
                    .node(app.scopes.header.element(MODE_3D))
                    .expect("3D button")
                    .rect;
                let left = layout
                    .node(app.scopes.header.element(37))
                    .expect("left controls")
                    .rect;
                let right = layout
                    .node(app.scopes.header.element(39))
                    .expect("right controls")
                    .rect;
                assert!(
                    left.right() <= gallery.x && viewport.right() <= right.x,
                    "header commands must have separate reachable hit areas: {left:?}, {gallery:?}, {viewport:?}, {right:?}"
                );
                let hits = HitTestTree::build(&root, &layout);
                for id in [APP_MENU, MODE_2D, MODE_3D, WINDOW_CLOSE] {
                    let element = app.scopes.header.element(id);
                    let rect = layout.node(element).expect("header control").rect;
                    assert_eq!(
                        app.ui_actions
                            .target_for_hit(
                                hits.hit(Point::new(
                                    rect.x + rect.width * 0.5,
                                    rect.y + rect.height * 0.5
                                ))
                                .expect("header hit")
                            )
                            .map(|(id, _)| id),
                        Some(element)
                    );
                }
                for id in [HEADER_FILE_MENU, HEADER_VIEW_MENU, HEADER_WINDOW_MENU] {
                    assert_eq!(
                        app.ui_actions
                            .action_at(app.scopes.header.element(id))
                            .is_some(),
                        expanded
                    );
                }
            }
        }
        app.close_menu(false);
        let mut last_items = Vec::new();
        for width in [900.0, 1280.0] {
            let root = install_action_route_with_viewport(&mut app, Size::new(width, 600.0));
            assert!(
                app.ui_actions
                    .path_to(GallerySectionId::Buttons.element(app.scopes.gallery, 0))
                    .is_none(),
                "hidden Controls page must not contribute its section or focus targets"
            );
            let last_item = app
                .ui_actions
                .focus_order()
                .iter()
                .copied()
                .find(|id| app.ui_actions.action_at(*id) == Some(&DemoAction::SelectList(2)))
                .expect("navigation list's last item");
            let layout = LayoutEngine
                .compute(&root, Viewport::new(width, 600.0, 1.25))
                .expect("gallery layout");
            let header = layout
                .node(app.scopes.header.element(2))
                .expect("header layout")
                .rect;
            for id in [
                APP_MENU,
                HEADER_FILE_MENU,
                HEADER_VIEW_MENU,
                HEADER_WINDOW_MENU,
                MODE_2D,
                MODE_3D,
                THEME_TOGGLE,
                WINDOW_CLOSE,
            ] {
                if let Some(control) = layout.node(app.scopes.header.element(id)) {
                    assert!(
                        control.rect.y >= header.y && control.rect.bottom() <= header.bottom(),
                        "header control escaped its chrome: {:?}",
                        control.rect
                    );
                }
            }
            let status = layout
                .node(app.scopes.status.element(1))
                .expect("status layout")
                .rect;
            let gallery = layout
                .node(app.scopes.gallery.element(3))
                .expect("gallery scroll layout")
                .rect;
            assert!(gallery.bottom() <= status.y);
            assert!(status.bottom() <= 600.0);
            for id in [
                app.scopes.gallery.element(TEXT_FIELD),
                app.scopes.gallery.element(SEARCH_FIELD),
                last_item,
            ] {
                let rect = layout.node(id).expect("control layout").rect;
                assert!(
                    rect.x >= 0.0 && rect.right() <= width,
                    "unreachable control: {rect:?}"
                );
            }
            last_items.push(last_item);
        }
        assert_eq!(
            last_items[0], last_items[1],
            "resize must preserve item identity"
        );
        install_action_route(&mut app);
        app.traverse_focus(false);
        assert_eq!(
            app.input_ownership.focus().map(|focus| focus.element),
            Some(app.scopes.header.element(APP_MENU))
        );
        for id in [MODE_2D, MODE_3D] {
            app.traverse_focus(false);
            assert_eq!(
                app.input_ownership.focus().map(|focus| focus.element),
                Some(app.scopes.header.element(id))
            );
        }
        let replayed = app.handle_window_key_input(
            STANDALONE_WINDOW,
            &Key::Named(NamedKey::Space),
            ElementState::Pressed,
            true,
        );
        assert_eq!(
            app.mode,
            AcceptanceMode::Gallery2d,
            "focus replay must not activate the focused viewport command"
        );
        assert!(!replayed);
        assert!(app.handle_window_key_input(
            STANDALONE_WINDOW,
            &Key::Named(NamedKey::Space),
            ElementState::Pressed,
            false
        ));
        assert_eq!(app.mode, AcceptanceMode::Viewport3d);
        assert_eq!(
            app.input_ownership.focus().map(|focus| focus.element),
            Some(app.scopes.header.element(MODE_3D))
        );
        if cfg!(target_os = "windows") {
            assert_eq!(
                app.ui_actions
                    .action_at(app.scopes.header.element(WINDOW_CLOSE)),
                Some(&DemoAction::WindowClose)
            );
        }
        app.mode = AcceptanceMode::Gallery2d;
        activate_gallery_page(&mut app, "gallery.controls");
        install_action_route(&mut app);
        for mutation in [DemoAction::ToggleSwitch, DemoAction::SelectRadio(1)] {
            for restore in [false, true] {
                app.demo.toggle = true;
                app.demo.radio = 0;
                install_action_route(&mut app);
                let root = GallerySectionId::Conditional.element(app.scopes.gallery, 0);
                let child = app
                    .ui_actions
                    .focus_order()
                    .iter()
                    .copied()
                    .filter(|id| app.ui_actions.is_descendant_of(*id, root))
                    .find(|id| app.ui_actions.action_at(*id) == Some(&DemoAction::StepNumber(1)))
                    .expect("live conditional child");
                let focus = if restore {
                    app.scopes.header.element(MODE_2D)
                } else {
                    child
                };
                app.set_window_focus(STANDALONE_WINDOW, Some(focus));
                assert!(app.pointer_click.press(
                    STANDALONE_WINDOW,
                    PointerButton::Primary,
                    &app.ui_actions,
                    Some(child),
                ));
                app.apply_window_action(STANDALONE_WINDOW, mutation);
                if restore {
                    // Radio selection deliberately stages focus on the new option.
                    if mutation == DemoAction::ToggleSwitch {
                        assert_eq!(
                            app.window_focus(STANDALONE_WINDOW),
                            Some(focus),
                            "unrelated live focus survives without an explicit focus action"
                        );
                    }
                    app.apply_window_action(
                        STANDALONE_WINDOW,
                        match mutation {
                            DemoAction::ToggleSwitch => DemoAction::ToggleSwitch,
                            _ => DemoAction::SelectRadio(0),
                        },
                    );
                } else {
                    assert_eq!(app.window_focus(STANDALONE_WINDOW), None);
                }
                assert!(
                    app.pointer_click
                        .release(
                            STANDALONE_WINDOW,
                            PointerButton::Primary,
                            &app.ui_actions,
                            Some(child),
                        )
                        .is_none(),
                    "disabled/hidden owners must retire before the next route rebuild, even if restored: {mutation:?}/{restore}"
                );
            }
            app.demo.toggle = true;
            app.demo.radio = 0;
            install_action_route(&mut app);
            let root = GallerySectionId::Conditional.element(app.scopes.gallery, 0);
            let parent = app
                .ui_actions
                .focus_order()
                .iter()
                .copied()
                .find(|id| {
                    app.ui_actions.is_descendant_of(*id, root)
                        && app.ui_actions.action_at(*id) == Some(&DemoAction::ToggleSwitch)
                })
                .expect("live conditional parent");
            app.set_window_focus(STANDALONE_WINDOW, Some(parent));
            assert!(app.pointer_click.press(
                STANDALONE_WINDOW,
                PointerButton::Primary,
                &app.ui_actions,
                Some(parent)
            ));
            app.apply_window_action(STANDALONE_WINDOW, mutation);
            if mutation == DemoAction::ToggleSwitch {
                assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(parent));
            }
            assert_eq!(
                app.pointer_click.release(
                    STANDALONE_WINDOW,
                    PointerButton::Primary,
                    &app.ui_actions,
                    Some(parent)
                ),
                Some(&DemoAction::ToggleSwitch),
                "retiring the child must preserve a live parent's pending click"
            );
        }
        app.demo.toggle = true;
        app.demo.radio = 0;
        install_action_route(&mut app);
        let a = app.scopes.gallery.element(RADIO_GROUP_ROOT + 1);
        let b = app.scopes.gallery.element(RADIO_GROUP_ROOT + 5);
        app.set_window_focus(STANDALONE_WINDOW, Some(a));
        for (key, value, focus) in [
            (NamedKey::ArrowRight, 1, b),
            (NamedKey::ArrowRight, 0, a),
            (NamedKey::ArrowLeft, 1, b),
        ] {
            let previous_radio = app.demo.radio;
            let previous_focus = app.window_focus(STANDALONE_WINDOW);
            for (state, synthetic) in [
                (ElementState::Pressed, true),
                (ElementState::Released, true),
                (ElementState::Released, false),
            ] {
                assert!(!app.handle_window_key_input(
                    STANDALONE_WINDOW,
                    &Key::Named(key),
                    state,
                    synthetic
                ));
                assert_eq!(
                    app.demo.radio, previous_radio,
                    "replay/release cannot step a control"
                );
                assert_eq!(app.window_focus(STANDALONE_WINDOW), previous_focus);
            }
            assert!(app.handle_window_key_input(
                STANDALONE_WINDOW,
                &Key::Named(key),
                ElementState::Pressed,
                false
            ));
            install_action_route(&mut app);
            assert_eq!(app.demo.radio, value);
            assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(focus));
            assert!(app.ui_actions.focus_order().contains(&focus));
            assert!(
                !app.ui_actions
                    .focus_order()
                    .contains(&if focus == a { b } else { a })
            );
        }
        app.set_window_focus(STANDALONE_WINDOW, None);
        let shortcut = Key::Character("2".into());
        assert!(!app.handle_window_key_input(
            STANDALONE_WINDOW,
            &shortcut,
            ElementState::Pressed,
            true
        ));
        assert_eq!(
            app.mode,
            AcceptanceMode::Gallery2d,
            "focus replay cannot reach a global shortcut without a focused control"
        );
        assert!(app.handle_window_key_input(
            STANDALONE_WINDOW,
            &shortcut,
            ElementState::Pressed,
            false
        ));
        assert_eq!(app.mode, AcceptanceMode::Viewport3d);
        assert!(!app.handle_window_key_input(
            UiWindowId(999),
            &Key::Named(NamedKey::ArrowRight),
            ElementState::Pressed,
            false
        ));
        assert_eq!(app.demo.radio, 1);
    }
}
