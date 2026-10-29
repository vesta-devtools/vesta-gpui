//! Opt-in physical-input boundary fixture. Ordinary event dispatch stays authoritative.
use super::*;

mod area;
mod arguments;
mod availability;
mod capture_loss;
mod configured_theme;
mod dock;
mod dock_admission;
mod dock_geometry;
mod dock_visual;
mod document_loss;
mod keyboard;
mod number_loss;
mod numeric_cancel;
mod scrollbar_cancel;
mod scrollbar_loss;
mod selected_feedback;
mod splitter_loss;

pub(super) fn selected_feedback_enabled(app: &App) -> bool {
    app.native_capture
        .as_ref()
        .is_some_and(|fixture| fixture.selected_feedback)
}

pub(super) fn dock_visual_sample(app: &App, container: ContainerRef) -> Option<DockDragVisual> {
    app.native_capture
        .as_ref()
        .filter(|fixture| fixture.dock_visual_sample && app.workspace_drag.is_none())?;
    dock_visual::sample(&app.demo.workspace, container)
}

pub(super) fn project_selected_feedback(
    input: &UiBuildState<'_>,
    pages: &mut HashMap<&str, Vec<Element>>,
    actions: &mut Vec<(ElementId, DemoAction)>,
) {
    if input.selected_feedback {
        selected_feedback::install(input, pages, actions);
    }
}

#[cfg(test)]
pub(super) fn qualify_selected_feedback_preparation() {
    selected_feedback::qualify();
}

#[cfg(test)]
pub(super) fn qualify_numeric_loss() {
    number_loss::qualify();
    numeric_cancel::qualify();
}

#[cfg(test)]
pub(super) fn qualify_scrollbar_loss(app: &mut App) {
    scrollbar_loss::qualify(app);
}

#[cfg(test)]
pub(super) fn qualify_floating_area_preparation() {
    area::qualify();
    configured_theme::qualify();
}

#[cfg(test)]
pub(super) fn qualify_floating_nowrap_preparation() {
    let mut app = App::new();
    let item = app
        .demo
        .workspace
        .panes()
        .into_iter()
        .flat_map(|pane| pane.items.iter())
        .find(|item| item.panel_type.as_str() == "gallery.layout")
        .expect("initial Layout item")
        .id;
    let original = app.demo.workspace.clone();
    assert!(app.prepare_floating_nowrap(item).is_err());
    assert_eq!(app.demo.workspace, original, "Main item refusal is atomic");
    assert!(app.float_workspace_item(item, WindowPlacement::new(80.0, 40.0, 560.0, 640.0)));
    let main = app.demo.workspace.root().clone();
    let (window, pane) = app
        .demo
        .workspace
        .floating_windows()
        .iter()
        .flat_map(|window| {
            window
                .root
                .panes()
                .into_iter()
                .map(move |pane| (window.id, pane))
        })
        .find(|(_, pane)| {
            app.demo
                .workspace
                .pane(*pane)
                .is_some_and(|pane| pane.items.iter().any(|candidate| candidate.id == item))
        })
        .expect("floated item identity");
    let blank = app
        .prepare_floating_nowrap(item)
        .expect("ordinary floating preparation");
    assert_eq!(app.demo.workspace.root(), &main);
    let panes = app
        .demo
        .workspace
        .floating_window(window)
        .expect("same floating window")
        .root
        .panes();
    assert_eq!(panes.len(), 2);
    assert!(panes.contains(&pane));
    assert_eq!(
        app.demo
            .workspace
            .pane(pane)
            .expect("original Layout pane")
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        [item]
    );
    let sibling = panes
        .into_iter()
        .find(|id| *id != pane)
        .expect("new sibling");
    assert_eq!(
        app.demo.workspace.pane(sibling).expect("blank pane").active,
        Some(blank)
    );
    assert_eq!(
        app.demo
            .workspace
            .item(blank)
            .expect("new stable item")
            .panel_type
            .as_str(),
        "gallery.blank"
    );
    let prepared = app.demo.workspace.clone();
    assert!(app.prepare_floating_nowrap(item).is_err());
    assert_eq!(
        app.demo.workspace, prepared,
        "duplicate preparation cannot append another item"
    );
    // Exercise the real consumer preparation, including the floating-source case.
    for case in ["observe-nowrap", "float-blur-splitter"] {
        let mut prepared = App::new();
        prepared.native_capture = Fixture::from_case(case);
        prepared
            .prepare_native_capture()
            .expect("two-pane native consumer preparation");
        let floating = prepared
            .demo
            .workspace
            .floating_windows()
            .first()
            .expect("prepared native floating window");
        assert_eq!(floating.root.panes().len(), 2);
        assert!(floating.root.panes().into_iter().any(|pane| {
            prepared.demo.workspace.pane(pane).is_some_and(|pane| {
                pane.items
                    .iter()
                    .any(|item| item.panel_type.as_str() == "gallery.layout")
            })
        }));
    }
}

const PREFIX: &str = "--native-capture-fixture=";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Main,
    Float,
}

#[derive(Clone, Copy)]
pub(super) enum Event {
    Press,
    Release,
    Blur,
    Move,
    Focus,
    Resized(winit::dpi::PhysicalSize<u32>),
    WindowMoved(winit::dpi::PhysicalPosition<i32>),
    ThemeKey {
        state: ElementState,
        synthetic: bool,
        repeat: bool,
    },
    Ime {
        phase: &'static str,
        bytes: usize,
        cursor: Option<(usize, usize)>,
    },
    Other,
}

impl From<&WindowEvent> for Event {
    fn from(event: &WindowEvent) -> Self {
        match event {
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => match state {
                ElementState::Pressed => Self::Press,
                ElementState::Released => Self::Release,
            },
            WindowEvent::Focused(false) => Self::Blur,
            WindowEvent::Focused(true) => Self::Focus,
            WindowEvent::CursorMoved { .. } => Self::Move,
            WindowEvent::Resized(size) => Self::Resized(*size),
            WindowEvent::Moved(position) => Self::WindowMoved(*position),
            WindowEvent::Ime(ime) => {
                let (phase, bytes, cursor) = match ime {
                    winit::event::Ime::Enabled => ("enabled", 0, None),
                    winit::event::Ime::Preedit(text, cursor) => ("preedit", text.len(), *cursor),
                    winit::event::Ime::Commit(text) => ("commit", text.len(), None),
                    winit::event::Ime::Disabled => ("disabled", 0, None),
                };
                Self::Ime {
                    phase,
                    bytes,
                    cursor,
                }
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } if event.physical_key
                == winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyT) =>
            {
                Self::ThemeKey {
                    state: event.state,
                    synthetic: *is_synthetic,
                    repeat: event.repeat,
                }
            }
            _ => Self::Other,
        }
    }
}

struct Pending {
    window: UiWindowId,
    initial: f32,
    blurred: bool,
}

pub(super) struct Fixture {
    source: Source,
    deadline: Instant,
    pending: Option<Pending>,
    completed: Option<Instant>,
    sequence: u64,
    dock: Option<dock::Probe>,
    dock_admission: Option<dock_admission::Probe>,
    dock_geometry: Option<dock_geometry::Probe>,
    document_loss: Option<document_loss::Probe>,
    keyboard: Option<keyboard::Probe>,
    availability: Option<availability::Probe>,
    capture_loss: Option<capture_loss::Probe>,
    number_loss: Option<number_loss::Probe>,
    numeric_cancel: Option<numeric_cancel::Probe>,
    scrollbar_loss: Option<scrollbar_loss::Probe>,
    scrollbar_cancel: Option<scrollbar_cancel::Probe>,
    splitter_loss: Option<splitter_loss::Probe>,
    nowrap_split: bool,
    area_pair: bool,
    positioned: bool,
    narrow: bool,
    observe_only: bool,
    configured_theme: bool,
    selected_feedback: bool,
    dock_visual_sample: bool,
    ime_request_observation: Option<String>,
}

pub(super) fn validate_args() -> Result<(), String> {
    arguments::validate_args()
}

impl Fixture {
    pub(super) fn from_args() -> Option<Self> {
        let case = std::env::args().find_map(|arg| arg.strip_prefix(PREFIX).map(str::to_owned))?;
        Self::from_case(&case)
    }

    fn from_case(case: &str) -> Option<Self> {
        let source = arguments::source(case)?;
        Some(Self {
            source,
            deadline: Instant::now() + Duration::from_secs(300),
            pending: None,
            completed: None,
            sequence: 0,
            dock: dock::Probe::from_case(case, source),
            dock_admission: dock_admission::Probe::from_case(case),
            dock_geometry: dock_geometry::Probe::from_case(case),
            document_loss: document_loss::Probe::from_case(case, source),
            keyboard: keyboard::Probe::from_case(case, source),
            availability: availability::Probe::from_case(case, source),
            capture_loss: capture_loss::Probe::from_case(case, source),
            number_loss: number_loss::Probe::from_case(case, source),
            numeric_cancel: numeric_cancel::Probe::from_case(case, source),
            scrollbar_loss: scrollbar_loss::Probe::from_case(case, source),
            scrollbar_cancel: scrollbar_cancel::Probe::from_case(case, source),
            splitter_loss: splitter_loss::Probe::from_case(case, source),
            nowrap_split: matches!(case, "observe-nowrap" | "float-blur-splitter"),
            area_pair: case == "observe-area",
            positioned: false,
            narrow: matches!(case, "main-narrow" | "observe-narrow"),
            observe_only: matches!(
                case,
                "observe"
                    | "observe-configured"
                    | "observe-float"
                    | "observe-narrow"
                    | "observe-nowrap"
                    | "observe-area"
                    | selected_feedback::CASE
                    | dock_visual::CASE
            ),
            configured_theme: case == "observe-configured",
            selected_feedback: case == selected_feedback::CASE,
            dock_visual_sample: case == dock_visual::CASE,
            ime_request_observation: None,
        })
    }

    fn trace_ime_request(&mut self, app: &App, window: UiWindowId) {
        if !self.observe_only {
            return;
        }
        let Some(session) = app.text_session else {
            if self.ime_request_observation.take().is_some() {
                eprintln!(
                    "native_ime_request observed window={} session_window=None",
                    window.0
                );
            }
            return;
        };
        if session.window != window {
            return;
        }
        let bridge = if window == STANDALONE_WINDOW {
            Some(&app.text_bridge)
        } else {
            app.native_text_bridges.get(window)
        };
        // Bridge Debug contains request/cursor/modifier/click metadata, not editor text.
        let observed = format!(
            "window={} field={:?} bridge={bridge:?} composing={}",
            window.0,
            session.field,
            app.window_text_composing(window)
        );
        if self.ime_request_observation.as_ref() != Some(&observed) {
            eprintln!("native_ime_request observed {observed}");
            self.ime_request_observation = Some(observed);
        }
    }

    fn observe(
        &mut self,
        app: &mut App,
        event_loop: &ActiveEventLoop,
        window: UiWindowId,
        event: Event,
    ) -> Result<(), String> {
        self.trace_ime_request(app, window);
        if self.observe_only
            && matches!(
                event,
                Event::Press | Event::Release | Event::Move | Event::Blur
            )
        {
            self.sequence += 1;
            numeric_cancel::trace(app, window, event, self.sequence);
            document_loss::trace(app, window, event, self.sequence);
        }
        if let Event::Ime {
            phase,
            bytes,
            cursor,
        } = event
        {
            self.sequence += 1;
            eprintln!(
                "native_ime_fixture observed window={} sequence={} phase={} bytes={} cursor={cursor:?} session_window={:?} field={:?} focus={:?} composing={}",
                window.0,
                self.sequence,
                phase,
                bytes,
                app.text_session.map(|session| session.window.0),
                app.text_session_field(window),
                app.fixture_input(window)
                    .and_then(InputOwnership::focus)
                    .map(|target| target.element.get()),
                app.window_text_composing(window),
            );
            return Ok(());
        }
        if self.observe_only || self.completed.is_some() {
            return Ok(());
        }
        if let Some(dock_geometry) = &mut self.dock_geometry {
            self.sequence += 1;
            if dock_geometry.observe(app, window, event, self.sequence)? {
                self.completed = Some(Instant::now() + Duration::from_secs(30));
            }
            return Ok(());
        }
        if let Some(splitter_loss) = &mut self.splitter_loss {
            self.sequence += 1;
            if splitter_loss.observe(app, window, event, self.sequence)? {
                self.completed = Some(Instant::now() + Duration::from_secs(30));
            }
            return Ok(());
        }
        if let Some(dock_admission) = &mut self.dock_admission {
            self.sequence += 1;
            if dock_admission.observe(app, window, event, self.sequence)? {
                self.completed = Some(Instant::now() + Duration::from_secs(30));
            }
            return Ok(());
        }
        if let Some(document_loss) = &mut self.document_loss {
            self.sequence += 1;
            if document_loss.observe(app, event_loop, window, event, self.sequence)? {
                self.completed = Some(Instant::now() + Duration::from_secs(30));
            }
            return Ok(());
        }
        if let Some(availability) = &mut self.availability {
            self.sequence += 1;
            if availability.observe(app, window, event, self.sequence)? {
                self.completed = Some(Instant::now() + Duration::from_secs(30));
            }
            return Ok(());
        }
        if let Some(capture_loss) = &mut self.capture_loss {
            self.sequence += 1;
            if capture_loss.observe(app, window, event, self.sequence)? {
                self.completed = Some(Instant::now() + capture_loss.inspection_grace());
            }
            return Ok(());
        }
        if let Some(number_loss) = &mut self.number_loss {
            self.sequence += 1;
            if number_loss.observe(app, window, event, self.sequence)? {
                self.completed = Some(Instant::now() + Duration::from_secs(30));
            }
            return Ok(());
        }
        if let Some(numeric_cancel) = &mut self.numeric_cancel {
            self.sequence += 1;
            if numeric_cancel.observe(app, window, event, self.sequence)? {
                self.completed = Some(Instant::now() + Duration::from_secs(30));
            }
            return Ok(());
        }
        if let Some(scrollbar_cancel) = &mut self.scrollbar_cancel {
            self.sequence += 1;
            if scrollbar_cancel.observe(app, window, event, self.sequence)? {
                self.completed = Some(Instant::now() + Duration::from_secs(30));
            }
            return Ok(());
        }
        if let Some(scrollbar_loss) = &mut self.scrollbar_loss {
            self.sequence += 1;
            if scrollbar_loss.observe(app, window, event, self.sequence)? {
                self.completed = Some(Instant::now() + Duration::from_secs(30));
            }
            return Ok(());
        }
        if matches!(event, Event::Other) {
            return Ok(());
        }
        if let Some(keyboard) = &mut self.keyboard {
            self.sequence += 1;
            match keyboard.observe(app, window, event, self.sequence)? {
                keyboard::Status::Pending => {}
                keyboard::Status::Released => {
                    self.deadline = Instant::now() + Duration::from_secs(1);
                }
                keyboard::Status::Passed => {
                    self.completed = Some(Instant::now() + Duration::from_secs(1));
                }
            }
            return Ok(());
        }
        if let Some(dock) = &mut self.dock {
            self.sequence += 1;
            if dock.observe(app, event_loop, window, event, self.sequence)? {
                self.completed = Some(Instant::now() + Duration::from_secs(1));
            }
            return Ok(());
        }
        if matches!(event, Event::Move) {
            return Ok(());
        }
        self.sequence += 1;
        match event {
            Event::Press => {
                if self.pending.is_some() {
                    return Err(
                        "native_capture_interrupted: another press preceded the late release"
                            .into(),
                    );
                }
                if (self.source == Source::Main) != (window == STANDALONE_WINDOW) {
                    return Ok(());
                }
                let Some((initial, capture)) = app.fixture_slider_capture(window) else {
                    return Ok(());
                };
                if initial.to_bits() == app.demo.slider.to_bits() {
                    eprintln!(
                        "native_capture_fixture unchanged_press window={} sequence={}",
                        window.0, self.sequence
                    );
                    return Ok(());
                }
                let destination = if window == STANDALONE_WINDOW {
                    app.native_windows
                        .values()
                        .find(|host| host.surface.available())
                        .map(|host| host.window.clone())
                } else {
                    app.window.clone()
                }
                .ok_or("native_capture_fixture: destination not ready")?;
                self.pending = Some(Pending {
                    window,
                    initial,
                    blurred: false,
                });
                eprintln!(
                    "native_capture_fixture press window={} sequence={} target={} initial={} preview={} capture=true focus_requested=true",
                    window.0,
                    self.sequence,
                    capture.target.element.get(),
                    initial,
                    app.demo.slider
                );
                destination.focus_window();
            }
            Event::Blur => {
                if let Some(pending) = self
                    .pending
                    .as_mut()
                    .filter(|pending| pending.window == window)
                {
                    app.verify_fixture_slider_canceled(pending)?;
                    pending.blurred = true;
                    eprintln!(
                        "native_capture_fixture blur window={} sequence={} rollback=true capture=false",
                        window.0, self.sequence
                    );
                }
            }
            Event::Release => {
                if let Some(pending) = &self.pending {
                    if !pending.blurred {
                        return Err(
                            "native_capture_order: release arrived before source blur".into()
                        );
                    }
                    app.verify_fixture_slider_canceled(pending)?;
                    if app
                        .fixture_input(window)
                        .is_some_and(|input| input.capture().is_some())
                    {
                        return Err("native_capture_release: destination retained capture".into());
                    }
                    eprintln!(
                        "native_capture_fixture passed source={} release_window={} sequence={} rollback=true capture=false late_commit=false",
                        pending.window.0, window.0, self.sequence
                    );
                    self.completed = Some(Instant::now() + Duration::from_secs(1));
                }
            }
            Event::Other
            | Event::Resized(_)
            | Event::WindowMoved(_)
            | Event::Move
            | Event::Focus
            | Event::ThemeKey { .. }
            | Event::Ime { .. } => {}
        }
        Ok(())
    }
}

impl App {
    pub(super) fn observe_native_control_key(
        &self,
        id: WindowId,
        input: &WindowEvent,
    ) -> Option<(UiWindowId, NamedKey, ElementState)> {
        if !self
            .native_capture
            .as_ref()
            .is_some_and(|fixture| fixture.observe_only)
        {
            return None;
        }
        let WindowEvent::KeyboardInput {
            event,
            is_synthetic,
            ..
        } = input
        else {
            return None;
        };
        let Key::Named(key) = &event.logical_key else {
            return None;
        };
        if !matches!(
            key,
            NamedKey::Process
                | NamedKey::Tab
                | NamedKey::Enter
                | NamedKey::Escape
                | NamedKey::Home
                | NamedKey::End
                | NamedKey::ArrowLeft
                | NamedKey::ArrowRight
                | NamedKey::ArrowUp
                | NamedKey::ArrowDown
                | NamedKey::Backspace
                | NamedKey::Delete
        ) {
            return None;
        }
        let window = if self.window.as_ref().is_some_and(|window| window.id() == id) {
            Some(STANDALONE_WINDOW)
        } else {
            self.native_windows.get(&id).map(|host| host.ui_window)
        };
        if let Some(window) = window {
            eprintln!(
                "native_control_key received window={} key={key:?} state={:?} synthetic={} repeat={} focus_before={:?} field_before={:?} composing_before={} caret_visible_before={:?} capture_before={:?} slider_edit_initial_before={:?} slider_value_before={}",
                window.0,
                event.state,
                is_synthetic,
                event.repeat,
                self.window_focus(window).map(ElementId::get),
                self.text_session_field(window),
                self.window_text_composing(window),
                self.observed_caret_visibility(window),
                self.fixture_input(window).and_then(InputOwnership::capture),
                self.observed_slider_edit_initial(window),
                self.demo.slider,
            );
            return Some((window, *key, event.state));
        }
        None
    }

    pub(super) fn observe_native_control_return(
        &self,
        (window, key, state): (UiWindowId, NamedKey, ElementState),
    ) {
        eprintln!(
            "native_control_key completed window={} key={key:?} state={state:?} focus_after={:?} field_after={:?} composing_after={} caret_visible_after={:?} capture_after={:?} slider_edit_initial_after={:?} slider_value_after={}",
            window.0,
            self.window_focus(window).map(ElementId::get),
            self.text_session_field(window),
            self.window_text_composing(window),
            self.observed_caret_visibility(window),
            self.fixture_input(window).and_then(InputOwnership::capture),
            self.observed_slider_edit_initial(window),
            self.demo.slider,
        );
    }

    fn observed_slider_edit_initial(&self, window: UiWindowId) -> Option<f32> {
        let edit = if window == STANDALONE_WINDOW {
            self.slider_drag.as_ref()
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .and_then(|host| host.slider_drag.as_ref())
        };
        edit.map(|edit| *edit.initial())
    }

    fn observed_caret_visibility(&self, window: UiWindowId) -> Option<bool> {
        if window == STANDALONE_WINDOW {
            Some(self.caret_blink.visible())
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .map(|host| host.caret_blink.visible())
        }
    }

    pub(super) fn position_native_capture_windows(&mut self) {
        let Some(fixture) = &mut self.native_capture else {
            return;
        };
        if fixture.positioned || self.rendered_frames == 0 {
            return;
        }
        let Some(main) = &self.window else { return };
        let Ok(origin) = main.inner_position() else {
            return;
        };
        if self.native_windows.is_empty()
            || self.native_windows.values().any(|host| {
                // This tree is initially empty and published only after a
                // successful native render; smoke_frames stays zero here.
                !host.surface.available()
                    || !host
                        .actions
                        .focus_order()
                        .iter()
                        .any(|element| host.semantics.node_for_element(*element).is_some())
            })
        {
            return;
        }
        let position = winit::dpi::PhysicalPosition::new(
            origin
                .x
                .saturating_add((280.0 * main.scale_factor()).round() as i32),
            origin
                .y
                .saturating_add((80.0 * main.scale_factor()).round() as i32),
        );
        for host in self.native_windows.values() {
            host.window.set_outer_position(position);
        }
        fixture.positioned = true;
        eprintln!(
            "native_capture_fixture positioned relative_to_main=true physical_x={} physical_y={}",
            position.x, position.y
        );
    }

    pub(super) fn prepare_native_capture(&mut self) -> Result<(), String> {
        let Some(fixture) = &self.native_capture else {
            return Ok(());
        };
        let source = fixture.source;
        let nowrap_split = fixture.nowrap_split;
        let area_pair = fixture.area_pair;
        let selected_feedback = fixture.selected_feedback;
        let primary_panel = if fixture.document_loss.is_some() {
            "gallery.documents"
        } else if fixture.scrollbar_loss.is_some() || fixture.scrollbar_cancel.is_some() {
            "gallery.collections"
        } else if area_pair || fixture.number_loss.is_some() || fixture.numeric_cancel.is_some() {
            "gallery.inputs"
        } else {
            "gallery.controls"
        };
        let primary_scroll = if fixture.document_loss.is_some()
            || selected_feedback
            || area_pair
            || fixture.number_loss.is_some()
            || fixture.numeric_cancel.is_some()
            || fixture.scrollbar_loss.is_some()
            || fixture.scrollbar_cancel.is_some()
        {
            0.0
        } else {
            1_000.0
        };
        let (width, height) = if fixture.narrow {
            (320.0, 422.0)
        } else {
            (720.0, 640.0)
        };
        let single_tab = fixture.dock.as_ref().is_some_and(dock::Probe::single_tab);
        if fixture.configured_theme {
            configured_theme::prepare(self)?;
        }
        let panel = if source == Source::Main || nowrap_split {
            "gallery.layout"
        } else {
            primary_panel
        };
        let item = self
            .demo
            .workspace
            .panes()
            .into_iter()
            .flat_map(|pane| pane.items.iter())
            .find(|item| item.panel_type.as_str() == panel)
            .map(|item| item.id)
            .ok_or("native_capture_fixture: floating page missing")?;
        if !self.float_workspace_item(item, WindowPlacement::new(320.0, 180.0, width, height)) {
            return Err("native_capture_fixture: could not create floating page".into());
        }
        if selected_feedback {
            self.demo
                .page_scroll
                .insert(item, ScrollAreaState::new(0.0));
        }
        if nowrap_split {
            self.prepare_floating_nowrap(item)?;
        }
        if area_pair {
            area::prepare(self, item)?;
        }
        let primary = self
            .demo
            .workspace
            .panes()
            .into_iter()
            .flat_map(|pane| pane.items.iter())
            .find(|item| item.panel_type.as_str() == primary_panel)
            .map(|item| item.id);
        if let Some(primary) = primary {
            if single_tab {
                let siblings = self
                    .demo
                    .workspace
                    .panes()
                    .into_iter()
                    .find(|pane| pane.items.iter().any(|item| item.id == primary))
                    .ok_or("native_dock_fixture: source pane missing")?
                    .items
                    .iter()
                    .filter(|item| item.id != primary)
                    .map(|item| item.id)
                    .collect::<Vec<_>>();
                for sibling in siblings {
                    self.apply_workspace_action(WorkspaceViewAction::CloseTab(sibling));
                }
            }
            self.demo
                .workspace
                .activate(primary)
                .map_err(|error| error.to_string())?;
            self.demo
                .page_scroll
                .insert(primary, ScrollAreaState::new(primary_scroll));
        } else {
            self.demo
                .page_scroll
                .insert(item, ScrollAreaState::new(primary_scroll));
        }
        self.demo.sync_workspace_ui();
        if let Some(keyboard) = self
            .native_capture
            .as_mut()
            .and_then(|fixture| fixture.keyboard.as_mut())
        {
            keyboard.prepare(self.theme.mode);
        }
        if self
            .native_capture
            .as_ref()
            .is_some_and(|fixture| fixture.observe_only)
        {
            eprintln!(
                "native_input_observer ready timeout_seconds=300 persistence=false acceptance=unasserted"
            );
        } else {
            eprintln!(
                "native_capture_fixture ready source={source:?} timeout_seconds=300 persistence=false"
            );
        }
        Ok(())
    }

    fn fixture_input(&self, window: UiWindowId) -> Option<&InputOwnership> {
        if window == STANDALONE_WINDOW {
            Some(&self.input_ownership)
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .map(|host| &host.input)
        }
    }

    fn fixture_slider_capture(
        &self,
        window: UiWindowId,
    ) -> Option<(f32, vesta_gpui::PointerCapture)> {
        let initial = if window == STANDALONE_WINDOW {
            *self.slider_drag.as_ref()?.initial()
        } else {
            *self
                .native_windows
                .values()
                .find(|host| host.ui_window == window)?
                .slider_drag
                .as_ref()?
                .initial()
        };
        let capture = self.fixture_input(window)?.capture()?;
        (capture.target.window == window && capture.button == PointerButton::Primary)
            .then_some((initial, capture))
    }

    fn fixture_number_scrub(&self, window: UiWindowId) -> Option<&NumberScrub> {
        if window == STANDALONE_WINDOW {
            self.number_scrub.as_ref()
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .and_then(|host| host.number_scrub.as_ref())
        }
    }

    fn fixture_vector_scrub(&self, window: UiWindowId) -> Option<&VectorScrub> {
        if window == STANDALONE_WINDOW {
            self.vector_scrub.as_ref()
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .and_then(|host| host.vector_scrub.as_ref())
        }
    }

    fn verify_fixture_slider_canceled(&self, pending: &Pending) -> Result<(), String> {
        let edit_pending = if pending.window == STANDALONE_WINDOW {
            self.slider_drag.is_some()
        } else {
            self.native_windows
                .values()
                .any(|host| host.ui_window == pending.window && host.slider_drag.is_some())
        };
        if self.demo.slider.to_bits() != pending.initial.to_bits()
            || edit_pending
            || self
                .fixture_input(pending.window)
                .is_none_or(|input| input.capture().is_some())
        {
            return Err("native_capture_rollback: preview or capture survived blur/release".into());
        }
        Ok(())
    }

    pub(super) fn observe_native_capture(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WindowId,
        event: Event,
    ) {
        let Some(mut fixture) = self.native_capture.take() else {
            return;
        };
        let window = if self.window.as_ref().is_some_and(|window| window.id() == id) {
            Some(STANDALONE_WINDOW)
        } else {
            self.native_windows.get(&id).map(|host| host.ui_window)
        };
        let window = window.or_else(|| {
            let retired = fixture.document_loss.as_ref()?.retired_window(id)?;
            if matches!(event, Event::Release) {
                eprintln!(
                    "native_document_retired_event window={} native_id={id:?} kind=release product_dispatch=false",
                    retired.0
                );
            }
            Some(retired)
        });
        if let Some(window) = window
            && let Err(error) = fixture.observe(self, event_loop, window, event)
        {
            self.fatal_error = Some(error);
        }
        self.native_capture = Some(fixture);
    }

    pub(super) fn native_capture_deadline(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Option<Instant> {
        let fixture = self.native_capture.as_ref()?;
        let deadline = fixture.completed.unwrap_or(fixture.deadline);
        if Instant::now() >= deadline {
            if !fixture.observe_only && fixture.completed.is_none() {
                self.fatal_error = Some("native_capture_timeout: required physical press/blur/release sequence incomplete".into());
            }
            event_loop.exit();
        }
        Some(deadline)
    }

    pub(super) fn require_native_capture_completion(&mut self) {
        if self
            .native_capture
            .as_ref()
            .is_some_and(|fixture| fixture.observe_only)
        {
            eprintln!("native_input_observer finished acceptance=unasserted");
            return;
        }
        if self
            .native_capture
            .as_ref()
            .is_some_and(|fixture| fixture.completed.is_none())
            && self.fatal_error.is_none()
        {
            self.fatal_error = Some(
                "native_capture_incomplete: window closed before required physical sequence".into(),
            );
        }
    }
}

impl App {
    fn prepare_floating_nowrap(&mut self, item: ItemId) -> Result<ItemId, String> {
        if self
            .demo
            .workspace
            .item(item)
            .is_none_or(|item| item.panel_type.as_str() != "gallery.layout")
        {
            return Err("native_nowrap_source: expected the Layout fixture item".into());
        }
        let (window, pane) = self
            .demo
            .workspace
            .floating_windows()
            .iter()
            .filter(|window| window.root.panes().len() == 1)
            .flat_map(|window| {
                window
                    .root
                    .panes()
                    .into_iter()
                    .map(move |pane| (window.id, pane))
            })
            .find(|(_, pane)| {
                self.demo
                    .workspace
                    .pane(*pane)
                    .is_some_and(|pane| pane.items.iter().any(|candidate| candidate.id == item))
            })
            .ok_or("native_nowrap_source: missing single-pane floating Layout")?;
        let main_before = self.demo.workspace.root().clone();
        let blank = self
            .demo
            .workspace
            .add_item(pane, "New Tab", PanelTypeId::new("gallery.blank"))
            .map_err(|error| format!("native_nowrap_add: {error}"))?;
        self.demo
            .workspace
            .dock_item(
                blank,
                DockAction::Split {
                    pane,
                    side: Side::Right,
                },
            )
            .map_err(|error| format!("native_nowrap_split: {error}"))?;
        self.demo
            .workspace
            .activate(item)
            .map_err(|error| format!("native_nowrap_activate: {error}"))?;
        if self.demo.workspace.root() != &main_before
            || self
                .demo
                .workspace
                .floating_window(window)
                .is_none_or(|window| {
                    let panes = window.root.panes();
                    panes.len() != 2 || !panes.contains(&pane)
                })
            || self.demo.workspace.pane(pane).is_none_or(|pane| {
                pane.active != Some(item)
                    || pane.items.len() != 1
                    || !pane.items.iter().all(|candidate| candidate.id == item)
            })
        {
            return Err("native_nowrap_preparation: source identity or Main root changed".into());
        }
        self.demo.sync_workspace_ui();
        eprintln!(
            "native_nowrap_fixture prepared window={} layout={} pane={} blank={} typed_workspace_commands=true direct_tree=false main_unchanged=true",
            window.0, item.0, pane.0, blank.0
        );
        Ok(blank)
    }
}
