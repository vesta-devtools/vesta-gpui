//! New modal/palette authority must beat an older completed Dock frame.
use super::*;

pub(super) fn current_scope_rejects_cached_workspace_targets() {
    let mut failures = Vec::new();
    for main_is_source in [false, true] {
        for palette in [false, true] {
            let mut app = App::new();
            let item = app.demo.workspace.active_item().expect("source item").id;
            let main_pane = app.demo.workspace.active_pane();
            let floated = if main_is_source {
                app.demo
                    .workspace
                    .pane(main_pane)
                    .expect("main pane")
                    .items
                    .iter()
                    .find(|candidate| candidate.id != item)
                    .expect("destination item")
                    .id
            } else {
                item
            };
            app.demo
                .workspace
                .dock_item(
                    floated,
                    DockAction::Float {
                        placement: WindowPlacement::new(500.0, 0.0, 400.0, 300.0),
                    },
                )
                .expect("real Core Float");
            let floating = app
                .demo
                .workspace
                .floating_windows()
                .first()
                .expect("Float");
            let floating_container = ContainerRef::Floating(floating.id);
            let floating_pane = floating.root.panes()[0];
            let (source, destination, pane) = if main_is_source {
                (ContainerRef::Main, floating_container, floating_pane)
            } else {
                (floating_container, ContainerRef::Main, main_pane)
            };
            app.demo.sync_workspace_ui();
            install_action_route(&mut app);
            let cached = WorkspaceTabDrag {
                item,
                origin_container: source,
                origin: Point::new(20.0, 20.0),
                started: true,
                pointer_container: Some(destination),
                cursor: Point::new(200.0, 150.0),
                target: Some(DockDropTarget {
                    action: DockAction::Merge { pane, slot: 0 },
                    preview: Rect::new(0.0, 32.0, 400.0, 268.0),
                    insertion: None,
                }),
            };
            app.workspace_drag = Some(cached);
            app.action_window = STANDALONE_WINDOW;
            app.apply_demo_action(if palette {
                DemoAction::OpenCommandPalette
            } else {
                DemoAction::OpenDemoDialog
            });
            assert!(app.focus_root_for_window(STANDALONE_WINDOW).is_some());
            let tag = format!("main_is_source={main_is_source},palette={palette}");
            if app.workspace_drag.is_some_and(|drag| drag.target.is_some()) {
                failures.push(format!("{tag}: cached preview survives new scope"));
            }
            let before = app.demo.workspace.clone();
            let admitted = app.dock_window_admitted(STANDALONE_WINDOW, true);
            if admitted {
                failures.push(format!("{tag}: current modal/palette admitted"));
            }
            let frame = |pane, container| DockFrame {
                panes: vec![DockPaneFrame {
                    flow_direction: vesta_gpui::FlowDirection::LeftToRight,
                    pane,
                    container,
                    root: Rect::new(0.0, 0.0, 400.0, 300.0),
                    tab_strip: Rect::new(0.0, 0.0, 400.0, 32.0),
                    body: Rect::new(0.0, 32.0, 400.0, 268.0),
                }],
                workspace: Some(Rect::new(0.0, 0.0, 400.0, 300.0)),
                ..DockFrame::default()
            };
            let main_frame = frame(main_pane, ContainerRef::Main);
            let float_frame = frame(floating_pane, floating_container);
            let main_geometry = DockWindowGeometry {
                container: ContainerRef::Main,
                outer: Rect::new(0.0, 0.0, 400.0, 300.0),
                client: Rect::new(0.0, 0.0, 400.0, 300.0),
                scale: 1.0,
            };
            let float_geometry = DockWindowGeometry {
                container: floating_container,
                outer: Rect::new(500.0, 0.0, 400.0, 300.0),
                client: Rect::new(500.0, 0.0, 400.0, 300.0),
                scale: 1.0,
            };
            let (source_geometry, pointer) = if main_is_source {
                (main_geometry, Point::new(700.0, 150.0))
            } else {
                (float_geometry, Point::new(-300.0, 150.0))
            };
            let route = vesta_gpui_workspace::route_dock_pointer_with_admission(
                source_geometry,
                pointer,
                &[(main_geometry, &main_frame), (float_geometry, &float_frame)],
                &app.demo.workspace,
                |container| container != ContainerRef::Main || admitted,
            );
            if route.is_some() {
                failures.push(format!("{tag}: old frame routed through new scope"));
            }
            if let Some((container, cursor, target)) = route {
                app.update_workspace_drag(container, cursor, target);
            } else {
                app.leave_workspace_drag(destination);
            }
            app.finish_workspace_drag_with_admission(source, |container| {
                container != ContainerRef::Main || admitted
            });
            if app.demo.workspace != before {
                failures.push(format!("{tag}: late release mutated Core"));
            }
            assert!(app.workspace_drag.is_none());
            assert!(
                !app.finish_workspace_drag_with_admission(source, |_| panic!(
                    "release consumed once"
                ))
            );
            app.apply_demo_action(if palette {
                DemoAction::OpenCommandPalette
            } else {
                DemoAction::CancelDialog(1)
            });
            if palette {
                app.close_command_palette();
            }
            assert!(app.focus_root_for_window(STANDALONE_WINDOW).is_none());
            assert!(
                !app.finish_workspace_drag_with_admission(source, |_| panic!(
                    "closing scope cannot revive gesture"
                ))
            );
            // A newly begun gesture still consumes the same ordinary Core path.
            app.demo.workspace = before;
            app.demo.sync_workspace_ui();
            install_action_route(&mut app);
            assert!(app.dock_window_admitted(STANDALONE_WINDOW, true));
            assert!(!app.dock_window_admitted(STANDALONE_WINDOW, false));
            app.workspace_drag = Some(cached);
            assert!(app.finish_workspace_drag_with_admission(source, |_| true));
            assert_eq!(
                app.demo.workspace.container_of_item(item),
                Some(destination)
            );
            assert!(app.workspace_drag.is_none());
        }
    }
    assert!(
        failures.is_empty(),
        "stale Dock scope admission: {failures:?}"
    );
}
