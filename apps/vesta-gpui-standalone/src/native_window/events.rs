//! Native event routing for owned floating windows.
use super::*;

impl App {
    pub(crate) fn native_window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.unavailable_input(window_id, &event) {
            return;
        }
        if let Some(window) = self
            .native_windows
            .get(&window_id)
            .map(|host| host.ui_window)
        {
            self.observe_text_pointer_event(window, &event);
        }
        if let Some(host) = self.native_windows.get_mut(&window_id) {
            if vesta_gpui_winit::observe_focus_visibility(&event, &mut host.input) {
                host.window.request_redraw();
            }
            if self.ui_query.input_origin == vesta_gpui_winit::UiInputOrigin::Native
                && let WindowEvent::MouseInput { state, button, .. } = &event
            {
                self.frame_profile.observe_native(|| {
                    let scale = host.window.scale_factor();
                    NativeObservation::Pointer {
                        window: host.ui_window,
                        state: *state,
                        button: *button,
                        logical: host.cursor,
                        derived_physical: (
                            f64::from(host.cursor.x) * scale,
                            f64::from(host.cursor.y) * scale,
                        ),
                        hit: host.hit_test.hit(host.cursor),
                    }
                });
            }
        }
        if let WindowEvent::ModifiersChanged(modifiers) = &event
            && let Some(bridge) = self
                .native_windows
                .get(&window_id)
                .and_then(|host| self.native_text_bridges.get_mut(host.ui_window))
        {
            bridge.update_modifiers(modifiers);
        }
        // A window that owns the caret consumes its own key and IME events, exactly like the
        // main window; without this the bridge would never see them (ADR-0252).
        let host_ui_window = self
            .native_windows
            .get(&window_id)
            .map(|host| host.ui_window);
        if let Some(ui_window) = host_ui_window {
            self.action_window = ui_window;
        }
        if let Some(window) = host_ui_window
            && (self.handle_palette_scroll_event(window, &event)
                || self.handle_suggestion_scroll_event(window, &event))
        {
            self.request_all_redraw();
            return;
        }
        if let Some(ui_window) = host_ui_window
            && self.handle_window_document_event(ui_window, &event)
        {
            self.request_all_redraw();
            return;
        }
        if let Some(ui_window) = host_ui_window
            && self.handle_window_keyboard_event(ui_window, &event)
        {
            self.request_all_redraw();
            if self.exit_requested {
                _event_loop.exit();
            }
            return;
        }
        if let Some(ui_window) = host_ui_window
            && matches!(
                event,
                WindowEvent::KeyboardInput { .. } | WindowEvent::Ime(_) | WindowEvent::Text(_)
            )
            && self.text_session_field(ui_window).is_some()
        {
            self.handle_window_text_event(ui_window, &event);
            if let Some(host) = self.native_windows.get(&window_id) {
                host.window.request_redraw();
            }
            return;
        }
        let menu_hover = matches!(&event, WindowEvent::CursorMoved { .. });
        let mut begin_native_menu = None;
        let mut action = None;
        let mut restore = false;
        let mut redraw = false;
        let mut placement = None;
        let mut drag_update = None;
        let mut split_resize_update = None;
        let mut finish_split_resize = None;
        let mut begin_drag = None;
        let mut finish_drag = false;
        let mut ramp_redraw = false;
        let mut finish_ramp = None;
        let mut cancel_values_window = None;
        let mut dismiss_popover = false;
        let mut dismiss_combo = false;
        let mut close_palette = false;
        // Precomputed before any host borrow: the drawer geometry only reads shared state.
        let drawer_state = self.drawer_state();
        let mut begin_text = None;
        let mut text_drag_update = None;
        let mut text_exit = None;
        let mut number_draft_dirty = false;
        let mut vector_draft_axis = None;
        let drag_active = self.workspace_drag.is_some();
        let source_container = self
            .native_windows
            .get(&window_id)
            .map(|host| ContainerRef::Floating(host.logical_id));
        match event {
            WindowEvent::CloseRequested => restore = true,
            WindowEvent::Focused(false) => {
                if let Some(window) = host_ui_window {
                    let focused = self.window_focus(window);
                    self.toast_focus.observe_window_activation(
                        window,
                        false,
                        focused.map(|element| InputTarget { window, element }),
                    );
                }
                if host_ui_window.is_some_and(|window| {
                    self.is_password_control(window, self.window_focus(window))
                }) {
                    self.demo.password_revealed = false;
                }
                if let Some(host) = self.native_windows.get_mut(&window_id) {
                    host.tooltip.set_active(false, Instant::now());
                    cancel_values_window = Some(host.ui_window);
                    text_exit = Some(TextSessionExit::WindowBlur(host.ui_window));
                }
                redraw = true;
            }
            WindowEvent::Focused(true) => {
                if let Some(host) = self.native_windows.get_mut(&window_id) {
                    self.toast_focus
                        .observe_window_activation(host.ui_window, true, None);
                    host.tooltip.set_active(true, Instant::now());
                }
                self.native_window_order.retain(|id| *id != window_id);
                self.native_window_order.push(window_id);
                redraw = true;
            }
            WindowEvent::Resized(size) => {
                finish_split_resize = host_ui_window.map(|window| (window, false));
                if let Some(window) = host_ui_window {
                    self.cancel_table_resize_for(window);
                    self.cancel_window_scrollbar_drag(window);
                }
                if let Some(host) = self.native_windows.get_mut(&window_id) {
                    host.surface.resize(size.width, size.height);
                }
                placement = self.native_placement_for_event(window_id, &event);
                redraw = true;
            }
            WindowEvent::Moved(_) => {
                placement = self.native_placement_for_event(window_id, &event);
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(window) = host_ui_window {
                    self.cancel_table_resize_for(window);
                    self.cancel_window_scrollbar_drag(window);
                }
                if let Some(host) = self.native_windows.get_mut(&window_id) {
                    let size = host.window.inner_size();
                    host.surface.resize(size.width, size.height);
                }
                redraw = true;
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(host) = self.native_windows.get_mut(&window_id) {
                    let logical = position.to_logical::<f32>(host.window.scale_factor());
                    host.cursor = Point::new(logical.x, logical.y);
                    host.tooltip.hover(
                        host.hit_test.hit(host.cursor),
                        &host.actions,
                        Instant::now(),
                    );
                    let size = host
                        .window
                        .inner_size()
                        .to_logical::<f32>(host.window.scale_factor());
                    let resize = (!host.window.is_maximized() && cfg!(target_os = "windows"))
                        .then(|| {
                            chrome_resize_direction(host.cursor, Size::new(size.width, size.height))
                        })
                        .flatten();
                    host.window
                        .set_cursor(resize.map(CursorIcon::from).unwrap_or(CursorIcon::Default));
                    if let (Some(track), Some(drag)) =
                        (host.ramp_track_rect, host.ramp_drag.as_mut())
                    {
                        ramp_redraw = update_ramp_drag_at(
                            drag,
                            host.ui_window,
                            track,
                            host.cursor.x,
                            &mut self.demo.ramp,
                        ) || ramp_redraw;
                    }
                    if host
                        .slider_drag
                        .as_ref()
                        .is_some_and(|edit| edit.accepts(host.ui_window))
                        && let Some(rect) = host.slider_rect
                    {
                        let value = slider_value_at(rect, host.cursor).clamp(0.0, 1.0);
                        if value != self.demo.slider {
                            self.demo.slider = value;
                            ramp_redraw = true;
                        }
                    }
                    if let Some(mut scrub) = host.number_scrub {
                        let next = number_scrub_value(
                            &mut scrub,
                            host.ui_window,
                            host.cursor.x,
                            self.demo.number,
                        );
                        host.number_scrub = Some(scrub);
                        if let Some(value) = next {
                            self.demo.number = value;
                            number_draft_dirty = true;
                            ramp_redraw = true;
                        }
                    }
                    if let Some(mut scrub) = host.vector_scrub {
                        let current = self.demo.vector[usize::from(scrub.axis)];
                        let next =
                            vector_scrub_value(&mut scrub, host.ui_window, host.cursor.x, current);
                        host.vector_scrub = Some(scrub);
                        if let Some(value) = next {
                            self.demo.vector[usize::from(scrub.axis)] = Some(value);
                            vector_draft_axis = Some(scrub.axis);
                            ramp_redraw = true;
                        }
                    }
                    if let Some(mut drag) = host.drawer_resize
                        && let Some(delta) = drag.offset(host.ui_window, host.cursor.x)
                    {
                        host.drawer_resize = Some(drag);
                        let width = sidebar_drawer_width_at(*drag.initial(), delta, drawer_state);
                        if (width - self.demo.drawer_width).abs() > f32::EPSILON {
                            self.demo.drawer_width = width;
                            ramp_redraw = true;
                        }
                    }
                    if let Some(mut drag) = host.table_resize
                        && let Some(rect) = host.table_rect
                        && let Some(delta) = drag.session.offset(host.ui_window, host.cursor.x)
                    {
                        host.table_resize = Some(drag);
                        let table_width = rect.width.max(1.0);
                        if let Some(spec) = table_resize_width_bounds(
                            &self.demo.table_column_fractions,
                            table_width,
                            drag.column,
                        ) && let Some(initial_widths) =
                            table_column_widths(drag.session.initial(), table_width)
                        {
                            let start_width = initial_widths[drag.column];
                            let spec = TableColumnResize {
                                handle: WidgetIdentity::new(
                                    table_resize_element(host.scopes.gallery, drag.column),
                                    format!("gallery.table.column.{}.resize", drag.column),
                                ),
                                min_width: spec.0,
                                max_width: spec.1,
                                keyboard_step: 8.0,
                            };
                            let width = table_column_width_at(start_width, delta, &spec);
                            if let Some(next) = table_column_fractions_after(
                                &self.demo.table_column_fractions,
                                table_width,
                                drag.column,
                                width,
                            ) {
                                self.demo.table_column_fractions = next;
                                ramp_redraw = true;
                            }
                        }
                    }
                    if let Some((element, drag)) = host.scrollbar_drag {
                        let mut states = ScrollStates {
                            tree: &mut self.demo.tree_scroll,
                            virtual_list: &mut self.demo.virtual_scroll,
                            virtual_table: &mut self.demo.virtual_table_scroll,
                            nested: &mut self.demo.nested_scroll,
                            horizontal: &mut self.demo.horizontal_scroll,
                            gallery: None,
                            pages: &mut self.demo.page_scroll,
                            tabs: &mut self.demo.tab_scroll,
                        };
                        if update_scrollbar_drag_state(
                            host.scopes.gallery,
                            &host.scroll_geometry,
                            &mut states,
                            element,
                            drag,
                            host.cursor,
                        ) {
                            ramp_redraw = true;
                        }
                    }
                    text_drag_update = Some((host.ui_window, host.cursor));
                    split_resize_update = Some((host.ui_window, host.cursor));
                    drag_update = Some((ContainerRef::Floating(host.logical_id), host.cursor));
                }
                redraw = true;
            }
            WindowEvent::CursorLeft { .. } => {
                if let Some(host) = self.native_windows.get_mut(&window_id) {
                    host.pointer_click.cancel();
                    host.cursor = Point::new(-1.0, -1.0);
                    host.tooltip.leave(Instant::now());
                }
                redraw = true;
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                if let Some(host) = self.native_windows.get_mut(&window_id) {
                    host.pointer_click.cancel();
                    let tab = host
                        .hit_test
                        .hit(host.cursor)
                        .and_then(|hit| host.actions.action_for_hit(hit))
                        .and_then(|action| match action {
                            DemoAction::Workspace(WorkspaceViewAction::Tab(item)) => Some(*item),
                            _ => None,
                        });
                    begin_native_menu = Some((host.ui_window, tab, host.cursor));
                }
                redraw = true;
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(host) = self.native_windows.get_mut(&window_id) {
                    let hit = host.hit_test.hit(host.cursor);
                    let scrollbar = {
                        let mut states = ScrollStates {
                            tree: &mut self.demo.tree_scroll,
                            virtual_list: &mut self.demo.virtual_scroll,
                            virtual_table: &mut self.demo.virtual_table_scroll,
                            nested: &mut self.demo.nested_scroll,
                            horizontal: &mut self.demo.horizontal_scroll,
                            gallery: None,
                            pages: &mut self.demo.page_scroll,
                            tabs: &mut self.demo.tab_scroll,
                        };
                        begin_scrollbar_drag(
                            host.scopes.gallery,
                            &host.scroll_geometry,
                            &host.actions,
                            &mut states,
                            hit,
                            host.cursor,
                        )
                    };
                    // A painted thumb may overlap the frameless resize strip.
                    if scrollbar.is_none()
                        && self.ui_query.input_origin == vesta_gpui_winit::UiInputOrigin::Native
                        && native_window::try_window_chrome_press(
                            &host.window,
                            host.cursor,
                            &host.hit_test,
                            &host.actions,
                            &mut host.pointer_click,
                            &mut host.last_title_click,
                        )
                    {
                        return;
                    }
                    if host.pane_menu.is_some()
                        && !hit.is_some_and(|hit| {
                            host.actions
                                .is_descendant_of(hit, host.scopes.menu.element(1))
                        })
                    {
                        host.close_pane_menu();
                    }
                    // Same non-modal rule as the main window (ADR-0140): a press outside the
                    // overlay closes it while keeping its own action, and a press inside it is
                    // left to the routed action below.
                    dismiss_popover = self.demo.popover.opener().is_some_and(|opener| {
                        popup_dismiss_for_hit(
                            opener,
                            host.scopes.popover.element(POPOVER_ROOT),
                            host.ui_window,
                            hit,
                            &host.actions,
                        )
                    });
                    dismiss_combo = self.demo.combo.opener().is_some_and(|opener| {
                        popup_dismiss_for_hit(
                            opener,
                            host.scopes.gallery.element(COMBO_POPUP),
                            host.ui_window,
                            hit,
                            &host.actions,
                        )
                    });
                    let routed = hit
                        .and_then(|hit| host.actions.target_for_hit(hit))
                        .map(|(element, action)| (element, *action));
                    let ramp_point = hit.and_then(|hit| ramp_point_action(&host.actions, hit));
                    let text_field = hit.and_then(|hit| {
                        TextFieldKind::ALL
                            .into_iter()
                            .find(|field| field.element(host.scopes.gallery) == hit)
                    });
                    // Scrollbars first, like the main window: the thumb owns the press when the
                    // pointer is on it. A palette owned by this window closes on its backdrop
                    // before anything else (ADR-0254).
                    if self.demo.palette_session.is_open()
                        && self.overlay_owner == Some(host.ui_window)
                        && hit == Some(host.scopes.root.element(PALETTE_ROOT))
                    {
                        close_palette = true;
                    } else if let Some((element, drag)) = scrollbar {
                        host.input.set_focus(Some(InputTarget {
                            window: host.ui_window,
                            element,
                        }));
                        host.input.capture_pointer(
                            InputTarget {
                                window: host.ui_window,
                                element,
                            },
                            PointerButton::Primary,
                        );
                        host.scrollbar_drag = Some((element, drag));
                        text_exit = Some(TextSessionExit::FocusChange);
                    } else if let Some(field) = text_field {
                        begin_text = Some((host.ui_window, field, host.cursor));
                        match field {
                            TextFieldKind::Number => {
                                host.number_scrub = NumberScrub::new(
                                    InputTarget {
                                        window: host.ui_window,
                                        element: host.scopes.gallery.element(NUMBER_FIELD),
                                    },
                                    host.cursor.x,
                                    self.demo.number,
                                    4.0,
                                )
                                .ok();
                            }
                            TextFieldKind::Vector(axis) => {
                                host.vector_scrub =
                                    self.demo.vector.get(usize::from(axis)).and_then(|value| {
                                        Some(VectorScrub {
                                            axis,
                                            session: HorizontalEditSession::new(
                                                InputTarget {
                                                    window: host.ui_window,
                                                    element: host.scopes.gallery.element(
                                                        VECTOR_VALUE_BASE + u32::from(axis),
                                                    ),
                                                },
                                                host.cursor.x,
                                                *value,
                                                4.0,
                                            )
                                            .ok()?,
                                        })
                                    });
                            }
                            _ => {}
                        }
                    } else if hit == Some(host.scopes.gallery.element(DRAWER_RESIZE))
                        && self.demo.drawer_open
                    {
                        let element = host.scopes.gallery.element(DRAWER_RESIZE);
                        host.input.set_focus(Some(InputTarget {
                            window: host.ui_window,
                            element,
                        }));
                        host.input.capture_pointer(
                            InputTarget {
                                window: host.ui_window,
                                element,
                            },
                            PointerButton::Primary,
                        );
                        host.drawer_resize = DrawerResizeDrag::new(
                            InputTarget {
                                window: host.ui_window,
                                element,
                            },
                            host.cursor.x,
                            self.demo.drawer_width,
                            0.0,
                        )
                        .ok();
                        text_exit = Some(TextSessionExit::FocusChange);
                    } else if let Some(column) = (0..2).find(|column| {
                        hit == Some(table_resize_element(host.scopes.gallery, *column))
                    }) {
                        if let Some(rect) = host.table_rect
                            && table_resize_width_bounds(
                                &self.demo.table_column_fractions,
                                rect.width.max(1.0),
                                column,
                            )
                            .is_some()
                        {
                            let element = table_resize_element(host.scopes.gallery, column);
                            host.input.set_focus(Some(InputTarget {
                                window: host.ui_window,
                                element,
                            }));
                            host.input.capture_pointer(
                                InputTarget {
                                    window: host.ui_window,
                                    element,
                                },
                                PointerButton::Primary,
                            );
                            host.table_resize = HorizontalEditSession::new(
                                InputTarget {
                                    window: host.ui_window,
                                    element,
                                },
                                host.cursor.x,
                                self.demo.table_column_fractions,
                                0.0,
                            )
                            .ok()
                            .map(|session| TableResizeDrag { column, session });
                            text_exit = Some(TextSessionExit::FocusChange);
                        }
                    } else if let Some((element, id)) = ramp_point
                        && let Some(track) = host.ramp_track_rect
                    {
                        let drag = press_ramp_point(
                            &mut self.demo.ramp,
                            InputTarget {
                                window: host.ui_window,
                                element,
                            },
                            track,
                            id,
                            host.cursor.x,
                            SelectionModifiers {
                                additive: self
                                    .native_text_bridges
                                    .get(host.ui_window)
                                    .is_some_and(|bridge| bridge.modifiers().control_key()),
                                range: self
                                    .native_text_bridges
                                    .get(host.ui_window)
                                    .is_some_and(|bridge| bridge.modifiers().shift_key()),
                            },
                        );
                        if drag.is_some() {
                            host.input.capture_pointer(
                                InputTarget {
                                    window: host.ui_window,
                                    element,
                                },
                                PointerButton::Primary,
                            );
                        }
                        host.pointer_click.cancel();
                        host.input.set_focus(Some(InputTarget {
                            window: host.ui_window,
                            element,
                        }));
                        host.ramp_drag = drag;
                        text_exit = Some(TextSessionExit::FocusChange);
                        ramp_redraw = true;
                    } else if let Some((element, DemoAction::SetSlider)) = routed {
                        // The track starts its drag on press, exactly like the main window.
                        if let Some(rect) = host.slider_rect {
                            host.slider_drag = Some(ValueEditSession::new(
                                InputTarget {
                                    window: host.ui_window,
                                    element,
                                },
                                self.demo.slider,
                            ));
                            host.input.set_focus(Some(InputTarget {
                                window: host.ui_window,
                                element,
                            }));
                            host.input.capture_pointer(
                                InputTarget {
                                    window: host.ui_window,
                                    element,
                                },
                                PointerButton::Primary,
                            );
                            self.demo.slider = slider_value_at(rect, host.cursor).clamp(0.0, 1.0);
                            ramp_redraw = true;
                        } else {
                            host.pointer_click.press(
                                host.ui_window,
                                PointerButton::Primary,
                                &host.actions,
                                hit,
                            );
                        }
                    } else if let Some((element, routed_action)) = routed {
                        host.input.set_focus(Some(InputTarget {
                            window: host.ui_window,
                            element,
                        }));
                        if let DemoAction::Workspace(WorkspaceViewAction::Tab(item)) = routed_action
                        {
                            host.pointer_click.cancel();
                            begin_drag = Some((
                                item,
                                ContainerRef::Floating(host.logical_id),
                                host.cursor,
                                routed_action,
                            ));
                        } else if routed_action.starts_pointer_drag() {
                            host.pointer_click.cancel();
                            action = Some(routed_action);
                        } else {
                            host.pointer_click.press(
                                host.ui_window,
                                PointerButton::Primary,
                                &host.actions,
                                hit,
                            );
                        }
                    } else {
                        host.pointer_click.press(
                            host.ui_window,
                            PointerButton::Primary,
                            &host.actions,
                            hit,
                        );
                    }
                }
                redraw = true;
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(host) = self.native_windows.get_mut(&window_id) {
                    finish_split_resize = Some((host.ui_window, true));
                    let hit = host.hit_test.hit(host.cursor);
                    action = host
                        .pointer_click
                        .release(host.ui_window, PointerButton::Primary, &host.actions, hit)
                        .copied();
                    // Without this the window keeps the capture after the button is up, so a
                    // text selection or ramp drag would keep following the pointer.
                    host.input
                        .release_pointer(host.ui_window, PointerButton::Primary);
                    if let Some(mut edit) = host.slider_drag.take() {
                        edit.finish(host.ui_window, &self.demo.slider, true);
                    }
                    if let Some(mut scrub) = host.number_scrub.take() {
                        scrub.finish(host.ui_window, &self.demo.number, true);
                    }
                    if let Some(mut scrub) = host.vector_scrub.take() {
                        let current = self.demo.vector[usize::from(scrub.axis)];
                        scrub.finish(host.ui_window, &current, true);
                    }
                    if let Some(mut drag) = host.drawer_resize.take() {
                        drag.finish(host.ui_window, &self.demo.drawer_width, true);
                    }
                    if let Some(mut drag) = host.table_resize.take() {
                        drag.session.finish(
                            host.ui_window,
                            &self.demo.table_column_fractions,
                            true,
                        );
                    }
                    finish_scrollbar_drag(&mut host.scrollbar_drag, MouseButton::Left);
                    if let Some(mut drag) = host.ramp_drag.take()
                        && let Some(PointerEditOutcome::Commit { before, .. }) = drag
                            .session
                            .finish(host.ui_window, &self.demo.ramp.snapshot(), true)
                    {
                        finish_ramp = Some(before);
                    }
                }
                finish_drag = drag_active;
                redraw = true;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // Same inner-to-outer consumption as the main window, through this window's
                // own scroll geometry (ADR-0253 decision 7).
                let mut movement = match delta {
                    MouseScrollDelta::LineDelta(x, y) => Point::new(-x * 34.0, -y * 34.0),
                    MouseScrollDelta::PixelDelta(position) => {
                        let scale = self
                            .native_windows
                            .get(&window_id)
                            .map_or(1.0, |host| host.window.scale_factor());
                        let point = logical_point(position, scale);
                        Point::new(-point.x, -point.y)
                    }
                };
                if host_ui_window.is_some_and(|window| self.window_modifiers(window).shift_key())
                    && movement.x.abs() <= f32::EPSILON
                {
                    movement.x = movement.y;
                    movement.y = 0.0;
                }
                // The TextArea viewport and the Pane tab strip belong to this window, so they
                // take the wheel before the shared scroll path (ADR-0253 decision 7).
                if movement.x.is_finite()
                    && movement.y.is_finite()
                    && (movement.x.abs() > f32::EPSILON || movement.y.abs() > f32::EPSILON)
                    && let Some(window) = host_ui_window
                {
                    retire_caret_reveal(&mut self.caret_reveal, window);
                }
                if let Some(host) = self.native_windows.get(&window_id)
                    && let Some(view) = host.text_views.get(TextFieldKind::Area)
                    && view.bounds.contains(host.cursor)
                    && host.hit_test.hit(host.cursor)
                        == Some(host.scopes.gallery.element(TEXT_AREA))
                    && movement.y.abs() > f32::EPSILON
                {
                    restore_area_view_layout(&mut self.area_state, &mut self.text_context, view);
                    let limit = (self.area_state.snapshot().layout_height()
                        - (view.bounds.height - 4.0).max(0.0))
                    .max(0.0);
                    let next = (host.area_scroll_y + movement.y).clamp(0.0, limit);
                    let consumed = next - host.area_scroll_y;
                    if consumed.abs() > f32::EPSILON {
                        if let Some(host) = self.native_windows.get_mut(&window_id) {
                            host.area_scroll_y = next;
                            host.area_follow_caret = false;
                        }
                        movement.y -= consumed;
                    }
                }
                let tab_strip = self.native_windows.get(&window_id).and_then(|host| {
                    host.scroll_geometry
                        .tabs
                        .values()
                        .find(|(_, rect)| rect.contains(host.cursor))
                        .map(|&(pane, _)| pane)
                });
                if let Some(pane) = tab_strip {
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
                    if let Some(host) = self.native_windows.get(&window_id) {
                        host.window.request_redraw();
                    }
                }
                if tab_strip.is_none()
                    && let Some(host) = self.native_windows.get(&window_id)
                {
                    let gallery = host.scopes.gallery;
                    let hit = host.hit_test.hit(host.cursor);
                    let geometry = host.scroll_geometry.clone();
                    let mut states = ScrollStates {
                        tree: &mut self.demo.tree_scroll,
                        virtual_list: &mut self.demo.virtual_scroll,
                        virtual_table: &mut self.demo.virtual_table_scroll,
                        nested: &mut self.demo.nested_scroll,
                        horizontal: &mut self.demo.horizontal_scroll,
                        gallery: None,
                        pages: &mut self.demo.page_scroll,
                        tabs: &mut self.demo.tab_scroll,
                    };
                    wheel_scroll_path(
                        host.ui_window,
                        &host.actions,
                        &mut states,
                        hit,
                        &geometry,
                        gallery,
                        movement,
                    );
                }
                redraw = true;
            }
            WindowEvent::KeyboardInput {
                event: key_event,
                is_synthetic: false,
                ..
            } if key_event.state == ElementState::Pressed => {
                // Shared routing has handled controls, text barriers and shortcuts.
                // An unclaimed Escape retains the floating host's Dock-back convention.
                if key_event.logical_key == Key::Named(NamedKey::Escape) {
                    restore = true;
                }
            }
            WindowEvent::RedrawRequested => self.redraw_native_window(window_id),
            _ => {}
        }
        if let Some((window, tab, anchor)) = begin_native_menu
            && let Some(context) = self.pane_context_at(window, tab)
        {
            self.open_window_menu(
                window,
                if tab.is_some() { MenuId(6) } else { MenuId(1) },
                anchor,
                context,
            );
        }
        if menu_hover && let Some(host) = self.native_windows.get(&window_id) {
            self.hover_window_menu_item(host.ui_window, host.hit_test.hit(host.cursor));
        }
        if let Some((item, container, origin, activate)) = begin_drag {
            self.workspace_drag = Some(WorkspaceTabDrag {
                item,
                origin_container: container,
                origin,
                started: false,
                pointer_container: Some(container),
                cursor: origin,
                target: None,
            });
            self.apply_demo_action(activate);
            self.request_all_redraw();
        }
        if let Some((container, cursor)) = drag_update
            && self.update_native_dock_pointer(container, cursor)
        {
            self.request_all_redraw();
        }
        if let Some((window, pointer)) = split_resize_update
            && self.update_workspace_resize(window, pointer)
        {
            self.request_all_redraw();
        }
        if let Some((window, commit)) = finish_split_resize
            && self.finish_workspace_resize_for(window, commit)
        {
            action = None;
            self.request_all_redraw();
        }
        if finish_drag && source_container.is_some_and(|source| self.finish_workspace_drag(source))
        {
            action = None;
        }
        if let Some(snapshot) = finish_ramp {
            self.demo.ramp.commit_snapshot(snapshot);
            ramp_redraw = true;
        }
        if dismiss_popover {
            self.close_popover_for(self.action_window, PopupCloseReason::FocusTransfer);
        }
        if dismiss_combo {
            self.close_combo_for(self.action_window, PopupCloseReason::FocusTransfer);
        }
        if close_palette {
            // The modal backdrop consumes this press; use the main window's cancellation
            // path so focus returns instead of preserving a non-modal outside target.
            self.close_command_palette();
            self.request_all_redraw();
        }
        if let Some((window, field, pointer)) = begin_text {
            self.begin_text_selection(window, field, Some(pointer));
            self.request_all_redraw();
        }
        if let Some((window, pointer)) = text_drag_update
            && self.update_window_text_selection(window, pointer)
        {
            self.request_all_redraw();
        }
        if let Some(window) = cancel_values_window {
            if matches!(text_exit, Some(TextSessionExit::WindowBlur(_))) {
                self.cancel_window_editors_for(window, PopupCloseReason::FocusTransfer);
            } else {
                self.cancel_window_value_edits(window);
            }
        }
        self.complete_window_text_event(text_exit, number_draft_dirty, vector_draft_axis);
        if ramp_redraw {
            self.request_all_redraw();
        }
        if let Some((logical_id, placement)) = placement
            && placement.x.is_finite()
            && placement.y.is_finite()
            && placement.width.is_finite()
            && placement.height.is_finite()
            && placement.width >= 1.0
            && placement.height >= 1.0
            && let Some(window) = self.demo.workspace.floating_window_mut(logical_id)
        {
            window.placement = placement;
        }
        if restore {
            self.restore_native_window(window_id);
            return;
        }
        if let Some(action) = action {
            self.apply_native_action(window_id, action);
            self.request_all_redraw();
        } else if redraw && let Some(host) = self.native_windows.get(&window_id) {
            host.window.request_redraw();
        }
    }
}
