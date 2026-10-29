//! Observe ordinary native focus/size invalidation of a real captured splitter.
use super::*;
use vesta_gpui_workspace::SplitResizeSession;
use winit::dpi::PhysicalSize;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Change {
    Blur,
    Resize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Captured,
    Requested,
    Observed,
    Returning,
    Returned,
}

struct Pending {
    source: UiWindowId,
    session: SplitResizeSession,
    before: WorkspaceModel,
    values: String,
    size: PhysicalSize<u32>,
    destination: Option<UiWindowId>,
    destination_focused: bool,
    phase: Phase,
}

pub(super) struct Probe {
    source: Source,
    change: Change,
    pending: Option<Pending>,
}

impl Probe {
    pub(super) fn from_case(case: &str, source: Source) -> Option<Self> {
        let change = match case {
            "main-blur-splitter" | "float-blur-splitter" => Change::Blur,
            "main-resize-splitter" => Change::Resize,
            _ => return None,
        };
        Some(Self {
            source,
            change,
            pending: None,
        })
    }

    fn native(app: &App, window: UiWindowId) -> Option<&Window> {
        if window == STANDALONE_WINDOW {
            app.window.as_deref()
        } else {
            app.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .map(|host| host.window.as_ref())
        }
    }

    fn values(app: &App) -> String {
        format!(
            "{}/{:?}/{}",
            app.demo.number, app.demo.vector, app.demo.slider
        )
    }

    pub(super) fn observe(
        &mut self,
        app: &mut App,
        window: UiWindowId,
        event: Event,
        sequence: u64,
    ) -> Result<bool, String> {
        if matches!(event, Event::Press) {
            if self.pending.is_some() {
                return Err("native_splitter_loss_interrupted: another press".into());
            }
            if (self.source == Source::Main) != (window == STANDALONE_WINDOW) {
                return Ok(false);
            }
            let Some(session) = app
                .workspace_resize
                .filter(|session| session.target().window == window)
            else {
                return Ok(false);
            };
            if app
                .fixture_input(window)
                .and_then(InputOwnership::capture)
                .is_none_or(|capture| {
                    capture.target != session.target() || capture.button != PointerButton::Primary
                })
            {
                return Err("native_splitter_loss_capture: real primary capture missing".into());
            }
            let size = Self::native(app, window)
                .ok_or("native_splitter_loss_source: native window missing")?
                .inner_size();
            self.pending = Some(Pending {
                source: window,
                session,
                before: app.demo.workspace.clone(),
                values: Self::values(app),
                size,
                destination: None,
                destination_focused: false,
                phase: Phase::Captured,
            });
            eprintln!(
                "native_splitter_loss press source={} sequence={} target={} primary_capture=true",
                window.0,
                sequence,
                session.target().element.get()
            );
            return Ok(false);
        }
        let Some(pending) = &mut self.pending else {
            return Ok(false);
        };
        if matches!(event, Event::Move)
            && window == pending.source
            && pending.phase == Phase::Captured
        {
            if app.workspace_resize != Some(pending.session) {
                return Err("native_splitter_loss_identity: captured session replaced".into());
            }
            if app.demo.workspace == pending.before {
                return Ok(false);
            }
            pending.phase = Phase::Requested;
            if self.change == Change::Resize {
                let native = Self::native(app, window)
                    .ok_or("native_splitter_loss_resize: source missing")?;
                let width = pending
                    .size
                    .width
                    .checked_sub(80)
                    .filter(|width| *width > 0)
                    .ok_or("native_splitter_loss_resize: source too narrow")?;
                let requested = PhysicalSize::new(width, pending.size.height);
                let _ = native.request_inner_size(requested);
                eprintln!(
                    "native_splitter_loss resize_requested source={} sequence={} before={:?} requested={requested:?} preview_changed=true fixture_cancel=false",
                    window.0, sequence, pending.size
                );
            } else {
                let destination = if window == STANDALONE_WINDOW {
                    app.native_windows
                        .values()
                        .find(|host| host.surface.available())
                        .map(|host| host.ui_window)
                } else {
                    app.window.as_ref().map(|_| STANDALONE_WINDOW)
                }
                .ok_or("native_splitter_loss_focus: destination missing")?;
                pending.destination = Some(destination);
                Self::native(app, destination)
                    .ok_or("native_splitter_loss_focus: destination native window missing")?
                    .focus_window();
                eprintln!(
                    "native_splitter_loss focus_requested source={} destination={} sequence={} preview_changed=true fixture_cancel=false",
                    window.0, destination.0, sequence
                );
            }
        }
        if matches!(event, Event::Blur)
            && window == pending.source
            && pending.phase == Phase::Requested
            && self.change == Change::Blur
        {
            Self::verify(app, pending)?;
            pending.phase = Phase::Observed;
            eprintln!(
                "native_splitter_loss blur source={} sequence={} original_model=true values_preserved=true capture=false gesture=false",
                window.0, sequence
            );
        }
        if matches!(event, Event::Focus) && pending.destination == Some(window) {
            pending.destination_focused = true;
        }
        if self.change == Change::Blur
            && pending.phase == Phase::Observed
            && pending.destination_focused
        {
            pending.phase = Phase::Returning;
            Self::native(app, pending.source)
                .ok_or("native_splitter_loss_return: source missing")?
                .focus_window();
            eprintln!(
                "native_splitter_loss return_requested source={} sequence={} destination_focus_observed=true",
                pending.source.0, sequence
            );
        }
        if matches!(event, Event::Focus)
            && window == pending.source
            && pending.phase == Phase::Returning
        {
            Self::verify(app, pending)?;
            pending.phase = Phase::Returned;
            eprintln!(
                "native_splitter_loss source_refocused source={} sequence={}",
                window.0, sequence
            );
        }
        if let Event::Resized(size) = event
            && window == pending.source
            && pending.phase == Phase::Requested
            && self.change == Change::Resize
        {
            if size == pending.size
                || Self::native(app, window).map(Window::inner_size) != Some(size)
            {
                return Err("native_splitter_loss_resize: actual size did not change".into());
            }
            Self::verify(app, pending)?;
            pending.phase = Phase::Observed;
            eprintln!(
                "native_splitter_loss resized source={} sequence={} actual={size:?} original_model=true capture=false gesture=false",
                window.0, sequence
            );
        }
        if matches!(event, Event::Release) {
            let required = if self.change == Change::Blur {
                Phase::Returned
            } else {
                Phase::Observed
            };
            if window != pending.source || pending.phase != required {
                return Err(
                    "native_splitter_loss_release: same-source release before native loss/return"
                        .into(),
                );
            }
            Self::verify(app, pending)?;
            eprintln!(
                "native_splitter_loss passed source={} release_window={} sequence={} original_model=true values_preserved=true capture=false gesture=false late_commit=false fixture_cancel=false",
                pending.source.0, window.0, sequence
            );
            return Ok(true);
        }
        Ok(false)
    }

    fn verify(app: &App, pending: &Pending) -> Result<(), String> {
        if app.demo.workspace != pending.before
            || Self::values(app) != pending.values
            || app.workspace_resize.is_some()
            || app.workspace_drag.is_some()
            || app.floating_move.is_some()
            || app
                .fixture_input(pending.source)
                .is_none_or(|input| input.capture().is_some())
        {
            return Err(
                "native_splitter_loss_rollback: original model/values or idle capture changed"
                    .into(),
            );
        }
        Ok(())
    }
}
