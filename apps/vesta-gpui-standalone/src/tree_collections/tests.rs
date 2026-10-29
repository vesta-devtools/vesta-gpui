use super::*;
use crate::tests::{activate_gallery_page, install_action_route};
use winit::keyboard::ModifiersState;

#[test]
fn tree_refresh_reconciles_subtrees_selection_scroll_and_owned_focus() {
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
        label: format!("Item {id}"),
        depth,
        has_children,
    })
    .collect();
    app.demo.tree_expansion = TreeExpansionState::default();
    app.demo.tree_expansion.toggle(100);
    app.demo.tree_expansion.toggle(10);
    app.demo.tree_selection = ListSelectionState::single(41);
    install_action_route(&mut app);
    for width in [180.0, 320.0, 780.0] {
        for theme in [WidgetTheme::light(), WidgetTheme::dark()] {
            app.theme = theme;
            let root = install_action_route(&mut app);
            let mut controls = root
                .find(app.scopes.gallery.element(26_000_000))
                .expect("tree commands")
                .clone();
            controls.style.width = Dimension::Px(width);
            let layout = vesta_gpui::LayoutEngine
                .compute(&controls, Viewport::new(width, 500.0, 1.25))
                .expect("command reflow");
            let hits = HitTestTree::build(&controls, &layout);
            for index in 0..6 {
                let id = app.scopes.gallery.element(26_000_001 + index * 2);
                let node = layout.node(id).expect("command layout");
                assert!(
                    node.rect.x >= 0.0 && node.rect.right() <= width + 0.01,
                    "command clipped after wrap"
                );
                let point = Point::new(
                    node.rect.x + node.rect.width * 0.5,
                    node.rect.y + node.rect.height * 0.5,
                );
                let hit = hits.hit(point).expect("command hit");
                assert_eq!(
                    app.ui_actions.action_for_hit(hit),
                    app.ui_actions.action_at(id),
                    "reflow mapped to another command"
                );
            }
        }
    }
    let gallery = app.scopes.gallery;
    let row = |id| gallery.element(TREE_ROW_BASE + id);
    let child = row(41);
    let parent = row(10);
    app.set_focus_target(Some(child));
    app.demo
        .tree_scroll
        .sync_content_height(20_000.0, TREE_VIEWPORT_HEIGHT);
    app.demo.tree_scroll.scroll_by(10_000.0);
    app.apply_demo_action(DemoAction::TreeData(TreeDataAction::CollapseAll));
    assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(row(100)));
    assert!(
        app.demo.tree_selection.contains(&41),
        "collapse discarded hidden selection"
    );
    assert_eq!(
        app.demo.tree_scroll.offset_y(),
        0.0,
        "collapse retained obsolete scroll bounds"
    );
    app.apply_demo_action(DemoAction::SelectVirtualTree(73));
    assert!(app.demo.tree_selection.contains(&41) && !app.demo.tree_selection.contains(&73));
    app.apply_demo_action(DemoAction::TreeData(TreeDataAction::ExpandAll));
    install_action_route(&mut app);
    app.set_focus_target(Some(child));
    app.apply_demo_action(DemoAction::TreeData(TreeDataAction::Reverse));
    assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(child));
    assert_eq!(
        app.demo
            .tree_entries
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        [900, 100, 8, 10, 73, 41]
    );
    app.apply_demo_action(DemoAction::TreeData(TreeDataAction::RemoveSelected));
    assert_eq!(
        app.window_focus(STANDALONE_WINDOW),
        Some(parent),
        "deleted leaf should prefer surviving parent"
    );
    assert!(!app.demo.tree_selection.contains(&41));
    install_action_route(&mut app);
    app.demo.tree_selection = ListSelectionState::single(10);
    app.set_focus_target(Some(parent));
    app.apply_demo_action(DemoAction::TreeData(TreeDataAction::RemoveSelected));
    assert!(
        !app.demo
            .tree_entries
            .iter()
            .any(|entry| matches!(entry.id, 10 | 73))
    );
    assert!(!app.demo.tree_expansion.contains(&10));
    install_action_route(&mut app);
    let unrelated = app.scopes.gallery.element(26_000_001);
    app.set_focus_target(Some(unrelated));
    app.apply_demo_action(DemoAction::TreeData(TreeDataAction::Restore));
    assert_eq!(
        app.window_focus(STANDALONE_WINDOW),
        Some(unrelated),
        "tree refresh stole another control's focus"
    );
    assert_eq!(app.demo.tree_entries.len(), 1001);
    app.apply_demo_action(DemoAction::TreeData(TreeDataAction::ExpandAll));
    app.demo.tree_selection = ListSelectionState::single(0);
    install_action_route(&mut app);
    app.restore_after_rebuild(STANDALONE_WINDOW, Some(row(1000)));
    let projected = install_action_route(&mut app);
    assert!(
        projected.find(row(1000)).is_some(),
        "pending tree focus was omitted from bounded projection"
    );
    app.set_focus_target(Some(row(1000)));
    app.apply_demo_action(DemoAction::TreeData(TreeDataAction::RemoveSelected));
    assert!(app.demo.tree_entries.is_empty());
    assert_eq!(app.window_focus(STANDALONE_WINDOW), None);
    assert_eq!(app.demo.tree_scroll.offset_y(), 0.0);
    app.apply_demo_action(DemoAction::TreeData(TreeDataAction::Restore));
    app.apply_demo_action(DemoAction::TreeData(TreeDataAction::ExpandAll));
    install_action_route(&mut app);
    app.set_focus_target(Some(row(1)));
    for modifiers in [ModifiersState::ALT, ModifiersState::SUPER] {
        app.text_bridge.update_modifiers(&modifiers.into());
        assert!(
            !app.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowDown))
        );
        assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(row(1)));
    }
    app.text_bridge
        .update_modifiers(&ModifiersState::empty().into());
    app.demo.tree_selection = ListSelectionState::single(1000);
    app.apply_demo_action(DemoAction::TreeData(TreeDataAction::CollapseAll));
    install_action_route(&mut app);
    app.apply_demo_action(DemoAction::TreeData(TreeDataAction::RevealSelected));
    assert_eq!(
        app.input_ownership
            .pending_focus_target(STANDALONE_WINDOW)
            .map(|target| target.element),
        Some(row(1000))
    );
    let revealed = install_action_route(&mut app);
    assert!(
        revealed.find(row(1000)).is_some(),
        "reveal did not rebuild selected path"
    );
    assert!(app.demo.tree_expansion.contains(&0));
    assert!(app.demo.tree_selection.contains(&1000));
    assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(row(1000)));
}
