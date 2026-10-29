//! Optional command styling and busy state over the ordinary button projection.

use crate::{ControlState, Widget, WidgetIdentity, WidgetTheme, button, theme::caption_on_fill};
use vesta_gpui::ElementKind;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonTone {
    #[default]
    Normal,
    Primary,
    Danger,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonPresentation {
    pub tone: ButtonTone,
    pub loading: bool,
}

/// Loading decorates the label, retains its accessible name/identity and rejects
/// activation and Tab.
/// The caller owns the work, timing and cancellation; no worker is created here.
#[must_use]
pub fn button_with_presentation<A>(
    root: WidgetIdentity,
    label: WidgetIdentity,
    text: impl Into<String>,
    action: A,
    state: ControlState,
    presentation: ButtonPresentation,
    theme: WidgetTheme,
) -> Widget<A> {
    let effective = if presentation.loading {
        ControlState::Disabled
    } else {
        state
    };
    let mut widget = button(root, label, text, action, effective, theme);
    if presentation.loading
        && let Some(label) = widget.element.children.first_mut()
        && let ElementKind::Text(text) = &mut label.kind
    {
        text.insert_str(0, "◐ ");
    }
    if effective != ControlState::Disabled {
        let color = match presentation.tone {
            ButtonTone::Normal => theme.palette.control_background(state),
            ButtonTone::Primary if state == ControlState::Hovered => theme.palette.accent_hovered,
            ButtonTone::Primary => theme.palette.accent,
            ButtonTone::Danger => theme.palette.danger,
        };
        widget.element.style.visual.background = Some(color);
        if presentation.tone != ButtonTone::Normal
            && let Some(label) = widget.element.children.first_mut()
        {
            label.style.visual.text_color = caption_on_fill(color);
        }
    }
    if let Some(semantic) = &mut widget.element.semantic {
        semantic.busy = presentation.loading;
    }
    widget
}
