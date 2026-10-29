//! Determinate values or explicit host-driven indeterminate phase.

use crate::{WidgetIdentity, WidgetTheme, progress_bar};
use vesta_gpui::{Element, ElementKind, Semantic, SemanticRole};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProgressState {
    Determinate(f32),
    /// A finite cyclic phase; the host decides when visible work redraws.
    Indeterminate {
        phase: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressStateError {
    NonFiniteValue,
    NonFinitePhase,
}

pub fn progress_bar_state(
    root: WidgetIdentity,
    fill: WidgetIdentity,
    label: WidgetIdentity,
    state: ProgressState,
    theme: WidgetTheme,
) -> Result<Element, ProgressStateError> {
    let (value, phase) = match state {
        ProgressState::Determinate(value) if value.is_finite() => (value.clamp(0.0, 1.0), None),
        ProgressState::Determinate(_) => return Err(ProgressStateError::NonFiniteValue),
        ProgressState::Indeterminate { phase } if phase.is_finite() => {
            (1.0, Some(phase.rem_euclid(1.0)))
        }
        ProgressState::Indeterminate { .. } => return Err(ProgressStateError::NonFinitePhase),
    };
    let fill_id = fill.element;
    let label_id = label.element;
    let mut element = progress_bar(root, fill, label, value, theme);
    if let Some(phase) = phase {
        element.semantic =
            Some(Semantic::new(SemanticRole::ProgressIndicator, "Working").busy(true));
        if let Some(fill) = element
            .children
            .iter_mut()
            .find(|child| child.id == fill_id)
            && let Some(color) = &mut fill.style.visual.background
        {
            color.alpha *= 0.35 + 0.65 * (phase * std::f32::consts::TAU).sin().mul_add(0.5, 0.5);
        }
        if let Some(label) = element
            .children
            .iter_mut()
            .find(|child| child.id == label_id)
        {
            label.kind = ElementKind::Text("Working".into());
        }
    } else if let Some(semantic) = &mut element.semantic {
        *semantic = semantic
            .clone()
            .numeric(f64::from(value) * 100.0, 0.0, 100.0, None);
    }
    Ok(element)
}
