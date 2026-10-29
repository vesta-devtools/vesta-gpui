//! Real Markdown capture followed by normal native focus loss and late release.
use super::*;
use vesta_gpui_widgets::MarkdownSelection;
mod restore;

#[derive(Clone, Debug, PartialEq, Eq)]
struct State {
    target: InputTarget,
    selection: Option<MarkdownSelection>,
    noncollapsed: bool,
    binding: String,
    dragging: bool,
    scroll_pending: bool,
    capture: Option<(InputTarget, PointerButton)>,
}

impl State {
    fn read(app: &mut App, window: UiWindowId) -> Option<Self> {
        app.with_document_input(window, |interaction, surface, input| Self {
            target: interaction.target(),
            selection: interaction.selection(),
            noncollapsed: interaction.selection().is_some_and(|selection| {
                surface
                    .document
                    .copy_selection(window, selection, 64 * 1024)
                    .is_ok_and(|text| !text.is_empty())
            }),
            binding: format!(
                "{:?}/{:?}",
                surface.document.document_id(),
                surface.document.revision()
            ),
            dragging: interaction.is_dragging(),
            scroll_pending: interaction.drag_scroll_deadline().is_some(),
            capture: input
                .capture()
                .map(|capture| (capture.target, capture.button)),
        })
    }
}

pub(super) fn trace(app: &mut App, window: UiWindowId, event: Event, sequence: u64) {
    let phase = match event {
        Event::Press => "press",
        Event::Release => "release",
        Event::Blur => "blur",
        _ => return,
    };
    if let Some(state) = State::read(app, window) {
        eprintln!(
            "native_document_pointer window={} sequence={sequence} phase={phase} state={state:?}",
            window.0,
        );
    }
}

struct Pending {
    window: UiWindowId,
    before: State,
    workspace: vesta_workspace::WorkspaceModel,
    focus_requested: bool,
    blurred: bool,
}

pub(super) struct Probe {
    source: Source,
    before: Option<State>,
    pending: Option<Pending>,
    restore: Option<restore::Probe>,
}

impl Probe {
    pub(super) fn retired_window(&self, native: WindowId) -> Option<UiWindowId> {
        self.restore.as_ref()?.retired_window(native)
    }

    pub(super) fn from_case(case: &str, source: Source) -> Option<Self> {
        matches!(
            case,
            "main-blur-document" | "float-blur-document" | "float-restore-document"
        )
        .then_some(Self {
            source,
            before: None,
            pending: None,
            restore: (case == "float-restore-document").then(restore::Probe::default),
        })
    }

    pub(super) fn observe(
        &mut self,
        app: &mut App,
        event_loop: &ActiveEventLoop,
        window: UiWindowId,
        event: Event,
        sequence: u64,
    ) -> Result<bool, String> {
        if let Some(restore) = &mut self.restore {
            return restore.observe(app, event_loop, window, event, sequence);
        }
        let is_source = (self.source == Source::Main) == (window == STANDALONE_WINDOW);
        if self.pending.is_none() && is_source {
            let Some(state) = State::read(app, window) else {
                return Ok(false);
            };
            if !state.dragging {
                self.before = Some(state);
                return Ok(false);
            }
            if !matches!(event, Event::Press) {
                return Err("native_document_order: capture did not begin at native press".into());
            }
            let before = self
                .before
                .take()
                .ok_or("native_document_baseline: no observed pre-gesture source")?;
            if before.dragging
                || before.scroll_pending
                || before.capture.is_some()
                || state.target != before.target
                || state.binding != before.binding
                || state.capture != Some((state.target, PointerButton::Primary))
            {
                return Err(
                    "native_document_capture: source/binding or Primary owner mismatch".into(),
                );
            }
            eprintln!(
                "native_document_loss press window={} sequence={} target={} capture=true binding={}",
                window.0,
                sequence,
                state.target.element.get(),
                state.binding
            );
            self.pending = Some(Pending {
                window,
                before,
                workspace: app.demo.workspace.clone(),
                focus_requested: false,
                blurred: false,
            });
            return Ok(false);
        }
        let Some(pending) = &mut self.pending else {
            return Ok(false);
        };
        match event {
            Event::Press => {
                return Err("native_document_interrupted: another press preceded release".into());
            }
            Event::Move if window == pending.window && !pending.focus_requested => {
                let state = State::read(app, window)
                    .ok_or("native_document_source: live source projection missing")?;
                if !state.dragging || state.capture != Some((state.target, PointerButton::Primary))
                {
                    return Err("native_document_capture: motion lost its actual owner".into());
                }
                if !state.noncollapsed {
                    return Ok(false);
                }
                let destination = if window == STANDALONE_WINDOW {
                    app.native_windows
                        .values()
                        .find(|host| host.surface.available())
                        .map(|host| host.window.clone())
                } else {
                    app.window.clone()
                }
                .ok_or("native_document_destination: live window missing")?;
                pending.focus_requested = true;
                eprintln!(
                    "native_document_loss motion window={} sequence={} selection_noncollapsed=true capture=true focus_requested=true fixture_cancel=false fixture_rebuild=false",
                    window.0, sequence
                );
                destination.focus_window();
            }
            Event::Blur if window == pending.window && pending.focus_requested => {
                Self::verify(app, pending)?;
                pending.blurred = true;
                eprintln!(
                    "native_document_loss blur window={} sequence={} selection_rollback=true gesture=false capture=false scroll_deadline=false workspace_preserved=true",
                    window.0, sequence
                );
            }
            Event::Release => {
                if !pending.blurred {
                    return Err("native_document_order: release preceded native source blur".into());
                }
                Self::verify(app, pending)?;
                if app
                    .fixture_input(window)
                    .is_some_and(|input| input.capture().is_some())
                {
                    return Err("native_document_release: release window retained capture".into());
                }
                eprintln!(
                    "native_document_loss passed source={} release_window={} foreign_release={} sequence={} selection_rollback=true binding_preserved=true workspace_preserved=true gesture=false capture=false scroll_deadline=false late_activation=false fixture_cancel=false fixture_rebuild=false",
                    pending.window.0,
                    window.0,
                    pending.window != window,
                    sequence
                );
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }

    fn verify(app: &mut App, pending: &Pending) -> Result<(), String> {
        let after = State::read(app, pending.window)
            .ok_or("native_document_source: source disappeared during blur qualification")?;
        if after != pending.before || app.demo.workspace != pending.workspace {
            return Err(format!(
                "native_document_cancellation: source did not return to baseline: before={:?} after={after:?}",
                pending.before
            ));
        }
        Ok(())
    }
}
