//! Actual198: a moved-away destination accepted a cached split on real release.
use super::*;
use winit::dpi::{PhysicalPosition, PhysicalSize};

pub(super) fn changed_window_retires_only_its_dock_coordinates() {
    splitter_geometry_restores_owned_preview();
    for event in [
        WindowEvent::Moved(PhysicalPosition::new(1500, 100)),
        WindowEvent::Resized(PhysicalSize::new(650, 813)),
        WindowEvent::ScaleFactorChanged { scale_factor: 1.5 },
    ] {
        for floating_source in [false, true] {
            for changed in 0..3 {
                let mut app = App::new();
                let main_item = app.demo.workspace.pane(PaneId(1)).expect("Main").items[0].id;
                let layout = app
                    .demo
                    .workspace
                    .panes()
                    .into_iter()
                    .flat_map(|pane| pane.items.iter())
                    .find(|item| item.panel_type.as_str() == "gallery.layout")
                    .expect("Layout")
                    .id;
                let collection = app.demo.workspace.pane(PaneId(2)).expect("other").items[0].id;
                for item in [layout, collection] {
                    app.demo
                        .workspace
                        .dock_item(
                            item,
                            DockAction::Float {
                                placement: WindowPlacement::new(560.0, 80.0, 520.0, 650.4),
                            },
                        )
                        .expect("real Float containers");
                }
                let floating = app.demo.workspace.container_of_item(layout).expect("Float");
                let unrelated = app
                    .demo
                    .workspace
                    .container_of_item(collection)
                    .expect("unrelated Float");
                let (item, origin, destination) = if floating_source {
                    (layout, floating, ContainerRef::Main)
                } else {
                    (main_item, ContainerRef::Main, floating)
                };
                let pane = container_panes(&app.demo.workspace, destination)
                    .into_iter()
                    .find(|pane| {
                        app.demo.workspace.pane(*pane).is_some_and(|pane| {
                            pane.items.iter().any(|item| {
                                item.id == if floating_source { main_item } else { layout }
                            })
                        })
                    })
                    .expect("current destination pane");
                let target = DockDropTarget {
                    action: DockAction::Split {
                        pane,
                        side: Side::Right,
                    },
                    preview: Rect::new(300.0, 32.0, 220.0, 600.0),
                    insertion: None,
                };
                app.workspace_drag = Some(WorkspaceTabDrag {
                    item,
                    origin_container: origin,
                    origin: Point::new(91.0, 48.0),
                    started: true,
                    pointer_container: Some(destination),
                    cursor: Point::new(480.0, 260.0),
                    target: Some(target),
                });
                let original_drag = format!("{:?}", app.workspace_drag);
                let before = app.demo.workspace.clone();
                // Unrelated notifications cannot consume an owning transaction.
                app.observe_workspace_geometry(unrelated, &WindowEvent::RedrawRequested);
                assert_eq!(format!("{:?}", app.workspace_drag), original_drag);
                let affected = [origin, destination, unrelated][changed];
                app.observe_workspace_geometry(affected, &event);
                assert_eq!(
                    app.demo.workspace, before,
                    "retirement does not author layout"
                );
                if changed == 0 {
                    assert!(
                        app.workspace_drag.is_none(),
                        "changed origin retires gesture"
                    );
                } else if changed == 1 {
                    let retained = app.workspace_drag.expect("foreign source stays live");
                    assert_eq!(retained.item, item);
                    assert_eq!(retained.origin_container, origin);
                    assert!(retained.pointer_container.is_none() && retained.target.is_none());
                    assert!(app.dock_drag_visual(destination).is_none());
                } else {
                    assert_eq!(format!("{:?}", app.workspace_drag), original_drag);
                }
                app.finish_workspace_drag_with_admission(origin, |_| true);
                if changed < 2 {
                    assert_eq!(app.demo.workspace, before, "old release cannot dock");
                    assert!(app.dock_focus.is_none());
                } else {
                    assert_eq!(
                        app.demo.workspace.container_of_item(item),
                        Some(destination)
                    );
                }
                if changed == 2 {
                    let focus = app.dock_focus.expect("accepted drop requests focus");
                    app.observe_workspace_geometry(destination, &event);
                    assert_eq!(
                        app.dock_focus,
                        Some(focus),
                        "normal creation geometry preserves accepted-drop focus"
                    );
                }
                assert!(app.workspace_drag.is_none());
                assert!(!app.finish_workspace_drag_with_admission(origin, |_| true));
                if changed == 1 {
                    // A genuinely fresh pointer resolves a new current target normally.
                    vesta_gpui_workspace::begin_tab_drag(
                        &mut app.workspace_drag,
                        item,
                        origin,
                        Point::new(91.0, 48.0),
                    );
                    app.update_workspace_drag(destination, Point::new(480.0, 260.0), Some(target));
                    assert!(app.finish_workspace_drag_with_admission(origin, |_| true));
                    assert_eq!(
                        app.demo.workspace.container_of_item(item),
                        Some(destination)
                    );
                    assert!(!app.finish_workspace_drag_with_admission(origin, |_| true));
                }
            }
        }
    }
}

fn splitter_geometry_restores_owned_preview() {
    for event in [
        WindowEvent::ScaleFactorChanged { scale_factor: 1.5 },
        WindowEvent::Moved(PhysicalPosition::new(1500, 100)),
        WindowEvent::Resized(PhysicalSize::new(650, 813)),
    ] {
        for floating_source in [false, true] {
            let mut app = App::new();
            let (container, owner, unrelated) = if floating_source {
                let items = ["gallery.controls", "gallery.inputs"].map(|kind| {
                    app.demo
                        .workspace
                        .panes()
                        .into_iter()
                        .flat_map(|pane| pane.items.iter())
                        .find(|item| item.panel_type.as_str() == kind)
                        .expect("initial panel")
                        .id
                });
                app.demo
                    .workspace
                    .dock_item(
                        items[0],
                        DockAction::Float {
                            placement: WindowPlacement::new(560.0, 80.0, 520.0, 650.4),
                        },
                    )
                    .expect("source Float");
                let window = app.demo.workspace.floating_windows()[0].id;
                let pane = app
                    .demo
                    .workspace
                    .floating_window(window)
                    .expect("Float")
                    .root
                    .panes()[0];
                app.demo
                    .workspace
                    .dock_item(
                        items[1],
                        DockAction::Split {
                            pane,
                            side: Side::Right,
                        },
                    )
                    .expect("real split in Float");
                (
                    ContainerRef::Floating(window),
                    UiWindowId(2),
                    ContainerRef::Main,
                )
            } else {
                let layout = app
                    .demo
                    .workspace
                    .panes()
                    .into_iter()
                    .flat_map(|pane| pane.items.iter())
                    .find(|item| item.panel_type.as_str() == "gallery.layout")
                    .expect("unrelated panel")
                    .id;
                app.demo
                    .workspace
                    .dock_item(
                        layout,
                        DockAction::Float {
                            placement: WindowPlacement::new(560.0, 80.0, 520.0, 650.4),
                        },
                    )
                    .expect("unrelated Float");
                (
                    ContainerRef::Main,
                    STANDALONE_WINDOW,
                    app.demo.workspace.container_of_item(layout).expect("Float"),
                )
            };
            let root = match container {
                ContainerRef::Main => app.demo.workspace.root(),
                ContainerRef::Floating(window) => {
                    &app.demo
                        .workspace
                        .floating_window(window)
                        .expect("Float")
                        .root
                }
            };
            let WorkspaceNode::Split { id, axis, .. } = root else {
                panic!("fixture requires the real source split");
            };
            let (split, axis) = (*id, *axis);
            let original = dock_split_ratio(&app.demo.workspace, container, split).expect("ratio");
            let before = app.demo.workspace.clone();
            let target = InputTarget {
                window: owner,
                element: ElementId::new(88),
            };
            app.workspace_resize = Some(
                vesta_gpui_workspace::SplitResizeSession::new(
                    target,
                    vesta_gpui_workspace::DockSplitFrame {
                        container,
                        split,
                        axis,
                        rect: Rect::new(0.0, 0.0, 800.0, 400.0),
                        flow_direction: vesta_gpui::FlowDirection::LeftToRight,
                    },
                    original,
                )
                .expect("owned splitter session"),
            );
            if owner == STANDALONE_WINDOW {
                app.input_ownership
                    .capture_pointer(target, PointerButton::Primary);
            }
            assert!(app.update_workspace_resize(owner, Point::new(760.0, 360.0)));
            assert_ne!(
                dock_split_ratio(&app.demo.workspace, container, split),
                Some(original)
            );
            let active = app.workspace_resize;
            app.observe_workspace_geometry(unrelated, &event);
            app.observe_workspace_geometry(container, &WindowEvent::RedrawRequested);
            assert_eq!(
                app.workspace_resize, active,
                "foreign/nongeometry input preserves preview"
            );
            app.observe_workspace_geometry(container, &event);
            assert!(
                app.workspace_resize.is_none(),
                "source geometry retires old splitter bounds: event={event:?} container={container:?} original={original} current={:?}",
                dock_split_ratio(&app.demo.workspace, container, split)
            );
            assert_eq!(
                app.demo.workspace, before,
                "source loss restores exact Core split ratio"
            );
            if owner == STANDALONE_WINDOW {
                assert!(app.input_ownership.capture().is_none());
            }
            assert!(!app.finish_workspace_resize_for(owner, true));
            assert_eq!(
                app.demo.workspace, before,
                "late release cannot commit an old preview"
            );
        }
    }
}
