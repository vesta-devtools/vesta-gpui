//! Requested platform IME state is distinct from received Ime::Enabled state.
use vesta_gpui::TextInputViewport;
use winit::{
    dpi::{LogicalPosition, LogicalSize},
    window::{Window, WindowId},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CursorArea {
    position: LogicalPosition<f64>,
    size: LogicalSize<f64>,
    scale_bits: u64,
}

impl CursorArea {
    pub(crate) fn from_view(view: Option<TextInputViewport>, scale: f64) -> Option<Self> {
        let caret = view?.caret?;
        Some(Self {
            position: LogicalPosition::new(f64::from(caret.x), f64::from(caret.bottom())),
            size: LogicalSize::new(
                f64::from(caret.width.max(1.0)),
                f64::from(caret.height.max(1.0)),
            ),
            scale_bits: scale.to_bits(),
        })
    }

    pub(crate) fn publish(self, window: &Window) {
        window.set_ime_cursor_area(self.position, self.size);
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ImeRequestState {
    window: Option<WindowId>,
    allowed: Option<bool>,
    cursor: Option<CursorArea>,
}

struct Update {
    allowed: Option<bool>,
    cursor: Option<CursorArea>,
}

impl ImeRequestState {
    pub(crate) fn invalidate(&mut self) {
        self.allowed = None;
        self.cursor = None;
    }

    pub(crate) fn invalidate_cursor(&mut self) {
        self.cursor = None;
    }

    pub(crate) fn sync(&mut self, window: &Window, enabled: bool, view: Option<TextInputViewport>) {
        let cursor = enabled
            .then(|| CursorArea::from_view(view, window.scale_factor()))
            .flatten();
        let update = self.request(window.id(), enabled, cursor);
        if let Some(allowed) = update.allowed {
            window.set_ime_allowed(allowed);
        }
        if let Some(cursor) = update.cursor {
            cursor.publish(window);
        }
    }

    fn request(&mut self, window: WindowId, allowed: bool, cursor: Option<CursorArea>) -> Update {
        if self.window != Some(window) {
            self.window = Some(window);
            self.invalidate();
        }
        let change_allowed = (self.allowed != Some(allowed)).then_some(allowed);
        self.allowed = Some(allowed);
        let cursor = allowed.then_some(cursor).flatten();
        let change_cursor = (self.cursor != cursor).then_some(cursor).flatten();
        self.cursor = cursor;
        Update {
            allowed: change_allowed,
            cursor: change_cursor,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settled_ime_requests_are_elided_and_session_or_geometry_changes_republish() {
        // Pure request planning: these synthetic IDs never address an OS window.
        let main = WindowId::from(17);
        let floating = WindowId::from(29);
        let area = CursorArea {
            position: LogicalPosition::new(10.0, 20.0),
            size: LogicalSize::new(1.0, 16.0),
            scale_bits: 1.0f64.to_bits(),
        };
        let mut state = ImeRequestState::default();
        let initial = state.request(main, true, Some(area));
        assert_eq!(initial.allowed, Some(true));
        assert_eq!(initial.cursor, Some(area));
        let settled = state.request(main, true, Some(area));
        assert_eq!(settled.allowed, None);
        assert_eq!(settled.cursor, None);

        let moved = CursorArea {
            position: LogicalPosition::new(18.0, 32.0),
            ..area
        };
        assert_eq!(state.request(main, true, Some(moved)).cursor, Some(moved));
        let scaled = CursorArea {
            scale_bits: 1.25f64.to_bits(),
            ..moved
        };
        assert_eq!(state.request(main, true, Some(scaled)).cursor, Some(scaled));
        state.invalidate_cursor(); // A newly enabled platform context needs placement.
        let reenabled = state.request(main, true, Some(scaled));
        assert_eq!(reenabled.allowed, None);
        assert_eq!(reenabled.cursor, Some(scaled));

        assert_eq!(state.request(main, true, None).cursor, None);
        assert_eq!(state.request(main, true, Some(scaled)).cursor, Some(scaled));
        let disabled = state.request(main, false, Some(scaled));
        assert_eq!(disabled.allowed, Some(false));
        assert_eq!(disabled.cursor, None);
        assert_eq!(state.request(main, false, None).allowed, None);
        state.invalidate(); // Explicit session end/reset or Ime::Disabled, not observed Enabled.
        let fresh = state.request(main, true, Some(area));
        assert_eq!(fresh.allowed, Some(true));
        assert_eq!(fresh.cursor, Some(area));
        let other = state.request(floating, true, Some(area));
        assert_eq!(other.allowed, Some(true));
        assert_eq!(other.cursor, Some(area));
        let returning = state.request(main, true, Some(area));
        assert_eq!(returning.allowed, Some(true));
        assert_eq!(returning.cursor, Some(area));
    }
}
