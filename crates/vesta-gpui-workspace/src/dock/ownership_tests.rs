//! Current model ownership, after a valid drag's last pointer motion.
use super::*;
use vesta_workspace::{Pane, PanelTypeId};

pub(super) fn floating_destination(
    model: &WorkspaceModel,
    item: ItemId,
    target: DockDropTarget,
) -> (WorkspaceModel, DockDropTarget) {
    let mut model = model.clone();
    model
        .dock_item(
            item,
            DockAction::Float {
                placement: WindowPlacement::new(20.0, 30.0, 800.0, 600.0),
            },
        )
        .expect("real destination container");
    let pane = *model
        .floating_windows()
        .first()
        .expect("float")
        .root
        .panes()
        .first()
        .expect("destination pane");
    (
        model,
        DockDropTarget {
            action: DockAction::Merge { pane, slot: 0 },
            ..target
        },
    )
}

pub(super) fn qualify() {
    let pane = PaneId(1);
    let mut model = WorkspaceModel::with_tree(WorkspaceNode::leaf(pane), [Pane::new(pane)])
        .expect("source model");
    let source = model
        .add_item(pane, "Source", PanelTypeId::new("test"))
        .expect("source");
    let other = model
        .add_item(pane, "Destination", PanelTypeId::new("test"))
        .expect("other");
    let placement = WindowPlacement::new(20.0, 30.0, 360.0, 220.0);
    model
        .dock_item(other, DockAction::Float { placement })
        .expect("destination");
    let floating = model.floating_windows().first().expect("float");
    let destination = ContainerRef::Floating(floating.id);
    let destination_pane = *floating.root.panes().first().expect("destination pane");
    let floating_id = floating.id;
    let target = DockDropTarget {
        action: DockAction::Merge {
            pane: destination_pane,
            slot: 1,
        },
        preview: Rect::new(0.0, 0.0, 300.0, 200.0),
        insertion: None,
    };
    let drag = WorkspaceTabDrag {
        item: source,
        origin_container: ContainerRef::Main,
        origin: Point::new(50.0, 14.0),
        started: true,
        pointer_container: Some(destination),
        cursor: Point::new(100.0, 100.0),
        target: Some(target),
    };
    let mut pending = Some(drag);
    let (item, action) = finish_tab_drag(&mut pending, &model).expect("current cross-window drop");
    let mut accepted = model.clone();
    accepted.dock_item(item, action).expect("atomic valid drop");
    assert!(
        accepted
            .pane(destination_pane)
            .expect("destination")
            .items
            .iter()
            .any(|i| i.id == source)
    );
    assert!(pending.is_none());

    let mut failures = Vec::new();
    let mut moved = model.clone();
    moved
        .dock_item(source, DockAction::Float { placement })
        .expect("relocate source");
    reject(&mut failures, "relocated Main source", drag, &moved, false);
    let reverse = WorkspaceTabDrag {
        item: other,
        origin_container: destination,
        pointer_container: Some(ContainerRef::Main),
        target: Some(DockDropTarget {
            action: DockAction::Merge { pane, slot: 0 },
            ..target
        }),
        ..drag
    };
    let mut restored = model.clone();
    restored
        .restore_floating_window(floating_id, pane)
        .expect("restore source");
    reject(
        &mut failures,
        "restored Float source",
        reverse,
        &restored,
        false,
    );
    let mut closed = model.clone();
    closed
        .close_floating_window(floating_id)
        .expect("close destination");
    for action in [
        target.action,
        DockAction::Split {
            pane: destination_pane,
            side: Side::Left,
        },
        DockAction::WindowSplit {
            container: destination,
            side: Side::Top,
        },
    ] {
        let stale = WorkspaceTabDrag {
            target: Some(DockDropTarget { action, ..target }),
            ..drag
        };
        reject(&mut failures, "closed destination", stale, &closed, true);
    }
    let wrong_container = WorkspaceTabDrag {
        pointer_container: Some(ContainerRef::Main),
        ..drag
    };
    reject(
        &mut failures,
        "foreign target container",
        wrong_container,
        &model,
        true,
    );

    // A normal tear-off creates a new window; it does not target an existing pane.
    let tear_off = WorkspaceTabDrag {
        pointer_container: Some(ContainerRef::Main),
        target: Some(DockDropTarget {
            action: DockAction::Float { placement },
            ..target
        }),
        ..drag
    };
    assert!(finish_tab_drag(&mut Some(tear_off), &model).is_some());
    assert!(
        failures.is_empty(),
        "stale Dock ownership accepted: {failures:?}"
    );
}

fn reject(
    failures: &mut Vec<String>,
    reason: &str,
    drag: WorkspaceTabDrag,
    model: &WorkspaceModel,
    source_remains: bool,
) {
    let visual = tab_drag_visual(&Some(drag), model, drag.pointer_container.expect("pointer"));
    if if source_remains {
        visual.is_some_and(|v| v.target.is_some())
    } else {
        visual.is_some()
    } {
        failures.push(format!("{reason}: stale preview"));
    }
    let mut pending = Some(drag);
    if finish_tab_drag(&mut pending, model).is_some() {
        failures.push(format!("{reason}: old release"));
    }
    assert!(
        pending.is_none(),
        "rejection consumes the source exactly once"
    );
    assert!(finish_tab_drag(&mut pending, model).is_none());
}
