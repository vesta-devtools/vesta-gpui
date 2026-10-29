//! Ordered, host-executed lifecycle effects. Values/IME/platform objects stay in the host.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionCleanupStage {
    CancelPreviews,
    EndTextSession,
    DismissTransient,
    ClearOwnership,
}

/// Restored values/drafts must precede text exit, which may publish a draft.
/// The caller applies each effect to the affected window only and chooses the close cause.
pub fn finish_window_interaction(mut apply: impl FnMut(InteractionCleanupStage)) {
    for stage in [
        InteractionCleanupStage::CancelPreviews,
        InteractionCleanupStage::EndTextSession,
        InteractionCleanupStage::DismissTransient,
        InteractionCleanupStage::ClearOwnership,
    ] {
        apply(stage);
    }
}
