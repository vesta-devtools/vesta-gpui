//! Existing popup closure and live focus decisions. ADR-0331.
use vesta_gpui::{FocusScope, InputTarget, UiWindowId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupCloseReason {
    Explicit,
    Confirm,
    TraverseFocus,
    OutsidePress,
    FocusTransfer,
    OwnerRemoved,
    WindowUnavailable,
}

/// An accepted, once-only close intent. Hosts retain input/IME and business state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PopupDismissal {
    pub source_window: UiWindowId,
    pub reason: PopupCloseReason,
    pub opener: Option<InputTarget>,
}

impl PopupDismissal {
    /// A deferred request must still be checked against the rebuilt FocusScope.
    #[must_use]
    pub fn focus_request(self) -> Option<InputTarget> {
        if !matches!(
            self.reason,
            PopupCloseReason::Explicit
                | PopupCloseReason::Confirm
                | PopupCloseReason::TraverseFocus
        ) {
            return None;
        }
        self.opener
            .filter(|opener| opener.window == self.source_window)
    }
    /// Use the current tree after closure. Outside presses keep their original
    /// hit/focus/action; deleted, disabled or modal-excluded openers cannot return.
    #[must_use]
    pub fn restore_target<A>(self, scope: &FocusScope<'_, A>) -> Option<InputTarget> {
        self.focus_request()
            .and_then(|opener| scope.restore_target(opener))
    }
}
