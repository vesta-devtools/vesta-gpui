//! WorkspaceModel: authoritative, engine-independent workspace state and its
//! deterministic dock-tree operations.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::types::*;

mod session;
pub use session::{RestoredWorkspace, UnavailablePanel, WorkspaceSession, WorkspaceSessionError};

/// Authoritative, engine-independent workspace state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceModel {
    root: WorkspaceNode,
    panes: BTreeMap<PaneId, Pane>,
    floating: Vec<FloatingWindow>,
    active_pane: PaneId,
    next_item_id: u64,
    next_pane_id: u64,
    next_split_id: u64,
    next_window_id: u64,
}

impl WorkspaceModel {
    /// Builds a workspace from an authored tree and its pane set.
    ///
    /// Validates that pane identities are unique, every leaf references an existing pane, and
    /// split identities are unique.
    pub fn with_tree(
        root: WorkspaceNode,
        panes: impl IntoIterator<Item = Pane>,
    ) -> Result<Self, WorkspaceError> {
        let mut seen_panes = HashSet::new();
        let mut panes_by_id = BTreeMap::new();
        for pane in panes {
            if !seen_panes.insert(pane.id) {
                return Err(WorkspaceError::DuplicatePane(pane.id));
            }
            panes_by_id.insert(pane.id, pane);
        }
        for pane_id in root.panes() {
            if !seen_panes.contains(&pane_id) {
                return Err(WorkspaceError::MissingPane(pane_id));
            }
        }

        let mut seen_splits = HashSet::new();
        let mut max_split = 0_u64;
        collect_split_ids(&root, &mut seen_splits, &mut max_split)?;

        let active_pane = *root.panes().first().ok_or(WorkspaceError::EmptyWorkspace)?;
        let next_pane_id = panes_by_id
            .keys()
            .map(|id| id.0)
            .max()
            .map_or(1, |max| max.saturating_add(1));
        // Item ids must be globally unique and the allocator must continue after any
        // pre-seeded items, otherwise restore/save would mint duplicate identities.
        let mut seen_items = HashSet::new();
        let mut max_item = 0_u64;
        for pane in panes_by_id.values() {
            for item in &pane.items {
                if !seen_items.insert(item.id) {
                    return Err(WorkspaceError::DuplicateItem(item.id));
                }
                max_item = max_item.max(item.id.0);
            }
        }
        Ok(Self {
            root,
            panes: panes_by_id,
            floating: Vec::new(),
            active_pane,
            next_item_id: max_item.saturating_add(1),
            next_pane_id,
            next_split_id: max_split.saturating_add(1),
            next_window_id: 1,
        })
    }

    // -- queries ------------------------------------------------------------

    #[must_use]
    pub fn root(&self) -> &WorkspaceNode {
        &self.root
    }

    /// All panes in depth-first tree order.
    #[must_use]
    pub fn panes(&self) -> Vec<&Pane> {
        self.root
            .panes()
            .into_iter()
            .filter_map(|id| self.panes.get(&id))
            .collect()
    }

    #[must_use]
    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.panes.get(&id)
    }

    #[must_use]
    pub fn item(&self, id: ItemId) -> Option<&WorkspaceItem> {
        self.panes
            .values()
            .flat_map(|pane| pane.items.iter())
            .find(|item| item.id == id)
    }

    /// 把现有 item 切换为另一种面板类型（Tab 表头右键切换），并同步标题。
    pub fn set_item_type(&mut self, id: ItemId, panel_type: PanelTypeId, title: &str) {
        for pane in self.panes.values_mut() {
            for item in &mut pane.items {
                if item.id == id {
                    item.panel_type = panel_type;
                    item.title = title.to_owned();
                    return;
                }
            }
        }
    }

    #[must_use]
    pub const fn active_pane(&self) -> PaneId {
        self.active_pane
    }

    #[must_use]
    pub fn active_item(&self) -> Option<&WorkspaceItem> {
        self.panes
            .get(&self.active_pane)
            .and_then(Pane::active_item)
    }

    #[must_use]
    pub fn floating_windows(&self) -> &[FloatingWindow] {
        &self.floating
    }

    #[must_use]
    pub fn floating_window(&self, id: FloatingWindowId) -> Option<&FloatingWindow> {
        self.floating.iter().find(|window| window.id == id)
    }

    pub fn floating_window_mut(&mut self, id: FloatingWindowId) -> Option<&mut FloatingWindow> {
        self.floating.iter_mut().find(|window| window.id == id)
    }

    /// 创建并返回一个包含指定面板的浮动窗口（节点属性等独立编辑面板使用）。
    pub fn spawn_floating_window(
        &mut self,
        title: String,
        placement: WindowPlacement,
        panel_title: String,
        panel_type: PanelTypeId,
    ) -> FloatingWindowId {
        let item = WorkspaceItem {
            id: ItemId(self.next_item_id),
            title: panel_title,
            panel_type,
        };
        self.next_item_id = self.next_item_id.saturating_add(1);
        let new_pane = self.spawn_pane_with(item);
        let id = FloatingWindowId(self.next_window_id);
        self.floating.push(FloatingWindow {
            id,
            title,
            placement,
            root: WorkspaceNode::Leaf(new_pane),
        });
        self.next_window_id = self.next_window_id.saturating_add(1);
        id
    }

    #[must_use]
    pub fn container_of_pane(&self, pane: PaneId) -> Option<ContainerRef> {
        if self.root.panes().contains(&pane) {
            Some(ContainerRef::Main)
        } else {
            self.floating
                .iter()
                .find(|window| window.root.panes().contains(&pane))
                .map(|window| ContainerRef::Floating(window.id))
        }
    }

    // -- item mutation ------------------------------------------------------

    pub fn add_item(
        &mut self,
        pane: PaneId,
        title: impl Into<String>,
        panel_type: PanelTypeId,
    ) -> Result<ItemId, WorkspaceError> {
        let id = ItemId(self.next_item_id);
        self.next_item_id = self.next_item_id.saturating_add(1);
        let target = self
            .panes
            .get_mut(&pane)
            .ok_or(WorkspaceError::MissingPane(pane))?;
        target.items.push(WorkspaceItem {
            id,
            title: title.into(),
            panel_type,
        });
        target.active = Some(id);
        self.active_pane = pane;
        Ok(id)
    }

    pub fn activate(&mut self, item: ItemId) -> Result<(), WorkspaceError> {
        let pane = self
            .panes
            .values_mut()
            .find(|pane| pane.items.iter().any(|candidate| candidate.id == item))
            .ok_or(WorkspaceError::MissingItem(item))?;
        pane.active = Some(item);
        self.active_pane = pane.id;
        Ok(())
    }

    /// Applies a semantic dock operation to an existing item.
    ///
    /// All targets are validated before any mutation: a failed dock never loses the item
    /// or leaves a half-applied tree.
    pub fn dock_item(&mut self, item: ItemId, action: DockAction) -> Result<(), WorkspaceError> {
        let source_container = self
            .container_of_item(item)
            .ok_or(WorkspaceError::MissingItem(item))?;

        match action {
            DockAction::Merge { pane, .. } => {
                if !self.panes.contains_key(&pane) {
                    return Err(WorkspaceError::MissingPane(pane));
                }
            }
            DockAction::Split { pane, .. } => {
                self.container_of_pane(pane)
                    .ok_or(WorkspaceError::MissingPane(pane))?;
            }
            DockAction::WindowSplit { container, .. } => {
                if !self.container_exists(container) {
                    return Err(WorkspaceError::MissingContainer(container));
                }
            }
            DockAction::Float { .. } => {}
        }

        let moved = self.remove_item_from_pane(item)?;
        let result = match action {
            DockAction::Merge { pane, slot } => {
                let adjusted_slot = if pane == moved.1 {
                    slot.saturating_sub(1)
                } else {
                    slot
                };
                self.insert_item(pane, moved.0, adjusted_slot);
                Ok(())
            }
            DockAction::Split { pane, side } => {
                let container = self
                    .container_of_pane(pane)
                    .expect("the split target was validated above");
                let new_pane = self.spawn_pane_with(moved.0);
                self.replace_pane_with_split(container, pane, side, new_pane)
            }
            DockAction::WindowSplit { container, side } => {
                let new_pane = self.spawn_pane_with(moved.0);
                self.split_container_edge(container, side, new_pane)
            }
            DockAction::Float { placement } => {
                let title = moved.0.title.clone();
                let new_pane = self.spawn_pane_with(moved.0);
                self.floating.push(FloatingWindow {
                    id: FloatingWindowId(self.next_window_id),
                    title,
                    placement,
                    root: WorkspaceNode::Leaf(new_pane),
                });
                self.next_window_id = self.next_window_id.saturating_add(1);
                Ok(())
            }
        };

        if result.is_ok() {
            self.reclaim_empty_panes(source_container);
        }
        result
    }

    /// Resizes one split inside a container. `ratio` is clamped to `(0.05, 0.95)`.
    pub fn set_split_ratio(
        &mut self,
        container: ContainerRef,
        split: SplitId,
        ratio: f32,
    ) -> Result<(), WorkspaceError> {
        let root = self.container_root_mut(container).ok_or(match container {
            ContainerRef::Main => WorkspaceError::EmptyWorkspace,
            ContainerRef::Floating(id) => WorkspaceError::MissingWindow(id),
        })?;
        if !update_split_ratio(root, split, ratio) {
            return Err(WorkspaceError::MissingSplit(split));
        }
        Ok(())
    }

    /// Closes an item and reclaims the pane when it becomes empty.
    pub fn close_item(&mut self, item: ItemId) -> Result<WorkspaceItem, WorkspaceError> {
        let source_container = self
            .container_of_item(item)
            .ok_or(WorkspaceError::MissingItem(item))?;
        let (removed, _) = self.remove_item_from_pane(item)?;
        self.reclaim_empty_panes(source_container);
        Ok(removed)
    }

    /// Closes a floating window and drops all of its items.
    pub fn close_floating_window(&mut self, id: FloatingWindowId) -> Result<(), WorkspaceError> {
        let index = self
            .floating
            .iter()
            .position(|window| window.id == id)
            .ok_or(WorkspaceError::MissingWindow(id))?;
        if self.active_pane_id_is_inside(&self.floating[index]) {
            self.active_pane = self.first_main_pane();
        }
        self.floating.remove(index);
        self.reclaim_empty_panes(ContainerRef::Floating(id));
        Ok(())
    }

    /// Atomically move every item from one floating window back into a main-window pane.
    ///
    /// Hosts use this for an OS close request so closing a presentation surface cannot silently
    /// discard authored workspace items. Items keep their stable identities and their depth-first
    /// pane/tab order from the floating tree.
    pub fn restore_floating_window(
        &mut self,
        id: FloatingWindowId,
        target: PaneId,
    ) -> Result<Vec<ItemId>, WorkspaceError> {
        if !self.root.panes().contains(&target) {
            return Err(WorkspaceError::MissingPane(target));
        }
        let window = self
            .floating_window(id)
            .ok_or(WorkspaceError::MissingWindow(id))?;
        let items = window
            .root
            .panes()
            .into_iter()
            .filter_map(|pane| self.panes.get(&pane))
            .flat_map(|pane| pane.items.iter().map(|item| item.id))
            .collect::<Vec<_>>();

        let mut candidate = self.clone();
        for item in &items {
            let slot = candidate
                .pane(target)
                .ok_or(WorkspaceError::MissingPane(target))?
                .items
                .len();
            candidate.dock_item(*item, DockAction::Merge { pane: target, slot })?;
        }
        if items.is_empty() {
            candidate.close_floating_window(id)?;
        }
        *self = candidate;
        Ok(items)
    }

    /// Current owning container of a stable item, or `None` when it is absent.
    /// Hosts use this same model lookup to retire gestures after item relocation.
    #[must_use]
    pub fn container_of_item(&self, item: ItemId) -> Option<ContainerRef> {
        let pane = self
            .panes
            .values()
            .find(|pane| pane.items.iter().any(|candidate| candidate.id == item))?;
        self.container_of_pane(pane.id)
    }

    // -- internals ----------------------------------------------------------

    /// Removes an item from its pane and fixes the pane's active item.
    fn remove_item_from_pane(
        &mut self,
        item: ItemId,
    ) -> Result<(WorkspaceItem, PaneId), WorkspaceError> {
        let pane = self
            .panes
            .values_mut()
            .find(|pane| pane.items.iter().any(|candidate| candidate.id == item))
            .ok_or(WorkspaceError::MissingItem(item))?;
        let slot = pane
            .items
            .iter()
            .position(|candidate| candidate.id == item)
            .expect("the matching item exists");
        let removed = pane.items.remove(slot);
        if pane.active == Some(item) {
            pane.active = pane
                .items
                .get(slot)
                .or_else(|| pane.items.last())
                .map(|candidate| candidate.id);
        }
        Ok((removed, pane.id))
    }

    fn insert_item(&mut self, pane: PaneId, item: WorkspaceItem, slot: usize) {
        let target = self.panes.get_mut(&pane).expect("the target pane exists");
        let insertion = slot.min(target.items.len());
        target.items.insert(insertion, item);
        target.active = Some(target.items[insertion].id);
        self.active_pane = pane;
    }

    fn spawn_pane_with(&mut self, item: WorkspaceItem) -> PaneId {
        let id = PaneId(self.next_pane_id);
        self.next_pane_id = self.next_pane_id.saturating_add(1);
        let item_id = item.id;
        let mut pane = Pane::new(id);
        pane.items.push(item);
        pane.active = Some(item_id);
        self.panes.insert(id, pane);
        self.active_pane = id;
        id
    }

    fn replace_pane_with_split(
        &mut self,
        container: ContainerRef,
        pane: PaneId,
        side: Side,
        new_pane: PaneId,
    ) -> Result<(), WorkspaceError> {
        let axis = SplitAxis::for_side(side);
        let split = WorkspaceNode::split(
            SplitId(self.next_split_id),
            axis,
            0.5,
            match side {
                Side::Left | Side::Top => WorkspaceNode::leaf(new_pane),
                Side::Right | Side::Bottom => WorkspaceNode::leaf(pane),
            },
            match side {
                Side::Left | Side::Top => WorkspaceNode::leaf(pane),
                Side::Right | Side::Bottom => WorkspaceNode::leaf(new_pane),
            },
        );
        self.next_split_id = self.next_split_id.saturating_add(1);
        let root = self
            .container_root_mut(container)
            .ok_or(WorkspaceError::MissingContainer(container))?;
        if !replace_leaf(root, pane, split) {
            return Err(WorkspaceError::MissingPane(pane));
        }
        Ok(())
    }

    fn split_container_edge(
        &mut self,
        container: ContainerRef,
        side: Side,
        new_pane: PaneId,
    ) -> Result<(), WorkspaceError> {
        let axis = SplitAxis::for_side(side);
        let first = matches!(side, Side::Left | Side::Top);
        let split_id = SplitId(self.next_split_id);
        self.next_split_id = self.next_split_id.saturating_add(1);
        let root = self
            .container_root_mut(container)
            .ok_or(WorkspaceError::MissingContainer(container))?;
        // Move the existing root out; the placeholder leaf never escapes this call
        // because the root is fully rewritten before anything reads it again.
        let existing = std::mem::replace(root, WorkspaceNode::leaf(PLACEHOLDER_PANE));
        let (first_node, second_node) = if first {
            (WorkspaceNode::leaf(new_pane), existing)
        } else {
            (existing, WorkspaceNode::leaf(new_pane))
        };
        *root = WorkspaceNode::split(split_id, axis, 0.3, first_node, second_node);
        Ok(())
    }

    fn container_root_mut(&mut self, container: ContainerRef) -> Option<&mut WorkspaceNode> {
        match container {
            ContainerRef::Main => Some(&mut self.root),
            ContainerRef::Floating(id) => self
                .floating
                .iter_mut()
                .find(|window| window.id == id)
                .map(|window| &mut window.root),
        }
    }

    fn container_exists(&self, container: ContainerRef) -> bool {
        match container {
            ContainerRef::Main => true,
            ContainerRef::Floating(id) => self.floating.iter().any(|window| window.id == id),
        }
    }

    /// Removes empty leaf panes after a mutation. The main window always keeps at least one
    /// pane; a floating window with no remaining pane is closed. Panes that are no longer
    /// referenced by any surviving tree are dropped from the pane store.
    fn reclaim_empty_panes(&mut self, source_container: ContainerRef) {
        if source_container == ContainerRef::Main {
            let fallback_pane = self.first_main_pane();
            let pruned = prune_node(
                std::mem::replace(&mut self.root, WorkspaceNode::leaf(fallback_pane)),
                &self.panes,
                true,
            );
            self.root = pruned.unwrap_or_else(|| {
                let pane = PaneId(self.next_pane_id);
                self.next_pane_id = self.next_pane_id.saturating_add(1);
                self.panes.insert(pane, Pane::new(pane));
                WorkspaceNode::leaf(pane)
            });
        } else {
            let mut surviving = Vec::with_capacity(self.floating.len());
            for mut window in std::mem::take(&mut self.floating) {
                if let Some(root) = prune_node(window.root, &self.panes, false) {
                    window.root = root;
                    surviving.push(window);
                }
            }
            self.floating = surviving;
        }
        let mut referenced = HashSet::new();
        referenced.extend(self.root.panes());
        for window in &self.floating {
            referenced.extend(window.root.panes());
        }
        self.panes.retain(|id, _| referenced.contains(id));
        self.refresh_active_pane();
    }

    fn refresh_active_pane(&mut self) {
        if !self.panes.contains_key(&self.active_pane) {
            self.active_pane = self.first_main_pane();
        }
    }

    fn first_main_pane(&self) -> PaneId {
        self.root
            .panes()
            .first()
            .copied()
            .expect("the main tree always contains at least one pane")
    }

    fn active_pane_id_is_inside(&self, window: &FloatingWindow) -> bool {
        window.root.panes().contains(&self.active_pane)
    }
}

// ---------------------------------------------------------------------------
// Tree helpers
// ---------------------------------------------------------------------------

/// Temporary identity used only while moving a container root out of place during a
/// split; it never survives past the same function call.
const PLACEHOLDER_PANE: PaneId = PaneId(0);

fn collect_split_ids(
    node: &WorkspaceNode,
    seen: &mut HashSet<SplitId>,
    max: &mut u64,
) -> Result<(), WorkspaceError> {
    match node {
        WorkspaceNode::Leaf(_) => Ok(()),
        WorkspaceNode::Split {
            id, first, second, ..
        } => {
            if !seen.insert(*id) {
                return Err(WorkspaceError::DuplicateSplit(*id));
            }
            *max = (*max).max(id.0);
            collect_split_ids(first, seen, max)?;
            collect_split_ids(second, seen, max)
        }
    }
}

fn replace_leaf(node: &mut WorkspaceNode, target: PaneId, replacement: WorkspaceNode) -> bool {
    match node {
        WorkspaceNode::Leaf(pane) if *pane == target => {
            *node = replacement;
            true
        }
        WorkspaceNode::Split { first, second, .. } => {
            replace_leaf(first, target, replacement.clone())
                || replace_leaf(second, target, replacement)
        }
        _ => false,
    }
}

fn update_split_ratio(node: &mut WorkspaceNode, split: SplitId, ratio: f32) -> bool {
    match node {
        WorkspaceNode::Split {
            id,
            ratio: current,
            first,
            second,
            ..
        } => {
            if *id == split {
                *current = sanitize_ratio(ratio, *current);
                true
            } else {
                update_split_ratio(first, split, ratio) || update_split_ratio(second, split, ratio)
            }
        }
        WorkspaceNode::Leaf(_) => false,
    }
}

/// Clamps a ratio into `(0.05, 0.95)`, falling back when the value is NaN (which
/// `f32::clamp` would otherwise propagate and poison the derived layout).
pub(crate) fn sanitize_ratio(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.05, 0.95)
    } else {
        fallback
    }
}

/// Recursively removes empty panes. Returns `None` when the whole subtree should be removed.
fn prune_node(
    node: WorkspaceNode,
    panes: &BTreeMap<PaneId, Pane>,
    keep_root_leaf: bool,
) -> Option<WorkspaceNode> {
    match node {
        WorkspaceNode::Leaf(pane_id) => {
            let empty = panes.get(&pane_id).is_none_or(|pane| pane.items.is_empty());
            if empty && !keep_root_leaf {
                None
            } else {
                Some(WorkspaceNode::Leaf(pane_id))
            }
        }
        WorkspaceNode::Split {
            id,
            axis,
            ratio,
            first,
            second,
        } => {
            let first = prune_node(*first, panes, false);
            let second = prune_node(*second, panes, false);
            match (first, second) {
                (Some(first), Some(second)) => Some(WorkspaceNode::Split {
                    id,
                    axis,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (Some(only), None) | (None, Some(only)) => Some(only),
                (None, None) => None,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceError {
    EmptyWorkspace,
    DuplicatePane(PaneId),
    DuplicateSplit(SplitId),
    DuplicateItem(ItemId),
    MissingPane(PaneId),
    MissingItem(ItemId),
    MissingSplit(SplitId),
    MissingWindow(FloatingWindowId),
    MissingContainer(ContainerRef),
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyWorkspace => formatter.write_str("workspace must contain at least one pane"),
            Self::DuplicatePane(id) => write!(formatter, "pane id {} is duplicated", id.0),
            Self::DuplicateSplit(id) => write!(formatter, "split id {} is duplicated", id.0),
            Self::DuplicateItem(id) => {
                write!(formatter, "workspace item id {} is duplicated", id.0)
            }
            Self::MissingPane(id) => write!(formatter, "pane id {} does not exist", id.0),
            Self::MissingItem(id) => write!(formatter, "workspace item id {} does not exist", id.0),
            Self::MissingSplit(id) => write!(formatter, "split id {} does not exist", id.0),
            Self::MissingWindow(id) => {
                write!(formatter, "floating window id {} does not exist", id.0)
            }
            Self::MissingContainer(container) => {
                write!(formatter, "container {container:?} does not exist")
            }
        }
    }
}

impl std::error::Error for WorkspaceError {}
