//! Exercise the existing document action/key/focus adapters, without an OS window.
use super::*;
use gallery::documents::DocumentAction;

fn frame(app: &mut App, width: f32) -> (Element, LayoutTree) {
    for _ in 0..20 {
        // The generic test builder resolves restores before layout. Re-stage this
        // source's request so this fixture follows the native document frame order.
        let pending = app.input_ownership.pending_focus_target(STANDALONE_WINDOW);
        let (root, regions) = install_action_route_with_regions(app, Size::new(width, 800.0));
        if let Some(target) = pending {
            app.input_ownership.request_focus(target);
        }
        let layout = LayoutEngine
            .compute(&root, Viewport::new(width, 800.0, 1.25))
            .expect("document frame");
        if app
            .documents
            .sync_layout(
                app.scopes.markdown,
                &layout,
                &mut app.text_context,
                app.theme,
            )
            .expect("document shape")
        {
            continue;
        }
        if app.documents.sync_input(
            STANDALONE_WINDOW,
            app.scopes.markdown,
            &layout,
            &mut app.input_ownership,
        ) {
            continue;
        }
        let scope = FocusScope::new(
            STANDALONE_WINDOW,
            &app.ui_actions,
            app.focus_root_for_window(STANDALONE_WINDOW),
        );
        if let Some(restored) = scope.resolve_request(&mut app.input_ownership) {
            if let Some(target) = restored {
                app.reveal_current_layout_focus(
                    STANDALONE_WINDOW,
                    app.scopes.gallery,
                    &layout,
                    &app.ui_actions.clone(),
                    &regions,
                    target.element,
                );
            }
            app.set_focus_target(restored.map(|target| target.element));
            continue;
        }
        let pages = regions
            .panes
            .iter()
            .filter_map(|region| {
                app.demo
                    .workspace
                    .pane(region.pane)?
                    .active_item()
                    .map(|item| item.id)
            })
            .collect::<Vec<_>>();
        app.scroll_geometry =
            ScrollGeometry::from_layout(app.scopes.gallery, &layout, pages.iter().copied())
                .with_tabs(&layout, &regions);
        let mut changed = false;
        for item in pages {
            if let Some(node) = layout.node(page_scroll_id(app.scopes.gallery, item)) {
                changed |= app
                    .demo
                    .page_scroll
                    .entry(item)
                    .or_default()
                    .sync_layout(node);
            }
        }
        if let Some(node) = layout.node(app.scopes.gallery.element(3)) {
            changed |= app.gallery_scroll.sync_layout(node);
        }
        if !changed {
            return (root, layout);
        }
    }
    panic!("document frame failed to settle")
}

fn key(app: &mut App, key: NamedKey, modifiers: ModifiersState) {
    app.text_bridge.update_modifiers(&modifiers.into());
    assert!(
        app.handle_window_key(STANDALONE_WINDOW, &Key::Named(key)),
        "document key {key:?}"
    );
}

fn selection(app: &mut App) -> vesta_gpui_widgets::MarkdownSelection {
    app.with_document_input(STANDALONE_WINDOW, |interaction, _, _| {
        interaction.selection()
    })
    .flatten()
    .expect("accepted document selection")
}

fn focus_row(app: &mut App) -> usize {
    let selected = selection(app);
    app.documents
        .markdown
        .selection_focus_row(STANDALONE_WINDOW, selected)
        .expect("current logical row")
}

fn visible_focus(app: &mut App, layout: &LayoutTree, item: ItemId) {
    let focus = app
        .window_document_focus_rect(STANDALONE_WINDOW, app.scopes.markdown.element(0))
        .expect("current document focus geometry");
    let node = layout
        .node(page_scroll_id(app.scopes.gallery, item))
        .expect("document scroll owner");
    let visible = node
        .rect
        .intersection(node.inherited_clip)
        .expect("visible page");
    assert!(
        visible.contains_rect(focus),
        "focus {focus:?} outside current page {visible:?}; rows {:?}, offset {:?}",
        app.documents.range(),
        app.demo
            .page_scroll
            .get(&item)
            .map(|scroll| scroll.offset())
    );
}

pub(super) fn qualify() {
    let mut app = App::new();
    app.show_documents();
    let item = ensure_documents_tab(&mut app.demo.workspace).expect("document item");
    frame(&mut app, 920.0);
    app.apply_demo_action(DemoAction::Document(DocumentAction::NextSample));
    frame(&mut app, 920.0);
    let len = app.documents.markdown.len();
    assert!(len > 64);
    assert_eq!(
        app.documents.range(),
        0..len,
        "full long view retains continuous scrolling"
    );
    key(
        &mut app,
        NamedKey::End,
        ModifiersState::CONTROL | ModifiersState::SHIFT,
    );
    let (_, layout) = frame(&mut app, 920.0);
    visible_focus(&mut app, &layout, item);
    let accepted = selection(&mut app);
    let original = app
        .documents
        .markdown
        .copy_selection(STANDALONE_WINDOW, accepted, 64 * 1024)
        .expect("full mixed source");
    assert!(original.contains("Paragraph 60") && original.ends_with("Empty\t\tمرحبا 👨‍👩‍👧‍👦"));
    let new_projection = app.new_window_documents().expect("native document factory");
    let all = new_projection
        .markdown
        .select_all(UiWindowId(42))
        .expect("new document range")
        .expect("new document text");
    assert!(
        new_projection
            .markdown
            .copy_selection(UiWindowId(42), all, 64 * 1024)
            .expect("new window content")
            == original,
        "a new native projection must use the current document source, not reset to the brief default"
    );
    let revision = app.documents.markdown.revision();
    app.apply_demo_action(DemoAction::Document(DocumentAction::NextSample));
    assert!(app.documents.range().len() <= 18 && app.documents.range().end == len);
    assert_eq!(
        app.documents.markdown.revision(),
        revision,
        "view policy cannot replace author/source identity"
    );
    let (root, layout) = frame(&mut app, 920.0);
    assert_eq!(selection(&mut app), accepted);
    visible_focus(&mut app, &layout, item);
    let document = root
        .find(app.scopes.markdown.element(0))
        .expect("paged document");
    fn complete_rows(element: &Element) {
        if element
            .semantic
            .as_ref()
            .is_some_and(|semantic| semantic.role == SemanticRole::Row)
        {
            assert_eq!(
                element.children.len(),
                3,
                "a bounded page cannot split a table row"
            );
        }
        for child in &element.children {
            complete_rows(child);
        }
    }
    complete_rows(document);
    frame(&mut app, 360.0);
    assert_eq!(selection(&mut app), accepted);
    assert_eq!(
        app.documents
            .markdown
            .copy_selection(STANDALONE_WINDOW, accepted, 64 * 1024)
            .expect("narrow copy"),
        original
    );
    key(&mut app, NamedKey::Home, ModifiersState::CONTROL);
    let (_, layout) = frame(&mut app, 920.0);
    visible_focus(&mut app, &layout, item);
    let mut last = 0;
    for _ in 0..len * 10 {
        key(&mut app, NamedKey::ArrowDown, ModifiersState::empty());
        frame(&mut app, 920.0);
        let row = focus_row(&mut app);
        assert!(
            row >= last
                && app.documents.range().contains(&row)
                && app.documents.range().len() <= 18
        );
        last = row;
        if row + 1 == len {
            break;
        }
    }
    assert_eq!(
        last + 1,
        len,
        "continuous Down reaches the document end across bounded pages"
    );
    for _ in 0..len * 2 {
        key(&mut app, NamedKey::PageUp, ModifiersState::empty());
        frame(&mut app, 920.0);
        let row = focus_row(&mut app);
        if row == 0 {
            break;
        }
    }
    let row = focus_row(&mut app);
    assert_eq!(row, 0, "PageUp returns across bounded pages");
    key(&mut app, NamedKey::End, ModifiersState::CONTROL);
    assert_eq!(
        app.input_ownership
            .pending_focus_target(STANDALONE_WINDOW)
            .map(|target| target.element),
        Some(app.scopes.markdown.element(0))
    );
    let wheel = WindowEvent::MouseWheel {
        device_id: winit::event::DeviceId::dummy(),
        delta: MouseScrollDelta::LineDelta(0.0, 1.0),
        phase: winit::event::TouchPhase::Moved,
    };
    app.handle_window_document_event(STANDALONE_WINDOW, &wheel);
    assert!(
        app.input_ownership
            .pending_focus_target(STANDALONE_WINDOW)
            .is_none(),
        "manual scroll retires this document's deferred reveal before geometry is ready"
    );
    frame(&mut app, 920.0);
    let foreign = InputTarget {
        window: UiWindowId(42),
        element: app.scopes.markdown.element(0),
    };
    app.input_ownership.request_focus(foreign);
    app.documents.cancel_input(&mut app.input_ownership);
    assert_eq!(
        app.input_ownership.pending_focus_target(foreign.window),
        Some(foreign)
    );
    app.input_ownership.cancel_focus_request();
    app.apply_demo_action(DemoAction::Document(DocumentAction::NextSample));
    frame(&mut app, 920.0);
    assert!(app.documents.markdown.len() < len && app.input_ownership.capture().is_none());
    assert!(
        app.with_document_input(STANDALONE_WINDOW, |interaction, _, _| interaction
            .selection())
            .flatten()
            .is_none(),
        "brief source replacement retires the old accepted selection"
    );
}
