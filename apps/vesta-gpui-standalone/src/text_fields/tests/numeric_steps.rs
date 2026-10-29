//! Stepping must refresh the stepped model's snapshot without resetting another draft.
use super::*;
use vesta_gpui::{TextEditKind, TextInputAction};

pub(super) fn qualify() {
    overlay_actions_keep_number_projection_current();
    replacement_capture_survives_numeric_cancellation();
    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.inputs");
    install_action_route(&mut app);
    assert!(app.text_session.is_none());
    app.apply_window_action(STANDALONE_WINDOW, DemoAction::StepNumber(1));
    assert_eq!(app.demo.number, 129);
    assert_eq!(
        app.number_state.text(),
        "129",
        "idle step leaves stale text"
    );
    app.apply_window_action(STANDALONE_WINDOW, DemoAction::StepNumber(-1));
    assert_eq!(app.number_state.text(), "128");

    app.set_text_focus(Some(TextFieldKind::Main));
    app.text_state
        .reset(&mut app.text_context, "base")
        .expect("other editor");
    app.text_state
        .insert(&mut app.text_context, "_", TextEditKind::Typing)
        .expect("other editor history");
    app.text_state
        .preedit(&mut app.text_context, "你", None)
        .expect("other editor composition");
    let composing = app.text_state.text().to_owned();
    for (delta, expected) in [(i32::MAX, NUMBER_MAX), (i32::MIN, NUMBER_MIN)] {
        app.apply_window_action(STANDALONE_WINDOW, DemoAction::StepNumber(delta));
        assert_eq!(app.demo.number, expected);
        assert_eq!(app.number_state.text(), expected.to_string());
        assert_eq!(app.text_state.text(), composing);
        assert!(app.text_state.is_composing());
        assert_eq!(
            app.text_session_field(STANDALONE_WINDOW),
            Some(TextFieldKind::Main)
        );
    }
    app.apply_window_action(STANDALONE_WINDOW, DemoAction::StepVector(1, 1));
    assert_eq!(app.demo.vector, [Some(1.0), Some(1.0), Some(3.0)]);
    assert_eq!(
        app.vector_states[1].text(),
        "1",
        "idle mixed-axis step stays stale"
    );
    assert_eq!(app.text_state.text(), composing);
    assert!(app.text_state.is_composing());
    app.text_state.cancel_compose(&mut app.text_context);
    assert_eq!(app.text_state.text(), "base_");
    assert!(
        app.text_state
            .act(&mut app.text_context, TextInputAction::Undo)
    );
    assert_eq!(app.text_state.text(), "base");
    assert!(
        app.text_state
            .act(&mut app.text_context, TextInputAction::Redo)
    );
    assert_eq!(app.text_state.text(), "base_");

    app.set_text_focus(Some(TextFieldKind::Vector(0)));
    app.vector_states[0]
        .reset(&mut app.text_context, "6.25")
        .expect("independent axis draft");
    app.apply_window_action(STANDALONE_WINDOW, DemoAction::StepVector(1, -2));
    assert_eq!(app.demo.vector, [Some(1.0), Some(-1.0), Some(3.0)]);
    assert_eq!(app.vector_states[1].text(), "-1");
    assert_eq!(app.vector_states[0].text(), "6.25");
    app.apply_window_action(STANDALONE_WINDOW, DemoAction::StepVector(0, 1));
    assert_eq!(app.demo.vector[0], Some(2.0));
    assert_eq!(
        app.vector_states[0].text(),
        "2",
        "active step still replaces its draft"
    );
    let model = app.demo.vector;
    let drafts = app
        .vector_states
        .each_ref()
        .map(|state| state.text().to_owned());
    app.apply_window_action(STANDALONE_WINDOW, DemoAction::StepVector(u8::MAX, 1));
    assert_eq!(app.demo.vector, model);
    assert_eq!(
        app.vector_states
            .each_ref()
            .map(|state| state.text().to_owned()),
        drafts
    );
}

fn replacement_capture_survives_numeric_cancellation() {
    for axis in [None, Some(0), Some(1), Some(2)] {
        for same_target in [false, true] {
            let mut app = App::new();
            activate_gallery_page(&mut app, "gallery.inputs");
            install_action_route(&mut app);
            app.cursor = Point::new(100.0, 100.0);
            let field = axis.map_or(TextFieldKind::Number, TextFieldKind::Vector);
            app.begin_text_selection(STANDALONE_WINDOW, field, Some(app.cursor));
            if let Some(axis) = axis {
                app.begin_vector_scrub(axis);
            } else {
                app.begin_number_scrub();
            }
            app.cursor.x += 8.0;
            assert!(if axis.is_some() {
                app.update_vector_scrub()
            } else {
                app.update_number_scrub()
            });
            let replacement = InputTarget {
                window: STANDALONE_WINDOW,
                element: if same_target {
                    field.element(app.scopes.gallery)
                } else {
                    TextFieldKind::Main.element(app.scopes.gallery)
                },
            };
            app.input_ownership.capture_pointer(
                replacement,
                if same_target {
                    PointerButton::Secondary
                } else {
                    PointerButton::Primary
                },
            );
            let capture = app.input_ownership.capture();
            assert!(if axis.is_some() {
                app.cancel_vector_scrub()
            } else {
                app.cancel_number_scrub()
            });
            assert_eq!(app.demo.number, 128);
            assert_eq!(app.demo.vector, [Some(1.0), None, Some(3.0)]);
            assert_eq!(
                app.input_ownership.capture(),
                capture,
                "old numeric cancellation cleared replacement {axis:?}/{same_target}"
            );
            assert!(!app.cancel_number_scrub() && !app.cancel_vector_scrub());
            assert_eq!(app.input_ownership.capture(), capture);
        }
    }
}

fn overlay_actions_keep_number_projection_current() {
    let mut stale = Vec::new();
    for open in [
        DemoAction::ShowToast,
        DemoAction::TogglePopover,
        DemoAction::OpenDemoDialog,
    ] {
        let mut app = App::new();
        let page = match open {
            DemoAction::TogglePopover => "gallery.inputs",
            DemoAction::OpenDemoDialog => "gallery.layout",
            _ => "gallery.controls",
        };
        activate_gallery_page(&mut app, page);
        // Keep the real openers in this source-only fixture's visible route.
        let viewport = Size::new(1280.0, 2400.0);
        crate::tests::install_action_route_with_viewport(&mut app, viewport);
        let opener = app
            .ui_actions
            .focus_order()
            .iter()
            .copied()
            .find(|id| app.ui_actions.action_at(*id) == Some(&open))
            .unwrap_or_else(|| panic!("current routed opener {open:?}"));
        app.set_focus_target(Some(opener));
        app.apply_window_action(STANDALONE_WINDOW, open);
        crate::tests::install_action_route_with_viewport(&mut app, viewport);
        let finish = match open {
            DemoAction::ShowToast => {
                DemoAction::ToastDetails(app.demo.toast_queue.current().expect("live notice").id)
            }
            DemoAction::TogglePopover => DemoAction::PopoverStep,
            DemoAction::OpenDemoDialog => DemoAction::ConfirmDialog(1),
            _ => unreachable!(),
        };
        assert!(
            app.ui_actions
                .focus_order()
                .iter()
                .any(|id| app.ui_actions.action_at(*id) == Some(&finish)),
            "current routed finish {finish:?}"
        );
        app.apply_window_action(UiWindowId(999), finish);
        assert_eq!(
            app.demo.number, 128,
            "unknown source cannot finish {open:?}"
        );
        assert_eq!(app.number_state.text(), "128");
        app.apply_window_action(STANDALONE_WINDOW, finish);
        assert_eq!(app.demo.number, 129, "live finish {open:?}");
        if app.number_state.text() != "129" {
            stale.push(open);
        }

        // A queued duplicate must not reset a newly opened numeric draft.
        activate_gallery_page(&mut app, "gallery.inputs");
        install_action_route(&mut app);
        app.set_text_focus(Some(TextFieldKind::Number));
        app.number_state
            .reset(&mut app.text_context, "137")
            .expect("next numeric draft");
        app.apply_window_action(STANDALONE_WINDOW, finish);
        assert_eq!(app.demo.number, 129, "duplicate finish {open:?}");
        assert_eq!(app.number_state.text(), "137", "duplicate reset {open:?}");
        assert_eq!(
            app.text_session_field(STANDALONE_WINDOW),
            Some(TextFieldKind::Number)
        );
    }
    assert!(
        stale.is_empty(),
        "overlay number projections stay stale: {stale:?}"
    );
}
