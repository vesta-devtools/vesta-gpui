//! Every existing component must remain editable through the common page reveal.
use super::*;

fn project(app: &mut App, size: Size, flow: FlowDirection, target: ElementId) -> LayoutTree {
    let mut root = install_action_route_with_viewport(app, size);
    root.style.flow_direction = Some(flow);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(size.width, size.height, 1.0))
        .expect("current Vector layout");
    let pages = app
        .demo
        .workspace
        .panes()
        .into_iter()
        .filter_map(|pane| pane.active_item().map(|item| item.id))
        .collect::<Vec<_>>();
    app.scroll_geometry =
        ScrollGeometry::from_layout(app.scopes.gallery, &layout, pages.iter().copied());
    for item in &pages {
        let page = layout
            .node(page_scroll_id(app.scopes.gallery, *item))
            .expect("current Vector page");
        app.demo
            .page_scroll
            .entry(*item)
            .or_default()
            .sync_layout(page);
        app.page_scroll_rects.insert(*item, page.rect);
    }
    app.focus_rects = app
        .ui_actions
        .focus_order()
        .iter()
        .filter_map(|element| layout.node(*element).map(|node| (*element, node.rect)))
        .collect();
    app.hit_test = HitTestTree::build(&root, &layout);
    app.reveal_window_focus(STANDALONE_WINDOW, target);
    layout
}

pub(super) fn qualify() {
    let mut failures = Vec::new();
    for flow in [FlowDirection::LeftToRight, FlowDirection::RightToLeft] {
        for width in [1280.0, 360.0] {
            let mut app = App::new();
            activate_gallery_page(&mut app, "gallery.inputs");
            let size = Size::new(width, 640.0);
            for axis in 0..3 {
                let target = TextFieldKind::Vector(axis).element(app.scopes.gallery);
                app.set_focus_target(Some(target));
                for _ in 0..3 {
                    project(&mut app, size, flow, target);
                }
                let layout = project(&mut app, size, flow, target);
                let node = layout.node(target).expect("stable Vector editor");
                let visible = node.rect.intersection(node.inherited_clip);
                let point = Point::new(
                    node.rect.x + node.rect.width * 0.5,
                    node.rect.y + node.rect.height * 0.5,
                );
                let hit = app.hit_test.hit(point);
                println!(
                    "GPN03-VECTOR-NARROW flow={flow:?} width={width} axis={axis} rect={:?} visible={visible:?} hit={hit:?}",
                    node.rect
                );
                if node.rect.width <= 0.0 || visible != Some(node.rect) || hit != Some(target) {
                    for element in app.ui_actions.path_to(target).expect("current Vector path") {
                        let ancestor = layout.node(element).expect("current ancestor");
                        println!(
                            "GPN03-VECTOR-ANCESTOR id={element:?} rect={:?} clip={:?}",
                            ancestor.rect, ancestor.inherited_clip
                        );
                    }
                    failures.push(format!("{flow:?}/{width}/{axis}"));
                }
                assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(target));
                assert_eq!(app.demo.vector, [Some(1.0), None, Some(3.0)]);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "Vector component must remain editable: {failures:?}"
    );
}
