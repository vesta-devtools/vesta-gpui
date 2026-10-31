use super::*;
fn publish(app: &mut App, other: UiWindowId) {
    let root = crate::tests::install_action_route(app);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("layout");
    for window in [STANDALONE_WINDOW, other] {
        let snapshot = app
            .capture_query_snapshot(
                window,
                app.scopes.gallery,
                &root,
                &layout,
                &app.ui_actions,
                None,
                None,
            )
            .expect("real preview observation");
        app.ui_query.publish_root(
            snapshot,
            root.clone(),
            &app.ui_actions,
            &layout,
            "cancel-preview".into(),
        );
    }
    app.input_layout_ready = true;
}

#[test]
fn unavailable_window_retires_its_click_sequence_and_shared_preview_publication() {
    let mut app = App::new();
    app.ui_query = Store::new(true);
    app.ui_query.control_enabled = true;
    let other = UiWindowId(42);
    app.native_text_bridges
        .register(other)
        .expect("other bridge");
    let before = app.demo.slider;
    let draft = app.text_state.text().to_owned();
    crate::tests::activate_gallery_page(&mut app, "gallery.controls");
    crate::tests::install_action_route(&mut app);
    let slider = app
        .ui_actions
        .focus_order()
        .iter()
        .copied()
        .find(|id| app.ui_actions.action_at(*id) == Some(&DemoAction::SetSlider))
        .expect("production slider action");
    let target = InputTarget {
        window: STANDALONE_WINDOW,
        element: slider,
    };
    app.input_ownership.set_focus(Some(target));
    app.input_ownership
        .capture_pointer(target, PointerButton::Primary);
    app.slider_drag = Some(ValueEditSession::new(target, before));
    app.apply_demo_action(DemoAction::SetSliderValue(0.91));
    assert_ne!(app.demo.slider, before);
    publish(&mut app, other);
    let old_revision = app.ui_query.latest[&other].id.revision;
    let preview = app.ui_query.latest[&other].clone();
    assert_eq!(
        preview
            .nodes()
            .iter()
            .find(|node| node.id == slider)
            .and_then(|node| node.numeric)
            .map(|value| value.value),
        Some(f64::from(0.91_f32))
    );
    app.ui_query.pin(preview.clone());
    assert!(app.ui_query.dirty.is_empty());
    let start = Instant::now();
    let point = Point::new(10.0, 20.0);
    let text_target = InputTarget {
        window: STANDALONE_WINDOW,
        element: app.scopes.gallery.element(TEXT_FIELD),
    };
    let foreign_target = InputTarget {
        window: other,
        ..text_target
    };
    assert_eq!(
        app.text_bridge
            .text_pointer_press(text_target, point, start, false),
        Some(vesta_gpui_winit::TextPointerAction::Caret)
    );
    assert_eq!(
        app.native_text_bridges
            .get_mut(other)
            .expect("other bridge")
            .text_pointer_press(foreign_target, point, start, false),
        Some(vesta_gpui_winit::TextPointerAction::Caret)
    );
    app.begin_input_layout(STANDALONE_WINDOW);
    app.cancel_unavailable_window_interactions(STANDALONE_WINDOW, ContainerRef::Main);
    let own_click = app.text_bridge.text_pointer_press(
        text_target,
        point,
        start + Duration::from_millis(100),
        false,
    );
    let foreign_click = app
        .native_text_bridges
        .get_mut(other)
        .expect("other bridge")
        .text_pointer_press(
            foreign_target,
            point,
            start + Duration::from_millis(100),
            false,
        );
    let shared_pending = app.ui_query.dirty.contains(&other);
    eprintln!(
        "unavailable_cancel own_click={own_click:?} foreign_click={foreign_click:?} shared_pending={shared_pending} slider={} restored={before}",
        app.demo.slider
    );
    assert_eq!(app.demo.slider, before);
    assert_eq!(app.text_state.text(), draft);
    assert_eq!(app.input_ownership.focus(), Some(target));
    assert!(app.input_ownership.capture().is_none() && app.slider_drag.is_none());
    assert_eq!(
        foreign_click,
        Some(vesta_gpui_winit::TextPointerAction::Word)
    );
    assert!(
        own_click == Some(vesta_gpui_winit::TextPointerAction::Caret) && shared_pending,
        "unavailable retirement must reset its click sequence and invalidate the other window's shared preview"
    );
    assert_eq!(
        app.ui_query.latest[&other].id.revision, old_revision,
        "cancellation is not a presented frame"
    );
    let other_target = app
        .ui_query
        .owners
        .reference(other, slider)
        .expect("other live query owner");
    assert_eq!(
        app.answer_ui_control(&Operation::Invoke {
            request_id: "cancelled-preview-focus".into(),
            target: other_target,
            command: vesta_gpui::UiCommand::Focus,
        }),
        Err("presentation_pending")
    );
    publish(&mut app, other);
    assert!(app.ui_query.dirty.is_empty());
    assert!(app.ui_query.latest[&other].id.revision > old_revision);
    assert_eq!(
        app.ui_query.latest[&other]
            .nodes()
            .iter()
            .find(|node| node.id == slider)
            .and_then(|node| node.numeric)
            .map(|value| value.value),
        Some(f64::from(before))
    );
    assert!(
        app.ui_query
            .pinned
            .iter()
            .any(|snapshot| snapshot.id == preview.id)
    );
    assert_eq!(
        app.text_bridge.text_pointer_press(
            text_target,
            point,
            start + Duration::from_millis(200),
            false
        ),
        Some(vesta_gpui_winit::TextPointerAction::Word),
        "a stable republish preserves a valid click pair"
    );
    app.begin_input_layout(STANDALONE_WINDOW);
    app.cancel_unavailable_window_interactions(STANDALONE_WINDOW, ContainerRef::Main);
    app.cancel_window_value_edits(UiWindowId(99));
    assert_eq!(
        app.ui_query.dirty,
        HashSet::from([STANDALONE_WINDOW]),
        "empty/foreign cleanup does not invalidate a clean other window"
    );
    assert_eq!(app.demo.slider, before);
    assert_eq!(app.text_state.text(), draft);
    assert_eq!(app.input_ownership.focus(), Some(target));
}
