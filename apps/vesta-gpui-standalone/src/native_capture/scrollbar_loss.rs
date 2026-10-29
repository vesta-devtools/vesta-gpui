//! Observe physical thumb movement, then exercise ordinary subtree retirement.
use super::*;

pub(super) fn drag(app: &App, window: UiWindowId) -> Option<ElementId> {
    if window == STANDALONE_WINDOW {
        app.scrollbar_drag.map(|(element, _)| element)
    } else {
        app.native_windows
            .values()
            .find(|host| host.ui_window == window)?
            .scrollbar_drag
            .map(|(element, _)| element)
    }
}

struct Pressed {
    window: UiWindowId,
    target: ElementId,
    initial: Point,
}

struct Hidden {
    pressed: Pressed,
    reached: Point,
    focus: Option<ElementId>,
    workspace: WorkspaceModel,
}

enum State {
    Idle,
    Pressed(Pressed),
    Hidden(Hidden),
    Passed,
}

pub(super) struct Probe {
    source: Source,
    state: State,
}

impl Probe {
    pub(super) fn from_case(case: &str, source: Source) -> Option<Self> {
        matches!(case, "main-hide-scrollbar" | "float-hide-scrollbar").then_some(Self {
            source,
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
        match &self.state {
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
                let capture = app.fixture_input(window).and_then(InputOwnership::capture);
                if !capture.is_some_and(|capture| {
                    capture.target
                        == InputTarget {
                            window,
                            element: target,
                        }
                        && capture.button == PointerButton::Primary
                }) {
                    return Err(
                        "native_scrollbar_loss_press: matching primary capture missing".into(),
                    );
                }
                let initial = app.demo.horizontal_scroll.offset();
                eprintln!(
                    "native_scrollbar_loss_fixture press window={} sequence={} target={} initial={initial:?} capture=Primary",
                    window.0,
                    sequence,
                    target.get()
                );
                self.state = State::Pressed(Pressed {
                    window,
                    target,
                    initial,
                });
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
                if drag(app, window) != Some(pressed.target) {
                    return Err(
                        "native_scrollbar_loss_move: source drag retired before movement".into(),
                    );
                }
                let reached = app.demo.horizontal_scroll.offset();
                if reached == pressed.initial {
                    return Ok(false);
                }
                let scopes = app
                    .gallery_scopes(window)
                    .ok_or("native_scrollbar_loss_scope")?;
                let body = GallerySectionId::Horizontal.element(scopes, 3);
                let focus = app.window_focus(window).map(|element| {
                    if app
                        .window_actions(window)
                        .is_some_and(|route| route.is_descendant_of(element, body))
                    {
                        GallerySectionId::Horizontal.element(scopes, 1)
                    } else {
                        element
                    }
                });
                let hidden = Hidden {
                    pressed: Pressed {
                        window,
                        target: pressed.target,
                        initial: pressed.initial,
                    },
                    reached,
                    focus,
                    workspace: app.demo.workspace.clone(),
                };
                eprintln!(
                    "native_scrollbar_loss_fixture moved window={} sequence={} target={} initial={:?} reached={reached:?}",
                    window.0,
                    sequence,
                    pressed.target.get(),
                    pressed.initial
                );
                app.apply_window_action(
                    window,
                    DemoAction::ToggleGallerySection(GallerySectionId::Horizontal),
                );
                verify_retired(app, &hidden)?;
                eprintln!(
                    "native_scrollbar_loss_fixture hidden window={} sequence={} reached={reached:?} fixture_cancel=false fixture_rebuild=false fixture_input=false",
                    window.0, sequence
                );
                self.state = State::Hidden(hidden);
                app.request_all_redraw();
            }
            State::Hidden(hidden) => {
                verify_retired(app, hidden)?;
                if window != hidden.pressed.window || !matches!(event, Event::Release) {
                    return Ok(false);
                }
                eprintln!(
                    "native_scrollbar_loss_fixture passed window={} sequence={} target={} initial={:?} final={:?} focus={:?} capture=false workspace_unchanged=true",
                    window.0,
                    sequence,
                    hidden.pressed.target.get(),
                    hidden.pressed.initial,
                    hidden.reached,
                    hidden.focus.map(ElementId::get)
                );
                self.state = State::Passed;
                return Ok(true);
            }
            State::Passed => {}
        }
        Ok(false)
    }
}

fn verify_retired(app: &App, hidden: &Hidden) -> Result<(), String> {
    let window = hidden.pressed.window;
    if app.demo.horizontal_scroll.offset() != hidden.reached
        || drag(app, window).is_some()
        || app
            .fixture_input(window)
            .is_none_or(|input| input.capture().is_some())
        || !app
            .demo
            .collapsed_gallery_sections
            .contains(&GallerySectionId::Horizontal)
        || app.window_focus(window) != hidden.focus
        || app.demo.workspace != hidden.workspace
    {
        return Err(
            "native_scrollbar_loss_retirement: offset/session/capture/focus/workspace changed"
                .into(),
        );
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn qualify(app: &mut App) {
    let mut probe = Probe::from_case("main-hide-scrollbar", Source::Main).expect("scrollbar probe");
    assert!(
        !probe
            .observe(app, UiWindowId(999), Event::Press, 1)
            .expect("foreign press")
    );
    assert!(
        !probe
            .observe(app, STANDALONE_WINDOW, Event::Press, 2)
            .expect("primary press")
    );
    assert!(
        !probe
            .observe(app, STANDALONE_WINDOW, Event::Move, 3)
            .expect("unchanged offset")
    );
    assert!(
        !app.demo
            .collapsed_gallery_sections
            .contains(&GallerySectionId::Horizontal)
    );
    app.cursor.x -= 8.0;
    app.update_scrollbar_drag();
    assert!(
        !probe
            .observe(app, STANDALONE_WINDOW, Event::Move, 4)
            .expect("ordinary hide")
    );
    app.cursor.x -= 8.0;
    app.update_scrollbar_drag();
    assert!(
        !probe
            .observe(app, STANDALONE_WINDOW, Event::Move, 5)
            .expect("late move")
    );
    assert!(
        !probe
            .observe(app, UiWindowId(999), Event::Release, 6)
            .expect("foreign release")
    );
    assert!(
        probe
            .observe(app, STANDALONE_WINDOW, Event::Release, 7)
            .expect("same-source late release")
    );
    assert!(
        !probe
            .observe(app, STANDALONE_WINDOW, Event::Release, 8)
            .expect("duplicate release")
    );
}
