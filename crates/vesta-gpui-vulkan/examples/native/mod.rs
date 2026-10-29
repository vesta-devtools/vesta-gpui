//! Reference host composition, not a framework-owned runtime. ADR-0356 decision 9.
mod profile;
mod view;
use std::{
    error::Error,
    sync::Arc,
    time::{Duration, Instant},
};
use vesta_gpui::{
    AccessibilityTree, ActionRoute, ElementId, ElementIdAllocator, FocusInput, FocusScope,
    HitTestTree, InputOwnership, InputTarget, LayoutTree, PaintList, Point, PointerButton,
    PointerClick, RetainedLayoutEngine, TextEditKind, TextInputAction, TextInputContext,
    TextInputState, TextInputViewport, TextRasterizer, UiWindowId, Viewport,
};
use vesta_gpui_vulkan::{
    FrameCpuProfile, RecoveryPoll, RenderOptions, RendererCreation, RendererUpdates,
    RetirementMonitor, TargetConfiguration, WindowRenderer,
};
use vesta_gpui_widgets::{ScrollInputPath, WidgetTheme};
use vesta_gpui_winit::{
    AccessibilityAdapter, AccessibilityEvent, AccessibilityWindowEvent, Clipboard, TextInputBridge,
    TextInputResponse, logical_point, observe_focus_visibility, wheel_delta,
};
pub use view::Mode;
use view::{Action, EDITOR, INCREMENT, Images, Model, SCROLL};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

const WINDOW: UiWindowId = UiWindowId(1);
const CARET_PERIOD: Duration = Duration::from_millis(500);

#[derive(Default)]
struct SystemClipboard(Option<arboard::Clipboard>);
impl SystemClipboard {
    fn get(&mut self) -> Result<&mut arboard::Clipboard, String> {
        if self.0.is_none() {
            self.0 = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
        }
        self.0
            .as_mut()
            .ok_or_else(|| "clipboard unavailable".into())
    }
}
impl Clipboard for SystemClipboard {
    type Error = String;
    fn read_text(&mut self) -> Result<String, String> {
        self.get()?.get_text().map_err(|e| e.to_string())
    }
    fn write_text(&mut self, text: &str) -> Result<(), String> {
        self.get()?
            .set_text(text.to_owned())
            .map_err(|e| e.to_string())
    }
}

struct Frame {
    route: ActionRoute<Action>,
    layout: LayoutTree,
    hits: HitTestTree,
    editor: Option<TextInputViewport>,
}

struct Host {
    model: Model,
    context: TextInputContext,
    rasterizer: TextRasterizer,
    layout: RetainedLayoutEngine,
    semantics: AccessibilityTree,
    bridge: TextInputBridge,
    clipboard: SystemClipboard,
    images: Images,
    click: PointerClick,
    pointer: Point,
    frame: Option<Frame>,
    window: Option<Arc<Window>>,
    proxy: winit::event_loop::EventLoopProxy<AccessibilityEvent>,
    accessibility: Option<AccessibilityAdapter>,
    creation: Option<RendererCreation>,
    renderer: Option<Box<WindowRenderer>>,
    updates: RendererUpdates,
    monitor: Option<RetirementMonitor>,
    wake: Option<Instant>,
    caret_at: Instant,
    caret_visible: bool,
    active: bool,
    error: Option<String>,
    smoke: bool,
    smoke_at: Instant,
    smoke_changed: bool,
    smoke_resized: bool,
    frames: u64,
    profile: Option<profile::Profile>,
}

impl Host {
    fn request_frame(&mut self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl std::fmt::Display) {
        self.error = Some(error.to_string());
        event_loop.exit();
    }
    fn target(&self, local: u32) -> InputTarget {
        InputTarget {
            window: WINDOW,
            element: self.model.id(local),
        }
    }
    fn editor_focused(&self) -> bool {
        self.model.input.focus() == Some(self.target(EDITOR))
    }
    fn focus(&mut self, target: Option<InputTarget>) {
        if self.model.input.focus() != target {
            self.model.text.cancel_compose(&mut self.context);
            self.model.text.end_group();
            self.bridge.clear_ime_session();
            self.model.input.cancel_capture();
            self.click.cancel();
            self.model.input.set_focus(target);
        }
        self.caret_visible = true;
        self.caret_at = Instant::now() + CARET_PERIOD;
        self.sync_ime();
        self.request_frame();
    }
    fn sync_ime(&mut self) {
        let enabled = self.active && self.editor_focused();
        if let Some(window) = &self.window {
            self.bridge
                .sync_ime(window, enabled, self.frame.as_ref().and_then(|f| f.editor));
        }
    }
    fn traverse(&mut self, backwards: bool) {
        let Some(frame) = &self.frame else {
            return;
        };
        let mut input = self.model.input.clone();
        FocusScope::new(WINDOW, &frame.route, None).traverse(&mut input, backwards);
        let target = input.focus();
        self.model.input.observe_focus_input(FocusInput::Keyboard);
        self.focus(target);
    }
    fn apply(&mut self, action: Action) {
        match action {
            Action::Increment => self.model.count = self.model.count.saturating_add(1),
            Action::Theme => {
                self.model.theme = if self.model.theme == WidgetTheme::dark() {
                    WidgetTheme::light()
                } else {
                    WidgetTheme::dark()
                }
            }
        }
        self.request_frame();
    }
    fn draw(&mut self) -> Result<(), String> {
        let started = self.profile.as_ref().map(|_| Instant::now());
        let Some(window) = self.window.clone() else {
            return Ok(());
        };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 || window.is_minimized() == Some(true) {
            return Ok(());
        }
        if !self
            .updates
            .prepare(
                &mut self.renderer,
                TargetConfiguration::ui([size.width, size.height]),
            )
            .map_err(|e| e.to_string())?
        {
            self.frame = None;
            self.wake = Some(Instant::now() + Duration::from_millis(16));
            return Ok(());
        }
        let scale = window.scale_factor() as f32;
        let viewport = Viewport::new(size.width as f32 / scale, size.height as f32 / scale, scale);
        let widget = self.model.view();
        let layout = self
            .layout
            .compute_with_text_context(&widget.element, viewport, &mut self.context)
            .map_err(|e| e.to_string())?;
        let route = ActionRoute::build(&widget.element, widget.actions)
            .map_err(|e| format!("action route: {e:?}"))?;
        FocusScope::new(WINDOW, &route, None).reconcile(&mut self.model.input);
        let scroll_changed = layout
            .node(self.model.id(SCROLL))
            .is_some_and(|node| self.model.scroll.sync_layout(node));
        let paint = PaintList::build_with_focus(
            &widget.element,
            &layout,
            self.model.input.visible_focus().map(|t| t.element),
            self.model.theme.palette.focus,
        );
        let renderer = self.renderer.as_mut().ok_or("current renderer missing")?;
        if self.profile.is_some() {
            renderer.enable_cpu_profiling(true);
        }
        let output = renderer
            .render(
                viewport,
                &paint,
                &mut self.context,
                &mut self.rasterizer,
                Some(&self.images),
                RenderOptions {
                    clear: vesta_gpui::render::linear_color(self.model.theme.palette.canvas),
                    selection: self.model.theme.palette.selection,
                    caret: self.model.theme.palette.text,
                    caret_visible: self.active && self.caret_visible,
                },
                false,
            )
            .map_err(|e| e.to_string())?;
        let Some(output) = output else {
            self.wake = renderer.frame_retry().map(|r| r.deadline);
            return Ok(());
        };
        let native_profile = renderer.cpu_profile();
        if !output.image_diagnostics.is_empty() {
            return Err(format!("host image: {:?}", output.image_diagnostics));
        }
        self.updates.presented();
        self.frames += 1;
        self.wake = None;
        let editor = output
            .text_inputs
            .iter()
            .find(|(id, _)| *id == self.model.id(EDITOR))
            .map(|(_, view)| *view);
        let mut update = self.semantics.build(
            &widget.element,
            &layout,
            self.model.input.focus().map(|t| t.element),
            self.model.mode.title(),
        );
        if let Some(view) = editor {
            self.semantics.enrich_text_input(
                &mut update,
                self.model.id(EDITOR),
                &mut self.model.text,
                &mut self.context,
                view.origin,
            );
        }
        if let Some(adapter) = &mut self.accessibility {
            adapter.update_if_active(|| update);
        }
        self.frame = Some(Frame {
            hits: HitTestTree::build(&widget.element, &layout),
            route,
            layout,
            editor,
        });
        self.sync_ime();
        if scroll_changed {
            self.request_frame();
        }
        self.smoke_step()?;
        if let (Some(profile), Some(started)) = (&mut self.profile, started) {
            profile.observe(
                started.elapsed(),
                native_profile,
                [size.width, size.height],
                window.scale_factor(),
            )?;
        }
        Ok(())
    }
    // Source-controlled public-route check plus real OS resize/presentation. Not physical input.
    fn smoke_step(&mut self) -> Result<(), String> {
        if !self.smoke || self.smoke_changed {
            return Ok(());
        }
        let frame = self.frame.as_ref().ok_or("smoke frame missing")?;
        let rect = frame
            .layout
            .node(self.model.id(INCREMENT))
            .ok_or("smoke button geometry missing")?
            .rect;
        let hit = frame.hits.hit(Point::new(
            rect.x + rect.width / 2.0,
            rect.y + rect.height / 2.0,
        ));
        let mut click = PointerClick::default();
        click.press(WINDOW, PointerButton::Primary, &frame.route, hit);
        let action = click
            .release(WINDOW, PointerButton::Primary, &frame.route, hit)
            .copied()
            .ok_or("smoke public action did not route")?;
        self.apply(action);
        if matches!(self.model.mode, Mode::Interaction) {
            self.model
                .text
                .act(&mut self.context, TextInputAction::SelectAll);
            self.model
                .text
                .insert(&mut self.context, "SDK edit · 中文", TextEditKind::Paste)
                .map_err(|e| format!("smoke edit: {e:?}"))?;
            self.model
                .text
                .act(&mut self.context, TextInputAction::Undo);
            self.model
                .text
                .act(&mut self.context, TextInputAction::Redo);
            if self.model.text.text() != "SDK edit · 中文" {
                return Err("smoke undo/redo mismatch".into());
            }
        }
        self.smoke_changed = true;
        if let Some(window) = &self.window {
            let _ = window.request_inner_size(LogicalSize::new(720.0, 560.0));
        }
        self.request_frame();
        Ok(())
    }
    fn pointer_event(&mut self, state: ElementState) {
        let Some(frame) = &self.frame else {
            return;
        };
        let hit = frame.hits.hit(self.pointer);
        if state == ElementState::Released {
            let action = self
                .click
                .release(WINDOW, PointerButton::Primary, &frame.route, hit)
                .copied();
            self.model
                .input
                .release_pointer(WINDOW, PointerButton::Primary);
            if let Some(action) = action {
                self.apply(action);
            }
            self.request_frame();
            return;
        }
        if let Some(view) = frame
            .editor
            .filter(|view| view.bounds.contains(self.pointer) && view.clip.contains(self.pointer))
        {
            self.focus(Some(self.target(EDITOR)));
            self.model.text.point(
                &mut self.context,
                view.to_layout(self.pointer),
                self.bridge.modifiers().shift_key(),
                false,
            );
            self.model
                .input
                .capture_pointer(self.target(EDITOR), PointerButton::Primary);
        } else {
            let target = hit
                .and_then(|hit| frame.route.target_for_hit(hit))
                .map(|(element, _)| InputTarget {
                    window: WINDOW,
                    element,
                });
            self.focus(target);
            if let Some(frame) = &self.frame
                && self
                    .click
                    .press(WINDOW, PointerButton::Primary, &frame.route, hit)
                && let Some(target) = target
            {
                self.model
                    .input
                    .capture_pointer(target, PointerButton::Primary);
            }
        }
        self.request_frame();
    }
}

impl ApplicationHandler<AccessibilityEvent> for Host {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<(), Box<dyn Error>> {
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title(self.model.mode.title())
                        .with_visible(false)
                        .with_inner_size(LogicalSize::new(640.0, 520.0))
                        .with_min_inner_size(LogicalSize::new(360.0, 400.0)),
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
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(window) = self.window.clone().filter(|window| window.id() == id) else {
            return;
        };
        if let Some(adapter) = &mut self.accessibility {
            adapter.process_event(&window, &event);
        }
        if observe_focus_visibility(&event, &mut self.model.input) {
            self.request_frame();
        }
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
            return;
        }
        if let WindowEvent::RedrawRequested = event {
            if let Err(error) = self.draw() {
                self.fail(event_loop, error);
            }
            if self.smoke
                && self.smoke_changed
                && self.smoke_resized
                && self.frames >= 3
                && self.model.count == 1
            {
                println!(
                    "native_smoke_complete: {:?} frames={} extent={:?}; source-controlled input, not platform qualification",
                    self.model.mode,
                    self.frames,
                    window.inner_size()
                );
                event_loop.exit();
            }
            if self
                .profile
                .as_ref()
                .is_some_and(profile::Profile::complete)
            {
                event_loop.exit();
            }
            return;
        }
        match &event {
            WindowEvent::Resized(size) => {
                self.smoke_resized |= size.width == (720.0 * window.scale_factor()) as u32
                    && size.height == (560.0 * window.scale_factor()) as u32;
                self.frame = None;
                self.model.input.cancel_capture();
                self.click.cancel();
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
                self.request_frame();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.frame = None;
                self.bridge.clear_ime_session();
                self.request_frame();
            }
            WindowEvent::Focused(active) => {
                self.active = *active;
                if !active {
                    self.model.text.cancel_compose(&mut self.context);
                    self.model.text.end_group();
                    self.model.input.cancel_capture();
                    self.click.cancel();
                    self.bridge.reset();
                }
                self.sync_ime();
                self.request_frame();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = logical_point(*position, window.scale_factor());
                if self
                    .model
                    .input
                    .capture()
                    .is_some_and(|c| c.target == self.target(EDITOR))
                    && let Some(view) = self.frame.as_ref().and_then(|f| f.editor)
                {
                    self.model.text.point(
                        &mut self.context,
                        view.to_layout(self.pointer),
                        true,
                        false,
                    );
                    self.request_frame();
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } if self.frame.is_some() => self.pointer_event(*state),
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(frame) = &self.frame
                    && let Some(hit) = frame.hits.hit(self.pointer)
                    && let Some(path) = ScrollInputPath::new(
                        &FocusScope::new(WINDOW, &frame.route, None),
                        InputTarget {
                            window: WINDOW,
                            element: hit,
                        },
                    )
                {
                    let movement = wheel_delta(*delta, window.scale_factor(), 32.0);
                    let scroll_id = self.model.id(SCROLL);
                    let old = self.model.scroll.offset();
                    path.scroll(Point::new(-movement.x, -movement.y), |id, delta| {
                        if id == scroll_id {
                            self.model.scroll.scroll_by_point(delta)
                        } else {
                            delta
                        }
                    });
                    if self.model.scroll.offset() != old {
                        self.request_frame();
                    }
                }
            }
            _ => (),
        }
        if self.frame.is_none() {
            return;
        }
        if self.editor_focused() {
            match self.bridge.handle_window_event(
                &event,
                &mut self.model.text,
                &mut self.context,
                &mut self.clipboard,
                false,
            ) {
                Ok(TextInputResponse::TraverseFocus { backwards }) => self.traverse(backwards),
                Ok(TextInputResponse::Submit) => self.apply(Action::Increment),
                Ok(TextInputResponse::Cancel) => {
                    self.model.input.cancel_capture();
                    self.click.cancel();
                    self.request_frame();
                }
                Ok(response) if response.is_handled() => {
                    self.caret_visible = true;
                    self.caret_at = Instant::now() + CARET_PERIOD;
                    self.request_frame();
                }
                Err(error) => eprintln!("native_text_input: {error}"),
                _ => (),
            }
        } else {
            match event {
                WindowEvent::ModifiersChanged(modifiers) => {
                    self.bridge.update_modifiers(&modifiers)
                }
                WindowEvent::KeyboardInput { event: key, .. }
                    if key.state == ElementState::Pressed =>
                {
                    match key.logical_key {
                        Key::Named(NamedKey::Tab) => {
                            self.traverse(self.bridge.modifiers().shift_key())
                        }
                        Key::Named(NamedKey::Enter | NamedKey::Space) => {
                            if let Some(action) = self
                                .frame
                                .as_ref()
                                .and_then(|f| {
                                    self.model
                                        .input
                                        .focus()
                                        .and_then(|t| f.route.action_at(t.element))
                                })
                                .copied()
                            {
                                self.apply(action);
                            }
                        }
                        Key::Named(NamedKey::Escape) => {
                            self.model.input.cancel_capture();
                            self.click.cancel();
                            self.request_frame();
                        }
                        _ => (),
                    }
                }
                _ => (),
            }
        }
    }
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AccessibilityEvent) {
        if self
            .window
            .as_ref()
            .is_none_or(|w| w.id() != event.window_id)
        {
            return;
        }
        match event.window_event {
            AccessibilityWindowEvent::InitialTreeRequested => self.request_frame(),
            AccessibilityWindowEvent::ActionRequested(request) => {
                let target = self.frame.as_ref().and_then(|f| {
                    FocusScope::new(WINDOW, &f.route, None).accessible_target(
                        &self.semantics,
                        request.target,
                        request.action,
                    )
                });
                let Some(target) = target else {
                    return;
                };
                self.model.input.observe_focus_input(FocusInput::Keyboard);
                match request.action {
                    accesskit::Action::Focus => self.focus(Some(target)),
                    accesskit::Action::Click => {
                        if let Some(action) = self
                            .frame
                            .as_ref()
                            .and_then(|f| f.route.action_at(target.element))
                            .copied()
                        {
                            self.apply(action);
                        }
                    }
                    accesskit::Action::SetValue if target == self.target(EDITOR) => {
                        if let Some(accesskit::ActionData::Value(value)) = request.data {
                            self.focus(Some(target));
                            self.model
                                .text
                                .act(&mut self.context, TextInputAction::SelectAll);
                            if let Err(error) = self.model.text.insert(
                                &mut self.context,
                                &value,
                                TextEditKind::Paste,
                            ) {
                                eprintln!("native_accessible_value: {error:?}");
                            }
                        }
                    }
                    accesskit::Action::SetTextSelection if target == self.target(EDITOR) => {
                        if let Some(accesskit::ActionData::SetTextSelection(selection)) =
                            request.data
                        {
                            self.focus(Some(target));
                            self.semantics.select_text_input(
                                target.element,
                                &mut self.model.text,
                                &mut self.context,
                                &selection,
                            );
                        }
                    }
                    _ => (),
                }
                self.request_frame();
            }
            AccessibilityWindowEvent::AccessibilityDeactivated => (),
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(creation) = &mut self.creation {
            match creation.poll() {
                RecoveryPoll::Ready(mut renderers) if renderers.len() == 1 => {
                    self.renderer = renderers.pop().map(Box::new);
                    if let Some(renderer) = &mut self.renderer {
                        renderer.enable_cpu_profiling(self.profile.is_some());
                    }
                    self.creation = None;
                    if let Some(window) = &self.window {
                        window.set_visible(true);
                    }
                    self.request_frame();
                }
                RecoveryPoll::Pending { deadline } => self.wake = Some(deadline),
                RecoveryPoll::Failed(error) => {
                    self.fail(event_loop, error);
                    return;
                }
                _ => {
                    self.fail(
                        event_loop,
                        "native_creation_result: expected exactly one renderer",
                    );
                    return;
                }
            }
        }
        let now = Instant::now();
        if let Some(profile) = &mut self.profile {
            if now.duration_since(profile.started) > profile::TIMEOUT {
                self.fail(event_loop, "native_profile_timeout");
                return;
            }
            if now >= profile.next && !profile.complete() {
                profile.next = now + profile::PERIOD;
                self.model.count = (self.frames % 10) as u32;
                self.request_frame();
            }
        }
        if self.smoke && now.duration_since(self.smoke_at) > Duration::from_secs(15) {
            self.fail(event_loop, "native_smoke_timeout");
            return;
        }
        if self.active && self.editor_focused() && now >= self.caret_at {
            self.caret_visible = !self.caret_visible;
            self.caret_at = now + CARET_PERIOD;
            self.request_frame();
        }
        if self.wake.is_some_and(|at| now >= at) {
            self.wake = None;
            self.request_frame();
        }
        if self.smoke && self.smoke_changed && self.smoke_resized && self.frames < 3 {
            self.request_frame();
        }
        let mut deadline = self.wake;
        if self.active && self.editor_focused() {
            deadline = Some(deadline.map_or(self.caret_at, |at| at.min(self.caret_at)));
        }
        if self.smoke {
            let at = self.smoke_at + Duration::from_secs(15);
            deadline = Some(deadline.map_or(at, |old| old.min(at)));
        }
        if let Some(profile) = &self.profile {
            deadline = Some(deadline.map_or(profile.next, |at| at.min(profile.next)));
        }
        event_loop.set_control_flow(deadline.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(creation) = &mut self.creation {
            creation.cancel();
        }
        self.updates.cancel();
        self.creation = None;
        self.renderer = None;
        self.bridge.reset();
        self.model.input.cancel_capture();
        self.click.cancel();
        self.frame = None;
    }
}

pub fn run(mode: Mode) -> Result<(), Box<dyn Error>> {
    let mut fonts = Vec::new();
    let mut smoke = false;
    let mut profiling = false;
    for arg in std::env::args().skip(1) {
        if arg == "--smoke" {
            smoke = true;
        } else if arg == "--profile" {
            profiling = true;
        } else if let Some(path) = arg.strip_prefix("--font=") {
            fonts.push(path.to_owned());
        } else {
            return Err(format!(
                "unknown argument {arg}; use --font=<TTF/OTF> [--font=<fallback>] [--smoke | --profile]"
            )
            .into());
        }
    }
    if fonts.is_empty() {
        return Err("pass --font=<TTF/OTF>; the host supplies fonts explicitly".into());
    }
    if smoke && profiling {
        return Err("--profile and --smoke are separate finite probes".into());
    }
    let mut context = TextInputContext::default();
    for path in fonts {
        if !context.register_font(Arc::new(std::fs::read(&path)?)) {
            return Err(format!("invalid host font: {path}").into());
        }
    }
    // register_font appends host families to sans-serif; naming only Segoe UI
    // would discard that supplied CJK fallback for this editor.
    let mut text = TextInputState::new(16.0, "sans-serif");
    text.reset(&mut context, "Hello · Vesta")
        .map_err(|e| format!("initial text: {e:?}"))?;
    let event_loop = EventLoop::<AccessibilityEvent>::with_user_event().build()?;
    let now = Instant::now();
    let mut host = Host {
        model: Model {
            mode,
            scope: ElementIdAllocator
                .allocate_scope()
                .map_err(|e| format!("element identity: {e:?}"))?,
            count: 0,
            text,
            input: InputOwnership::default(),
            scroll: Default::default(),
            theme: WidgetTheme::dark(),
        },
        context,
        rasterizer: TextRasterizer::default(),
        layout: Default::default(),
        semantics: Default::default(),
        bridge: Default::default(),
        clipboard: Default::default(),
        images: Default::default(),
        click: Default::default(),
        pointer: Point::new(-1.0, -1.0),
        frame: None,
        window: None,
        proxy: event_loop.create_proxy(),
        accessibility: None,
        creation: None,
        renderer: None,
        updates: Default::default(),
        monitor: None,
        wake: None,
        caret_at: now + CARET_PERIOD,
        caret_visible: true,
        active: false,
        error: None,
        smoke,
        smoke_at: now,
        smoke_changed: false,
        smoke_resized: false,
        frames: 0,
        profile: profiling.then(|| profile::Profile::new(now)),
    };
    event_loop.run_app(&mut host)?;
    let monitor = host.monitor.take();
    let error = host.error.take();
    let frames = host.frames;
    let profile = host.profile.take();
    drop(host);
    if let Some(monitor) = monitor {
        monitor.observe(Duration::from_secs(2))?;
        if monitor.pending() {
            return Err(
                "native_retirement_pending: observation budget expired; no completion claim".into(),
            );
        }
    }
    if let Some(error) = error {
        return Err(error.into());
    }
    if let Some(profile) = profile {
        profile.report(mode)?;
    }
    println!("native_example_closed: {mode:?} frames={frames} retirement_pending=false");
    Ok(())
}
