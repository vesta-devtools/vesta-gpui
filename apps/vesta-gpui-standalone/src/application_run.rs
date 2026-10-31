//! Standalone startup, event-loop and original GPU retirement orchestration.
use super::*;

pub(super) fn run() -> Result<(), Box<dyn Error>> {
    if std::env::args().any(|argument| argument == "--vulkan-probe") {
        return Err(
            "standalone_vulkan_probe_retired: use the default Gallery with --smoke --smoke-resize"
                .into(),
        );
    }
    render_backend::validate_args()?;
    window_documents::validate_smoke_args()?;
    native_capture::validate_args()?;
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new();
    app.prepare_native_capture()?;
    app.prepare_frame_reflow_probe()?;
    if app.smoke && app.mode == AcceptanceMode::Gallery2d {
        // Exercise text/popover controls and the image cache in the same visible Dock pages.
        for panel_type in ["gallery.inputs", "gallery.layout"] {
            let item = app
                .demo
                .workspace
                .panes()
                .into_iter()
                .flat_map(|pane| pane.items.iter())
                .find(|item| item.panel_type.as_str() == panel_type)
                .map(|item| item.id)
                .expect("smoke page exists");
            app.demo.workspace.activate(item)?;
            app.demo.sync_workspace_ui();
            if panel_type == "gallery.layout" {
                // The image example lives below the first fold. Keep the smoke
                // texture-cache check on genuinely visible, clipped content.
                app.demo
                    .page_scroll
                    .insert(item, ScrollAreaState::new(1_000.0));
            }
        }
    }
    if app.smoke && (std::env::args().any(|a| a == "--smoke-floating") || gpu_lifecycle::enabled())
    {
        let item = app
            .demo
            .workspace
            .panes()
            .into_iter()
            .flat_map(|pane| pane.items.iter())
            .find(|item| item.panel_type.as_str() == "gallery.collections")
            .map(|item| item.id)
            .ok_or("smoke floating page missing")?;
        if !app.float_workspace_item(item, WindowPlacement::new(100.0, 100.0, 640.0, 480.0)) {
            return Err("smoke could not float the Gallery page".into());
        }
        if gpu_lifecycle::enabled() {
            app.gpu_lifecycle = gpu_lifecycle::Smoke::First {
                item,
                resize_requested: false,
            };
        }
    }
    app.event_proxy = Some(event_loop.create_proxy());
    // The probe enables local snapshots itself; only explicit existing stdio
    // flags opt it into the public worker. Its windows remain hidden/nonactive.
    if let Some(control) = std::env::args()
        .filter_map(|arg| ui_query::stdio_argument(&arg))
        .max()
    {
        app.ui_query.control_enabled = control;
        ui_query::start(event_loop.create_proxy(), app.ui_query.control_enabled)?;
    }
    let event_result = event_loop.run_app(&mut app);
    if frame_reflow_probe::requested()
        && !app
            .frame_reflow_probe
            .as_ref()
            .is_some_and(|probe| probe.completed)
    {
        app.fatal_error
            .get_or_insert("frame_reflow_probe_incomplete".into());
    }
    app.require_native_capture_completion();
    let retirement = app.gpu.as_ref().map(GpuState::retirement_monitor);
    let context_probe = render_backend::retirement_probe_enabled()
        .then(|| {
            app.gpu
                .as_ref()
                .and_then(GpuState::gpu_context)
                .map(|gpu| Arc::downgrade(&gpu))
        })
        .flatten();
    #[cfg(feature = "gpu-fault-injection")]
    let capture_probe = app.gpu.as_mut().and_then(GpuState::take_retirement_capture);
    let smoke = app.smoke;
    let missing_frame = smoke && app.rendered_frames == 0;
    let fatal = app.fatal_error.take();
    if let Some(window) = &app.window {
        window.set_visible(false);
    }
    for host in app.native_windows.values() {
        host.window.set_visible(false);
    }
    let mut gpu_profile = std::mem::take(&mut app.gpu_profile);
    drop(app);
    let retired = (|| -> Result<(), vesta_gpui_vulkan::RendererError> {
        if let Some(retirement) = &retirement {
            let observed = retirement.observe(Duration::from_secs(5));
            #[cfg(feature = "gpu-fault-injection")]
            if render_backend::completion_retirement_fault_enabled() {
                let unknown = observed.as_ref().is_err_and(|error| {
                    error.code == "native_vulkan_completion_dependency_panicked"
                        && error.fault == vesta_gpui_vulkan::RendererFault::Other
                });
                eprintln!(
                    "UI-R08 exit: pending={} unknown_panic_is_other={unknown}",
                    retirement.pending()
                );
            }
            observed?;
            #[cfg(feature = "gpu-fault-injection")]
            if render_backend::retirement_probe_enabled() {
                let lease_error = |message: &str| vesta_gpui_vulkan::RendererError {
                    code: "native_vulkan_readback_context_lease",
                    stage: "retirement_capture",
                    fault: vesta_gpui_vulkan::RendererFault::Other,
                    message: message.into(),
                    element: None,
                };
                let readback = capture_probe
                    .ok_or_else(|| lease_error("retirement probe did not capture a real frame"))?
                    .wait(Duration::from_secs(5))?;
                let held = context_probe
                    .as_ref()
                    .is_some_and(|gpu| gpu.upgrade().is_some());
                eprintln!(
                    "UI-R07 readback: real_frame={} context_held_after_renderer_drop={held}",
                    readback.frame
                );
                if !held {
                    return Err(lease_error("public readback outlived its GPU context"));
                }
                drop(readback);
                retirement.observe(Duration::from_secs(5))?;
                let released = context_probe
                    .as_ref()
                    .is_none_or(|gpu| gpu.upgrade().is_none());
                eprintln!("UI-R07 readback: released=true context_retired_on_worker={released}");
                if !released {
                    return Err(lease_error("public readback context did not retire"));
                }
            }
            drop(context_probe);
            retirement.observe(Duration::from_secs(5))?;
            if smoke {
                eprintln!("UI-R07 exit: all GPU owners retired on worker; pending=false");
            }
        }
        Ok(())
    })();
    gpu_profile.finish(
        retirement.as_ref().is_some_and(|monitor| monitor.pending()),
        retired.as_ref().err().map(|error| error.code),
    );
    retired?;
    event_result?;
    if let Some(error) = fatal {
        return Err(error.into());
    }
    if missing_frame {
        return Err("smoke run exited without presenting a frame".into());
    }
    Ok(())
}

impl ApplicationHandler<UserEvent> for App {
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: winit::event::StartCause) {
        self.lifecycle_profile.new_events(cause);
    }
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        if !self.smoke && self.native_capture.is_none() && !frame_reflow_probe::requested() {
            match ui_preferences::load() {
                Ok(Some(preferences)) => {
                    let drawer = self.drawer_state();
                    self.demo.drawer_width = preferences
                        .drawer_width
                        .clamp(drawer.min_width, drawer.max_width);
                    self.demo.locale = preferences.locale;
                }
                Ok(None) => {}
                Err(error) => eprintln!("UI preference restore: {error}"),
            }
            match workspace_session::load() {
                Ok(Some(restored)) => {
                    for missing in &restored.unavailable_panels {
                        eprintln!(
                            "workspace panel {} is unavailable for item {}",
                            missing.panel_type.as_str(),
                            missing.item.0
                        );
                    }
                    self.demo.workspace = restored.workspace;
                    ensure_documents_tab(&mut self.demo.workspace);
                    self.demo.pane_history.clear();
                    self.demo.zoomed_panes.clear();
                    self.demo.sync_workspace_ui();
                }
                Ok(None) => {}
                Err(error) => eprintln!("workspace layout restore: {error}"),
            }
        }
        let initial_width = if std::env::args().any(|argument| argument == "--narrow-status") {
            400.0
        } else {
            1280.0
        };
        let attributes: WindowAttributes = Window::default_attributes()
            .with_title(self.demo.locale.window_title())
            .with_decorations(!cfg!(target_os = "windows"))
            .with_visible(false)
            .with_active(!frame_reflow_probe::requested())
            .with_inner_size(LogicalSize::new(initial_width, 800.0))
            .with_min_inner_size(LogicalSize::new(400.0, 600.0));
        #[cfg(target_os = "windows")]
        let attributes = {
            use winit::platform::windows::{CornerPreference, WindowAttributesExtWindows};
            attributes
                .with_corner_preference(CornerPreference::Round)
                .with_undecorated_shadow(true)
        };
        let Ok(window) = event_loop.create_window(attributes).map(Arc::new) else {
            event_loop.exit();
            return;
        };
        let accessibility = AccessibilityAdapter::with_event_loop_proxy(
            event_loop,
            &window,
            self.event_proxy
                .clone()
                .expect("event loop proxy initialized"),
        );
        match GpuState::new(
            event_loop,
            window.clone(),
            self.mode == AcceptanceMode::Viewport3d,
        ) {
            Ok(gpu) => {
                self.demo.viewport3d_available = true;
                window.set_visible(!frame_reflow_probe::requested());
                self.frame_profile.report_native_geometry(
                    STANDALONE_WINDOW,
                    NativeGeometryEvent::Created,
                    &window,
                );
                self.window = Some(window);
                self.accessibility = Some(accessibility);
                self.gpu = Some(gpu);
                self.request_redraw();
                self.sync_native_windows(event_loop);
            }
            Err(error) => {
                eprintln!("failed to initialize GPU: {error}");
                self.fatal_error = Some(error);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: NativeWindowEvent,
    ) {
        self.native_platform_event(event_loop, window_id, event);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        let event = match event {
            UserEvent::UiQuery(request) => {
                self.answer_ui_query(event_loop, *request);
                if self.exit_requested {
                    event_loop.exit();
                }
                return;
            }
            UserEvent::Accessibility(event) => event,
        };
        if matches!(
            &event.window_event,
            AccessibilityWindowEvent::ActionRequested(_)
        ) {
            let window = if self
                .window
                .as_ref()
                .is_some_and(|window| window.id() == event.window_id)
            {
                Some(STANDALONE_WINDOW)
            } else {
                self.native_windows
                    .get(&event.window_id)
                    .map(|host| host.ui_window)
            };
            if window.is_some() {
                self.ui_query.invalidate_shared();
                self.request_all_redraw();
            }
        }
        if (self.display_blocked(event.window_id)
            || !self.native_input_layout_ready(event.window_id))
            && matches!(
                event.window_event,
                AccessibilityWindowEvent::ActionRequested(_)
            )
        {
            return;
        }
        if self
            .window
            .as_ref()
            .is_some_and(|window| window.id() == event.window_id)
        {
            match event.window_event {
                AccessibilityWindowEvent::InitialTreeRequested => self.request_redraw(),
                AccessibilityWindowEvent::ActionRequested(request) => {
                    self.accessibility_action(request);
                }
                AccessibilityWindowEvent::AccessibilityDeactivated => {}
            }
        } else if self.native_windows.contains_key(&event.window_id) {
            match event.window_event {
                AccessibilityWindowEvent::InitialTreeRequested => {
                    if let Some(host) = self.native_windows.get(&event.window_id) {
                        host.window.request_redraw();
                    }
                }
                AccessibilityWindowEvent::ActionRequested(request) => {
                    self.native_accessibility_action(event.window_id, request);
                }
                AccessibilityWindowEvent::AccessibilityDeactivated => {}
            }
        } else {
            return;
        }
        if self.exit_requested {
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.wait_for_windows(event_loop);
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if std::env::args().any(|argument| argument == "--profile-ui") {
            eprintln!(
                "Standalone frame lifecycle: {} completed main frames",
                self.rendered_frames
            );
        }
        if !self.smoke
            && self.native_capture.is_none()
            && self.window.is_some()
            && !frame_reflow_probe::requested()
        {
            let drawer_width = self
                .drawer_resize
                .map_or(self.demo.drawer_width, |drag| *drag.initial());
            if let Err(error) = ui_preferences::save(ui_preferences::UiPreferences {
                drawer_width,
                locale: self.demo.locale,
            }) {
                eprintln!("UI preference save: {error}");
            }
            if let Err(error) = workspace_session::save(&self.demo.workspace) {
                eprintln!("workspace layout save: {error}");
            }
        }
    }
}
