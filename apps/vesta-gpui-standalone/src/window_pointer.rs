//! Main-window pointer and value-gesture dispatch; state remains on App.
use super::*;

pub(super) type NumberScrub = HorizontalEditSession<i32>;
impl App {
    pub(super) fn workspace_cursor_icon(&self) -> Option<CursorIcon> {
        let axis = self.workspace_resize.map(|drag| drag.axis()).or_else(|| {
            if self.workspace_drag.is_some() || self.floating_move.is_some() {
                return None;
            }
            let hit = self.hit_test.hit(self.cursor)?;
            match self.ui_actions.action_for_hit(hit)? {
                DemoAction::Workspace(WorkspaceViewAction::BeginResize { axis, .. }) => Some(*axis),
                _ => None,
            }
        })?;
        Some(match axis {
            SplitAxis::Row => CursorIcon::EwResize,
            SplitAxis::Column => CursorIcon::NsResize,
        })
    }

    pub(super) fn table_resize_cursor_icon(&self) -> Option<CursorIcon> {
        if self.table_resize.is_some()
            || self.hit_test.hit(self.cursor).is_some_and(|hit| {
                (0..2).any(|column| hit == table_resize_element(self.scopes.gallery, column))
            })
        {
            Some(CursorIcon::EwResize)
        } else {
            None
        }
    }

    pub(super) fn drawer_resize_cursor_icon(&self) -> Option<CursorIcon> {
        if self.drawer_resize.is_some()
            || self.hit_test.hit(self.cursor) == Some(self.scopes.gallery.element(DRAWER_RESIZE))
        {
            Some(CursorIcon::EwResize)
        } else {
            None
        }
    }

    pub(super) fn update_resize_cursor(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        let size = window.inner_size().to_logical::<f32>(window.scale_factor());
        let direction = (!window.is_maximized())
            .then(|| chrome_resize_direction(self.cursor, Size::new(size.width, size.height)))
            .flatten();
        let icon = direction
            .map(CursorIcon::from)
            .or_else(|| self.drawer_resize_cursor_icon())
            .or_else(|| self.table_resize_cursor_icon())
            .or_else(|| self.workspace_cursor_icon())
            .unwrap_or(CursorIcon::Default);
        if self.cursor_icon != icon {
            window.set_cursor(icon);
            self.cursor_icon = icon;
        }
    }

    pub(super) fn drawer_state(&self) -> SidebarDrawerState<'static> {
        window_controls::drawer_state(&self.demo)
    }

    pub(super) fn begin_drawer_resize(&mut self) -> bool {
        if !self.demo.drawer_open {
            return false;
        }
        let element = self.scopes.gallery.element(DRAWER_RESIZE);
        self.set_text_focus(None);
        self.set_focus_target(Some(element));
        self.input_ownership.capture_pointer(
            InputTarget {
                window: STANDALONE_WINDOW,
                element,
            },
            PointerButton::Primary,
        );
        self.drawer_resize = DrawerResizeDrag::new(
            InputTarget {
                window: STANDALONE_WINDOW,
                element,
            },
            self.cursor.x,
            self.demo.drawer_width,
            0.0,
        )
        .ok();
        self.drawer_resize.is_some()
    }

    pub(super) fn update_drawer_resize(&mut self) -> bool {
        let Some(mut drag) = self.drawer_resize else {
            return false;
        };
        let Some(delta) = drag.offset(STANDALONE_WINDOW, self.cursor.x) else {
            return false;
        };
        self.drawer_resize = Some(drag);
        let width = sidebar_drawer_width_at(*drag.initial(), delta, self.drawer_state());
        if (width - self.demo.drawer_width).abs() <= f32::EPSILON {
            return false;
        }
        self.demo.drawer_width = width;
        true
    }

    pub(super) fn finish_drawer_resize(&mut self) -> bool {
        self.drawer_resize.take().is_some_and(|mut drag| {
            drag.finish(STANDALONE_WINDOW, &self.demo.drawer_width, true)
                .is_some()
        })
    }

    pub(super) fn cancel_drawer_resize(&mut self) -> bool {
        let Some(mut drag) = self.drawer_resize.take() else {
            return false;
        };
        if let Some(PointerEditOutcome::Cancel { restore }) =
            drag.finish(STANDALONE_WINDOW, &self.demo.drawer_width, false)
        {
            self.demo.drawer_width = restore;
        }
        self.input_ownership.cancel_capture();
        true
    }

    pub(super) fn try_scrollbar_press(&mut self) -> bool {
        if self.mode != AcceptanceMode::Gallery2d
            || self.demo.palette_session.is_open()
            || !self.demo.dialog_stack.is_empty()
        {
            return false;
        }
        let hit = self.hit_test.hit(self.cursor);
        let gallery = self.scopes.gallery;
        let pointer = self.cursor;
        let geometry = self.scroll_geometry.clone();
        let mut states = ScrollStates {
            tree: &mut self.demo.tree_scroll,
            virtual_list: &mut self.demo.virtual_scroll,
            virtual_table: &mut self.demo.virtual_table_scroll,
            nested: &mut self.demo.nested_scroll,
            horizontal: &mut self.demo.horizontal_scroll,
            gallery: Some(&mut self.gallery_scroll),
            pages: &mut self.demo.page_scroll,
            tabs: &mut self.demo.tab_scroll,
        };
        let Some((element, drag)) = begin_scrollbar_drag(
            gallery,
            &geometry,
            &self.ui_actions,
            &mut states,
            hit,
            pointer,
        ) else {
            return false;
        };
        self.set_text_focus(None);
        self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
        self.input_ownership.capture_pointer(
            InputTarget {
                window: STANDALONE_WINDOW,
                element,
            },
            PointerButton::Primary,
        );
        self.scrollbar_drag = Some((element, drag));
        true
    }

    pub(super) fn update_scrollbar_drag(&mut self) {
        let Some((element, drag)) = self.scrollbar_drag else {
            return;
        };
        let gallery = self.scopes.gallery;
        let pointer = self.cursor;
        let geometry = self.scroll_geometry.clone();
        let mut states = ScrollStates {
            tree: &mut self.demo.tree_scroll,
            virtual_list: &mut self.demo.virtual_scroll,
            virtual_table: &mut self.demo.virtual_table_scroll,
            nested: &mut self.demo.nested_scroll,
            horizontal: &mut self.demo.horizontal_scroll,
            gallery: Some(&mut self.gallery_scroll),
            pages: &mut self.demo.page_scroll,
            tabs: &mut self.demo.tab_scroll,
        };
        if update_scrollbar_drag_state(gallery, &geometry, &mut states, element, drag, pointer) {
            self.request_redraw();
        }
    }

    pub(super) fn pointer_press(&mut self) {
        self.pointer_click.cancel();
        if self.demo.palette_session.is_open()
            && self.hit_test.hit(self.cursor) == Some(self.scopes.root.element(PALETTE_ROOT))
        {
            self.close_command_palette();
            self.request_redraw();
            return;
        }
        if self.try_scrollbar_press() {
            self.request_redraw();
            return;
        }
        let hit = self.hit_test.hit(self.cursor);
        if self.demo.status_overflow_open
            && !hit.is_some_and(|element| {
                self.ui_actions
                    .is_descendant_of(element, self.scopes.status.element(30))
                    || self
                        .ui_actions
                        .is_descendant_of(element, self.scopes.status.element(40))
            })
        {
            self.demo.status_overflow_open = false;
        }
        self.dismiss_popover_for_hit(hit);
        self.dismiss_combo_for_hit(hit);
        match hit {
            Some(hit)
                if self.mode == AcceptanceMode::Gallery2d
                    && hit == self.scopes.gallery.element(DRAWER_RESIZE) =>
            {
                self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
                self.begin_drawer_resize();
            }
            Some(hit)
                if self.mode == AcceptanceMode::Gallery2d
                    && (0..2)
                        .any(|column| hit == table_resize_element(self.scopes.gallery, column)) =>
            {
                self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
                let column = (0..2)
                    .find(|column| hit == table_resize_element(self.scopes.gallery, *column))
                    .expect("matched table resize column");
                self.begin_table_resize(column);
            }
            Some(hit)
                if self.mode == AcceptanceMode::Gallery2d
                    && TextFieldKind::ALL
                        .into_iter()
                        .any(|field| field.element(self.scopes.gallery) == hit) =>
            {
                let field = TextFieldKind::ALL
                    .into_iter()
                    .find(|field| field.element(self.scopes.gallery) == hit)
                    .expect("matched a stable text field");
                self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
                self.start_text_selection(field);
                match field {
                    TextFieldKind::Number => self.begin_number_scrub(),
                    TextFieldKind::Vector(axis) => self.begin_vector_scrub(axis),
                    _ => {}
                }
            }
            Some(hit)
                if self.mode == AcceptanceMode::Gallery2d && self.ramp_point_at(hit).is_some() =>
            {
                self.begin_ramp_drag(hit);
            }
            Some(hit) if self.ui_actions.action_for_hit(hit).is_some() => {
                let routed = self
                    .ui_actions
                    .target_for_hit(hit)
                    .map(|(target, action)| (target, *action));
                if let Some((target, action)) = routed {
                    // A press inside the open popup must keep the menu alive until the matching
                    // release resolves the click. Closing it here rebuilds the tree without the
                    // item, so `PointerClick::release` no longer finds the pressed target and
                    // silently drops actions such as TearOffTab. Presses elsewhere still dismiss
                    // the menu, and menu-bar entries keep their own exemptions below.
                    let inside_open_menu = self
                        .ui_actions
                        .is_descendant_of(hit, self.scopes.menu.element(1));
                    if self.demo.menu.is_open()
                        && !inside_open_menu
                        && !matches!(
                            action,
                            DemoAction::ToggleMenu
                                | DemoAction::ToggleApplicationMenu
                                | DemoAction::OpenMenu(_)
                                | DemoAction::OpenSubmenu(_)
                                | DemoAction::MenuCommand(_)
                        )
                    {
                        self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
                    }
                    if self.ui_actions.focus_order().contains(&target) {
                        self.set_focus_target(Some(target));
                    } else if !matches!(action, DemoAction::FocusSearch | DemoAction::ClearSearch) {
                        self.set_text_focus(None);
                    }
                    if action.starts_pointer_drag() {
                        if let DemoAction::Workspace(WorkspaceViewAction::Tab(item)) = action {
                            self.workspace_drag = Some(WorkspaceTabDrag {
                                item,
                                origin_container: ContainerRef::Main,
                                origin: self.cursor,
                                started: false,
                                pointer_container: Some(ContainerRef::Main),
                                cursor: self.cursor,
                                target: None,
                            });
                        }
                        self.apply_demo_action(action);
                    } else {
                        self.pointer_click.press(
                            STANDALONE_WINDOW,
                            PointerButton::Primary,
                            &self.ui_actions,
                            Some(hit),
                        );
                    }
                }
            }
            Some(_) => {
                self.set_text_focus(None);
                self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
                self.close_combo_for(self.action_window, PopupCloseReason::FocusTransfer);
            }
            None if self.mode == AcceptanceMode::Viewport3d => {
                self.set_text_focus(None);
                if let Some(window) = &self.window {
                    let size = window.inner_size().to_logical::<f32>(window.scale_factor());
                    if let Some(drag) = TranslationDrag::begin(
                        &self.scene,
                        self.camera,
                        size.width,
                        size.height,
                        self.cursor.x,
                        self.cursor.y,
                    ) {
                        self.transform_drag = Some(drag);
                        self.apply_transform_action(drag, drag.begin_action());
                    } else {
                        let ndc_x = self.cursor.x / size.width.max(1.0) * 2.0 - 1.0;
                        let ndc_y = self.cursor.y / size.height.max(1.0) * 2.0 - 1.0;
                        let selected = pick(
                            &self.scene,
                            self.camera,
                            size.width / size.height.max(1.0),
                            ndc_x,
                            ndc_y,
                        );
                        self.scene.select(selected);
                        self.camera_drag = true;
                        self.last_cursor = self.cursor;
                    }
                }
            }
            None => {
                self.set_text_focus(None);
                self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
                self.close_combo_for(self.action_window, PopupCloseReason::FocusTransfer);
            }
        }
        self.request_redraw();
    }

    pub(super) fn apply_transform_action(
        &mut self,
        drag: TranslationDrag,
        action: TransformAction,
    ) {
        match action {
            TransformAction::Begin { part, .. } => self.scene.gizmo.active = Some(part),
            TransformAction::Update { delta } => {
                if let Some(renderable) = self
                    .scene
                    .renderables
                    .iter_mut()
                    .find(|renderable| renderable.id == drag.target())
                {
                    renderable.translation = drag.original().plus(delta);
                }
            }
            TransformAction::Commit => self.scene.gizmo.active = None,
            TransformAction::Cancel => {
                if let Some(renderable) = self
                    .scene
                    .renderables
                    .iter_mut()
                    .find(|renderable| renderable.id == drag.target())
                {
                    renderable.translation = drag.original();
                }
                self.scene.gizmo.active = None;
            }
        }
    }

    pub(super) fn cancel_transform_drag(&mut self) {
        if let Some(drag) = self.transform_drag.take() {
            self.apply_transform_action(drag, TransformAction::Cancel);
        }
    }

    pub(super) fn paint_interaction(&self) -> PaintInteraction {
        self.window_paint_interaction(self.cursor, self.scrollbar_drag.map(|(element, _)| element))
    }

    pub(super) fn window_paint_interaction(
        &self,
        cursor: Point,
        active_scrollbar: Option<ElementId>,
    ) -> PaintInteraction {
        let moving_dock = self.workspace_drag.is_some_and(|drag| drag.started)
            || self.floating_move.is_some()
            || self.workspace_resize.is_some();
        PaintInteraction {
            pointer: (!moving_dock).then_some(cursor),
            active_scrollbar,
        }
    }

    pub(super) fn cancel_floating_move(&mut self) {
        if let Some(drag) = self.floating_move.take()
            && let Some(window) = self.demo.workspace.floating_window_mut(drag.window)
        {
            window.placement = drag.placement;
        }
    }

    pub(super) fn update_floating_move(&mut self) {
        let (Some(drag), Some(workspace)) = (self.floating_move, self.workspace_rect) else {
            return;
        };
        if let Some(window) = self.demo.workspace.floating_window_mut(drag.window) {
            window.placement.x = (drag.placement.x + self.cursor.x - drag.origin.x)
                .clamp(0.0, (workspace.width - drag.placement.width).max(0.0));
            window.placement.y = (drag.placement.y + self.cursor.y - drag.origin.y)
                .clamp(0.0, (workspace.height - drag.placement.height).max(0.0));
            self.request_redraw();
        }
    }

    pub(super) fn wheel(&mut self, delta: MouseScrollDelta) {
        let mut movement = match delta {
            MouseScrollDelta::LineDelta(x, y) => Point::new(-x * 34.0, -y * 34.0),
            MouseScrollDelta::PixelDelta(position) => {
                let scale = self
                    .window
                    .as_ref()
                    .map_or(1.0, |window| window.scale_factor());
                let point = logical_point(position, scale);
                Point::new(-point.x, -point.y)
            }
        };
        if self.text_bridge.modifiers().shift_key() && movement.x.abs() <= f32::EPSILON {
            movement.x = movement.y;
            movement.y = 0.0;
        }
        if self.mode == AcceptanceMode::Gallery2d {
            if movement.x.is_finite()
                && movement.y.is_finite()
                && (movement.x.abs() > f32::EPSILON || movement.y.abs() > f32::EPSILON)
            {
                retire_caret_reveal(&mut self.caret_reveal, STANDALONE_WINDOW);
            }
            if let Some(view) = self.area_view
                && view.bounds.contains(self.cursor)
                && self.hit_test.hit(self.cursor) == Some(self.scopes.gallery.element(TEXT_AREA))
                && movement.y.abs() > f32::EPSILON
            {
                restore_area_view_layout(&mut self.area_state, &mut self.text_context, view);
                let limit = (self.area_state.snapshot().layout_height()
                    - (view.bounds.height - 4.0).max(0.0))
                .max(0.0);
                let next = (self.area_scroll_y + movement.y).clamp(0.0, limit);
                let consumed = next - self.area_scroll_y;
                if consumed.abs() > f32::EPSILON {
                    self.area_scroll_y = next;
                    self.area_follow_caret = false;
                    movement.y -= consumed;
                }
            }
            if let Some(&(pane, _)) = self
                .scroll_geometry
                .tabs
                .values()
                .find(|(_, rect)| rect.contains(self.cursor))
            {
                let horizontal = if movement.x.abs() > f32::EPSILON {
                    movement.x
                } else {
                    movement.y
                };
                self.demo
                    .tab_scroll
                    .entry(pane)
                    .or_default()
                    .scroll_by_point(Point::new(horizontal, 0.0));
                self.request_redraw();
                return;
            }
            let gallery = self.scopes.gallery;
            let hit = self.hit_test.hit(self.cursor);
            let geometry = self.scroll_geometry.clone();
            let mut states = ScrollStates {
                tree: &mut self.demo.tree_scroll,
                virtual_list: &mut self.demo.virtual_scroll,
                virtual_table: &mut self.demo.virtual_table_scroll,
                nested: &mut self.demo.nested_scroll,
                horizontal: &mut self.demo.horizontal_scroll,
                gallery: Some(&mut self.gallery_scroll),
                pages: &mut self.demo.page_scroll,
                tabs: &mut self.demo.tab_scroll,
            };
            wheel_scroll_path(
                STANDALONE_WINDOW,
                &self.ui_actions,
                &mut states,
                hit,
                &geometry,
                gallery,
                movement,
            );
        } else if self.hit_test.hit(self.cursor).is_none() {
            apply_action(
                &mut self.camera,
                ViewportAction::Dolly {
                    delta: -movement.y * 0.02,
                },
            );
        }
        self.request_redraw();
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct VectorScrub {
    pub(super) axis: u8,
    pub(super) session: HorizontalEditSession<Option<f32>>,
}

impl std::ops::Deref for VectorScrub {
    type Target = HorizontalEditSession<Option<f32>>;
    fn deref(&self) -> &Self::Target {
        &self.session
    }
}

impl std::ops::DerefMut for VectorScrub {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.session
    }
}

/// Starts a scrollbar thumb drag for the hit window, using that window's own viewport rects.
pub(super) fn begin_scrollbar_drag(
    gallery: ElementScope,
    geometry: &ScrollGeometry,
    actions: &ActionRoute<DemoAction>,
    states: &mut ScrollStates<'_>,
    hit: Option<ElementId>,
    pointer: Point,
) -> Option<(ElementId, ScrollbarDrag)> {
    let hit = hit?;
    let scope = FocusScope::new(STANDALONE_WINDOW, actions, None);
    let path = vesta_gpui_widgets::ScrollInputPath::new(
        &scope,
        InputTarget {
            window: STANDALONE_WINDOW,
            element: hit,
        },
    )?;
    let targets = geometry.targets(gallery);
    path.begin_drag(pointer, |element, pointer| {
        let (_, rect, target) = targets
            .iter()
            .find(|(candidate, _, _)| *candidate == element)?;
        let state = states.for_target(*target)?;
        if let Some(node) = geometry.nodes.get(&element) {
            state.sync_layout(node);
            state.begin_thumb_drag_in_layout(node, pointer)
        } else {
            state.begin_thumb_drag(*rect, pointer)
        }
    })
}

/// Only the thumb's primary-button release can end its session.
pub(super) fn finish_scrollbar_drag(
    session: &mut Option<(ElementId, ScrollbarDrag)>,
    button: MouseButton,
) -> bool {
    button == MouseButton::Left && session.take().is_some()
}

/// Applies one pointer move to a scrollbar drag through the window that owns it.
pub(super) fn update_scrollbar_drag_state(
    gallery: ElementScope,
    geometry: &ScrollGeometry,
    states: &mut ScrollStates<'_>,
    element: ElementId,
    drag: ScrollbarDrag,
    pointer: Point,
) -> bool {
    for (candidate, _, target) in geometry.targets(gallery) {
        if candidate != element {
            continue;
        }
        let Some(state) = states.for_target(target) else {
            return false;
        };
        if let Some(node) = geometry.nodes.get(&element) {
            state.sync_layout(node);
        }
        return drag.update(state, pointer);
    }
    false
}

/// One wheel step, shared by every window: movement is consumed along the action path from
/// the innermost scroll element outwards, so an inner list with range scrolls before its
/// ancestors. Returns the movement no viewport consumed.
pub(super) fn wheel_scroll_path(
    window: UiWindowId,
    actions: &ActionRoute<DemoAction>,
    states: &mut ScrollStates<'_>,
    hit: Option<ElementId>,
    geometry: &ScrollGeometry,
    gallery: ElementScope,
    movement: Point,
) -> Point {
    let root = gallery.element(3);
    let targets = geometry
        .targets(gallery)
        .into_iter()
        .map(|(element, _, target)| (element, target))
        .collect::<Vec<_>>();
    let target = hit
        .filter(|hit| actions.is_descendant_of(*hit, root))
        .unwrap_or(root);
    vesta_gpui_widgets::scroll_wheel_path(
        &FocusScope::new(window, actions, Some(root)),
        InputTarget {
            window,
            element: target,
        },
        movement,
        |element, mut remaining| {
            if let Some((_, scroll_target)) = targets.iter().find(|(id, _)| *id == element)
                && let Some(state) = states.for_target(*scroll_target)
            {
                remaining = state.scroll_by_point(remaining);
            }
            remaining
        },
    )
}
