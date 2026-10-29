//! Per-window click sequences; selection and capture stay in Core.

use crate::{TextInputBridge, UiInputEvent as WindowEvent, UiInputEventSource};
use std::time::{Duration, Instant};
use vesta_gpui::{InputTarget, Point};
use winit::event::{ElementState, Ime, MouseButton};

const DOUBLE_CLICK_TIME: Duration = Duration::from_millis(450);
const CLICK_SLOP: f32 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextPointerAction {
    Caret,
    Extend,
    Word,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TextPointerPress {
    target: InputTarget,
    point: Point,
    at: Instant,
    action: TextPointerAction,
    dragged: bool,
}

fn near(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() <= CLICK_SLOP && (a.y - b.y).abs() <= CLICK_SLOP
}

impl TextInputBridge {
    /// Forget click timing after a programmatic caret move without ending IME.
    pub fn clear_text_pointer(&mut self) {
        self.text_pointer = None;
    }

    /// A primary text press in window-logical coordinates. The caller supplies
    /// monotonic time and routes only a live, enabled field in this bridge's window.
    /// The result is a mode for Core's point operation, not a focus/capture request.
    pub fn text_pointer_press(
        &mut self,
        target: InputTarget,
        point: Point,
        at: Instant,
        composing: bool,
    ) -> Option<TextPointerAction> {
        if composing || !point.x.is_finite() || !point.y.is_finite() {
            self.text_pointer = None;
            return None;
        }
        let action = if self.modifiers.shift_key() {
            TextPointerAction::Extend
        } else if self.text_pointer.is_some_and(|previous| {
            previous.target == target
                && previous.action == TextPointerAction::Caret
                && !previous.dragged
                && near(previous.point, point)
                && at
                    .checked_duration_since(previous.at)
                    .is_some_and(|elapsed| elapsed <= DOUBLE_CLICK_TIME)
        }) {
            TextPointerAction::Word
        } else {
            TextPointerAction::Caret
        };
        self.text_pointer = Some(TextPointerPress {
            target,
            point,
            at,
            action,
            dragged: false,
        });
        Some(action)
    }

    /// Called only for an existing matching primary capture. Tiny post-double-click
    /// motion must retain the word; once a drag starts it continues inside the slop.
    pub fn text_pointer_drag(&mut self, target: InputTarget, point: Point) -> bool {
        if !point.x.is_finite() || !point.y.is_finite() {
            self.text_pointer = None;
            return false;
        }
        let Some(press) = self.text_pointer.as_mut() else {
            return true;
        };
        if press.target != target {
            self.text_pointer = None;
            return false;
        }
        if !near(press.point, point) {
            press.dragged = true;
        }
        press.action != TextPointerAction::Word || press.dragged
    }

    /// Observe before host dispatch, including events consumed by another control.
    /// `text_hit` identifies a primary press routed to the host's actual text field.
    /// IME activation may occur between the two clicks and is deliberately preserved.
    pub fn observe_text_pointer_event(&mut self, event: &impl UiInputEventSource, text_hit: bool) {
        let event = event.ui_input_event();
        let clear = match event.as_ref() {
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } => *button != MouseButton::Left || !text_hit,
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => crate::is_user_key_press(event.state, *is_synthetic),
            WindowEvent::Ime(Ime::Preedit(value, _)) => !value.is_empty(),
            WindowEvent::Ime(Ime::Commit(_) | Ime::Disabled)
            | WindowEvent::Text(_)
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::CursorLeft { .. }
            | WindowEvent::Resized(_)
            | WindowEvent::ScaleFactorChanged { .. }
            | WindowEvent::Focused(false)
            | WindowEvent::Occluded(true)
            | WindowEvent::Destroyed => true,
            _ => false,
        };
        if clear {
            self.text_pointer = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vesta_gpui::{ElementId, UiWindowId};
    use winit::{
        dpi::PhysicalSize,
        event::{DeviceId, Modifiers},
        keyboard::ModifiersState,
    };

    #[test]
    fn click_pair_is_owned_bounded_and_invalidated_by_real_input_and_lifecycle() {
        let target = InputTarget {
            window: UiWindowId(1),
            element: ElementId::new(10),
        };
        let point = Point::new(12.0, 24.0);
        let start = Instant::now();
        let mut bridge = TextInputBridge::default();
        let press = |bridge: &mut TextInputBridge, target, at| {
            bridge.text_pointer_press(target, point, at, false)
        };
        assert_eq!(
            press(&mut bridge, target, start),
            Some(TextPointerAction::Caret)
        );
        bridge.observe_text_pointer_event(&WindowEvent::Ime(Ime::Enabled), true);
        bridge
            .observe_text_pointer_event(&WindowEvent::Ime(Ime::Preedit(String::new(), None)), true);
        assert_eq!(
            press(&mut bridge, target, start + Duration::from_millis(450)),
            Some(TextPointerAction::Word)
        );
        assert!(!bridge.text_pointer_drag(target, point));
        assert!(!bridge.text_pointer_drag(target, Point::new(15.0, 25.0)));
        assert!(bridge.text_pointer_drag(target, Point::new(18.0, 24.0)));
        assert!(bridge.text_pointer_drag(target, point));
        assert_eq!(
            press(&mut bridge, target, start + Duration::from_millis(451)),
            Some(TextPointerAction::Caret)
        );
        assert_eq!(
            press(&mut bridge, target, start),
            Some(TextPointerAction::Caret),
            "backward time"
        );
        assert_eq!(
            press(&mut bridge, target, start + Duration::from_millis(451)),
            Some(TextPointerAction::Caret),
            "expired pair"
        );
        for other in [
            InputTarget {
                window: UiWindowId(2),
                ..target
            },
            InputTarget {
                element: ElementId::new(11),
                ..target
            },
        ] {
            assert_eq!(
                press(&mut bridge, other, start + Duration::from_millis(452)),
                Some(TextPointerAction::Caret)
            );
        }
        for event in [
            WindowEvent::Focused(false),
            WindowEvent::Occluded(true),
            WindowEvent::Resized(PhysicalSize::new(400, 300)),
            WindowEvent::Ime(Ime::Preedit("候选".into(), None)),
            WindowEvent::Ime(Ime::Commit("你".into())),
            WindowEvent::Ime(Ime::Disabled),
        ] {
            bridge.reset();
            press(&mut bridge, target, start);
            bridge.observe_text_pointer_event(&event, true);
            assert_eq!(
                press(&mut bridge, target, start + Duration::from_millis(100)),
                Some(TextPointerAction::Caret),
                "{event:?}"
            );
        }
        for (button, text_hit) in [(MouseButton::Left, false), (MouseButton::Right, true)] {
            bridge.reset();
            press(&mut bridge, target, start);
            bridge.observe_text_pointer_event(
                &WindowEvent::MouseInput {
                    device_id: DeviceId::dummy(),
                    state: ElementState::Pressed,
                    button,
                },
                text_hit,
            );
            assert_eq!(
                press(&mut bridge, target, start + Duration::from_millis(100)),
                Some(TextPointerAction::Caret)
            );
        }
        bridge.update_modifiers(&Modifiers::from(ModifiersState::SHIFT));
        assert_eq!(
            press(&mut bridge, target, start),
            Some(TextPointerAction::Extend)
        );
        bridge.update_modifiers(&Modifiers::from(ModifiersState::empty()));
        assert_eq!(
            press(&mut bridge, target, start + Duration::from_millis(100)),
            Some(TextPointerAction::Caret)
        );
        assert_eq!(bridge.text_pointer_press(target, point, start, true), None);
        assert_eq!(
            press(&mut bridge, target, start),
            Some(TextPointerAction::Caret)
        );
        assert_eq!(
            bridge.text_pointer_press(target, Point::new(f32::NAN, 0.0), start, false),
            None
        );
        assert_eq!(
            press(&mut bridge, target, start),
            Some(TextPointerAction::Caret)
        );
        bridge.clear_ime_session();
        assert_eq!(
            press(&mut bridge, target, start + Duration::from_millis(100)),
            Some(TextPointerAction::Caret)
        );
        bridge.clear_text_pointer();
        press(&mut bridge, target, start);
        assert_eq!(
            press(&mut bridge, target, start + Duration::from_millis(100)),
            Some(TextPointerAction::Word)
        );
        assert_eq!(
            press(&mut bridge, target, start + Duration::from_millis(200)),
            Some(TextPointerAction::Caret),
            "pair consumed"
        );
        bridge.text_pointer_drag(target, Point::new(30.0, 24.0));
        assert_eq!(
            press(&mut bridge, target, start + Duration::from_millis(300)),
            Some(TextPointerAction::Caret),
            "drag cannot seed a pair"
        );
    }
}
