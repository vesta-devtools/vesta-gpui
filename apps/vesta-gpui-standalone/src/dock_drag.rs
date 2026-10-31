//! Standalone host adapter over the shared dock geometry and drag transaction.
//!
//! The portable part lives in `vesta_gpui_workspace`: drop-target resolution,
//! the drag transaction and the preview rectangles. This module only adds what
//! needs winit or this host's state: window geometry, key-to-pixel mapping, and
//! the `App` methods that mutate the Standalone workspace.

use super::*;
mod focus;
pub(crate) use focus::DockFocusTransfer;

// Geometry types the host reads by name; several are referenced only from the
// host's test module, which `cargo check` does not compile.
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use vesta_gpui_workspace::{
    DockDragVisual, DockDropTarget, DockFrame, DockPaneFrame, DockSplitFrame, DockTabFrame,
    DockWindowGeometry, FloatingMoveDrag, WorkspaceTabDrag, dock_edge_preview, dock_split_ratio,
    dock_tab_split_regions, route_dock_pointer,
};
use vesta_gpui_workspace::{
    finish_tab_drag_from, leave_tab_drag, tab_drag_visual, update_tab_drag,
};

impl App {
    pub(super) fn reset_workspace(&mut self) {
        self.dock_focus = None;
        let owners = std::iter::once(STANDALONE_WINDOW)
            .chain(self.native_windows.values().map(|host| host.ui_window))
            .collect::<Vec<_>>();
        // Stable IDs in the replacement are not the old interaction's source.
        // Restore previews before a numeric field's ordinary exit commits a draft.
        for owner in owners {
            self.reset_window_editors(owner);
        }
        self.demo.workspace = demo_workspace();
        self.demo.pane_history.clear();
        self.demo.zoomed_panes.clear();
        self.demo.sync_workspace_ui();
        self.workspace_drag = None;
        self.floating_move = None;
        self.workspace_resize = None;
    }
}

/// Keyboard resizing stays host-side: `vesta-gpui` has no windowing dependency,
/// so this maps winit keys onto the shared pixel-delta helper.
pub(crate) fn split_ratio_for_key(frame: DockSplitFrame, current: f32, key: &Key) -> Option<f32> {
    let delta = match (frame.axis, key) {
        (SplitAxis::Row, Key::Named(NamedKey::ArrowLeft))
        | (SplitAxis::Column, Key::Named(NamedKey::ArrowUp)) => -16.0,
        (SplitAxis::Row, Key::Named(NamedKey::ArrowRight))
        | (SplitAxis::Column, Key::Named(NamedKey::ArrowDown)) => 16.0,
        (_, Key::Named(NamedKey::PageUp)) => -64.0,
        (_, Key::Named(NamedKey::PageDown)) => 64.0,
        (_, Key::Named(NamedKey::Home)) => f32::NEG_INFINITY,
        (_, Key::Named(NamedKey::End)) => f32::INFINITY,
        _ => return None,
    };
    Some(vesta_gpui_workspace::split_ratio_from_delta(
        frame.rect,
        frame.axis,
        frame.flow_direction,
        current,
        delta,
    ))
}

pub(super) fn window_geometry(
    container: ContainerRef,
    window: &Window,
) -> Option<DockWindowGeometry> {
    if window.is_minimized() == Some(true) || window.is_visible() == Some(false) {
        return None;
    }
    let outer = window.outer_position().ok()?;
    let client = window.inner_position().ok()?;
    let outer_size = window.outer_size();
    let client_size = window.inner_size();
    if client_size.width == 0 || client_size.height == 0 {
        return None;
    }
    Some(DockWindowGeometry {
        container,
        outer: Rect::new(
            outer.x as f32,
            outer.y as f32,
            outer_size.width as f32,
            outer_size.height as f32,
        ),
        client: Rect::new(
            client.x as f32,
            client.y as f32,
            client_size.width as f32,
            client_size.height as f32,
        ),
        scale: window.scale_factor() as f32,
    })
}

impl App {
    pub(super) fn apply_workspace_action(&mut self, action: WorkspaceViewAction) {
        match action {
            WorkspaceViewAction::Tab(item) => {
                let _ = self.demo.workspace.activate(item);
                self.demo.sync_workspace_ui();
                self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
                self.close_popover_for(self.action_window, PopupCloseReason::FocusTransfer);
                self.close_combo_for(self.action_window, PopupCloseReason::FocusTransfer);
            }
            WorkspaceViewAction::Navigate {
                pane,
                active,
                direction,
            } => {
                if self
                    .window_container(self.action_window)
                    .is_some_and(|container| {
                        container_panes(&self.demo.workspace, container).contains(&pane)
                    })
                    && self.demo.workspace.pane(pane).and_then(|pane| pane.active) == Some(active)
                    && let Some((index, target)) = self
                        .demo
                        .pane_history
                        .get(&pane)
                        .and_then(|history| history.target(direction))
                    && self
                        .demo
                        .workspace
                        .pane(pane)
                        .is_some_and(|pane| pane.items.iter().any(|item| item.id == target))
                    && self.demo.workspace.activate(target).is_ok()
                {
                    if let Some(history) = self.demo.pane_history.get_mut(&pane) {
                        history.cursor = index;
                    }
                    self.demo.sync_workspace_ui();
                    self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
                    self.close_popover_for(self.action_window, PopupCloseReason::FocusTransfer);
                    self.close_combo_for(self.action_window, PopupCloseReason::FocusTransfer);
                }
            }
            WorkspaceViewAction::HeaderCommand {
                pane,
                active,
                command,
            } => {
                if !self
                    .window_container(self.action_window)
                    .is_some_and(|container| {
                        container_panes(&self.demo.workspace, container).contains(&pane)
                    })
                    || self.demo.workspace.pane(pane).and_then(|pane| pane.active) != Some(active)
                {
                    return;
                }
                match command {
                    HEADER_NEW_TAB => {
                        let _ = self.demo.workspace.add_item(
                            pane,
                            "New Tab",
                            PanelTypeId::new("gallery.blank"),
                        );
                        self.demo.sync_workspace_ui();
                    }
                    HEADER_SPLIT_RIGHT => {
                        if self
                            .demo
                            .workspace
                            .pane(pane)
                            .is_some_and(|pane| pane.items.len() > 1)
                        {
                            self.split_workspace_item_in_window(self.action_window, pane, active);
                        }
                    }
                    HEADER_TOGGLE_ZOOM => {
                        if let Some(container) = self.window_container(self.action_window)
                            && container_panes(&self.demo.workspace, container).len() > 1
                        {
                            if self.demo.zoomed_panes.get(&container) == Some(&pane) {
                                self.demo.zoomed_panes.remove(&container);
                            } else {
                                self.demo.zoomed_panes.insert(container, pane);
                            }
                        }
                    }
                    HEADER_OVERFLOW => {
                        let anchor = self
                            .input_ownership
                            .focus()
                            .and_then(|focus| self.focus_rects.get(&focus.element))
                            .map_or(
                                Point::new(self.cursor.x, self.cursor.y + CHROME_HEIGHT),
                                |rect| Point::new(rect.x, rect.bottom()),
                            );
                        if let Some(container) = self.window_container(self.action_window) {
                            self.open_window_menu(
                                self.action_window,
                                MenuId(5),
                                anchor,
                                MenuContext::capture(&self.demo, container, pane, None),
                            );
                        }
                    }
                    _ => {}
                }
            }
            WorkspaceViewAction::DockTarget(_) => {}
            WorkspaceViewAction::CloseTab(item) => {
                self.retire_workspace_page_input(item);
                let _ = self.demo.workspace.close_item(item);
                self.demo.sync_workspace_ui();
                self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
                self.close_popover_for(self.action_window, PopupCloseReason::FocusTransfer);
                self.close_combo_for(self.action_window, PopupCloseReason::FocusTransfer);
            }
            WorkspaceViewAction::BeginResize {
                container,
                split,
                axis,
            } => {
                self.begin_workspace_resize(container, split, axis);
            }
            WorkspaceViewAction::BeginMoveFloating(window) => {
                if let Some(floating) = self.demo.workspace.floating_window(window) {
                    self.floating_move = Some(FloatingMoveDrag {
                        window,
                        origin: self.cursor,
                        placement: floating.placement,
                    });
                }
            }
            WorkspaceViewAction::CloseFloating(window) => {
                let _ = self.demo.workspace.close_floating_window(window);
                self.demo.sync_workspace_ui();
            }
        }
        self.reconcile_workspace_gestures();
    }

    pub(super) fn move_active_workspace_item(&mut self) {
        let Some(item) = self.demo.workspace.active_item().map(|item| item.id) else {
            return;
        };
        let active = self.demo.workspace.active_pane();
        let target = self
            .demo
            .workspace
            .panes()
            .into_iter()
            .find(|pane| pane.id != active)
            .map(|pane| pane.id);
        if let Some(pane) = target {
            let slot = self
                .demo
                .workspace
                .pane(pane)
                .map_or(0, |pane| pane.items.len());
            let _ = self
                .demo
                .workspace
                .dock_item(item, DockAction::Merge { pane, slot });
            self.demo.sync_workspace_ui();
        }
    }

    pub(super) fn split_active_workspace_item(&mut self) {
        if let Some(item) = self.demo.workspace.active_item().map(|item| item.id) {
            self.split_workspace_item_in_window(
                self.action_window,
                self.demo.workspace.active_pane(),
                item,
            );
        }
    }

    pub(super) fn float_active_workspace_item(&mut self) {
        let Some(item) = self.demo.workspace.active_item().map(|item| item.id) else {
            return;
        };
        self.float_workspace_item(item, WindowPlacement::new(80.0, 40.0, 360.0, 220.0));
    }

    pub(super) fn float_workspace_item(
        &mut self,
        item: ItemId,
        placement: WindowPlacement,
    ) -> bool {
        if self.demo.workspace.item(item).is_none() {
            return false;
        }
        if self
            .demo
            .workspace
            .dock_item(item, DockAction::Float { placement })
            .is_err()
        {
            return false;
        }
        self.demo.sync_workspace_ui();
        self.request_all_redraw();
        true
    }

    pub(super) fn reconcile_workspace_gestures(&mut self) {
        if self
            .dock_focus
            .is_some_and(|transfer| !transfer.is_current(&self.demo.workspace))
        {
            self.dock_focus = None;
        }
        fn contains_split(node: &WorkspaceNode, split: SplitId, axis: SplitAxis) -> bool {
            match node {
                WorkspaceNode::Leaf(_) => false,
                WorkspaceNode::Split {
                    id,
                    axis: node_axis,
                    first,
                    second,
                    ..
                } => {
                    (*id == split && *node_axis == axis)
                        || contains_split(first, split, axis)
                        || contains_split(second, split, axis)
                }
            }
        }

        if self.workspace_drag.is_some_and(|drag| {
            self.demo.workspace.container_of_item(drag.item) != Some(drag.origin_container)
        }) {
            self.workspace_drag = None;
        }
        if self
            .floating_move
            .is_some_and(|drag| self.demo.workspace.floating_window(drag.window).is_none())
        {
            self.floating_move = None;
        }
        if self.workspace_resize.is_some_and(|drag| {
            let root = match drag.container() {
                ContainerRef::Main => Some(self.demo.workspace.root()),
                ContainerRef::Floating(window) => self
                    .demo
                    .workspace
                    .floating_window(window)
                    .map(|window| &window.root),
            };
            !root.is_some_and(|root| contains_split(root, drag.split(), drag.axis()))
        }) && let Some(session) = self.workspace_resize
        {
            self.finish_workspace_resize_for(session.target().window, true);
        }
    }

    pub(super) fn cancel_workspace_drag_for_container(&mut self, container: ContainerRef) {
        if self
            .dock_focus
            .is_some_and(|transfer| transfer.destination() == container)
        {
            self.dock_focus = None;
        }
        vesta_gpui_workspace::cancel_tab_drag(&mut self.workspace_drag, container);
    }

    pub(super) fn cancel_window_workspace_drag(&mut self, window: UiWindowId) {
        if let Some(container) = self.window_container(window) {
            self.cancel_workspace_drag_for_container(container);
        }
    }

    pub(super) fn observe_workspace_geometry(
        &mut self,
        container: ContainerRef,
        event: &WindowEvent,
    ) {
        if self.workspace_drag.is_some_and(|drag| {
            drag.origin_container == container || drag.pointer_container == Some(container)
        }) && matches!(
            event,
            WindowEvent::Moved(_)
                | WindowEvent::Resized(_)
                | WindowEvent::ScaleFactorChanged { .. }
        ) {
            self.cancel_workspace_drag_for_container(container);
        }
        if matches!(
            event,
            WindowEvent::Moved(_)
                | WindowEvent::Resized(_)
                | WindowEvent::ScaleFactorChanged { .. }
        ) && let Some(session) = self
            .workspace_resize
            .filter(|session| session.container() == container)
        {
            self.finish_workspace_resize_for(session.target().window, false);
        }
    }

    pub(super) fn dock_window_admitted(&self, window: UiWindowId, display_available: bool) -> bool {
        display_available && self.focus_root_for_window(window).is_none()
    }

    pub(super) fn dock_container_admitted(&self, container: ContainerRef) -> bool {
        let window = match container {
            ContainerRef::Main => self
                .window
                .as_ref()
                .map(|window| (window.id(), STANDALONE_WINDOW, window.as_ref())),
            ContainerRef::Floating(logical_id) => self
                .native_windows
                .iter()
                .find(|(_, host)| host.logical_id == logical_id)
                .map(|(id, host)| (*id, host.ui_window, host.window.as_ref())),
        };
        window.is_some_and(|(native, owner, window)| {
            let size = window.inner_size();
            self.dock_window_admitted(
                owner,
                !self.display_blocked(native)
                    && window.is_minimized() != Some(true)
                    && window.is_visible() != Some(false)
                    && size.width > 0
                    && size.height > 0,
            )
        })
    }

    pub(super) fn begin_workspace_resize(
        &mut self,
        container: ContainerRef,
        split: SplitId,
        axis: SplitAxis,
    ) -> bool {
        if self.workspace_resize.is_some() {
            return false;
        }
        let window = self.action_window;
        let action = DemoAction::Workspace(WorkspaceViewAction::BeginResize {
            container,
            split,
            axis,
        });
        let Some(element) = self.window_actions(window).and_then(|actions| {
            actions
                .focus_order()
                .iter()
                .copied()
                .find(|id| actions.action_at(*id) == Some(&action))
        }) else {
            return false;
        };
        let frame = if window == STANDALONE_WINDOW {
            self.dock_frame.split_frame(container, split, axis)
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .and_then(|host| host.dock_frame.split_frame(container, split, axis))
        };
        let (Some(frame), Some(ratio)) = (
            frame,
            dock_split_ratio(&self.demo.workspace, container, split),
        ) else {
            return false;
        };
        let target = InputTarget { window, element };
        let Ok(session) = vesta_gpui_workspace::SplitResizeSession::new(target, frame, ratio)
        else {
            return false;
        };
        self.set_window_focus(window, Some(element));
        if window == STANDALONE_WINDOW {
            self.input_ownership
                .capture_pointer(target, PointerButton::Primary);
            self.pointer_click.cancel();
        } else if let Some(host) = self
            .native_windows
            .values_mut()
            .find(|host| host.ui_window == window)
        {
            host.input.capture_pointer(target, PointerButton::Primary);
            host.pointer_click.cancel();
        }
        self.workspace_resize = Some(session);
        true
    }

    pub(super) fn update_workspace_resize(&mut self, window: UiWindowId, pointer: Point) -> bool {
        let Some(session) = self.workspace_resize else {
            return false;
        };
        let Some(ratio) = session.ratio_for_pointer(window, pointer) else {
            return false;
        };
        let before = dock_split_ratio(&self.demo.workspace, session.container(), session.split());
        if self
            .demo
            .workspace
            .set_split_ratio(session.container(), session.split(), ratio)
            .is_err()
        {
            self.finish_workspace_resize_for(window, true);
            return false;
        }
        before != dock_split_ratio(&self.demo.workspace, session.container(), session.split())
    }

    pub(super) fn finish_workspace_resize_for(&mut self, window: UiWindowId, commit: bool) -> bool {
        if !self
            .workspace_resize
            .is_some_and(|drag| drag.target().window == window)
        {
            return false;
        }
        let Some(mut session) = self.workspace_resize.take() else {
            return false;
        };
        if let Some(current) =
            dock_split_ratio(&self.demo.workspace, session.container(), session.split())
            && let Some(PointerEditOutcome::Cancel { restore }) =
                session.finish(window, current, commit)
        {
            let _ =
                self.demo
                    .workspace
                    .set_split_ratio(session.container(), session.split(), restore);
        }
        let input = if window == STANDALONE_WINDOW {
            Some(&mut self.input_ownership)
        } else {
            self.native_windows
                .values_mut()
                .find(|host| host.ui_window == window)
                .map(|host| &mut host.input)
        };
        if let Some(input) = input
            && input.capture().is_some_and(|capture| {
                capture.target == session.target() && capture.button == PointerButton::Primary
            })
        {
            input.cancel_capture();
        }
        true
    }

    pub(super) fn update_workspace_drag(
        &mut self,
        container: ContainerRef,
        cursor: Point,
        target: Option<DockDropTarget>,
    ) -> bool {
        update_tab_drag(&mut self.workspace_drag, container, cursor, target)
    }

    pub(super) fn leave_workspace_drag(&mut self, container: ContainerRef) -> bool {
        leave_tab_drag(&mut self.workspace_drag, container)
    }

    pub(super) fn finish_workspace_drag(&mut self, source: ContainerRef) -> bool {
        let containers = self.workspace_drag.map_or([None, None], |drag| {
            [Some(drag.origin_container), drag.pointer_container]
        });
        let admitted = containers.map(|container| {
            container.filter(|container| self.dock_container_admitted(*container))
        });
        self.finish_workspace_drag_with_admission(source, |container| {
            admitted.contains(&Some(container))
        })
    }

    pub(super) fn finish_workspace_drag_with_admission(
        &mut self,
        source: ContainerRef,
        admitted: impl Fn(ContainerRef) -> bool,
    ) -> bool {
        if !self
            .workspace_drag
            .is_some_and(|drag| drag.origin_container == source)
        {
            return false;
        }
        if let Some((item, action)) = finish_tab_drag_from(
            &mut self.workspace_drag,
            source,
            &self.demo.workspace,
            admitted,
        ) {
            if self.demo.workspace.dock_item(item, action).is_ok() {
                self.dock_focus = DockFocusTransfer::after_drop(item, source, &self.demo.workspace);
            }
            self.demo.sync_workspace_ui();
        }
        self.request_all_redraw();
        true
    }

    pub(super) fn dock_drag_visual(&self, container: ContainerRef) -> Option<DockDragVisual> {
        tab_drag_visual(&self.workspace_drag, &self.demo.workspace, container)
            .or_else(|| native_capture::dock_visual_sample(self, container))
    }

    pub(super) fn update_native_dock_pointer(
        &mut self,
        source: ContainerRef,
        cursor: Point,
    ) -> bool {
        if self.workspace_drag.is_none() || !self.dock_container_admitted(source) {
            return false;
        }
        let mut windows = self
            .native_window_order
            .iter()
            .rev()
            .filter_map(|id| {
                let host = self.native_windows.get(id)?;
                let geometry =
                    window_geometry(ContainerRef::Floating(host.logical_id), &host.window)?;
                Some((geometry, &host.dock_frame))
            })
            .collect::<Vec<_>>();
        if let Some(window) = &self.window
            && let Some(geometry) = window_geometry(ContainerRef::Main, window)
        {
            windows.push((geometry, &self.dock_frame));
        }
        let source_geometry = windows
            .iter()
            .find(|(geometry, _)| geometry.container == source);
        let route = if let Some((geometry, _)) = source_geometry {
            vesta_gpui_workspace::route_dock_pointer_with_admission(
                *geometry,
                cursor,
                &windows,
                &self.demo.workspace,
                |container| self.dock_container_admitted(container),
            )
        } else {
            // Hosts without desktop positions can still dock inside their own window.
            let frame = match source {
                ContainerRef::Main => Some(&self.dock_frame),
                ContainerRef::Floating(id) => self
                    .native_windows
                    .values()
                    .find(|host| host.logical_id == id)
                    .map(|host| &host.dock_frame),
            };
            frame.map(|frame| (source, cursor, frame.target(cursor, &self.demo.workspace)))
        };
        if let Some((container, cursor, target)) = route {
            self.update_workspace_drag(container, cursor, target)
        } else if let Some(container) = self.workspace_drag.and_then(|drag| drag.pointer_container)
        {
            self.leave_workspace_drag(container)
        } else {
            false
        }
    }
}
