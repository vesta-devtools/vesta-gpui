//! Observe held thumb retirement through ordinary modal or native focus changes.
use super::*;
use scrollbar_loss::drag;

#[derive(Clone, Copy, Debug)]
enum Mutation {
    Modal,
    Blur,
    Resize,
}

struct Pressed {
    window: UiWindowId,
    target: ElementId,
    initial: Point,
}

struct Pending {
    pressed: Pressed,
    reached: Point,
    workspace: WorkspaceModel,
    blurred: bool,
    released: Option<UiWindowId>,
    requested_size: Option<winit::dpi::PhysicalSize<u32>>,
    resized: bool,
}

enum State {
    Idle,
    Pressed(Pressed),
    Pending(Pending),
    Passed,
}

pub(super) struct Probe {
    source: Source,
    mutation: Mutation,
    state: State,
}

impl Probe {
    pub(super) fn from_case(case: &str, source: Source) -> Option<Self> {
        let mutation = match case {
            "main-modal-scrollbar" | "float-modal-scrollbar" => Mutation::Modal,
            "main-blur-scrollbar" | "float-blur-scrollbar" => Mutation::Blur,
            "float-resize-scrollbar" => Mutation::Resize,
            _ => return None,
        };
        Some(Self {
            source,
            mutation,
            state: State::Idle,
        })
    }

    pub(super) fn observe(
        &mut self,
        app: &mut App,
        window: UiWindowId,
        event: Event,
        sequence: u64,
    ) -> Result<bool, String> {
        match &mut self.state {
            State::Idle => {
                if !matches!(event, Event::Press)
                    || (self.source == Source::Main) != (window == STANDALONE_WINDOW)
                {
                    return Ok(false);
                }
                let Some(scope) = app.gallery_scopes(window) else {
                    return Ok(false);
                };
                let target = scope.element(HORIZONTAL_SCROLL);
                if drag(app, window) != Some(target) {
                    return Ok(false);
                }
                if !app.fixture_input(window).is_some_and(|input| {
                    input.capture().is_some_and(|capture| {
                        capture.target
                            == InputTarget {
                                window,
                                element: target,
                            }
                            && capture.button == PointerButton::Primary
                    })
                }) {
                    return Err("native_scrollbar_cancel_press: primary capture missing".into());
                }
                let initial = app.demo.horizontal_scroll.offset();
                eprintln!(
                    "native_scrollbar_cancel_fixture press window={} sequence={} target={} initial={initial:?} capture=Primary mutation={:?}",
                    window.0,
                    sequence,
                    target.get(),
                    self.mutation
                );
                let pressed = Pressed {
                    window,
                    target,
                    initial,
                };
                self.state = if matches!(self.mutation, Mutation::Blur) {
                    let pending = Pending {
                        pressed,
                        reached: initial,
                        workspace: app.demo.workspace.clone(),
                        blurred: false,
                        released: None,
                        requested_size: None,
                        resized: false,
                    };
                    let destination = if window == STANDALONE_WINDOW {
                        app.native_windows
                            .values()
                            .find(|host| host.surface.available())
                            .map(|host| host.window.clone())
                    } else {
                        app.window.clone()
                    }
                    .ok_or("native_scrollbar_cancel_destination: window not ready")?;
                    destination.focus_window();
                    eprintln!(
                        "native_scrollbar_cancel_fixture focus_requested window={} sequence={} after_actual_press=true",
                        window.0, sequence
                    );
                    State::Pending(pending)
                } else {
                    State::Pressed(pressed)
                };
            }
            State::Pressed(pressed) => {
                if window != pressed.window {
                    return Ok(false);
                }
                if matches!(event, Event::Release) {
                    self.state = State::Idle;
                    return Ok(false);
                }
                if !matches!(event, Event::Move) {
                    return Ok(false);
                }
                let reached = app.demo.horizontal_scroll.offset();
                if reached == pressed.initial {
                    return Ok(false);
                }
                if drag(app, window) != Some(pressed.target) {
                    return Err("native_scrollbar_cancel_move: live drag missing".into());
                }
                let mut pending = Pending {
                    pressed: Pressed {
                        window,
                        target: pressed.target,
                        initial: pressed.initial,
                    },
                    reached,
                    workspace: app.demo.workspace.clone(),
                    blurred: false,
                    released: None,
                    requested_size: None,
                    resized: false,
                };
                eprintln!(
                    "native_scrollbar_cancel_fixture moved window={} sequence={} initial={:?} reached={reached:?} mutation={:?}",
                    window.0, sequence, pressed.initial, self.mutation
                );
                if matches!(self.mutation, Mutation::Resize) {
                    let host = app
                        .native_windows
                        .values()
                        .find(|host| host.ui_window == window)
                        .ok_or("native_scrollbar_geometry_source: floating window missing")?;
                    let mut size = host.window.inner_size();
                    size.width += 80;
                    pending.requested_size = Some(size);
                    let logical = size.to_logical::<f32>(host.window.scale_factor());
                    let expected = pending
                        .workspace
                        .floating_window(host.logical_id)
                        .ok_or("native_scrollbar_geometry_model: floating placement missing")?;
                    let placement = WindowPlacement::new(
                        expected.placement.x,
                        expected.placement.y,
                        logical.width,
                        logical.height,
                    );
                    pending
                        .workspace
                        .set_floating_placement(host.logical_id, placement)
                        .map_err(|error| format!("native_scrollbar_geometry_model: {error}"))?;
                    let _ = host.window.request_inner_size(size);
                    eprintln!(
                        "native_scrollbar_cancel_fixture resize_requested window={} sequence={} physical={size:?} fixture_cancel=false fixture_input=false",
                        window.0, sequence
                    );
                } else {
                    app.apply_window_action(window, DemoAction::OpenDemoDialog);
                }
                self.state = State::Pending(pending);
                app.request_all_redraw();
            }
            State::Pending(pending) => {
                if matches!(event, Event::Press) {
                    return Err(
                        "native_scrollbar_cancel_interrupted: second press before completion"
                            .into(),
                    );
                }
                if matches!(self.mutation, Mutation::Blur) && !pending.blurred {
                    if window == pending.pressed.window && matches!(event, Event::Blur) {
                        // Further physical movement before the OS blur may advance navigation.
                        pending.reached = app.demo.horizontal_scroll.offset();
                        pending.blurred = true;
                        eprintln!(
                            "native_scrollbar_cancel_fixture blurred window={} sequence={} reached={:?}",
                            window.0, sequence, pending.reached
                        );
                    } else if matches!(event, Event::Release) {
                        return Err(
                            "native_scrollbar_cancel_order: release preceded actual blur".into(),
                        );
                    } else {
                        return Ok(false);
                    }
                }
                if matches!(self.mutation, Mutation::Resize) && !pending.resized {
                    if let Event::Resized(size) = event
                        && window == pending.pressed.window
                        && Some(size) == pending.requested_size
                    {
                        pending.resized = true;
                        eprintln!(
                            "native_scrollbar_cancel_fixture resized window={} sequence={} physical={size:?} reached={:?} actual_offset={:?} drag={:?} capture={:?} workspace_expected={}",
                            window.0,
                            sequence,
                            pending.reached,
                            app.demo.horizontal_scroll.offset(),
                            drag(app, window),
                            app.fixture_input(window).and_then(InputOwnership::capture),
                            app.demo.workspace == pending.workspace
                        );
                    } else if matches!(event, Event::Release) {
                        return Err(
                            "native_scrollbar_geometry_order: release preceded actual resize"
                                .into(),
                        );
                    } else {
                        return Ok(false);
                    }
                }
                if matches!(event, Event::Release) {
                    if pending.released.replace(window).is_some() {
                        return Err("native_scrollbar_cancel_release: repeated release".into());
                    }
                    eprintln!(
                        "native_scrollbar_cancel_fixture release source={} window={} sequence={} reached={:?}",
                        pending.pressed.window.0, window.0, sequence, pending.reached
                    );
                }
                verify_retired(app, pending)?;
                let Some(released) = pending.released else {
                    return Ok(false);
                };
                if app
                    .fixture_input(released)
                    .is_some_and(|input| input.capture().is_some())
                {
                    return Err(
                        "native_scrollbar_cancel_release: destination capture retained".into(),
                    );
                }
                if matches!(self.mutation, Mutation::Modal) {
                    let scopes = app
                        .window_scopes(pending.pressed.window)
                        .ok_or("native_scrollbar_cancel_scope: modal window missing")?;
                    let expected = dialog_element(scopes.root, 1, 5);
                    if app.demo.dialog_stack.top() != Some(1)
                        || app.overlay_owner != Some(pending.pressed.window)
                    {
                        return Err(
                            "native_scrollbar_cancel_modal: replacement modal changed".into()
                        );
                    }
                    // Wait for ordinary publication; the probe cannot rebuild or focus it.
                    if app
                        .window_actions(pending.pressed.window)
                        .and_then(|route| route.action_at(expected))
                        .is_none()
                    {
                        return Ok(false);
                    }
                    if app.window_focus(pending.pressed.window) != Some(expected) {
                        return Err("native_scrollbar_cancel_focus: modal focus changed".into());
                    }
                } else if matches!(self.mutation, Mutation::Blur)
                    && app.window_focus(pending.pressed.window).is_some()
                {
                    return Err("native_scrollbar_cancel_blur: source focus retained".into());
                }
                eprintln!(
                    "native_scrollbar_cancel_fixture passed source={} release_window={} sequence={} target={} initial={:?} final={:?} mutation={:?} actual_blur={} actual_resize={} capture=false workspace_expected=true fixture_cancel=false fixture_rebuild=false fixture_input=false",
                    pending.pressed.window.0,
                    released.0,
                    sequence,
                    pending.pressed.target.get(),
                    pending.pressed.initial,
                    pending.reached,
                    self.mutation,
                    pending.blurred,
                    pending.resized
                );
                self.state = State::Passed;
                return Ok(true);
            }
            State::Passed => {}
        }
        Ok(false)
    }
}

fn verify_retired(app: &App, pending: &Pending) -> Result<(), String> {
    if app.demo.horizontal_scroll.offset() != pending.reached
        || drag(app, pending.pressed.window).is_some()
        || app
            .fixture_input(pending.pressed.window)
            .is_none_or(|input| input.capture().is_some())
        || app.demo.workspace != pending.workspace
    {
        return Err(
            "native_scrollbar_cancel_retirement: offset/drag/capture/workspace changed".into(),
        );
    }
    Ok(())
}
