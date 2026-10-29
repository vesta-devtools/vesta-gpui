//! Lossless borrowing of the Search editor by existing temporary consumers.
use super::*;

impl App {
    /// Save/Cancel are inside the editing group; moving elsewhere cancels the draft.
    pub(super) fn cancel_inline_for_focus(&mut self, window: UiWindowId, next: Option<ElementId>) {
        if self.demo.inline_session.owner() != Some(window) {
            return;
        }
        let stays_inside = self.gallery_scopes(window).is_some_and(|scope| {
            next == Some(TextFieldKind::Search.element(scope))
                || next.is_some_and(|next| {
                    self.window_actions(window).is_some_and(|actions| {
                        actions.is_descendant_of(next, scope.element(INLINE_ROOT))
                    })
                })
        });
        if !stays_inside {
            self.finish_inline_edit_for(window, PopupCloseReason::FocusTransfer);
        }
    }

    /// End the prior native owner before moving its text state, rather than cancelling
    /// composition on the replacement editor during the subsequent focus transition.
    pub(super) fn end_borrowed_text_session(&mut self) {
        if let Some(session) = self.text_session {
            self.finish_window_text_session(session.window);
        }
    }

    pub(super) fn begin_inline_edit(&mut self) {
        if self.demo.inline_session.is_open()
            || self.demo.palette_session.is_open()
            || !self.demo.dialog_stack.is_empty()
        {
            return;
        }
        let Some(opener) = self.editor_target(INLINE_ROOT + 1) else {
            return;
        };
        let mut draft = TextInputState::default();
        if draft
            .reset(&mut self.text_context, &self.demo.inline_label)
            .is_err()
        {
            return;
        }
        self.end_borrowed_text_session();
        if self.demo.inline_session.exchange_in(
            opener.window,
            Some(opener),
            &mut self.search_state,
            draft,
        ) {
            self.begin_text_selection(opener.window, TextFieldKind::Search, None);
        }
    }

    pub(super) fn finish_inline_edit(&mut self, commit: bool) {
        if self.demo.inline_session.owner() != Some(self.action_window) {
            return;
        }
        self.finish_inline_edit_for(
            self.action_window,
            if commit {
                PopupCloseReason::Confirm
            } else {
                PopupCloseReason::Explicit
            },
        );
    }

    pub(super) fn finish_inline_edit_for(&mut self, owner: UiWindowId, reason: PopupCloseReason) {
        if self.demo.inline_session.owner() != Some(owner)
            || (reason == PopupCloseReason::Confirm
                && (self.search_state.is_composing() || self.search_state.text().trim().is_empty()))
        {
            return;
        }
        self.finish_window_text_session(owner);
        let Some(exit) =
            self.demo
                .inline_session
                .finish_into(owner, reason, &mut self.search_state)
        else {
            return;
        };
        if exit.commit {
            self.demo.inline_label = exit.draft.text().trim().to_owned();
        }
        if let Some(target) = exit.dismissal.focus_request() {
            self.restore_after_rebuild(owner, Some(target.element));
        }
    }

    pub(super) fn handle_inline_edit_key_for(&mut self, window: UiWindowId, key: &Key) -> bool {
        let Some(owner) = self.demo.inline_session.opener() else {
            return false;
        };
        let decision =
            shortcut_chord(key.as_ref(), self.window_modifiers(window)).and_then(|chord| {
                inline_edit_key_action(
                    owner,
                    window,
                    self.text_session_field(window) == Some(TextFieldKind::Search),
                    self.search_state.is_composing(),
                    chord,
                )
            });
        let Some(decision) = decision else {
            return false;
        };
        self.finish_inline_edit_for(
            window,
            if decision == InlineEditKeyAction::Commit {
                PopupCloseReason::Confirm
            } else {
                PopupCloseReason::Explicit
            },
        );
        true
    }
}

#[cfg(test)]
mod tests;
