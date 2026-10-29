//! Field-independent decisions for transferring a shared editor between windows.

use crate::UiWindowId;

/// A host-owned typed editor identity and the window that currently owns its caret.
/// Content, drafts and platform IME objects are deliberately not part of this record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowTextSession<K> {
    pub window: UiWindowId,
    pub field: K,
}

/// The same field may move between windows without changing its content or draft.
/// Its previous IME session still has to end before the new window can accept composition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSessionTransition<K> {
    previous: Option<WindowTextSession<K>>,
    next: Option<WindowTextSession<K>>,
}

impl<K: Copy + Eq> TextSessionTransition<K> {
    #[must_use]
    pub const fn new(
        previous: Option<WindowTextSession<K>>,
        next: Option<WindowTextSession<K>>,
    ) -> Self {
        Self { previous, next }
    }

    #[must_use]
    pub fn previous_field(self) -> Option<K> {
        self.previous.map(|session| session.field)
    }

    #[must_use]
    pub fn next_field(self) -> Option<K> {
        self.next.map(|session| session.field)
    }

    /// Hosts use this decision to publish the old field draft and initialize the new one.
    #[must_use]
    pub fn fields_changed(self) -> bool {
        self.previous_field() != self.next_field()
    }

    /// Includes owner-only transfers and excludes duplicate focus.
    /// The host rejects a foreign window's blur before requesting a transition.
    #[must_use]
    pub fn ime_owner_to_end(self) -> Option<UiWindowId> {
        self.previous
            .filter(|_| self.previous != self.next)
            .map(|session| session.window)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_field_window_transfer_ends_only_the_old_ime_without_reinitializing_the_draft() {
        let first = WindowTextSession {
            window: UiWindowId(1),
            field: 10_u32,
        };
        let second = WindowTextSession {
            window: UiWindowId(2),
            ..first
        };
        let transfer = TextSessionTransition::new(Some(first), Some(second));
        assert!(!transfer.fields_changed());
        assert_eq!(transfer.ime_owner_to_end(), Some(first.window));
        assert_eq!(
            TextSessionTransition::new(Some(second), Some(second)).ime_owner_to_end(),
            None
        );
        let leave = TextSessionTransition::new(Some(second), None);
        assert!(leave.fields_changed());
        assert_eq!(leave.ime_owner_to_end(), Some(second.window));
        let enter = TextSessionTransition::new(None, Some(first));
        assert!(enter.fields_changed());
        assert_eq!(enter.ime_owner_to_end(), None);

        // The same typed editor's geometry must follow its window, never a reused
        // display label or a previous frame's hidden field.
        use crate::{
            ElementId, InputTarget, Point, Rect, TextInputViewError, TextInputViewport,
            TextInputViews,
        };
        let element = ElementId::new(20);
        let other = ElementId::new(21);
        let view = TextInputViewport {
            bounds: Rect::new(10.0, 20.0, 100.0, 24.0),
            clip: Rect::new(10.0, 20.0, 100.0, 24.0),
            origin: Point::new(10.0, 20.0),
            caret: None,
            scroll_x: 0.0,
            scroll_y: 0.0,
        };
        let geometry = [(element, view)];
        let views = TextInputViews::from_layout(first.window, &geometry, |_| Some(first.field))
            .expect("unique geometry");
        assert_eq!(views.get(first.field), Some(view));
        assert_eq!(
            views.field_at(InputTarget {
                window: first.window,
                element
            }),
            Some(first.field)
        );
        assert_eq!(
            views.field_at(InputTarget {
                window: second.window,
                element
            }),
            None
        );
        let hidden = TextInputViews::from_layout(first.window, &[], |_| Some(first.field))
            .expect("empty frame");
        assert_eq!(hidden.get(first.field), None);
        assert_eq!(
            TextInputViews::from_layout(first.window, &[(element, view), (element, view)], |_| {
                Some(first.field)
            })
            .err(),
            Some(TextInputViewError::DuplicateElement(element)),
        );
        assert_eq!(
            TextInputViews::from_layout(first.window, &[(element, view), (other, view)], |_| Some(
                first.field
            ))
            .err(),
            Some(TextInputViewError::DuplicateField {
                first: element,
                second: other
            }),
        );
    }
}
