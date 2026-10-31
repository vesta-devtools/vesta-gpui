//! The one invariant validator for construction and versioned restoration.
use super::{WorkspaceError, WorkspaceState};
use crate::types::*;
use std::collections::BTreeSet;

pub(super) fn placement(id: FloatingWindowId, p: WindowPlacement) -> Result<(), WorkspaceError> {
    if p.width <= 0.0
        || p.height <= 0.0
        || ![p.x, p.y, p.width, p.height, p.x + p.width, p.y + p.height]
            .into_iter()
            .all(f32::is_finite)
    {
        return Err(WorkspaceError::InvalidWindowPlacement(id));
    }
    Ok(())
}
pub(super) fn validate(state: &WorkspaceState) -> Result<(), WorkspaceError> {
    let mut panes = BTreeSet::new();
    let mut splits = BTreeSet::new();
    node(&state.root, &mut panes, &mut splits)?;
    let mut windows = BTreeSet::new();
    for window in &state.floating {
        if window.id.0 == 0 {
            return Err(WorkspaceError::InvalidIdentity("window"));
        }
        if !windows.insert(window.id) {
            return Err(WorkspaceError::DuplicateWindow(window.id));
        }
        placement(window.id, window.placement)?;
        node(&window.root, &mut panes, &mut splits)?;
    }
    if let Some(missing) = panes.iter().find(|id| !state.panes.contains_key(id)) {
        return Err(WorkspaceError::MissingPane(*missing));
    }
    let mut items = BTreeSet::new();
    for (id, pane) in &state.panes {
        if !panes.contains(id) {
            return Err(WorkspaceError::OrphanPane(*id));
        }
        if *id != pane.id {
            return Err(WorkspaceError::InvalidPaneKey(*id));
        }
        if pane.active.is_some() == pane.items.is_empty()
            || pane
                .active
                .is_some_and(|active| !pane.items.iter().any(|item| item.id == active))
        {
            return Err(WorkspaceError::InvalidActiveItem(*id));
        }
        for item in &pane.items {
            if item.id.0 == 0 {
                return Err(WorkspaceError::InvalidIdentity("item"));
            }
            if !items.insert(item.id) {
                return Err(WorkspaceError::DuplicateItem(item.id));
            }
        }
    }
    if !panes.contains(&state.active_pane) {
        return Err(WorkspaceError::InvalidActivePane(state.active_pane));
    }
    for (name, next, max) in [
        (
            "item",
            state.next_item_id,
            items.last().map_or(0, |id| id.0),
        ),
        (
            "pane",
            state.next_pane_id,
            panes.last().map_or(0, |id| id.0),
        ),
        (
            "split",
            state.next_split_id,
            splits.last().map_or(0, |id| id.0),
        ),
        (
            "window",
            state.next_window_id,
            windows.last().map_or(0, |id| id.0),
        ),
    ] {
        if next == 0 || next <= max {
            return Err(WorkspaceError::InvalidAllocator(name));
        }
    }
    Ok(())
}
fn node(
    value: &WorkspaceNode,
    panes: &mut BTreeSet<PaneId>,
    splits: &mut BTreeSet<SplitId>,
) -> Result<(), WorkspaceError> {
    match value {
        WorkspaceNode::Leaf(pane) => {
            if pane.0 == 0 {
                return Err(WorkspaceError::InvalidIdentity("pane"));
            }
            if !panes.insert(*pane) {
                return Err(WorkspaceError::DuplicatePane(*pane));
            }
        }
        WorkspaceNode::Split {
            id,
            ratio,
            first,
            second,
            ..
        } => {
            if id.0 == 0 {
                return Err(WorkspaceError::InvalidIdentity("split"));
            }
            if !splits.insert(*id) {
                return Err(WorkspaceError::DuplicateSplit(*id));
            }
            if !ratio.is_finite() || !(0.05..=0.95).contains(ratio) {
                return Err(WorkspaceError::InvalidSplitRatio(*id));
            }
            node(first, panes, splits)?;
            node(second, panes, splits)?;
        }
    }
    Ok(())
}
