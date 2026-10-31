//! Explicit hidden native split/reflow workload, never a physical input claim.
use super::*;
use std::path::PathBuf;
mod cancellation;
mod collections;
mod viewport_scene;

const PREFIX: &str = "--probe-frame-reflow=";
const STEPS: usize = 64;
const DRAFT: &str = "Preserved 草稿 😀 — mixed text wraps with each pane width. ";

/// Fixture-only observations from the existing stdio worker. No ticket wait or
/// result callback is introduced on the native event thread.
#[cfg(feature = "gpu-fault-injection")]
pub(super) fn capture_result(
    window: UiWindowId,
    result: Result<(), vesta_gpui_vulkan::RendererFault>,
) {
    if !requested() || !std::env::args().any(|arg| arg == "--probe-frame-case=capture") {
        return;
    }
    let _trace = vesta_gpui::trace::span("fixture", "reflow.capture_result");
    vesta_gpui::trace::counter(
        "fixture",
        "reflow.source",
        if window == STANDALONE_WINDOW {
            0.0
        } else {
            1.0
        },
    );
    let outcome = match result {
        Ok(()) => 1.0,
        Err(vesta_gpui_vulkan::RendererFault::SwapchainOutOfDate) => 0.0,
        Err(_) => -1.0,
    };
    vesta_gpui::trace::counter("fixture", "reflow.capture_outcome", outcome);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Case {
    Inputs,
    Documents,
    Focus,
    Deferred,
    Budget,
    Cancellation,
    Capture,
    Viewport,
    Collections(Option<vesta_gpui::trace::TraceDetail>),
}

impl Case {
    // Existing report identities remain stable when a case carries options.
    fn report_code(self) -> u8 {
        match self {
            Self::Inputs => 0,
            Self::Documents => 1,
            Self::Focus => 2,
            Self::Deferred => 3,
            Self::Budget => 4,
            Self::Cancellation => 5,
            Self::Capture => 6,
            Self::Viewport => 7,
            Self::Collections(_) => 8,
        }
    }
    fn from_arguments(args: &[String]) -> Result<Self, &'static str> {
        let cases = args
            .iter()
            .filter_map(|arg| arg.strip_prefix("--probe-frame-case="))
            .collect::<Vec<_>>();
        if cases.len() > 1 || args.iter().any(|arg| arg == "--probe-frame-case") {
            return Err("frame_reflow_invalid_case");
        }
        match cases.first().copied().unwrap_or("inputs") {
            "inputs" => Ok(Self::Inputs),
            "documents" => Ok(Self::Documents),
            "focus" => Ok(Self::Focus),
            "deferred" if cfg!(feature = "gpu-fault-injection") => Ok(Self::Deferred),
            "budget" => Ok(Self::Budget),
            "cancellation" => Ok(Self::Cancellation),
            "capture" if cfg!(feature = "gpu-fault-injection") => Ok(Self::Capture),
            "viewport" => Ok(Self::Viewport),
            "collections-off" => Ok(Self::Collections(None)),
            "collections-stages" => Ok(Self::Collections(Some(
                vesta_gpui::trace::TraceDetail::Stages,
            ))),
            "collections-detailed" => Ok(Self::Collections(Some(
                vesta_gpui::trace::TraceDetail::Detailed,
            ))),
            _ => Err("frame_reflow_invalid_case"),
        }
    }

    fn panel_type(self) -> &'static str {
        if matches!(self, Self::Collections(_)) {
            "gallery.collections"
        } else if self == Self::Documents {
            "gallery.documents"
        } else {
            "gallery.inputs"
        }
    }

    fn is_budget(self) -> bool {
        matches!(self, Self::Budget | Self::Cancellation)
    }
}

pub(super) fn requested() -> bool {
    static REQUESTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *REQUESTED.get_or_init(|| std::env::args().any(|arg| arg.starts_with(PREFIX)))
}

/// A reduced process-local fixture budget reaches the production bound exit.
/// Ordinary hosts and other probe cases retain the eight-pass policy.
pub(super) fn layout_pass_limit() -> u8 {
    static LIMIT: std::sync::OnceLock<u8> = std::sync::OnceLock::new();
    *LIMIT.get_or_init(|| {
        if requested()
            && std::env::args().any(|arg| {
                matches!(
                    arg.as_str(),
                    "--probe-frame-case=budget" | "--probe-frame-case=cancellation"
                )
            })
        {
            1
        } else {
            FRAME_LAYOUT_PASS_LIMIT
        }
    })
}

/// Compare the private CPU projection with the production painter on actual
/// successful probe frames; ordinary application frames do no comparison work.
pub(super) fn verify_rendered_text_views(
    window: UiWindowId,
    gallery: ElementScope,
    prepared: &TextViews,
    rendered: &[(ElementId, vesta_gpui::TextInputViewport)],
) -> Result<(), String> {
    if !requested() {
        return Ok(());
    }
    let rendered = text_views_for(window, gallery, rendered)
        .map_err(|e| format!("frame_reflow_rendered_views: {e:?}"))?;
    for field in TextFieldKind::ALL {
        if prepared.get(field) != rendered.get(field) {
            return Err(format!(
                "frame_reflow_paint_view_mismatch: {window:?} {field:?}"
            ));
        }
    }
    Ok(())
}

pub(super) fn recording() -> Result<Option<vesta_gpui_winit::UiTraceRecording>, Box<dyn Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let paths = args
        .iter()
        .filter_map(|arg| arg.strip_prefix(PREFIX))
        .collect::<Vec<_>>();
    if paths.is_empty() {
        if args.iter().any(|arg| {
            arg == "--probe-frame-reflow"
                || arg.starts_with("--probe-frame-case")
                || arg.starts_with("--probe-frame-retry-once")
        }) {
            return Err("frame_reflow_probe_requires_new_html_path".into());
        }
        return Ok(None);
    }
    let case = Case::from_arguments(&args)?;
    if matches!(case, Case::Capture | Case::Viewport)
        && !args
            .iter()
            .any(|arg| ui_query::stdio_argument(arg) == Some(true))
    {
        return Err("frame_capture_probe_requires_explicit_control_stdio".into());
    }
    let retry_flags = args
        .iter()
        .filter(|arg| arg.starts_with("--probe-frame-retry-once"))
        .collect::<Vec<_>>();
    if !retry_flags.is_empty()
        && (!cfg!(feature = "gpu-fault-injection")
            || case != Case::Inputs
            || retry_flags.len() != 1
            || retry_flags[0].as_str() != "--probe-frame-retry-once")
    {
        return Err("frame_reflow_invalid_retry_probe".into());
    }
    if paths.len() != 1
        || paths[0].is_empty()
        || PathBuf::from(paths[0])
            .extension()
            .is_none_or(|ext| ext != "html")
        || args.iter().any(|arg| {
            arg.starts_with("--trace-ui")
                || arg.starts_with("--smoke")
                || arg.starts_with("--native-capture-fixture")
                || (arg.starts_with("--ui-") && ui_query::stdio_argument(arg).is_none())
                || arg == "--mode=3d"
        })
    {
        return Err("frame_reflow_probe_requires_isolated_default_host".into());
    }
    let detail = if let Case::Collections(detail) = case {
        if !args.iter().any(|arg| arg == "--profile-ui")
            || !args
                .iter()
                .any(|arg| ui_query::stdio_argument(arg) == Some(true))
        {
            return Err("frame_collection_profile_requires_control_stdio_and_profile_ui".into());
        }
        if PathBuf::from(paths[0]).try_exists()? {
            return Err("frame_collection_profile_destination_exists".into());
        }
        detail
    } else {
        Some(vesta_gpui::trace::TraceDetail::Stages)
    };
    Ok(Some(match detail {
        Some(detail) => vesta_gpui_winit::UiTraceRecording::start(
            paths[0],
            vesta_gpui::trace::TraceConfig {
                max_events: 200_000,
                max_duration: Duration::from_secs(120),
            },
            detail,
        )?,
        None => vesta_gpui_winit::UiTraceRecording::from_arguments(std::iter::empty::<String>())?,
    }))
}

pub(super) struct Probe {
    workload: Workload,
    started: Instant,
    pub(super) completed: bool,
}

enum Workload {
    Reflow(ReflowProbe),
    Viewport(viewport_scene::Probe),
    Collections(collections::Probe),
}

struct ReflowProbe {
    case: Case,
    floating: FloatingWindowId,
    step: usize,
    warm: usize,
    recover_after: Option<vesta_gpui::UiSnapshotId>,
    #[cfg(feature = "gpu-fault-injection")]
    retry_injected: bool,
    #[cfg(feature = "gpu-fault-injection")]
    capture_requests: [u8; 2],
}

impl App {
    pub(super) fn prepare_frame_reflow_probe(&mut self) -> Result<(), Box<dyn Error>> {
        if !requested() {
            return Ok(());
        }
        self.demo.busy = false;
        // Local snapshots; a stdin worker still requires explicit stdio opt-in.
        self.ui_query = ui_query::Store::new(true);
        self.ui_query.control_enabled = true;
        let case = Case::from_arguments(&std::env::args().collect::<Vec<_>>())?;
        if case == Case::Viewport {
            self.mode = AcceptanceMode::Viewport3d;
            self.frame_reflow_probe = Some(Probe {
                workload: Workload::Viewport(viewport_scene::Probe::new()),
                started: Instant::now(),
                completed: false,
            });
            return Ok(());
        }
        let inputs = self
            .demo
            .workspace
            .panes()
            .into_iter()
            .flat_map(|pane| &pane.items)
            .find(|item| item.panel_type.as_str() == case.panel_type())
            .ok_or("frame_reflow_main_inputs_missing")?
            .id;
        self.demo.workspace.activate(inputs)?;
        let mut blank_pane = *self
            .demo
            .workspace
            .root()
            .panes()
            .last()
            .ok_or("frame_reflow_main_pane_missing")?;
        if matches!(case, Case::Collections(_)) {
            blank_pane = self
                .demo
                .workspace
                .root()
                .panes()
                .into_iter()
                .rev()
                .find(|pane| {
                    self.demo
                        .workspace
                        .pane(*pane)
                        .is_some_and(|pane| !pane.items.iter().any(|item| item.id == inputs))
                })
                .ok_or("frame_collection_profile_other_pane_missing")?;
        }
        let blank =
            self.demo
                .workspace
                .add_item(blank_pane, "Blank", PanelTypeId::new("gallery.blank"))?;
        self.demo.workspace.activate(blank)?;
        let floating = self.demo.workspace.spawn_floating_window(
            "Reflow".into(),
            WindowPlacement::new(100.0, 100.0, 1120.0, 800.0),
            if case == Case::Documents {
                "Documents"
            } else if matches!(case, Case::Collections(_)) {
                "Collections"
            } else {
                "Inputs"
            }
            .into(),
            PanelTypeId::new(case.panel_type()),
        )?;
        let pane = self
            .demo
            .workspace
            .floating_window(floating)
            .ok_or("frame_reflow_float_missing")?
            .root
            .panes()[0];
        let input = self
            .demo
            .workspace
            .pane(pane)
            .and_then(Pane::active_item)
            .ok_or("frame_reflow_float_inputs_missing")?
            .id;
        let blank =
            self.demo
                .workspace
                .add_item(pane, "Blank", PanelTypeId::new("gallery.blank"))?;
        self.demo.workspace.dock_item(
            blank,
            DockAction::Split {
                pane,
                side: Side::Right,
            },
        )?;
        self.demo.workspace.activate(input)?;
        self.demo.sync_workspace_ui();
        if let Case::Collections(detail) = case {
            self.demo.virtual_ids = (0..100_000).collect();
            self.frame_reflow_probe = Some(Probe {
                workload: Workload::Collections(collections::Probe::new(
                    floating,
                    [inputs, input],
                    detail,
                )),
                started: Instant::now(),
                completed: false,
            });
            return Ok(());
        }
        self.area_state
            .reset(&mut self.text_context, &DRAFT.repeat(16))?;
        self.frame_reflow_probe = Some(Probe {
            workload: Workload::Reflow(ReflowProbe {
                case,
                floating,
                step: 0,
                warm: 8,
                recover_after: None,
                #[cfg(feature = "gpu-fault-injection")]
                retry_injected: false,
                #[cfg(feature = "gpu-fault-injection")]
                capture_requests: [0; 2],
            }),
            started: Instant::now(),
            completed: false,
        });
        Ok(())
    }

    pub(super) fn drive_frame_reflow_probe(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let Some(mut probe) = self.frame_reflow_probe.take() else {
            return false;
        };
        let result = probe.advance(self, event_loop);
        match result {
            Ok(true) => {
                probe.completed = true;
                event_loop.exit();
            }
            Ok(false) if probe.active_driver() => event_loop.set_control_flow(
                ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(2)),
            ),
            Ok(false) => (),
            Err(error) => {
                self.fatal_error = Some(error);
                event_loop.exit();
            }
        }
        let driving = probe.completed || probe.active_driver();
        self.frame_reflow_probe = Some(probe);
        driving
    }

    pub(super) fn frame_probe_passive_deadline(&self) -> Option<Instant> {
        self.frame_reflow_probe
            .as_ref()
            .and_then(|probe| match &probe.workload {
                Workload::Collections(probe) => probe.passive_deadline(),
                _ => None,
            })
    }
}

impl ReflowProbe {
    #[cfg(feature = "gpu-fault-injection")]
    pub(super) fn capture_requested(&mut self, window: UiWindowId) -> bool {
        if self.case != Case::Capture {
            return false;
        }
        let source = usize::from(window != STANDALONE_WINDOW);
        self.capture_requests[source] = self.capture_requests[source].saturating_add(1);
        self.warm = 8;
        let inject = self.capture_requests[source] == 1;
        eprintln!(
            "Frame capture probe: window={} request={} inject_out_of_date={inject}",
            window.0, self.capture_requests[source]
        );
        inject
    }

    fn advance(&mut self, app: &mut App, event_loop: &ActiveEventLoop) -> Result<bool, String> {
        let Some((id, available)) = app
            .native_windows
            .iter()
            .find(|(_, host)| host.logical_id == self.floating)
            .map(|(id, host)| (*id, host.surface.available()))
        else {
            return Ok(false);
        };
        if !available || app.gpu.as_ref().is_none_or(|gpu| !gpu.available()) {
            return Ok(false);
        }
        if self.warm > 0 || self.recover_after.is_some() {
            if self.case == Case::Cancellation && self.step == 0 && self.warm == 8 {
                let other = app
                    .native_windows
                    .get(&id)
                    .ok_or("frame_cancel_float_lost")?;
                let owner = other.ui_window;
                let field = other.scopes.gallery.element(NUMBER_FIELD);
                app.set_window_focus(
                    STANDALONE_WINDOW,
                    Some(app.scopes.gallery.element(NUMBER_FIELD)),
                );
                app.reveal_window_focus(
                    STANDALONE_WINDOW,
                    app.scopes.gallery.element(NUMBER_FIELD),
                );
                app.set_window_focus(owner, Some(field));
                app.reveal_window_focus(owner, field);
            }
            app.redraw();
            app.redraw_native_window(id);
            self.warm = self.warm.saturating_sub(1);
            if self
                .recover_after
                .is_some_and(|before| app.ui_query.next(before.window) != Some(before))
            {
                if self.case.is_budget() {
                    let native = if self
                        .recover_after
                        .is_some_and(|before| before.window == STANDALONE_WINDOW)
                    {
                        app.window.as_ref().ok_or("frame_budget_main_lost")?.id()
                    } else {
                        id
                    };
                    if !app.native_input_layout_ready(native)
                        || app.unavailable_input(
                            native,
                            &WindowEvent::Text("ignored recovery input".into()),
                        )
                    {
                        return Err("frame_budget_recovery_input_still_blocked".into());
                    }
                }
                self.recover_after = None;
            }
            return Ok(false);
        }
        if app
            .window
            .as_ref()
            .is_none_or(|window| app.display_blocked(window.id()))
            || app.display_blocked(id)
        {
            app.redraw();
            app.redraw_native_window(id);
            return Ok(false);
        }
        if self.step == STEPS * 2 {
            let host = app
                .native_windows
                .get(&id)
                .ok_or("frame_reflow_float_lost")?;
            if app.area_state.text() != DRAFT.repeat(16)
                || (self.case != Case::Documents
                    && (app.area_view.is_none()
                        || host.text_views.get(TextFieldKind::Area).is_none()))
                || app.input_ownership.capture().is_some()
                || host.input.capture().is_some()
            {
                return Err("frame_reflow_changed_text_or_projection".into());
            }
            if self.case == Case::Documents {
                for (documents, owner) in [
                    (&app.documents, STANDALONE_WINDOW),
                    (&host.documents, host.ui_window),
                ] {
                    let selection = documents
                        .markdown
                        .select_all(owner)
                        .map_err(|e| format!("frame_reflow_document_selection: {e:?}"))?
                        .ok_or("frame_reflow_empty_document")?;
                    let copied = documents
                        .markdown
                        .copy_selection(owner, selection, 16 * 1024)
                        .map_err(|e| format!("frame_reflow_document_copy: {e:?}"))?;
                    if !copied.contains("Structured data") || !copied.contains("Seed") {
                        return Err("frame_reflow_document_projection_lost".into());
                    }
                }
            }
            if matches!(self.case, Case::Focus | Case::Budget)
                && (host.input.focus()
                    != Some(InputTarget {
                        window: host.ui_window,
                        element: host.scopes.gallery.element(TEXT_AREA),
                    })
                    || app.input_ownership.focus()
                        != Some(InputTarget {
                            window: STANDALONE_WINDOW,
                            element: app.scopes.gallery.element(TEXT_AREA),
                        })
                    || app.text_session
                        != Some(TextSession {
                            window: host.ui_window,
                            field: TextFieldKind::Area,
                        }))
            {
                return Err("frame_reflow_text_focus_or_session_owner_mismatch".into());
            }
            if matches!(self.case, Case::Focus | Case::Budget) {
                eprintln!(
                    "Frame reflow focus: window_memory_preserved=true single_caret_owner=float physical_focus=false"
                );
            }
            eprintln!(
                "Frame reflow workload complete: main=64 float=64 text_preserved=true physical_input=false"
            );
            return Ok(true);
        }
        let source = self.step / STEPS;
        #[cfg(feature = "gpu-fault-injection")]
        if self.case == Case::Capture && self.capture_requests != [2, 2] {
            app.redraw();
            app.redraw_native_window(id);
            return Ok(false);
        }
        let step = self.step % STEPS;
        let container = if source == 0 {
            ContainerRef::Main
        } else {
            ContainerRef::Floating(self.floating)
        };
        let root = if source == 0 {
            app.demo.workspace.root()
        } else {
            &app.demo
                .workspace
                .floating_window(self.floating)
                .ok_or("frame_reflow_float_lost")?
                .root
        };
        let WorkspaceNode::Split { id: split, .. } = root else {
            return Err("frame_reflow_split_missing".into());
        };
        let position = if step < STEPS / 2 {
            step
        } else {
            STEPS - 1 - step
        };
        let ratio = 0.15
            + position as f32 * 0.006
            + if self.case.is_budget() && step >= STEPS / 2 {
                0.003
            } else {
                0.0
            };
        app.demo
            .workspace
            .set_split_ratio(container, *split, ratio)
            .map_err(|e| format!("frame_reflow_ratio: {e}"))?;
        if self.case == Case::Focus && step == 0 {
            if source == 0 {
                app.input_ownership.request_focus(InputTarget {
                    window: STANDALONE_WINDOW,
                    element: app.scopes.gallery.element(TEXT_AREA),
                });
            } else if let Some(host) = app.native_windows.get_mut(&id) {
                host.input.request_focus(InputTarget {
                    window: host.ui_window,
                    element: host.scopes.gallery.element(TEXT_AREA),
                });
            }
        }
        {
            let step_trace = vesta_gpui::trace::span("fixture", "reflow.attempt");
            let owner = if source == 0 {
                STANDALONE_WINDOW
            } else {
                app.native_windows
                    .get(&id)
                    .ok_or("frame_reflow_float_lost")?
                    .ui_window
            };
            let before_presented = app.ui_query.next(owner);
            let budget_target = if self.case.is_budget() {
                let gallery = if source == 0 {
                    app.scopes.gallery
                } else {
                    app.native_windows
                        .get(&id)
                        .ok_or("frame_reflow_float_lost")?
                        .scopes
                        .gallery
                };
                let element = gallery.element(if self.case == Case::Cancellation {
                    NUMBER_FIELD
                } else {
                    TEXT_AREA
                });
                let instance = before_presented
                    .ok_or("frame_budget_missing_presentation")?
                    .instance;
                let commands = control(
                    app,
                    event_loop,
                    ui_query::Operation::Commands {
                        instance,
                        window: owner,
                    },
                )
                .map_err(|e| format!("frame_budget_commands: {e}"))?;
                let target = command_target(&commands, instance, owner, element)?;
                app.set_window_focus(owner, Some(element));
                let target_input = InputTarget {
                    window: owner,
                    element,
                };
                if source == 0 {
                    app.input_ownership
                        .capture_pointer(target_input, vesta_gpui::PointerButton::Primary);
                } else if let Some(host) = app.native_windows.get_mut(&id) {
                    host.input
                        .capture_pointer(target_input, vesta_gpui::PointerButton::Primary);
                }
                Some(target)
            } else {
                None
            };
            vesta_gpui::trace::counter("fixture", "reflow.source", source as f64);
            vesta_gpui::trace::counter("fixture", "reflow.index", step as f64);
            vesta_gpui::trace::counter("fixture", "reflow.ratio", f64::from(ratio));
            vesta_gpui::trace::counter(
                "fixture",
                "reflow.case",
                f64::from(self.case.report_code()),
            );
            let cancellation = if self.case == Case::Cancellation {
                Some(cancellation::Armed::new(
                    app,
                    event_loop,
                    id,
                    budget_target.ok_or("frame_cancel_target_missing")?,
                )?)
            } else {
                None
            };
            #[cfg(feature = "gpu-fault-injection")]
            if self.case == Case::Deferred
                || (!self.retry_injected
                    && std::env::args().any(|arg| arg == "--probe-frame-retry-once"))
            {
                let gpu = if source == 0 {
                    app.gpu.as_mut()
                } else {
                    app.native_windows
                        .get_mut(&id)
                        .map(|host| &mut host.surface)
                }
                .ok_or("frame_reflow_deferred_surface_missing")?;
                gpu.capture_renderer()
                    .map_err(|e| format!("frame_reflow_deferred_renderer: {e:?}"))?
                    .inject_acquire_faults();
                if self.case != Case::Deferred {
                    self.retry_injected = true;
                    vesta_gpui::trace::instant("fixture", "reflow.retry_once_injected");
                }
            }
            if source == 0 {
                app.redraw();
            } else {
                app.redraw_native_window(id);
            }
            if let Some(target) = budget_target {
                if let Some(cancellation) = cancellation {
                    cancellation.verify(app, event_loop, source, step)?;
                }
                let native_id = if source == 0 {
                    app.window.as_ref().ok_or("frame_budget_main_lost")?.id()
                } else {
                    id
                };
                let blocked = app
                    .unavailable_input(native_id, &WindowEvent::Text("ignored probe input".into()));
                let capture_released = if source == 0 {
                    app.input_ownership.capture().is_none()
                } else {
                    app.native_windows
                        .get(&id)
                        .is_some_and(|host| host.input.capture().is_none())
                };
                let scale = if source == 0 {
                    app.window
                        .as_ref()
                        .ok_or("frame_budget_main_lost")?
                        .scale_factor()
                } else {
                    app.native_windows
                        .get(&id)
                        .ok_or("frame_reflow_float_lost")?
                        .window
                        .scale_factor()
                };
                let motion_blocked = app.unavailable_input(
                    native_id,
                    &WindowEvent::CursorMoved {
                        device_id: winit::event::DeviceId::dummy(),
                        position: winit::dpi::PhysicalPosition::new(32.0 * scale, 64.0 * scale),
                    },
                );
                let cached_cursor = if source == 0 {
                    app.cursor
                } else {
                    app.native_windows
                        .get(&id)
                        .ok_or("frame_reflow_float_lost")?
                        .cursor
                };
                let left_blocked = app.unavailable_input(
                    native_id,
                    &WindowEvent::CursorLeft {
                        device_id: winit::event::DeviceId::dummy(),
                    },
                );
                let left_cursor = if source == 0 {
                    app.cursor
                } else {
                    app.native_windows
                        .get(&id)
                        .ok_or("frame_reflow_float_lost")?
                        .cursor
                };
                let cursor_cached = motion_blocked
                    && cached_cursor == Point::new(32.0, 64.0)
                    && left_blocked
                    && left_cursor == Point::new(-1.0, -1.0);
                let lifecycle_allowed = [
                    WindowEvent::CloseRequested,
                    WindowEvent::RedrawRequested,
                    WindowEvent::Focused(false),
                ]
                .iter()
                .all(|event| !app.unavailable_input(native_id, event));
                let focus_preserved = if source == 0 {
                    app.input_ownership.focus()
                } else {
                    app.native_windows
                        .get(&id)
                        .and_then(|host| host.input.focus())
                } == Some(InputTarget {
                    window: owner,
                    element: target.element,
                });
                let input = control(
                    app,
                    event_loop,
                    ui_query::Operation::Input {
                        request_id: format!("budget-input-{source}-{step}"),
                        request: vesta_gpui_winit::UiInputRequest {
                            target,
                            revision: before_presented
                                .ok_or("frame_budget_missing_presentation")?
                                .revision
                                - 1,
                            modifiers: Default::default(),
                            steps: vec![vesta_gpui_winit::UiInputStep::Text(
                                "must not replace draft".into(),
                            )],
                        },
                    },
                );
                let logical_blocked = matches!(input, Err("presentation_pending"));
                let command = control(
                    app,
                    event_loop,
                    ui_query::Operation::Invoke {
                        request_id: format!("budget-{source}-{step}"),
                        target,
                        command: vesta_gpui::UiCommand::Focus,
                    },
                );
                let command_blocked = matches!(command, Err("presentation_pending"));
                let unchanged = before_presented.is_some_and(|id| id.revision > 1)
                    && app.ui_query.next(owner) == before_presented;
                for (name, accepted) in [
                    ("reflow.input_blocked", blocked),
                    ("reflow.capture_released", capture_released),
                    ("reflow.command_blocked", command_blocked),
                    ("reflow.logical_blocked", logical_blocked),
                    ("reflow.lifecycle_allowed", lifecycle_allowed),
                    ("reflow.focus_preserved", focus_preserved),
                    ("reflow.cursor_cached", cursor_cached),
                    ("reflow.presented_unchanged", unchanged),
                ] {
                    vesta_gpui::trace::counter("fixture", name, if accepted { 1.0 } else { 0.0 });
                }
                if !(blocked
                    && capture_released
                    && command_blocked
                    && logical_blocked
                    && lifecycle_allowed
                    && focus_preserved
                    && cursor_cached
                    && unchanged)
                {
                    return Err(format!(
                        "frame_budget_admission_mismatch: source={source} step={step} native_blocked={blocked} capture_released={capture_released} command={command:?} presented_unchanged={unchanged} cursor_cached={cursor_cached}"
                    ));
                }
                self.recover_after = before_presented;
            }
            if self.case == Case::Deferred {
                let (view, bounds, hits, area) = if source == 0 {
                    (
                        app.area_view,
                        app.focus_rects.get(&app.scopes.gallery.element(TEXT_AREA)),
                        &app.hit_test,
                        app.scopes.gallery.element(TEXT_AREA),
                    )
                } else {
                    let host = app
                        .native_windows
                        .get(&id)
                        .ok_or("frame_reflow_float_lost")?;
                    (
                        host.text_views.get(TextFieldKind::Area),
                        host.focus_rects
                            .get(&host.scopes.gallery.element(TEXT_AREA)),
                        &host.hit_test,
                        host.scopes.gallery.element(TEXT_AREA),
                    )
                };
                let agrees = view.zip(bounds).is_some_and(|(view, bounds)| {
                    view.bounds == *bounds
                        && view.clip.width > 0.0
                        && view.clip.height > 0.0
                        && hits.hit(Point::new(
                            view.clip.x + view.clip.width * 0.5,
                            view.clip.y + view.clip.height * 0.5,
                        )) == Some(area)
                });
                vesta_gpui::trace::counter(
                    "fixture",
                    "reflow.geometry_agrees",
                    if agrees { 1.0 } else { 0.0 },
                );
                if !agrees {
                    return Err(format!(
                        "frame_reflow_deferred_geometry_mismatch: source={source} step={step} view={view:?} bounds={bounds:?}"
                    ));
                }
                let native_id = if source == 0 {
                    app.window.as_ref().ok_or("frame_reflow_main_lost")?.id()
                } else {
                    id
                };
                let ready = app.native_input_layout_ready(native_id)
                    && !app.unavailable_input(
                        native_id,
                        &WindowEvent::Text("ignored deferred input".into()),
                    );
                vesta_gpui::trace::counter(
                    "fixture",
                    "reflow.cpu_input_ready",
                    if ready { 1.0 } else { 0.0 },
                );
                if !ready {
                    return Err("frame_reflow_stable_cpu_input_blocked_by_acquire".into());
                }
                let unchanged = before_presented.is_some_and(|id| id.revision > 1)
                    && app.ui_query.next(owner) == before_presented;
                vesta_gpui::trace::counter(
                    "fixture",
                    "reflow.presented_unchanged",
                    if unchanged { 1.0 } else { 0.0 },
                );
                if !unchanged {
                    return Err("frame_reflow_deferred_published_snapshot".into());
                }
                self.recover_after = before_presented;
            }
            if !self.case.is_budget()
                && self.case != Case::Deferred
                && app.ui_query.next(owner) == before_presented
            {
                vesta_gpui::trace::instant("fixture", "reflow.await_submission");
                self.warm = 4;
                return Ok(false);
            }
            step_trace.finish_with_name("reflow.step");
        }
        self.step += 1;
        self.warm = 4;
        Ok(false)
    }
}

impl Probe {
    #[cfg(feature = "gpu-fault-injection")]
    pub(super) fn capture_requested(&mut self, window: UiWindowId) -> bool {
        match &mut self.workload {
            Workload::Reflow(probe) => probe.capture_requested(window),
            Workload::Viewport(_) => false,
            Workload::Collections(_) => false,
        }
    }

    fn advance(&mut self, app: &mut App, event_loop: &ActiveEventLoop) -> Result<bool, String> {
        if self.completed {
            return Ok(true);
        }
        if self.started.elapsed() > Duration::from_secs(100) {
            return Err("frame_reflow_probe_timeout".into());
        }
        match &mut self.workload {
            Workload::Reflow(probe) => probe.advance(app, event_loop),
            Workload::Viewport(probe) => Ok(probe.advance(app)),
            Workload::Collections(probe) => probe.advance(app, event_loop),
        }
    }

    fn active_driver(&self) -> bool {
        match &self.workload {
            Workload::Collections(probe) => probe.active_driver(),
            _ => true,
        }
    }
}

fn control(
    app: &mut App,
    event_loop: &ActiveEventLoop,
    operation: ui_query::Operation,
) -> Result<serde_json::Value, &'static str> {
    let (reply, receiver) = std::sync::mpsc::sync_channel(1);
    app.answer_ui_query(event_loop, ui_query::Request { operation, reply });
    match receiver
        .try_recv()
        .map_err(|_| "frame_probe_missing_reply")??
    {
        (_, ui_query::Reply::Control(value)) => Ok(value),
        _ => Err("frame_probe_unexpected_reply"),
    }
}

fn command_target(
    commands: &serde_json::Value,
    instance: u128,
    window: UiWindowId,
    element: ElementId,
) -> Result<vesta_gpui::UiTargetRef, &'static str> {
    let owner = commands["commands"]
        .as_array()
        .and_then(|commands| {
            commands
                .iter()
                .find(|command| command["element"].as_str() == Some(&element.get().to_string()))
        })
        .and_then(|command| command["owner"].as_str())
        .and_then(|owner| owner.parse::<u64>().ok())
        .ok_or("frame_reflow_missing_control_owner")?;
    Ok(vesta_gpui::UiTargetRef {
        instance,
        window,
        element,
        owner,
    })
}
