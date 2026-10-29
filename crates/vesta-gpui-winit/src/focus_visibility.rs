//! Shared platform press admission and focus presentation.

use crate::{UiInputEvent as WindowEvent, UiInputEventSource};
use vesta_gpui::{FocusInput, InputOwnership};
use winit::{
    event::{ElementState, TouchPhase},
    keyboard::{Key, NamedKey},
};

/// Admit a pressed event rather than winit's focus-generated key replay.
/// Read both fields from the same WindowEvent::KeyboardInput before invoking
/// logical-key actions or changing interaction presentation. Repeats are unchanged.
#[must_use]
pub fn is_user_key_press(state: ElementState, is_synthetic: bool) -> bool {
    state == ElementState::Pressed && !is_synthetic
}

/// Observe before a host's text/menu/control handlers consume an event. Only a
/// deliberate press changes focus presentation; motion, release and activation
/// preserve it. The return value requests repainting an existing focus outline.
pub fn observe_focus_visibility(
    event: &impl UiInputEventSource,
    input: &mut InputOwnership,
) -> bool {
    let event = event.ui_input_event();
    let method = match event.as_ref() {
        WindowEvent::MouseInput {
            state: ElementState::Pressed,
            ..
        } => Some(FocusInput::Pointer),
        WindowEvent::Touch(touch) if touch.phase == TouchPhase::Started => {
            Some(FocusInput::Pointer)
        }
        WindowEvent::KeyboardInput {
            event,
            is_synthetic,
            ..
        } => keyboard_focus_input(&event.logical_key, event.state, *is_synthetic),
        _ => None,
    };
    method.is_some_and(|method| input.observe_focus_input(method))
}

fn keyboard_focus_input(key: &Key, state: ElementState, is_synthetic: bool) -> Option<FocusInput> {
    if !is_user_key_press(state, is_synthetic) {
        return None;
    }
    (!matches!(
        key,
        Key::Named(
            NamedKey::Shift
                | NamedKey::Control
                | NamedKey::Alt
                | NamedKey::AltGraph
                | NamedKey::Super
                | NamedKey::Meta
        )
    ))
    .then_some(FocusInput::Keyboard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vesta_gpui::{ElementId, InputTarget, PointerButton, UiWindowId};
    use winit::{
        dpi::PhysicalPosition,
        event::{DeviceId, MouseButton},
    };

    #[test]
    fn deliberate_input_changes_only_focus_presentation() {
        let target = InputTarget {
            window: UiWindowId(1),
            element: ElementId::new(7),
        };
        let mut input = InputOwnership::default();
        input.set_focus(Some(target));
        input.capture_pointer(target, PointerButton::Primary);
        let mouse = |state| WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state,
            button: MouseButton::Left,
        };
        assert!(observe_focus_visibility(
            &mouse(ElementState::Pressed),
            &mut input
        ));
        assert_eq!(input.visible_focus(), None);
        assert_eq!(input.focus(), Some(target));
        assert_eq!(input.capture().map(|capture| capture.target), Some(target));
        assert!(!observe_focus_visibility(
            &mouse(ElementState::Released),
            &mut input
        ));
        for modifier in [
            NamedKey::Shift,
            NamedKey::Control,
            NamedKey::Alt,
            NamedKey::Super,
        ] {
            assert_eq!(
                keyboard_focus_input(&Key::Named(modifier), ElementState::Pressed, false),
                None
            );
        }
        for (state, synthetic) in [
            (ElementState::Pressed, true),
            (ElementState::Released, true),
            (ElementState::Released, false),
        ] {
            assert!(
                !keyboard_focus_input(&Key::Named(NamedKey::Tab), state, synthetic)
                    .is_some_and(|method| input.observe_focus_input(method))
            );
            assert_eq!(
                input.visible_focus(),
                None,
                "focus replay/release is not deliberate navigation"
            );
            assert_eq!(input.focus(), Some(target));
            assert_eq!(input.capture().map(|capture| capture.target), Some(target));
        }
        input.observe_focus_input(
            keyboard_focus_input(&Key::Named(NamedKey::Tab), ElementState::Pressed, false)
                .expect("navigation"),
        );
        assert_eq!(input.visible_focus(), Some(target));
        assert!(!observe_focus_visibility(
            &WindowEvent::CursorMoved {
                device_id: DeviceId::dummy(),
                position: PhysicalPosition::new(10.0, 20.0)
            },
            &mut input
        ));
        assert!(!observe_focus_visibility(
            &WindowEvent::Focused(true),
            &mut input
        ));
        assert!(!observe_focus_visibility(
            &mouse(ElementState::Released),
            &mut input
        ));
        assert_eq!(input.visible_focus(), Some(target));
        assert_eq!(input.capture().map(|capture| capture.target), Some(target));
    }
}
