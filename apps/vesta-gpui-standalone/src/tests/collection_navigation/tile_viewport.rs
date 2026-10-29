//! Current Tile focus must remain completely within its inherited clip.
use super::*;

pub(super) fn qualify() {
    let mut failures = Vec::new();
    for flow in [
        vesta_gpui::FlowDirection::LeftToRight,
        vesta_gpui::FlowDirection::RightToLeft,
    ] {
        for width in [1280.0, 720.0, 360.0] {
            let mut app = App::new();
            activate_gallery_page(&mut app, "gallery.collections");
            let size = Size::new(width, 640.0);
            table_viewport::project(&mut app, size, flow);
            table_viewport::project(&mut app, size, flow);
            app.set_focus_target(Some(app.scopes.gallery.element(TILE_ROOT + 100)));
            assert!(app.handle_window_key(STANDALONE_WINDOW, &Key::Named(NamedKey::End)));
            table_viewport::project(&mut app, size, flow);
            let layout = table_viewport::project(&mut app, size, flow);
            let target = app.scopes.gallery.element(TILE_ROOT + 116);
            assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(target));
            assert_eq!(app.demo.tile_selected, 8);
            let row = layout.node(target).expect("focused tile");
            let visible = row.rect.intersection(row.inherited_clip);
            let point = Point::new(
                if flow == vesta_gpui::FlowDirection::LeftToRight {
                    row.rect.right() - 0.25
                } else {
                    row.rect.x + 0.25
                },
                row.rect.y + row.rect.height * 0.5,
            );
            let hit = app.hit_test.hit(point);
            let admitted = hit.is_some_and(|hit| app.ui_actions.is_descendant_of(hit, target));
            println!(
                "GPV04-TILE-EDGE flow={flow:?} width={width} row={:?} visible={visible:?} admitted={admitted}",
                row.rect
            );
            if visible != Some(row.rect) || !admitted {
                for element in app.ui_actions.path_to(target).expect("current Tile path") {
                    if let Some(node) = layout.node(element) {
                        println!(
                            "GPV04-TILE-ANCESTOR id={element:?} rect={:?} clip={:?} viewport={:?}",
                            node.rect,
                            node.inherited_clip,
                            node.scroll_viewport()
                        );
                    }
                }
                failures.push(format!("{flow:?}/{width}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "current Tile edge mismatch: {failures:?}"
    );
    feature_availability_retires_tile_ownership();
}

fn feature_availability_retires_tile_ownership() {
    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.collections");
    let size = Size::new(720.0, 640.0);
    let flow = vesta_gpui::FlowDirection::LeftToRight;
    table_viewport::project(&mut app, size, flow);
    table_viewport::project(&mut app, size, flow);
    app.set_focus_target(Some(app.scopes.gallery.element(TILE_ROOT + 100)));
    assert!(app.handle_window_key(STANDALONE_WINDOW, &Key::Named(NamedKey::End)));
    table_viewport::project(&mut app, size, flow);
    let layout = table_viewport::project(&mut app, size, flow);
    let owner = InputTarget {
        window: STANDALONE_WINDOW,
        element: app.scopes.gallery.element(TILE_ROOT + 116),
    };
    let child = app.scopes.gallery.element(TILE_ROOT + 1_068);
    let row = layout
        .node(owner.element)
        .expect("current selected tile")
        .rect;
    let point = Point::new(row.x + row.width * 0.5, row.y + row.height * 0.5);
    let initial = (
        app.demo.tile_selected,
        app.demo.number,
        app.demo.vector,
        app.demo.table_note.clone(),
    );
    assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(owner.element));
    app.input_ownership
        .capture_pointer(owner, PointerButton::Primary);
    app.apply_window_action(STANDALONE_WINDOW, DemoAction::ToggleSwitch);
    let disabled = table_viewport::project(&mut app, size, flow);
    let _ = FocusScope::new(STANDALONE_WINDOW, &app.ui_actions, None)
        .reconcile(&mut app.input_ownership);
    assert!(
        app.window_focus(STANDALONE_WINDOW).is_none(),
        "disabled Tile owner must retire"
    );
    assert!(app.input_ownership.capture().is_none());
    assert_eq!(
        disabled
            .node(owner.element)
            .expect("disabled stays laid out")
            .rect,
        row
    );
    assert_eq!(
        app.ui_actions.action_at(child),
        None,
        "disabled child action must be absent"
    );
    assert!(
        app.hit_test
            .hit(point)
            .and_then(|hit| app.ui_actions.action_for_hit(hit))
            .is_none()
    );
    assert!(!app.handle_window_key(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowUp)));

    let root = crate::tests::install_action_route_with_viewport(&mut app, size);
    let mut semantics = vesta_gpui::AccessibilityTree::default();
    let update = semantics.build(&root, &disabled, None, "Tile availability");
    let id = semantics
        .node_for_element(child)
        .expect("disabled child semantics");
    assert!(
        update
            .nodes
            .iter()
            .find(|(candidate, _)| *candidate == id)
            .expect("disabled semantic update")
            .1
            .is_disabled()
    );

    app.apply_window_action(STANDALONE_WINDOW, DemoAction::ToggleSwitch);
    table_viewport::project(&mut app, size, flow);
    let _ = FocusScope::new(STANDALONE_WINDOW, &app.ui_actions, None)
        .reconcile(&mut app.input_ownership);
    assert!(
        app.window_focus(STANDALONE_WINDOW).is_none(),
        "enable must not resurrect an old Tile owner"
    );
    assert_eq!(
        app.ui_actions.action_at(child),
        Some(&DemoAction::OpenCommandPalette)
    );
    assert_eq!(
        (
            app.demo.tile_selected,
            app.demo.number,
            app.demo.vector,
            app.demo.table_note
        ),
        initial
    );
}
