//! A reserved horizontal scrollbar must not hide the keyboard-focused table row.
use super::*;

pub(super) fn project(app: &mut App, size: Size, flow: vesta_gpui::FlowDirection) -> LayoutTree {
    let (mut root, regions) = crate::tests::install_action_route_with_regions(app, size);
    root.style.flow_direction = Some(flow);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(size.width, size.height, 1.0))
        .expect("current table layout");
    let pages = app
        .demo
        .workspace
        .panes()
        .into_iter()
        .filter_map(|pane| pane.active_item().map(|item| item.id))
        .collect::<Vec<_>>();
    crate::tests::capture_scroll_geometry(app, &layout, &pages);
    for item in &pages {
        let viewport = layout
            .node(page_scroll_id(app.scopes.gallery, *item))
            .expect("current page viewport");
        app.demo
            .page_scroll
            .entry(*item)
            .or_default()
            .sync_layout(viewport);
    }
    let body = layout
        .node(app.scopes.gallery.element(TABLE_VIRTUAL_ROOT + 2))
        .expect("table body");
    app.demo.virtual_table_scroll.sync_layout(body);
    for region in regions.panes {
        let pane = layout.node(region.root).expect("current pane");
        app.demo.pane_widths.insert(region.pane, pane.rect.width);
    }
    app.focus_rects = app
        .ui_actions
        .focus_order()
        .iter()
        .filter_map(|element| layout.node(*element).map(|node| (*element, node.rect)))
        .collect();
    app.hit_test = HitTestTree::build(&root, &layout);
    layout
}

pub(super) fn qualify() {
    let mut failures = Vec::new();
    for flow in [
        vesta_gpui::FlowDirection::LeftToRight,
        vesta_gpui::FlowDirection::RightToLeft,
    ] {
        for width in [1280.0, 360.0] {
            let size = Size::new(width, 800.0);
            let mut app = App::new();
            activate_gallery_page(&mut app, "gallery.collections");
            project(&mut app, size, flow);
            let layout = project(&mut app, size, flow);
            let body_id = app.scopes.gallery.element(TABLE_VIRTUAL_ROOT + 2);
            let body = layout.node(body_id).expect("initial table body");
            let allocation = body.rect.height;
            let visible = body.scroll_viewport().height;
            let page_rows = (0..TABLE_VIRTUAL_COUNT)
                .filter_map(|index| {
                    layout.node(
                        app.scopes
                            .gallery
                            .element(TABLE_VIRTUAL_ROOT + 1_000 + index as u32 * 4),
                    )
                })
                .filter(|row| {
                    row.rect.y < body.scroll_viewport().bottom()
                        && row.rect.bottom() > body.scroll_viewport().y
                })
                .count();
            assert!(
                visible < allocation,
                "current horizontal gutter must remain"
            );
            assert!(
                body.scrollbar_geometry(app.demo.virtual_table_scroll.offset())
                    .2
                    .is_some()
            );
            app.set_focus_target(Some(app.scopes.gallery.element(TABLE_VIRTUAL_ROOT + 1_001)));
            for (key, expected) in [
                (NamedKey::End, TABLE_VIRTUAL_COUNT - 1),
                (NamedKey::Home, 0),
                (NamedKey::PageDown, page_rows),
                (NamedKey::PageUp, 0),
            ] {
                assert!(app.handle_window_key(STANDALONE_WINDOW, &Key::Named(key)));
                project(&mut app, size, flow);
                let layout = project(&mut app, size, flow);
                let index = app
                    .focused_virtual_table_index()
                    .expect("current table focus");
                assert_eq!(index, expected);
                assert_eq!(app.demo.virtual_table_selected, expected);
                let body = layout.node(body_id).expect("current table body");
                let view = body.scroll_viewport();
                let row = layout
                    .node(
                        app.scopes
                            .gallery
                            .element(TABLE_VIRTUAL_ROOT + 1_000 + index as u32 * 4),
                    )
                    .expect("bounded focused row");
                assert_eq!(
                    body.rect.height, allocation,
                    "effective height must not shrink allocation"
                );
                assert_eq!(view.height, visible);
                assert!(
                    body.scrollbar_geometry(app.demo.virtual_table_scroll.offset())
                        .2
                        .is_some()
                );
                println!(
                    "GPS03-TABLE-VIEWPORT flow={flow:?} width={width} key={key:?} offset={} allocation={allocation} visible={visible} row_y={} row_h={} view_y={} view_h={}",
                    app.demo.virtual_table_scroll.offset_y(),
                    row.rect.y,
                    row.rect.height,
                    view.y,
                    view.height
                );
                if row.rect.y < view.y - 0.001 || row.rect.bottom() > view.bottom() + 0.001 {
                    failures.push(format!("{flow:?}/{width}/{key:?}"));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "table scrollport reveal mismatch: {failures:?}"
    );
}
