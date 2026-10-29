//! Static keyboard focus feedback; widgets can explicitly own their editor feedback.

use crate::{Color, Element, ElementId, FocusIndicator, PaintCommand, Rect, SemanticRole};

pub(super) fn append_indicator(
    element: &Element,
    bounds: Rect,
    focus: Option<(ElementId, Color)>,
    commands: &mut Vec<PaintCommand>,
) {
    let Some((focused, color)) = focus else {
        return;
    };
    if focused != element.id
        || !element.style.focusable
        || element.style.focus_indicator != FocusIndicator::Outline
    {
        return;
    }
    let rotation = element.style.rotation.unwrap_or(0.0);
    if element.semantic.as_ref().map(|semantic| semantic.role) == Some(SemanticRole::Splitter) {
        let thickness = 2.0_f32.min(bounds.width.min(bounds.height));
        let rect = if bounds.width <= bounds.height {
            Rect::new(
                bounds.x + (bounds.width - thickness) * 0.5,
                bounds.y,
                thickness,
                bounds.height,
            )
        } else {
            Rect::new(
                bounds.x,
                bounds.y + (bounds.height - thickness) * 0.5,
                bounds.width,
                thickness,
            )
        };
        commands.push(PaintCommand::FillRoundedRect {
            element: element.id,
            rect,
            radius: 0.0,
            rotation,
            color,
        });
    } else {
        commands.push(PaintCommand::StrokeRoundedRect {
            element: element.id,
            rect: bounds,
            radius: element.style.visual.corner_radius.max(2.0),
            width: 2.0,
            rotation,
            color,
        });
    }
}
