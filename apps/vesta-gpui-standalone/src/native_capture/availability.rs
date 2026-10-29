//! Native pending-click cancellation when an existing typed intent disables its owner.
use super::*;

struct Pending {
    window: UiWindowId,
    target: ElementId,
    initial: i32,
    disabled: bool,
    released: bool,
}

pub(super) struct Probe {
    source: Source,
    calibrated: Option<ElementId>,
    pending: Option<Pending>,
}

impl Probe {
    pub(super) fn from_case(case: &str, source: Source) -> Option<Self> {
        matches!(case, "main-disable" | "float-disable").then_some(Self {
            source,
            calibrated: None,
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
        if matches!(event, Event::Press) {
            if self.pending.is_some() {
                return Err("native_disabled_interrupted: a new press preceded completion".into());
            }
            if (self.source == Source::Main) != (window == STANDALONE_WINDOW) {
                return Ok(false);
            }
            let Some(target) = pressed_child(app, window) else {
                return Ok(false);
            };
            if !app.demo.toggle || app.demo.radio != 0 || app.demo.number == NUMBER_MAX {
                return Err(
                    "native_disabled_setup: child must initially be enabled and visible".into(),
                );
            }
            if self.calibrated.is_some_and(|expected| target != expected) {
                return Err("native_disabled_identity: calibrated child changed".into());
            }
            let disabled = self.calibrated.is_some();
            let initial = app.demo.number;
            self.pending = Some(Pending {
                window,
                target,
                initial,
                disabled,
                released: false,
            });
            eprintln!(
                "native_disabled_fixture press window={} sequence={} target={} value={} calibrated={} focus=true",
                window.0,
                sequence,
                target.get(),
                app.demo.number,
                disabled,
            );
            if disabled {
                // Mutate only through the existing window-qualified author intent.
                // The ordinary host must reconcile its own tree and pending input.
                app.apply_window_action(window, DemoAction::ToggleSwitch);
                if app.demo.toggle || initial != app.demo.number {
                    return Err(
                        "native_disabled_mutation: ToggleSwitch did not only disable child".into(),
                    );
                }
                app.request_all_redraw();
                eprintln!(
                    "native_disabled_fixture disabled window={} sequence={} target={} intent=ToggleSwitch route_rebuilt=false input_canceled=false",
                    window.0,
                    sequence,
                    target.get(),
                );
            }
            return Ok(false);
        }
        let Some(pending) = self.pending.as_mut() else {
            return Ok(false);
        };
        if matches!(event, Event::Blur) && window == pending.window {
            return Err("native_disabled_interrupted: source blurred before completion".into());
        }
        if matches!(event, Event::Release) {
            if window != pending.window || pending.released {
                return Err("native_disabled_release: missing unique same-window release".into());
            }
            pending.released = true;
            if !pending.disabled {
                if app.demo.number != pending.initial + 1 || !app.demo.toggle || app.demo.radio != 0
                {
                    return Err(
                        "native_disabled_control: ordinary child click did not increment once"
                            .into(),
                    );
                }
                eprintln!(
                    "native_disabled_fixture calibrated window={} sequence={} target={} before={} after={} increment=1",
                    window.0,
                    sequence,
                    pending.target.get(),
                    pending.initial,
                    app.demo.number,
                );
                self.calibrated = Some(pending.target);
                self.pending = None;
                return Ok(false);
            }
            eprintln!(
                "native_disabled_fixture release window={} sequence={} target={} value={}",
                window.0,
                sequence,
                pending.target.get(),
                app.demo.number,
            );
        }
        if !pending.disabled {
            return Ok(false);
        }
        if app.demo.number != pending.initial || app.demo.toggle || app.demo.radio != 0 {
            return Err(
                "native_disabled_late_commit: disabled child or parent accepted stale input".into(),
            );
        }
        if !pending.released {
            return Ok(false);
        }
        let route = app
            .window_actions(pending.window)
            .ok_or("native_disabled_owner: window missing")?;
        let input = app
            .fixture_input(pending.window)
            .ok_or("native_disabled_owner: input missing")?;
        // A normal redraw may follow the physical release; observing it is not
        // permission for the fixture to rebuild routes or clear host ownership.
        if route.action_at(pending.target).is_some()
            || input
                .focus()
                .is_some_and(|focus| focus.element == pending.target)
            || input.capture().is_some()
        {
            return Ok(false);
        }
        eprintln!(
            "native_disabled_fixture passed window={} sequence={} target={} value={} native_release=true action=false focus=false capture=false late_commit=false",
            pending.window.0,
            sequence,
            pending.target.get(),
            app.demo.number,
        );
        Ok(true)
    }
}

fn pressed_child(app: &App, window: UiWindowId) -> Option<ElementId> {
    let route = app.window_actions(window)?;
    let root = GallerySectionId::Conditional.element(app.gallery_scopes(window)?, 0);
    let mut candidates = route.focus_order().iter().copied().filter(|target| {
        route.is_descendant_of(*target, root)
            && route.action_at(*target) == Some(&DemoAction::StepNumber(1))
    });
    let target = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    let hit = if window == STANDALONE_WINDOW {
        app.hit_test.hit(app.cursor)
    } else {
        let host = app
            .native_windows
            .values()
            .find(|host| host.ui_window == window)?;
        host.hit_test.hit(host.cursor)
    }?;
    (route.target_for_hit(hit).map(|(owner, _)| owner) == Some(target)
        && app.fixture_input(window)?.focus()
            == Some(InputTarget {
                window,
                element: target,
            }))
    .then_some(target)
}
