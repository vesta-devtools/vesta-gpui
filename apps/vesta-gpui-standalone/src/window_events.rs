//! Main native event dispatch; floating windows retain their existing adapter.
use super::*;

impl App {
    pub(super) fn dispatch_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if !self.command_input_allowed(window_id, &event) {
            return;
        }
        let owned = self
            .window
            .as_ref()
            .is_some_and(|window| window.id() == window_id)
            || self.native_windows.contains_key(&window_id);
        let deliberate = match &event {
            WindowEvent::MouseInput { state, .. } => *state == ElementState::Pressed,
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => vesta_gpui_winit::is_user_key_press(event.state, *is_synthetic),
            _ => false,
        };
        if owned && deliberate {
            self.dock_focus = None;
        }
        if self.dock_focus.is_some() && matches!(event, WindowEvent::RedrawRequested) {
            // Retire an emptied source HWND before the destination acquires focus.
            self.sync_native_windows(event_loop);
        }
        if self
            .window
            .as_ref()
            .is_none_or(|window| window.id() != window_id)
        {
            if self.native_windows.contains_key(&window_id) {
                self.native_window_event(event_loop, window_id, event);
            }
            return;
        }
        if self.recovery_input(window_id, &event) {
            return;
        }
        // Input in the main window always owns the overlays it opens (ADR-0254).
        self.observe_text_pointer_event(STANDALONE_WINDOW, &event);
        if self.ui_query.input_origin == vesta_gpui_winit::UiInputOrigin::Native
            && let WindowEvent::MouseInput { state, button, .. } = &event
        {
            self.frame_profile.observe_native(|| {
                let scale = self
                    .window
                    .as_ref()
                    .map_or(1.0, |window| window.scale_factor());
                NativeObservation::Pointer {
                    window: STANDALONE_WINDOW,
                    state: *state,
                    button: *button,
                    logical: self.cursor,
                    derived_physical: (
                        f64::from(self.cursor.x) * scale,
                        f64::from(self.cursor.y) * scale,
                    ),
                    hit: self.hit_test.hit(self.cursor),
                }
            });
        }
        self.action_window = STANDALONE_WINDOW;
        if vesta_gpui_winit::observe_focus_visibility(&event, &mut self.input_ownership) {
            self.request_redraw();
        }
        if self.handle_palette_scroll_event(STANDALONE_WINDOW, &event)
            || self.handle_suggestion_scroll_event(STANDALONE_WINDOW, &event)
        {
            self.request_all_redraw();
            return;
        }
        if self.handle_window_document_event(STANDALONE_WINDOW, &event) {
            self.request_all_redraw();
            return;
        }
        if self.handle_window_keyboard_event(STANDALONE_WINDOW, &event) {
            self.request_all_redraw();
            if self.exit_requested {
                event_loop.exit();
            }
            return;
        }
        if self.focused_text_field().is_some() {
            self.handle_window_text_event(STANDALONE_WINDOW, &event);
        } else if let WindowEvent::ModifiersChanged(modifiers) = &event {
            self.text_bridge.update_modifiers(modifiers);
        }
        match event {
            WindowEvent::CloseRequested => {
                self.cancel_window_editors(STANDALONE_WINDOW);
                event_loop.exit();
            }
            WindowEvent::Focused(false) => {
                self.toast_focus.observe_window_activation(
                    STANDALONE_WINDOW,
                    false,
                    self.input_ownership.focus(),
                );
                self.demo.password_revealed = false;
                self.demo.key_capture.cancel_for(STANDALONE_WINDOW);
                self.demo.status_overflow_open = false;
                self.cursor = Point::new(-1.0, -1.0);
                self.hovered = None;
                self.tooltip.set_active(false, Instant::now());
                self.pointer_click.cancel();
                self.cancel_window_editors_for(STANDALONE_WINDOW, PopupCloseReason::FocusTransfer);
                self.last_title_click = None;
                self.cancel_transform_drag();
                self.camera_drag = false;
                self.camera_pan = false;
                self.cancel_floating_move();
                self.update_resize_cursor();
                self.request_all_redraw();
            }
            WindowEvent::Focused(true) => {
                self.toast_focus
                    .observe_window_activation(STANDALONE_WINDOW, true, None);
                self.tooltip.set_active(true, Instant::now());
                self.request_redraw();
            }
            WindowEvent::Resized(size) => {
                self.main_window_resized(size);
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.main_window_scale_factor_changed();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self
                    .window
                    .as_ref()
                    .map_or(1.0, |window| window.scale_factor());
                self.cursor = logical_point(position, scale);
                self.update_resize_cursor();
                let hovered = self.hit_test.hit(self.cursor);
                if self.hovered != hovered {
                    self.hovered = hovered;
                    self.tooltip
                        .hover(hovered, &self.ui_actions, Instant::now());
                    self.request_redraw();
                }
                if self.demo.menu.is_open() {
                    self.hover_menu_item(hovered);
                }
                self.update_scrollbar_drag();
                if self
                    .slider_drag
                    .as_ref()
                    .is_some_and(|edit| edit.accepts(STANDALONE_WINDOW))
                    && let Some(rect) = self.slider_rect
                {
                    self.apply_demo_action(DemoAction::SetSliderValue(slider_value_at(
                        rect,
                        self.cursor,
                    )));
                    self.request_redraw();
                }
                if self.update_number_scrub() {
                    self.request_redraw();
                }
                if self.update_vector_scrub() {
                    self.request_redraw();
                }
                if self.update_ramp_drag() {
                    self.request_redraw();
                }
                if self.update_drawer_resize() {
                    self.request_redraw();
                }
                if self.update_table_resize() {
                    self.request_redraw();
                }
                if let Some(capture) = self.input_ownership.capture() {
                    let field = TextFieldKind::ALL
                        .into_iter()
                        .find(|field| field.element(self.scopes.gallery) == capture.target.element);
                    let view = field.and_then(|field| match field {
                        TextFieldKind::Main => self.text_view,
                        TextFieldKind::Search => self.search_view,
                        TextFieldKind::Area => self.area_view,
                        TextFieldKind::Number => self.number_view,
                        TextFieldKind::Vector(axis) => self.vector_views[usize::from(axis)],
                        TextFieldKind::ColorHex => self.color_hex_view,
                        TextFieldKind::Password => self.password_view,
                        TextFieldKind::TableNote => self.table_note_view,
                        TextFieldKind::Label => self.readonly_label_view,
                    });
                    if let (Some(field), Some(view)) = (field, view)
                        && !(field == TextFieldKind::Number
                            && self.number_scrub.is_some_and(|scrub| scrub.is_dragging()))
                        && !(matches!(field, TextFieldKind::Vector(_))
                            && self.vector_scrub.is_some_and(|scrub| scrub.is_dragging()))
                    {
                        let revealed =
                            field == TextFieldKind::Password && self.demo.password_revealed;
                        let input = match field {
                            TextFieldKind::Main => &mut self.text_state,
                            TextFieldKind::Search => &mut self.search_state,
                            TextFieldKind::Area => &mut self.area_state,
                            TextFieldKind::Number => &mut self.number_state,
                            TextFieldKind::Vector(axis) => {
                                &mut self.vector_states[usize::from(axis)]
                            }
                            TextFieldKind::ColorHex => &mut self.color_hex_state,
                            TextFieldKind::Password => &mut self.password_state,
                            TextFieldKind::TableNote => &mut self.table_note_state,
                            TextFieldKind::Label => &mut self.readonly_label_state,
                        };
                        if revealed {
                            input.point_revealed(
                                &mut self.text_context,
                                view.to_layout(self.cursor),
                                true,
                                false,
                            );
                        } else {
                            input.point(
                                &mut self.text_context,
                                view.to_layout(self.cursor),
                                true,
                                false,
                            );
                        }
                        self.request_redraw();
                    }
                }
                if self.update_workspace_resize(STANDALONE_WINDOW, self.cursor) {
                    self.request_redraw();
                }
                if self.update_native_dock_pointer(ContainerRef::Main, self.cursor) {
                    self.request_all_redraw();
                }
                if self.floating_move.is_some() {
                    self.update_floating_move();
                }
                if self.camera_drag {
                    apply_action(
                        &mut self.camera,
                        ViewportAction::Orbit {
                            delta_x: self.cursor.x - self.last_cursor.x,
                            delta_y: self.cursor.y - self.last_cursor.y,
                        },
                    );
                    self.last_cursor = self.cursor;
                    self.request_redraw();
                }
                if self.camera_pan {
                    apply_action(
                        &mut self.camera,
                        ViewportAction::Pan {
                            delta_x: self.cursor.x - self.last_cursor.x,
                            delta_y: self.cursor.y - self.last_cursor.y,
                        },
                    );
                    self.last_cursor = self.cursor;
                    self.request_redraw();
                }
                if let Some(drag) = self.transform_drag {
                    self.apply_transform_action(drag, drag.update(self.cursor.x, self.cursor.y));
                    self.request_redraw();
                }
                if self.mode == AcceptanceMode::Gallery2d {
                    self.request_redraw();
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor = Point::new(-1.0, -1.0);
                self.hovered = None;
                self.tooltip.leave(Instant::now());
                self.update_resize_cursor();
                if self.workspace_drag.is_none() {
                    self.leave_workspace_drag(ContainerRef::Main);
                }
                self.request_all_redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if !self.try_native_primary_gesture() {
                    self.pointer_press();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } if self.mode == AcceptanceMode::Gallery2d => {
                self.pointer_click.cancel();
                self.open_context_menu(self.cursor);
                self.request_redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Middle,
                ..
            } if self.mode == AcceptanceMode::Viewport3d => {
                self.camera_pan = true;
                self.last_cursor = self.cursor;
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button,
                ..
            } if matches!(button, MouseButton::Left | MouseButton::Middle) => {
                let click_action = if button == MouseButton::Left {
                    self.pointer_click
                        .release(
                            STANDALONE_WINDOW,
                            PointerButton::Primary,
                            &self.ui_actions,
                            self.hit_test.hit(self.cursor),
                        )
                        .copied()
                } else {
                    None
                };
                if button == MouseButton::Left {
                    self.finish_workspace_drag(ContainerRef::Main);
                    self.floating_move = None;
                }
                self.camera_drag = false;
                self.camera_pan = false;
                if button == MouseButton::Left {
                    self.finish_slider_edit();
                }
                let was_number_scrub = button == MouseButton::Left && self.finish_number_scrub();
                let was_vector_scrub = button == MouseButton::Left && self.finish_vector_scrub();
                let was_drawer_resize = button == MouseButton::Left && self.finish_drawer_resize();
                let was_table_resize = button == MouseButton::Left && self.finish_table_resize();
                let was_ramp_drag = button == MouseButton::Left && self.finish_ramp_drag();
                let was_scrollbar_drag = finish_scrollbar_drag(&mut self.scrollbar_drag, button);
                if button == MouseButton::Left
                    && let Some(drag) = self.transform_drag.take()
                {
                    self.apply_transform_action(drag, TransformAction::Commit);
                    self.request_redraw();
                }
                if button == MouseButton::Left {
                    self.input_ownership
                        .release_pointer(STANDALONE_WINDOW, PointerButton::Primary);
                }
                if button == MouseButton::Left {
                    self.finish_workspace_resize_for(STANDALONE_WINDOW, true);
                }
                self.update_resize_cursor();
                if was_scrollbar_drag
                    || was_number_scrub
                    || was_vector_scrub
                    || was_drawer_resize
                    || was_table_resize
                    || was_ramp_drag
                {
                    self.request_redraw();
                }
                if let Some(action) = click_action {
                    self.apply_demo_action(action);
                    self.request_redraw();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => self.wheel(delta),
            WindowEvent::RedrawRequested => {
                let previous_frames = self.rendered_frames;
                self.redraw();
                if self.smoke {
                    // Surface recovery can return before a frame is presented.
                    // Advance scripted actions only after a completed frame.
                    if self.rendered_frames == previous_frames {
                        return;
                    }
                    if window_documents::smoke_enabled() {
                        match self.advance_document_smoke() {
                            Ok(true) => event_loop.exit(),
                            Ok(false) => self.request_redraw(),
                            Err(error) => {
                                self.fatal_error = Some(error);
                                event_loop.exit();
                            }
                        }
                        return;
                    }
                    if self.rendered_frames == 1 {
                        if std::env::args().any(|a| a == "--smoke-resize")
                            && let Some(window) = &self.window
                        {
                            let _ = window.request_inner_size(LogicalSize::new(1024.0, 768.0));
                        }
                        if self.mode == AcceptanceMode::Gallery2d {
                            self.apply_demo_action(DemoAction::TogglePopover);
                        }
                    }
                    if self.rendered_frames == 2 && self.mode == AcceptanceMode::Gallery2d {
                        self.apply_demo_action(DemoAction::ToggleCombo);
                    }
                    if let Some(mode) = render_backend::smoke_mode_action(self.rendered_frames) {
                        self.apply_demo_action(DemoAction::SelectMode(mode));
                    }
                    #[cfg(feature = "gpu-fault-injection")]
                    self.start_target_probe();
                    let lifecycle_ready = match self.advance_gpu_lifecycle() {
                        Ok(ready) => ready,
                        Err(error) => {
                            self.fatal_error = Some(error);
                            return;
                        }
                    };
                    if self.rendered_frames
                        >= render_backend::smoke_frame_limit(self.mode == AcceptanceMode::Gallery2d)
                    {
                        let floating_pending = !self.gpu_recovery.ready()
                            || !lifecycle_ready
                            || (std::env::args().any(|a| a == "--smoke-floating")
                                && !render_backend::recovery_close_enabled()
                                && (self.native_windows.is_empty()
                                    || self.native_windows.values().any(|w| {
                                        w.smoke_frames
                                            < render_backend::smoke_frame_limit(
                                                self.mode == AcceptanceMode::Gallery2d,
                                            )
                                    })));
                        #[cfg(feature = "gpu-fault-injection")]
                        let floating_pending = floating_pending || self.target_probe.pending();
                        let pending_allowance = if gpu_lifecycle::enabled()
                            || render_backend::recovery_fault().is_some()
                        {
                            120
                        } else {
                            12
                        };
                        let pending_limit = render_backend::smoke_frame_limit(
                            self.mode == AcceptanceMode::Gallery2d,
                        )
                        .saturating_add(pending_allowance);
                        let factory_pending = self
                            .native_windows
                            .values()
                            .any(|host| host.surface.creating());
                        if floating_pending
                            && (self.rendered_frames < pending_limit || factory_pending)
                        {
                            self.request_all_redraw();
                        } else {
                            if floating_pending {
                                self.fatal_error =
                                    Some("smoke floating window did not present".into());
                            }
                            eprintln!(
                                "Vesta Standalone smoke: {} main frames, {} floating windows",
                                self.rendered_frames,
                                self.native_windows.len()
                            );
                            if let Some((fps, ms)) = self.frame_meter.display {
                                eprintln!(
                                    "Vesta Standalone last steady sample: {fps:.1} UI FPS / {ms:.3} ms"
                                );
                            }
                            if let Some(gpu) = &self.gpu {
                                gpu.report_smoke(
                                    std::env::args().any(|a| a == "--smoke-resize"),
                                    &mut self.fatal_error,
                                );
                            }
                            for host in self.native_windows.values() {
                                eprintln!(
                                    "Vesta Standalone floating smoke: {} frames",
                                    host.smoke_frames
                                );
                            }
                            if let Err(error) = self.gpu_recovery.report() {
                                self.fatal_error = Some(error);
                            }
                            event_loop.exit();
                        }
                    } else if std::env::args().any(|a| a == "--smoke-floating") {
                        self.request_all_redraw();
                    } else {
                        self.request_redraw();
                    }
                }
            }
            _ => {}
        }
        if self.exit_requested {
            event_loop.exit();
        }
    }
}
