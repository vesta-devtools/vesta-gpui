//! Engine-independent workspace state shared by Vesta authoring hosts.
//!
//! The workspace is a recursive split tree of panes. Every pane, item, split and floating
//! window has a stable identity that never depends on display text or array position.
//! Dock operations, pane resizing and empty-pane reclamation are deterministic mutations
//! that hosts can unit test without any renderer.

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
