use super::*;

fn semantic_id(element: &Element, role: SemanticRole) -> Option<ElementId> {
    if element
        .semantic
        .as_ref()
        .is_some_and(|semantic| semantic.role == role)
    {
        return Some(element.id);
    }
    element
        .children
        .iter()
        .find_map(|child| semantic_id(child, role))
}

/// Mirrors a window build: derives the shared scroll geometry the wheel and scrollbar
/// paths read (ADR-0253 decision 7) for the given active pages.
fn capture_scroll_geometry(app: &mut App, layout: &LayoutTree, pages: &[ItemId]) {
    app.scroll_geometry =
        ScrollGeometry::from_layout(app.scopes.gallery, layout, pages.iter().copied());
    for item in pages {
        if let Some(node) = layout.node(page_scroll_id(app.scopes.gallery, *item)) {
            app.page_scroll_rects.insert(*item, node.rect);
        }
    }
}

pub(super) fn install_action_route(app: &mut App) -> Element {
    install_action_route_with_viewport(app, Size::new(1280.0, 800.0))
}

pub(super) fn activate_gallery_page(app: &mut App, panel_type: &str) -> ItemId {
    let item = app
        .demo
        .workspace
        .panes()
        .into_iter()
        .flat_map(|pane| pane.items.iter())
        .find(|item| item.panel_type.as_str() == panel_type)
        .map(|item| item.id)
        .expect("gallery page exists");
    app.demo
        .workspace
        .activate(item)
        .expect("gallery item is live");
    item
}

pub(super) fn install_action_route_with_viewport(app: &mut App, viewport_size: Size) -> Element {
    install_action_route_with_regions(app, viewport_size).0
}

fn install_action_route_with_regions(
    app: &mut App,
    viewport_size: Size,
) -> (Element, WorkspaceViewRegions) {
    install_action_route_for_container(app, viewport_size, ContainerRef::Main)
}

fn install_action_route_for_container(
    app: &mut App,
    viewport_size: Size,
    container: ContainerRef,
) -> (Element, WorkspaceViewRegions) {
    app.area_layout
        .prepare(&mut app.area_state, &mut app.text_context);
    let password_snapshot = app.password_display_snapshot();
    let table_note_snapshot = app.table_note_state.snapshot();
    let built = build_root(
        UiBuildState {
            documents: &app.documents,
            selected_feedback: native_capture::selected_feedback_enabled(app),
            ui_window: STANDALONE_WINDOW,
            menu_open: app.demo.menu.is_open(),
            scopes: app.scopes,
            workspace_container: container,
            mode: app.mode,
            maximized: false,
            theme: app.theme,
            scroll_offset: app.gallery_scroll.offset(),
            viewport_size,
            snapshot: app.text_state.snapshot(),
            focused: app.focused_text_field() == Some(TextFieldKind::Main),
            search_snapshot: app.search_state.snapshot(),
            search_query: app.search_state.text(),
            search_focused: app.focused_text_field() == Some(TextFieldKind::Search),
            area_snapshot: app
                .area_state
                .snapshot()
                .with_viewport_scroll(Point::new(0.0, app.area_scroll_y), app.area_follow_caret),
            area_focused: app.focused_text_field() == Some(TextFieldKind::Area),
            number_snapshot: app.number_state.snapshot(),
            number_focused: app.focused_text_field() == Some(TextFieldKind::Number),
            vector_snapshots: std::array::from_fn(|axis| app.vector_states[axis].snapshot()),
            vector_focused: std::array::from_fn(|axis| {
                app.focused_text_field() == Some(TextFieldKind::Vector(axis as u8))
            }),
            color_hex_snapshot: app.color_hex_state.snapshot(),
            password_mask: app.password_state.masked_value(),
            password_snapshot,
            table_note_snapshot,
            color_hex_focused: app.focused_text_field() == Some(TextFieldKind::ColorHex),
            table_note_focused: app.focused_text_field() == Some(TextFieldKind::TableNote),
            readonly_label: &app.readonly_label_state,
            readonly_label_focused: app.focused_text_field() == Some(TextFieldKind::Label),
            virtual_focused: app.focused_virtual_index(),
            virtual_table_focused: app.focused_virtual_table_index(),
            virtual_table_viewport_height: window_collections::table_viewport_height(
                &app.scroll_geometry,
                app.scopes.gallery,
            ),
            tree_focused: app.focused_tree_index(),
            demo: &app.demo,
            hovered: app.hovered,
            focused_element: app.input_ownership.focus().map(|focus| focus.element),
            dock_drag: app.dock_drag_visual(container),
            popover_anchor: None,
            combo_anchor: None,
            gizmo_mode: app.scene.gizmo.mode,
            frame_stats: app.frame_meter.display,
            overlay_owner: app.overlay_owner,
        },
        &mut app.text_context,
    );
    app.tile_columns = built.tile_columns;
    app.ui_actions = ActionRoute::build(&built.root, built.actions).expect("valid UI actions");
    let scope = FocusScope::new(
        STANDALONE_WINDOW,
        &app.ui_actions,
        app.focus_root_for_window(STANDALONE_WINDOW),
    );
    if let Some(restored) = scope.resolve_request(&mut app.input_ownership) {
        app.set_focus_target(restored.map(|target| target.element));
    }
    (built.root, built.workspace_regions)
}

#[test]
fn tab_context_tear_off_targets_the_clicked_item_and_rejects_a_stale_target() {
    let mut app = App::new();
    let pane = PaneId(2);
    let selected = app.demo.workspace.pane(pane).expect("right pane").items[0].id;
    let clicked = app.demo.workspace.pane(pane).expect("right pane").items[1].id;
    let root = install_action_route(&mut app);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("gallery layout");
    app.hit_test = HitTestTree::build(&root, &layout);
    let tab = app
        .ui_actions
        .focus_order()
        .iter()
        .copied()
        .find(|element| {
            app.ui_actions.action_at(*element)
                == Some(&DemoAction::Workspace(WorkspaceViewAction::Tab(clicked)))
        })
        .expect("inactive tab action");
    let rect = layout.node(tab).expect("inactive tab layout").rect;
    app.open_context_menu(Point::new(
        rect.x + rect.width * 0.5,
        rect.y + rect.height * 0.5,
    ));
    assert_eq!(app.demo.menu.current(), Some(MenuId(6)));
    assert_eq!(
        app.demo.workspace.pane(pane).and_then(|pane| pane.active),
        Some(selected)
    );
    let command = demo_menu(MenuId(6), &app.demo).items[0].action;
    assert_eq!(command, DemoAction::TearOffTab(clicked));
    app.apply_demo_action(command);
    assert_eq!(app.demo.workspace.floating_windows().len(), 1);
    assert!(
        app.demo.workspace.floating_windows()[0]
            .root
            .panes()
            .into_iter()
            .any(|pane| app
                .demo
                .workspace
                .pane(pane)
                .is_some_and(|pane| pane.items.iter().any(|item| item.id == clicked)))
    );
    assert_eq!(
        app.demo.workspace.pane(pane).and_then(|pane| pane.active),
        Some(selected)
    );
    app.demo
        .workspace
        .close_item(clicked)
        .expect("close torn tab");
    app.apply_demo_action(command);
    assert!(app.demo.workspace.item(clicked).is_none());
    assert!(app.demo.workspace.floating_windows().is_empty());
}

mod collection_navigation;
mod collections;
mod dock_lifecycle;
#[path = "documents.rs"]
mod documents;
mod layout;
mod scroll_input;

fn collect_test_elements<'a>(element: &'a Element, out: &mut Vec<&'a Element>) {
    out.push(element);
    for child in &element.children {
        collect_test_elements(child, out);
    }
}

fn collection_window_fixture(
    row: &Element,
    action: DemoAction,
) -> (UiScopes, ElementId, ActionRoute<DemoAction>) {
    let scopes = UiScopes::new();
    let focus = scopes.gallery.element(1_000_001);
    let mut row = row.clone();
    row.id = focus;
    let root = Element::container(
        scopes.root.element(1),
        ElementKey::new("native.collection.fixture"),
    )
    .child(row);
    let route = ActionRoute::build(&root, [(focus, action)]).expect("native collection route");
    (scopes, focus, route)
}

/// ADR-0242 decision 9: a mouse click must keep the open popup alive until the release
/// resolves it. Closing the menu on press rebuilds the tree without the item, so
/// `PointerClick::release` no longer finds the pressed target and the action (for example
/// TearOffTab) is dropped while dragging the same tab still works.
#[test]
fn pressing_a_menu_entry_keeps_the_popup_until_the_click_resolves() {
    let mut app = App::new();
    let pane = PaneId(2);
    let clicked = app.demo.workspace.pane(pane).expect("right pane").items[1].id;

    let root = install_action_route(&mut app);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("gallery layout");
    app.hit_test = HitTestTree::build(&root, &layout);
    let tab = app
        .ui_actions
        .focus_order()
        .iter()
        .copied()
        .find(|element| {
            app.ui_actions.action_at(*element)
                == Some(&DemoAction::Workspace(WorkspaceViewAction::Tab(clicked)))
        })
        .expect("inactive tab action");
    let rect = layout.node(tab).expect("inactive tab layout").rect;
    app.open_context_menu(Point::new(
        rect.x + rect.width * 0.5,
        rect.y + rect.height * 0.5,
    ));

    // The host paints the tab menu before the pointer is released.
    let root = install_action_route(&mut app);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("menu layout");
    app.hit_test = HitTestTree::build(&root, &layout);
    let mut elements = Vec::new();
    collect_test_elements(&root, &mut elements);
    let item = elements
        .iter()
        .copied()
        .find(|element| {
            app.ui_actions.action_at(element.id) == Some(&DemoAction::TearOffTab(clicked))
        })
        .expect("tear off menu entry");
    let item_rect = layout.node(item.id).expect("menu entry layout").rect;
    let item_center = Point::new(
        item_rect.x + item_rect.width * 0.5,
        item_rect.y + item_rect.height * 0.5,
    );
    app.cursor = item_center;
    app.pointer_press();
    assert!(
        app.demo.menu.is_open(),
        "pressing a menu entry must not close the popup before the release"
    );

    // The press requests a frame; the release then resolves the same routed target.
    let root = install_action_route(&mut app);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("rebuilt menu layout");
    app.hit_test = HitTestTree::build(&root, &layout);
    let released = app
        .pointer_click
        .release(
            STANDALONE_WINDOW,
            PointerButton::Primary,
            &app.ui_actions,
            app.hit_test.hit(app.cursor),
        )
        .copied();
    assert_eq!(released, Some(DemoAction::TearOffTab(clicked)));

    // A press outside the popup still dismisses the menu and keeps its own action.
    let mut elements = Vec::new();
    collect_test_elements(&root, &mut elements);
    let stepper = elements
        .iter()
        .copied()
        .find(|element| {
            matches!(
                app.ui_actions.action_at(element.id),
                Some(DemoAction::StepNumber(_))
            )
        })
        .expect("gallery stepper button");
    let stepper_rect = layout.node(stepper.id).expect("stepper layout").rect;
    app.cursor = Point::new(
        stepper_rect.x + stepper_rect.width * 0.5,
        stepper_rect.y + stepper_rect.height * 0.5,
    );
    app.pointer_press();
    assert!(
        !app.demo.menu.is_open(),
        "a press outside the popup still dismisses the menu"
    );
}

#[test]
fn gallery_password_masks_secret_and_blocks_copy_cut_commands() {
    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.inputs");
    app.password_state
        .reset(&mut app.text_context, "private-value")
        .expect("password");
    let root = app.scopes.gallery.element(PASSWORD_ROOT);
    let display = app.scopes.gallery.element(PASSWORD_ROOT + 1);
    let hidden = install_action_route(&mut app);
    assert!(matches!(
        &hidden.find(display).expect("masked display").kind,
        vesta_gpui::ElementKind::TextInput { .. }
    ));
    app.apply_demo_action(DemoAction::FocusPassword);
    assert_eq!(
        app.text_session_field(STANDALONE_WINDOW),
        Some(TextFieldKind::Password)
    );
    assert_eq!(app.text_session_field(UiWindowId(77)), None);
    assert!(!app.handle_window_text_event(
        UiWindowId(77),
        &WindowEvent::Ime(winit::event::Ime::Commit("foreign".to_owned()))
    ));
    assert_eq!(app.password_state.text(), "private-value");
    assert!(app.handle_window_text_event(
        STANDALONE_WINDOW,
        &WindowEvent::Ime(winit::event::Ime::Enabled)
    ));
    app.password_state.act(
        &mut app.text_context,
        vesta_gpui::TextInputAction::TextEnd { extend: false },
    );
    assert!(app.handle_window_text_event(
        STANDALONE_WINDOW,
        &WindowEvent::Ime(winit::event::Ime::Commit("z".to_owned()))
    ));
    assert_eq!(app.password_state.text(), "private-valuez");
    app.password_state.act(
        &mut app.text_context,
        vesta_gpui::TextInputAction::SelectAll,
    );
    app.password_state
        .insert(
            &mut app.text_context,
            "Clipboard 中文 😀",
            vesta_gpui::TextEditKind::Paste,
        )
        .expect("replace selection");
    assert_eq!(app.password_state.text(), "Clipboard 中文 😀");
    app.password_state
        .act(&mut app.text_context, vesta_gpui::TextInputAction::Undo);
    assert_eq!(app.password_state.text(), "private-valuez");
    app.apply_demo_action(DemoAction::TogglePasswordReveal);
    let revealed = install_action_route(&mut app);
    app.set_focus_target(Some(app.scopes.gallery.element(PASSWORD_ROOT + 3)));
    assert!(
        app.demo.password_revealed,
        "reveal button belongs to the field"
    );
    app.apply_demo_action(DemoAction::TogglePasswordReveal);
    assert!(
        !app.demo.password_revealed,
        "Hide must not reveal again during focus transfer"
    );
    app.apply_demo_action(DemoAction::TogglePasswordReveal);
    assert!(matches!(
        &revealed.find(display).expect("revealed display").kind,
        vesta_gpui::ElementKind::TextInput { .. }
    ));
    for _ in 0..2 {
        app.traverse_focus(false);
        assert!(app.demo.password_revealed, "field controls retain reveal");
    }
    app.traverse_focus(false);
    assert!(!app.demo.password_revealed, "Tab out hides the secret");
    app.set_focus_target(Some(root));
    app.apply_demo_action(DemoAction::TogglePasswordReveal);
    let layout = LayoutEngine
        .compute(&revealed, Viewport::new(1280.0, 2400.0, 1.0))
        .expect("password semantics layout");
    let mut update = app.semantics.build(
        &revealed,
        &layout,
        app.input_ownership.focus().map(|focus| focus.element),
        WINDOW_TITLE,
    );
    let password_node = app
        .semantics
        .node_for_element(root)
        .expect("password semantic node");
    for (element, state) in [
        (app.scopes.gallery.element(TEXT_FIELD), &mut app.text_state),
        (
            app.scopes.gallery.element(SEARCH_FIELD),
            &mut app.search_state,
        ),
        (app.scopes.gallery.element(TEXT_AREA), &mut app.area_state),
        (root, &mut app.password_state),
    ] {
        app.semantics.enrich_text_input(
            &mut update,
            element,
            state,
            &mut app.text_context,
            Point::new(0.0, 0.0),
        );
    }
    assert_eq!(update.focus, password_node);
    app.set_focus_target(None);
    assert!(!app.demo.password_revealed);
    app.apply_demo_action(DemoAction::ClearPassword);
    assert!(app.password_state.is_empty());
    app.password_state
        .act(&mut app.text_context, vesta_gpui::TextInputAction::Undo);
    assert_eq!(app.password_state.text(), "private-valuez");
}

/// ADR-0252: the caret belongs to one window at a time. Keys and IME events reach the
/// shared bridge only through that window; the other window's identical field stays a
/// read-only mirror of the same shared text.
#[test]
fn text_session_routes_keys_and_ime_to_the_owning_window_only() {
    use winit::event::Ime;

    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.inputs");
    install_action_route(&mut app);
    let floating = UiWindowId(0x2000_0001);
    app.native_text_bridges
        .register(floating)
        .expect("registered simulated window");

    app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::Main, None);
    assert_eq!(
        app.text_session_field(STANDALONE_WINDOW),
        Some(TextFieldKind::Main)
    );
    assert_eq!(app.text_session_field(floating), None);

    let theme_before = app.theme;
    assert!(!app.handle_window_key(STANDALONE_WINDOW, &Key::Character("t".into())));
    assert_eq!(
        app.theme, theme_before,
        "text-owned keys cannot run a global action"
    );
    app.text_state
        .preedit(&mut app.text_context, "候选", None)
        .expect("composition");
    for key in [
        Key::Named(NamedKey::Escape),
        Key::Named(NamedKey::Tab),
        Key::Character("t".into()),
    ] {
        assert!(
            !app.handle_window_key(STANDALONE_WINDOW, &key),
            "IME owns its key"
        );
        assert!(app.text_state.is_composing());
        assert!(!app.demo.palette_session.is_open());
        assert_eq!(app.theme, theme_before);
    }
    app.text_state.cancel_compose(&mut app.text_context);

    let before = app.text_state.text().to_owned();
    assert!(
        !app.handle_window_text_event(floating, &WindowEvent::Ime(Ime::Commit("他".to_owned()))),
        "a window without the caret must not consume text events"
    );
    assert_eq!(app.text_state.text(), before);

    assert!(app.handle_window_text_event(STANDALONE_WINDOW, &WindowEvent::Ime(Ime::Enabled)));
    assert!(app.handle_window_text_event(
        STANDALONE_WINDOW,
        &WindowEvent::Ime(Ime::Commit("他".to_owned()))
    ));
    assert!(
        app.text_state.text().contains('他'),
        "the owning window commits into the shared field: {}",
        app.text_state.text()
    );
    assert!(app.handle_window_text_event(STANDALONE_WINDOW, &WindowEvent::Ime(Ime::Disabled)));

    app.clear_text_session();
    assert_eq!(app.text_session_field(STANDALONE_WINDOW), None);
    assert!(
        !app.handle_window_text_event(
            STANDALONE_WINDOW,
            &WindowEvent::Ime(Ime::Commit("文".to_owned()))
        ),
        "without a caret session no window consumes text events"
    );

    app.text_session = Some(TextSession {
        window: floating,
        field: TextFieldKind::Main,
    });
    assert!(app.handle_window_text_event(floating, &WindowEvent::Ime(Ime::Enabled)));
    assert!(app.handle_window_text_event(
        floating,
        &WindowEvent::Ime(Ime::Preedit("候选".into(), Some((0, 6))))
    ));
    let before = app.text_state.text().to_owned();
    app.finish_window_text_session(STANDALONE_WINDOW);
    assert_eq!(app.text_session_field(floating), Some(TextFieldKind::Main));
    assert!(
        app.text_state.is_composing(),
        "foreign blur must preserve the active composition"
    );
    assert_eq!(app.text_state.text(), before);
    assert!(app.handle_window_text_event(floating, &WindowEvent::Ime(Ime::Commit("界".into()))));
    assert!(
        app.text_state.text().contains('界'),
        "foreign blur must not reset the owning bridge"
    );
    app.finish_window_text_session(floating);
    assert!(app.text_session.is_none());

    // Native blur restores a scrub's value before completing the text session.
    // Its last drag draft must not be committed over that restored value.
    app.demo.number = 128;
    app.number_state
        .reset(&mut app.text_context, "400")
        .expect("drag draft");
    app.text_session = Some(TextSession {
        window: floating,
        field: TextFieldKind::Number,
    });
    app.complete_window_text_event(Some(TextSessionExit::WindowBlur(floating)), true, None);
    assert_eq!(app.demo.number, 128);
    assert_eq!(app.number_state.text(), "128");
    assert!(app.text_session.is_none());
    for value in [Some(1.0), None] {
        app.demo.vector[1] = value;
        app.vector_states[1]
            .reset(&mut app.text_context, "9")
            .expect("vector drag draft");
        app.text_session = Some(TextSession {
            window: floating,
            field: TextFieldKind::Vector(1),
        });
        app.complete_window_text_event(Some(TextSessionExit::WindowBlur(floating)), false, Some(1));
        assert_eq!(app.demo.vector[1], value);
        assert_eq!(
            app.vector_states[1].text(),
            if value.is_some() { "1" } else { "—" }
        );
    }
    app.text_session = Some(TextSession {
        window: floating,
        field: TextFieldKind::Number,
    });
    app.number_state
        .replace_accessibility(&mut app.text_context, "-42")
        .expect("ordinary draft");
    app.complete_window_text_event(Some(TextSessionExit::FocusChange), false, None);
    assert_eq!(
        app.demo.number, -42,
        "explicit focus transfer still commits the old field"
    );
    assert!(app.text_session.is_none());

    // Read-only labels share the window session without admitting stale IME
    // edits. Selection remains in the caller-owned source when focus exits.
    app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::Label, None);
    assert_eq!(
        app.text_session_field(STANDALONE_WINDOW),
        Some(TextFieldKind::Label)
    );
    app.readonly_label_state.act(
        &mut app.text_context,
        vesta_gpui::TextInputAction::SelectAll,
    );
    assert_eq!(
        app.readonly_label_state.selected_text(),
        Some(SELECTABLE_LABEL_SOURCE)
    );
    for ime in [
        Ime::Enabled,
        Ime::Preedit("候选".into(), None),
        Ime::Commit("提交".into()),
        Ime::Disabled,
    ] {
        assert!(app.handle_window_text_event(STANDALONE_WINDOW, &WindowEvent::Ime(ime)));
        assert_eq!(app.readonly_label_state.text(), SELECTABLE_LABEL_SOURCE);
        assert!(!app.readonly_label_state.is_composing());
    }
    app.finish_window_text_session(STANDALONE_WINDOW);
    assert_eq!(
        app.readonly_label_state.selected_text(),
        Some(SELECTABLE_LABEL_SOURCE)
    );
}

#[test]
fn changing_text_focus_cancels_the_old_ime_preedit() {
    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.inputs");
    install_action_route(&mut app);
    app.set_text_focus(Some(TextFieldKind::Main));
    app.text_bridge
        .handle_ime::<std::convert::Infallible>(
            &winit::event::Ime::Enabled,
            &mut app.text_state,
            &mut app.text_context,
            false,
        )
        .expect("old field IME starts");
    let original_text = app.text_state.text().to_owned();
    app.text_state
        .preedit(&mut app.text_context, "中文", None)
        .expect("preedit succeeds");
    assert!(app.text_state.is_composing());

    app.traverse_focus(false);
    assert_eq!(app.focused_text_field(), Some(TextFieldKind::Search));
    assert!(!app.text_state.is_composing());
    assert_eq!(app.text_state.text(), original_text);

    let search_before = app.search_state.text().to_owned();
    app.text_bridge
        .handle_ime::<std::convert::Infallible>(
            &winit::event::Ime::Commit("旧字段提交".to_owned()),
            &mut app.search_state,
            &mut app.text_context,
            false,
        )
        .expect("late commit is ignored");
    assert_eq!(app.search_state.text(), search_before);

    app.search_state
        .preedit(&mut app.text_context, "搜索", None)
        .expect("search preedit succeeds");
    app.set_text_focus(None);
    assert!(!app.search_state.is_composing());
    assert!(!app.search_state.text().contains("搜索"));
}

#[test]
fn leaving_and_returning_to_text_field_starts_a_new_undo_group() {
    let mut app = App::new();
    app.text_state
        .reset(&mut app.text_context, "")
        .expect("empty");
    app.set_text_focus(Some(TextFieldKind::Main));
    app.text_state
        .insert(
            &mut app.text_context,
            "first",
            vesta_gpui::TextEditKind::Typing,
        )
        .expect("first edit");
    app.set_text_focus(None);
    app.set_text_focus(Some(TextFieldKind::Main));
    app.text_state
        .insert(
            &mut app.text_context,
            "second",
            vesta_gpui::TextEditKind::Typing,
        )
        .expect("second edit");
    app.text_state
        .act(&mut app.text_context, vesta_gpui::TextInputAction::Undo);
    assert_eq!(app.text_state.text(), "first");
}

#[test]
fn menu_dismissal_restores_only_a_live_opener_focus() {
    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.inputs");
    install_action_route(&mut app);
    app.set_text_focus(Some(TextFieldKind::Main));
    app.open_menu(Point::new(100.0, 100.0));
    assert_eq!(app.focused_text_field(), None);
    let first_item = menu_item_element(app.scopes.menu, MenuItemId(10));
    assert_eq!(
        app.input_ownership.focus().map(|target| target.element),
        Some(first_item)
    );
    install_action_route(&mut app);
    assert!(app.ui_actions.focus_order().contains(&first_item));
    assert!(!app.handle_window_menu_key(UiWindowId(99), &Key::Named(NamedKey::Escape)));
    assert!(app.demo.menu.is_open());
    let theme = app.theme;
    assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Character("t".into())));
    assert_eq!(
        app.theme, theme,
        "an open menu owns otherwise global shortcuts"
    );
    assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::End)));
    assert_eq!(app.demo.menu.highlighted(), Some(MenuItemId(11)));
    assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Home)));
    assert_eq!(app.demo.menu.highlighted(), Some(MenuItemId(10)));
    let export = menu_item_element(app.scopes.menu, MenuItemId(13));
    app.hover_menu_item(Some(export));
    assert_eq!(app.demo.menu.highlighted(), Some(MenuItemId(13)));
    assert_eq!(
        app.input_ownership.focus().map(|target| target.element),
        Some(export)
    );
    assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowRight)));
    assert_eq!(app.demo.menu.current(), Some(MenuId(2)));
    assert_eq!(
        app.demo.menu.highlighted(),
        None,
        "disabled export entries cannot focus"
    );
    let before = app.demo.workspace.clone();
    app.demo.menu.highlight(Some(MenuItemId(20)));
    assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Enter)));
    assert_eq!(app.demo.menu.highlighted(), None);
    assert!(app.demo.menu.is_open());
    assert_eq!(
        app.demo.workspace, before,
        "invalid highlight cannot activate reset"
    );
    assert!(
        !app.prepare_window_menu_action(STANDALONE_WINDOW, DemoAction::MenuCommand(MenuItemId(20)))
    );
    assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowLeft)));
    assert_eq!(app.demo.menu.current(), Some(MenuId(1)));
    app.apply_demo_action(DemoAction::MenuCommand(MenuItemId(1)));
    assert_eq!(app.focused_text_field(), Some(TextFieldKind::Main));

    app.open_menu(Point::new(100.0, 100.0));
    app.close_menu(false);
    assert_eq!(app.focused_text_field(), None);

    app.open_menu(Point::new(100.0, 100.0));
    app.demo.menu.open_for(
        MenuId(1),
        Some(InputTarget {
            window: STANDALONE_WINDOW,
            element: ElementId::new(u64::MAX),
        }),
    );
    app.close_menu(true);
    assert_eq!(app.input_ownership.focus(), None);

    let opener = app.scopes.header.element(APP_MENU);
    app.set_focus_target(Some(opener));
    app.apply_demo_action(DemoAction::ToggleApplicationMenu);
    assert_eq!(app.demo.menu_origin, MenuOrigin::ApplicationBar);
    assert_eq!(app.demo.menu.current(), Some(MenuId(0)));
    let expanded = install_action_route(&mut app);
    assert!(
        expanded
            .find(app.scopes.header.element(HEADER_FILE_MENU))
            .is_some()
    );
    assert!(expanded.find(app.scopes.header.element(25)).is_none());
    for expected in [MenuId(1), MenuId(3), MenuId(4), MenuId(0)] {
        assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowRight)));
        assert_eq!(app.demo.menu.current(), Some(expected));
    }
    assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowLeft)));
    assert_eq!(app.demo.menu.current(), Some(MenuId(4)));
    app.hover_menu_item(Some(app.scopes.header.element(HEADER_FILE_MENU)));
    assert_eq!(app.demo.menu.current(), Some(MenuId(1)));
    app.hover_menu_item(Some(export));
    assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowRight)));
    assert_eq!(app.demo.menu.current(), Some(MenuId(2)));
    assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowLeft)));
    assert_eq!(app.demo.menu.current(), Some(MenuId(1)));
    assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Escape)));
    assert!(!app.demo.menu.is_open());
    assert_eq!(app.demo.menu_origin, MenuOrigin::Context);
    assert_eq!(
        app.input_ownership.focus().map(|focus| focus.element),
        Some(opener)
    );
    let collapsed = install_action_route(&mut app);
    assert!(collapsed.find(app.scopes.header.element(25)).is_some());
    assert!(
        collapsed
            .find(app.scopes.header.element(HEADER_FILE_MENU))
            .is_none()
    );
    app.apply_demo_action(DemoAction::ToggleApplicationMenu);
    app.apply_demo_action(DemoAction::ToggleTheme);
    assert!(
        !app.demo.menu.is_open(),
        "application commands dismiss the menu bar"
    );
    app.set_focus_target(Some(opener));
    app.apply_demo_action(DemoAction::ToggleApplicationMenu);
    install_action_route(&mut app);
    assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Tab)));
    install_action_route(&mut app);
    assert_eq!(
        app.window_focus(STANDALONE_WINDOW),
        Some(app.scopes.header.element(MODE_2D)),
        "menu Tab traverses the collapsed tree, not a disappearing File header"
    );
    let previous = FocusScope::new(STANDALONE_WINDOW, &app.ui_actions, None)
        .order()
        .last()
        .copied();
    app.set_focus_target(Some(opener));
    app.apply_demo_action(DemoAction::ToggleApplicationMenu);
    install_action_route(&mut app);
    app.text_bridge
        .update_modifiers(&winit::event::Modifiers::from(
            winit::keyboard::ModifiersState::SHIFT,
        ));
    assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Tab)));
    install_action_route(&mut app);
    assert_eq!(app.input_ownership.focus(), previous);
    app.text_bridge
        .update_modifiers(&winit::event::Modifiers::from(
            winit::keyboard::ModifiersState::empty(),
        ));
    app.open_menu(Point::new(100.0, 100.0));
    assert_eq!(app.demo.menu_origin, MenuOrigin::Context);
    app.hover_menu_item(Some(app.scopes.header.element(HEADER_WINDOW_MENU)));
    assert_eq!(
        app.demo.menu.current(),
        Some(MenuId(1)),
        "context menus do not use header navigation"
    );
    assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowLeft)));
    assert!(!app.demo.menu.is_open());
}

#[test]
fn frameless_resize_resolves_edges_and_corners_without_swallowing_the_center() {
    let size = Size::new(900.0, 600.0);
    assert_eq!(
        chrome_resize_direction(Point::new(2.0, 2.0), size),
        Some(ResizeDirection::NorthWest)
    );
    assert_eq!(
        chrome_resize_direction(Point::new(898.0, 300.0), size),
        Some(ResizeDirection::East)
    );
    assert_eq!(
        chrome_resize_direction(Point::new(450.0, 599.0), size),
        Some(ResizeDirection::South)
    );
    assert_eq!(chrome_resize_direction(Point::new(450.0, 30.0), size), None);
    scroll_input::scrollbar_thumb_precedes_window_chrome();
}

#[test]
fn accessibility_requests_use_the_same_action_and_text_edit_paths() {
    fn enrich_text_fields(app: &mut App, update: &mut accesskit::TreeUpdate) {
        for (element, state) in [
            (app.scopes.gallery.element(TEXT_FIELD), &mut app.text_state),
            (
                app.scopes.gallery.element(SEARCH_FIELD),
                &mut app.search_state,
            ),
            (app.scopes.gallery.element(TEXT_AREA), &mut app.area_state),
            (
                app.scopes.gallery.element(NUMBER_FIELD),
                &mut app.number_state,
            ),
        ] {
            app.semantics.enrich_text_input(
                update,
                element,
                state,
                &mut app.text_context,
                Point::new(0.0, 0.0),
            );
        }
    }
    #[derive(Default)]
    struct AccessibilityChanges {
        focus: Vec<Option<accesskit::NodeId>>,
    }
    impl accesskit_consumer::TreeChangeHandler for AccessibilityChanges {
        fn node_added(&mut self, _: &accesskit_consumer::Node<'_>) {}
        fn node_updated(
            &mut self,
            _: &accesskit_consumer::Node<'_>,
            _: &accesskit_consumer::Node<'_>,
        ) {
        }
        fn focus_moved(
            &mut self,
            _: Option<&accesskit_consumer::Node<'_>>,
            new_node: Option<&accesskit_consumer::Node<'_>>,
        ) {
            self.focus.push(new_node.map(|node| node.id()));
        }
        fn node_removed(&mut self, _: &accesskit_consumer::Node<'_>) {}
    }
    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.inputs");
    let root = install_action_route_with_viewport(&mut app, Size::new(1280.0, 2400.0));
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 2400.0, 1.0))
        .expect("gallery layout");
    let mut update = app.semantics.build(&root, &layout, None, WINDOW_TITLE);
    enrich_text_fields(&mut app, &mut update);
    let mut consumer = accesskit_consumer::Tree::new(update.clone(), true);
    let mut changes = AccessibilityChanges::default();

    let text = app
        .semantics
        .node_for_element(app.scopes.gallery.element(TEXT_FIELD))
        .expect("text input semantic node");
    app.accessibility_action(accesskit::ActionRequest {
        action: AccessibilityAction::SetValue,
        target: text,
        data: Some(AccessibilityActionData::Value("无障碍输入".into())),
    });
    assert_eq!(app.text_state.text(), "无障碍输入");
    assert_eq!(app.focused_text_field(), Some(TextFieldKind::Main));
    app.text_state
        .act(&mut app.text_context, vesta_gpui::TextInputAction::Undo);
    assert_eq!(app.text_state.text(), "中文 IME / Selection / Undo");
    app.text_state
        .act(&mut app.text_context, vesta_gpui::TextInputAction::Redo);
    assert_eq!(app.text_state.text(), "无障碍输入");
    app.window_accessibility_action(
        UiWindowId(999),
        accesskit::ActionRequest {
            action: AccessibilityAction::SetValue,
            target: text,
            data: Some(AccessibilityActionData::Value("foreign edit".into())),
        },
    );
    assert_eq!(app.text_state.text(), "无障碍输入");

    let password = app
        .semantics
        .node_for_element(app.scopes.gallery.element(PASSWORD_ROOT))
        .expect("password semantic node");
    app.accessibility_action(accesskit::ActionRequest {
        action: AccessibilityAction::SetValue,
        target: password,
        data: Some(AccessibilityActionData::Value("sample-secret".into())),
    });
    assert_eq!(app.password_state.text(), "sample-secret");
    assert_eq!(app.focused_text_field(), Some(TextFieldKind::Password));
    let before = app.password_state.clone();
    app.accessibility_action(accesskit::ActionRequest {
        action: AccessibilityAction::SetValue,
        target: password,
        data: Some(AccessibilityActionData::Value("x".repeat(257).into())),
    });
    assert_eq!(
        app.password_state, before,
        "rejected platform edits preserve selection and history"
    );

    let area = app
        .semantics
        .node_for_element(app.scopes.gallery.element(TEXT_AREA))
        .expect("multiline input semantic node");
    assert_eq!(
        update
            .nodes
            .iter()
            .find(|(id, _)| *id == area)
            .expect("multiline node in update")
            .1
            .role(),
        accesskit::Role::MultilineTextInput
    );
    app.accessibility_action(accesskit::ActionRequest {
        action: AccessibilityAction::SetValue,
        target: area,
        data: Some(AccessibilityActionData::Value("地形\n道路".into())),
    });
    assert_eq!(app.area_state.text(), "地形\n道路");
    assert_eq!(app.focused_text_field(), Some(TextFieldKind::Area));
    let mut focused_update = app.semantics.build(
        &root,
        &layout,
        app.input_ownership.focus().map(|focus| focus.element),
        WINDOW_TITLE,
    );
    enrich_text_fields(&mut app, &mut focused_update);
    assert_eq!(focused_update.focus, area);
    let selection = focused_update
        .nodes
        .iter()
        .find(|(id, _)| *id == area)
        .expect("area projection")
        .1
        .text_selection()
        .cloned()
        .expect("area selection");
    consumer.update_and_process_changes(focused_update, &mut changes);
    assert_eq!(consumer.state().focus_id(), Some(area));
    assert_eq!(changes.focus.pop(), Some(Some(area)));
    let search = app
        .semantics
        .node_for_element(app.scopes.gallery.element(SEARCH_FIELD))
        .expect("search semantic node");
    app.set_text_focus(Some(TextFieldKind::Search));
    let mut focused_update = app.semantics.build(
        &root,
        &layout,
        app.input_ownership.focus().map(|focus| focus.element),
        WINDOW_TITLE,
    );
    enrich_text_fields(&mut app, &mut focused_update);
    assert_eq!(focused_update.focus, search);
    consumer.update_and_process_changes(focused_update, &mut changes);
    assert_eq!(consumer.state().focus_id(), Some(search));
    assert_eq!(changes.focus.pop(), Some(Some(search)));
    app.accessibility_action(accesskit::ActionRequest {
        action: AccessibilityAction::SetTextSelection,
        target: area,
        data: Some(AccessibilityActionData::SetTextSelection(selection)),
    });
    assert_eq!(
        app.focused_text_field(),
        Some(TextFieldKind::Area),
        "an unchanged valid range still acquires the source field's session"
    );
    app.set_text_focus(Some(TextFieldKind::Search));
    let invalid = accesskit::TextPosition {
        node: accesskit::NodeId(u64::MAX),
        character_index: 0,
    };
    app.accessibility_action(accesskit::ActionRequest {
        action: AccessibilityAction::SetTextSelection,
        target: area,
        data: Some(AccessibilityActionData::SetTextSelection(
            accesskit::TextSelection {
                anchor: invalid,
                focus: invalid,
            },
        )),
    });
    assert_eq!(
        app.focused_text_field(),
        Some(TextFieldKind::Search),
        "invalid run identities do not take focus"
    );

    let number = app
        .semantics
        .node_for_element(app.scopes.gallery.element(NUMBER_FIELD))
        .expect("number semantic node");
    let number_run = update
        .nodes
        .iter()
        .find(|(id, _)| *id == number)
        .expect("number projection")
        .1
        .children()[0];
    app.accessibility_action(accesskit::ActionRequest {
        action: AccessibilityAction::SetTextSelection,
        target: number,
        data: Some(AccessibilityActionData::SetTextSelection(
            accesskit::TextSelection {
                anchor: accesskit::TextPosition {
                    node: number_run,
                    character_index: 0,
                },
                focus: accesskit::TextPosition {
                    node: number_run,
                    character_index: 2,
                },
            },
        )),
    });
    assert_eq!(app.focused_text_field(), Some(TextFieldKind::Number));
    assert_eq!(
        app.number_state.selected_text(),
        Some("12"),
        "entering the scalar session must not erase the platform range"
    );

    activate_gallery_page(&mut app, "gallery.controls");
    let slider_root = install_action_route_with_viewport(&mut app, Size::new(1280.0, 2400.0));
    let slider_layout = LayoutEngine
        .compute(&slider_root, Viewport::new(1280.0, 2400.0, 1.0))
        .expect("controls layout");
    let slider_update = app
        .semantics
        .build(&slider_root, &slider_layout, None, WINDOW_TITLE);
    let slider_element = semantic_id(&slider_root, SemanticRole::Slider).expect("slider element");
    let slider_node = app
        .semantics
        .node_for_element(slider_element)
        .expect("slider semantic node");
    let slider_semantic = &slider_update
        .nodes
        .iter()
        .find(|(id, _)| *id == slider_node)
        .expect("slider node in update")
        .1;
    assert!(slider_semantic.supports_action(AccessibilityAction::SetValue));
    assert_eq!(slider_semantic.min_numeric_value(), Some(0.0));
    assert_eq!(slider_semantic.max_numeric_value(), Some(1.0));
    app.accessibility_action(accesskit::ActionRequest {
        action: AccessibilityAction::SetValue,
        target: slider_node,
        data: Some(AccessibilityActionData::NumericValue(0.75)),
    });
    assert_eq!(app.demo.slider, 0.75);

    let mode = app
        .semantics
        .node_for_element(app.scopes.header.element(MODE_3D))
        .expect("3D mode semantic node");
    app.accessibility_action(accesskit::ActionRequest {
        action: AccessibilityAction::Click,
        target: mode,
        data: None,
    });
    assert_eq!(app.mode, AcceptanceMode::Viewport3d);

    // An action queued by the platform before a rebuild must not edit a
    // field that the current interactive tree has removed or disabled.
    app.ui_actions = ActionRoute::default();
    app.accessibility_action(accesskit::ActionRequest {
        action: AccessibilityAction::SetValue,
        target: text,
        data: Some(AccessibilityActionData::Value("stale edit".into())),
    });
    assert_eq!(app.text_state.text(), "无障碍输入");
}

#[test]
fn tab_drag_targets_reorder_split_and_float_through_workspace_actions() {
    fn assert_rect_near(actual: Rect, expected: Rect) {
        assert!((actual.x - expected.x).abs() < 0.01);
        assert!((actual.y - expected.y).abs() < 0.01);
        assert!((actual.width - expected.width).abs() < 0.01);
        assert!((actual.height - expected.height).abs() < 0.01);
    }

    let model = demo_workspace();
    let pane = model.panes().first().expect("demo pane").id;
    let frame = DockFrame {
        panes: vec![DockPaneFrame {
            flow_direction: vesta_gpui::FlowDirection::LeftToRight,
            pane,
            container: ContainerRef::Main,
            root: Rect::new(0.0, 0.0, 400.0, 300.0),
            tab_strip: Rect::new(0.0, 0.0, 400.0, 32.0),
            body: Rect::new(0.0, 32.0, 400.0, 268.0),
        }],
        tabs: vec![
            DockTabFrame {
                pane,
                rect: Rect::new(0.0, 0.0, 100.0, 32.0),
            },
            DockTabFrame {
                pane,
                rect: Rect::new(100.0, 0.0, 100.0, 32.0),
            },
        ],
        splits: vec![
            DockSplitFrame {
                flow_direction: vesta_gpui::FlowDirection::LeftToRight,
                container: ContainerRef::Main,
                split: SplitId(1),
                axis: SplitAxis::Row,
                rect: Rect::new(0.0, 0.0, 400.0, 300.0),
            },
            DockSplitFrame {
                flow_direction: vesta_gpui::FlowDirection::LeftToRight,
                container: ContainerRef::Main,
                split: SplitId(2),
                axis: SplitAxis::Column,
                rect: Rect::new(0.0, 0.0, 200.0, 300.0),
            },
        ],
        workspace: Some(Rect::new(0.0, 0.0, 400.0, 300.0)),
    };
    assert_eq!(
        frame.split_rect(ContainerRef::Main, SplitId(2), SplitAxis::Column),
        Some(Rect::new(0.0, 0.0, 200.0, 300.0)),
    );
    let tab_insert = frame
        .target(Point::new(240.0, 18.0), &model)
        .expect("tab insertion target");
    assert_eq!(tab_insert.action, DockAction::Merge { pane, slot: 2 });
    assert_eq!(tab_insert.preview, frame.panes[0].tab_strip.inset(1.0));
    assert!(tab_insert.insertion.is_some());
    for region in dock_tab_split_regions(frame.tabs[1].rect) {
        let point = Point::new(
            region.rect.x + region.rect.width * 0.5,
            region.rect.y + region.rect.height * 0.5,
        );
        let target = frame.target(point, &model).expect("tab split target");
        assert_eq!(
            target.action,
            DockAction::Split {
                pane,
                side: region.side,
            },
        );
        assert_eq!(
            target.preview,
            dock_edge_preview(frame.panes[0].root, region.side, 0.42)
        );
    }
    assert_eq!(
        frame
            .target(Point::new(40.0, 150.0), &model)
            .map(|target| target.action),
        Some(DockAction::Split {
            pane,
            side: Side::Left
        }),
    );
    for (side, point) in [
        (Side::Top, Point::new(250.0, 55.0)),
        (Side::Right, Point::new(360.0, 150.0)),
        (Side::Bottom, Point::new(250.0, 275.0)),
    ] {
        let target = frame.target(point, &model).expect("pane edge target");
        assert_eq!(target.action, DockAction::Split { pane, side });
        assert_eq!(
            target.preview,
            dock_edge_preview(frame.panes[0].root, side, 0.42),
        );
    }
    let center = frame
        .target(Point::new(200.0, 150.0), &model)
        .expect("center target");
    assert_eq!(
        center.action,
        DockAction::Merge {
            pane,
            slot: model.pane(pane).expect("live destination").items.len()
        }
    );
    assert_eq!(center.preview, frame.panes[0].root.inset(5.0));
    let window_edge = frame
        .target(Point::new(8.0, 150.0), &model)
        .expect("window edge target");
    assert_eq!(
        window_edge.action,
        DockAction::WindowSplit {
            container: ContainerRef::Main,
            side: Side::Left,
        },
    );
    assert_eq!(
        window_edge.preview,
        dock_edge_preview(frame.workspace.expect("workspace"), Side::Left, 0.25),
    );
    for (side, point) in [
        (Side::Top, Point::new(250.0, 5.0)),
        (Side::Right, Point::new(395.0, 150.0)),
        (Side::Bottom, Point::new(250.0, 295.0)),
    ] {
        let target = frame.target(point, &model).expect("window edge target");
        assert_eq!(
            target.action,
            DockAction::WindowSplit {
                container: ContainerRef::Main,
                side,
            },
        );
        assert_eq!(
            target.preview,
            dock_edge_preview(frame.workspace.expect("workspace"), side, 0.25),
        );
    }
    let floating = frame
        .target(Point::new(450.0, 450.0), &model)
        .expect("floating target");
    let DockAction::Float { placement } = floating.action else {
        panic!("dragging outside the workspace should float the tab");
    };
    assert!(placement.x >= 0.0 && placement.y >= 0.0);
    assert!(placement.x + placement.width <= 400.0);
    assert!(placement.y + placement.height <= 300.0);
    assert_eq!(floating.preview.x, placement.x);
    assert_eq!(floating.preview.y, placement.y);

    let guide_target = frame
        .target(Point::new(140.0, 8.0), &model)
        .expect("directional target");
    let mut app = App::new();
    let item = app.demo.workspace.panes()[0].items[0].id;
    app.cursor = Point::new(140.0, 8.0);
    app.workspace_drag = Some(WorkspaceTabDrag {
        item,
        origin_container: ContainerRef::Main,
        origin: Point::new(120.0, 16.0),
        started: true,
        pointer_container: Some(ContainerRef::Main),
        cursor: Point::new(140.0, 8.0),
        target: Some(guide_target),
    });
    assert_eq!(app.paint_interaction().pointer, None);
    let root = install_action_route(&mut app);
    let overlay = root
        .children
        .iter()
        .find(|child| child.id == app.scopes.root.element(100))
        .expect("dock overlay belongs to the application root");
    assert!(!overlay.style.pointer_events);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("dock overlay layout");
    let paint = PaintList::build(&root, &layout);
    assert!(
        paint
            .commands()
            .iter()
            .any(|command| command.element_id() == Some(app.scopes.root.element(101)))
    );
    let page_id = page_scroll_id(app.scopes.gallery, item);
    let page = layout.node(page_id).expect("active dock page scrollbox");
    app.cursor = Point::new(page.rect.x + page.rect.width * 0.5, page.rect.y + 20.0);
    let painted_scrollbox_commands = |interaction| {
        PaintList::build_with_focus_and_interaction(
            &root,
            &layout,
            None,
            app.theme.palette.focus,
            interaction,
        )
        .commands()
        .iter()
        .filter(|command| {
            matches!(command, vesta_gpui::PaintCommand::FillRoundedRect { element, .. } if *element == page_id)
        })
        .count()
    };
    let dock_drag_count = painted_scrollbox_commands(app.paint_interaction());
    let hover_count = painted_scrollbox_commands(PaintInteraction {
        pointer: Some(app.cursor),
        active_scrollbar: None,
    });
    assert!(
        hover_count > dock_drag_count,
        "captured dock pointer must not reveal the adjacent page scrollbar"
    );
    let foreign_cursor = Point::new(-100.0, -100.0);
    assert_eq!(
        painted_scrollbox_commands(app.window_paint_interaction(foreign_cursor, None)),
        dock_drag_count,
        "another window's cursor must not reveal this scrollbar"
    );
    assert!(
        painted_scrollbox_commands(app.window_paint_interaction(foreign_cursor, Some(page_id)))
            > dock_drag_count,
        "the owning window's captured scrollbar remains visible outside its viewport"
    );
    assert_rect_near(
        layout
            .node(app.scopes.root.element(101))
            .expect("visible preview")
            .rect,
        guide_target.preview,
    );
    for index in 0..4 {
        assert!(
            !paint.commands().iter().any(|command| {
                command.element_id() == Some(app.scopes.root.element(103 + index))
            }),
            "directional marker should not cover the tab during a drag"
        );
    }
    app.cursor = Point::new(240.0, 18.0);
    app.workspace_drag.as_mut().expect("active drag").target = Some(tab_insert);
    let root = install_action_route(&mut app);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("tab insertion overlay layout");
    assert_rect_near(
        layout
            .node(app.scopes.root.element(101))
            .expect("tab strip preview")
            .rect,
        tab_insert.preview,
    );
    assert_rect_near(
        layout
            .node(app.scopes.root.element(107))
            .expect("tab insertion marker")
            .rect,
        tab_insert.insertion.expect("insertion rect"),
    );
    app.workspace_drag = None;
    assert_eq!(app.paint_interaction().pointer, Some(app.cursor));
}

#[test]
fn workspace_drag_crosses_native_containers_and_commits_the_stable_target() {
    let mut app = App::new();
    let item = app
        .demo
        .workspace
        .pane(PaneId(1))
        .expect("source pane")
        .items[0]
        .id;
    let floating_item = app
        .demo
        .workspace
        .pane(PaneId(2))
        .expect("target source pane")
        .items[0]
        .id;
    app.demo
        .workspace
        .dock_item(
            floating_item,
            DockAction::Float {
                placement: WindowPlacement::new(40.0, 40.0, 360.0, 240.0),
            },
        )
        .expect("target floating window");
    let floating = app.demo.workspace.floating_windows()[0].id;
    let target_pane = app.demo.workspace.floating_windows()[0].root.panes()[0];
    let main_geometry = DockWindowGeometry {
        container: ContainerRef::Main,
        outer: Rect::new(-800.0, 80.0, 1600.0, 1000.0),
        client: Rect::new(-800.0, 80.0, 1600.0, 1000.0),
        scale: 1.25,
    };
    let floating_geometry = DockWindowGeometry {
        container: ContainerRef::Floating(floating),
        outer: Rect::new(-620.0, 130.0, 500.0, 540.0),
        client: Rect::new(-600.0, 170.0, 480.0, 480.0),
        scale: 2.0,
    };
    let frame_for = |pane, container, width, height| DockFrame {
        panes: vec![DockPaneFrame {
            flow_direction: vesta_gpui::FlowDirection::LeftToRight,
            pane,
            container,
            root: Rect::new(0.0, 0.0, width, height),
            tab_strip: Rect::new(0.0, 0.0, width, 32.0),
            body: Rect::new(0.0, 32.0, width, height - 32.0),
        }],
        workspace: Some(Rect::new(0.0, 0.0, width, height)),
        ..DockFrame::default()
    };
    let main_frame = frame_for(PaneId(1), ContainerRef::Main, 1280.0, 800.0);
    let floating_frame = frame_for(target_pane, ContainerRef::Floating(floating), 240.0, 240.0);
    let windows = [
        (floating_geometry, &floating_frame),
        (main_geometry, &main_frame),
    ];
    let (container, cursor, target) = route_dock_pointer(
        main_geometry,
        Point::new(352.0, 256.0),
        &windows,
        &app.demo.workspace,
    )
    .expect("captured main event resolves floating window on a negative desktop");
    assert_eq!(container, ContainerRef::Floating(floating));
    assert_eq!(cursor, Point::new(120.0, 115.0));
    let target = target.expect("floating Dock body");
    assert_eq!(
        target.action,
        DockAction::Merge {
            pane: target_pane,
            slot: 1
        }
    );
    assert!(
        route_dock_pointer(
            main_geometry,
            Point::new(352.0, 56.0),
            &windows,
            &app.demo.workspace,
        )
        .is_none(),
        "floating title bar must occlude main Dock targets"
    );
    let (_, _, edge) = route_dock_pointer(
        main_geometry,
        Point::new(163.2, 232.0),
        &windows,
        &app.demo.workspace,
    )
    .expect("floating edge route");
    assert_eq!(
        edge.expect("floating edge").action,
        DockAction::WindowSplit {
            container: ContainerRef::Floating(floating),
            side: Side::Left,
        }
    );
    app.workspace_drag = Some(WorkspaceTabDrag {
        item,
        origin_container: ContainerRef::Main,
        origin: Point::new(120.0, 16.0),
        started: false,
        pointer_container: Some(ContainerRef::Main),
        cursor: Point::new(120.0, 16.0),
        target: None,
    });

    assert!(app.update_workspace_drag(
        ContainerRef::Floating(floating),
        Point::new(16.0, 18.0),
        Some(target),
    ));
    assert!(app.dock_drag_visual(ContainerRef::Main).is_none());
    assert_eq!(
        app.dock_drag_visual(ContainerRef::Floating(floating))
            .expect("target window preview")
            .target
            .expect("resolved dock target")
            .action,
        target.action,
    );

    // This geometry fixture has no OS display; use explicit admission.
    app.finish_workspace_drag_with_admission(ContainerRef::Main, |_| true);
    assert!(
        app.demo
            .workspace
            .pane(target_pane)
            .expect("target pane survives")
            .items
            .iter()
            .any(|candidate| candidate.id == item)
    );
    assert_eq!(
        app.demo.workspace.container_of_pane(target_pane),
        Some(ContainerRef::Floating(floating))
    );

    let return_pane = PaneId(1);
    let (return_container, return_cursor, return_target) = route_dock_pointer(
        floating_geometry,
        Point::new(275.0, 192.5),
        &windows,
        &app.demo.workspace,
    )
    .expect("captured floating event resolves main client");
    assert_eq!(return_container, ContainerRef::Main);
    assert_eq!(return_cursor, Point::new(600.0, 380.0));
    let return_target = return_target.expect("main Dock body");
    assert_eq!(
        return_target.action,
        DockAction::Merge {
            pane: return_pane,
            slot: app
                .demo
                .workspace
                .pane(return_pane)
                .expect("live destination")
                .items
                .len()
        }
    );
    app.workspace_drag = Some(WorkspaceTabDrag {
        item,
        origin_container: ContainerRef::Floating(floating),
        origin: Point::new(80.0, 16.0),
        started: false,
        pointer_container: Some(ContainerRef::Floating(floating)),
        cursor: Point::new(80.0, 16.0),
        target: None,
    });
    assert!(app.update_workspace_drag(
        ContainerRef::Main,
        Point::new(240.0, 120.0),
        Some(return_target),
    ));
    assert!(
        app.dock_drag_visual(ContainerRef::Floating(floating))
            .is_none()
    );
    assert!(app.dock_drag_visual(ContainerRef::Main).is_some());
    app.finish_workspace_drag_with_admission(ContainerRef::Floating(floating), |_| true);
    assert!(
        app.demo
            .workspace
            .pane(return_pane)
            .expect("main target pane survives")
            .items
            .iter()
            .any(|candidate| candidate.id == item)
    );
    assert_eq!(
        app.demo.workspace.container_of_pane(return_pane),
        Some(ContainerRef::Main)
    );
}

#[test]
fn nested_menu_stays_inside_a_small_window_and_flips_left() {
    let mut app = App::new();
    let mut demo = DemoState {
        menu_anchor: Point::new(880.0, 570.0),
        ..DemoState::default()
    };
    demo.menu.open(MenuId(1));
    demo.menu.enter_submenu(MenuId(2));
    let mut actions = Vec::new();
    let popup = build_menu_popup(
        app.scopes.menu,
        app.theme,
        &demo,
        None,
        Size::new(900.0, 600.0),
        &mut actions,
        &mut app.text_context,
    );
    let menus = popup
        .children
        .iter()
        .filter(|child| child.key.as_str().contains("/menu.popup"))
        .collect::<Vec<_>>();
    assert_eq!(menus.len(), 2);
    assert_eq!(
        menus[0].semantic.as_ref().map(|semantic| semantic.role),
        Some(SemanticRole::Menu)
    );
    let root_x = menus[0].style.left.expect("root popup x");
    let child_x = menus[1].style.left.expect("submenu x");
    assert!(child_x < root_x, "submenu should flip to the left edge");
    for menu in menus {
        let x = menu.style.left.expect("popup x");
        let y = menu.style.top.expect("popup y");
        let Dimension::Px(width) = menu.style.width else {
            panic!("measured popup width")
        };
        assert!(x >= 8.0 && x + width <= 892.0);
        assert!(y >= CHROME_HEIGHT + 6.0 && y + 12.0 <= 592.0 - CHROME_STATUS_HEIGHT);
    }
    let route = ActionRoute::build(&popup, actions).expect("menu actions");
    assert!(
        route
            .focus_order()
            .contains(&menu_item_element(app.scopes.menu, MenuItemId(13)))
    );
    assert!(
        !route
            .focus_order()
            .contains(&menu_item_element(app.scopes.menu, MenuItemId(20)))
    );
    demo.menu.open(MenuId(3));
    for theme in [WidgetTheme::dark(), WidgetTheme::light()] {
        let mut actions = Vec::new();
        let popup = build_menu_popup(
            app.scopes.menu,
            theme,
            &demo,
            None,
            Size::new(900.0, 600.0),
            &mut actions,
            &mut app.text_context,
        );
        let entry = popup
            .find(menu_item_element(app.scopes.menu, MenuItemId(32)))
            .expect("theme entry");
        assert_eq!(
            entry.semantic.as_ref(),
            Some(
                &Semantic::new(SemanticRole::MenuItemCheckBox, "Dark theme")
                    .checked(theme.mode == ThemeMode::Dark)
            )
        );
        let route = ActionRoute::build(&popup, actions).expect("theme menu route");
        assert_eq!(
            route.action_at(entry.id),
            Some(&DemoAction::MenuCommand(MenuItemId(32)))
        );
    }
}

#[test]
fn translation_preview_cancels_or_commits_through_typed_actions() {
    let mut app = App::new();
    let selected = &app.scene.renderables[0];
    let original = selected.translation;
    let start = vesta_viewport::project_to_pixel(app.camera, 1280.0, 800.0, original)
        .expect("selected center projects");
    let end = vesta_viewport::project_to_pixel(
        app.camera,
        1280.0,
        800.0,
        original.plus(vesta_viewport::Vec3::new(1.3, 0.0, 0.0)),
    )
    .expect("axis projects");
    let cursor = [start[0] * 0.4 + end[0] * 0.6, start[1] * 0.4 + end[1] * 0.6];
    let drag = TranslationDrag::begin(&app.scene, app.camera, 1280.0, 800.0, cursor[0], cursor[1])
        .expect("translation handle");
    let advanced = [
        cursor[0] + (end[0] - start[0]) * 0.25,
        cursor[1] + (end[1] - start[1]) * 0.25,
    ];
    app.transform_drag = Some(drag);
    app.apply_transform_action(drag, drag.begin_action());
    app.apply_transform_action(drag, drag.update(advanced[0], advanced[1]));
    assert!(app.scene.renderables[0].translation.x > original.x);
    app.cancel_transform_drag();
    assert_eq!(app.scene.renderables[0].translation, original);
    assert_eq!(app.scene.gizmo.active, None);

    app.transform_drag = Some(drag);
    app.apply_transform_action(drag, drag.begin_action());
    app.apply_transform_action(drag, drag.update(advanced[0], advanced[1]));
    app.transform_drag = None;
    app.apply_transform_action(drag, TransformAction::Commit);
    assert!(app.scene.renderables[0].translation.x > original.x);
    assert_eq!(app.scene.gizmo.active, None);
}
