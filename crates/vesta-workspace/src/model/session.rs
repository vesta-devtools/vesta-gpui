//! Versioned untrusted snapshot; only restore creates a live model.
use super::{WorkspaceError, WorkspaceModel, WorkspaceState, validation};
use crate::types::{ItemId, PanelTypeId};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

const SESSION_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSession {
    version: u32,
    workspace: WorkspaceState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RestoredWorkspace {
    pub workspace: WorkspaceModel,
    pub unavailable_panels: Vec<UnavailablePanel>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnavailablePanel {
    pub item: ItemId,
    pub panel_type: PanelTypeId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceSessionError {
    Json(String),
    UnsupportedVersion(u32),
    InvalidWorkspace(WorkspaceError),
}
impl fmt::Display for WorkspaceSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "workspace session JSON: {error}"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported workspace session version {version}")
            }
            Self::InvalidWorkspace(error) => {
                write!(formatter, "invalid workspace session: {error}")
            }
        }
    }
}
impl Error for WorkspaceSessionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidWorkspace(error) => Some(error),
            _ => None,
        }
    }
}
impl WorkspaceSession {
    pub fn capture(workspace: &WorkspaceModel) -> Result<Self, WorkspaceSessionError> {
        validation::validate(&workspace.state).map_err(WorkspaceSessionError::InvalidWorkspace)?;
        Ok(Self {
            version: SESSION_VERSION,
            workspace: workspace.state.clone(),
        })
    }
    pub fn to_json(&self) -> Result<String, WorkspaceSessionError> {
        serde_json::to_string_pretty(self)
            .map_err(|error| WorkspaceSessionError::Json(error.to_string()))
    }
    /// Decode an envelope, not a live model. Call restore to validate it.
    pub fn from_json(json: &str) -> Result<Self, WorkspaceSessionError> {
        serde_json::from_str(json).map_err(|error| WorkspaceSessionError::Json(error.to_string()))
    }
    /// Validate saved state and report missing panels without losing their identities.
    pub fn restore(
        self,
        mut panel_available: impl FnMut(&PanelTypeId) -> bool,
    ) -> Result<RestoredWorkspace, WorkspaceSessionError> {
        if self.version != SESSION_VERSION {
            return Err(WorkspaceSessionError::UnsupportedVersion(self.version));
        }
        validation::validate(&self.workspace).map_err(WorkspaceSessionError::InvalidWorkspace)?;
        let pane_ids = self.workspace.root.panes().into_iter().chain(
            self.workspace
                .floating
                .iter()
                .flat_map(|window| window.root.panes()),
        );
        let unavailable_panels = pane_ids
            .filter_map(|pane| self.workspace.panes.get(&pane))
            .flat_map(|pane| &pane.items)
            .filter(|item| !panel_available(&item.panel_type))
            .map(|item| UnavailablePanel {
                item: item.id,
                panel_type: item.panel_type.clone(),
            })
            .collect();
        Ok(RestoredWorkspace {
            workspace: WorkspaceModel {
                state: self.workspace,
            },
            unavailable_panels,
        })
    }
}
