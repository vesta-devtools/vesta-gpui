use super::*;
use winit::keyboard::ModifiersState;

#[test]
fn document_tab_survives_restore_and_each_window_reflows_its_shared_views() {
    let mut app = App::new();
    #[cfg(windows)]
    {
        let glyphs = |snapshot: &vesta_gpui::TextInputSnapshot| {
            let bounds = Rect::new(0.0, 0.0, 1000.0, 100.0);
            let view = vesta_gpui::TextInputViewport {
                bounds,
                clip: bounds,
                origin: Point::default(),
                caret: None,
                scroll_x: 0.0,
                scroll_y: 0.0,
            };
            let mut keys = Vec::new();
            TextRasterizer::default()
                .visit_glyph_refs(snapshot, view, 1.25, |glyph| {
                    if glyph.width > 0 && glyph.height > 0 {
                        keys.push(glyph.key.clone());
                    }
                })
                .expect("document glyphs");
            keys
        };
        let text = "let name = \"地形\";";
        let source = app
            .text_context
            .shape_static_text(text, 13.0, FontRole::Mono)
            .expect("diff code");
        let cjk = app
            .text_context
            .shape_static_text(text, 13.0, FontRole::MonoCjk)
            .expect("CJK reference");
        assert_eq!(
            glyphs(&source),
            glyphs(&cjk),
            "Diff code must use real CJK glyphs rather than missing-glyph boxes"
        );
        let code = app
            .text_context
            .shape_styled_text(text, 13.0, FontRole::Mono, &[], 1000.0)
            .expect("Markdown code");
        let cjk = app
            .text_context
            .shape_styled_text(text, 13.0, FontRole::MonoCjk, &[], 1000.0)
            .expect("styled CJK reference");
        assert_eq!(
            glyphs(&code),
            glyphs(&cjk),
            "Markdown code uses the same explicit CJK fallback"
        );
    }
    let item = ensure_documents_tab(&mut app.demo.workspace).expect("document tab");
    let session = vesta_workspace::WorkspaceSession::capture(&app.demo.workspace).expect("capture");
    let mut restored = session.restore(|_| true).expect("restore").workspace;
    assert_eq!(ensure_documents_tab(&mut restored), Some(item));
    app.demo.workspace = restored;
    app.apply_demo_action(DemoAction::ShowDocuments);
    let mut stable = false;
    for _ in 0..4 {
        let root = install_action_route(&mut app);
        let layout = LayoutEngine
            .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
            .expect("document layout");
        if !app
            .documents
            .sync_layout(
                app.scopes.markdown,
                &layout,
                &mut app.text_context,
                app.theme,
            )
            .expect("document shaping")
        {
            stable = true;
            break;
        }
    }
    assert!(
        stable,
        "styled Markdown must converge before publishing a frame"
    );
    let root = install_action_route(&mut app);
    assert!(root.find(app.scopes.markdown.element(0)).is_some());
    assert!(root.find(app.scopes.diff.element(0)).is_some());
    let native_scopes = UiScopes::new();
    let mut native_documents = GalleryDocuments::default();
    let project_native = |documents: &GalleryDocuments| {
        Element::container(
            native_scopes.root.element(0),
            ElementKey::new("document.float"),
        )
        .styled(
            Style::default()
                .axis(Axis::Column)
                .size(Dimension::Percent(1.0), Dimension::Auto),
        )
        .children(documents.sections(native_scopes, app.theme, &mut Vec::new()))
    };
    let native = project_native(&native_documents);
    let layout = LayoutEngine
        .compute(&native, Viewport::new(320.0, 800.0, 1.0))
        .expect("narrow floating document layout");
    assert!(
        native_documents
            .sync_layout(
                native_scopes.markdown,
                &layout,
                &mut app.text_context,
                app.theme
            )
            .expect("independent window shaping")
    );
    let main_layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("main remains stable");
    assert!(
        !app.documents
            .sync_layout(
                app.scopes.markdown,
                &main_layout,
                &mut app.text_context,
                app.theme
            )
            .expect("native reflow does not overwrite main cache")
    );
    let mut replacement_context = TextInputContext::default();
    for (documents, scope, layout) in [
        (&mut app.documents, app.scopes.markdown, &main_layout),
        (&mut native_documents, native_scopes.markdown, &layout),
    ] {
        assert!(
            documents
                .sync_layout(scope, layout, &mut replacement_context, app.theme)
                .expect("each window invalidates its own font-context cache")
        );
        assert!(
            !documents
                .sync_layout(scope, layout, &mut replacement_context, app.theme)
                .expect("replacement context converges")
        );
        assert!(
            documents
                .sync_layout(scope, layout, &mut app.text_context, app.theme)
                .expect("return to registered host fonts invalidates the cache")
        );
        assert!(
            !documents
                .sync_layout(scope, layout, &mut app.text_context, app.theme)
                .expect("original context converges")
        );
    }
    let native = project_native(&native_documents);
    // Keys are unique within a window's tree. Independent windows may reuse
    // local keys, while their process-wide scoped ElementIds cannot overlap.
    let mut element_ids = std::collections::HashSet::new();
    for source in [&root, &native] {
        ActionRoute::<DemoAction>::build(source, Vec::new()).expect("valid window identity");
        let mut pending = vec![source];
        while let Some(element) = pending.pop() {
            assert!(
                element_ids.insert(element.id),
                "window element IDs must not overlap"
            );
            pending.extend(&element.children);
        }
    }
    app.documents.sync_input(
        STANDALONE_WINDOW,
        app.scopes.markdown,
        &main_layout,
        &mut app.input_ownership,
    );
    let root = install_action_route(&mut app);
    let target = app.scopes.markdown.element(0);
    let link = app
        .documents
        .markdown
        .links_in(STANDALONE_WINDOW, 0..app.documents.markdown.len())
        .next()
        .expect("Gallery link");
    let link_target = app
        .documents
        .markdown
        .link_target(app.scopes.markdown, STANDALONE_WINDOW, link)
        .expect("Gallery link target");
    let link_layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("interactive link geometry");
    app.documents.sync_input(
        STANDALONE_WINDOW,
        app.scopes.markdown,
        &link_layout,
        &mut app.input_ownership,
    );
    assert!(app.ui_actions.focus_order().contains(&link_target.element));
    let mut semantics = AccessibilityTree::default();
    let update = semantics.build(&root, &link_layout, Some(link_target.element), "Documents");
    let native_id = semantics
        .node_for_element(link_target.element)
        .expect("platform link identity");
    let native = &update
        .nodes
        .iter()
        .find(|(id, _)| *id == native_id)
        .expect("platform link")
        .1;
    assert_eq!(native.role(), accesskit::Role::Link);
    assert!(native.supports_action(AccessibilityAction::Click));
    app.set_window_focus(STANDALONE_WINDOW, Some(link_target.element));
    assert_eq!(
        app.with_document_input(STANDALONE_WINDOW, |interaction, surface, input| interaction
            .key(
                &surface,
                KeyChord::new(KeyStroke::Enter, KeyModifiers::default()),
                64 * 1024,
                input
            )),
        Some(Ok(Some(vesta_gpui_widgets::MarkdownKeyIntent::Activate(
            link
        ))))
    );
    let was_smoke = app.smoke;
    app.smoke = true;
    assert!(
        app.activate_window_document_target(STANDALONE_WINDOW, link_target),
        "host activation is wired without launching a browser in smoke"
    );
    app.smoke = was_smoke;
    assert!(
        root.find(target)
            .expect("interactive document")
            .style
            .focusable
    );
    app.set_window_focus(STANDALONE_WINDOW, Some(target));
    let command = KeyModifiers {
        control: true,
        ..Default::default()
    };
    let select_all = KeyChord::character('a', command);
    let copy = KeyChord::character('c', command);
    assert_eq!(
        app.with_document_input(STANDALONE_WINDOW, |interaction, surface, input| interaction
            .key(&surface, select_all, 64 * 1024, input)),
        Some(Ok(Some(vesta_gpui_widgets::MarkdownKeyIntent::Handled)))
    );
    let expected = app
        .documents
        .markdown
        .copy_selection(
            STANDALONE_WINDOW,
            app.documents
                .markdown
                .select_all(STANDALONE_WINDOW)
                .expect("all")
                .expect("nonempty"),
            64 * 1024,
        )
        .expect("rendered source");
    assert!(
        expected.contains("项目\t数值\t说明\nSeed\t42\t固定\nOrigin\t0, 0\t帮助\nEmpty\t\t—"),
        "the ordinary Documents consumer must preserve table cell/row separators, inline content and an empty cell"
    );
    assert_eq!(
        app.with_document_input(STANDALONE_WINDOW, |interaction, surface, input| interaction
            .key(&surface, copy, 64 * 1024, input)),
        Some(Ok(Some(vesta_gpui_widgets::MarkdownKeyIntent::Copy(
            expected.clone()
        ))))
    );
    let mut accessible = app
        .semantics
        .build(&root, &link_layout, Some(target), "Documents");
    app.documents
        .enrich_accessibility(&mut app.semantics, &mut accessible)
        .expect("native document enrichment");
    let document_id = app
        .semantics
        .node_for_element(target)
        .expect("document node");
    let document = &accessible
        .nodes
        .iter()
        .find(|(id, _)| *id == document_id)
        .expect("document projection")
        .1;
    assert_eq!(document.role(), accesskit::Role::Document);
    let platform = *document
        .text_selection()
        .expect("accepted full document selection");
    let consumer = accesskit_consumer::Tree::new(accessible.clone(), true);
    let native_document = consumer
        .state()
        .node_by_id(document_id)
        .expect("host Document");
    assert_eq!(
        native_document.document_range().text(),
        expected,
        "host font projection must retain every rendered byte, including final LF"
    );
    assert_eq!(
        native_document
            .text_selection()
            .expect("host selected range")
            .text(),
        expected,
        "platform range endpoints must include the owner's final LF"
    );
    let mut collapsed = platform;
    collapsed.focus = collapsed.anchor;
    let before_accessibility = app
        .with_document_input(STANDALONE_WINDOW, |interaction, _, _| {
            interaction.selection()
        })
        .expect("controller selection");
    app.window_accessibility_action(
        STANDALONE_WINDOW,
        accesskit::ActionRequest {
            action: AccessibilityAction::SetTextSelection,
            target: document_id,
            data: Some(AccessibilityActionData::SetTextSelection(collapsed)),
        },
    );
    assert_ne!(
        app.with_document_input(STANDALONE_WINDOW, |interaction, _, _| interaction
            .selection())
            .expect("platform selection"),
        before_accessibility,
        "platform requests must reach the existing document controller"
    );
    assert_eq!(
        app.input_ownership.focus().map(|target| target.element),
        Some(target)
    );
    app.window_accessibility_action(
        STANDALONE_WINDOW,
        accesskit::ActionRequest {
            action: AccessibilityAction::SetTextSelection,
            target: document_id,
            data: Some(AccessibilityActionData::SetTextSelection(platform)),
        },
    );
    assert_eq!(
        app.with_document_input(STANDALONE_WINDOW, |interaction, surface, input| interaction
            .key(&surface, copy, 64 * 1024, input)),
        Some(Ok(Some(vesta_gpui_widgets::MarkdownKeyIntent::Copy(
            expected.clone()
        ))))
    );
    let float_window = UiWindowId(0x2000);
    app.with_document_input(STANDALONE_WINDOW, |interaction, surface, input| {
        interaction
            .key(
                &surface,
                KeyChord::new(KeyStroke::Home, command),
                64 * 1024,
                input,
            )
            .expect("document start")
    })
    .expect("main adapter");
    app.demo
        .page_scroll
        .entry(item)
        .or_default()
        .sync_content_height(1000.0, 120.0);
    app.scroll_geometry
        .pages
        .insert(item, Rect::new(0.0, CHROME_HEIGHT, 640.0, 120.0));
    app.text_bridge
        .update_modifiers(&ModifiersState::CONTROL.into());
    assert!(app.handle_window_document_key(STANDALONE_WINDOW, &Key::Named(NamedKey::End)));
    assert!(
        app.demo.page_scroll[&item].offset_y() > 0.0,
        "Ctrl+End must dispatch reveal through the same retained page scroll owner"
    );
    app.text_bridge
        .update_modifiers(&ModifiersState::empty().into());
    app.with_document_input(STANDALONE_WINDOW, |interaction, surface, input| {
        interaction.key(
            &surface,
            KeyChord::new(KeyStroke::Home, command),
            64 * 1024,
            input,
        )
    })
    .expect("main adapter")
    .expect("restart logical navigation");
    for _ in 0..10 {
        assert!(
            app.handle_window_document_key(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowDown))
        );
    }
    assert!(
        app.demo
            .page_scroll
            .get(&item)
            .expect("page scroll")
            .offset_y()
            > 0.0,
        "actual document-key dispatch reveals offscreen focus through the retained page scroll owner"
    );
    assert_eq!(
        app.input_ownership.focus().map(|focus| focus.element),
        Some(target)
    );
    let before_page = app
        .with_document_input(STANDALONE_WINDOW, |interaction, _, _| {
            interaction.selection()
        })
        .expect("main document projection");
    assert!(app.handle_window_document_key(STANDALONE_WINDOW, &Key::Named(NamedKey::PageUp)));
    assert_ne!(
        app.with_document_input(STANDALONE_WINDOW, |interaction, _, _| interaction
            .selection())
            .expect("page navigation projection"),
        before_page,
        "the actual Host dispatch moves the document selection by a viewport page"
    );
    assert_eq!(
        app.input_ownership.focus().map(|focus| focus.element),
        Some(target)
    );
    let mut float_input = InputOwnership::default();
    native_documents.sync_input(
        float_window,
        native_scopes.markdown,
        &layout,
        &mut float_input,
    );
    {
        let (interaction, surface) = native_documents
            .input_parts(STANDALONE_WINDOW)
            .expect("foreign projection");
        assert_eq!(
            interaction.key(&surface, copy, 64 * 1024, &mut float_input),
            Ok(None)
        );
        assert!(interaction.selection().is_none());
    }
    {
        let (interaction, surface) = native_documents
            .input_parts(float_window)
            .expect("float projection");
        float_input.set_focus(Some(interaction.target()));
        assert_eq!(
            interaction.key(&surface, select_all, 64 * 1024, &mut float_input),
            Ok(Some(vesta_gpui_widgets::MarkdownKeyIntent::Handled))
        );
        assert_eq!(
            interaction.key(&surface, copy, 64 * 1024, &mut float_input),
            Ok(Some(vesta_gpui_widgets::MarkdownKeyIntent::Copy(expected)))
        );
        let before = interaction.selection();
        let block = surface.document.block_id_at(0).expect("first heading");
        let paragraph = surface
            .document
            .paragraph_id(native_scopes.markdown, block)
            .expect("heading element");
        let node = surface.layout.node(paragraph).expect("heading geometry");
        let projected =
            surface
                .document
                .view(native_scopes.markdown, 0..surface.document.len(), app.theme);
        let vesta_gpui::ElementKind::TextInput { snapshot, .. } =
            &projected.find(paragraph).expect("heading").kind
        else {
            panic!("shaped heading");
        };
        let clip = snapshot
            .viewport(node.rect, node.inherited_clip, Point::default())
            .clip;
        assert_eq!(
            interaction.press(
                &surface,
                Point::new(clip.x + clip.width * 0.5, clip.y + clip.height * 0.5),
                false,
                &mut float_input
            ),
            Ok(vesta_gpui_widgets::MarkdownPointerResult::Selection)
        );
        assert!(interaction.is_dragging());
        assert!(interaction.cancel(&mut float_input));
        assert_eq!(
            interaction.selection(),
            before,
            "float cancellation restores its own accepted selection"
        );
        interaction
            .key(
                &surface,
                KeyChord::new(KeyStroke::Home, command),
                64 * 1024,
                &mut float_input,
            )
            .expect("floating document start");
        assert_eq!(
            interaction.key(
                &surface,
                KeyChord::new(KeyStroke::ArrowDown, KeyModifiers::default()),
                64 * 1024,
                &mut float_input
            ),
            Ok(Some(vesta_gpui_widgets::MarkdownKeyIntent::RevealSelection))
        );
        assert!(
            interaction
                .selection_focus_rect(&surface)
                .expect("floating geometry")
                .is_some()
        );
    }
    assert!(float_input.capture().is_none());
    assert!(
        app.with_document_input(STANDALONE_WINDOW, |interaction, _, _| interaction
            .selection()
            .is_some())
            .expect("main still bound")
    );
    let absent = LayoutEngine
        .compute(
            &Element::container(native_scopes.root.element(99), ElementKey::new("gone")),
            Viewport::new(320.0, 800.0, 1.0),
        )
        .expect("closed document projection");
    native_documents.sync_input(
        float_window,
        native_scopes.markdown,
        &absent,
        &mut float_input,
    );
    assert!(
        native_documents.input_parts(float_window).is_none(),
        "closed tab cannot keep a stale Copy geometry owner"
    );
    drag_scroll::verify_standalone_drag_scroll();
    paging::qualify();
}

#[path = "documents/drag_scroll.rs"]
mod drag_scroll;
#[path = "documents/paging.rs"]
mod paging;
