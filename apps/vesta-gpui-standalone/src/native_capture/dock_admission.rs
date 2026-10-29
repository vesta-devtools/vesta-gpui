//! A real drag must not dock behind a newer target-window modal scope.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TargetChange {
    Dialog,
    Palette,
    Minimize,
}

struct Pending {
    source: UiWindowId,
    origin: ContainerRef,
    item: ItemId,
    target: UiWindowId,
    destination: ContainerRef,
    after: Option<vesta_workspace::WorkspaceModel>,
    values: String,
}

pub(super) struct Probe {
    change: TargetChange,
    pending: Option<Pending>,
}

impl Probe {
    pub(super) fn from_case(case: &str) -> Option<Self> {
        let change = match case {
            "main-modal-dock-target" => TargetChange::Dialog,
            "main-palette-dock-target" => TargetChange::Palette,
            "main-minimize-dock-target" => TargetChange::Minimize,
            _ => return None,
        };
        Some(Self {
            change,
            pending: None,
        })
    }

    pub(super) fn observe(
        &mut self,
        app: &mut App,
        window: UiWindowId,
        event: Event,
        sequence: u64,
    ) -> Result<bool, String> {
        let palette = self.change == TargetChange::Palette;
        match event {
            Event::Press => {
                if self.pending.is_some() {
                    return Err("native_dock_scope_interrupted: another press".into());
                }
                if window != STANDALONE_WINDOW {
                    return Ok(false);
                }
                let Some(drag) = app.workspace_drag.filter(|drag| !drag.started) else {
                    return Ok(false);
                };
                let target = app
                    .native_windows
                    .values()
                    .find(|host| host.surface.available())
                    .ok_or("native_dock_scope_target: live Float missing")?;
                let (target, destination) =
                    (target.ui_window, ContainerRef::Floating(target.logical_id));
                if app.focus_root_for_window(target).is_some() {
                    return Err("native_dock_scope_setup: target already blocked".into());
                }
                eprintln!(
                    "native_dock_scope press source={} target={} sequence={} item={} palette={} started=false",
                    window.0, target.0, sequence, drag.item.0, palette
                );
                self.pending = Some(Pending {
                    source: window,
                    origin: drag.origin_container,
                    item: drag.item,
                    target,
                    destination,
                    after: None,
                    values: Self::values(app),
                });
            }
            Event::Move => {
                let Some(pending) = self
                    .pending
                    .as_mut()
                    .filter(|p| p.source == window && p.after.is_none())
                else {
                    return Ok(false);
                };
                let Some(drag) = app.workspace_drag.filter(|drag| {
                    drag.started
                        && drag.item == pending.item
                        && drag.pointer_container == Some(pending.destination)
                        && drag.target.is_some()
                }) else {
                    return Ok(false);
                };
                if drag.origin_container != pending.origin {
                    return Err("native_dock_scope_identity: source changed".into());
                }
                let before = app.demo.workspace.clone();
                app.apply_window_action(
                    pending.target,
                    match self.change {
                        TargetChange::Dialog => DemoAction::OpenDemoDialog,
                        TargetChange::Palette => DemoAction::OpenCommandPalette,
                        TargetChange::Minimize => DemoAction::WindowMinimize,
                    },
                );
                if self.change == TargetChange::Minimize {
                    let (native, host) = app
                        .native_windows
                        .iter()
                        .find(|(_, host)| host.ui_window == pending.target)
                        .ok_or("native_dock_minimize_target_lost")?;
                    let minimized = host.window.is_minimized() == Some(true);
                    let gpu_ready = !app.display_blocked(*native);
                    let admitted = app.dock_container_admitted(pending.destination);
                    let cached_preview = app.workspace_drag.is_some_and(|d| d.target.is_some());
                    eprintln!(
                        "native_dock_minimize observed source={} target={} sequence={} minimized={} gpu_ready={} admitted={} cached_preview={} model_preserved={} fixture_cancel=false fixture_reroute=false",
                        window.0,
                        pending.target.0,
                        sequence,
                        minimized,
                        gpu_ready,
                        admitted,
                        cached_preview,
                        app.demo.workspace == before
                    );
                    if !minimized {
                        return Err("native_dock_minimize_not_observed".into());
                    }
                    if admitted || cached_preview || app.demo.workspace != before {
                        return Err("native_dock_minimize_admission: minimized target retains admission, preview or model change".into());
                    }
                    pending.after = Some(before);
                    app.request_all_redraw();
                    return Ok(false);
                }
                if app.demo.workspace != before
                    || app.focus_root_for_window(pending.target).is_none()
                {
                    return Err(
                        "native_dock_scope_open: normal target scope not established".into(),
                    );
                }
                let native = app
                    .native_windows
                    .iter()
                    .find(|(_, host)| host.ui_window == pending.target)
                    .map(|(native, _)| *native)
                    .ok_or("native_dock_scope_target_lost")?;
                let display_available = !app.display_blocked(native);
                if !display_available || app.dock_window_admitted(pending.target, display_available)
                {
                    return Err("native_dock_scope_admission: live blocked target admitted".into());
                }
                if !app.workspace_drag.is_some_and(|current| {
                    current.item == pending.item
                        && current.origin_container == pending.origin
                        && current.target.is_none()
                }) {
                    return Err(
                        "native_dock_scope_preview: foreign source lost or old target survived"
                            .into(),
                    );
                }
                pending.after = Some(before);
                app.request_all_redraw();
                eprintln!(
                    "native_dock_scope opened source={} target={} sequence={} palette={} display_available=true scope_blocked=true cached_preview=false source_live=true fixture_cancel=false fixture_reroute=false",
                    window.0, pending.target.0, sequence, palette
                );
            }
            Event::Release => {
                let Some(pending) = &self.pending else {
                    return Ok(false);
                };
                let after = pending
                    .after
                    .as_ref()
                    .ok_or("native_dock_scope_order: no scoped target reached")?;
                if self.change == TargetChange::Minimize {
                    eprintln!(
                        "native_dock_minimize release_check source={} target={} release_window={} sequence={} workspace_preserved={} values_preserved={} drag={} focus_transfer={} origin_preserved={} source_capture={:?} target_capture={:?} minimized={}",
                        pending.source.0,
                        pending.target.0,
                        window.0,
                        sequence,
                        &app.demo.workspace == after,
                        Self::values(app) == pending.values,
                        app.workspace_drag.is_some(),
                        app.dock_focus.is_some(),
                        app.demo.workspace.container_of_item(pending.item) == Some(pending.origin),
                        app.fixture_input(pending.source)
                            .and_then(|input| input.capture()),
                        app.fixture_input(pending.target)
                            .and_then(|input| input.capture()),
                        app.native_windows
                            .values()
                            .any(|host| host.ui_window == pending.target
                                && host.window.is_minimized() == Some(true)),
                    );
                    if &app.demo.workspace != after {
                        eprintln!(
                            "native_dock_minimize model_before={after:?} model_after={:?}",
                            app.demo.workspace
                        );
                    }
                }
                if &app.demo.workspace != after
                    || Self::values(app) != pending.values
                    || app.workspace_drag.is_some()
                    || app.dock_focus.is_some()
                    || app.demo.workspace.container_of_item(pending.item) != Some(pending.origin)
                    || match self.change {
                        TargetChange::Minimize => !app.native_windows.values().any(|host| {
                            host.ui_window == pending.target
                                && host.window.is_minimized() == Some(true)
                        }),
                        _ => app.focus_root_for_window(pending.target).is_none(),
                    }
                    || app
                        .fixture_input(pending.source)
                        .is_some_and(|input| input.capture().is_some())
                    || app
                        .fixture_input(pending.target)
                        .is_some_and(|input| input.capture().is_some())
                {
                    return Err("native_dock_scope_release: blocked drop changed model, values or ownership".into());
                }
                if self.change == TargetChange::Minimize {
                    eprintln!(
                        "native_dock_minimize passed source={} target={} release_window={} sequence={} workspace_preserved=true values_preserved=true origin_preserved=true drag=false capture=false focus_transfer=false minimized=true fixture_cancel=false fixture_reroute=false",
                        pending.source.0, pending.target.0, window.0, sequence
                    );
                    return Ok(true);
                }
                eprintln!(
                    "native_dock_scope passed source={} target={} release_window={} sequence={} palette={} foreign_release={} workspace_preserved=true values_preserved=true origin_preserved=true drag=false capture=false focus_transfer=false target_scope_retained=true fixture_cancel=false fixture_reroute=false",
                    pending.source.0,
                    pending.target.0,
                    window.0,
                    sequence,
                    palette,
                    pending.source != window
                );
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }

    fn values(app: &App) -> String {
        format!("{:?}/{:?}", app.demo.slider, app.demo.number)
    }
}
