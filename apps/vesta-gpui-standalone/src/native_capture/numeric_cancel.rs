//! Observe numeric capture retirement through ordinary modal and native focus paths.
use super::*;
use number_loss::{Value, edit};

pub(super) fn trace(app: &App, window: UiWindowId, event: Event, sequence: u64) {
    let kind = match event {
        Event::Press => "pressed",
        Event::Release => "released",
        Event::Move => "moved",
        Event::Blur => "blurred",
        _ => return,
    };
    let number = app.fixture_number_scrub(window);
    let vector = app
        .fixture_vector_scrub(window)
        .map(|scrub| (scrub.axis, &scrub.session));
    if number.is_none() && vector.is_none() && !matches!(event, Event::Press | Event::Release) {
        return;
    }
    let cursor = if window == STANDALONE_WINDOW {
        Some(app.cursor)
    } else {
        app.native_windows
            .values()
            .find(|host| host.ui_window == window)
            .map(|host| host.cursor)
    };
    eprintln!(
        "native_numeric_pointer observed window={} sequence={} event={} cursor={cursor:?} number={} vector={:?} number_session={number:?} vector_session={vector:?} capture={:?}",
        window.0,
        sequence,
        kind,
        app.demo.number,
        app.demo.vector,
        app.fixture_input(window).and_then(InputOwnership::capture)
    );
}

#[derive(Clone, Copy, Debug)]
enum Mutation {
    Modal,
    Blur,
}

struct Pressed {
    target: InputTarget,
    initial: Value,
    number: i32,
    vector: [Option<f32>; 3],
}

struct Pending {
    pressed: Pressed,
    workspace: WorkspaceModel,
    blurred: bool,
    released: Option<UiWindowId>,
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
            "main-modal-number" => Mutation::Modal,
            "float-blur-vector" => Mutation::Blur,
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
                let Some(scrub) = edit(app, window, matches!(self.mutation, Mutation::Blur)) else {
                    return Ok(false);
                };
                if scrub.target.window != window
                    || app.text_field_at(window, Some(scrub.target.element))
                        != Some(scrub.initial.field())
                    || !app.fixture_input(window).is_some_and(|input| {
                        input.capture().is_some_and(|capture| {
                            capture.target == scrub.target
                                && capture.button == PointerButton::Primary
                        })
                    })
                {
                    return Err("native_numeric_cancel_press: current primary owner missing".into());
                }
                let mut pressed = Pressed {
                    target: scrub.target,
                    initial: scrub.initial,
                    number: app.demo.number,
                    vector: app.demo.vector,
                };
                match scrub.initial {
                    Value::Number(value) => pressed.number = value,
                    Value::Vector { values, .. } => pressed.vector = values,
                }
                eprintln!(
                    "native_numeric_cancel_fixture press window={} sequence={} target={} family={} initial={} capture=Primary mutation={:?}",
                    window.0,
                    sequence,
                    pressed.target.element.get(),
                    pressed.initial.family(),
                    pressed.initial,
                    self.mutation
                );
                self.state = if matches!(self.mutation, Mutation::Blur) {
                    let pending = Pending {
                        pressed,
                        workspace: app.demo.workspace.clone(),
                        blurred: false,
                        released: None,
                    };
                    let destination = app
                        .window
                        .as_ref()
                        .ok_or("native_numeric_cancel_destination: Main not ready")?;
                    destination.focus_window();
                    eprintln!(
                        "native_numeric_cancel_fixture focus_requested window={} sequence={} after_actual_press=true changed_preview=false",
                        window.0, sequence
                    );
                    State::Pending(pending)
                } else {
                    State::Pressed(pressed)
                };
            }
            State::Pressed(pressed) => {
                if window != pressed.target.window {
                    return Ok(false);
                }
                if matches!(event, Event::Release) {
                    self.state = State::Idle;
                    return Ok(false);
                }
                if matches!(event, Event::Press) {
                    return Err("native_numeric_cancel_press: repeated press".into());
                }
                if !matches!(event, Event::Move) {
                    return Ok(false);
                }
                let scrub = edit(app, window, false)
                    .ok_or("native_numeric_cancel_move: Number edit missing")?;
                if scrub.target != pressed.target || scrub.initial != pressed.initial {
                    return Err("native_numeric_cancel_move: Number owner changed".into());
                }
                if !scrub.dragging || scrub.initial.current(app) == scrub.initial {
                    return Ok(false);
                }
                eprintln!(
                    "native_numeric_cancel_fixture moved window={} sequence={} initial={} preview={} dragging=true",
                    window.0,
                    sequence,
                    pressed.initial,
                    pressed.initial.current(app)
                );
                let pending = Pending {
                    pressed: Pressed {
                        target: pressed.target,
                        initial: pressed.initial,
                        number: pressed.number,
                        vector: pressed.vector,
                    },
                    workspace: app.demo.workspace.clone(),
                    blurred: false,
                    released: None,
                };
                app.apply_window_action(window, DemoAction::OpenDemoDialog);
                self.state = State::Pending(pending);
                app.request_all_redraw();
            }
            State::Pending(pending) => {
                if matches!(event, Event::Press) {
                    return Err("native_numeric_cancel_interrupted: second press".into());
                }
                if matches!(self.mutation, Mutation::Blur) && !pending.blurred {
                    if window == pending.pressed.target.window && matches!(event, Event::Blur) {
                        pending.blurred = true;
                        eprintln!(
                            "native_numeric_cancel_fixture blurred window={} sequence={} initial={} current={}",
                            window.0,
                            sequence,
                            pending.pressed.initial,
                            pending.pressed.initial.current(app)
                        );
                    } else if matches!(event, Event::Release) {
                        return Err(
                            "native_numeric_cancel_order: release preceded actual blur".into()
                        );
                    } else {
                        return Ok(false);
                    }
                }
                verify_retired(app, pending)?;
                if matches!(event, Event::Release) {
                    if pending.released.replace(window).is_some() {
                        return Err("native_numeric_cancel_release: repeated release".into());
                    }
                    eprintln!(
                        "native_numeric_cancel_fixture release source={} window={} sequence={}",
                        pending.pressed.target.window.0, window.0, sequence
                    );
                }
                let Some(released) = pending.released else {
                    return Ok(false);
                };
                if app
                    .fixture_input(released)
                    .is_some_and(|input| input.capture().is_some())
                {
                    return Err(
                        "native_numeric_cancel_release: destination capture retained".into(),
                    );
                }
                let source = pending.pressed.target.window;
                if matches!(self.mutation, Mutation::Modal) {
                    let scopes = app
                        .window_scopes(source)
                        .ok_or("native_numeric_cancel_scope: modal window missing")?;
                    let expected = dialog_element(scopes.root, 1, 5);
                    if app.demo.dialog_stack.top() != Some(1) || app.overlay_owner != Some(source) {
                        return Err("native_numeric_cancel_modal: replacement modal changed".into());
                    }
                    if app
                        .window_actions(source)
                        .and_then(|route| route.action_at(expected))
                        .is_none()
                    {
                        return Ok(false);
                    }
                    if app.window_focus(source) != Some(expected) {
                        return Err("native_numeric_cancel_modal: replacement focus changed".into());
                    }
                } else if app.window_focus(source).is_some() {
                    return Err("native_numeric_cancel_blur: source focus retained".into());
                }
                eprintln!(
                    "native_numeric_cancel_fixture passed source={} release_window={} sequence={} target={} initial={} final={} mutation={:?} actual_blur={} capture=false workspace_unchanged=true fixture_cancel=false fixture_rebuild=false fixture_input=false",
                    source.0,
                    released.0,
                    sequence,
                    pending.pressed.target.element.get(),
                    pending.pressed.initial,
                    pending.pressed.initial.current(app),
                    self.mutation,
                    pending.blurred
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
    let window = pending.pressed.target.window;
    if app.demo.number != pending.pressed.number
        || app.demo.vector != pending.pressed.vector
        || edit(app, window, false).is_some()
        || edit(app, window, true).is_some()
        || app.text_session_field(window) == Some(pending.pressed.initial.field())
        || app
            .fixture_input(window)
            .is_none_or(|input| input.capture().is_some())
        || app.demo.workspace != pending.workspace
    {
        return Err(
            "native_numeric_cancel_retirement: values/edit/capture/workspace changed".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn qualify() {
    use crate::tests::{activate_gallery_page, install_action_route};

    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.inputs");
    install_action_route(&mut app);
    app.cursor = Point::new(100.0, 100.0);
    app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::Number, Some(app.cursor));
    app.begin_number_scrub();
    let mut probe = Probe::from_case("main-modal-number", Source::Main).expect("numeric modal");
    assert!(
        !probe
            .observe(&mut app, UiWindowId(999), Event::Press, 1)
            .expect("foreign")
    );
    assert!(
        !probe
            .observe(&mut app, STANDALONE_WINDOW, Event::Press, 2)
            .expect("primary")
    );
    app.cursor.x += 8.0;
    assert!(app.update_number_scrub());
    assert!(
        !probe
            .observe(&mut app, STANDALONE_WINDOW, Event::Move, 3)
            .expect("modal")
    );
    assert_eq!(app.demo.number, 128);
    assert!(app.number_scrub.is_none() && app.input_ownership.capture().is_none());
    install_action_route(&mut app);
    assert!(
        probe
            .observe(&mut app, STANDALONE_WINDOW, Event::Release, 4)
            .expect("late release")
    );
    assert!(!app.finish_number_scrub());

    // A source cancellation alone must never qualify an unobserved OS blur.
    let target = InputTarget {
        window: STANDALONE_WINDOW,
        element: TextFieldKind::Number.element(app.scopes.gallery),
    };
    let mut blur = Probe::from_case("float-blur-vector", Source::Float).expect("numeric blur");
    blur.state = State::Pending(Pending {
        pressed: Pressed {
            target,
            initial: Value::Number(128),
            number: 128,
            vector: app.demo.vector,
        },
        workspace: app.demo.workspace.clone(),
        blurred: false,
        released: None,
    });
    assert!(
        blur.observe(&mut app, STANDALONE_WINDOW, Event::Release, 5)
            .expect_err("release cannot stand in for actual blur")
            .contains("release preceded actual blur")
    );
}
