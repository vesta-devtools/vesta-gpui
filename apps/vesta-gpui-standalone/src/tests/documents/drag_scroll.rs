use super::*;

pub(super) fn verify_standalone_drag_scroll() {
    let mut app = App::new();
    app.apply_demo_action(DemoAction::ShowDocuments);
    let item = ensure_documents_tab(&mut app.demo.workspace).expect("document tab");
    let project = |app: &mut App| {
        for _ in 0..6 {
            let root = install_action_route_with_viewport(app, Size::new(1280.0, 240.0));
            let layout = LayoutEngine
                .compute(&root, Viewport::new(1280.0, 240.0, 1.0))
                .expect("short Host layout");
            let changed = app
                .documents
                .sync_layout(
                    app.scopes.markdown,
                    &layout,
                    &mut app.text_context,
                    app.theme,
                )
                .expect("Host fonts");
            app.documents.sync_input(
                STANDALONE_WINDOW,
                app.scopes.markdown,
                &layout,
                &mut app.input_ownership,
            );
            capture_scroll_geometry(app, &layout, &[item]);
            app.demo.page_scroll.entry(item).or_default().sync_layout(
                layout
                    .node(page_scroll_id(app.scopes.gallery, item))
                    .expect("page scroll"),
            );
            if !changed
                && app
                    .ui_actions
                    .focus_order()
                    .contains(&app.scopes.markdown.element(0))
            {
                return;
            }
        }
        panic!("Host document failed to converge");
    };
    project(&mut app);
    let scope = app.scopes.markdown;
    let theme = app.theme;
    let (anchor, initial_focus) = app
        .with_document_input(STANDALONE_WINDOW, |interaction, surface, input| {
            let block = surface.document.block_id_at(0).expect("heading");
            let id = surface
                .document
                .paragraph_id(scope, block)
                .expect("paragraph");
            let root = surface.document.view(scope, surface.range.clone(), theme);
            let vesta_gpui::ElementKind::TextInput { snapshot, .. } =
                &root.find(id).expect("shaped heading").kind
            else {
                panic!("heading snapshot");
            };
            let node = surface.layout.node(id).expect("heading geometry");
            let clip = snapshot
                .viewport(node.rect, node.inherited_clip, Point::default())
                .clip;
            let point = Point::new(clip.x + clip.width * 0.5, clip.y + clip.height * 0.5);
            assert_eq!(
                interaction.press(&surface, point, false, input),
                Ok(vesta_gpui_widgets::MarkdownPointerResult::Selection)
            );
            let document = surface
                .layout
                .node(interaction.target().element)
                .expect("Document");
            let visible = document
                .rect
                .intersection(document.inherited_clip)
                .expect("clipped Document");
            assert!(
                document.rect.height > visible.height,
                "the actual Host fixture has offscreen text"
            );
            interaction
                .pointer_move(
                    &surface,
                    Point::new(point.x, visible.y + visible.height + 80.0),
                    input,
                )
                .expect("held edge");
            let selection = interaction
                .accessibility(&surface)
                .expect("current range projection")
                .selection
                .expect("initial range");
            (selection.anchor, selection.focus)
        })
        .expect("Host input adapter");
    let mut now = Instant::now();
    assert!(
        app.tick_window_document_drag(STANDALONE_WINDOW, now)
            .is_some()
    );
    for _ in 0..20 {
        now += Duration::from_millis(20);
        app.tick_window_document_drag(STANDALONE_WINDOW, now);
        project(&mut app);
    }
    assert!(
        app.demo.page_scroll[&item].offset_y() > 0.0,
        "the real Host timer consumes its retained scroll path"
    );
    app.with_document_input(STANDALONE_WINDOW, |interaction, surface, input| {
        let selection = interaction
            .accessibility(&surface)
            .expect("current range projection")
            .selection
            .expect("advanced range");
        assert_eq!(selection.anchor, anchor);
        assert_ne!(
            selection.focus, initial_focus,
            "the stationary point advances on the new layout"
        );
        assert!(interaction.cancel(input));
        assert!(interaction.drag_scroll_deadline().is_none());
    })
    .expect("Host owner");
    assert!(app.input_ownership.capture().is_none());
    assert!(
        app.tick_window_document_drag(STANDALONE_WINDOW, now)
            .is_none()
    );
}
