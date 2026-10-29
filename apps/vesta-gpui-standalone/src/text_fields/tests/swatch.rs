//! Swatch-specific ownership and mode transitions through the shared Host paths.
use super::*;
use vesta_gpui::TextInputAction;

fn write_hex(app: &mut App, value: &str) {
    app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::ColorHex, None);
    app.color_hex_state
        .act(&mut app.text_context, TextInputAction::SelectAll);
    assert!(app.handle_window_text_event(
        STANDALONE_WINDOW,
        &WindowEvent::Ime(winit::event::Ime::Enabled)
    ));
    assert!(app.handle_window_text_event(
        STANDALONE_WINDOW,
        &WindowEvent::Ime(winit::event::Ime::Commit(value.into()))
    ));
}

pub(super) fn mode_terminals_preserve_the_owned_draft() {
    for mode in [ColorPickerMode::Rgb, ColorPickerMode::Hsv] {
        for apply in [false, true] {
            let mut app = App::new();
            activate_gallery_page(&mut app, "gallery.inputs");
            install_action_route(&mut app);
            let original = app.demo.color_committed;
            let original_hex = format_rgba_hex(original);
            let ramp = app.demo.ramp.snapshot();
            let ramp_undo = app.demo.ramp.undo.len();
            let numeric = (app.demo.number, app.demo.vector);
            app.apply_window_action(STANDALONE_WINDOW, DemoAction::ToggleColorPicker);
            app.apply_window_action(STANDALONE_WINDOW, DemoAction::SelectColorMode(mode));
            install_action_route(&mut app);
            let root = app.scopes.gallery.element(COLOR_PICKER_ROOT);
            let hex = TextFieldKind::ColorHex.element(app.scopes.gallery);
            assert!(app.ui_actions.is_descendant_of(hex, root));
            assert!(
                !app.ui_actions
                    .is_descendant_of(hex, app.scopes.gallery.element(RAMP_ROOT + 10))
            );
            assert_eq!(
                app.ui_actions
                    .action_at(app.scopes.gallery.element(COLOR_PICKER_ROOT + 22)),
                Some(&DemoAction::StepColor(0, -8))
            );
            assert_eq!(
                app.ui_actions
                    .action_at(app.scopes.gallery.element(COLOR_PICKER_ROOT + 54)),
                Some(&DemoAction::StepColor(3, 8))
            );
            write_hex(&mut app, "#FF000080");
            assert_eq!(
                app.demo.color_draft, original,
                "text is still an unaccepted draft"
            );
            assert_eq!(app.demo.color_committed, original);
            let other_mode = match mode {
                ColorPickerMode::Rgb => ColorPickerMode::Hsv,
                ColorPickerMode::Hsv => ColorPickerMode::Rgb,
            };
            app.apply_window_action(STANDALONE_WINDOW, DemoAction::SelectColorMode(other_mode));
            app.apply_window_action(STANDALONE_WINDOW, DemoAction::SelectColorMode(mode));
            install_action_route(&mut app);
            assert_eq!(app.color_hex_state.text(), "#FF000080");
            assert_eq!(
                app.text_session_field(STANDALONE_WINDOW),
                Some(TextFieldKind::ColorHex)
            );
            assert!(
                app.color_hex_state
                    .act(&mut app.text_context, TextInputAction::Undo)
            );
            assert_eq!(app.color_hex_state.text(), original_hex);
            assert!(
                app.color_hex_state
                    .act(&mut app.text_context, TextInputAction::Redo)
            );
            assert_eq!(app.color_hex_state.text(), "#FF000080");
            assert_eq!(app.demo.color_committed, original);
            assert!(app.commit_color_hex_draft());
            assert_eq!(app.demo.color_draft, [255, 0, 0, 128]);
            app.apply_window_action(STANDALONE_WINDOW, DemoAction::StepColor(0, -8));
            app.apply_window_action(STANDALONE_WINDOW, DemoAction::StepColor(3, 8));
            let preview = match mode {
                ColorPickerMode::Rgb => [247, 0, 0, 136],
                ColorPickerMode::Hsv => [255, 0, 34, 136],
            };
            assert_eq!(
                app.demo.color_draft, preview,
                "channel interpretation {mode:?}"
            );
            assert_eq!(app.demo.color_committed, original);
            write_hex(&mut app, "#00FF0080");
            // Exercise the shared handler's owner admission, rather than the
            // outer adapter rejecting a token with no native Host.
            app.action_window = UiWindowId(999);
            for action in [
                DemoAction::ApplyColor,
                DemoAction::CancelColor,
                DemoAction::StepColor(0, 8),
                DemoAction::SelectColorMode(other_mode),
                DemoAction::PickColor,
            ] {
                app.apply_demo_action(action);
                assert!(app.demo.color_open);
                assert_eq!(app.demo.color_target, ColorEditTarget::Swatch);
                assert_eq!(app.demo.color_mode, mode);
                assert_eq!(app.demo.color_draft, preview);
                assert_eq!(app.color_hex_state.text(), "#00FF0080");
                assert_eq!(app.demo.color_committed, original);
                assert!(app.can_edit_color(STANDALONE_WINDOW));
            }
            let terminal = if apply {
                DemoAction::ApplyColor
            } else {
                DemoAction::CancelColor
            };
            app.apply_window_action(STANDALONE_WINDOW, terminal);
            let expected = if apply { [0, 255, 0, 128] } else { original };
            assert_eq!(
                app.demo.color_committed, expected,
                "terminal {mode:?}/{terminal:?}"
            );
            assert_eq!(app.demo.color_draft, expected);
            assert_eq!(app.color_hex_state.text(), format_rgba_hex(expected));
            assert!(!app.demo.color_open && app.demo.color_session.is_none());
            assert!(app.text_session.is_none());
            for late in [DemoAction::ApplyColor, DemoAction::CancelColor] {
                app.apply_window_action(STANDALONE_WINDOW, late);
                assert_eq!(app.demo.color_committed, expected);
                assert!(!app.demo.color_open && app.demo.color_session.is_none());
            }
            assert_eq!(app.demo.ramp.snapshot(), ramp);
            assert_eq!(app.demo.ramp.undo.len(), ramp_undo);
            assert_eq!((app.demo.number, app.demo.vector), numeric);
            eprintln!(
                "swatch-terminal mode={mode:?} apply={apply} mode-transfer/draft-history/channel/source/terminal passed"
            );
        }
    }
}
