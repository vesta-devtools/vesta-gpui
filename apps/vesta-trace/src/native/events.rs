//! Native event-loop and worker publication lifecycle.
use super::*;
impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            self.redraw();
            return;
        }
        let result = (|| -> Result<()> {
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("Vesta Trace — native frame analysis")
                        .with_inner_size(LogicalSize::new(1480.0, 900.0))
                        .with_min_inner_size(LogicalSize::new(980.0, 640.0))
                        .with_visible(!self.smoke),
                )?,
            );
            self.accessibility = Some(AccessibilityAdapter::with_event_loop_proxy(
                event_loop,
                &window,
                self.proxy.clone(),
            ));
            let creation = RendererCreation::start(event_loop, window.clone())?;
            self.monitor = Some(creation.retirement_monitor());
            self.creation = Some(creation);
            self.window = Some(window);
            Ok(())
        })();
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, message: UserEvent) {
        let message = match message {
            UserEvent::Worker(message) => *message,
            UserEvent::Accessibility(event) => {
                if event_loop.exiting()
                    || self
                        .window
                        .as_ref()
                        .is_none_or(|w| w.id() != event.window_id)
                {
                    return;
                }
                match event.window_event {
                    AccessibilityWindowEvent::InitialTreeRequested => self.redraw(),
                    AccessibilityWindowEvent::ActionRequested(request) => {
                        if let Err(error) = self.accessible_action(request) {
                            self.status = error.to_string();
                            self.redraw();
                        }
                    }
                    AccessibilityWindowEvent::AccessibilityDeactivated => (),
                }
                return;
            }
        };
        if self.smoke {
            println!(
                "trace_job_result generation={} current={} success={}",
                message.generation,
                self.generation,
                message.result.is_ok()
            );
        }
        if message.generation == self.generation {
            let activity = std::mem::take(&mut self.activity);
            match message.result {
                Ok(Product::Loaded {
                    path,
                    session,
                    report,
                }) => {
                    let desired_panel = self.panel;
                    let desired_filter = if self.search.input.is_composing()
                        || self.search.input.text().len() > 256
                    {
                        self.filter.clone()
                    } else {
                        self.search.input.text().to_owned()
                    };
                    self.checkpoint = None;
                    self.suspend_geometry();
                    if self
                        .session
                        .as_ref()
                        .is_none_or(|old| old.source.sha256 != session.source.sha256)
                    {
                        // Element slots may coincide across files. Platform node
                        // identities belong to the immutable source, not a row rank.
                        self.semantics = Default::default();
                        self.smoke_semantics = None;
                    }
                    self.source_path = Some(path.clone());
                    self.timeline = Some(Timeline::new(session.bounds));
                    self.session = Some(session);
                    self.timer = report.inspection.timer.clone();
                    self.gpu = None;
                    self.gpu_scroll = Default::default();
                    self.report = Some(report);
                    self.panel = Panel::Timers;
                    self.filter.clear();
                    self.reset_pages();
                    if !self.search_focused() {
                        self.set_focus(None);
                    }
                    self.status = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    self.click.cancel();
                    self.drag = None;
                    if !desired_filter.is_empty() || desired_panel != Panel::Timers {
                        self.begin_change();
                        self.panel = desired_panel;
                        self.filter = desired_filter;
                        if let Err(error) = self.reanalyze() {
                            self.fail(event_loop, error);
                        }
                    }
                    if self.scene == Scene::Lifecycle && self.lifecycle.cancel_followup {
                        self.lifecycle.cancel_followup = false;
                        self.lifecycle.cancelled_followup = Some(self.generation);
                        if let Err(error) = self.command(Command::Cancel) {
                            self.fail(event_loop, error);
                        }
                    }
                }
                Ok(Product::Analyzed(report)) => {
                    self.checkpoint = None;
                    self.timer = report.inspection.timer.clone();
                    self.report = Some(report);
                    self.status = self.source_name();
                }
                Ok(Product::CaptureSaved(path)) => {
                    self.capture_waiting = false;
                    self.captured_step = Some(self.smoke_step);
                    println!(
                        "trace_capture_saved step={} path={}",
                        self.smoke_step,
                        path.display()
                    );
                }
                Err(error) => {
                    if let Err(error) = self.restore_view() {
                        self.fail(event_loop, error);
                        return;
                    }
                    let target = if let Activity::Import(path) = activity {
                        format!(" · could not open {}", path.display())
                    } else {
                        String::new()
                    };
                    self.status = format!("{}{target} · {error}", self.source_name());
                    if self.smoke && !self.expected_work_error(message.generation, &error) {
                        self.fail(event_loop, error);
                    }
                }
            }
        } else if self.scene == Scene::Lifecycle {
            self.lifecycle.stale.push(message.generation);
        }
        if let Err(error) = self.flush() {
            self.fail(event_loop, error);
        }
        self.redraw();
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if event_loop.exiting() || self.window.as_ref().is_none_or(|w| w.id() != id) {
            return;
        }
        if let Some(adapter) = &mut self.accessibility {
            adapter.process_event(self.window.as_ref().expect("matched live window"), &event);
        }
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                return;
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.draw(event_loop) {
                    self.fail(event_loop, error);
                }
                return;
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.suspend_geometry();
                self.redraw();
            }
            WindowEvent::Occluded(occluded) => {
                self.suspend_geometry();
                if occluded {
                    self.search.end_session(&mut self.context);
                }
                self.occluded = occluded;
                if !occluded {
                    self.redraw();
                }
            }
            _ => {}
        }
        if let Err(error) = self.input(
            &vesta_gpui_winit::UiInputEvent::from(&event),
            vesta_gpui_winit::UiInputOrigin::Native,
        ) {
            self.fail(event_loop, error);
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if event_loop.exiting() {
            return;
        }
        if let Some(creation) = &mut self.creation {
            match creation.poll() {
                RecoveryPoll::Ready(mut renderers) if renderers.len() == 1 => {
                    self.renderer = renderers.pop().map(Box::new);
                    self.creation = None;
                    self.redraw();
                }
                RecoveryPoll::Failed(error) => {
                    self.fail(event_loop, error);
                    return;
                }
                _ => {}
            }
        }
        if self.updates.pending()
            && let Some(window) = &self.window
        {
            let size = window.inner_size();
            match self.updates.poll(
                &mut self.renderer,
                TargetConfiguration::ui([size.width, size.height]),
            ) {
                Ok(retry) => {
                    self.retry = retry;
                    if !self.updates.pending() {
                        self.redraw();
                    }
                }
                Err(error) => {
                    self.fail(event_loop, error);
                    return;
                }
            }
        }
        let now = Instant::now();
        if self.search.due.is_some_and(|due| now >= due)
            && let Err(error) = self.apply_search()
        {
            self.fail(event_loop, error);
            return;
        }
        if self.retry.is_some_and(|at| now >= at) {
            self.retry = None;
            self.redraw();
        }
        if self.smoke
            && self.renderer.is_some()
            && self.retry.is_none()
            && !self.updates.pending()
            && let Err(error) = self.draw(event_loop)
        {
            self.fail(event_loop, error);
            return;
        }
        let pending = self.creation.is_some() || self.updates.pending();
        let mut deadline = match (self.retry, self.search.due) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        // The finite hidden scenario has no visible-window redraw cadence.
        // Observe completion/resize without borrowing the desktop or relaxing its deadline.
        if self.smoke && (self.capture_waiting || self.session.is_some() && !self.busy()) {
            deadline = Some(now + Duration::from_millis(16));
        }
        if pending {
            deadline = Some(deadline.map_or(now + Duration::from_millis(16), |d| {
                d.min(now + Duration::from_millis(16))
            }));
        }
        if self.smoke {
            let end = self.started + Duration::from_secs(45);
            if now >= end {
                self.fail(event_loop, "trace_smoke_timeout");
                return;
            }
            deadline = Some(deadline.map_or(end, |d| d.min(end)));
        }
        event_loop.set_control_flow(deadline.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.search.end_session(&mut self.context);
        self.cancel.store(true, Ordering::Relaxed);
        self.pending = None;
        self.renderer = None;
        self.creation = None;
        self.updates = RendererUpdates::default();
    }
}
