//! Engine-independent workspace state shared by Vesta authoring hosts.
//!
//! The workspace is a recursive split tree of panes. Every pane, item, split and floating
//! window has a stable identity that never depends on display text or array position.
//! Dock operations, pane resizing and empty-pane reclamation are deterministic mutations
//! that hosts can unit test without any renderer.
//!
//! Restore persisted layouts through the versioned session boundary. A raw
//! diagnostic dump cannot be decoded into an unvalidated live model:
//! ```compile_fail
//! let _: vesta_workspace::WorkspaceModel = serde_json::from_str("{}").unwrap();
//! ```
//! Floating topology and identities are changed only by model operations:
//! ```compile_fail
//! use vesta_workspace::{WorkspaceModel, WorkspaceNode, FloatingWindowId, PaneId};
//! fn replace_tree(model: &mut WorkspaceModel) {
//!     model.floating_window_mut(FloatingWindowId(1)).unwrap().root =
//!         WorkspaceNode::leaf(PaneId(99));
//! }
//! ```

#![forbid(unsafe_code)]

mod model;
mod types;

pub use model::{
    RestoredWorkspace, UnavailablePanel, WorkspaceError, WorkspaceModel, WorkspaceSession,
    WorkspaceSessionError,
};
pub use types::*;

#[cfg(test)]
mod tests {
    include!("workspace_tests.rs");
}
