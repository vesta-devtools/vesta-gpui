//! Physical numeric preview followed by the ordinary section-visibility action.
use super::*;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Value {
    Number(i32),
    Vector { axis: u8, values: [Option<f32>; 3] },
}

impl Value {
    pub(super) fn family(self) -> &'static str {
        match self {
            Self::Number(_) => "number",
            Self::Vector { .. } => "vector",
        }
    }

    pub(super) fn field(self) -> TextFieldKind {
        match self {
            Self::Number(_) => TextFieldKind::Number,
            Self::Vector { axis, .. } => TextFieldKind::Vector(axis),
        }
    }

    pub(super) fn current(self, app: &App) -> Self {
        match self {
            Self::Number(_) => Self::Number(app.demo.number),
            Self::Vector { axis, .. } => Self::Vector {
                axis,
                values: app.demo.vector,
            },
        }
    }

    fn section(self) -> GallerySectionId {
        match self {
            Self::Number(_) => GallerySectionId::Inputs,
            Self::Vector { .. } => GallerySectionId::Property,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Number(value) => value.fmt(f),
            Self::Vector { values, .. } => write!(f, "{values:?}"),
        }
    }
}

pub(super) struct Edit {
    pub(super) target: InputTarget,
    pub(super) initial: Value,
    pub(super) dragging: bool,
}

pub(super) fn edit(app: &App, window: UiWindowId, vector: bool) -> Option<Edit> {
    if vector {
        let scrub = app.fixture_vector_scrub(window)?;
        let mut values = app.demo.vector;
        values[usize::from(scrub.axis)] = *scrub.initial();
        Some(Edit {
            target: scrub.target(),
            initial: Value::Vector {
                axis: scrub.axis,
                values,
            },
            dragging: scrub.is_dragging(),
        })
    } else {
        let scrub = app.fixture_number_scrub(window)?;
        Some(Edit {
            target: scrub.target(),
            initial: Value::Number(*scrub.initial()),
            dragging: scrub.is_dragging(),
        })
    }
}

#[derive(Clone, Copy)]
struct Gesture {
    source: UiWindowId,
    target: ElementId,
    initial: Value,
}

struct Hidden {
    gesture: Gesture,
    workspace: WorkspaceModel,
    header: ElementId,
}

enum State {
    Idle,
    Pressed(Gesture),
    Hidden(Hidden),
    Passed,
}

pub(super) struct Probe {
    source: Source,
    vector: bool,
    state: State,
}

impl Probe {
    pub(super) fn from_case(case: &str, source: Source) -> Option<Self> {
        matches!(
            case,
            "main-hide-number" | "float-hide-number" | "main-hide-vector" | "float-hide-vector"
        )
        .then_some(Self {
            source,
            vector: case.ends_with("-vector"),
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
                let Some(scrub) = edit(app, window, self.vector) else {
                    return Ok(false);
                };
                let gesture = Gesture {
                    source: window,
                    target: scrub.target.element,
                    initial: scrub.initial,
                };
                if scrub.target.window != window
                    || app.text_field_at(window, Some(gesture.target))
                        != Some(gesture.initial.field())
                {
                    return Err(format!(
                        "native_{}_loss_target: not the current numeric field",
                        gesture.initial.family()
                    ));
                }
                eprintln!(
                    "native_{}_loss_fixture press window={} sequence={} target={} initial={} preview={}",
                    gesture.initial.family(),
                    window.0,
                    sequence,
                    gesture.target.get(),
                    gesture.initial,
                    gesture.initial.current(app),
                );
                self.state = State::Pressed(gesture);
            }
            State::Pressed(gesture) => {
                let gesture = *gesture;
                if window != gesture.source {
                    return Ok(false);
                }
                if matches!(event, Event::Press) {
                    return Err(format!(
                        "native_{}_loss_press: repeated press before release",
                        gesture.initial.family()
                    ));
                }
                if matches!(event, Event::Release) {
                    // A normal click may focus the field before the actual scrub gesture.
                    self.state = State::Idle;
                    return Ok(false);
                }
                if !matches!(event, Event::Move) {
                    return Ok(false);
                }
                let Some(scrub) = edit(app, window, self.vector) else {
                    return Err(format!(
                        "native_{}_loss_edit: numeric edit lost before preview",
                        gesture.initial.family()
                    ));
                };
                if scrub.target.element != gesture.target || scrub.initial != gesture.initial {
                    return Err(format!(
                        "native_{}_loss_owner: numeric owner changed before preview",
                        gesture.initial.family()
                    ));
                }
                if !scrub.dragging || gesture.initial.current(app) == gesture.initial {
                    return Ok(false);
                }
                let header = gesture.initial.section().element(
                    app.gallery_scopes(window)
                        .ok_or("native_number_loss_source: window missing")?,
                    1,
                );
                eprintln!(
                    "native_{}_loss_fixture preview window={} sequence={} target={} initial={} preview={} dragging=true",
                    gesture.initial.family(),
                    window.0,
                    sequence,
                    gesture.target.get(),
                    gesture.initial,
                    gesture.initial.current(app),
                );
                app.apply_window_action(
                    window,
                    DemoAction::ToggleGallerySection(gesture.initial.section()),
                );
                let hidden = Hidden {
                    gesture,
                    workspace: app.demo.workspace.clone(),
                    header,
                };
                verify_retired(app, &hidden)?;
                eprintln!(
                    "native_{}_loss_fixture hidden window={} sequence={} value={} fixture_cancel=false fixture_rebuild=false fixture_input=false",
                    gesture.initial.family(),
                    window.0,
                    sequence,
                    gesture.initial.current(app),
                );
                self.state = State::Hidden(hidden);
                app.request_all_redraw();
            }
            State::Hidden(hidden) => {
                verify_retired(app, hidden)?;
                if matches!(event, Event::Press) {
                    return Err(format!(
                        "native_{}_loss_press: another press before late release",
                        hidden.gesture.initial.family()
                    ));
                }
                if !matches!(event, Event::Release) {
                    return Ok(false);
                }
                eprintln!(
                    "native_{}_loss_fixture passed source={} released={} sequence={} target={} initial={} final={} editing=false focus={} workspace_unchanged=true",
                    hidden.gesture.initial.family(),
                    hidden.gesture.source.0,
                    window.0,
                    sequence,
                    hidden.gesture.target.get(),
                    hidden.gesture.initial,
                    hidden.gesture.initial.current(app),
                    hidden.header.get(),
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
    let value = hidden.gesture.initial;
    if value.current(app) != value
        || edit(
            app,
            hidden.gesture.source,
            matches!(value, Value::Vector { .. }),
        )
        .is_some()
        || app.text_session_field(hidden.gesture.source) == Some(value.field())
        || !app
            .demo
            .collapsed_gallery_sections
            .contains(&value.section())
    {
        return Err(format!(
            "native_{}_loss_retirement: preview or numeric edit survived hide",
            value.family()
        ));
    }
    if app.demo.workspace != hidden.workspace
        || app.window_focus(hidden.gesture.source) != Some(hidden.header)
    {
        return Err(format!(
            "native_{}_loss_late: replacement workspace or focus changed",
            value.family()
        ));
    }
    if app
        .fixture_input(hidden.gesture.source)
        .is_none_or(|input| input.capture().is_some())
    {
        return Err(format!(
            "native_{}_loss_capture: source capture survived hide",
            value.family()
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn qualify() {
    use crate::tests::{activate_gallery_page, install_action_route};

    for axis in [None, Some(0), Some(1), Some(2)] {
        let mut app = App::new();
        activate_gallery_page(&mut app, "gallery.inputs");
        install_action_route(&mut app);
        app.cursor = Point::new(100.0, 100.0);
        let field = axis.map_or(TextFieldKind::Number, TextFieldKind::Vector);
        app.begin_text_selection(STANDALONE_WINDOW, field, Some(app.cursor));
        if let Some(axis) = axis {
            app.begin_vector_scrub(axis);
        } else {
            app.begin_number_scrub();
        }
        let case = if axis.is_some() {
            "main-hide-vector"
        } else {
            "main-hide-number"
        };
        let mut probe = Probe::from_case(case, Source::Main).expect("existing numeric probe");
        assert!(
            !probe
                .observe(&mut app, UiWindowId(999), Event::Press, 1)
                .expect("foreign press")
        );
        assert!(
            !probe
                .observe(&mut app, STANDALONE_WINDOW, Event::Press, 2)
                .expect("owned press")
        );
        assert!(
            !probe
                .observe(&mut app, STANDALONE_WINDOW, Event::Move, 3)
                .expect("no preview")
        );
        assert!(
            !app.demo
                .collapsed_gallery_sections
                .contains(&GallerySectionId::Inputs)
        );
        app.cursor.x += 8.0;
        assert!(if axis.is_some() {
            app.update_vector_scrub()
        } else {
            app.update_number_scrub()
        });
        assert!(
            !probe
                .observe(&mut app, STANDALONE_WINDOW, Event::Move, 4)
                .expect("hide preview")
        );
        assert_eq!(app.demo.vector, [Some(1.0), None, Some(3.0)]);
        assert_eq!(app.demo.number, 128);
        assert!(
            !probe
                .observe(&mut app, STANDALONE_WINDOW, Event::Move, 5)
                .expect("retired movement")
        );
        assert!(
            probe
                .observe(&mut app, STANDALONE_WINDOW, Event::Release, 6)
                .expect("late release")
        );
        assert!(
            !probe
                .observe(&mut app, STANDALONE_WINDOW, Event::Release, 7)
                .expect("duplicate release")
        );
        assert!(!app.finish_vector_scrub() && !app.finish_number_scrub());
    }
}
