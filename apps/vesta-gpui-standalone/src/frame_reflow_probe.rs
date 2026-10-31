//! Explicit hidden native split/reflow workload, never a physical input claim.
use super::*;
use std::path::PathBuf;

const PREFIX: &str = "--probe-frame-reflow=";
const STEPS: usize = 64;
const DRAFT: &str = "Preserved 草稿 😀 — mixed text wraps with each pane width. ";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Case {
    Inputs,
    Documents,
    Focus,
}

impl Case {
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
            _ => Err("frame_reflow_invalid_case"),
        }
    }

    fn panel_type(self) -> &'static str {
        if self == Self::Documents {
            "gallery.documents"
        } else {
            "gallery.inputs"
        }
    }
}

pub(super) fn requested() -> bool {
    std::env::args().any(|arg| arg.starts_with(PREFIX))
}

pub(super) fn recording() -> Result<Option<vesta_gpui_winit::UiTraceRecording>, Box<dyn Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let paths = args
        .iter()
        .filter_map(|arg| arg.strip_prefix(PREFIX))
        .collect::<Vec<_>>();
    if paths.is_empty() {
        if args
            .iter()
            .any(|arg| arg == "--probe-frame-reflow" || arg.starts_with("--probe-frame-case"))
        {
            return Err("frame_reflow_probe_requires_new_html_path".into());
        }
        return Ok(None);
    }
    Case::from_arguments(&args)?;
    if paths.len() != 1
        || paths[0].is_empty()
        || PathBuf::from(paths[0])
            .extension()
            .is_none_or(|ext| ext != "html")
        || args.iter().any(|arg| {
            arg.starts_with("--trace-ui")
                || arg.starts_with("--smoke")
                || arg.starts_with("--native-capture-fixture")
                || arg.starts_with("--ui-")
                || arg == "--mode=3d"
        })
    {
        return Err("frame_reflow_probe_requires_isolated_default_host".into());
    }
    Ok(Some(vesta_gpui_winit::UiTraceRecording::start(
        paths[0],
        vesta_gpui::trace::TraceConfig {
            max_events: 200_000,
            max_duration: Duration::from_secs(120),
        },
        vesta_gpui::trace::TraceDetail::Stages,
    )?))
}

pub(super) struct Probe {
    case: Case,
    floating: FloatingWindowId,
    step: usize,
    warm: usize,
    started: Instant,
    pub(super) completed: bool,
}

impl App {
    pub(super) fn prepare_frame_reflow_probe(&mut self) -> Result<(), Box<dyn Error>> {
        if !requested() {
            return Ok(());
        }
        self.demo.busy = false;
        let case = Case::from_arguments(&std::env::args().collect::<Vec<_>>())?;
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
        let right = *self
            .demo
            .workspace
            .root()
            .panes()
            .last()
            .ok_or("frame_reflow_main_pane_missing")?;
        let blank =
            self.demo
                .workspace
                .add_item(right, "Blank", PanelTypeId::new("gallery.blank"))?;
        self.demo.workspace.activate(blank)?;
        let floating = self.demo.workspace.spawn_floating_window(
            "Reflow".into(),
            WindowPlacement::new(100.0, 100.0, 1120.0, 800.0),
            if case == Case::Documents {
                "Documents"
            } else {
                "Inputs"
            }
            .into(),
            PanelTypeId::new(case.panel_type()),
        );
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
        self.area_state
            .reset(&mut self.text_context, &DRAFT.repeat(16))?;
        self.frame_reflow_probe = Some(Probe {
            case,
            floating,
            step: 0,
            warm: 8,
            started: Instant::now(),
            completed: false,
        });
        Ok(())
    }

    pub(super) fn drive_frame_reflow_probe(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let Some(mut probe) = self.frame_reflow_probe.take() else {
            return false;
        };
        let result = probe.advance(self);
        match result {
            Ok(true) => {
                probe.completed = true;
                event_loop.exit();
            }
            Ok(false) => event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(2),
            )),
            Err(error) => {
                self.fatal_error = Some(error);
                event_loop.exit();
            }
        }
        self.frame_reflow_probe = Some(probe);
        true
    }
}

impl Probe {
    fn advance(&mut self, app: &mut App) -> Result<bool, String> {
        if self.completed {
            return Ok(true);
        }
        if self.started.elapsed() > Duration::from_secs(100) {
            return Err("frame_reflow_probe_timeout".into());
        }
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
        if self.warm > 0 {
            app.redraw();
            app.redraw_native_window(id);
            self.warm -= 1;
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
            if self.case == Case::Focus
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
            if self.case == Case::Focus {
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
        let ratio = 0.15 + position as f32 * 0.006;
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
            let _step = vesta_gpui::trace::span("fixture", "reflow.step");
            vesta_gpui::trace::counter("fixture", "reflow.source", source as f64);
            vesta_gpui::trace::counter("fixture", "reflow.index", step as f64);
            vesta_gpui::trace::counter("fixture", "reflow.ratio", f64::from(ratio));
            vesta_gpui::trace::counter("fixture", "reflow.case", self.case as u8 as f64);
            if source == 0 {
                app.redraw();
            } else {
                app.redraw_native_window(id);
            }
        }
        self.step += 1;
        self.warm = 4;
        Ok(false)
    }
}
