//! Per-window idle deadlines and ordinary native lifecycle polling.
use super::*;

pub(super) use vesta_gpui_widgets::CaretBlink;

fn drawable(window: &Window) -> bool {
    let size = window.inner_size();
    size.width > 0 && size.height > 0 && window.is_minimized() != Some(true)
}

pub(super) fn visible_geometry(layout: &LayoutTree, id: ElementId) -> bool {
    layout.node(id).is_some_and(|node| {
        node.rect
            .intersection(node.inherited_clip)
            .is_some_and(|rect| rect.width > 0.0 && rect.height > 0.0)
    })
}

impl App {
    pub(super) fn restart_caret_on_input(&mut self, window: UiWindowId, event: &WindowEvent) {
        let activity = match event {
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => vesta_gpui_winit::is_user_key_press(event.state, *is_synthetic),
            WindowEvent::MouseInput { state, .. } => *state == ElementState::Pressed,
            WindowEvent::Text(_) | WindowEvent::Ime(_) => true,
            _ => false,
        };
        if !activity {
            return;
        }
        let now = Instant::now();
        if window == STANDALONE_WINDOW {
            if self.caret_blink.restart(now) {
                self.request_redraw();
            }
        } else if let Some(host) = self
            .native_windows
            .values_mut()
            .find(|host| host.ui_window == window)
            && host.caret_blink.restart(now)
        {
            host.window.request_redraw();
        }
    }

    pub(super) fn main_window_resized(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.finish_workspace_resize_for(STANDALONE_WINDOW, false);
        self.cancel_table_resize();
        self.cancel_window_scrollbar_drag(STANDALONE_WINDOW);
        if let Some(gpu) = &mut self.gpu {
            gpu.resize(size.width, size.height);
        }
        if let Some(window) = &self.window {
            self.frame_profile.report_native_geometry(
                STANDALONE_WINDOW,
                NativeGeometryEvent::Resized(size),
                window,
            );
        }
        self.request_redraw();
    }

    pub(super) fn main_window_scale_factor_changed(&mut self) {
        self.cancel_table_resize();
        self.cancel_window_scrollbar_drag(STANDALONE_WINDOW);
        if let (Some(window), Some(gpu)) = (&self.window, &mut self.gpu) {
            let size = window.inner_size();
            gpu.resize(size.width, size.height);
        }
        if let Some(window) = &self.window {
            self.frame_profile.report_native_geometry(
                STANDALONE_WINDOW,
                NativeGeometryEvent::ScaleFactorChanged,
                window,
            );
        }
        self.request_redraw();
    }

    pub(super) fn wait_for_windows(&mut self, event_loop: &ActiveEventLoop) {
        let native_capture_deadline = self.native_capture_deadline(event_loop);
        if self.fatal_error.is_some() {
            event_loop.exit();
            return;
        }
        #[cfg(feature = "gpu-fault-injection")]
        if self
            .gpu
            .as_mut()
            .is_some_and(GpuState::close_creation_fixture)
        {
            event_loop.exit();
            return;
        }
        let mut creation_deadline = None;
        let mut update_errors = Vec::new();
        if let (Some(gpu), Some(window)) = (&mut self.gpu, &self.window) {
            match gpu.poll_creation(window) {
                Ok(deadline) => creation_deadline = deadline,
                Err(error) => {
                    self.fatal_error = Some(error.to_string());
                    event_loop.exit();
                    return;
                }
            }
        }
        if let (Some(gpu), Some(window)) = (&mut self.gpu, &self.window) {
            match gpu.poll_updates(window, self.mode == AcceptanceMode::Viewport3d) {
                Ok(deadline) => {
                    creation_deadline = creation_deadline.into_iter().chain(deadline).min()
                }
                Err(error) => update_errors.push((window.id(), error)),
            }
        }
        let mut creation_errors = Vec::new();
        let mut floating_created = false;
        for (id, host) in &mut self.native_windows {
            let was_available = host.surface.available();
            match host.surface.poll_creation(&host.window) {
                Ok(deadline) => {
                    creation_deadline = creation_deadline.into_iter().chain(deadline).min();
                    if self.smoke && !was_available && host.surface.available() {
                        floating_created = true;
                        let shared = self
                            .gpu
                            .as_ref()
                            .and_then(GpuState::gpu_context)
                            .zip(host.surface.gpu_context())
                            .is_some_and(|(main, floating)| Arc::ptr_eq(&main, &floating));
                        if !shared {
                            self.fatal_error = Some(
                                "ui_gpu_context_mismatch: floating window created another context"
                                    .into(),
                            );
                        }
                        eprintln!(
                            "UI-R02 floating={} shared_gpu_context={shared}",
                            host.logical_id.0
                        );
                    }
                }
                Err(error) => creation_errors.push((*id, error)),
            }
            match host.surface.poll_updates(&host.window, false) {
                Ok(deadline) => {
                    creation_deadline = creation_deadline.into_iter().chain(deadline).min()
                }
                Err(error) => update_errors.push((*id, error)),
            }
        }
        for (id, error) in creation_errors {
            self.gpu_fault(id, error);
        }
        for (id, error) in update_errors {
            self.gpu_update_fault(id, error);
        }
        if floating_created {
            self.request_redraw();
        }
        let recovery_deadline = self.drive_gpu_recovery();
        #[cfg(feature = "gpu-fault-injection")]
        if self.poll_target_probe() {
            event_loop.exit();
            return;
        }
        #[cfg(feature = "gpu-fault-injection")]
        if let Err(error) = self.poll_lifecycle_suspension() {
            self.fatal_error = Some(error);
            event_loop.exit();
            return;
        }
        self.sync_native_windows(event_loop);
        self.position_native_capture_windows();
        let now = Instant::now();
        let caret_target = self
            .window
            .as_ref()
            .filter(|window| drawable(window) && window.has_focus())
            .and_then(|_| self.focused_text_field())
            .map(|field| field.element(self.scopes.gallery));
        if self.caret_blink.update(caret_target, now) {
            self.request_redraw();
        }
        for host in self.native_windows.values_mut() {
            let target = host
                .input
                .focus()
                .filter(|_| drawable(&host.window) && host.window.has_focus())
                .filter(|focus| {
                    TextFieldKind::ALL
                        .into_iter()
                        .any(|field| field.element(host.scopes.gallery) == focus.element)
                })
                .map(|focus| focus.element);
            if host.caret_blink.update(target, now) {
                host.window.request_redraw();
            }
        }
        let caret_deadline = self
            .caret_blink
            .deadline()
            .into_iter()
            .chain(
                self.native_windows
                    .values()
                    .filter_map(|host| host.caret_blink.deadline()),
            )
            .min();
        let document_deadline = self.drive_document_drag_scroll(now);
        if self
            .gpu
            .as_ref()
            .and_then(GpuState::frame_retry_deadline)
            .is_some_and(|deadline| now >= deadline)
        {
            self.request_redraw();
        }
        for host in self.native_windows.values() {
            if host
                .surface
                .frame_retry_deadline()
                .is_some_and(|deadline| now >= deadline)
            {
                host.window.request_redraw();
            }
        }
        let gpu_deadline = self
            .gpu
            .as_ref()
            .and_then(GpuState::frame_retry_deadline)
            .into_iter()
            .chain(
                self.native_windows
                    .values()
                    .filter_map(|host| host.surface.frame_retry_deadline()),
            )
            // Due redraws were just requested. Wait for their events rather
            // than continuously rescheduling an already-expired timer.
            .filter(|deadline| *deadline > now)
            .min();
        let retirement_deadline = self
            .gpu
            .as_ref()
            .filter(|gpu| gpu.retirement_monitor().pending())
            .map(|_| now + Duration::from_millis(16));
        if self.tooltip.reveal_due(now) {
            self.request_redraw();
        }
        for host in self.native_windows.values_mut() {
            if host.tooltip.reveal_due(now) {
                host.window.request_redraw();
            }
        }
        self.expire_toasts(now);
        let busy_main =
            self.busy_visible && self.window.as_ref().is_some_and(|window| drawable(window));
        let busy_floating = self
            .native_windows
            .values()
            .any(|host| host.busy_visible && drawable(&host.window));
        let busy_active = self.demo.busy
            && self.mode == AcceptanceMode::Gallery2d
            && (busy_main || busy_floating);
        if busy_active && now >= self.next_busy_frame {
            self.demo.busy_phase = (self.demo.busy_phase + 1) % 4;
            self.next_busy_frame = now + Duration::from_millis(80);
            if busy_main {
                self.request_redraw();
            }
            for host in self.native_windows.values() {
                if host.busy_visible && drawable(&host.window) {
                    host.window.request_redraw();
                }
            }
        }
        let frame_active = !self.smoke
            && self.window.as_ref().is_some_and(|window| {
                let size = window.inner_size();
                size.width > 0 && size.height > 0 && window.is_minimized() != Some(true)
            });
        if !frame_active && self.window.is_some() && !self.smoke {
            self.frame_meter.reset();
        }
        let tooltip_deadline = self
            .tooltip
            .deadline(now)
            .into_iter()
            .chain(
                self.native_windows
                    .values()
                    .filter_map(|host| host.tooltip.deadline(now)),
            )
            .min();
        let busy_deadline = busy_active.then_some(self.next_busy_frame);
        let toast_deadline = self.demo.toast_queue.next_deadline();
        let deadline = tooltip_deadline
            .into_iter()
            .chain(busy_deadline)
            .chain(toast_deadline)
            .chain(gpu_deadline)
            .chain(recovery_deadline)
            .chain(creation_deadline)
            .chain(retirement_deadline)
            .chain(document_deadline)
            .chain(caret_deadline)
            .chain(native_capture_deadline)
            .min();
        self.lifecycle_profile.waiting(deadline);
        event_loop.set_control_flow(deadline.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::{event::DeviceId, keyboard::KeyLocation};

    #[test]
    fn edit_input_reveals_the_current_caret_without_restarting_for_replay_release_or_foreign_windows()
     {
        let mut app = App::new();
        let target = TextFieldKind::Main.element(app.scopes.gallery);
        let now = Instant::now();
        let key = |state, is_synthetic| WindowEvent::KeyboardInput {
            device_id: DeviceId::dummy(),
            event: vesta_gpui_winit::UiKeyEvent {
                physical_key: winit::keyboard::PhysicalKey::Code(
                    winit::keyboard::KeyCode::ArrowLeft,
                ),
                logical_key: Key::Named(NamedKey::ArrowLeft),
                text: None,
                location: KeyLocation::Standard,
                state,
                repeat: false,
            },
            is_synthetic,
        };
        for event in [
            key(ElementState::Pressed, false),
            WindowEvent::Text("x".into()),
            WindowEvent::Ime(winit::event::Ime::Preedit("你".into(), None)),
            WindowEvent::MouseInput {
                device_id: DeviceId::dummy(),
                state: ElementState::Pressed,
                button: MouseButton::Left,
            },
        ] {
            app.caret_blink.update(None, now);
            app.caret_blink.update(Some(target), now);
            app.caret_blink.tick(now + Duration::from_secs(1));
            assert!(!app.caret_blink.visible());
            app.observe_text_pointer_event(STANDALONE_WINDOW, &event);
            assert!(
                app.caret_blink.visible(),
                "input reveals the existing caret"
            );
            assert!(app.caret_blink.deadline().is_some());
        }
        app.caret_blink.tick(now + Duration::from_secs(2));
        let deadline = app.caret_blink.deadline();
        for (window, event) in [
            (STANDALONE_WINDOW, key(ElementState::Pressed, true)),
            (STANDALONE_WINDOW, key(ElementState::Released, false)),
            (UiWindowId(999), WindowEvent::Text("foreign".into())),
            (STANDALONE_WINDOW, WindowEvent::RedrawRequested),
        ] {
            app.observe_text_pointer_event(window, &event);
            assert!(
                !app.caret_blink.visible(),
                "non-input must not reveal the caret"
            );
            assert_eq!(app.caret_blink.deadline(), deadline);
        }
        app.caret_blink.update(None, now);
        app.observe_text_pointer_event(STANDALONE_WINDOW, &WindowEvent::Text("ownerless".into()));
        assert_eq!(app.caret_blink.deadline(), None);
    }
}
