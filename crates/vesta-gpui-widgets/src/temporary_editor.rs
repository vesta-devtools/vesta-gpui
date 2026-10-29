//! A window-owned temporary editor borrowing another field's state.
use crate::{PopupCloseReason, PopupDismissal};
use vesta_gpui::{InputTarget, UiWindowId};

#[derive(Clone, Debug)]
pub struct TemporaryEditorState<T> {
    session: Option<(UiWindowId, Option<InputTarget>, T)>,
}
impl<T> Default for TemporaryEditorState<T> {
    fn default() -> Self {
        Self { session: None }
    }
}
pub struct TemporaryEditorExit<T> {
    pub saved: T,
    pub commit: bool,
    pub dismissal: PopupDismissal,
}
/// The suspended value has already been restored; only this temporary draft is returned.
pub struct TemporaryEditorExchange<T> {
    pub draft: T,
    pub commit: bool,
    pub dismissal: PopupDismissal,
}
impl<T> TemporaryEditorState<T> {
    fn can_open(&self, window: UiWindowId, opener: Option<InputTarget>) -> bool {
        self.session.is_none() && opener.is_none_or(|target| target.window == window)
    }
    pub fn is_open(&self) -> bool {
        self.session.is_some()
    }
    pub fn opener(&self) -> Option<InputTarget> {
        self.session.as_ref().and_then(|(_, opener, _)| *opener)
    }
    pub fn owner(&self) -> Option<UiWindowId> {
        self.session.as_ref().map(|(window, _, _)| *window)
    }
    /// Refuses replacement of a live session so its saved borrowed state cannot be lost.
    pub fn open_in(&mut self, window: UiWindowId, opener: Option<InputTarget>, saved: T) -> bool {
        if !self.can_open(window, opener) {
            return false;
        }
        self.session = Some((window, opener, saved));
        true
    }
    /// Moves the whole slot into suspension. A rejected open leaves the slot untouched.
    /// The replacement is disposable on refusal; no clone or text reconstruction occurs.
    pub fn exchange_in(
        &mut self,
        window: UiWindowId,
        opener: Option<InputTarget>,
        slot: &mut T,
        replacement: T,
    ) -> bool {
        if !self.can_open(window, opener) {
            return false;
        }
        self.session = Some((window, opener, std::mem::replace(slot, replacement)));
        true
    }

    /// Restores the exact suspended value once, returning the displaced draft to its host.
    /// The host ends its matching native text/IME session before restoring an editor slot.
    pub fn finish_into(
        &mut self,
        source: UiWindowId,
        reason: PopupCloseReason,
        slot: &mut T,
    ) -> Option<TemporaryEditorExchange<T>> {
        let exit = self.finish(source, reason)?;
        Some(TemporaryEditorExchange {
            draft: std::mem::replace(slot, exit.saved),
            commit: exit.commit,
            dismissal: exit.dismissal,
        })
    }
    pub fn finish(
        &mut self,
        source: UiWindowId,
        reason: PopupCloseReason,
    ) -> Option<TemporaryEditorExit<T>> {
        if self.owner() != Some(source) {
            return None;
        }
        let (_, opener, saved) = self.session.take()?;
        Some(TemporaryEditorExit {
            saved,
            commit: reason == PopupCloseReason::Confirm,
            dismissal: PopupDismissal {
                source_window: source,
                reason,
                opener,
            },
        })
    }
}
