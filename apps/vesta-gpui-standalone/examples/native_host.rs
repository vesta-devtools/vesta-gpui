//! Small native consumer: one window/editor, no product model or framework facade.
use std::{
    error::Error,
    sync::Arc,
    time::{Duration, Instant},
};
use vesta_gpui::{
    Axis, Color, Dimension, Element, ElementId, ElementKey, InputOwnership, InputTarget, Insets,
    PaintCommand, PaintList, Point, PointerButton, RetainedLayoutEngine, Style, TextInputAction,
    TextInputContext, TextInputState, TextInputViewport, TextRasterizer, UiWindowId, Viewport,
    text_input_wrap_width,
};
use vesta_gpui_vulkan::{
    RecoveryPoll, RenderOptions, RendererCreation, RendererUpdates, RetirementMonitor,
    TargetConfiguration, WindowRenderer,
};
use vesta_gpui_winit::{
    Clipboard, TextInputBridge, TextPointerAction, UiInputEvent, UiTraceRecording, logical_point,
    wheel_delta,
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, Ime, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const FIELD: ElementId = ElementId::new(2);
const TARGET: InputTarget = InputTarget {
    window: UiWindowId(1),
    element: FIELD,
};
const TEXT: &str = "A native GPUI host / 原生文本输入\nType, select, copy, paste and Undo.\nResize the window to reflow this same editor; application text and history remain owned here.";
const WIDTHS: [f64; 5] = [760.0, 480.0, 900.0, 480.0, 760.0];

struct SystemClipboard(arboard::Clipboard);
impl Clipboard for SystemClipboard {
    type Error = arboard::Error;
    fn read_text(&mut self) -> std::result::Result<String, Self::Error> {
        self.0.get_text()
    }
    fn write_text(&mut self, text: &str) -> std::result::Result<(), Self::Error> {
        self.0.set_text(text.to_owned())
    }
}

struct Host {
    window: Option<Arc<Window>>,
    creation: Option<RendererCreation>,
    renderer: Option<Box<WindowRenderer>>,
    updates: RendererUpdates,
    monitor: Option<RetirementMonitor>,
    context: TextInputContext,
    input: TextInputState,
    layout: RetainedLayoutEngine,
    rasterizer: TextRasterizer,
    bridge: TextInputBridge,
    clipboard: SystemClipboard,
    ownership: InputOwnership,
    view: Option<TextInputViewport>,
    pointer: Point,
    scroll: Point,
    reveal_caret: bool,
    active: bool,
    occluded: bool,
    retry: Option<Instant>,
    error: Option<Box<dyn Error>>,
    smoke: bool,
    step: usize,
    started: Instant,
}

impl Host {
    fn redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
    fn suspend_geometry(&mut self) {
        self.view = None;
        self.ownership
            .release_pointer(TARGET.window, PointerButton::Primary);
        self.bridge.clear_text_pointer();
    }
    fn end_text_session(&mut self) {
        self.input.cancel_compose(&mut self.context);
        self.input.end_group();
        self.bridge.reset();
        if let Some(window) = &self.window {
            self.bridge.sync_ime(window, false, None);
        }
        self.suspend_geometry();
    }
    // This recipe's field bounds are fixed by window size, not by its content.
    // Refresh text/scroll origin immediately so queued input never uses old glyph geometry.
    fn refresh_view(&mut self) {
        if let Some(old) = self.view {
            let view = self
                .input
                .snapshot()
                .with_viewport_scroll(self.scroll, self.reveal_caret)
                .viewport(old.bounds, old.clip, self.scroll);
            self.scroll = Point::new(view.scroll_x, view.scroll_y);
            self.view = Some(view);
        }
    }
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl Into<Box<dyn Error>>) {
        self.error = Some(error.into());
        event_loop.exit();
    }
    fn tree(&self, viewport: Viewport) -> Element {
        Element::container(ElementId::new(1), ElementKey::new("recipe.root"))
            .styled(
                Style::default()
                    .size(
                        Dimension::Px(viewport.width),
                        Dimension::Px(viewport.height),
                    )
                    .axis(Axis::Column)
                    .gap(16.0)
                    .padding(Insets::all(24.0))
                    .background(Color::rgb8(20, 23, 29)),
            )
            .child(
                Element::text(
                    ElementId::new(3),
                    ElementKey::new("recipe.title"),
                    "One window, one editor / 单窗口宿主",
                )
                .styled(
                    Style::default()
                        .size(Dimension::Percent(1.0), Dimension::Px(28.0))
                        .text(18.0, Color::rgb8(230, 234, 242)),
                ),
            )
            .child(
                Element::text_input(
                    FIELD,
                    ElementKey::new("recipe.editor"),
                    self.input
                        .snapshot()
                        .with_viewport_scroll(self.scroll, self.reveal_caret),
                    self.active && !self.occluded,
                )
                .styled(
                    Style::default()
                        .size(
                            Dimension::Percent(1.0),
                            Dimension::Px((viewport.height - 92.0).max(1.0)),
                        )
                        .background(Color::rgb8(31, 36, 45))
                        .border(1.0, Color::rgb8(91, 141, 214))
                        .clipped(),
                ),
            )
    }
    fn draw(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        if event_loop.exiting() {
            return Ok(());
        }
        let _frame = vesta_gpui::trace::frame("Recipe.Main");
        let window = self.window.as_ref().ok_or("recipe.window_missing")?.clone();
        let size = window.inner_size();
        self.view = None;
        if (self.occluded && !self.smoke) || size.width == 0 || size.height == 0 {
            self.suspend_geometry();
            return Ok(());
        }
        let desired = TargetConfiguration::ui([size.width, size.height]);
        if !self.updates.prepare(&mut self.renderer, desired)? {
            self.suspend_geometry();
            return Ok(());
        }
        let scale = window.scale_factor() as f32;
        let viewport = Viewport::new(size.width as f32 / scale, size.height as f32 / scale, scale);
        let mut passes = 0;
        let paint = loop {
            passes += 1;
            if passes > 8 {
                return Err("recipe.layout_budget_exhausted".into());
            }
            let root = self.tree(viewport);
            let layout =
                self.layout
                    .compute_with_text_context(&root, viewport, &mut self.context)?;
            let bounds = layout.node(FIELD).ok_or("recipe.editor_missing")?.rect;
            if self
                .input
                .set_wrap_width(&mut self.context, text_input_wrap_width(bounds))
            {
                continue;
            }
            break PaintList::build(&root, &layout);
        };
        // Install current CPU geometry before acquire. A deferred frame cannot
        // mix this layout with a previous renderer's text geometry.
        let view = paint
            .commands()
            .iter()
            .find_map(|command| match command {
                PaintCommand::TextInput {
                    element,
                    bounds,
                    clip,
                    snapshot,
                    ..
                } if *element == FIELD => Some(snapshot.viewport(*bounds, *clip, self.scroll)),
                _ => None,
            })
            .ok_or("recipe.editor_paint_missing")?;
        self.scroll = Point::new(view.scroll_x, view.scroll_y);
        self.reveal_caret = false;
        self.view = Some(view);
        vesta_gpui::trace::counter("recipe", "layout.passes", f64::from(passes));
        let renderer = self.renderer.as_mut().ok_or("recipe.renderer_missing")?;
        let output = renderer.render(
            viewport,
            &paint,
            &mut self.context,
            &mut self.rasterizer,
            None,
            RenderOptions {
                clear: [0.02, 0.025, 0.035, 1.0],
                selection: Color::rgb8(60, 94, 155),
                caret: Color::rgb8(235, 240, 250),
                caret_visible: self.active && !self.occluded,
            },
            false,
        )?;
        self.retry = renderer.frame_retry().map(|retry| retry.deadline);
        if let Some(output) = output {
            if output
                .text_inputs
                .iter()
                .find(|(element, _)| *element == FIELD)
                .map(|(_, view)| *view)
                != Some(view)
            {
                return Err("recipe.cpu_painter_geometry_mismatch".into());
            }
            self.updates.presented();
            self.bridge
                .sync_ime(&window, self.active && !self.occluded, Some(view));
            if self.smoke {
                self.smoke_presented(event_loop, viewport)?;
            }
        }
        Ok(())
    }
    fn smoke_presented(&mut self, event_loop: &ActiveEventLoop, viewport: Viewport) -> Result<()> {
        if (f64::from(viewport.width) - WIDTHS[self.step]).abs() > 1.0 {
            return Ok(());
        }
        if self.step != 2 && self.input.text() != TEXT {
            return Err("recipe.draft_changed_on_resize".into());
        }
        if self.step == 2 && !self.input.text().contains("logical edit") {
            return Err("recipe.edit_not_applied".into());
        }
        println!(
            "recipe_presented step={} width={} text_bytes={} passes_geometry=true",
            self.step,
            viewport.width,
            self.input.text().len()
        );
        vesta_gpui::trace::counter("recipe", "smoke.step", self.step as f64);
        if self.step == 1 {
            self.bridge.handle_window_event(
                &UiInputEvent::Text(" — logical edit".into()),
                &mut self.input,
                &mut self.context,
                &mut self.clipboard,
                false,
            )?;
        } else if self.step == 2 && !self.input.act(&mut self.context, TextInputAction::Undo) {
            return Err("recipe.undo_unavailable".into());
        }
        self.step += 1;
        if self.step == WIDTHS.len() {
            event_loop.exit();
        } else {
            self.reveal_caret = true;
            self.suspend_geometry();
            let _ = self
                .window
                .as_ref()
                .ok_or("recipe.window_missing")?
                .request_inner_size(LogicalSize::new(WIDTHS[self.step], 360.0));
            self.retry = Some(Instant::now() + Duration::from_millis(16));
            self.redraw();
        }
        Ok(())
    }
    fn text_event(&mut self, event: &WindowEvent) -> Result<()> {
        let hit = self
            .view
            .is_some_and(|view| view.clip.contains(self.pointer));
        self.bridge.observe_text_pointer_event(event, hit);
        if self.view.is_some() && self.active && !self.occluded
            || matches!(
                event,
                WindowEvent::ModifiersChanged(_) | WindowEvent::Ime(Ime::Disabled)
            )
        {
            let response = self.bridge.handle_window_event(
                event,
                &mut self.input,
                &mut self.context,
                &mut self.clipboard,
                false,
            )?;
            if response.is_handled() {
                self.reveal_caret = true;
                self.refresh_view();
                self.redraw();
            }
        }
        let Some(view) = self.view else {
            return Ok(());
        };
        match event {
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } if hit => {
                if let Some(action) = self.bridge.text_pointer_press(
                    TARGET,
                    self.pointer,
                    Instant::now(),
                    self.input.is_composing(),
                ) {
                    self.ownership
                        .capture_pointer(TARGET, PointerButton::Primary);
                    self.input.point(
                        &mut self.context,
                        view.to_layout(self.pointer),
                        action == TextPointerAction::Extend,
                        action == TextPointerAction::Word,
                    );
                    self.reveal_caret = true;
                    self.refresh_view();
                    self.redraw();
                }
            }
            WindowEvent::CursorMoved { .. } if self.ownership.capture().is_some() => {
                if self.bridge.text_pointer_drag(TARGET, self.pointer) {
                    self.input
                        .point(&mut self.context, view.to_layout(self.pointer), true, false);
                    self.reveal_caret = true;
                    self.refresh_view();
                    self.redraw();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                self.ownership
                    .release_pointer(TARGET.window, PointerButton::Primary);
            }
            WindowEvent::MouseWheel { delta, .. } if hit => {
                self.scroll.y -= wheel_delta(*delta, f64::from(self.layout_view_scale()), 32.0).y;
                self.reveal_caret = false;
                self.refresh_view();
                self.redraw();
            }
            _ => (),
        }
        Ok(())
    }
    fn layout_view_scale(&self) -> f32 {
        self.window
            .as_ref()
            .map_or(1.0, |w| w.scale_factor() as f32)
    }
}

impl ApplicationHandler for Host {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            self.redraw();
            return;
        }
        let result = (|| -> Result<()> {
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("Vesta GPUI — native host recipe")
                        .with_visible(!self.smoke)
                        .with_inner_size(LogicalSize::new(WIDTHS[0], 360.0))
                        .with_min_inner_size(LogicalSize::new(340.0, 220.0)),
                )?,
            );
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
        if event_loop.exiting() {
            return;
        }
        if self.window.as_ref().is_none_or(|window| window.id() != id) {
            return;
        }
        match &event {
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
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = logical_point(*position, f64::from(self.layout_view_scale()));
            }
            WindowEvent::Focused(active) => {
                self.active = *active;
                if !active {
                    self.end_text_session();
                    self.redraw();
                    return;
                }
                self.redraw();
            }
            WindowEvent::Occluded(occluded) => {
                self.occluded = *occluded;
                if *occluded {
                    self.end_text_session();
                } else {
                    self.redraw();
                }
            }
            _ => (),
        }
        if let Err(error) = self.text_event(&event) {
            self.fail(event_loop, error);
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if event_loop.exiting() {
            return;
        }
        if self.updates.pending()
            && let Some(window) = &self.window
        {
            let size = window.inner_size();
            match self.updates.poll(
                &mut self.renderer,
                TargetConfiguration::ui([size.width, size.height]),
            ) {
                Ok(_) => {
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
        if let Some(creation) = &mut self.creation {
            match creation.poll() {
                RecoveryPoll::Ready(mut renderers) if renderers.len() == 1 => {
                    self.renderer = renderers.pop().map(Box::new);
                    self.creation = None;
                    self.redraw();
                }
                RecoveryPoll::Pending { .. } => (),
                RecoveryPoll::Failed(error) => {
                    self.fail(event_loop, error);
                    return;
                }
                _ => {
                    self.fail(event_loop, "recipe.creation_result_invalid");
                    return;
                }
            }
        }
        let now = Instant::now();
        if self.retry.is_some_and(|at| now >= at) {
            self.retry = None;
            self.redraw();
        }
        // Hidden Windows windows do not deliver ordinary RedrawRequested events.
        // The explicit finite smoke uses the same production draw entry; normal
        // visible use remains driven by native redraw/input, with no idle timer.
        if self.smoke
            && self.renderer.is_some()
            && self.retry.is_none()
            && !self.updates.pending()
            && let Err(error) = self.draw(event_loop)
        {
            self.fail(event_loop, error);
            return;
        }
        if event_loop.exiting() {
            return;
        }
        let pending = self.creation.is_some() || self.updates.pending();
        if pending && self.retry.is_none() {
            self.retry = Some(now + Duration::from_millis(16));
        }
        let mut deadline = self.retry;
        if self.smoke {
            let end = self.started + Duration::from_secs(20);
            if now >= end {
                self.fail(event_loop, "recipe.smoke_timeout");
                return;
            }
            deadline = Some(deadline.map_or(end, |at| at.min(end)));
        }
        event_loop.set_control_flow(deadline.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.end_text_session();
    }
    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.end_text_session();
        self.creation = None;
        self.updates = RendererUpdates::default();
        self.renderer = None;
    }
}

fn run() -> Result<()> {
    let mut context = TextInputContext::default();
    for path in [
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\msyh.ttc",
    ] {
        if !context.register_font(Arc::new(std::fs::read(path)?)) {
            return Err("recipe.font_registration_failed".into());
        }
    }
    let mut input = TextInputState::new_multiline(16.0, "Segoe UI, Microsoft YaHei", 700.0);
    input.reset(&mut context, TEXT)?;
    let mut ownership = InputOwnership::default();
    ownership.set_focus(Some(TARGET));
    let mut host = Host {
        window: None,
        creation: None,
        renderer: None,
        updates: Default::default(),
        monitor: None,
        context,
        input,
        layout: Default::default(),
        rasterizer: Default::default(),
        bridge: Default::default(),
        clipboard: SystemClipboard(arboard::Clipboard::new()?),
        ownership,
        view: None,
        pointer: Point::new(-1.0, -1.0),
        scroll: Point::default(),
        reveal_caret: true,
        active: false,
        occluded: false,
        retry: None,
        error: None,
        smoke: std::env::args().any(|arg| arg == "--smoke"),
        step: 0,
        started: Instant::now(),
    };
    let result = EventLoop::new()?.run_app(&mut host);
    // No UI thread waits for GPU retirement. First release every owned slot;
    // this observation happens only after the native loop has returned.
    host.creation = None;
    host.updates = RendererUpdates::default();
    host.renderer = None;
    host.window = None;
    let retired = host
        .monitor
        .as_ref()
        .map_or(Ok(()), |monitor| monitor.observe(Duration::from_secs(10)));
    result?;
    retired?;
    println!("recipe_gpu_retired=true");
    if let Some(error) = host.error {
        return Err(error);
    }
    if host.smoke && host.step != WIDTHS.len() {
        return Err("recipe.smoke_incomplete".into());
    }
    println!("recipe_closed gpu_retired=true smoke_steps={}", host.step);
    Ok(())
}
fn main() -> Result<()> {
    UiTraceRecording::from_arguments(std::env::args_os())?.finish_after_run(run())
}
