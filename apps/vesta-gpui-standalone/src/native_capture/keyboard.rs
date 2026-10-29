//! Observes real held-key focus replay; never creates or alters an input event.
use super::*;
use vesta_gpui_widgets::ThemeMode;

pub(super) enum Status {
    Pending,
    Released,
    Passed,
}

struct Pending {
    source: UiWindowId,
    destination: UiWindowId,
    expected: ThemeMode,
    blurred: bool,
    focused: bool,
    replay_pressed: bool,
    released: bool,
}

pub(super) struct Probe {
    source: Source,
    initial: Option<ThemeMode>,
    pending: Option<Pending>,
}

impl Probe {
    pub(super) fn from_case(case: &str, source: Source) -> Option<Self> {
        case.ends_with("-key-replay").then_some(Self {
            source,
            initial: None,
            pending: None,
        })
    }

    pub(super) fn prepare(&mut self, initial: ThemeMode) {
        self.initial = Some(initial);
        eprintln!(
            "native_key_fixture ready source={:?} initial={initial:?} key=T",
            self.source
        );
    }

    pub(super) fn observe(
        &mut self,
        app: &mut App,
        window: UiWindowId,
        event: Event,
        sequence: u64,
    ) -> Result<Status, String> {
        if let Event::ThemeKey {
            state,
            synthetic,
            repeat,
        } = event
        {
            eprintln!(
                "native_key_fixture key window={} sequence={} state={state:?} synthetic={synthetic} repeat={repeat} theme={:?}",
                window.0, sequence, app.theme.mode
            );
            if state == ElementState::Pressed && !synthetic {
                if self.pending.is_some() || repeat {
                    return Err("native_key_interrupted: another ordinary press or repeat".into());
                }
                if (self.source == Source::Main) != (window == STANDALONE_WINDOW) {
                    return Ok(Status::Pending);
                }
                let initial = self
                    .initial
                    .ok_or("native_key_setup: initial theme missing")?;
                let expected = WidgetTheme::for_mode(initial).toggled().mode;
                if app.theme.mode != expected {
                    return Err(
                        "native_key_action: source T did not toggle the initial theme once".into(),
                    );
                }
                let (destination, native) = if window == STANDALONE_WINDOW {
                    app.native_windows
                        .values()
                        .find(|host| host.surface.available())
                        .map(|host| (host.ui_window, host.window.clone()))
                } else {
                    app.window.clone().map(|native| (STANDALONE_WINDOW, native))
                }
                .ok_or("native_key_setup: destination unavailable")?;
                self.pending = Some(Pending {
                    source: window,
                    destination,
                    expected,
                    blurred: false,
                    focused: false,
                    replay_pressed: false,
                    released: false,
                });
                eprintln!(
                    "native_key_fixture transfer source={} destination={} sequence={} theme={expected:?} focus_requested=true",
                    window.0, destination.0, sequence
                );
                native.focus_window();
            }
        }
        let Some(pending) = &mut self.pending else {
            return Ok(Status::Pending);
        };
        if app.theme.mode != pending.expected {
            return Err(
                "native_key_replay_action: theme changed after the one ordinary press".into(),
            );
        }
        let mut release = false;
        match event {
            Event::Blur if window == pending.source => pending.blurred = true,
            Event::Focus if window == pending.destination => pending.focused = true,
            Event::ThemeKey {
                state: ElementState::Pressed,
                synthetic: true,
                ..
            } if window == pending.destination => pending.replay_pressed = true,
            Event::ThemeKey {
                state: ElementState::Released,
                synthetic: false,
                ..
            } => {
                pending.released = true;
                release = true;
            }
            Event::Press => {
                return Err("native_key_interrupted: pointer press during replay".into());
            }
            _ => {}
        }
        if matches!(event, Event::Blur | Event::Focus | Event::ThemeKey { .. }) {
            eprintln!(
                "native_key_fixture observed window={} sequence={} blurred={} focused={} replay_pressed={} ordinary_release={} theme={:?}",
                window.0,
                sequence,
                pending.blurred,
                pending.focused,
                pending.replay_pressed,
                pending.released,
                app.theme.mode
            );
        }
        if pending.blurred && pending.focused && pending.replay_pressed && pending.released {
            eprintln!(
                "native_key_fixture passed source={} destination={} sequence={} synthetic_pressed=true ordinary_release=true one_action=true",
                pending.source.0, pending.destination.0, sequence
            );
            Ok(Status::Passed)
        } else if release {
            Ok(Status::Released)
        } else {
            Ok(Status::Pending)
        }
    }
}
