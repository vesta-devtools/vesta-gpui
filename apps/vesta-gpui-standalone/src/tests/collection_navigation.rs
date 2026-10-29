use super::*;
use winit::keyboard::ModifiersState;

mod pane_width;
mod table_viewport;
mod tile_viewport;

#[test]
fn virtual_tree_restores_parent_focus_when_a_selected_branch_collapses() {
    pane_width::qualify();
    tile_viewport::qualify();
    table_viewport::qualify();
    collection_keyboard_toggle_keeps_row_focus();
    tree_ranges_follow_the_current_hierarchy();
    let mut paging_failures = collection_paging_uses_the_current_visible_rows();
    paging_failures.extend(fixed_collection_paging_uses_the_current_visible_rows());
    assert!(
        paging_failures.is_empty(),
        "current viewport paging mismatch: {paging_failures:?}"
    );
    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.collections");
    let root = install_action_route(&mut app);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("Gallery layout");
    let tree = layout
        .node(app.scopes.gallery.element(TREE_SCROLL))
        .expect("tree viewport");
    for flow in [
        vesta_gpui::FlowDirection::LeftToRight,
        vesta_gpui::FlowDirection::RightToLeft,
    ] {
        for width in [320.0, 180.0] {
            let mut isolated = root.find(tree.id).expect("tree element").clone();
            isolated.style.width = Dimension::Px(width);
            isolated.style.flow_direction = Some(flow);
            let projection = LayoutEngine
                .compute(&isolated, Viewport::new(width, TREE_VIEWPORT_HEIGHT, 1.0))
                .expect("tree projection");
            let node = projection.node(tree.id).expect("scroll");
            let visible = node.scroll_viewport();
            let (Some(track), _, horizontal, _) = node.scrollbar_geometry(Point::default()) else {
                panic!("overflowing tree reserves a vertical track");
            };
            assert!(
                horizontal.is_none(),
                "responsive tree content fits the remaining width"
            );
            let command = projection
                .node(app.scopes.gallery.element(21_000_000 + 4 * 8 + 4))
                .expect("child command");
            assert!(
                command.rect.x >= visible.x && command.rect.right() <= visible.right(),
                "tree indentation must not clip the descendant command: {:?} in {:?}",
                command.rect,
                visible
            );
            let pointer = Point::new(track.x + track.width * 0.5, track.y + track.height * 0.5);
            assert_eq!(
                HitTestTree::build(&isolated, &projection).hit(pointer),
                Some(tree.id)
            );
        }
    }
    let (_, content_height) = tree.scroll_content.expect("tree content");
    app.demo
        .tree_scroll
        .sync_content_height(content_height, tree.rect.height);
    app.set_focus_target(Some(app.scopes.gallery.element(TREE_ROW_BASE + 4)));
    let row_command = app.scopes.gallery.element(21_000_000 + 4 * 8 + 4);
    assert_eq!(
        app.ui_actions.action_for_hit(row_command),
        Some(&DemoAction::OpenCommandPalette)
    );
    app.set_focus_target(Some(row_command));
    app.demo.tree_scroll.scroll_by(2_000.0);
    let retained_child = install_action_route(&mut app);
    assert!(retained_child.find(row_command).is_some());
    assert_eq!(app.focused_tree_index(), Some(4));
    app.demo.tree_scroll.scroll_by(-2_000.0);
    install_action_route(&mut app);
    app.set_focus_target(Some(app.scopes.gallery.element(TREE_ROW_BASE + 4)));
    assert!(app.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::End)));
    assert!(app.demo.tree_selection.contains(&1_000));
    install_action_route(&mut app);
    let last = app.scopes.gallery.element(TREE_ROW_BASE + 1_000);
    assert!(app.ui_actions.focus_order().contains(&last));
    app.apply_demo_action(DemoAction::ToggleVirtualTree(0));
    assert!(!app.demo.tree_expansion.contains(&0));
    let root_row = app.scopes.gallery.element(TREE_ROW_BASE);
    assert_eq!(
        app.input_ownership.focus().map(|focus| focus.element),
        Some(root_row)
    );
    install_action_route(&mut app);
    assert!(!app.ui_actions.focus_order().contains(&last));
    assert!(app.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowRight)));
    install_action_route(&mut app);
    assert!(app.ui_actions.focus_order().contains(&root_row));
    assert!(app.demo.tree_expansion.contains(&0));
    let (_, focus, route) = collection_window_fixture(
        root.find(app.scopes.gallery.element(TREE_ROW_BASE + 4))
            .expect("tree fixture"),
        DemoAction::SelectVirtualTree(4),
    );
    let native_focus =
        collection_focus(&app.demo, &route, Some(focus), None).expect("native tree focus");
    assert_eq!(
        window_collections::collection_key_update(
            &app.demo,
            native_focus,
            1,
            TABLE_VIRTUAL_VIEWPORT_HEIGHT,
            &Key::Named(NamedKey::ArrowLeft)
        ),
        Some(window_collections::CollectionKeyUpdate::Move(
            CollectionFocus::Tree { id: 0, index: 0 }
        ))
    );
    app.apply_demo_action(DemoAction::ToggleVirtualTree(0));
    assert_eq!(
        collection_focus(&app.demo, &route, Some(focus), None),
        None,
        "collapsed descendants cannot become a floating virtual focus projection"
    );
}

fn collection_keyboard_toggle_keeps_row_focus() {
    let mut failures = Vec::new();
    for tree in [false, true] {
        let mut app = App::new();
        activate_gallery_page(&mut app, "gallery.collections");
        app.apply_demo_action(if tree {
            DemoAction::SelectVirtualTree(4)
        } else {
            DemoAction::SelectVirtual(4)
        });
        install_action_route(&mut app);
        let row = app.scopes.gallery.element(if tree {
            TREE_ROW_BASE + 4
        } else {
            VIRTUAL_ROW_BASE + 4
        });
        app.set_focus_target(Some(row));
        app.text_bridge
            .update_modifiers(&ModifiersState::CONTROL.into());
        let first = app.handle_window_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Space));
        let selected_after_first = if tree {
            app.demo.tree_selection.contains(&4)
        } else {
            app.demo.virtual_selection.contains(&4)
        };
        let root = install_action_route(&mut app);
        let semantic_first = root
            .find(row)
            .expect("live toggled row")
            .semantic
            .as_ref()
            .expect("selected semantic")
            .selected;
        let focus_first = app.window_focus(STANDALONE_WINDOW);
        let second = app.handle_window_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Space));
        let selected_after_second = if tree {
            app.demo.tree_selection.contains(&4)
        } else {
            app.demo.virtual_selection.contains(&4)
        };
        println!(
            "GPV02-CTRL-SPACE tree={tree} first={first} selected_first={selected_after_first} semantic_first={semantic_first:?} focus_kept={} second={second} selected_second={selected_after_second}",
            focus_first == Some(row) && app.window_focus(STANDALONE_WINDOW) == Some(row)
        );
        if !first
            || selected_after_first
            || semantic_first != Some(false)
            || focus_first != Some(row)
            || !second
            || !selected_after_second
            || app.window_focus(STANDALONE_WINDOW) != Some(row)
        {
            failures.push(tree);
        }
        let selection = if tree {
            app.demo.tree_selection.clone()
        } else {
            app.demo.virtual_selection.clone()
        };
        for modifiers in [
            ModifiersState::empty(),
            ModifiersState::CONTROL | ModifiersState::SHIFT,
            ModifiersState::CONTROL | ModifiersState::ALT,
            ModifiersState::CONTROL | ModifiersState::SUPER,
        ] {
            app.text_bridge.update_modifiers(&modifiers.into());
            assert!(
                !app.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::Space))
            );
        }
        app.text_bridge
            .update_modifiers(&ModifiersState::CONTROL.into());
        assert!(!app.handle_window_key(UiWindowId(999), &Key::Named(NamedKey::Space)));
        let command = app.scopes.gallery.element(if tree {
            21_000_000 + 4 * 8 + 4
        } else {
            20_000_000 + 4 * 8 + 4
        });
        app.set_focus_target(Some(command));
        assert!(!app.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::Space)));
        if tree {
            app.set_focus_target(Some(app.scopes.gallery.element(TREE_DISCLOSURE_BASE)));
            assert!(
                !app.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::Space))
            );
        }
        app.set_focus_target(Some(row));
        app.demo.menu.open(MenuId(1));
        assert!(!app.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::Space)));
        app.demo.menu.close();
        app.apply_demo_action(DemoAction::OpenDemoDialog);
        install_action_route(&mut app);
        assert!(!app.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::Space)));
        app.close_demo_dialog(1, false);
        install_action_route(&mut app);
        app.set_focus_target(Some(TextFieldKind::TableNote.element(app.scopes.gallery)));
        assert!(!app.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::Space)));
        assert_eq!(
            if tree {
                &app.demo.tree_selection
            } else {
                &app.demo.virtual_selection
            },
            &selection
        );
        println!(
            "GPV02-CTRL-SPACE-GUARDS tree={tree} modifiers=4 foreign=true child=true disclosure={tree} menu=true modal=true text=true"
        );
    }
    assert!(
        failures.is_empty(),
        "Ctrl+Space must toggle exact selection rows without moving focus: {failures:?}"
    );
}

fn tree_ranges_follow_the_current_hierarchy() {
    for width in [1280.0, 360.0] {
        let mut app = App::new();
        activate_gallery_page(&mut app, "gallery.collections");
        app.demo.tree_entries = [
            (100, 0, true),
            (10, 1, true),
            (41, 2, false),
            (73, 2, false),
            (8, 1, false),
            (900, 0, false),
        ]
        .into_iter()
        .map(|(id, depth, has_children)| TreeViewEntry {
            id,
            label: "Item".into(),
            depth,
            has_children,
        })
        .collect();
        app.demo.tree_expansion = TreeExpansionState::default();
        app.demo.tree_expansion.toggle(100);
        app.demo.tree_expansion.toggle(10);
        app.demo.tree_selection = ListSelectionState::default();
        let row = |id| app.scopes.gallery.element(TREE_ROW_BASE + id);
        let row41 = row(41);
        let row73 = row(73);
        let row10 = row(10);
        install_action_route_with_viewport(&mut app, Size::new(width, 800.0));
        app.apply_demo_action(DemoAction::SelectVirtualTree(41));
        app.set_focus_target(Some(row41));
        tree_selection_frame(
            &mut app,
            width,
            "initial",
            Some(41),
            &[41],
            &[100, 10, 41, 73, 8, 900],
        );

        tree_selection_key(&mut app, ModifiersState::SHIFT, NamedKey::ArrowDown);
        tree_selection_frame(
            &mut app,
            width,
            "shift_down",
            Some(73),
            &[41, 73],
            &[100, 10, 41, 73, 8, 900],
        );
        tree_selection_key(&mut app, ModifiersState::CONTROL, NamedKey::ArrowDown);
        tree_selection_frame(
            &mut app,
            width,
            "ctrl_focus",
            Some(8),
            &[41, 73],
            &[100, 10, 41, 73, 8, 900],
        );
        tree_selection_key(&mut app, ModifiersState::CONTROL, NamedKey::Space);
        tree_selection_frame(
            &mut app,
            width,
            "toggle_on",
            Some(8),
            &[8, 41, 73],
            &[100, 10, 41, 73, 8, 900],
        );
        tree_selection_key(&mut app, ModifiersState::CONTROL, NamedKey::Space);
        tree_selection_frame(
            &mut app,
            width,
            "toggle_off",
            Some(8),
            &[41, 73],
            &[100, 10, 41, 73, 8, 900],
        );
        tree_selection_key(
            &mut app,
            ModifiersState::CONTROL | ModifiersState::SHIFT,
            NamedKey::ArrowUp,
        );
        tree_selection_frame(
            &mut app,
            width,
            "add_range",
            Some(73),
            &[8, 41, 73],
            &[100, 10, 41, 73, 8, 900],
        );
        app.apply_window_action(STANDALONE_WINDOW, DemoAction::ToggleVirtualTree(10));
        let collapsed = tree_selection_frame(
            &mut app,
            width,
            "collapse_group",
            Some(10),
            &[8, 41, 73],
            &[100, 10, 8, 900],
        );
        assert!(collapsed.find(row41).is_none() && collapsed.find(row73).is_none());
        assert_eq!(
            collapsed
                .find(row10)
                .expect("parent row")
                .semantic
                .as_ref()
                .expect("parent semantics")
                .expanded,
            Some(false)
        );
        tree_selection_key(&mut app, ModifiersState::SHIFT, NamedKey::ArrowDown);
        tree_selection_frame(
            &mut app,
            width,
            "visible_anchor",
            Some(8),
            &[8],
            &[100, 10, 8, 900],
        );
        let selected = app.demo.tree_selection.clone();
        app.apply_window_action(STANDALONE_WINDOW, DemoAction::SelectVirtualTree(41));
        assert_eq!(
            app.demo.tree_selection, selected,
            "hidden rows reject stale selection intents"
        );
        app.text_bridge
            .update_modifiers(&ModifiersState::empty().into());
        app.apply_demo_action(DemoAction::SelectVirtualTree(10));
        app.set_focus_target(Some(row10));
        tree_selection_key(&mut app, ModifiersState::empty(), NamedKey::ArrowRight);
        tree_selection_frame(
            &mut app,
            width,
            "expand_group",
            Some(10),
            &[10],
            &[100, 10, 41, 73, 8, 900],
        );
        tree_selection_key(&mut app, ModifiersState::empty(), NamedKey::ArrowRight);
        tree_selection_frame(
            &mut app,
            width,
            "first_child",
            Some(41),
            &[41],
            &[100, 10, 41, 73, 8, 900],
        );
        app.apply_window_action(STANDALONE_WINDOW, DemoAction::ToggleVirtualTree(100));
        tree_selection_frame(
            &mut app,
            width,
            "collapse_root",
            Some(100),
            &[41],
            &[100, 900],
        );
        tree_selection_key(&mut app, ModifiersState::SHIFT, NamedKey::ArrowDown);
        tree_selection_frame(
            &mut app,
            width,
            "hidden_anchor_fallback",
            Some(900),
            &[900],
            &[100, 900],
        );
        tree_selection_key(&mut app, ModifiersState::SHIFT, NamedKey::ArrowUp);
        tree_selection_frame(
            &mut app,
            width,
            "fallback_anchor_range",
            Some(100),
            &[100, 900],
            &[100, 900],
        );
        app.apply_window_action(STANDALONE_WINDOW, DemoAction::ToggleVirtualTree(100));
        tree_selection_frame(
            &mut app,
            width,
            "restore_expansion",
            Some(100),
            &[100, 900],
            &[100, 10, 41, 73, 8, 900],
        );

        app.text_bridge
            .update_modifiers(&ModifiersState::empty().into());
        app.apply_demo_action(DemoAction::SelectVirtualTree(73));
        app.set_focus_target(Some(row73));
        // The typed fixture supplies a new data projection; this is not product filter UI.
        app.demo.tree_entries.retain(|entry| entry.id != 73);
        let live = app
            .demo
            .tree_entries
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>();
        assert!(app.demo.tree_selection.reconcile(&live));
        app.demo.tree_expansion.reconcile(&app.demo.tree_entries);
        let filtered = tree_selection_frame(
            &mut app,
            width,
            "filter_removed_owner",
            None,
            &[],
            &[100, 10, 41, 8, 900],
        );
        assert!(filtered.find(row73).is_none());
        assert!(!app.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowUp)));
        app.apply_demo_action(DemoAction::SelectVirtualTree(41));
        app.set_focus_target(Some(row41));
        let entries = app.demo.tree_entries.clone();
        app.demo.tree_entries = [100, 8, 10, 41, 900]
            .into_iter()
            .map(|id| {
                entries
                    .iter()
                    .find(|entry| entry.id == id)
                    .expect("stable reordered fixture ID")
                    .clone()
            })
            .collect();
        let live = app
            .demo
            .tree_entries
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>();
        app.demo.tree_selection.reconcile(&live);
        tree_selection_frame(
            &mut app,
            width,
            "reorder_survivor",
            Some(41),
            &[41],
            &[100, 8, 10, 41, 900],
        );
        tree_selection_key(&mut app, ModifiersState::CONTROL, NamedKey::ArrowUp);
        tree_selection_frame(
            &mut app,
            width,
            "reordered_ctrl_focus",
            Some(10),
            &[41],
            &[100, 8, 10, 41, 900],
        );
        tree_selection_key(&mut app, ModifiersState::SHIFT, NamedKey::ArrowUp);
        tree_selection_frame(
            &mut app,
            width,
            "reordered_range",
            Some(8),
            &[8, 10, 41],
            &[100, 8, 10, 41, 900],
        );
    }
}

fn tree_selection_key(app: &mut App, modifiers: ModifiersState, key: NamedKey) {
    app.text_bridge.update_modifiers(&modifiers.into());
    assert!(app.handle_window_key(STANDALONE_WINDOW, &Key::Named(key)));
}

fn tree_selection_frame(
    app: &mut App,
    width: f32,
    phase: &str,
    focus: Option<usize>,
    selected: &[usize],
    visible: &[usize],
) -> Element {
    let root = install_action_route_with_viewport(app, Size::new(width, 800.0));
    let layout = LayoutEngine
        .compute(&root, Viewport::new(width, 800.0, 1.0))
        .expect("hierarchy Gallery layout");
    app.demo.tree_scroll.sync_layout(
        layout
            .node(app.scopes.gallery.element(TREE_SCROLL))
            .expect("hierarchy viewport"),
    );
    FocusScope::new(STANDALONE_WINDOW, &app.ui_actions, None).reconcile(&mut app.input_ownership);
    assert_eq!(
        app.window_focus(STANDALONE_WINDOW),
        focus.map(|id| app.scopes.gallery.element(TREE_ROW_BASE + id as u32)),
        "{phase} focus"
    );
    let selected_ids = app
        .demo
        .tree_selection
        .selected_ids()
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(selected_ids, selected, "{phase} stable selection");
    let tree = VisibleTree::build(&app.demo.tree_entries, &app.demo.tree_expansion)
        .expect("valid fixture hierarchy");
    assert_eq!(
        (0..tree.len())
            .map(|index| tree.entry(index).expect("visible row").id)
            .collect::<Vec<_>>(),
        visible,
        "{phase} visible order"
    );
    for id in visible {
        assert_eq!(
            root.find(app.scopes.gallery.element(TREE_ROW_BASE + *id as u32))
                .expect("bounded fixture row")
                .semantic
                .as_ref()
                .expect("tree semantic")
                .selected,
            Some(selected.contains(id)),
            "{phase} selected semantic"
        );
    }
    println!(
        "GPV02-TREE-STATE width={width} phase={phase} focus={focus:?} selection={selected_ids:?} visible={visible:?}"
    );
    root
}

fn fixed_collection_paging_uses_the_current_visible_rows() -> Vec<String> {
    let mut failures = Vec::new();
    for tree in [false, true] {
        for upwards in [false, true] {
            for partial in [0.0, 14.0] {
                let mut app = App::new();
                activate_gallery_page(&mut app, "gallery.collections");
                let viewport_id = app.scopes.gallery.element(if tree {
                    TREE_SCROLL
                } else {
                    TABLE_VIRTUAL_ROOT + 2
                });
                let count = if tree {
                    app.demo.tree_entries.len()
                } else {
                    TABLE_VIRTUAL_COUNT
                };
                let root = install_action_route(&mut app);
                let initial = LayoutEngine
                    .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
                    .expect("initial fixed layout");
                let node = initial.node(viewport_id).expect("fixed viewport");
                let state = if tree {
                    &mut app.demo.tree_scroll
                } else {
                    &mut app.demo.virtual_table_scroll
                };
                state.sync_layout(node);
                if upwards {
                    state.scroll_by(count as f32 * VIRTUAL_ROW_HEIGHT);
                }
                state.scroll_by(if upwards { -partial } else { partial });
                let root = install_action_route(&mut app);
                let layout = LayoutEngine
                    .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
                    .expect("current fixed layout");
                let viewport = layout
                    .node(viewport_id)
                    .expect("fixed viewport")
                    .scroll_viewport();
                let row = |index: usize| {
                    app.scopes.gallery.element(if tree {
                        TREE_ROW_BASE + index as u32
                    } else {
                        TABLE_VIRTUAL_ROOT + 1000 + index as u32 * 4
                    })
                };
                let visible = (0..count)
                    .filter_map(|index| layout.node(row(index)))
                    .filter(|node| {
                        node.rect.y < viewport.bottom() && node.rect.bottom() > viewport.y
                    })
                    .count();
                assert!(visible > 0);
                let start = if upwards { count - 1 } else { 0 };
                let target = row(start);
                let expected = if upwards {
                    start.saturating_sub(visible)
                } else {
                    visible
                };
                app.set_focus_target(Some(target));
                assert!(app.handle_window_key(
                    STANDALONE_WINDOW,
                    &Key::Named(if upwards {
                        NamedKey::PageUp
                    } else {
                        NamedKey::PageDown
                    })
                ));
                install_action_route(&mut app);
                let actual = if tree {
                    app.focused_tree_index()
                } else {
                    app.focused_virtual_table_index()
                };
                println!(
                    "GPV02-FIXED-PAGE tree={tree} up={upwards} partial={partial} visible={visible} expected={expected} actual={actual:?}"
                );
                if actual != Some(expected) {
                    failures.push(format!("tree={tree} up={upwards} partial={partial}"));
                }
            }
        }
    }
    failures
}

fn collection_paging_uses_the_current_visible_rows() -> Vec<String> {
    let mut failures = Vec::new();
    for variable in [false, true] {
        for reversed in [false, true] {
            for upwards in [false, true] {
                for partial in [0.0, 14.0] {
                    for modifiers in [
                        ModifiersState::empty(),
                        ModifiersState::SHIFT,
                        ModifiersState::CONTROL,
                        ModifiersState::CONTROL | ModifiersState::SHIFT,
                    ] {
                        let mut app = App::new();
                        let item = activate_gallery_page(&mut app, "gallery.collections");
                        if variable {
                            app.apply_collection_data(CollectionDataAction::ToggleRowHeights);
                        }
                        if reversed {
                            app.apply_collection_data(CollectionDataAction::Reverse);
                        }
                        let root = install_action_route(&mut app);
                        let initial = LayoutEngine
                            .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
                            .expect("initial paging layout");
                        let viewport_id = app.scopes.gallery.element(VIRTUAL_SCROLL);
                        app.demo
                            .virtual_scroll
                            .sync_layout(initial.node(viewport_id).expect("list viewport"));
                        if upwards {
                            app.demo
                                .virtual_scroll
                                .scroll_by(app.demo.virtual_content_height());
                        }
                        app.demo
                            .virtual_scroll
                            .scroll_by(if upwards { -partial } else { partial });
                        let root = install_action_route(&mut app);
                        let layout = LayoutEngine
                            .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
                            .expect("current paging layout");
                        capture_scroll_geometry(&mut app, &layout, &[item]);
                        let viewport = layout
                            .node(viewport_id)
                            .expect("live viewport")
                            .scroll_viewport();
                        let visible = app
                            .demo
                            .virtual_ids
                            .iter()
                            .filter_map(|id| {
                                layout
                                    .node(app.scopes.gallery.element(VIRTUAL_ROW_BASE + *id as u32))
                            })
                            .filter(|node| {
                                node.rect.y < viewport.bottom() && node.rect.bottom() > viewport.y
                            })
                            .count();
                        assert!(visible > 0);
                        let start_index = if upwards {
                            app.demo.virtual_ids.len() - 1
                        } else {
                            0
                        };
                        let start = app.demo.virtual_ids[start_index];
                        let expected_index = if upwards {
                            start_index.saturating_sub(visible)
                        } else {
                            visible
                        };
                        let expected_id = app.demo.virtual_ids[expected_index];
                        let additive_range = modifiers.control_key() && modifiers.shift_key();
                        let extra = app.demo.virtual_ids[10];
                        if additive_range {
                            app.apply_demo_action(DemoAction::SelectVirtual(extra));
                            app.text_bridge
                                .update_modifiers(&ModifiersState::CONTROL.into());
                        }
                        app.apply_demo_action(DemoAction::SelectVirtual(start));
                        app.set_focus_target(Some(
                            app.scopes.gallery.element(VIRTUAL_ROW_BASE + start as u32),
                        ));
                        app.text_bridge.update_modifiers(&modifiers.into());
                        let key = Key::Named(if upwards {
                            NamedKey::PageUp
                        } else {
                            NamedKey::PageDown
                        });
                        assert!(app.handle_window_key(STANDALONE_WINDOW, &key));
                        install_action_route(&mut app);
                        let actual_index = app.focused_virtual_index();
                        let actual_selection = app
                            .demo
                            .virtual_selection
                            .selected_ids()
                            .copied()
                            .collect::<std::collections::BTreeSet<_>>();
                        let mut expected_selection = if modifiers.shift_key() {
                            app.demo.virtual_ids
                                [start_index.min(expected_index)..=start_index.max(expected_index)]
                                .iter()
                                .copied()
                                .collect::<std::collections::BTreeSet<_>>()
                        } else {
                            std::collections::BTreeSet::from([if modifiers.control_key() {
                                start
                            } else {
                                expected_id
                            }])
                        };
                        if additive_range {
                            expected_selection.insert(extra);
                        }
                        println!(
                            "GPV02-PAGE variable={variable} reversed={reversed} up={upwards} partial={partial} modifiers={modifiers:?} visible={visible} expected_index={expected_index} actual_index={actual_index:?} expected_selection={} actual_selection={}",
                            expected_selection.len(),
                            actual_selection.len()
                        );
                        if actual_index != Some(expected_index)
                            || actual_selection != expected_selection
                        {
                            failures.push(format!("variable={variable} reversed={reversed} up={upwards} partial={partial} modifiers={modifiers:?}"));
                        }
                    }
                }
            }
        }
    }
    failures
}
