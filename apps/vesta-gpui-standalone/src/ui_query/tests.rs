use super::*;

#[test]
fn semantic_commands_share_editor_undo_and_deduplicate_requests_without_using_repaint_as_revision()
{
    use vesta_gpui::{UiCommand, UiTargetRef};
    reveal_nonfocusable_document_table();
    fn publish(app: &mut App) {
        crate::tests::activate_gallery_page(app, "gallery.inputs");
        let root = crate::tests::install_action_route(app);
        let layout = LayoutEngine
            .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
            .expect("layout");
        app.input_layout_ready = true;
        let snapshot = app
            .capture_query_snapshot(
                STANDALONE_WINDOW,
                app.scopes.gallery,
                &root,
                &layout,
                &app.ui_actions,
                app.input_ownership.focus(),
                None,
            )
            .expect("snapshot");
        app.ui_query.publish_root(
            snapshot,
            root,
            &app.ui_actions,
            &layout,
            format!("{:?}", app.mode),
        );
    }
    let mut app = App::new();
    app.ui_query = Store::new(true);
    app.ui_query.control_enabled = true;
    let original = app.text_state.text().to_owned();
    app.set_window_focus(
        STANDALONE_WINDOW,
        Some(TextFieldKind::Main.element(app.scopes.gallery)),
    );
    publish(&mut app);
    let target = app
        .ui_query
        .owners
        .reference(STANDALONE_WINDOW, app.scopes.gallery.element(TEXT_FIELD))
        .expect("owner");
    app.begin_input_layout(STANDALONE_WINDOW);
    let pending = Operation::Invoke {
        request_id: "pending-projection".into(),
        target,
        command: UiCommand::Focus,
    };
    assert_eq!(app.answer_ui_control(&pending), Err("presentation_pending"));
    assert_eq!(app.text_state.text(), original);
    assert!(!app.input_layout_ready);
    publish(&mut app);
    assert_eq!(
        app.answer_ui_control(&pending),
        Err("presentation_pending"),
        "a rejected request is never replayed after recovery"
    );
    let click_target = InputTarget {
        window: STANDALONE_WINDOW,
        element: target.element,
    };
    let click_point = Point::new(32.0, 124.0);
    let click_at = Instant::now();
    assert_eq!(
        app.text_bridge
            .text_pointer_press(click_target, click_point, click_at, false),
        Some(vesta_gpui_winit::TextPointerAction::Caret)
    );
    let operation = Operation::Invoke {
        request_id: "replace-1".into(),
        target,
        command: UiCommand::ReplaceText {
            expected: original.clone(),
            text: "你好🙂".into(),
        },
    };
    let first = app.answer_ui_control(&operation).expect("replace");
    assert_eq!(app.text_state.text(), "你好🙂");
    assert_eq!(
        app.text_bridge.text_pointer_press(
            click_target,
            click_point,
            click_at + std::time::Duration::from_millis(100),
            false
        ),
        Some(vesta_gpui_winit::TextPointerAction::Caret),
        "accepted programmatic editing cannot complete an old physical click pair"
    );
    assert_eq!(app.answer_ui_control(&operation).expect("retry"), first);
    assert_eq!(app.text_state.text(), "你好🙂");
    let conflict = Operation::Invoke {
        request_id: "replace-1".into(),
        target,
        command: UiCommand::Undo,
    };
    assert_eq!(
        app.answer_ui_control(&conflict).expect_err("conflict"),
        "request_id_conflict"
    );
    publish(&mut app);
    let stale = Operation::Invoke {
        request_id: "replace-2".into(),
        target,
        command: UiCommand::ReplaceText {
            expected: original.clone(),
            text: "stale overwrite".into(),
        },
    };
    app.text_bridge.clear_text_pointer();
    app.text_bridge
        .text_pointer_press(click_target, click_point, click_at, false);
    assert_eq!(
        app.answer_ui_control(&stale).expect_err("stale source"),
        "stale_content"
    );
    assert_eq!(
        app.text_bridge.text_pointer_press(
            click_target,
            click_point,
            click_at + std::time::Duration::from_millis(100),
            false
        ),
        Some(vesta_gpui_winit::TextPointerAction::Word),
        "rejected editing preserves the physical click pair"
    );
    let undo = Operation::Invoke {
        request_id: "undo-1".into(),
        target,
        command: UiCommand::Undo,
    };
    app.answer_ui_control(&undo).expect("undo");
    assert_eq!(app.text_state.text(), original);
    publish(&mut app);
    let foreign = Operation::Invoke {
        request_id: "foreign".into(),
        target: UiTargetRef {
            instance: target.instance + 1,
            ..target
        },
        command: UiCommand::Activate,
    };
    assert_eq!(
        app.answer_ui_control(&foreign).expect_err("foreign"),
        "foreign_instance"
    );
    app.ui_query.control_enabled = false;
    assert_eq!(
        app.answer_ui_control(&undo).expect_err("read only"),
        "control_disabled"
    );
}

fn reveal_nonfocusable_document_table() {
    use vesta_gpui::UiCommand;
    fn publish(app: &mut App, item: ItemId) {
        let size = Size::new(360.0, 640.0);
        for _ in 0..6 {
            let root = crate::tests::install_action_route_with_viewport(app, size);
            let layout = LayoutEngine
                .compute(&root, Viewport::new(size.width, size.height, 1.25))
                .expect("document frame");
            if !app
                .documents
                .sync_layout(
                    app.scopes.markdown,
                    &layout,
                    &mut app.text_context,
                    app.theme,
                )
                .expect("document glyphs")
            {
                app.scroll_geometry =
                    ScrollGeometry::from_layout(app.scopes.gallery, &layout, [item].into_iter());
                app.demo.page_scroll.entry(item).or_default().sync_layout(
                    layout
                        .node(page_scroll_id(app.scopes.gallery, item))
                        .expect("document scroll viewport"),
                );
                let snapshot = app
                    .capture_query_snapshot(
                        STANDALONE_WINDOW,
                        app.scopes.gallery,
                        &root,
                        &layout,
                        &app.ui_actions,
                        app.input_ownership.focus(),
                        None,
                    )
                    .expect("document snapshot");
                app.ui_query.publish_root(
                    snapshot,
                    root,
                    &app.ui_actions,
                    &layout,
                    format!("{:?}", app.mode),
                );
                return;
            }
        }
        panic!("document layout did not converge");
    }
    let mut app = App::new();
    app.ui_query = Store::new(true);
    app.ui_query.control_enabled = true;
    app.apply_demo_action(DemoAction::ShowDocuments);
    let item = ensure_documents_tab(&mut app.demo.workspace).expect("documents item");
    publish(&mut app, item);
    let snapshot = app
        .ui_query
        .latest
        .get(&STANDALONE_WINDOW)
        .expect("published document");
    let table = snapshot
        .nodes()
        .iter()
        .find(|node| {
            node.role == Some(SemanticRole::Table)
                && app
                    .ui_actions
                    .is_descendant_of(node.id, app.scopes.markdown.element(0))
        })
        .expect("ordinary Markdown table");
    assert!(
        table.visible_bounds.is_none(),
        "the narrow document starts with the complete table offscreen"
    );
    assert!(
        !app.ui_actions.focus_order().contains(&table.id),
        "the read-only table is not a keyboard stop"
    );
    let id = table.id;
    let target = app
        .ui_query
        .owners
        .reference(STANDALONE_WINDOW, id)
        .expect("table owner");
    let before_focus = app.input_ownership.focus();
    let before_capture = app.input_ownership.capture();
    let operation = Operation::Invoke {
        request_id: "document-table-reveal".into(),
        target,
        command: UiCommand::Reveal,
    };
    app.answer_ui_control(&operation)
        .expect("admitted Reveal command");
    assert!(
        app.demo.page_scroll[&item].offset_y() > 0.0,
        "completed Reveal must scroll nonfocusable document content instead of silently doing nothing"
    );
    publish(&mut app, item);
    let table = app.ui_query.latest[&STANDALONE_WINDOW]
        .nodes()
        .iter()
        .find(|node| node.id == id)
        .expect("same table identity");
    assert!(
        table
            .visible_bounds
            .expect("revealed table")
            .contains_rect(table.bounds.expect("table geometry")),
        "the full table fits in the current page viewport"
    );
    assert_eq!(
        app.input_ownership.focus(),
        before_focus,
        "Reveal does not steal focus"
    );
    assert_eq!(
        app.input_ownership.capture(),
        before_capture,
        "Reveal does not change a gesture owner"
    );
    let offset = app.demo.page_scroll[&item].offset_y();
    app.answer_ui_control(&operation)
        .expect("deduplicated Reveal receipt");
    assert_eq!(app.demo.page_scroll[&item].offset_y(), offset);
}

#[test]
fn published_queries_pin_pages_and_reject_closed_foreign_or_evicted_lifetimes() {
    let mut app = App::new();
    app.ui_query = Store::new(true);
    let root = Element::text(ElementId::new(1), ElementKey::new("root"), "Public text")
        .styled(Style::default().size(Dimension::Px(100.0), Dimension::Px(100.0)));
    let layout = LayoutEngine
        .compute(&root, Viewport::new(100.0, 100.0, 1.0))
        .expect("layout");
    let route = ActionRoute::<DemoAction>::build(&root, []).expect("route");
    let first = app
        .capture_query_snapshot(
            STANDALONE_WINDOW,
            app.scopes.gallery,
            &root,
            &layout,
            &route,
            None,
            None,
        )
        .expect("frame");
    let identity = first.id;
    app.ui_query.publish(first);
    let operation_for = |instance, revision| Operation::Snapshot {
        instance,
        window: identity.window,
        revision,
        root: None,
        after: None,
        limit: 1,
    };
    let operation = operation_for(identity.instance, None);
    let Reply::Snapshot(pinned) = app
        .ui_query
        .answer(&operation, &[STANDALONE_WINDOW])
        .expect("query")
    else {
        panic!("snapshot");
    };
    for _ in 0..12 {
        let next = app
            .capture_query_snapshot(
                STANDALONE_WINDOW,
                app.scopes.gallery,
                &root,
                &layout,
                &route,
                None,
                None,
            )
            .expect("next");
        app.ui_query.publish(next);
    }
    let old = operation_for(identity.instance, Some(identity.revision));
    assert!(
        matches!(app.ui_query.answer(&old,&[STANDALONE_WINDOW]),Ok(Reply::Snapshot(value)) if value.id==pinned.id)
    );
    let foreign = operation_for(identity.instance + 1, None);
    assert_eq!(
        app.ui_query
            .answer(&foreign, &[STANDALONE_WINDOW])
            .expect_err("foreign"),
        "foreign_instance"
    );
    for _ in 0..9 {
        let next = app
            .capture_query_snapshot(
                STANDALONE_WINDOW,
                app.scopes.gallery,
                &root,
                &layout,
                &route,
                None,
                None,
            )
            .expect("next queried frame");
        app.ui_query.publish(next);
        app.ui_query
            .answer(&operation, &[STANDALONE_WINDOW])
            .expect("pin bounded revision");
    }
    assert_eq!(
        app.ui_query
            .answer(&old, &[STANDALONE_WINDOW])
            .expect_err("evicted"),
        "stale_snapshot"
    );
    assert_eq!(
        app.ui_query.answer(&old, &[]).expect_err("closed"),
        "window_not_found"
    );
    assert!(app.ui_query.latest.is_empty() && app.ui_query.pinned.is_empty());
    // Closing windows must free the cap even when no client sends another query.
    for number in 1..=16 {
        let snapshot = app
            .capture_query_snapshot(
                UiWindowId(number),
                app.scopes.gallery,
                &root,
                &layout,
                &route,
                None,
                None,
            )
            .expect("bounded window");
        app.ui_query.publish(snapshot);
    }
    assert!(app.ui_query.next(UiWindowId(17)).is_none());
    app.ui_query.invalidate_shared();
    assert_eq!(
        app.ui_query.dirty.len(),
        16,
        "shared editors/theme/workspace must invalidate every presented native window"
    );
    app.ui_query.forget(STANDALONE_WINDOW);
    app.ui_query.invalidate_shared();
    assert!(
        !app.ui_query.dirty.contains(&STANDALONE_WINDOW),
        "a completed close cannot resurrect retained dirty-window state"
    );
    assert!(app.ui_query.next(UiWindowId(17)).is_some());
}

#[test]
fn actual_gallery_snapshots_share_host_editors_and_redact_revealed_passwords_in_both_themes() {
    let mut app = App::new();
    app.ui_query = Store::new(true);
    app.demo.password_revealed = true;
    app.password_state
        .reset(&mut app.text_context, "do-not-publish-密码")
        .expect("password");
    app.text_state
        .reset(&mut app.text_context, "你好🙂")
        .expect("text");
    for theme in [WidgetTheme::dark(), WidgetTheme::light()] {
        app.theme = theme;
        crate::tests::activate_gallery_page(&mut app, "gallery.inputs");
        let mut root = crate::tests::install_action_route(&mut app);
        let layout = LayoutEngine
            .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
            .expect("gallery layout");
        let captured = app
            .capture_query_snapshot(
                STANDALONE_WINDOW,
                app.scopes.gallery,
                &root,
                &layout,
                &app.ui_actions,
                None,
                None,
            )
            .expect("gallery snapshot");
        assert!(!format!("{captured:?}").contains("do-not-publish-密码"));
        let editor = captured
            .nodes()
            .iter()
            .find(|node| node.id == TextFieldKind::Main.element(app.scopes.gallery))
            .expect("main editor");
        assert_eq!(editor.value.value.as_deref(), Some("你好🙂"));
        assert!(
            captured
                .nodes()
                .iter()
                .any(|node| node.role == Some(SemanticRole::Tab))
        );
        assert!(
            captured
                .nodes()
                .iter()
                .any(|node| node.action == vesta_gpui::UiActionAvailability::HostUnvalidated)
        );
        if cfg!(target_os = "windows") {
            // A real caption tooltip is appended after initial routing. Its passive
            // nodes must be present in the route used by inspection, without changing
            // the existing controls' owners or exposing the revealed password.
            let close = app.scopes.header.element(WINDOW_CLOSE);
            app.ui_query
                .owners
                .update(&captured, &app.ui_actions)
                .expect("base owners");
            let before = app
                .ui_query
                .owners
                .reference(STANDALONE_WINDOW, close)
                .expect("caption owner");
            let mut pairs = Vec::new();
            let mut pending = vec![&root];
            while let Some(element) = pending.pop() {
                if let Some(action) = app.ui_actions.action_at(element.id) {
                    pairs.push((element.id, *action));
                }
                pending.extend(&element.children);
            }
            let discovery_route = app.ui_actions.clone();
            app.set_window_focus(STANDALONE_WINDOW, Some(close));
            app.observe_window_focus_input(STANDALONE_WINDOW, vesta_gpui::FocusInput::Keyboard);
            let base_children = root.children.len();
            let tooltip_layout = app
                .layout_main_tooltips(
                    &mut root,
                    &discovery_route,
                    Viewport::new(1280.0, 800.0, 1.0),
                    true,
                    None,
                )
                .expect("focused tooltip layout");
            assert!(
                root.children.len() > base_children,
                "real popup was projected"
            );
            let original_actions = pairs.clone();
            let route = crate::window_tooltips::route_after_tooltip(
                &root,
                base_children,
                discovery_route,
                pairs,
            )
            .expect("final tooltip route");
            assert_eq!(route.focus_order(), app.ui_actions.focus_order());
            for (element, action) in original_actions {
                assert_eq!(route.action_at(element), Some(&action));
            }
            let with_tooltip = app
                .capture_query_snapshot(
                    STANDALONE_WINDOW,
                    app.scopes.gallery,
                    &root,
                    &tooltip_layout,
                    &route,
                    app.input_ownership.focus(),
                    None,
                )
                .expect("focused tooltip snapshot stays current");
            assert_eq!(with_tooltip.focus, Some(close));
            let popup = app.scopes.tooltip.element(1);
            assert!(with_tooltip.nodes().iter().any(|node| node.id == popup));
            assert!(!route.focus_order().contains(&popup));
            assert!(!format!("{with_tooltip:?}").contains("do-not-publish-密码"));
            app.ui_query
                .owners
                .update(&with_tooltip, &route)
                .expect("final owners");
            assert_eq!(
                app.ui_query.owners.reference(STANDALONE_WINDOW, close),
                Some(before)
            );
        }
        app.ui_query.publish(captured);
    }
}
