//! Transient input ownership. Hosts supply window identities and visible focus order.
use crate::ElementId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UiWindowId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InputTarget {
    pub window: UiWindowId,
    pub element: ElementId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointerCapture {
    pub target: InputTarget,
    pub button: PointerButton,
}

/// Deliberate input used to present focus; ownership and selection stay unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FocusInput {
    Pointer,
    /// Also used by accepted accessibility navigation requests.
    #[default]
    Keyboard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FocusRequest {
    Target(InputTarget),
    Traverse {
        window: UiWindowId,
        anchor: Option<InputTarget>,
        backwards: bool,
    },
}

#[derive(Clone, Debug, Default)]
pub struct InputOwnership {
    focus: Option<InputTarget>,
    focus_input: FocusInput,
    capture: Option<PointerCapture>,
    requested_focus: Option<FocusRequest>,
}

impl InputOwnership {
    pub fn focus(&self) -> Option<InputTarget> {
        self.focus
    }

    /// Generic outline target. Editors retain their own border/caret feedback.
    pub fn visible_focus(&self) -> Option<InputTarget> {
        (self.focus_input == FocusInput::Keyboard)
            .then_some(self.focus)
            .flatten()
    }

    /// Returns whether the current outline needs repainting, without changing focus,
    /// capture or a pending restore. Pointer motion alone must not call this.
    pub fn observe_focus_input(&mut self, input: FocusInput) -> bool {
        let before = self.visible_focus();
        self.focus_input = input;
        before != self.visible_focus()
    }

    pub fn set_focus(&mut self, target: Option<InputTarget>) -> bool {
        self.requested_focus = None;
        let changed = self.focus != target;
        if changed {
            self.capture = None;
            self.focus = target;
        }
        changed
    }

    /// Stage a stable target until the next authoritative action tree is available.
    /// The host ends the old text session before requesting a restore, and applies
    /// the result from `FocusScope::resolve_request` through its normal focus adapter.
    pub fn request_focus(&mut self, target: InputTarget) {
        self.requested_focus = Some(FocusRequest::Target(target));
        self.capture = None;
    }

    /// A source-qualified hint for bounded projection before the next tree is built.
    /// This does not consume or admit the request; `FocusScope` still validates it.
    pub fn pending_focus_target(&self, window: UiWindowId) -> Option<InputTarget> {
        match self.requested_focus {
            Some(FocusRequest::Target(target)) if target.window == window => Some(target),
            _ => None,
        }
    }

    /// Stage one keyboard step for the next authoritative tree, for example
    /// after a menu closes and its temporary tab stops disappear.
    pub fn request_focus_traversal(&mut self, window: UiWindowId, backwards: bool) {
        self.observe_focus_input(FocusInput::Keyboard);
        self.requested_focus = Some(FocusRequest::Traverse {
            window,
            anchor: self.focus.filter(|target| target.window == window),
            backwards,
        });
        self.capture = None;
    }

    pub(crate) fn take_focus_request(&mut self) -> Option<FocusRequest> {
        self.requested_focus.take()
    }

    /// Cancel deferred focus work when the window loses activation or is removed.
    pub fn cancel_focus_request(&mut self) -> bool {
        self.requested_focus.take().is_some()
    }

    /// The host supplies only live, enabled controls, in visual navigation order.
    pub fn traverse(&mut self, order: &[InputTarget], backwards: bool) -> bool {
        let presentation_changed = self.observe_focus_input(FocusInput::Keyboard);
        let next = Self::traversal_target(order, self.focus, backwards);
        self.set_focus(next) || presentation_changed
    }

    pub(crate) fn traversal_target(
        order: &[InputTarget],
        anchor: Option<InputTarget>,
        backwards: bool,
    ) -> Option<InputTarget> {
        if order.is_empty() {
            return None;
        }
        let current = anchor.and_then(|focus| order.iter().position(|t| *t == focus));
        let next = match (current, backwards) {
            (Some(index), false) => (index + 1) % order.len(),
            (Some(index), true) => (index + order.len() - 1) % order.len(),
            (None, false) => 0,
            (None, true) => order.len() - 1,
        };
        Some(order[next])
    }

    /// Drop focus and pointer capture when a rebuilt UI tree no longer
    /// contains the focused, enabled control.
    pub fn retain_live_focus(&mut self, order: &[InputTarget]) -> bool {
        if self.focus.is_some_and(|focus| !order.contains(&focus)) {
            self.set_focus(None)
        } else {
            false
        }
    }

    pub fn capture(&self) -> Option<PointerCapture> {
        self.capture
    }

    /// Drop a capture whose stable target is no longer live and enabled after a rebuild.
    pub fn retain_live_capture(&mut self, mut is_live: impl FnMut(InputTarget) -> bool) -> bool {
        if self.capture.is_some_and(|capture| !is_live(capture.target)) {
            self.capture = None;
            true
        } else {
            false
        }
    }

    pub fn capture_pointer(&mut self, target: InputTarget, button: PointerButton) {
        self.requested_focus = None;
        self.capture = Some(PointerCapture { target, button });
    }

    /// A different window or button cannot release this gesture.
    pub fn release_pointer(&mut self, window: UiWindowId, button: PointerButton) -> bool {
        if self
            .capture
            .is_some_and(|c| c.target.window == window && c.button == button)
        {
            self.capture = None;
            true
        } else {
            false
        }
    }

    pub fn cancel_capture(&mut self) -> bool {
        self.capture.take().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_and_capture_keep_window_control_and_button_ownership() {
        let field = InputTarget {
            window: UiWindowId(1),
            element: ElementId::new(10),
        };
        let send = InputTarget {
            element: ElementId::new(11),
            ..field
        };
        let mut input = InputOwnership::default();
        input.traverse(&[field, send], false);
        input.capture_pointer(field, PointerButton::Primary);
        assert_eq!(input.visible_focus(), Some(field));
        assert!(input.observe_focus_input(FocusInput::Pointer));
        assert_eq!(input.visible_focus(), None);
        assert_eq!(input.focus(), Some(field));
        assert_eq!(input.capture().map(|capture| capture.target), Some(field));
        assert!(!input.observe_focus_input(FocusInput::Pointer));
        // Traversal must repaint even when a single tab stop keeps logical focus.
        assert!(input.traverse(&[field], false));
        assert_eq!(input.visible_focus(), Some(field));
        assert_eq!(input.capture().map(|capture| capture.target), Some(field));
        assert!(!input.release_pointer(UiWindowId(2), PointerButton::Primary));
        assert!(!input.release_pointer(field.window, PointerButton::Secondary));
        assert_eq!(input.capture().map(|c| c.target), Some(field));
        input.traverse(&[field, send], true);
        assert_eq!(input.focus(), Some(send));
        assert_eq!(input.capture(), None);
        input.traverse(&[field], false);
        assert_eq!(input.focus(), Some(field));
        input.capture_pointer(field, PointerButton::Primary);
        assert!(input.release_pointer(field.window, PointerButton::Primary));
        input.traverse(&[], false);
        assert_eq!(input.focus(), None);
        assert_eq!(input.visible_focus(), None);
        input.set_focus(Some(field));
        input.request_focus(send);
        input.observe_focus_input(FocusInput::Pointer);
        assert_eq!(input.take_focus_request(), Some(FocusRequest::Target(send)));
        assert_eq!(input.focus(), Some(field));

        let mut other_window = InputOwnership::default();
        other_window.set_focus(Some(InputTarget {
            window: UiWindowId(2),
            ..field
        }));
        assert_eq!(other_window.visible_focus(), other_window.focus());
        assert_eq!(input.visible_focus(), None);
    }

    #[test]
    fn removed_focus_target_releases_its_capture() {
        let field = InputTarget {
            window: UiWindowId(1),
            element: ElementId::new(7),
        };
        let mut input = InputOwnership::default();
        input.set_focus(Some(field));
        input.capture_pointer(field, PointerButton::Primary);
        assert!(!input.retain_live_focus(&[field]));
        assert!(input.retain_live_focus(&[]));
        assert_eq!(input.focus(), None);
        assert_eq!(input.capture(), None);
        let scrollbar = InputTarget {
            element: ElementId::new(8),
            ..field
        };
        input.capture_pointer(scrollbar, PointerButton::Primary);
        assert!(!input.retain_live_capture(|target| target == scrollbar));
        assert!(input.retain_live_capture(|_| false));
        assert_eq!(input.capture(), None);
    }
}
