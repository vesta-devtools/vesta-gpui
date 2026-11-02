//! Independent application host. Captured files are read and analyzed on its worker.
mod accessibility;
mod commands;
mod events;
mod input;
mod scenario;
mod scroll;
mod search;
mod work;
use crate::{
    inspection::{Panel, Report, Selection as InspectionSelection},
    query::protocol::{Cursor, GpuKey, TimerKey},
    session::*,
    timeline::Timeline,
    view::{self, Command},
    worker::{self, Product, Request, Task, Worker},
};
use std::{
    error::Error,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use vesta_gpui::{
    ActionRoute, Color, FocusScope, HitTestTree, InputOwnership, InputTarget, PaintList, Point,
    PointerButton, PointerClick, RetainedLayoutEngine, TextInputContext, TextRasterizer,
    UiWindowId, Viewport,
};
use vesta_gpui_vulkan::{
    RecoveryPoll, RenderOptions, RendererCreation, RendererUpdates, RetirementMonitor,
    TargetConfiguration, WindowRenderer,
};
use vesta_gpui_winit::{AccessibilityAdapter, AccessibilityEvent, AccessibilityWindowEvent};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};
use work::{Activity, Checkpoint};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const WINDOW: UiWindowId = UiWindowId(1);
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Scene {
    #[default]
    Basic,
    Inspection,
    Navigation,
    Focus,
    Lifecycle,
}
pub(crate) struct Options {
    pub smoke: bool,
    pub filter: String,
    pub capture_dir: Option<PathBuf>,
    pub scene: Scene,
    pub alternate: Option<PathBuf>,
}
enum UserEvent {
    Worker(Box<worker::Message>),
    Accessibility(AccessibilityEvent),
}
impl From<worker::Message> for UserEvent {
    fn from(value: worker::Message) -> Self {
        Self::Worker(Box::new(value))
    }
}
impl From<AccessibilityEvent> for UserEvent {
    fn from(value: AccessibilityEvent) -> Self {
        Self::Accessibility(value)
    }
}
struct Drag {
    anchor: f32,
    original: crate::session::TimeRange,
    selection: Option<TimeRange>,
    pan: bool,
    moved: bool,
}
struct App {
    window: Option<Arc<Window>>,
    proxy: winit::event_loop::EventLoopProxy<UserEvent>,
    accessibility: Option<AccessibilityAdapter>,
    semantics: vesta_gpui::AccessibilityTree,
    smoke_semantics: Option<accesskit::TreeUpdate>,
    creation: Option<RendererCreation>,
    renderer: Option<Box<WindowRenderer>>,
    updates: RendererUpdates,
    monitor: Option<RetirementMonitor>,
    retry: Option<Instant>,
    context: TextInputContext,
    rasterizer: TextRasterizer,
    layout: RetainedLayoutEngine,
    route: Option<ActionRoute<Command>>,
    hit: Option<HitTestTree>,
    layout_tree: Option<vesta_gpui::LayoutTree>,
    click: PointerClick,
    pointer: Point,
    modifiers: winit::keyboard::ModifiersState,
    drag: Option<Drag>,
    area: vesta_gpui::Rect,
    table_area: vesta_gpui::Rect,
    displayed_analysis_range: Option<TimeRange>,
    table_scroll: usize,
    table_rows: usize,
    table_page_rows: usize,
    track_scroll_max: f32,
    panel: Panel,
    timer: Option<TimerKey>,
    gpu: Option<GpuKey>,
    gpu_scroll: vesta_gpui_widgets::ScrollAreaState,
    page: Option<Cursor>,
    previous_pages: Vec<Option<Cursor>>,
    search: search::Search,
    ownership: InputOwnership,
    active: bool,
    session: Option<Arc<Session>>,
    report: Option<Report>,
    timeline: Option<Timeline>,
    worker: Worker,
    pending: Option<Request>,
    cancel: Arc<AtomicBool>,
    generation: u64,
    status: String,
    filter: String,
    activity: Activity,
    checkpoint: Option<Checkpoint>,
    error: Option<Box<dyn Error>>,
    occluded: bool,
    smoke: bool,
    scene: Scene,
    navigation: scenario::navigation::State,
    focus_probe: scenario::focus::State,
    lifecycle: scenario::lifecycle::State,
    smoke_filter: Option<String>,
    smoke_source: Option<String>,
    smoke_page: bool,
    smoke_counter: Option<EventKey>,
    source_path: Option<PathBuf>,
    started: Instant,
    presented: usize,
    smoke_step: usize,
    capture_dir: Option<PathBuf>,
    capture_waiting: bool,
    captured_step: Option<usize>,
    drawn: usize,
    collapsed: usize,
}
impl App {
    fn redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl Into<Box<dyn Error>>) {
        self.error = Some(error.into());
        event_loop.exit();
    }
    fn queue(&mut self, task: Task) -> std::result::Result<(), TraceError> {
        let activity = match &task {
            Task::Load(path) => Activity::Import(path.clone()),
            Task::Analyze { .. } => Activity::Analysis,
            Task::Capture { .. } => Activity::Capture,
        };
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.generation = self.generation.checked_add(1).ok_or_else(|| {
            TraceError::new("trace_generation_exhausted", "job identities exhausted")
        })?;
        self.pending = self.worker.submit(Request {
            generation: self.generation,
            cancel: self.cancel.clone(),
            task,
        })?;
        self.activity = activity;
        if self.smoke {
            println!(
                "trace_job_queued generation={} panel={:?}",
                self.generation, self.panel
            );
        }
        self.route = None;
        self.hit = None;
        self.layout_tree = None;
        self.click.cancel();
        self.redraw();
        Ok(())
    }
    fn flush(&mut self) -> std::result::Result<(), TraceError> {
        if let Some(request) = self.pending.take() {
            self.pending = self.worker.submit(request)?;
        }
        Ok(())
    }
    fn reanalyze(&mut self) -> std::result::Result<(), TraceError> {
        if self.importing() {
            return Ok(());
        }
        if let Some((session, timeline)) = self.session.as_ref().zip(self.timeline.as_ref()) {
            self.queue(Task::Analyze {
                session: session.clone(),
                range: timeline.analysis_range(),
                filter: self.filter.clone(),
                tracks: self.report.as_ref().and_then(|r| r.tracks.clone()),
                selection: InspectionSelection {
                    gpu: self.gpu,
                    panel: self.panel,
                    timer: self.timer.clone(),
                    event: timeline.selected,
                    cursor: self.page.clone(),
                },
            })?;
        }
        Ok(())
    }
    fn draw(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let _frame = vesta_gpui::trace::frame("Trace.Main");
        if event_loop.exiting() {
            return Ok(());
        }
        let Some(window) = self.window.clone() else {
            return Ok(());
        };
        let size = window.inner_size();
        self.route = None;
        self.hit = None;
        self.layout_tree = None;
        if size.width == 0 || size.height == 0 || (self.occluded && !self.smoke) {
            return Ok(());
        }
        if !self.updates.prepare(
            &mut self.renderer,
            TargetConfiguration::ui([size.width, size.height]),
        )? {
            return Ok(());
        }
        let scale = window.scale_factor() as f32;
        let viewport = Viewport::new(size.width as f32 / scale, size.height as f32 / scale, scale);
        let status = match &self.activity {
            Activity::Import(path) => {
                format!("{} · opening {}…", self.source_name(), path.display())
            }
            Activity::Analysis => format!("{} · analyzing…", self.source_name()),
            _ if self.search.input.text() != self.filter => {
                format!("{} · search draft: press Enter", self.status)
            }
            _ => self.status.clone(),
        };
        let report = (!self.busy()).then_some(self.report.as_ref()).flatten();
        let projection = view::build(view::View {
            session: self.session.as_deref(),
            analysis: report.map(|r| r.analysis.as_ref()),
            timeline: self.timeline.as_ref(),
            inspection: report.map(|r| &r.inspection),
            tracks: report.and_then(|r| r.tracks.as_deref()),
            width: viewport.width,
            height: viewport.height,
            status: &status,
            filter: &self.filter,
            search: self.search.snapshot(),
            search_focused: self.search_focused() && self.active && !self.occluded,
            panel: self.panel,
            table_scroll: self.table_scroll,
            gpu_scroll: self.gpu_scroll.offset(),
            previous_page: !self.previous_pages.is_empty(),
            busy: self.busy(),
        });
        self.area = projection.timeline;
        if projection.drawn_events + projection.collapsed_events != projection.visible_events {
            return Err("trace_plot_accounting_mismatch".into());
        }
        self.table_area = projection.table;
        self.displayed_analysis_range = projection.analysis_range;
        self.table_rows = projection.table_rows;
        self.table_page_rows = projection.table_page_rows;
        self.track_scroll_max = projection.timeline_scroll_max;
        self.table_scroll = self
            .table_scroll
            .min(self.table_rows.saturating_sub(self.table_page_rows));
        if let Some(timeline) = &mut self.timeline
            && timeline.scroll > projection.timeline_scroll_max
        {
            timeline.scroll = projection.timeline_scroll_max;
            if self.smoke {
                println!(
                    "trace_layout_settlement reason=track_scroll generation={}",
                    self.generation
                );
            }
            self.retry = Some(Instant::now() + Duration::from_millis(1));
            self.redraw();
            return Ok(());
        }
        self.drawn = projection.drawn_events;
        self.collapsed = projection.collapsed_events;
        let layout = self.layout.compute_with_text_context(
            &projection.widget.element,
            viewport,
            &mut self.context,
        )?;
        let route = ActionRoute::build(&projection.widget.element, projection.widget.actions)?;
        if let Some(node) = layout.node(view::GPU_DETAIL)
            && self.gpu_scroll.sync_layout(node)
        {
            self.retry = Some(Instant::now() + Duration::from_millis(1));
            self.redraw();
            return Ok(());
        }
        // A loading projection is not the replacement data view. Retain the
        // logical target until actual results can validate or retire it; busy
        // platform actions still reject and missing nodes cannot activate.
        if !self.busy() {
            let old = self.ownership.focus();
            let reconciled = FocusScope::new(WINDOW, &route, None).reconcile(&mut self.ownership);
            if reconciled.lost_focus {
                if self.smoke {
                    println!(
                        "trace_layout_settlement reason=focus_retired generation={}",
                        self.generation
                    );
                }
                self.retry = Some(Instant::now() + Duration::from_millis(1));
                self.focus_changed(old);
                return Ok(());
            }
        }
        let paint = PaintList::build_with_focus(
            &projection.widget.element,
            &layout,
            self.ownership
                .visible_focus()
                .filter(|_| self.active && !self.occluded)
                .map(|t| t.element),
            Color::rgb8(240, 183, 100),
        );
        self.search.install(&paint);
        self.click
            .retain_live(WINDOW, |id| route.action_at(id).is_some());
        self.hit = Some(HitTestTree::build(&projection.widget.element, &layout));
        self.route = Some(route);
        self.layout_tree = Some(layout);
        let ready = self.smoke_ready(viewport);
        let caret_visible = self.active && !self.occluded && self.search_focused();
        let busy = self.busy();
        let renderer = self.renderer.as_mut().ok_or("trace_renderer_missing")?;
        let capture = if self.smoke
            && ready
            && self.session.is_some()
            && !busy
            && !self.capture_waiting
            && self.captured_step != Some(self.smoke_step)
        {
            if let Some(directory) = &self.capture_dir {
                Some((
                    renderer.capture_next_frame(vesta_gpui_vulkan::CaptureRegion {
                        origin: [0, 0],
                        extent: [size.width, size.height],
                    })?,
                    directory.join(format!("trace-step-{}.png", self.smoke_step)),
                ))
            } else {
                None
            }
        } else {
            None
        };
        let output = renderer.render(
            viewport,
            &paint,
            &mut self.context,
            &mut self.rasterizer,
            None,
            RenderOptions {
                clear: [0.04, 0.05, 0.07, 1.0],
                selection: Color::rgb8(60, 94, 155),
                caret: Color::rgb8(235, 240, 250),
                caret_visible,
            },
            false,
        )?;
        self.retry = renderer.frame_retry().map(|r| r.deadline);
        if output.is_some() {
            self.publish_accessibility(&projection.widget.element);
        }
        if let Some((ticket, path)) = capture {
            self.queue(Task::Capture { ticket, path })?;
            self.capture_waiting = true;
        }
        if let Some(output) = output {
            if output
                .text_inputs
                .iter()
                .find(|(id, _)| *id == view::SEARCH)
                .map(|(_, viewport)| *viewport)
                != self.search.view
            {
                return Err("trace_search_cpu_painter_geometry_mismatch".into());
            }
            let focused = self.active && !self.occluded && self.search_focused();
            self.search
                .bridge
                .sync_ime(&window, focused, self.search.view);
            self.updates.presented();
            self.presented += 1;
            if self.smoke
                && ready
                && self.session.is_some()
                && !self.busy()
                && (self.capture_dir.is_none() || self.captured_step == Some(self.smoke_step))
            {
                self.smoke_presented(event_loop)?;
            }
        }
        Ok(())
    }
}
pub fn run(path: Option<PathBuf>, options: Options) -> Result<()> {
    let recording = vesta_gpui_winit::UiTraceRecording::from_arguments(std::env::args_os())?;
    recording.finish_after_run(run_host(path, options))
}
fn run_host(path: Option<PathBuf>, options: Options) -> Result<()> {
    let Options {
        smoke,
        filter,
        capture_dir,
        scene,
        alternate,
    } = options;
    if smoke && path.is_none() {
        return Err("trace_smoke_requires_file".into());
    }
    if let Some(directory) = &capture_dir {
        std::fs::create_dir(directory)?;
    }
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let mut context = TextInputContext::default();
    for file in [
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\msyh.ttc",
    ] {
        if !context.register_font(Arc::new(std::fs::read(file)?)) {
            return Err("trace_font_registration".into());
        }
    }
    let search = search::Search::new(&mut context, &filter)?;
    let worker = Worker::new(event_loop.create_proxy())?;
    let mut app = App {
        window: None,
        proxy: event_loop.create_proxy(),
        accessibility: None,
        semantics: Default::default(),
        smoke_semantics: None,
        creation: None,
        renderer: None,
        updates: Default::default(),
        monitor: None,
        retry: None,
        context,
        rasterizer: Default::default(),
        layout: Default::default(),
        route: None,
        hit: None,
        layout_tree: None,
        click: Default::default(),
        pointer: Point::default(),
        modifiers: Default::default(),
        drag: None,
        area: Default::default(),
        table_area: Default::default(),
        displayed_analysis_range: None,
        table_scroll: 0,
        table_rows: 0,
        table_page_rows: 1,
        track_scroll_max: 0.0,
        panel: Panel::Timers,
        timer: None,
        gpu: None,
        gpu_scroll: Default::default(),
        page: None,
        previous_pages: Vec::new(),
        search,
        ownership: Default::default(),
        active: false,
        session: None,
        report: None,
        timeline: None,
        worker,
        pending: None,
        cancel: Arc::new(AtomicBool::new(false)),
        generation: 0,
        status: "Drop a GPUI trace file to begin".into(),
        filter,
        activity: Activity::Idle,
        checkpoint: None,
        error: None,
        occluded: false,
        smoke,
        scene,
        navigation: Default::default(),
        focus_probe: scenario::focus::State::new(alternate.clone()),
        lifecycle: scenario::lifecycle::State::new(alternate),
        smoke_filter: None,
        smoke_source: None,
        smoke_page: false,
        smoke_counter: None,
        source_path: None,
        started: Instant::now(),
        presented: 0,
        smoke_step: 0,
        capture_dir,
        capture_waiting: false,
        captured_step: None,
        drawn: 0,
        collapsed: 0,
    };
    if let Some(path) = path {
        app.open_source(path)?;
    }
    let result = event_loop.run_app(&mut app);
    app.cancel.store(true, Ordering::Relaxed);
    app.pending = None;
    app.creation = None;
    app.updates = RendererUpdates::default();
    app.renderer = None;
    app.accessibility = None;
    app.window = None;
    let worker = app.worker.finish();
    let retired = app
        .monitor
        .as_ref()
        .map_or(Ok(()), |m| m.observe(Duration::from_secs(10)));
    result?;
    worker?;
    retired?;
    println!("trace_resources_retired gpu=true worker_joined=true");
    if let Some(error) = app.error {
        return Err(error);
    }
    if smoke && app.smoke_step != app.smoke_steps() {
        return Err("trace_smoke_incomplete".into());
    }
    println!(
        "trace_closed gpu_retired=true worker_joined=true presentations={}",
        app.presented
    );
    Ok(())
}
