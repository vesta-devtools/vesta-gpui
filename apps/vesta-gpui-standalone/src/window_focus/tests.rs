use super::*;
use crate::tests::{
    activate_gallery_page, install_action_route, install_action_route_with_viewport,
};
use winit::keyboard::ModifiersState;

mod narrow_table;

#[test]
fn text_and_popup_focus_traversal_uses_live_owners_and_releases_capture() {
    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.inputs");
    install_action_route(&mut app);
    app.start_text_selection(TextFieldKind::Main);
    assert_eq!(app.focused_text_field(), Some(TextFieldKind::Main));
    assert_eq!(
        app.input_ownership
            .capture()
            .map(|capture| capture.target.element),
        Some(TextFieldKind::Main.element(app.scopes.gallery))
    );

    app.traverse_focus(false);
    assert_eq!(app.focused_text_field(), Some(TextFieldKind::Search));
    assert_eq!(
        app.text_session,
        Some(TextSession {
            window: STANDALONE_WINDOW,
            field: TextFieldKind::Search
        })
    );
    assert!(app.input_ownership.capture().is_none());

    app.start_text_selection(TextFieldKind::Search);
    assert!(
        app.ui_actions
            .focus_order()
            .contains(&TextFieldKind::Area.element(app.scopes.gallery))
    );
    app.set_text_focus(Some(TextFieldKind::Area));
    assert_eq!(app.focused_text_field(), Some(TextFieldKind::Area));
    assert!(app.input_ownership.capture().is_none());
    app.start_text_selection(TextFieldKind::Area);
    app.set_text_focus(None);
    assert!(app.input_ownership.capture().is_none());
    assert_eq!(app.focused_text_field(), None);
    app.set_text_focus(Some(TextFieldKind::Number));
    let _ = app.number_state.reset(&mut app.text_context, "-42");
    app.traverse_window_focus(STANDALONE_WINDOW, false);
    assert_eq!(
        app.demo.number, -42,
        "Tab commits the old numeric draft through the shared transition"
    );
    assert_ne!(
        app.text_session_field(STANDALONE_WINDOW),
        Some(TextFieldKind::Number)
    );
    app.traverse_window_focus(STANDALONE_WINDOW, true);
    assert_eq!(
        app.text_session_field(STANDALONE_WINDOW),
        Some(TextFieldKind::Number)
    );

    // A dismissed popup's temporary tab stops must not consume the step.
    // Compare both directions with the ordinary route before it opens.
    for (slot, action) in [
        (COMBO_TRIGGER, DemoAction::ToggleCombo),
        (POPOVER_TRIGGER, DemoAction::TogglePopover),
    ] {
        for backwards in [false, true] {
            install_action_route(&mut app);
            let opener = app.scopes.gallery.element(slot);
            let order = FocusScope::new(STANDALONE_WINDOW, &app.ui_actions, None).order();
            let index = order
                .iter()
                .position(|target| target.element == opener)
                .expect("live popup opener");
            let expected = order[if backwards {
                (index + order.len() - 1) % order.len()
            } else {
                (index + 1) % order.len()
            }];
            app.set_focus_target(Some(opener));
            app.apply_demo_action(action);
            install_action_route(&mut app);
            app.text_bridge
                .update_modifiers(&winit::event::Modifiers::from(if backwards {
                    ModifiersState::SHIFT
                } else {
                    ModifiersState::empty()
                }));
            assert!(app.handle_window_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Tab)));
            assert!(!app.demo.combo.is_open() && !app.demo.popover.is_open());
            install_action_route(&mut app);
            assert_eq!(
                app.input_ownership.focus(),
                Some(expected),
                "closed popup {slot}, backwards={backwards}"
            );
        }
    }

    // A focus transfer must retire the host preview as well as Core capture;
    // otherwise the old release can commit after Tab moved to another owner.
    activate_gallery_page(&mut app, "gallery.controls");
    install_action_route(&mut app);
    let slider = app
        .ui_actions
        .focus_order()
        .iter()
        .copied()
        .find(|element| app.ui_actions.action_at(*element) == Some(&DemoAction::SetSlider))
        .expect("live slider");
    app.text_bridge
        .update_modifiers(&winit::event::Modifiers::from(ModifiersState::empty()));
    for transfer in 0..3 {
        let initial = app.demo.slider;
        let target = InputTarget {
            window: STANDALONE_WINDOW,
            element: slider,
        };
        app.set_focus_target(Some(slider));
        app.input_ownership
            .capture_pointer(target, PointerButton::Primary);
        app.slider_drag = Some(ValueEditSession::new(target, initial));
        app.demo.slider = 0.9;
        app.set_window_focus(UiWindowId(999), None);
        app.set_window_focus(STANDALONE_WINDOW, Some(slider));
        assert!(
            app.slider_drag.is_some(),
            "foreign or unchanged focus keeps the preview"
        );
        assert_eq!(
            app.input_ownership.capture().map(|c| c.target),
            Some(target)
        );
        match transfer {
            0 => assert!(app.handle_window_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Tab))),
            1 => app.set_window_focus(STANDALONE_WINDOW, None),
            _ => {
                app.request_window_focus_traversal(STANDALONE_WINDOW, false);
                install_action_route(&mut app);
            }
        }
        assert!(app.input_ownership.capture().is_none());
        assert!(
            app.slider_drag.is_none(),
            "focus transfer must cancel the host preview"
        );
        assert_eq!(
            app.demo.slider, initial,
            "restore before publishing replacement focus"
        );
        assert!(!app.finish_slider_edit(), "obsolete release cannot commit");
    }
    activate_gallery_page(&mut app, "gallery.inputs");
    install_action_route(&mut app);
    app.set_text_focus(Some(TextFieldKind::Number));
    let target = InputTarget {
        window: STANDALONE_WINDOW,
        element: TextFieldKind::Number.element(app.scopes.gallery),
    };
    app.demo.number = 128;
    app.number_scrub = Some(NumberScrub::new(target, 0.0, 128, 0.0).expect("number preview"));
    app.input_ownership
        .capture_pointer(target, PointerButton::Primary);
    app.demo.number = 400;
    app.number_state
        .reset(&mut app.text_context, "400")
        .expect("preview draft");
    app.traverse_window_focus(STANDALONE_WINDOW, false);
    assert!(app.number_scrub.is_none());
    assert_eq!(
        app.demo.number, 128,
        "Tab must not commit the cancelled drag draft"
    );
    assert_eq!(app.number_state.text(), "128");
    assert!(
        !app.finish_number_scrub(),
        "obsolete number release cannot commit"
    );
}

#[test]
fn nested_dialog_traps_focus_and_restores_each_live_opener() {
    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.layout");
    install_action_route(&mut app);
    let opener = app
        .ui_actions
        .focus_order()
        .iter()
        .copied()
        .find(|element| app.ui_actions.action_at(*element) == Some(&DemoAction::OpenDemoDialog))
        .expect("dialog opener");
    app.set_focus_target(Some(opener));
    app.apply_demo_action(DemoAction::OpenDemoDialog);
    let opened = install_action_route(&mut app);
    assert_eq!(app.demo.dialog_stack.top(), Some(1));
    let layout = LayoutEngine
        .compute(&opened, Viewport::new(1280.0, 800.0, 1.0))
        .expect("dialog layout");
    let isolated_root = accessibility_root(&opened, Some(dialog_element(app.scopes.root, 1, 0)));
    app.semantics.build(
        isolated_root,
        &layout,
        app.input_ownership.focus().map(|focus| focus.element),
        WINDOW_TITLE,
    );
    assert!(app.semantics.node_for_element(opener).is_none());
    assert!(
        app.semantics
            .node_for_element(dialog_element(app.scopes.root, 1, 1))
            .is_some()
    );
    assert_eq!(
        app.input_ownership.focus().map(|focus| focus.element),
        Some(dialog_element(app.scopes.root, 1, 5))
    );
    app.traverse_focus(false);
    assert!(app.input_ownership.focus().is_some_and(|focus| {
        app.ui_actions
            .is_descendant_of(focus.element, dialog_element(app.scopes.root, 1, 0))
    }));
    let nested_opener = dialog_element(app.scopes.root, 1, 9);
    app.set_focus_target(Some(nested_opener));
    app.apply_demo_action(DemoAction::OpenNestedDialog);
    let nested = install_action_route(&mut app);
    assert_eq!(app.demo.dialog_stack.top(), Some(2));
    let layout = LayoutEngine
        .compute(&nested, Viewport::new(1280.0, 800.0, 1.0))
        .expect("nested dialog layout");
    let isolated_root = accessibility_root(&nested, Some(dialog_element(app.scopes.root, 2, 0)));
    app.semantics.build(
        isolated_root,
        &layout,
        app.input_ownership.focus().map(|focus| focus.element),
        WINDOW_TITLE,
    );
    assert!(
        app.semantics
            .node_for_element(dialog_element(app.scopes.root, 1, 1))
            .is_none()
    );
    assert!(
        app.semantics
            .node_for_element(dialog_element(app.scopes.root, 2, 1))
            .is_some()
    );
    app.traverse_focus(false);
    assert!(app.input_ownership.focus().is_some_and(|focus| {
        app.ui_actions
            .is_descendant_of(focus.element, dialog_element(app.scopes.root, 2, 0))
    }));
    app.restore_editor_opener(InputTarget {
        window: STANDALONE_WINDOW,
        element: opener,
    });
    assert_eq!(
        app.input_ownership.focus(),
        None,
        "background opener cannot escape the top modal"
    );
    let active = dialog_element(app.scopes.root, 2, 5);
    app.restore_editor_opener(InputTarget {
        window: STANDALONE_WINDOW,
        element: active,
    });
    assert_eq!(
        app.input_ownership.focus().map(|target| target.element),
        Some(active)
    );
    let floating = UiWindowId(0x2000_0001);
    let floating_order = FocusScope::new(
        floating,
        &app.ui_actions,
        Some(dialog_element(app.scopes.root, 2, 0)),
    )
    .order();
    assert!(!floating_order.is_empty());
    assert!(
        floating_order.iter().all(|target| target.window == floating
            && app
                .ui_actions
                .is_descendant_of(target.element, dialog_element(app.scopes.root, 2, 0))),
        "the same resolver restricts native targets to the owned top modal"
    );
    app.apply_demo_action(DemoAction::CancelDialog(1));
    assert_eq!(app.demo.dialog_stack.top(), Some(2));
    app.apply_demo_action(DemoAction::CancelDialog(2));
    install_action_route(&mut app);
    assert_eq!(app.demo.dialog_stack.top(), Some(1));
    assert_eq!(
        app.input_ownership.focus().map(|focus| focus.element),
        Some(nested_opener)
    );
    let previous = app.demo.number;
    app.apply_demo_action(DemoAction::ConfirmDialog(1));
    assert!(app.demo.dialog_stack.is_empty());
    assert_eq!(app.demo.number, previous + 1);
    assert_eq!(
        app.window_focus(STANDALONE_WINDOW),
        None,
        "restore waits for the rebuilt tree"
    );
    install_action_route(&mut app);
    assert_eq!(
        app.input_ownership.focus().map(|focus| focus.element),
        Some(opener)
    );
}

#[test]
fn tab_focus_reveals_offscreen_dock_page_controls_without_moving_header_focus() {
    narrow_table::current_table_editor_remains_usable();
    edited_caret_reveals_from_the_current_frame();
    caret_requests_preserve_source_and_latest_intent();
    let mut app = App::new();
    let item = activate_gallery_page(&mut app, "gallery.inputs");
    let root = install_action_route(&mut app);
    app.demo
        .page_scroll
        .entry(item)
        .or_default()
        .sync_content_height(1400.0, 400.0);
    let gallery_target = app.scopes.gallery.element(TEXT_FIELD);
    app.scroll_geometry.gallery = Some(Rect::new(0.0, CHROME_HEIGHT, 400.0, 400.0));
    app.scroll_geometry
        .pages
        .insert(item, Rect::new(0.0, CHROME_HEIGHT, 400.0, 400.0));
    app.focus_rects
        .insert(gallery_target, Rect::new(20.0, 500.0, 100.0, 30.0));
    app.reveal_window_focus(STANDALONE_WINDOW, gallery_target);
    let revealed_offset = app
        .demo
        .page_scroll
        .get(&item)
        .expect("page scroll")
        .offset_y();
    assert!(revealed_offset > 0.0);

    app.reveal_window_focus(STANDALONE_WINDOW, app.scopes.header.element(MODE_2D));
    assert_eq!(
        app.demo
            .page_scroll
            .get(&item)
            .expect("page scroll")
            .offset_y(),
        revealed_offset
    );

    let tab = app
        .ui_actions
        .focus_order()
        .iter()
        .copied()
        .find(|element| {
            app.ui_actions.action_at(*element)
                == Some(&DemoAction::Workspace(WorkspaceViewAction::Tab(item)))
        })
        .expect("Inputs tab is focusable");
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("gallery layout");
    let focus_color = app.theme.palette.focus;
    let has_tab_ring = |paint: PaintList| {
        paint.commands().iter().any(|command| {
            matches!(
                command,
                vesta_gpui::PaintCommand::StrokeRoundedRect { element, color, .. }
                    if *element == tab && *color == focus_color
            )
        })
    };
    app.set_focus_target(Some(tab));
    app.input_ownership
        .observe_focus_input(vesta_gpui::FocusInput::Pointer);
    assert!(!has_tab_ring(PaintList::build_with_focus(
        &root,
        &layout,
        app.input_ownership
            .visible_focus()
            .map(|focus| focus.element),
        app.theme.palette.focus,
    )));
    app.input_ownership
        .observe_focus_input(vesta_gpui::FocusInput::Keyboard);
    assert!(has_tab_ring(PaintList::build_with_focus(
        &root,
        &layout,
        app.input_ownership
            .visible_focus()
            .map(|focus| focus.element),
        app.theme.palette.focus,
    )));
    app.input_ownership
        .observe_focus_input(vesta_gpui::FocusInput::Pointer);
    app.set_focus_target(Some(gallery_target));
    assert_eq!(app.input_ownership.visible_focus(), None);
    assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(gallery_target));

    for scale in [1.0, 1.25, 2.0] {
        for focus_slider in [false, true] {
            let mut app = App::new();
            let item = activate_gallery_page(&mut app, "gallery.controls");
            let size = Size::new(640.0, 500.0);
            let viewport = Viewport::new(size.width, size.height, scale);
            let root = install_action_route_with_viewport(&mut app, size);
            let layout = LayoutEngine
                .compute(&root, viewport)
                .expect("narrow layout");
            let slider = app
                .ui_actions
                .focus_order()
                .iter()
                .copied()
                .find(|id| app.ui_actions.action_at(*id) == Some(&DemoAction::SetSlider))
                .expect("slider owner");
            let enable = slider_demo::enable_target(app.scopes.gallery);
            let page_id = page_scroll_id(app.scopes.gallery, item);
            let page = layout.node(page_id).expect("page viewport");
            let slider_rect = layout.node(slider).expect("slider layout").rect;
            let enable_rect = layout.node(enable).expect("enable layout").rect;
            assert!(enable_rect.y >= slider_rect.bottom(), "enable must wrap");
            let scroll = app.demo.page_scroll.entry(item).or_default();
            scroll.sync_content_height(page.scroll_content.expect("content").1, page.rect.height);
            scroll.scroll_by((slider_rect.bottom() - page.rect.bottom() + 1.0).max(0.0));
            let before = scroll.offset_y();
            let root = install_action_route_with_viewport(&mut app, size);
            let layout = LayoutEngine
                .compute(&root, viewport)
                .expect("scrolled layout");
            let page = layout.node(page_id).expect("page viewport");
            assert!(
                layout.node(slider).expect("slider").rect.bottom() <= page.rect.bottom() + 0.01
            );
            assert!(layout.node(enable).expect("enable").rect.y >= page.rect.bottom());
            app.scroll_geometry =
                ScrollGeometry::from_layout(app.scopes.gallery, &layout, std::iter::once(item));
            app.focus_rects = app
                .ui_actions
                .focus_order()
                .iter()
                .copied()
                .filter_map(|id| layout.node(id).map(|node| (id, node.rect)))
                .collect();
            let header = app.scopes.header.element(MODE_2D);
            app.set_focus_target(Some(if focus_slider { slider } else { header }));
            let target = InputTarget {
                window: STANDALONE_WINDOW,
                element: slider,
            };
            let initial = app.demo.slider;
            app.input_ownership
                .capture_pointer(target, PointerButton::Primary);
            app.slider_drag = Some(ValueEditSession::new(target, initial));
            app.demo.slider = 0.9;
            app.apply_window_action(STANDALONE_WINDOW, DemoAction::ToggleSliderEnabled);
            assert_eq!(app.demo.slider, initial);
            assert!(app.slider_drag.is_none());
            assert!(app.input_ownership.capture().is_none());
            let after = app
                .demo
                .page_scroll
                .get(&item)
                .expect("page scroll")
                .offset_y();
            if focus_slider {
                assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(enable));
                assert!(after > before, "replacement focus must scroll into view");
                let root = install_action_route_with_viewport(&mut app, size);
                let layout = LayoutEngine
                    .compute(&root, viewport)
                    .expect("revealed layout");
                let rect = layout.node(enable).expect("revealed enable").rect;
                let hit = HitTestTree::build(&root, &layout).hit(Point::new(
                    rect.x + rect.width / 2.0,
                    rect.y + rect.height / 2.0,
                ));
                assert_eq!(
                    hit.and_then(|id| app.ui_actions.action_for_hit(id)),
                    Some(&DemoAction::ToggleSliderEnabled)
                );
            } else {
                assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(header));
                assert_eq!(after, before, "unrelated focus must not move the page");
            }
        }
    }
}

fn caret_is_visible(view: vesta_gpui::TextInputViewport) -> bool {
    view.caret.is_some_and(|caret| {
        view.clip.width > 0.0
            && view.clip.height > 0.0
            && caret.x >= view.clip.x
            && caret.y >= view.clip.y
            && caret.right() <= view.clip.right()
            && caret.bottom() <= view.clip.bottom()
    })
}

fn prepare_caret_probe(app: &mut App, item: ItemId) -> (PaintList, bool) {
    let size = Size::new(640.0, 300.0);
    let root = install_action_route_with_viewport(app, size);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(size.width, size.height, 1.0))
        .expect("current editor layout");
    let wrap_changed = app.sync_area_wrap_layout(app.scopes.gallery, &layout);
    app.scroll_geometry =
        ScrollGeometry::from_layout(app.scopes.gallery, &layout, [item].into_iter());
    let scroll_changed = app.demo.page_scroll.entry(item).or_default().sync_layout(
        layout
            .node(page_scroll_id(app.scopes.gallery, item))
            .expect("page"),
    );
    app.focus_rects = app
        .ui_actions
        .focus_order()
        .iter()
        .filter_map(|id| layout.node(*id).map(|node| (*id, node.rect)))
        .collect();
    app.hit_test = HitTestTree::build(&root, &layout);
    let paint = PaintList::build(&root, &layout);
    (paint, !(wrap_changed || scroll_changed))
}

fn process_caret_probe(
    app: &mut App,
    paint: &PaintList,
    window: UiWindowId,
    layout_stable: bool,
    focus_root: Option<ElementId>,
) -> bool {
    complete_caret_reveal(
        &mut app.caret_reveal,
        app.text_session,
        CaretRevealFrame {
            window,
            gallery: app.scopes.gallery,
            actions: &app.ui_actions,
            focus: app.input_ownership.focus(),
            focus_root,
            geometry: &app.scroll_geometry,
            paint,
            layout_stable,
        },
        &mut ScrollStates {
            tree: &mut app.demo.tree_scroll,
            virtual_list: &mut app.demo.virtual_scroll,
            virtual_table: &mut app.demo.virtual_table_scroll,
            nested: &mut app.demo.nested_scroll,
            horizontal: &mut app.demo.horizontal_scroll,
            gallery: Some(&mut app.gallery_scroll),
            pages: &mut app.demo.page_scroll,
            tabs: &mut app.demo.tab_scroll,
        },
    )
}

fn caret_probe_frame(
    app: &mut App,
    item: ItemId,
    field: TextFieldKind,
) -> vesta_gpui::TextInputViewport {
    let (paint, stable) = prepare_caret_probe(app, item);
    let focus_root = app.focus_root_for_window(STANDALONE_WINDOW);
    process_caret_probe(app, &paint, STANDALONE_WINDOW, stable, focus_root);
    text_views_from_paint(STANDALONE_WINDOW, app.scopes.gallery, &paint)
        .expect("window-qualified text views")
        .get(field)
        .expect("retained current editor view")
}

fn edited_caret_reveals_from_the_current_frame() {
    let fields = TextFieldKind::ALL
        .into_iter()
        .map(|field| (field, None))
        .chain([
            (TextFieldKind::ColorHex, Some(ColorPickerMode::Rgb)),
            (TextFieldKind::ColorHex, Some(ColorPickerMode::Hsv)),
        ]);
    for (field, ramp_mode) in fields {
        for refresh_before_input in [false, true] {
            let mut app = App::new();
            let font = include_bytes!(
                "../../../../crates/vesta-gpui/tests/fixtures/MaterialIcons-Regular.ttf"
            );
            assert!(
                app.text_context
                    .register_font(std::sync::Arc::new(font.to_vec()))
            );
            let item = activate_gallery_page(
                &mut app,
                if field == TextFieldKind::TableNote {
                    "gallery.collections"
                } else {
                    "gallery.inputs"
                },
            );
            if field == TextFieldKind::ColorHex {
                if let Some(mode) = ramp_mode {
                    app.apply_window_action(STANDALONE_WINDOW, DemoAction::RampEditColor);
                    app.apply_window_action(STANDALONE_WINDOW, DemoAction::SelectColorMode(mode));
                } else {
                    app.apply_window_action(STANDALONE_WINDOW, DemoAction::ToggleColorPicker);
                }
            }
            caret_probe_frame(&mut app, item, field);
            app.begin_text_selection(STANDALONE_WINDOW, field, None);
            let (state, context) = app.text_state_and_context_mut(field);
            state.act(
                context,
                vesta_gpui::TextInputAction::TextEnd { extend: false },
            );
            caret_probe_frame(&mut app, item, field);
            let owner = field.element(app.scopes.gallery);
            app.reveal_window_focus(STANDALONE_WINDOW, owner);
            caret_probe_frame(&mut app, item, field);
            let visible_offset = app.demo.page_scroll[&item].offset_y();
            app.demo
                .page_scroll
                .get_mut(&item)
                .expect("page")
                .scroll_by(f32::MAX);
            let mut away = caret_probe_frame(&mut app, item, field);
            if caret_is_visible(away) {
                app.demo
                    .page_scroll
                    .get_mut(&item)
                    .expect("page")
                    .scroll_by(-f32::MAX);
                away = caret_probe_frame(&mut app, item, field);
            }
            assert!(
                away.caret.is_some() && !caret_is_visible(away),
                "fixture scrolls {field:?} away"
            );
            let away_offset = app.demo.page_scroll[&item].offset_y();
            if !refresh_before_input {
                let page = app.demo.page_scroll.get_mut(&item).expect("page");
                page.scroll_by(visible_offset - page.offset_y());
                caret_probe_frame(&mut app, item, field);
                let page = app.demo.page_scroll.get_mut(&item).expect("page");
                page.scroll_by(away_offset - page.offset_y());
            }
            let before = app.text_state_and_context_mut(field).0.text().to_owned();
            let model = (
                app.demo.number,
                app.demo.vector,
                app.demo.table_note.clone(),
                app.demo.color_committed,
                app.demo.color_draft,
                app.demo.ramp.snapshot(),
            );
            assert!(!app.handle_window_text_event(
                UiWindowId(999),
                &WindowEvent::Ime(winit::event::Ime::Commit("foreign".into()))
            ));
            assert!(app.handle_window_text_event(
                STANDALONE_WINDOW,
                &WindowEvent::Ime(winit::event::Ime::Enabled)
            ));
            assert!(app.handle_window_text_event(
                STANDALONE_WINDOW,
                &WindowEvent::Ime(winit::event::Ime::Commit("z".into()))
            ));
            if field == TextFieldKind::Label {
                assert_eq!(app.readonly_label_state.text(), before);
                assert!(!app.readonly_label_state.is_composing());
                assert!(
                    app.caret_reveal.is_none(),
                    "read-only IME cannot request reveal"
                );
                assert_eq!(app.demo.page_scroll[&item].offset_y(), away_offset);
                assert!(app.handle_window_text_event(
                    STANDALONE_WINDOW,
                    &WindowEvent::KeyboardInput {
                        device_id: winit::event::DeviceId::dummy(),
                        event: vesta_gpui_winit::UiKeyEvent {
                            physical_key: winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Home,
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
                assert_eq!(
                    app.readonly_label_state.inspection_selection(),
                    Some((0, 0))
                );
            }
            assert_eq!(
                app.text_state_and_context_mut(field).0.text(),
                if field == TextFieldKind::Label {
                    before.clone()
                } else {
                    format!("{before}z")
                }
            );
            let mut view = caret_probe_frame(&mut app, item, field);
            for _ in 0..2 {
                if caret_is_visible(view) {
                    break;
                }
                view = caret_probe_frame(&mut app, item, field);
            }
            assert!(
                caret_is_visible(view),
                "admitted edit must reveal current caret through outer page: field={field:?} refresh={refresh_before_input} offset_before={away_offset} offset_after={} caret={:?} clip={:?}",
                app.demo.page_scroll[&item].offset_y(),
                view.caret,
                view.clip
            );
            assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(owner));
            assert_eq!(app.text_session_field(STANDALONE_WINDOW), Some(field));
            assert_eq!(
                (
                    app.demo.number,
                    app.demo.vector,
                    app.demo.table_note.clone(),
                    app.demo.color_committed,
                    app.demo.color_draft,
                    app.demo.ramp.snapshot(),
                ),
                model,
                "revealing cannot commit drafts"
            );
            let (state, context) = app.text_state_and_context_mut(field);
            state.act(context, vesta_gpui::TextInputAction::Undo);
            assert_eq!(
                state.text(),
                before,
                "reveal preserves the single editor transaction"
            );
            eprintln!(
                "current-field-reveal field={field:?} ramp_mode={ramp_mode:?} refresh={refresh_before_input} offset_before={away_offset} offset_after={} caret={:?} clip={:?}",
                app.demo.page_scroll[&item].offset_y(),
                view.caret,
                view.clip
            );
        }
    }
}

fn caret_requests_preserve_source_and_latest_intent() {
    foreign_area_focus_preserves_the_main_manual_viewport();
    #[derive(Clone, Copy, Debug)]
    enum Exit {
        Frame,
        Focus,
        Removed,
        Excluded,
        Wheel,
    }
    for exit in [
        Exit::Frame,
        Exit::Focus,
        Exit::Removed,
        Exit::Excluded,
        Exit::Wheel,
    ] {
        let mut app = App::new();
        let font = include_bytes!(
            "../../../../crates/vesta-gpui/tests/fixtures/MaterialIcons-Regular.ttf"
        );
        assert!(
            app.text_context
                .register_font(std::sync::Arc::new(font.to_vec()))
        );
        let item = activate_gallery_page(&mut app, "gallery.inputs");
        caret_probe_frame(&mut app, item, TextFieldKind::Main);
        app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::Main, None);
        app.text_state.act(
            &mut app.text_context,
            vesta_gpui::TextInputAction::TextEnd { extend: false },
        );
        app.demo
            .page_scroll
            .get_mut(&item)
            .expect("page")
            .scroll_by(f32::MAX);
        let away = caret_probe_frame(&mut app, item, TextFieldKind::Main);
        assert!(!caret_is_visible(away));
        let offset = app.demo.page_scroll[&item].offset_y();
        let owner = TextFieldKind::Main.element(app.scopes.gallery);
        let before = app.text_state.text().to_owned();
        assert!(app.handle_window_text_event(
            STANDALONE_WINDOW,
            &WindowEvent::Ime(winit::event::Ime::Disabled)
        ));
        caret_probe_frame(&mut app, item, TextFieldKind::Main);
        assert_eq!(
            app.demo.page_scroll[&item].offset_y(),
            offset,
            "unchanged input preserves manual scroll"
        );
        assert_eq!(app.caret_reveal, None);
        app.handle_window_text_event(
            STANDALONE_WINDOW,
            &WindowEvent::Ime(winit::event::Ime::Enabled),
        );
        for value in ["a", "b"] {
            app.handle_window_text_event(
                STANDALONE_WINDOW,
                &WindowEvent::Ime(winit::event::Ime::Commit(value.into())),
            );
        }
        assert_eq!(app.text_state.text(), format!("{before}ab"));
        let pending = app.caret_reveal.expect("changed inputs coalesce");
        let (mut paint, mut stable) = prepare_caret_probe(&mut app, item);
        for _ in 0..2 {
            if stable {
                break;
            }
            (paint, stable) = prepare_caret_probe(&mut app, item);
        }
        assert!(stable);
        match exit {
            Exit::Frame => {
                app.wheel(MouseScrollDelta::LineDelta(f32::NAN, 0.0));
                assert_eq!(
                    app.caret_reveal,
                    Some(pending),
                    "invalid wheel must preserve the admitted caret request"
                );
                app.wheel(MouseScrollDelta::LineDelta(0.0, 0.0));
                assert_eq!(
                    app.caret_reveal,
                    Some(pending),
                    "zero wheel cannot supersede an edit"
                );
                assert!(!process_caret_probe(
                    &mut app,
                    &paint,
                    UiWindowId(999),
                    true,
                    None
                ));
                assert_eq!(
                    app.caret_reveal,
                    Some(pending),
                    "another frame cannot consume the owner"
                );
                assert!(!process_caret_probe(
                    &mut app,
                    &paint,
                    STANDALONE_WINDOW,
                    false,
                    None
                ));
                assert_eq!(app.caret_reveal, Some(pending), "wait for current geometry");
                assert_eq!(app.demo.page_scroll[&item].offset_y(), offset);
                assert!(process_caret_probe(
                    &mut app,
                    &paint,
                    STANDALONE_WINDOW,
                    true,
                    None
                ));
                assert_eq!(app.caret_reveal, None);
                let revealed = app.demo.page_scroll[&item].offset_y();
                assert!(!process_caret_probe(
                    &mut app,
                    &paint,
                    STANDALONE_WINDOW,
                    true,
                    None
                ));
                assert_eq!(
                    app.demo.page_scroll[&item].offset_y(),
                    revealed,
                    "request is one-shot"
                );
                assert!(caret_is_visible(caret_probe_frame(
                    &mut app,
                    item,
                    TextFieldKind::Main
                )));
                app.handle_window_text_event(
                    STANDALONE_WINDOW,
                    &WindowEvent::Ime(winit::event::Ime::Commit("c".into())),
                );
                let (paint, stable) = prepare_caret_probe(&mut app, item);
                assert!(stable);
                let before_scroll = app.demo.page_scroll[&item];
                let changed = process_caret_probe(&mut app, &paint, STANDALONE_WINDOW, true, None);
                eprintln!(
                    "suggestion-caret-stability before={before_scroll:?} after={:?}",
                    app.demo.page_scroll[&item]
                );
                assert!(!changed, "visible caret needs no redraw loop");
                assert_eq!(app.caret_reveal, None);
                assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(owner));
            }
            Exit::Focus => {
                let header = app.scopes.header.element(MODE_2D);
                app.set_window_focus(STANDALONE_WINDOW, Some(header));
                assert_eq!(app.caret_reveal, None);
                caret_probe_frame(&mut app, item, TextFieldKind::Main);
                assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(header));
                assert_eq!(app.demo.page_scroll[&item].offset_y(), offset);
            }
            Exit::Removed => {
                app.retire_window_subtree_input(STANDALONE_WINDOW, owner);
                assert_eq!(app.caret_reveal, None);
                caret_probe_frame(&mut app, item, TextFieldKind::Main);
                assert_eq!(app.text_session, None);
                assert_eq!(app.demo.page_scroll[&item].offset_y(), offset);
            }
            Exit::Excluded => {
                let root = app.scopes.header.element(MODE_2D);
                assert!(!process_caret_probe(
                    &mut app,
                    &paint,
                    STANDALONE_WINDOW,
                    true,
                    Some(root)
                ));
                assert_eq!(app.caret_reveal, None);
                assert_eq!(app.demo.page_scroll[&item].offset_y(), offset);
                assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(owner));
            }
            Exit::Wheel => {
                let page = app.scroll_geometry.pages[&item];
                let target = app
                    .ui_actions
                    .focus_order()
                    .iter()
                    .filter(|id| {
                        app.ui_actions
                            .is_descendant_of(**id, page_scroll_id(app.scopes.gallery, item))
                    })
                    .find_map(|id| app.focus_rects[id].intersection(page))
                    .expect("visible page control");
                app.cursor = Point::new(
                    target.x + target.width * 0.5,
                    target.y + target.height * 0.5,
                );
                app.wheel(MouseScrollDelta::LineDelta(0.0, 1.0));
                assert_eq!(
                    app.caret_reveal, None,
                    "later manual wheel supersedes edit reveal"
                );
                let manual = app.demo.page_scroll[&item].offset_y();
                assert!(manual < offset);
                caret_probe_frame(&mut app, item, TextFieldKind::Main);
                assert_eq!(app.demo.page_scroll[&item].offset_y(), manual);
                assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(owner));
            }
        }
    }
}

fn foreign_area_focus_preserves_the_main_manual_viewport() {
    let mut app = App::new();
    let foreign = UiWindowId(0x2000_0001);
    app.area_state
        .reset(&mut app.text_context, &"manual view\n".repeat(30))
        .expect("overflowing area");
    app.area_state.act(
        &mut app.text_context,
        vesta_gpui::TextInputAction::TextStart { extend: false },
    );
    app.area_scroll_y = 96.0;
    app.area_follow_caret = false;
    let bounds = Rect::new(0.0, 0.0, 360.0, 96.0);
    let main_view = |app: &App| {
        app.area_state
            .snapshot()
            .with_viewport_scroll(Point::new(0.0, app.area_scroll_y), app.area_follow_caret)
            .viewport(bounds, bounds, Point::default())
    };
    let before = main_view(&app);
    assert_eq!(before.scroll_y, 96.0);
    let content = app.area_state.text().to_owned();
    let selection = app.area_state.inspection_selection();
    // This typed boundary is called after destination admission. No native Host
    // or physical input is fabricated for the foreign source.
    app.transition_window_text_session(foreign, Some(TextFieldKind::Area));
    let after = main_view(&app);
    assert_eq!(
        after.scroll_y, before.scroll_y,
        "foreign Area focus must preserve the Main manual viewport"
    );
    assert_eq!(app.area_state.text(), content);
    assert_eq!(app.area_state.inspection_selection(), selection);
    app.transition_window_text_session(STANDALONE_WINDOW, Some(TextFieldKind::Area));
    let own = main_view(&app).scroll_y;
    assert_eq!(own, 0.0, "own caret transfer follows the caret");
    assert_eq!(app.area_scroll_y, 96.0, "projection owns reconciliation");
    app.area_follow_caret = false;
    app.transition_window_text_session(STANDALONE_WINDOW, Some(TextFieldKind::Area));
    let repeated = main_view(&app).scroll_y;
    assert_eq!(repeated, 96.0, "same session is not a new reveal");
    app.transition_window_text_session(foreign, Some(TextFieldKind::Main));
    let other_field = main_view(&app).scroll_y;
    assert_eq!(other_field, 96.0);
    assert_eq!(app.area_state.text(), content);
    assert_eq!(app.area_state.inspection_selection(), selection);
    eprintln!(
        "area-owner-follow before={} after_foreign={} own={} same_session={} other_field={}",
        before.scroll_y, after.scroll_y, own, repeated, other_field
    );
}
