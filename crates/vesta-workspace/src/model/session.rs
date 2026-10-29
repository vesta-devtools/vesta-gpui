//! Versioned, validated persistence boundary for the workspace model.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::WorkspaceModel;
use crate::types::{FloatingWindowId, ItemId, PaneId, PanelTypeId, SplitId, WorkspaceNode};

const SESSION_VERSION: u32 = 1;

/// Portable layout data. Decode through `restore` before projecting or mutating it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSession {
    version: u32,
    workspace: WorkspaceModel,
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
    MissingPane(PaneId),
    RepeatedPane(PaneId),
    OrphanPane(PaneId),
    RepeatedSplit(SplitId),
    RepeatedWindow(FloatingWindowId),
    RepeatedItem(ItemId),
    InvalidActivePane(PaneId),
    InvalidActiveItem(PaneId),
    InvalidSplitRatio(SplitId),
    InvalidWindowPlacement(FloatingWindowId),
    InvalidAllocator(&'static str),
}

impl fmt::Display for WorkspaceSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid workspace session: {self:?}")
    }
}

impl Error for WorkspaceSessionError {}

impl WorkspaceSession {
    pub fn capture(workspace: &WorkspaceModel) -> Result<Self, WorkspaceSessionError> {
        validate(workspace)?;
        Ok(Self {
            version: SESSION_VERSION,
            workspace: workspace.clone(),
        })
    }

    pub fn to_json(&self) -> Result<String, WorkspaceSessionError> {
        serde_json::to_string_pretty(self)
            .map_err(|error| WorkspaceSessionError::Json(error.to_string()))
    }

    pub fn from_json(json: &str) -> Result<Self, WorkspaceSessionError> {
        serde_json::from_str(json).map_err(|error| WorkspaceSessionError::Json(error.to_string()))
    }

    /// Validate saved state and report panels absent from the current host registry.
    /// Missing panels retain their stable IDs and panel types for later recovery.
    pub fn restore(
        self,
        mut panel_available: impl FnMut(&PanelTypeId) -> bool,
    ) -> Result<RestoredWorkspace, WorkspaceSessionError> {
        if self.version != SESSION_VERSION {
            return Err(WorkspaceSessionError::UnsupportedVersion(self.version));
        }
        validate(&self.workspace)?;
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
            workspace: self.workspace,
            unavailable_panels,
        })
    }
}

fn validate(workspace: &WorkspaceModel) -> Result<(), WorkspaceSessionError> {
    let mut panes = BTreeSet::new();
    let mut splits = BTreeSet::new();
    let mut max_pane = 0;
    let mut max_split = 0;
    validate_node(
        &workspace.root,
        &mut panes,
        &mut splits,
        &mut max_pane,
        &mut max_split,
    )?;

    let mut windows = BTreeSet::new();
    let mut max_window = 0;
    for window in &workspace.floating {
        if window.id.0 == 0 || !windows.insert(window.id) {
            return Err(WorkspaceSessionError::RepeatedWindow(window.id));
        }
        max_window = max_window.max(window.id.0);
        let placement = window.placement;
        if ![placement.x, placement.y, placement.width, placement.height]
            .into_iter()
            .all(f32::is_finite)
            || placement.width <= 0.0
            || placement.height <= 0.0
        {
            return Err(WorkspaceSessionError::InvalidWindowPlacement(window.id));
        }
        validate_node(
            &window.root,
            &mut panes,
            &mut splits,
            &mut max_pane,
            &mut max_split,
        )?;
    }

    let mut items = BTreeSet::new();
    let mut max_item = 0;
    for (pane_id, pane) in &workspace.panes {
        if !panes.contains(pane_id) {
            return Err(WorkspaceSessionError::OrphanPane(*pane_id));
        }
        if *pane_id != pane.id {
            return Err(WorkspaceSessionError::MissingPane(pane.id));
        }
        if pane.active.is_some() == pane.items.is_empty()
            || pane
                .active
                .is_some_and(|active| !pane.items.iter().any(|item| item.id == active))
        {
            return Err(WorkspaceSessionError::InvalidActiveItem(*pane_id));
        }
        for item in &pane.items {
            if item.id.0 == 0 || !items.insert(item.id) {
                return Err(WorkspaceSessionError::RepeatedItem(item.id));
            }
            max_item = max_item.max(item.id.0);
        }
    }
    if let Some(missing) = panes
        .iter()
        .find(|pane| !workspace.panes.contains_key(pane))
    {
        return Err(WorkspaceSessionError::MissingPane(*missing));
    }
    if !panes.contains(&workspace.active_pane) {
        return Err(WorkspaceSessionError::InvalidActivePane(
            workspace.active_pane,
        ));
    }
    for (name, next, max) in [
        ("item", workspace.next_item_id, max_item),
        ("pane", workspace.next_pane_id, max_pane),
        ("split", workspace.next_split_id, max_split),
        ("window", workspace.next_window_id, max_window),
    ] {
        if next == 0 || next <= max {
            return Err(WorkspaceSessionError::InvalidAllocator(name));
        }
    }
    Ok(())
}

fn validate_node(
    node: &WorkspaceNode,
    panes: &mut BTreeSet<PaneId>,
    splits: &mut BTreeSet<SplitId>,
    max_pane: &mut u64,
    max_split: &mut u64,
) -> Result<(), WorkspaceSessionError> {
    match node {
        WorkspaceNode::Leaf(pane) => {
            if pane.0 == 0 || !panes.insert(*pane) {
                return Err(WorkspaceSessionError::RepeatedPane(*pane));
            }
            *max_pane = (*max_pane).max(pane.0);
        }
        WorkspaceNode::Split {
            id,
            ratio,
            first,
            second,
            ..
        } => {
            if id.0 == 0 || !splits.insert(*id) {
                return Err(WorkspaceSessionError::RepeatedSplit(*id));
            }
            if !ratio.is_finite() || !(0.05..=0.95).contains(ratio) {
                return Err(WorkspaceSessionError::InvalidSplitRatio(*id));
            }
            *max_split = (*max_split).max(id.0);
            validate_node(first, panes, splits, max_pane, max_split)?;
            validate_node(second, panes, splits, max_pane, max_split)?;
        }
    }
    Ok(())
}
