//! Native104 lost focus after a real cross-HWND drop; protect its portable boundary.
use super::*;

fn tab_target(app: &App, item: ItemId) -> InputTarget {
    InputTarget {
        window: STANDALONE_WINDOW,
        element: app
            .ui_actions
            .focus_order()
            .iter()
            .copied()
            .find(|element| {
                app.ui_actions.action_at(*element)
                    == Some(&DemoAction::Workspace(WorkspaceViewAction::Tab(item)))
            })
            .expect("current tab route"),
    }
}

fn request(app: &mut App, container: ContainerRef, root: Option<ElementId>) -> bool {
    DockFocusTransfer::request(
        &mut app.dock_focus,
        &app.demo.workspace,
        container,
        STANDALONE_WINDOW,
        &app.ui_actions,
        root,
        &mut app.input_ownership,
    )
}

pub(super) fn accepted_drop_uses_only_the_current_destination_route() {
    let mut app = App::new();
    let item = app.demo.workspace.active_item().expect("initial item").id;
    let destination = app.demo.workspace.active_pane();
    app.demo
        .workspace
        .dock_item(
            item,
            DockAction::Float {
                placement: WindowPlacement::new(20.0, 20.0, 360.0, 220.0),
            },
        )
        .expect("source Float");
    let source = app.demo.workspace.container_of_item(item).expect("source");
    app.demo.sync_workspace_ui();
    install_action_route(&mut app);
    app.workspace_drag = Some(WorkspaceTabDrag {
        item,
        origin_container: source,
        origin: Point::new(20.0, 20.0),
        started: true,
        pointer_container: Some(ContainerRef::Main),
        cursor: Point::new(200.0, 40.0),
        target: Some(DockDropTarget {
            action: DockAction::Merge {
                pane: destination,
                slot: 0,
            },
            preview: Rect::new(0.0, 0.0, 100.0, 100.0),
            insertion: None,
        }),
    });
    // Display admission is portable here; native104/its repair run prove the HWND path.
    assert!(app.finish_workspace_drag_with_admission(source, |_| true));
    let accepted = app
        .dock_focus
        .expect("accepted drop retains its stable item intent");
    let after_drop = app.demo.workspace.clone();
    assert!(!app.finish_workspace_drag_with_admission(source, |_| {
        panic!("late release cannot consult or replace the accepted focus intent")
    }));
    assert_eq!(app.dock_focus, Some(accepted));
    assert!(
        !request(&mut app, source, None),
        "another frame cannot consume it"
    );
    assert_eq!(app.dock_focus, Some(accepted));
    install_action_route(&mut app);
    let target = tab_target(&app, item);
    assert!(request(&mut app, ContainerRef::Main, None));
    assert!(app.dock_focus.is_none());
    assert_eq!(
        app.input_ownership.pending_focus_target(STANDALONE_WINDOW),
        Some(target)
    );
    let restored = FocusScope::new(STANDALONE_WINDOW, &app.ui_actions, None)
        .resolve_request(&mut app.input_ownership)
        .expect("shared scope resolves the current destination");
    assert_eq!(restored, Some(target));
    app.set_window_focus(STANDALONE_WINDOW, restored.map(|focus| focus.element));
    assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(target.element));
    assert!(
        app.input_ownership.visible_focus().is_none(),
        "a pointer Dock must not inherit the destination's default keyboard outline"
    );
    app.observe_window_focus_input(STANDALONE_WINDOW, vesta_gpui::FocusInput::Keyboard);
    assert_eq!(
        app.input_ownership.visible_focus(),
        Some(target),
        "later keyboard input still reveals the same current tab"
    );
    assert!(!request(&mut app, ContainerRef::Main, None));
    assert_eq!(
        app.demo.workspace, after_drop,
        "focus is not a second Core action"
    );

    app.dock_focus = Some(accepted);
    let modal_root = app.scopes.header.element(MODE_2D);
    assert!(!request(&mut app, ContainerRef::Main, Some(modal_root)));
    assert!(app.dock_focus.is_none());
    assert!(
        app.input_ownership
            .pending_focus_target(STANDALONE_WINDOW)
            .is_none()
    );
    assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(target.element));

    app.dock_focus = Some(accepted);
    app.demo
        .workspace
        .dock_item(
            item,
            DockAction::Float {
                placement: WindowPlacement::new(20.0, 20.0, 360.0, 220.0),
            },
        )
        .expect("newer relocation");
    assert!(!request(&mut app, ContainerRef::Main, None));
    assert!(
        app.dock_focus.is_none(),
        "a current item in another container is not the old target"
    );
    app.demo.workspace = after_drop;
    app.demo.sync_workspace_ui();
    install_action_route(&mut app);

    app.dock_focus = Some(accepted);
    let other = app
        .demo
        .workspace
        .pane(destination)
        .expect("destination")
        .items
        .iter()
        .find(|candidate| candidate.id != item)
        .expect("another tab")
        .id;
    app.demo.workspace.activate(other).expect("newer selection");
    assert!(!request(&mut app, ContainerRef::Main, None));
    assert!(app.dock_focus.is_none(), "a newer active item wins");
    app.demo
        .workspace
        .activate(item)
        .expect("restore current item");
    app.dock_focus = Some(accepted);
    app.cancel_workspace_drag_for_container(source);
    assert_eq!(
        app.dock_focus,
        Some(accepted),
        "source closure preserves destination intent"
    );
    app.cancel_workspace_drag_for_container(ContainerRef::Main);
    assert!(
        app.dock_focus.is_none(),
        "destination display loss retires it"
    );

    app.dock_focus = Some(accepted);
    app.observe_window_focus_input(STANDALONE_WINDOW, vesta_gpui::FocusInput::Keyboard);
    assert!(app.dock_focus.is_none(), "new accepted navigation wins");
    app.dock_focus = Some(accepted);
    app.reset_workspace();
    assert!(
        app.dock_focus.is_none(),
        "reset may reuse IDs, never the old intent"
    );
    assert_eq!(
        app.demo.workspace.active_item().expect("reset item").id,
        item
    );
    let pane = app.demo.workspace.active_pane();
    let end = app
        .demo
        .workspace
        .pane(pane)
        .expect("reset pane")
        .items
        .len();
    app.demo
        .workspace
        .dock_item(item, DockAction::Merge { pane, slot: end })
        .expect("same-window reorder");
    app.demo.sync_workspace_ui();
    install_action_route(&mut app);
    app.dock_focus = DockFocusTransfer::after_drop(item, ContainerRef::Main, &app.demo.workspace);
    assert!(
        !request(&mut app, ContainerRef::Main, None),
        "same-window needs no HWND activation"
    );
    assert_eq!(
        app.input_ownership.pending_focus_target(STANDALONE_WINDOW),
        Some(tab_target(&app, item))
    );
    app.input_ownership.set_focus(None);
    app.dock_focus = Some(accepted);
    app.apply_workspace_action(WorkspaceViewAction::CloseTab(item));
    assert!(
        app.dock_focus.is_none(),
        "removal retires before old input can reuse its route"
    );
}
