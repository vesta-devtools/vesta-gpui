//! Real native Dock focus/display loss; the observer never cancels the gesture.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Loss {
    Blur,
    Minimize,
    Hide,
}

struct Pending {
    window: UiWindowId,
    item: ItemId,
    origin: ContainerRef,
    workspace: WorkspaceModel,
    values: String,
    destination: Option<UiWindowId>,
    requested: bool,
    blurred: bool,
    minimized: bool,
    hidden: bool,
    destination_focused: bool,
    return_requested: bool,
    source_refocused: bool,
}

pub(super) struct Probe {
    source: Source,
    loss: Loss,
    return_to_source: bool,
    pending: Option<Pending>,
}

impl Probe {
    pub(super) fn from_case(case: &str, source: Source) -> Option<Self> {
        let loss = match case {
            "main-blur-dock" | "float-blur-dock" => Loss::Blur,
            "main-minimize-dock" | "float-minimize-dock" | "main-minimize-return-dock" => {
                Loss::Minimize
            }
            "main-hide-dock" | "float-hide-dock" | "main-hide-return-dock" => Loss::Hide,
            _ => return None,
        };
        Some(Self {
            source,
            loss,
            return_to_source: case.ends_with("-return-dock"),
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
                return Err("native_dock_lifecycle_interrupted: another press".into());
            }
            if (self.source == Source::Main) != (window == STANDALONE_WINDOW) {
                return Ok(false);
            }
            let Some(drag) = app.workspace_drag.filter(|drag| {
                !drag.started && app.window_container(window) == Some(drag.origin_container)
            }) else {
                return Ok(false);
            };
            self.pending = Some(Pending {
                window,
                item: drag.item,
                origin: drag.origin_container,
                workspace: app.demo.workspace.clone(),
                values: Self::values(app),
                destination: None,
                requested: false,
                blurred: false,
                minimized: false,
                hidden: false,
                destination_focused: false,
                return_requested: false,
                source_refocused: false,
            });
            eprintln!(
                "native_dock_lifecycle press source={} sequence={} item={} loss={:?} started=false",
                window.0, sequence, drag.item.0, self.loss
            );
            return Ok(false);
        }
        let Some(pending) = &mut self.pending else {
            return Ok(false);
        };
        if matches!(event, Event::Move) && window == pending.window && !pending.requested {
            let drag = app
                .workspace_drag
                .ok_or("native_dock_lifecycle_order: gesture vanished before threshold")?;
            if drag.item != pending.item || drag.origin_container != pending.origin {
                return Err("native_dock_lifecycle_identity: source gesture changed".into());
            }
            if !drag.started {
                return Ok(false);
            }
            let (destination, native) = if pending.window == STANDALONE_WINDOW {
                app.native_windows
                    .values()
                    .find(|host| host.surface.available())
                    .map(|host| (host.ui_window, host.window.clone()))
            } else {
                app.window.clone().map(|native| (STANDALONE_WINDOW, native))
            }
            .ok_or("native_dock_lifecycle_destination: no owned live window")?;
            pending.requested = true;
            pending.destination = Some(destination);
            eprintln!(
                "native_dock_lifecycle motion source={} destination={} sequence={} item={} loss={:?} started=true fixture_cancel=false fixture_rebuild=false",
                pending.window.0, destination.0, sequence, pending.item.0, self.loss
            );
            if self.loss == Loss::Minimize {
                app.apply_window_action(pending.window, DemoAction::WindowMinimize);
            } else if self.loss == Loss::Hide {
                Self::native(app, pending.window)
                    .ok_or("native_dock_lifecycle_hide: source window missing")?
                    .set_visible(false);
            }
            Self::observe_display_loss(app, pending, self.loss, sequence);
            native.focus_window();
        }
        Self::observe_display_loss(app, pending, self.loss, sequence);
        if matches!(event, Event::Focus) && pending.destination == Some(window) {
            pending.destination_focused = true;
            eprintln!(
                "native_dock_lifecycle destination_focus window={} sequence={}",
                window.0, sequence
            );
        }
        if matches!(event, Event::Blur) && window == pending.window && pending.requested {
            Self::verify(app, pending)?;
            pending.blurred = true;
            eprintln!(
                "native_dock_lifecycle blur source={} sequence={} workspace_preserved=true values_preserved=true gesture=false capture=false",
                window.0, sequence
            );
        }
        if self.return_to_source
            && !pending.return_requested
            && pending.blurred
            && pending.destination_focused
            && ((self.loss == Loss::Minimize && pending.minimized)
                || (self.loss == Loss::Hide && pending.hidden))
        {
            Self::verify(app, pending)?;
            let native = Self::native(app, pending.window)
                .ok_or("native_dock_lifecycle_return: source window missing")?;
            pending.return_requested = true;
            if self.loss == Loss::Minimize {
                native.set_minimized(false);
            } else {
                native.set_visible(true);
            }
            native.focus_window();
            eprintln!(
                "native_dock_lifecycle return_requested source={} sequence={} fixture_cancel=false fixture_rebuild=false",
                pending.window.0, sequence
            );
        }
        if matches!(event, Event::Focus) && window == pending.window && pending.return_requested {
            let native = Self::native(app, pending.window)
                .ok_or("native_dock_lifecycle_return: source window missing")?;
            if native.is_visible() != Some(true) || native.is_minimized() != Some(false) {
                return Err("native_dock_lifecycle_return: source not actually restored".into());
            }
            Self::verify(app, pending)?;
            pending.source_refocused = true;
            eprintln!(
                "native_dock_lifecycle source_refocused source={} sequence={} actual_visible=true actual_minimized=false",
                window.0, sequence
            );
        }
        if matches!(event, Event::Release) {
            if !pending.blurred
                || (self.loss == Loss::Minimize && !pending.minimized)
                || (self.loss == Loss::Hide && !pending.hidden)
            {
                return Err(format!(
                    "native_dock_lifecycle_order: release before actual loss: blurred={} minimized={} hidden={} loss={:?}",
                    pending.blurred, pending.minimized, pending.hidden, self.loss
                ));
            }
            if window != pending.window && pending.destination != Some(window) {
                return Err("native_dock_lifecycle_release: unexpected owned window".into());
            }
            if self.return_to_source && (window != pending.window || !pending.source_refocused) {
                return Err(
                    "native_dock_lifecycle_return: same-source release before actual return".into(),
                );
            }
            Self::verify(app, pending)?;
            if app
                .fixture_input(window)
                .is_none_or(|input| input.capture().is_some())
            {
                return Err("native_dock_lifecycle_release: missing or captured live owner".into());
            }
            eprintln!(
                "native_dock_lifecycle passed source={} release_window={} foreign_release={} sequence={} item={} loss={:?} blurred=true minimized={} hidden={} destination_focus_observed={} workspace_preserved=true values_preserved=true gesture=false capture=false late_commit=false fixture_cancel=false fixture_rebuild=false",
                pending.window.0,
                window.0,
                pending.window != window,
                sequence,
                pending.item.0,
                self.loss,
                pending.minimized,
                pending.hidden,
                pending.destination_focused
            );
            return Ok(true);
        }
        Ok(false)
    }

    fn observe_display_loss(app: &App, pending: &mut Pending, loss: Loss, sequence: u64) {
        if !pending.requested {
            return;
        }
        let Some(native) = Self::native(app, pending.window) else {
            return;
        };
        if loss == Loss::Minimize && !pending.minimized && native.is_minimized() == Some(true) {
            pending.minimized = true;
            eprintln!(
                "native_dock_lifecycle minimized source={} sequence={} actual_minimized=true",
                pending.window.0, sequence
            );
        }
        if loss == Loss::Hide && !pending.hidden && native.is_visible() == Some(false) {
            pending.hidden = true;
            eprintln!(
                "native_dock_lifecycle hidden source={} sequence={} actual_visible=false",
                pending.window.0, sequence
            );
        }
    }

    fn verify(app: &App, pending: &Pending) -> Result<(), String> {
        if app.demo.workspace != pending.workspace {
            return Err("native_dock_lifecycle_workspace: original Core model changed".into());
        }
        if Self::values(app) != pending.values {
            return Err("native_dock_lifecycle_values: ordinary values changed".into());
        }
        if app.workspace_drag.is_some()
            || app.workspace_resize.is_some()
            || app.floating_move.is_some()
            || app.dock_drag_visual(ContainerRef::Main).is_some()
            || app.dock_drag_visual(pending.origin).is_some()
            || app
                .fixture_input(pending.window)
                .is_none_or(|input| input.capture().is_some())
        {
            return Err(
                "native_dock_lifecycle_cancellation: old gesture/preview/capture survived".into(),
            );
        }
        Ok(())
    }
}
