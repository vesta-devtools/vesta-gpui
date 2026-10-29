//! Display admission and retirement for the existing container-owned Dock gesture.
use crate::dock::{DockDropTarget, DockFrame, DockWindowGeometry, WorkspaceTabDrag};
use vesta_gpui::Point;
use vesta_workspace::{ContainerRef, DockAction, ItemId, WorkspaceModel};

/// Retire the origin's gesture, or invalidate only a suspended destination.
/// An unrelated container cannot consume the live source's transaction.
pub fn cancel_tab_drag(drag: &mut Option<WorkspaceTabDrag>, container: ContainerRef) -> bool {
    if drag.is_some_and(|current| current.origin_container == container) {
        *drag = None;
        true
    } else {
        crate::dock::leave_tab_drag(drag, container)
    }
}

/// Recheck the current display gate on release, which can precede the next
/// pointer move after a destination resize. Rejection still consumes the drag.
pub fn finish_tab_drag_with_admission(
    drag: &mut Option<WorkspaceTabDrag>,
    model: &WorkspaceModel,
    admitted: impl Fn(ContainerRef) -> bool,
) -> Option<(ItemId, DockAction)> {
    let owned = drag.take()?;
    if !admitted(owned.origin_container) || !owned.pointer_container.is_some_and(admitted) {
        return None;
    }
    crate::dock::finish_tab_drag(&mut Some(owned), model)
}

/// A release from another container cannot consume or inspect this transaction.
pub fn finish_tab_drag_from(
    drag: &mut Option<WorkspaceTabDrag>,
    source: ContainerRef,
    model: &WorkspaceModel,
    admitted: impl Fn(ContainerRef) -> bool,
) -> Option<(ItemId, DockAction)> {
    if !drag.is_some_and(|current| current.origin_container == source) {
        return None;
    }
    finish_tab_drag_with_admission(drag, model, admitted)
}

/// Route through completed, admitted displays. A blocked foreground window
/// still occludes the windows behind it; its old Dock frame is never consulted.
#[must_use]
pub fn route_dock_pointer_with_admission(
    source: DockWindowGeometry,
    cursor: Point,
    windows_front_to_back: &[(DockWindowGeometry, &DockFrame)],
    model: &WorkspaceModel,
    admitted: impl Fn(ContainerRef) -> bool,
) -> Option<(ContainerRef, Point, Option<DockDropTarget>)> {
    if !admitted(source.container) || !valid_window_geometry(source) {
        return None;
    }
    let screen = Point::new(
        source.client.x + cursor.x * source.scale,
        source.client.y + cursor.y * source.scale,
    );
    if !screen.x.is_finite() || !screen.y.is_finite() {
        return None;
    }
    for (geometry, frame) in windows_front_to_back {
        // Unknown bounds cannot establish that the window behind is exposed.
        if !valid_window_rect(geometry.outer) {
            return None;
        }
        if !geometry.outer.contains(screen) {
            continue;
        }
        // Native decoration and display suspension both occlude lower windows.
        if !valid_window_geometry(*geometry)
            || !geometry.client.contains(screen)
            || !admitted(geometry.container)
        {
            return None;
        }
        let local = Point::new(
            (screen.x - geometry.client.x) / geometry.scale,
            (screen.y - geometry.client.y) / geometry.scale,
        );
        if !local.x.is_finite() || !local.y.is_finite() {
            return None;
        }
        return Some((geometry.container, local, frame.target(local, model)));
    }
    let (geometry, frame) = windows_front_to_back
        .iter()
        .find(|(geometry, _)| geometry.container == source.container)?;
    if !valid_window_geometry(*geometry) {
        return None;
    }
    Some((source.container, cursor, frame.target(cursor, model)))
}

fn valid_window_geometry(geometry: DockWindowGeometry) -> bool {
    geometry.scale.is_finite()
        && geometry.scale > 0.0
        && valid_window_rect(geometry.outer)
        && valid_window_rect(geometry.client)
}

fn valid_window_rect(rect: vesta_gpui::Rect) -> bool {
    rect.width > 0.0
        && rect.height > 0.0
        && [
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            rect.right(),
            rect.bottom(),
        ]
        .into_iter()
        .all(f32::is_finite)
}

/// All-admitted routing for callers without a display suspension lifecycle.
#[must_use]
pub fn route_dock_pointer(
    source: DockWindowGeometry,
    cursor: Point,
    windows_front_to_back: &[(DockWindowGeometry, &DockFrame)],
    model: &WorkspaceModel,
) -> Option<(ContainerRef, Point, Option<DockDropTarget>)> {
    route_dock_pointer_with_admission(source, cursor, windows_front_to_back, model, |_| true)
}
