//! Native event ownership and shared Vulkan lifecycle. No authoring or Graph runtime.
use crate::{model::*, preview, study::Study, view};
use std::{
    error::Error,
    path::PathBuf,
    sync::Arc,
    thread::JoinHandle,
    time::{Duration, Instant},
};
use vesta_gpui::{
    AccessibilityTree, ActionRoute, FocusInput, FocusScope, HitTestTree, InputOwnership,
    InputTarget, LayoutTree, PaintList, Point, PointerButton, PointerClick, Rect,
    RetainedLayoutEngine, TextInputContext, TextRasterizer, UiWindowId, Viewport,
};
use vesta_gpui_vulkan::{
    CaptureRegion, RecoveryPoll, RenderOptions, RendererCreation, RendererUpdates,
    RetirementMonitor, TargetConfiguration, WindowRenderer,
};
use vesta_gpui_winit::{
    AccessibilityAdapter, AccessibilityEvent, AccessibilityWindowEvent, logical_point,
    observe_focus_visibility,
};
use vesta_viewport::render::ViewportRect;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Theme, Window, WindowId},
};

const WINDOW: UiWindowId = UiWindowId(1);
const PERIOD: Duration = Duration::from_millis(16);

#[derive(Default)]
struct Options {
    study: Study,
    tour: bool,
    smoke: bool,
    smoke_timeout: Option<Duration>,
    hidden: bool,
    capture: Option<PathBuf>,
    observe: Option<Duration>,
}
struct Frame {
    route: ActionRoute<Action>,
    hits: HitTestTree,
    layout: LayoutTree,
}
#[derive(Clone, Copy)]
struct Drag {
    action: Action,
    original: Parameters,
    time: f32,
    playing: bool,
}
struct App {
    model: Model,
    window: Option<Arc<Window>>,
    creation: Option<RendererCreation>,
    renderer: Option<Box<WindowRenderer>>,
    updates: RendererUpdates,
    monitor: Option<RetirementMonitor>,
    proxy: winit::event_loop::EventLoopProxy<AccessibilityEvent>,
    accessibility: Option<AccessibilityAdapter>,
    semantics: AccessibilityTree,
    context: TextInputContext,
    rasterizer: TextRasterizer,
    layout: RetainedLayoutEngine,
    frame: Option<Frame>,
    input: InputOwnership,
    click: PointerClick,
    pointer: Point,
    drag: Option<Drag>,
    shift: bool,
    occluded: bool,
    started: Instant,
    tick: Instant,
    next: Instant,
    retry: Option<Instant>,
    frames: u64,
    smoke_checked: bool,
    resize_requested: bool,
    resized: bool,
    options: Options,
    captures: Vec<JoinHandle<Result<(), String>>>,
    capture_frames: Vec<u64>,
    error: Option<String>,
}

impl App {
    fn redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl ToString) {
        self.error = Some(error.to_string());
        event_loop.exit();
    }
    fn cancel_drag(&mut self, rollback: bool) {
        if let Some(drag) = self.drag.take() {
            if rollback {
                self.model.restore_params(drag.original);
                self.model.time = drag.time;
            }
            if drag.action == Action::Seek {
                self.model.playing = drag.playing;
            }
        }
        self.click.cancel();
        self.input.cancel_capture();
    }
    fn apply(&mut self, action: Action) {
        if matches!(action, Action::Study(_)) {
            self.cancel_drag(true);
        }
        self.model.apply(action);
        if let Some(window) = &self.window {
            window.set_title(&format!("Vesta Motion — {}", self.model.study.title()));
        }
        self.tick = Instant::now();
        self.redraw();
    }
    fn drag_to(&mut self, action: Action) {
        let Some(frame) = &self.frame else {
            return;
        };
        match action {
            Action::Seek => {
                if let Some(node) = frame.layout.node(self.model.id(SEEK)) {
                    self.model.seek(
                        (self.pointer.x - node.rect.x - 10.0) / (node.rect.width - 20.0).max(1.0),
                    );
                }
            }
            Action::Parameter(param) => {
                if let Some(node) = frame.layout.node(self.model.id(param.slot() + 1)) {
                    self.model.set(
                        param,
                        vesta_gpui_widgets::slider_value_at(node.rect, self.pointer),
                    );
                }
            }
            _ => (),
        }
        self.redraw();
    }
    fn mouse(&mut self, state: ElementState) {
        let Some(frame) = &self.frame else {
            return;
        };
        let hit = frame.hits.hit(self.pointer);
        if state == ElementState::Released {
            if self.drag.is_some() {
                self.cancel_drag(false);
            } else {
                let action = self
                    .click
                    .release(WINDOW, PointerButton::Primary, &frame.route, hit)
                    .copied();
                self.input.release_pointer(WINDOW, PointerButton::Primary);
                if let Some(action) = action {
                    self.apply(action);
                }
            }
            self.redraw();
            return;
        }
        let target = hit.and_then(|hit| frame.route.target_for_hit(hit));
        if let Some((element, action)) = target {
            let action = *action;
            let target = InputTarget {
                window: WINDOW,
                element,
            };
            self.input.set_focus(Some(target));
            self.input.capture_pointer(target, PointerButton::Primary);
            if matches!(action, Action::Seek | Action::Parameter(_)) {
                self.drag = Some(Drag {
                    action,
                    original: self.model.params(),
                    time: self.model.time,
                    playing: self.model.playing,
                });
                if action == Action::Seek {
                    self.model.playing = false;
                }
                self.drag_to(action);
            } else {
                self.click
                    .press(WINDOW, PointerButton::Primary, &frame.route, hit);
            }
        } else {
            self.input.set_focus(None);
        }
        self.redraw();
    }
    fn key(&mut self, key: &Key) {
        if key == &Key::Named(NamedKey::Escape) {
            self.cancel_drag(true);
            self.redraw();
            return;
        }
        if key == &Key::Named(NamedKey::Tab) {
            if let Some(frame) = &self.frame {
                self.input.observe_focus_input(FocusInput::Keyboard);
                FocusScope::new(WINDOW, &frame.route, None).traverse(&mut self.input, self.shift);
            }
            self.redraw();
            return;
        }
        if key == &Key::Named(NamedKey::Space) {
            self.apply(Action::Play);
            return;
        }
        if let Key::Character(value) = key {
            if let Some(study) = Study::ALL
                .into_iter()
                .find(|study| value.as_str() == (study.index() + 1).to_string())
            {
                self.apply(Action::Study(study));
                return;
            }
            if value.eq_ignore_ascii_case("t") {
                self.apply(Action::Tour);
                return;
            }
        }
        let focused = self
            .frame
            .as_ref()
            .and_then(|frame| {
                self.input
                    .focus()
                    .and_then(|t| frame.route.action_at(t.element))
            })
            .copied();
        match (key, focused) {
            (Key::Named(NamedKey::Enter), Some(action)) => self.apply(action),
            (
                Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowDown),
                Some(Action::Parameter(param)),
            ) => self.model.set(param, self.model.value(param) - 0.015),
            (
                Key::Named(NamedKey::ArrowRight | NamedKey::ArrowUp),
                Some(Action::Parameter(param)),
            ) => self.model.set(param, self.model.value(param) + 0.015),
            (Key::Named(NamedKey::ArrowLeft), Some(Action::Seek)) => {
                self.model.seek((self.model.time - 1.0 / 30.0) / DURATION)
            }
            (Key::Named(NamedKey::ArrowRight), Some(Action::Seek)) => {
                self.model.seek((self.model.time + 1.0 / 30.0) / DURATION)
            }
            (Key::Named(NamedKey::Home), _) => self.apply(Action::Start),
            (Key::Named(NamedKey::End), _) => self.apply(Action::End),
            _ => (),
        }
        self.redraw();
    }
    fn draw(&mut self) -> Result<(), String> {
        let Some(window) = self.window.clone() else {
            return Ok(());
        };
        let extent = window.inner_size();
        if extent.width == 0
            || extent.height == 0
            || (!self.options.hidden && (self.occluded || window.is_minimized() == Some(true)))
        {
            return Ok(());
        }
        if !self
            .updates
            .prepare(
                &mut self.renderer,
                TargetConfiguration::ui([extent.width, extent.height]).with_scene(true),
            )
            .map_err(|e| e.to_string())?
        {
            self.frame = None;
            self.retry = Some(Instant::now() + PERIOD);
            return Ok(());
        }
        let scale = window.scale_factor() as f32;
        let viewport = Viewport::new(
            extent.width as f32 / scale,
            extent.height as f32 / scale,
            scale,
        );
        let widget = view::build(&self.model, viewport.width, viewport.height);
        let layout = self
            .layout
            .compute_with_text_context(&widget.element, viewport, &mut self.context)
            .map_err(|e| format!("motion.layout: {e}"))?;
        let surface = layout
            .node(self.model.id(SURFACE))
            .ok_or("motion.surface_missing")?
            .rect;
        let route = ActionRoute::build(&widget.element, widget.actions)
            .map_err(|e| format!("motion.actions: {e:?}"))?;
        let lost = FocusScope::new(WINDOW, &route, None).reconcile(&mut self.input);
        if lost.lost_capture {
            self.cancel_drag(true);
        }
        let paint = PaintList::build_with_focus(
            &widget.element,
            &layout,
            self.input.visible_focus().map(|t| t.element),
            MINT,
        );
        let rect = physical(surface, scale, [extent.width, extent.height]);
        let scene = preview::scene(&self.model);
        let renderer = self.renderer.as_mut().ok_or("motion.renderer_missing")?;
        // Queue only bounded readback; mapping and PNG encoding stay on a worker.
        if self.options.capture.is_some()
            && (self.frames == 15 || self.frames == 65 || self.frames == 95)
            && !self.capture_frames.contains(&self.frames)
        {
            let path = self
                .options
                .capture
                .as_ref()
                .ok_or("motion.capture_directory")?
                .join(format!("motion-frame-{:03}.png", self.frames));
            let ticket = renderer
                .capture_next_frame(CaptureRegion {
                    origin: [0, 0],
                    extent: [extent.width, extent.height],
                })
                .map_err(|e| e.to_string())?;
            self.capture_frames.push(self.frames);
            self.captures.push(std::thread::spawn(move || {
                let readback = ticket
                    .wait(Duration::from_secs(8))
                    .map_err(|e| e.to_string())?;
                let pixels = readback.rgba().map_err(|e| e.to_string())?;
                image::save_buffer_with_format(
                    &path,
                    &pixels,
                    readback.region.extent[0],
                    readback.region.extent[1],
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )
                .map_err(|e| e.to_string())?;
                println!(
                    "motion_capture: frame={} path={}",
                    readback.frame,
                    path.display()
                );
                Ok(())
            }));
        }
        let output = renderer
            .render_with_scene(
                viewport,
                &paint,
                &mut self.context,
                &mut self.rasterizer,
                None,
                RenderOptions {
                    clear: if self.model.visible(Layer::Background) {
                        [0.002, 0.004, 0.005, 1.0]
                    } else {
                        [0.035, 0.045, 0.05, 1.0]
                    },
                    selection: MINT,
                    caret: MINT,
                    caret_visible: false,
                },
                false,
                Some((preview::camera(&self.model), &scene, rect)),
            )
            .map_err(|e| e.to_string())?;
        self.retry = renderer.frame_retry().map(|r| r.deadline);
        if output.is_some() {
            self.updates.presented();
            self.frames += 1;
            let update = self.semantics.build(
                &widget.element,
                &layout,
                self.input.focus().map(|t| t.element),
                "Vesta Motion",
            );
            if let Some(adapter) = &mut self.accessibility {
                adapter.update_if_active(|| update);
            }
            self.frame = Some(Frame {
                hits: HitTestTree::build(&widget.element, &layout),
                route,
                layout,
            });
        }
        Ok(())
    }
    fn smoke(&mut self) -> Result<(), String> {
        if !self.options.smoke || self.smoke_checked || self.frames < 4 || self.frame.is_none() {
            return Ok(());
        }
        let point = |frame: &Frame, id| -> Result<Point, String> {
            let r = frame.layout.node(id).ok_or("motion.smoke_geometry")?.rect;
            Ok(Point::new(r.x + r.width * 0.5, r.y + r.height * 0.5))
        };
        let initial = self.model.study;
        for study in Study::ALL {
            self.pointer = point(
                self.frame.as_ref().ok_or("motion.smoke_frame")?,
                self.model.id(study.slot()),
            )?;
            self.mouse(ElementState::Pressed);
            self.mouse(ElementState::Released);
            if self.model.study != study {
                return Err(format!("motion.smoke_study_switch: {}", study.key()));
            }
        }
        self.apply(Action::Study(initial));
        self.pointer = point(
            self.frame.as_ref().ok_or("motion.smoke_frame")?,
            self.model.id(PLAY),
        )?;
        self.mouse(ElementState::Pressed);
        self.mouse(ElementState::Released);
        if self.model.playing {
            return Err("motion.smoke_pause_failed".into());
        }
        let rect = self
            .frame
            .as_ref()
            .ok_or("motion.smoke_frame")?
            .layout
            .node(self.model.id(SEEK))
            .ok_or("motion.smoke_timeline")?
            .rect;
        self.pointer = Point::new(rect.x + 10.0 + (rect.width - 20.0) * 0.5, rect.y + 40.0);
        self.mouse(ElementState::Pressed);
        self.mouse(ElementState::Released);
        if (self.model.time - 6.0).abs() > 0.01 {
            return Err("motion.smoke_seek_failed".into());
        }
        let rect = self
            .frame
            .as_ref()
            .ok_or("motion.smoke_frame")?
            .layout
            .node(self.model.id(Parameter::Count.slot() + 1))
            .ok_or("motion.smoke_parameter")?
            .rect;
        let thumb_width = self
            .frame
            .as_ref()
            .ok_or("motion.smoke_frame")?
            .layout
            .node(self.model.id(Parameter::Count.slot() + 3))
            .ok_or("motion.smoke_slider_thumb")?
            .rect
            .width;
        self.pointer = Point::new(
            rect.x + thumb_width * 0.5 + (rect.width - thumb_width) * 0.25,
            rect.y + 2.0,
        );
        self.mouse(ElementState::Pressed);
        self.mouse(ElementState::Released);
        if self.model.params().count != 42 {
            return Err(format!(
                "motion.smoke_parameter_failed: {}",
                self.model.params().count
            ));
        }
        self.apply(Action::Reset);
        self.apply(Action::Play);
        self.input.set_focus(None);
        self.smoke_checked = true;
        println!(
            "motion_controls: studies=6 pause=true seek=6 count=42 capture_released={} (source-controlled public hit/action routes; not physical input qualification)",
            self.input.capture().is_none()
        );
        Ok(())
    }
}

fn physical(rect: Rect, scale: f32, extent: [u32; 2]) -> ViewportRect {
    let x = (rect.x * scale).round().clamp(0.0, extent[0] as f32) as u32;
    let y = (rect.y * scale).round().clamp(0.0, extent[1] as f32) as u32;
    let right = (rect.right() * scale)
        .round()
        .clamp(x as f32, extent[0] as f32) as u32;
    let bottom = (rect.bottom() * scale)
        .round()
        .clamp(y as f32, extent[1] as f32) as u32;
    ViewportRect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    }
}

impl ApplicationHandler<AccessibilityEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<(), Box<dyn Error>> {
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title(format!("Vesta Motion — {}", self.model.study.title()))
                        .with_theme(Some(Theme::Dark))
                        .with_visible(false)
                        .with_inner_size(LogicalSize::new(1600.0, 960.0))
                        .with_min_inner_size(LogicalSize::new(1140.0, 760.0)),
                )?,
            );
            self.accessibility = Some(AccessibilityAdapter::with_event_loop_proxy(
                event_loop,
                &window,
                self.proxy.clone(),
            ));
            let creation = RendererCreation::start_with_scene(event_loop, window.clone(), true)?;
            self.monitor = Some(creation.retirement_monitor());
            self.creation = Some(creation);
            self.window = Some(window);
            Ok(())
        })();
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(window) = self.window.clone().filter(|w| w.id() == id) else {
            return;
        };
        if let Some(adapter) = &mut self.accessibility {
            adapter.process_event(&window, &event);
        }
        if observe_focus_visibility(&event, &mut self.input) {
            self.redraw();
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.draw().and_then(|_| self.smoke()) {
                    self.fail(event_loop, error);
                }
            }
            WindowEvent::Resized(size) => {
                if self.resize_requested {
                    let expected = size.width == (1140.0 * window.scale_factor()) as u32
                        && size.height == (760.0 * window.scale_factor()) as u32;
                    if expected && !self.resized {
                        self.resized = true;
                        self.smoke_checked = false;
                    }
                }
                self.cancel_drag(true);
                self.frame = None;
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
                self.redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.cancel_drag(true);
                self.frame = None;
                self.redraw();
            }
            WindowEvent::Focused(false) => {
                self.cancel_drag(true);
                self.input.set_focus(None);
                self.redraw();
            }
            WindowEvent::Occluded(value) => {
                self.occluded = value;
                self.tick = Instant::now();
                if !value {
                    self.redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = logical_point(position, window.scale_factor());
                if let Some(drag) = self.drag {
                    self.drag_to(drag.action);
                } else if let Some(frame) = &self.frame {
                    let hovered = frame
                        .hits
                        .hit(self.pointer)
                        .and_then(|hit| frame.route.target_for_hit(hit))
                        .map(|(id, _)| id);
                    if self.model.hovered != hovered {
                        self.model.hovered = hovered;
                        self.redraw();
                    }
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => self.mouse(state),
            WindowEvent::ModifiersChanged(value) => self.shift = value.state().shift_key(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                self.key(&event.logical_key)
            }
            _ => (),
        }
    }
    fn user_event(&mut self, _: &ActiveEventLoop, event: AccessibilityEvent) {
        if self
            .window
            .as_ref()
            .is_none_or(|w| w.id() != event.window_id)
        {
            return;
        }
        match event.window_event {
            AccessibilityWindowEvent::InitialTreeRequested => self.redraw(),
            AccessibilityWindowEvent::ActionRequested(request) => {
                let target = self.frame.as_ref().and_then(|f| {
                    FocusScope::new(WINDOW, &f.route, None).accessible_target(
                        &self.semantics,
                        request.target,
                        request.action,
                    )
                });
                if let Some(target) = target {
                    self.input.observe_focus_input(FocusInput::Keyboard);
                    self.input.set_focus(Some(target));
                    let action = self
                        .frame
                        .as_ref()
                        .and_then(|f| f.route.action_at(target.element))
                        .copied();
                    if request.action == accesskit::Action::Click {
                        if let Some(action) = action {
                            self.apply(action);
                        }
                    } else if request.action == accesskit::Action::SetValue
                        && let (Some(action), Some(accesskit::ActionData::NumericValue(value))) =
                            (action, request.data)
                    {
                        match action {
                            Action::Parameter(p) => self.model.set(p, value as f32),
                            Action::Seek => self.model.seek(value as f32 / DURATION),
                            _ => (),
                        }
                    }
                    self.redraw();
                }
            }
            AccessibilityWindowEvent::AccessibilityDeactivated => (),
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(creation) = &mut self.creation {
            match creation.poll() {
                RecoveryPoll::Ready(mut renderers) if renderers.len() == 1 => {
                    self.renderer = renderers.pop().map(Box::new);
                    self.creation = None;
                    if let Some(window) = &self.window
                        && !self.options.hidden
                    {
                        window.set_visible(true);
                    }
                    self.tick = Instant::now();
                    self.redraw();
                }
                RecoveryPoll::Pending { deadline } => self.retry = Some(deadline),
                RecoveryPoll::Failed(error) => {
                    self.fail(event_loop, error);
                    return;
                }
                _ => {
                    self.fail(event_loop, "motion.renderer_creation_count");
                    return;
                }
            }
        }
        let now = Instant::now();
        let visible = self.options.hidden
            || !self.occluded
                && self
                    .window
                    .as_ref()
                    .is_some_and(|w| w.is_minimized() != Some(true));
        if visible {
            let old_study = self.model.study;
            self.model
                .advance(now.duration_since(self.tick).as_secs_f32().min(0.25));
            if old_study != self.model.study
                && let Some(window) = &self.window
            {
                println!(
                    "motion_study: {} -> {} (demo reel)",
                    old_study.key(),
                    self.model.study.key()
                );
                window.set_title(&format!("Vesta Motion — {}", self.model.study.title()));
            }
        }
        self.tick = now;
        if self.options.smoke && self.smoke_checked && self.frames >= 85 && !self.resize_requested {
            if let Some(window) = &self.window {
                let _ = window.request_inner_size(LogicalSize::new(1140.0, 760.0));
            }
            self.resize_requested = true;
        }
        if self.options.smoke
            && self.frames >= 105
            && self.resized
            && self.captures.iter().all(|job| job.is_finished())
        {
            println!(
                "motion_smoke_complete: frames={} elapsed={:.2}s captures={} controls={} resize=true",
                self.frames,
                self.started.elapsed().as_secs_f64(),
                self.capture_frames.len(),
                self.smoke_checked
            );
            event_loop.exit();
            return;
        }
        if self
            .options
            .observe
            .is_some_and(|duration| now.duration_since(self.started) >= duration)
        {
            event_loop.exit();
            return;
        }
        let smoke_timeout = self
            .options
            .smoke_timeout
            .unwrap_or(Duration::from_secs(40));
        if self.options.smoke && now.duration_since(self.started) > smoke_timeout {
            self.fail(event_loop, "motion.smoke_timeout");
            return;
        }
        let continuous = visible
            && (self.model.playing
                || self.options.smoke
                || self.options.capture.is_some() && self.frames < 68);
        if self.retry.is_some_and(|at| at <= now) {
            self.retry = None;
            self.redraw();
        }
        if continuous && now >= self.next {
            self.next = now + PERIOD;
            if self.options.hidden {
                // Hidden native windows do not receive WM_PAINT. The finite GPU
                // probe still uses this exact layout/presentation path on its
                // bounded event-loop deadline, without showing or focusing it.
                if let Err(error) = self.draw().and_then(|_| self.smoke()) {
                    self.fail(event_loop, error);
                    return;
                }
            } else {
                self.redraw();
            }
        }
        let mut deadline = self.retry;
        if continuous {
            deadline = Some(deadline.map_or(self.next, |at| at.min(self.next)));
        }
        if let Some(duration) = self.options.observe {
            let at = self.started + duration;
            deadline = Some(deadline.map_or(at, |old| old.min(at)));
        }
        if self.options.smoke {
            let at = self.started + smoke_timeout;
            deadline = Some(deadline.map_or(at, |old| old.min(at)));
        }
        event_loop.set_control_flow(deadline.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.cancel_drag(true);
        self.frame = None;
    }
    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.cancel_drag(true);
        if let Some(creation) = &mut self.creation {
            creation.cancel();
        }
        self.creation = None;
        self.updates.cancel();
        self.renderer = None;
        self.frame = None;
    }
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let mut options = Options::default();
    for arg in std::env::args().skip(1) {
        if arg == "--smoke" {
            options.smoke = true;
        } else if let Some(value) = arg.strip_prefix("--smoke-timeout-seconds=") {
            let seconds: u64 = value.parse()?;
            if !(40..=300).contains(&seconds) {
                return Err("motion.smoke_timeout_seconds: expected 40..300".into());
            }
            options.smoke_timeout = Some(Duration::from_secs(seconds));
        } else if arg == "--tour" {
            options.tour = true;
        } else if let Some(key) = arg.strip_prefix("--study=") {
            options.study =
                Study::from_key(key).ok_or_else(|| format!("motion.study: unknown study {key}"))?;
        } else if arg == "--hidden" {
            options.hidden = true;
        } else if let Some(path) = arg.strip_prefix("--capture-dir=") {
            options.capture = Some(PathBuf::from(path));
        } else if let Some(value) = arg.strip_prefix("--observe-seconds=") {
            let seconds: u64 = value.parse()?;
            if !(1..=300).contains(&seconds) {
                return Err("motion.observe_seconds: expected 1..300".into());
            }
            options.observe = Some(Duration::from_secs(seconds));
        } else {
            return Err(format!("motion.argument: {arg}; use --study=<key>, --tour, --smoke, --hidden, --capture-dir=<path>, --observe-seconds=<1..300>").into());
        }
    }
    if options.hidden && !options.smoke && options.observe.is_none() {
        return Err("motion.hidden: finite --smoke or --observe-seconds is required".into());
    }
    if options.smoke_timeout.is_some() && !options.smoke {
        return Err("motion.smoke_timeout_seconds: requires --smoke".into());
    }
    if let Some(path) = &options.capture {
        std::fs::create_dir_all(path)?;
    }
    let mut context = TextInputContext::default();
    for path in [
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\seguisym.ttf",
        r"C:\Windows\Fonts\msyh.ttc",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            context.register_font(Arc::new(bytes));
        }
    }
    let event_loop = EventLoop::<AccessibilityEvent>::with_user_event().build()?;
    let now = Instant::now();
    let mut model = Model::new()?;
    model.apply(Action::Study(options.study));
    if options.tour {
        model.apply(Action::Tour);
    }
    let mut app = App {
        model,
        window: None,
        creation: None,
        renderer: None,
        updates: Default::default(),
        monitor: None,
        proxy: event_loop.create_proxy(),
        accessibility: None,
        semantics: Default::default(),
        context,
        rasterizer: Default::default(),
        layout: Default::default(),
        frame: None,
        input: Default::default(),
        click: Default::default(),
        pointer: Point::new(-1.0, -1.0),
        drag: None,
        shift: false,
        occluded: false,
        started: now,
        tick: now,
        next: now,
        retry: None,
        frames: 0,
        smoke_checked: false,
        resize_requested: false,
        resized: false,
        options,
        captures: Vec::new(),
        capture_frames: Vec::new(),
        error: None,
    };
    event_loop.run_app(&mut app)?;
    let monitor = app.monitor.take();
    let captures = std::mem::take(&mut app.captures);
    let error = app.error.take();
    drop(app);
    if let Some(monitor) = monitor {
        monitor.observe(Duration::from_secs(10))?;
    }
    for capture in captures {
        capture
            .join()
            .map_err(|_| "motion.capture_worker_panicked")??;
    }
    if let Some(error) = error {
        return Err(error.into());
    }
    Ok(())
}
