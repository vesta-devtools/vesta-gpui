//! Native157 exposed an icon rectangle overwriting the restorable Core layout.
use super::*;
use winit::dpi::{PhysicalPosition, PhysicalSize};

pub(super) fn minimized_events_preserve_restore_placement() {
    let pane = PaneId(1);
    let mut model =
        WorkspaceModel::with_tree(WorkspaceNode::leaf(pane), [Pane::new(pane)]).expect("workspace");
    let item = model
        .add_item(pane, "Layout", PanelTypeId::new("gallery.layout"))
        .expect("item");
    let original = WindowPlacement::new(560.0, 80.0, 520.0, 650.4);
    model
        .dock_item(
            item,
            DockAction::Float {
                placement: original,
            },
        )
        .expect("float");
    let floating = model.floating_windows()[0].id;
    let before = model.clone();
    for event in [
        WindowEvent::Moved(PhysicalPosition::new(0, 1346)),
        WindowEvent::Resized(PhysicalSize::new(199, 34)),
    ] {
        let current = model.floating_window(floating).expect("window").placement;
        if let Some(placement) =
            native_window::placement_from_native_event(current, &event, 1.25, true)
        {
            model
                .set_floating_placement(floating, placement)
                .expect("window");
        }
    }
    assert_eq!(
        model, before,
        "native minimized Move/Resize cannot overwrite the normal restore layout"
    );

    // Ordinary negative desktop origins and mixed positive scales remain valid.
    let moved = native_window::placement_from_native_event(
        original,
        &WindowEvent::Moved(PhysicalPosition::new(-1500, 300)),
        1.25,
        false,
    )
    .expect("normal move");
    model
        .set_floating_placement(floating, moved)
        .expect("window");
    assert_eq!(
        model.floating_window(floating).expect("window").placement,
        WindowPlacement::new(-1200.0, 240.0, original.width, original.height)
    );
    let resized = native_window::placement_from_native_event(
        moved,
        &WindowEvent::Resized(PhysicalSize::new(900, 600)),
        1.5,
        false,
    )
    .expect("normal resize");
    model
        .set_floating_placement(floating, resized)
        .expect("window");
    assert_eq!(
        model.floating_window(floating).expect("window").placement,
        WindowPlacement::new(moved.x, moved.y, 600.0, 400.0)
    );
    assert_eq!(
        model.container_of_item(item),
        Some(ContainerRef::Floating(floating))
    );
}
