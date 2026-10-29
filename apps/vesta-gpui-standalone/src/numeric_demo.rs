//! Existing numeric draft and scrub policy shared by Standalone windows.
use super::*;

/// One number scrub step. The session is per window, so the same arithmetic serves the main
/// window and every native floating window.
pub(super) fn number_scrub_value(
    scrub: &mut NumberScrub,
    window: UiWindowId,
    pointer_x: f32,
    current: i32,
) -> Option<i32> {
    let delta = scrub.offset(window, pointer_x)?;
    let steps = (delta / 4.0).round() as i32;
    let value = scrub
        .initial()
        .saturating_add(steps)
        .clamp(NUMBER_MIN, NUMBER_MAX);
    (value != current).then_some(value)
}

/// One vector component scrub step, clamped to the shared component range.
pub(super) fn vector_scrub_value(
    scrub: &mut VectorScrub,
    window: UiWindowId,
    pointer_x: f32,
    current: Option<f32>,
) -> Option<f32> {
    let delta = scrub.offset(window, pointer_x)?;
    let steps = (delta / 4.0).round();
    let value = (scrub.initial().unwrap_or(0.0) + steps * 0.1).clamp(VECTOR_MIN, VECTOR_MAX);
    (current != Some(value)).then_some(value)
}

fn retire_numeric_capture(input: &mut InputOwnership, target: InputTarget) {
    if input
        .capture()
        .is_some_and(|capture| capture.target == target && capture.button == PointerButton::Primary)
    {
        input.cancel_capture();
    }
}

impl App {
    pub(super) fn reset_number_draft(&mut self) {
        let value = self.demo.number.to_string();
        let _ = self.number_state.reset(&mut self.text_context, &value);
    }

    pub(super) fn commit_number_draft(&mut self) -> bool {
        let decision = resolve_value_draft(self.number_state.text(), &self.demo.number, |raw| {
            raw.trim()
                .parse::<i32>()
                .ok()
                .map(|value| value.clamp(NUMBER_MIN, NUMBER_MAX))
        });
        let accepted = match decision {
            DraftResolution::Accepted { value, .. } => {
                self.demo.number = value;
                true
            }
            DraftResolution::Rejected { .. } => false,
        };
        self.reset_number_draft();
        accepted
    }

    pub(super) fn reset_vector_draft(&mut self, axis: u8) {
        let index = usize::from(axis);
        if let (Some(value), Some(state)) = (
            self.demo.vector.get(index),
            self.vector_states.get_mut(index),
        ) {
            let text = value.map_or_else(|| "—".to_owned(), |number| number.to_string());
            let _ = state.reset(&mut self.text_context, &text);
        }
    }

    pub(super) fn commit_vector_draft(&mut self, axis: u8) -> bool {
        let index = usize::from(axis);
        let (Some(state), Some(component)) = (
            self.vector_states.get(index),
            self.demo.vector.get_mut(index),
        ) else {
            return false;
        };
        let decision = resolve_value_draft(state.text(), component, |raw| {
            raw.trim()
                .parse::<f32>()
                .ok()
                .filter(|number| number.is_finite())
                .map(|value| Some(value.clamp(VECTOR_MIN, VECTOR_MAX)))
        });
        let accepted = match decision {
            DraftResolution::Accepted { value, .. } => {
                *component = value;
                true
            }
            DraftResolution::Rejected { .. } => false,
        };
        self.reset_vector_draft(axis);
        accepted
    }

    pub(super) fn begin_number_scrub(&mut self) {
        self.number_scrub = NumberScrub::new(
            InputTarget {
                window: STANDALONE_WINDOW,
                element: self.scopes.gallery.element(NUMBER_FIELD),
            },
            self.cursor.x,
            self.demo.number,
            4.0,
        )
        .ok();
    }

    pub(super) fn update_number_scrub(&mut self) -> bool {
        let Some(mut scrub) = self.number_scrub else {
            return false;
        };
        let next = number_scrub_value(
            &mut scrub,
            STANDALONE_WINDOW,
            self.cursor.x,
            self.demo.number,
        );
        self.number_scrub = Some(scrub);
        let Some(value) = next else {
            return false;
        };
        self.demo.number = value;
        self.reset_number_draft();
        true
    }

    pub(super) fn finish_number_scrub(&mut self) -> bool {
        self.number_scrub.take().is_some_and(|mut scrub| {
            matches!(
                scrub.finish(STANDALONE_WINDOW, &self.demo.number, true),
                Some(PointerEditOutcome::Commit { .. } | PointerEditOutcome::Unchanged)
            )
        })
    }

    pub(super) fn cancel_number_scrub(&mut self) -> bool {
        let Some(mut scrub) = self.number_scrub.take() else {
            return false;
        };
        let Some(PointerEditOutcome::Cancel { restore }) =
            scrub.finish(STANDALONE_WINDOW, &self.demo.number, false)
        else {
            return false;
        };
        self.demo.number = restore;
        self.reset_number_draft();
        retire_numeric_capture(&mut self.input_ownership, scrub.target());
        true
    }

    pub(super) fn begin_vector_scrub(&mut self, axis: u8) {
        self.vector_scrub = self.demo.vector.get(usize::from(axis)).and_then(|value| {
            Some(VectorScrub {
                axis,
                session: HorizontalEditSession::new(
                    InputTarget {
                        window: STANDALONE_WINDOW,
                        element: self
                            .scopes
                            .gallery
                            .element(VECTOR_VALUE_BASE + u32::from(axis)),
                    },
                    self.cursor.x,
                    *value,
                    4.0,
                )
                .ok()?,
            })
        });
    }

    pub(super) fn update_vector_scrub(&mut self) -> bool {
        let Some(mut scrub) = self.vector_scrub else {
            return false;
        };
        let current = self.demo.vector[usize::from(scrub.axis)];
        let next = vector_scrub_value(&mut scrub, STANDALONE_WINDOW, self.cursor.x, current);
        self.vector_scrub = Some(scrub);
        let Some(value) = next else {
            return false;
        };
        self.demo.vector[usize::from(scrub.axis)] = Some(value);
        self.reset_vector_draft(scrub.axis);
        true
    }

    pub(super) fn finish_vector_scrub(&mut self) -> bool {
        self.vector_scrub.take().is_some_and(|mut scrub| {
            let current = self.demo.vector[usize::from(scrub.axis)];
            matches!(
                scrub.finish(STANDALONE_WINDOW, &current, true),
                Some(PointerEditOutcome::Commit { .. } | PointerEditOutcome::Unchanged)
            )
        })
    }

    pub(super) fn cancel_vector_scrub(&mut self) -> bool {
        let Some(mut scrub) = self.vector_scrub.take() else {
            return false;
        };
        let current = self.demo.vector[usize::from(scrub.axis)];
        let Some(PointerEditOutcome::Cancel { restore }) =
            scrub.finish(STANDALONE_WINDOW, &current, false)
        else {
            return false;
        };
        self.demo.vector[usize::from(scrub.axis)] = restore;
        self.reset_vector_draft(scrub.axis);
        if scrub.initial().is_none()
            && self.focused_text_field() == Some(TextFieldKind::Vector(scrub.axis))
        {
            let _ = self.vector_states[usize::from(scrub.axis)].reset(&mut self.text_context, "");
        }
        retire_numeric_capture(&mut self.input_ownership, scrub.target());
        true
    }
}
