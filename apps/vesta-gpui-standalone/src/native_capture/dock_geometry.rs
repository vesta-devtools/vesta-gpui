//! Observe a real captured drag when its native destination moves away.
use super::*;
use winit::dpi::PhysicalPosition;

struct Pending {
    source: UiWindowId,
    origin: ContainerRef,
    item: ItemId,
    target: UiWindowId,
    destination: FloatingWindowId,
    before: vesta_workspace::WorkspaceModel,
    after: Option<vesta_workspace::WorkspaceModel>,
    requested: PhysicalPosition<i32>,
    screen_pointer: Point,
    values: String,
    source_moves: u32,
    source_blurred: bool,
}

pub(super) struct Probe {
    pending: Option<Pending>,
}

impl Probe {
    pub(super) fn from_case(case: &str) -> Option<Self> {
        (case == "main-move-dock-target").then_some(Self { pending: None })
    }

    pub(super) fn observe(
        &mut self,
        app: &mut App,
        window: UiWindowId,
        event: Event,
        sequence: u64,
    ) -> Result<bool, String> {
        match event {
            Event::Move if self.pending.is_none() && window == STANDALONE_WINDOW => {
                let Some(drag) = app.workspace_drag.filter(|drag| {
                    drag.started
                        && drag.origin_container == ContainerRef::Main
                        && drag.target.is_some()
                }) else {
                    return Ok(false);
                };
                let Some(ContainerRef::Floating(destination)) = drag.pointer_container else {
                    return Ok(false);
                };
                let host = app
                    .native_windows
                    .values()
                    .find(|host| host.logical_id == destination)
                    .ok_or("native_dock_geometry_target_missing")?;
                let main = app
                    .window
                    .as_ref()
                    .ok_or("native_dock_geometry_source_missing")?;
                let source_origin = main
                    .inner_position()
                    .map_err(|_| "native_dock_geometry_source_position")?;
                let position = host
                    .window
                    .outer_position()
                    .map_err(|_| "native_dock_geometry_target_position")?;
                let requested = PhysicalPosition::new(
                    position
                        .x
                        .checked_add(800)
                        .ok_or("native_dock_geometry_position_overflow")?,
                    position.y,
                );
                let screen_pointer = Point::new(
                    source_origin.x as f32 + app.cursor.x * main.scale_factor() as f32,
                    source_origin.y as f32 + app.cursor.y * main.scale_factor() as f32,
                );
                self.pending = Some(Pending {
                    source: window,
                    origin: drag.origin_container,
                    item: drag.item,
                    target: host.ui_window,
                    destination,
                    before: app.demo.workspace.clone(),
                    after: None,
                    requested,
                    screen_pointer,
                    values: format!("{:?}/{:?}", app.demo.slider, app.demo.number),
                    source_moves: 0,
                    source_blurred: false,
                });
                eprintln!(
                    "native_dock_geometry requested source={} target={} sequence={} item={} from={position:?} to={requested:?} screen_pointer={screen_pointer:?} fixture_cancel=false fixture_reroute=false pointer_io=false",
                    window.0, host.ui_window.0, sequence, drag.item.0
                );
                // Ordinary winit movement only; the normal dispatcher must invalidate
                // stale targeting. The fixture never cancels or re-routes the gesture.
                host.window.set_outer_position(requested);
            }
            Event::WindowMoved(position) => {
                let Some(pending) = self
                    .pending
                    .as_mut()
                    .filter(|pending| pending.target == window && pending.after.is_none())
                else {
                    return Ok(false);
                };
                if position != pending.requested {
                    return Ok(false);
                }
                let host = app
                    .native_windows
                    .values()
                    .find(|host| host.ui_window == pending.target)
                    .ok_or("native_dock_geometry_moved_target_missing")?;
                let actual = host
                    .window
                    .outer_position()
                    .map_err(|_| "native_dock_geometry_actual_position")?;
                let size = host.window.outer_size();
                let outer = Rect::new(
                    actual.x as f32,
                    actual.y as f32,
                    size.width as f32,
                    size.height as f32,
                );
                if actual != position || outer.contains(pending.screen_pointer) {
                    return Err("native_dock_geometry_target_not_moved_away".into());
                }
                let mut expected = pending.before.clone();
                let original = expected
                    .floating_window(pending.destination)
                    .ok_or("native_dock_geometry_original_window_missing")?
                    .placement;
                let placement = native_window::placement_from_native_event(
                    original,
                    &WindowEvent::Moved(position),
                    host.window.scale_factor(),
                    false,
                )
                .ok_or("native_dock_geometry_ordinary_placement_missing")?;
                expected
                    .floating_window_mut(pending.destination)
                    .ok_or("native_dock_geometry_expected_window_missing")?
                    .placement = placement;
                eprintln!(
                    "native_dock_geometry moved source={} target={} sequence={} actual={actual:?} placement={placement:?} source_moves={} source_blurred={} target_admitted={} cached_preview={} model_matches_expected={} fixture_cancel=false fixture_reroute=false",
                    pending.source.0,
                    pending.target.0,
                    sequence,
                    pending.source_moves,
                    pending.source_blurred,
                    app.dock_container_admitted(ContainerRef::Floating(pending.destination)),
                    app.workspace_drag.is_some_and(|drag| drag.target.is_some()),
                    app.demo.workspace == expected,
                );
                pending.after = Some(expected);
            }
            Event::Move => {
                if let Some(pending) = &mut self.pending
                    && pending.source == window
                {
                    pending.source_moves += 1;
                }
            }
            Event::Blur => {
                if let Some(pending) = &mut self.pending
                    && pending.source == window
                {
                    pending.source_blurred = true;
                }
            }
            Event::Release => {
                let Some(pending) = &self.pending else {
                    return Ok(false);
                };
                let expected = pending
                    .after
                    .as_ref()
                    .ok_or("native_dock_geometry_release_before_actual_move")?;
                if pending.source_moves != 0 || pending.source_blurred || pending.source != window {
                    return Err("native_dock_geometry_intervening_source_input".into());
                }
                let values_preserved =
                    pending.values == format!("{:?}/{:?}", app.demo.slider, app.demo.number);
                let origin_preserved =
                    app.demo.workspace.container_of_item(pending.item) == Some(pending.origin);
                let capture = [pending.source, pending.target].into_iter().any(|window| {
                    app.fixture_input(window)
                        .is_some_and(|input| input.capture().is_some())
                });
                eprintln!(
                    "native_dock_geometry release source={} target={} release_window={} sequence={} workspace_preserved={} values_preserved={} origin_preserved={} drag={} capture={} focus_transfer={} source_moves=0 source_blurred=false fixture_cancel=false fixture_reroute=false",
                    pending.source.0,
                    pending.target.0,
                    window.0,
                    sequence,
                    &app.demo.workspace == expected,
                    values_preserved,
                    origin_preserved,
                    app.workspace_drag.is_some(),
                    capture,
                    app.dock_focus.is_some(),
                );
                if &app.demo.workspace != expected
                    || !values_preserved
                    || !origin_preserved
                    || app.workspace_drag.is_some()
                    || capture
                    || app.dock_focus.is_some()
                {
                    eprintln!(
                        "native_dock_geometry expected={expected:?} actual={:?}",
                        app.demo.workspace
                    );
                    return Err(
                        "native_dock_geometry_stale_release: moved-away target accepted old drop"
                            .into(),
                    );
                }
                eprintln!("native_dock_geometry passed sequence={sequence}");
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }
}
