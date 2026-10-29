use super::*;

struct Frame {
    view: vesta_gpui::TextInputViewport,
    cell: Rect,
    editor: Rect,
    stable: bool,
}

fn frame(app: &mut App, item: ItemId, size: Size, flow: vesta_gpui::FlowDirection) -> Frame {
    let mut root = install_action_route_with_viewport(app, size);
    root.style.flow_direction = Some(flow);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(size.width, size.height, 1.0))
        .expect("current narrow Table layout");
    let page_changed = app.demo.page_scroll.entry(item).or_default().sync_layout(
        layout
            .node(page_scroll_id(app.scopes.gallery, item))
            .expect("Table page"),
    );
    app.scroll_geometry =
        ScrollGeometry::from_layout(app.scopes.gallery, &layout, [item].into_iter());
    app.focus_rects = app
        .ui_actions
        .focus_order()
        .iter()
        .filter_map(|id| layout.node(*id).map(|node| (*id, node.rect)))
        .collect();
    app.hit_test = HitTestTree::build(&root, &layout);
    app.table_rect = Some(
        layout
            .node(app.scopes.gallery.element(TABLE_ROOT))
            .expect("Table")
            .rect,
    );
    let editor = layout
        .node(TextFieldKind::TableNote.element(app.scopes.gallery))
        .expect("stable note editor")
        .rect;
    assert_eq!(
        root.find(app.scopes.gallery.element(TABLE_ROOT + 111))
            .expect("cell identity")
            .semantic
            .as_ref(),
        Some(
            &Semantic::new(SemanticRole::Cell, "Cube")
                .selected(app.demo.table_selected_cell == Some((1, 0)))
        ),
        "compact editor preserves its object cell semantics"
    );
    let cell = layout
        .node(app.scopes.gallery.element(TABLE_ROOT + 111))
        .expect("stable first cell")
        .rect;
    let paint = PaintList::build(&root, &layout);
    let focus_root = app.focus_root_for_window(STANDALONE_WINDOW);
    let revealed = process_caret_probe(app, &paint, STANDALONE_WINDOW, !page_changed, focus_root);
    let view = paint
        .commands()
        .iter()
        .find_map(|command| match command {
            vesta_gpui::PaintCommand::TextInput {
                element,
                bounds,
                clip,
                snapshot,
                ..
            } if *element == TextFieldKind::TableNote.element(app.scopes.gallery) => {
                Some(snapshot.viewport(*bounds, *clip, Point::default()))
            }
            _ => None,
        })
        .expect("current note paint/input geometry");
    let stable = !page_changed && !revealed;
    if stable {
        app.table_note_view = text_views_for(
            STANDALONE_WINDOW,
            app.scopes.gallery,
            &[(TextFieldKind::TableNote.element(app.scopes.gallery), view)],
        )
        .expect("source-qualified Table input view")
        .get(TextFieldKind::TableNote);
    }
    Frame {
        view,
        cell,
        editor,
        stable,
    }
}

pub(super) fn current_table_editor_remains_usable() {
    let mut failures = Vec::new();
    for size in [Size::new(1280.0, 800.0), Size::new(360.0, 800.0)] {
        for constrained in [false, true] {
            for flow in [
                vesta_gpui::FlowDirection::LeftToRight,
                vesta_gpui::FlowDirection::RightToLeft,
            ] {
                let mut app = App::new();
                let font = include_bytes!(
                    "../../../../../crates/vesta-gpui/tests/fixtures/MaterialIcons-Regular.ttf"
                );
                assert!(
                    app.text_context
                        .register_font(std::sync::Arc::new(font.to_vec()))
                );
                let item = activate_gallery_page(&mut app, "gallery.collections");
                app.apply_demo_action(DemoAction::SelectTable(2));
                frame(&mut app, item, size, flow);
                if constrained {
                    app.set_focus_target(Some(table_resize_element(app.scopes.gallery, 0)));
                    assert!(
                        app.handle_scalar_control_key(
                            STANDALONE_WINDOW,
                            &Key::Named(NamedKey::Home)
                        )
                    );
                    frame(&mut app, item, size, flow);
                }
                app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::TableNote, None);
                app.table_note_state.act(
                    &mut app.text_context,
                    vesta_gpui::TextInputAction::TextEnd { extend: false },
                );
                let owner = TextFieldKind::TableNote.element(app.scopes.gallery);
                app.reveal_window_focus(STANDALONE_WINDOW, owner);
                frame(&mut app, item, size, flow);
                assert!(app.handle_window_text_event(
                    STANDALONE_WINDOW,
                    &WindowEvent::Ime(winit::event::Ime::Enabled)
                ));
                assert!(app.handle_window_text_event(
                    STANDALONE_WINDOW,
                    &WindowEvent::Ime(winit::event::Ime::Commit(
                        " / editable note 0123456789 abcdefghijklmnopqrstuvwxyz".into()
                    ))
                ));
                let draft = app.table_note_state.text().to_owned();
                inspect_caret(
                    &mut app,
                    item,
                    size,
                    flow,
                    constrained,
                    "End",
                    &mut failures,
                );
                assert!(app.handle_window_text_event(
                    STANDALONE_WINDOW,
                    &WindowEvent::KeyboardInput {
                        device_id: winit::event::DeviceId::dummy(),
                        event: vesta_gpui_winit::UiKeyEvent {
                            physical_key: winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Home
                            ),
                            logical_key: Key::Named(NamedKey::Home),
                            text: None,
                            location: winit::keyboard::KeyLocation::Standard,
                            state: ElementState::Pressed,
                            repeat: false,
                        },
                        is_synthetic: false,
                    }
                ));
                inspect_caret(
                    &mut app,
                    item,
                    size,
                    flow,
                    constrained,
                    "Home",
                    &mut failures,
                );
                assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(owner));
                assert_eq!(
                    app.demo.table_note, "Ready",
                    "reveal cannot publish a cell draft"
                );
                assert_eq!(
                    app.demo.table_selected, 2,
                    "editing must not select another row"
                );
                assert_eq!(app.ui_actions.action_for_hit(owner), None);
                assert!(
                    app.table_note_state
                        .act(&mut app.text_context, vesta_gpui::TextInputAction::Undo)
                );
                assert_eq!(app.table_note_state.text(), "Ready");
                assert!(
                    app.table_note_state
                        .act(&mut app.text_context, vesta_gpui::TextInputAction::Redo)
                );
                assert_eq!(app.table_note_state.text(), draft);
                assert!(app.handle_window_text_event(
                    STANDALONE_WINDOW,
                    &WindowEvent::KeyboardInput {
                        device_id: winit::event::DeviceId::dummy(),
                        event: vesta_gpui_winit::UiKeyEvent {
                            physical_key: winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Escape
                            ),
                            logical_key: Key::Named(NamedKey::Escape),
                            text: None,
                            location: winit::keyboard::KeyLocation::Standard,
                            state: ElementState::Pressed,
                            repeat: false,
                        },
                        is_synthetic: false,
                    }
                ));
                assert_eq!(app.demo.table_note, "Ready");
                assert_eq!(app.table_note_state.text(), "Ready");
            }
        }
    }
    live_width_changes_keep_the_edit_session(&mut failures);
    assert!(
        failures.is_empty(),
        "current compound note caret clipped inside legal Table tracks: {failures:?}"
    );
}

fn live_width_changes_keep_the_edit_session(failures: &mut Vec<String>) {
    for flow in [
        vesta_gpui::FlowDirection::LeftToRight,
        vesta_gpui::FlowDirection::RightToLeft,
    ] {
        let mut app = App::new();
        let font = include_bytes!(
            "../../../../../crates/vesta-gpui/tests/fixtures/MaterialIcons-Regular.ttf"
        );
        assert!(
            app.text_context
                .register_font(std::sync::Arc::new(font.to_vec()))
        );
        let item = activate_gallery_page(&mut app, "gallery.collections");
        app.apply_demo_action(DemoAction::SelectTable(2));
        frame(&mut app, item, Size::new(1280.0, 800.0), flow);
        app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::TableNote, None);
        app.table_note_state.act(
            &mut app.text_context,
            vesta_gpui::TextInputAction::TextEnd { extend: false },
        );
        assert!(app.handle_window_text_event(
            STANDALONE_WINDOW,
            &WindowEvent::Ime(winit::event::Ime::Enabled)
        ));
        assert!(app.handle_window_text_event(
            STANDALONE_WINDOW,
            &WindowEvent::Ime(winit::event::Ime::Commit(
                " / stable through width changes".into()
            ))
        ));
        let draft = app.table_note_state.text().to_owned();
        let selection = app.table_note_state.inspection_selection();
        let fractions = app.demo.table_column_fractions;
        let selected_cell = app.demo.table_selected_cell;
        let owner = TextFieldKind::TableNote.element(app.scopes.gallery);
        for (stage, size) in [
            Size::new(1280.0, 800.0),
            Size::new(360.0, 220.0),
            Size::new(1280.0, 800.0),
        ]
        .into_iter()
        .enumerate()
        {
            frame(&mut app, item, size, flow);
            for (key, code) in [
                (NamedKey::Home, winit::keyboard::KeyCode::Home),
                (NamedKey::End, winit::keyboard::KeyCode::End),
            ] {
                assert!(app.handle_window_text_event(
                    STANDALONE_WINDOW,
                    &WindowEvent::KeyboardInput {
                        device_id: winit::event::DeviceId::dummy(),
                        event: vesta_gpui_winit::UiKeyEvent {
                            physical_key: winit::keyboard::PhysicalKey::Code(code),
                            logical_key: Key::Named(key),
                            text: None,
                            location: winit::keyboard::KeyLocation::Standard,
                            state: ElementState::Pressed,
                            repeat: false,
                        },
                        is_synthetic: false,
                    }
                ));
            }
            inspect_caret(
                &mut app,
                item,
                size,
                flow,
                false,
                &format!("Live{stage}"),
                failures,
            );
            assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(owner));
            assert_eq!(
                app.text_session_field(STANDALONE_WINDOW),
                Some(TextFieldKind::TableNote)
            );
            assert_eq!(app.table_note_state.text(), draft);
            assert_eq!(app.table_note_state.inspection_selection(), selection);
            assert_eq!(app.demo.table_note, "Ready");
            assert_eq!(app.demo.table_selected, 2);
            assert_eq!(app.demo.table_selected_cell, selected_cell);
            assert_eq!(app.demo.table_column_fractions, fractions);
            println!(
                "GPV05-NARROW-LIVE flow={flow:?} stage={stage} width={} height={} focus=true selection=true draft=true model_held=true proportions_held=true",
                size.width, size.height
            );
        }
        assert!(
            app.table_note_state
                .act(&mut app.text_context, vesta_gpui::TextInputAction::Undo)
        );
        assert_eq!(
            app.table_note_state.text(),
            "Ready",
            "projection changes add no editor history"
        );
        assert!(
            app.table_note_state
                .act(&mut app.text_context, vesta_gpui::TextInputAction::Redo)
        );
        assert_eq!(app.table_note_state.text(), draft);
        assert!(app.handle_window_text_event(
            STANDALONE_WINDOW,
            &WindowEvent::KeyboardInput {
                device_id: winit::event::DeviceId::dummy(),
                event: vesta_gpui_winit::UiKeyEvent {
                    physical_key: winit::keyboard::PhysicalKey::Code(
                        winit::keyboard::KeyCode::Enter
                    ),
                    logical_key: Key::Named(NamedKey::Enter),
                    text: None,
                    location: winit::keyboard::KeyLocation::Standard,
                    state: ElementState::Pressed,
                    repeat: false,
                },
                is_synthetic: false,
            }
        ));
        assert_eq!(app.demo.table_note, draft);
        println!("GPV05-NARROW-LIVE-TERMINAL flow={flow:?} undo_redo=true commit=true");
    }
}

fn inspect_caret(
    app: &mut App,
    item: ItemId,
    size: Size,
    flow: vesta_gpui::FlowDirection,
    constrained: bool,
    phase: &str,
    failures: &mut Vec<String>,
) {
    let mut current = frame(app, item, size, flow);
    for _ in 0..5 {
        if current.stable && caret_is_visible(current.view) {
            break;
        }
        current = frame(app, item, size, flow);
    }
    let visible = caret_is_visible(current.view);
    if visible {
        let point = Point::new(
            current.view.clip.x + current.view.clip.width * 0.5,
            current.view.clip.y + current.view.clip.height * 0.5,
        );
        assert_eq!(
            app.hit_test.hit(point),
            Some(TextFieldKind::TableNote.element(app.scopes.gallery)),
            "current note input geometry hits the editor"
        );
        assert_eq!(app.table_note_view, Some(current.view));
    }
    println!(
        "GPV05-NARROW-NOTE width={} constrained={constrained} flow={flow:?} phase={phase} cell={:?} editor={:?} clip={:?} caret={:?} stable={} visible={visible}",
        size.width,
        current.cell,
        current.editor,
        current.view.clip,
        current.view.caret,
        current.stable
    );
    if !visible || !current.stable {
        failures.push(format!(
            "width={} constrained={constrained} flow={flow:?} phase={phase}",
            size.width
        ));
    }
}
