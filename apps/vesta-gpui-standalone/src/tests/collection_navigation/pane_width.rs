//! Current-container measurements must not size another window's collections.
use super::*;

fn project(app: &mut App, container: ContainerRef) -> LayoutTree {
    let size = Size::new(960.0, 800.0);
    let mut result = None;
    for _ in 0..3 {
        let (root, regions) =
            crate::tests::install_action_route_for_container(app, size, container);
        let layout = LayoutEngine
            .compute(&root, Viewport::new(size.width, size.height, 1.0))
            .expect("current collection container layout");
        app.demo
            .sync_pane_widths(regions.panes.iter().map(|region| {
                let pane = layout.node(region.root).expect("measured current pane");
                (region.pane, pane.rect.width)
            }));
        result = Some(layout);
    }
    result.expect("settled current collection frame")
}

pub(super) fn qualify() {
    let mut app = App::new();
    let main_item = activate_gallery_page(&mut app, "gallery.collections");
    let main_pane = app.demo.workspace.active_pane();
    let floating_item = app
        .demo
        .workspace
        .add_item(
            main_pane,
            "Floating collections",
            PanelTypeId::new("gallery.collections"),
        )
        .expect("distinct collection item");
    assert!(app.float_workspace_item(
        floating_item,
        WindowPlacement::new(80.0, 40.0, 960.0, 800.0)
    ));
    let (window, pane) = app
        .demo
        .workspace
        .floating_windows()
        .iter()
        .flat_map(|window| {
            window
                .root
                .panes()
                .into_iter()
                .map(move |pane| (window.id, pane))
        })
        .find(|(_, pane)| {
            app.demo
                .workspace
                .pane(*pane)
                .is_some_and(|pane| pane.items.iter().any(|item| item.id == floating_item))
        })
        .expect("floating item source");
    let blank = app
        .demo
        .workspace
        .add_item(pane, "Blank", PanelTypeId::new("gallery.blank"))
        .expect("sibling item");
    app.demo
        .workspace
        .dock_item(
            blank,
            DockAction::Split {
                pane,
                side: Side::Right,
            },
        )
        .expect("split floating source");
    app.demo
        .workspace
        .activate(main_item)
        .expect("Main source remains active");
    app.demo
        .workspace
        .activate(floating_item)
        .expect("Float source remains active");
    let container = ContainerRef::Floating(window);
    let WorkspaceNode::Split { id: split, .. } = app
        .demo
        .workspace
        .floating_window(window)
        .expect("floating model")
        .root
    else {
        panic!("two floating panes")
    };
    app.demo.table_selected = 3;
    app.demo.tile_selected = 4;
    let table = app.scopes.gallery.element(TABLE_ROOT);
    let main_before = project(&mut app, ContainerRef::Main)
        .node(table)
        .expect("Main table")
        .rect;
    app.demo
        .workspace
        .set_split_ratio(container, split, 0.25)
        .expect("narrow source");
    let narrow = project(&mut app, container)
        .node(table)
        .expect("Float table")
        .rect;
    app.demo
        .workspace
        .set_split_ratio(container, split, 0.75)
        .expect("wide source");
    let wide = project(&mut app, container)
        .node(table)
        .expect("same Float table")
        .rect;
    assert!(
        wide.width > narrow.width,
        "Float table must follow its own measured split, not Main or viewport fallback: {narrow:?} -> {wide:?}"
    );
    let main_after = project(&mut app, ContainerRef::Main)
        .node(table)
        .expect("same Main table")
        .rect;
    assert_eq!(
        main_after, main_before,
        "Float geometry cannot resize Main content"
    );
    assert_eq!(app.demo.table_selected, 3);
    assert_eq!(app.demo.tile_selected, 4);
    assert!(app.demo.workspace.item(main_item).is_some());
    assert!(app.demo.workspace.item(floating_item).is_some());
}
