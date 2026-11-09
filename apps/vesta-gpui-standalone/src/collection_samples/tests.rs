use super::*;

fn app() -> App {
    let mut app = App::new();
    let item =
        workspace_session::ensure_streaming_tab(&mut app.demo.workspace).expect("sample tab");
    app.demo.workspace.activate(item).expect("live tab");
    app.demo.sync_workspace_ui();
    app
}

fn frame(app: &mut App, width: f32) -> (Element, LayoutTree) {
    for _ in 0..FRAME_LAYOUT_PASS_LIMIT {
        let root = crate::tests::install_action_route_with_viewport(app, Size::new(width, 900.0));
        let layout = LayoutEngine
            .compute_with_text_context(
                &root,
                Viewport::new(width, 900.0, 1.5),
                &mut app.text_context,
            )
            .expect("native Host tree");
        if app
            .collection_samples
            .sync_layout(app.scopes.gallery, &layout)
        {
            continue;
        }
        app.scroll_geometry = ScrollGeometry::from_layout(
            app.scopes.gallery,
            &layout,
            app.demo.page_scroll.keys().copied(),
        );
        app.hit_test = HitTestTree::build(&root, &layout);
        let scope = FocusScope::new(STANDALONE_WINDOW, &app.ui_actions, None);
        app.collection_samples.commit_focus(
            app.scopes.gallery,
            STANDALONE_WINDOW,
            &scope,
            &mut app.input_ownership,
        );
        if let Some(target) = scope.resolve_request(&mut app.input_ownership) {
            app.input_ownership.set_focus(target);
            continue;
        }
        return (root, layout);
    }
    panic!("collection Host did not settle within the existing frame pass budget")
}

fn message_anchor(app: &App, layout: &LayoutTree) -> (u32, f32) {
    let viewport = layout
        .node(app.scopes.gallery.element(MESSAGE_VIEWPORT))
        .expect("message viewport")
        .scroll_viewport();
    app.demo
        .collection_source
        .messages
        .iter()
        .find_map(|message| {
            let row = layout.node(app.scopes.gallery.element(MESSAGE_BASE + message.id * 8))?;
            (row.rect.bottom() > viewport.y && row.rect.y < viewport.bottom())
                .then_some((message.id, viewport.y - row.rect.y))
        })
        .expect("a visible message")
}

#[test]
fn native_message_host_follows_stream_and_preserves_reader_across_splice_reflow_and_empty() {
    let mut app = app();
    frame(&mut app, 1280.0);
    assert!(app.collection_samples.messages.following);
    app.apply_sample_action(STANDALONE_WINDOW, Action::Append);
    app.apply_sample_action(STANDALONE_WINDOW, Action::Stream);
    frame(&mut app, 1280.0);
    assert_eq!(
        app.collection_samples.messages.scroll.offset_y(),
        app.collection_samples.messages.scroll.max_y()
    );
    app.collection_samples.messages.scroll.scroll_by(-600.0);
    let (_, layout) = frame(&mut app, 1280.0);
    let anchor = message_anchor(&app, &layout);
    assert!(!app.collection_samples.messages.following);
    app.apply_sample_action(STANDALONE_WINDOW, Action::Prepend);
    app.apply_sample_action(STANDALONE_WINDOW, Action::Append);
    let (_, layout) = frame(&mut app, 1280.0);
    assert_eq!(message_anchor(&app, &layout), anchor);
    let (_, layout) = frame(&mut app, 960.0);
    let resized = message_anchor(&app, &layout);
    assert_eq!(resized.0, anchor.0);
    assert!((resized.1 - anchor.1).abs() <= 1.0);
    app.apply_sample_action(STANDALONE_WINDOW, Action::Empty);
    let (root, layout) = frame(&mut app, 960.0);
    assert_eq!(app.collection_samples.messages.scroll.offset_y(), 0.0);
    assert!(app.demo.collection_source.messages.is_empty());
    assert!(
        layout
            .node(app.scopes.gallery.element(MESSAGE_BASE + anchor.0 * 8))
            .is_none()
    );
    assert!(
        root.find(app.scopes.gallery.element(MESSAGE_VIEWPORT))
            .is_some()
    );
    app.apply_sample_action(STANDALONE_WINDOW, Action::Append);
    frame(&mut app, 960.0);
    assert_eq!(app.demo.collection_source.messages.len(), 1);
    assert!(app.collection_samples.messages.following);
}

#[test]
fn native_table_host_scroll_keys_retain_focus_and_source_range_copy_survives_reorder() {
    let mut app = app();
    let (_, layout) = frame(&mut app, 1280.0);
    let child = table::cell_element(app.scopes.gallery, 0, 0);
    assert!(app.ui_actions.focus_order().contains(&child));
    app.input_ownership.set_focus(Some(InputTarget {
        window: STANDALONE_WINDOW,
        element: child,
    }));
    app.apply_sample_action(STANDALONE_WINDOW, Action::Cell(0, 0));
    assert!(app.handle_window_key(STANDALONE_WINDOW, &Key::Named(NamedKey::PageDown)));
    let (root, layout_after) = frame(&mut app, 1280.0);
    let focus = app
        .input_ownership
        .focus()
        .expect("retained grid focus")
        .element;
    assert_eq!(
        app.ui_actions.action_at(focus),
        Some(&DemoAction::Sample(Action::Cell(6, 0)))
    );
    let node = layout_after.node(focus).expect("revealed focused cell");
    assert!(
        node.rect
            .intersection(node.inherited_clip)
            .is_some_and(|r| r.width > 0.0 && r.height > 0.0)
    );
    let scroll = layout
        .node(app.scopes.gallery.element(TABLE_VIEWPORT))
        .expect("wide viewport");
    assert_eq!(scroll.scroll_content, Some((49_152.0, 280_000.0)));
    assert!(
        root.find(table::cell_element(app.scopes.gallery, 0, 200))
            .is_none()
    );
    // A Host-owned selection may span cells that have never been realized.
    app.collection_samples.selection = Some(((6, 0), (8, 40)));
    let copied = app
        .collection_samples
        .copy(&app.demo.collection_source)
        .expect("bounded source rectangle");
    assert_eq!(copied.lines().count(), 3);
    assert_eq!(
        copied
            .lines()
            .next()
            .expect("first row")
            .split('\t')
            .count(),
        41
    );
    assert!(copied.contains("8:40"));
    app.apply_sample_action(STANDALONE_WINDOW, Action::ReverseRows);
    app.apply_sample_action(STANDALONE_WINDOW, Action::MoveColumn);
    frame(&mut app, 1280.0);
    assert_eq!(app.collection_samples.selection, Some(((6, 0), (8, 40))));
    assert_eq!(
        app.ui_actions.action_at(focus),
        Some(&DemoAction::Sample(Action::Cell(6, 0)))
    );
    app.collection_samples.selection = Some(((0, 0), (9999, 511)));
    assert_eq!(
        app.collection_samples
            .copy(&app.demo.collection_source)
            .err(),
        Some("Select up to 4096 cells to copy")
    );
}

#[test]
fn two_window_message_views_measure_independently_over_the_same_source() {
    let mut source = Source::default();
    let mut context = TextInputContext::default();
    register_host_fonts(&mut context);
    let mut narrow = View::default();
    let mut wide = View::default();
    let scopes = [UiScopes::new().gallery, UiScopes::new().gallery];
    let widths = [300.0, 620.0];
    let mut last_heights = Vec::new();
    for (index, view) in [&mut narrow, &mut wide].into_iter().enumerate() {
        for pass in 0..FRAME_LAYOUT_PASS_LIMIT {
            view.prepare(&source, &mut context, None);
            let widget = view
                .messages
                .project(&source, scopes[index], None, WidgetTheme::dark());
            let layout = LayoutEngine
                .compute_with_text_context(
                    &widget.element,
                    Viewport::new(widths[index], VIEW_HEIGHT, 1.0),
                    &mut context,
                )
                .expect("window-specific message layout");
            if view.messages.sync_layout(scopes[index], &layout) {
                assert!(pass + 1 < FRAME_LAYOUT_PASS_LIMIT);
                continue;
            }
            let last = source.messages.last().expect("last message");
            last_heights.push(
                layout
                    .node(scopes[index].element(MESSAGE_BASE + last.id * 8))
                    .expect("visible last row")
                    .rect
                    .height,
            );
            break;
        }
    }
    assert_eq!(last_heights.len(), 2);
    assert!(last_heights[0] > last_heights[1]);
    narrow.messages.scroll.scroll_by(-500.0);
    narrow.prepare(&source, &mut context, None);
    source.change(Action::Append, None);
    narrow.prepare(&source, &mut context, None);
    wide.prepare(&source, &mut context, None);
    assert!(!narrow.messages.following && wide.messages.following);
    assert_eq!(
        wide.messages.scroll.offset_y(),
        wide.messages.scroll.max_y()
    );
}
