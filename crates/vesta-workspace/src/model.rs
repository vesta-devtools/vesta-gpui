//! WorkspaceModel: authoritative, engine-independent workspace state and its
//! deterministic dock-tree operations.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::types::*;

mod session;
mod validation;
pub use session::{RestoredWorkspace, UnavailablePanel, WorkspaceSession, WorkspaceSessionError};

/// Authoritative, engine-independent workspace state.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct WorkspaceModel {
    state: WorkspaceState,
}

// One field representation: a live model validates it; a session holds an
// untrusted snapshot until restore. No raw Deserialize for the live owner.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct WorkspaceState {
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
    /// Require complete, unique ownership, valid active items and finite ratios.
    /// Every nonempty pane must explicitly select one of its items as active.
    pub fn with_tree(
        root: WorkspaceNode,
        panes: impl IntoIterator<Item = Pane>,
    ) -> Result<Self, WorkspaceError> {
        let mut panes_by_id = BTreeMap::new();
        for pane in panes {
            let id = pane.id;
            if panes_by_id.insert(id, pane).is_some() {
                return Err(WorkspaceError::DuplicatePane(id));
            }
        }

        let max_split = max_split_id(&root);

        let active_pane = root.panes()[0]; // Every recursive node contains a leaf.
        let next_pane_id = panes_by_id
            .keys()
            .map(|id| id.0)
            .max()
            .map_or(Ok(1), |max| advance_id(max, "pane"))?;
        // Item ids must be globally unique and the allocator must continue after any
        // pre-seeded items, otherwise restore/save would mint duplicate identities.
        let max_item = panes_by_id
            .values()
            .flat_map(|pane| &pane.items)
            .map(|item| item.id.0)
            .max()
            .unwrap_or(0);
        let state = WorkspaceState {
            root,
            panes: panes_by_id,
            floating: Vec::new(),
            active_pane,
            next_item_id: advance_id(max_item, "item")?,
            next_pane_id,
            next_split_id: advance_id(max_split, "split")?,
            next_window_id: 1,
        };
        validation::validate(&state)?;
        Ok(Self { state })
    }

    // -- queries ------------------------------------------------------------

    #[must_use]
    pub fn root(&self) -> &WorkspaceNode {
        &self.state.root
    }

    /// All panes in depth-first tree order.
    #[must_use]
    pub fn panes(&self) -> Vec<&Pane> {
        self.state
            .root
            .panes()
            .into_iter()
            .filter_map(|id| self.state.panes.get(&id))
            .collect()
    }

    #[must_use]
    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.state.panes.get(&id)
    }

    #[must_use]
    pub fn item(&self, id: ItemId) -> Option<&WorkspaceItem> {
        self.state
            .panes
            .values()
            .flat_map(|pane| pane.items.iter())
            .find(|item| item.id == id)
    }

    /// 把现有 item 切换为另一种面板类型（Tab 表头右键切换），并同步标题。
    pub fn set_item_type(
        &mut self,
        id: ItemId,
        panel_type: PanelTypeId,
        title: &str,
    ) -> Result<(), WorkspaceError> {
        for pane in self.state.panes.values_mut() {
            for item in &mut pane.items {
                if item.id == id {
                    item.panel_type = panel_type;
                    item.title = title.to_owned();
                    return Ok(());
                }
            }
        }
        Err(WorkspaceError::MissingItem(id))
    }

    #[must_use]
    pub const fn active_pane(&self) -> PaneId {
        self.state.active_pane
    }

    #[must_use]
    pub fn active_item(&self) -> Option<&WorkspaceItem> {
        self.state
            .panes
            .get(&self.state.active_pane)
            .and_then(Pane::active_item)
    }

    #[must_use]
    pub fn floating_windows(&self) -> &[FloatingWindow] {
        &self.state.floating
    }

    #[must_use]
    pub fn floating_window(&self, id: FloatingWindowId) -> Option<&FloatingWindow> {
        self.state.floating.iter().find(|window| window.id == id)
    }

    pub fn set_floating_placement(
        &mut self,
        id: FloatingWindowId,
        placement: WindowPlacement,
    ) -> Result<(), WorkspaceError> {
        let window = self
            .state
            .floating
            .iter_mut()
            .find(|window| window.id == id)
            .ok_or(WorkspaceError::MissingWindow(id))?;
        validation::placement(id, placement)?;
        window.placement = placement;
        Ok(())
    }

    pub fn set_floating_title(
        &mut self,
        id: FloatingWindowId,
        title: impl Into<String>,
    ) -> Result<(), WorkspaceError> {
        let window = self
            .state
            .floating
            .iter_mut()
            .find(|window| window.id == id)
            .ok_or(WorkspaceError::MissingWindow(id))?;
        window.title = title.into();
        Ok(())
    }

    /// 创建并返回一个包含指定面板的浮动窗口（节点属性等独立编辑面板使用）。
    pub fn spawn_floating_window(
        &mut self,
        title: String,
        placement: WindowPlacement,
        panel_title: String,
        panel_type: PanelTypeId,
    ) -> Result<FloatingWindowId, WorkspaceError> {
        let id = FloatingWindowId(self.state.next_window_id);
        validation::placement(id, placement)?;
        let next_item = advance_id(self.state.next_item_id, "item")?;
        advance_id(self.state.next_pane_id, "pane")?;
        let next_window = advance_id(self.state.next_window_id, "window")?;
        let item = WorkspaceItem {
            id: ItemId(self.state.next_item_id),
            title: panel_title,
            panel_type,
        };
        self.state.next_item_id = next_item;
        let new_pane = self.spawn_pane_with(item);
        self.state.floating.push(FloatingWindow {
            id,
            title,
            placement,
            root: WorkspaceNode::Leaf(new_pane),
        });
        self.state.next_window_id = next_window;
        Ok(id)
    }

    #[must_use]
    pub fn container_of_pane(&self, pane: PaneId) -> Option<ContainerRef> {
        if self.state.root.panes().contains(&pane) {
            Some(ContainerRef::Main)
        } else {
            self.state
                .floating
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
        let id = ItemId(self.state.next_item_id);
        let target = self
            .state
            .panes
            .get_mut(&pane)
            .ok_or(WorkspaceError::MissingPane(pane))?;
        let next = advance_id(self.state.next_item_id, "item")?;
        target.items.push(WorkspaceItem {
            id,
            title: title.into(),
            panel_type,
        });
        target.active = Some(id);
        self.state.active_pane = pane;
        self.state.next_item_id = next;
        Ok(id)
    }

    pub fn activate(&mut self, item: ItemId) -> Result<(), WorkspaceError> {
        let pane = self
            .state
            .panes
            .values_mut()
            .find(|pane| pane.items.iter().any(|candidate| candidate.id == item))
            .ok_or(WorkspaceError::MissingItem(item))?;
        pane.active = Some(item);
        self.state.active_pane = pane.id;
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
                if !self.state.panes.contains_key(&pane) {
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
            DockAction::Float { placement } => {
                validation::placement(FloatingWindowId(self.state.next_window_id), placement)?;
            }
        }
        match action {
            DockAction::Split { .. } | DockAction::WindowSplit { .. } => {
                advance_id(self.state.next_pane_id, "pane")?;
                advance_id(self.state.next_split_id, "split")?;
            }
            DockAction::Float { .. } => {
                advance_id(self.state.next_pane_id, "pane")?;
                advance_id(self.state.next_window_id, "window")?;
            }
            DockAction::Merge { .. } => {}
        }

        let moved = self.remove_item_from_pane(item)?;
        match action {
            DockAction::Merge { pane, slot } => {
                let adjusted_slot = if pane == moved.1 && moved.2 < slot {
                    slot - 1
                } else {
                    slot
                };
                self.insert_item(pane, moved.0, adjusted_slot);
            }
            DockAction::Split { pane, side } => {
                let container = self
                    .container_of_pane(pane)
                    .expect("the split target was validated above");
                let new_pane = self.spawn_pane_with(moved.0);
                self.replace_pane_with_split(container, pane, side, new_pane);
            }
            DockAction::WindowSplit { container, side } => {
                let new_pane = self.spawn_pane_with(moved.0);
                self.split_container_edge(container, side, new_pane);
            }
            DockAction::Float { placement } => {
                let title = moved.0.title.clone();
                let new_pane = self.spawn_pane_with(moved.0);
                self.state.floating.push(FloatingWindow {
                    id: FloatingWindowId(self.state.next_window_id),
                    title,
                    placement,
                    root: WorkspaceNode::Leaf(new_pane),
                });
                self.state.next_window_id += 1; // All required cursors were preflighted.
            }
        }
        self.reclaim_empty_panes(source_container);
        Ok(())
    }

    /// Resizes one split. Finite ratios clamp to `0.05..=0.95`; others retain its value.
    pub fn set_split_ratio(
        &mut self,
        container: ContainerRef,
        split: SplitId,
        ratio: f32,
    ) -> Result<(), WorkspaceError> {
        let root = self
            .container_root_mut(container)
            .ok_or(WorkspaceError::MissingContainer(container))?;
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
        let (removed, _, _) = self.remove_item_from_pane(item)?;
        self.reclaim_empty_panes(source_container);
        Ok(removed)
    }

    /// Closes a floating window and drops all of its items.
    pub fn close_floating_window(&mut self, id: FloatingWindowId) -> Result<(), WorkspaceError> {
        let index = self
            .state
            .floating
            .iter()
            .position(|window| window.id == id)
            .ok_or(WorkspaceError::MissingWindow(id))?;
        if self.active_pane_id_is_inside(&self.state.floating[index]) {
            self.state.active_pane = self.first_main_pane();
        }
        self.state.floating.remove(index);
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
        if !self.state.root.panes().contains(&target) {
            return Err(WorkspaceError::MissingPane(target));
        }
        let window = self
            .floating_window(id)
            .ok_or(WorkspaceError::MissingWindow(id))?;
        let items = window
            .root
            .panes()
            .into_iter()
            .filter_map(|pane| self.state.panes.get(&pane))
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
            .state
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
    ) -> Result<(WorkspaceItem, PaneId, usize), WorkspaceError> {
        let pane = self
            .state
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
        Ok((removed, pane.id, slot))
    }

    fn insert_item(&mut self, pane: PaneId, item: WorkspaceItem, slot: usize) {
        let target = self
            .state
            .panes
            .get_mut(&pane)
            .expect("the target pane exists");
        let insertion = slot.min(target.items.len());
        target.items.insert(insertion, item);
        target.active = Some(target.items[insertion].id);
        self.state.active_pane = pane;
    }

    fn spawn_pane_with(&mut self, item: WorkspaceItem) -> PaneId {
        let id = PaneId(self.state.next_pane_id);
        self.state.next_pane_id += 1; // Preflighted before any mutation.
        let item_id = item.id;
        let mut pane = Pane::new(id);
        pane.items.push(item);
        pane.active = Some(item_id);
        self.state.panes.insert(id, pane);
        self.state.active_pane = id;
        id
    }

    fn replace_pane_with_split(
        &mut self,
        container: ContainerRef,
        pane: PaneId,
        side: Side,
        new_pane: PaneId,
    ) {
        let axis = SplitAxis::for_side(side);
        let split = WorkspaceNode::split(
            SplitId(self.state.next_split_id),
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
        self.state.next_split_id += 1;
        let root = self
            .container_root_mut(container)
            .expect("container preflighted before mutation");
        assert!(
            replace_leaf(root, pane, split),
            "pane preflighted before mutation"
        );
    }

    fn split_container_edge(&mut self, container: ContainerRef, side: Side, new_pane: PaneId) {
        let axis = SplitAxis::for_side(side);
        let first = matches!(side, Side::Left | Side::Top);
        let split_id = SplitId(self.state.next_split_id);
        self.state.next_split_id += 1;
        let root = self
            .container_root_mut(container)
            .expect("container preflighted before mutation");
        // Move the existing root out; the placeholder leaf never escapes this call
        // because the root is fully rewritten before anything reads it again.
        let existing = std::mem::replace(root, WorkspaceNode::leaf(PLACEHOLDER_PANE));
        let (first_node, second_node) = if first {
            (WorkspaceNode::leaf(new_pane), existing)
        } else {
            (existing, WorkspaceNode::leaf(new_pane))
        };
        *root = WorkspaceNode::split(split_id, axis, 0.3, first_node, second_node);
    }

    fn container_root_mut(&mut self, container: ContainerRef) -> Option<&mut WorkspaceNode> {
        match container {
            ContainerRef::Main => Some(&mut self.state.root),
            ContainerRef::Floating(id) => self
                .state
                .floating
                .iter_mut()
                .find(|window| window.id == id)
                .map(|window| &mut window.root),
        }
    }

    fn container_exists(&self, container: ContainerRef) -> bool {
        match container {
            ContainerRef::Main => true,
            ContainerRef::Floating(id) => self.state.floating.iter().any(|window| window.id == id),
        }
    }

    /// Removes empty leaf panes after a mutation. The main window always keeps at least one
    /// pane; a floating window with no remaining pane is closed. Panes that are no longer
    /// referenced by any surviving tree are dropped from the pane store.
    fn reclaim_empty_panes(&mut self, source_container: ContainerRef) {
        if source_container == ContainerRef::Main {
            let fallback_pane = self.first_main_pane();
            let pruned = prune_node(
                std::mem::replace(&mut self.state.root, WorkspaceNode::leaf(fallback_pane)),
                &self.state.panes,
                true,
            );
            self.state.root = pruned.unwrap_or(WorkspaceNode::leaf(fallback_pane));
        } else {
            let mut surviving = Vec::with_capacity(self.state.floating.len());
            for mut window in std::mem::take(&mut self.state.floating) {
                if let Some(root) = prune_node(window.root, &self.state.panes, false) {
                    window.root = root;
                    surviving.push(window);
                }
            }
            self.state.floating = surviving;
        }
        let mut referenced = HashSet::new();
        referenced.extend(self.state.root.panes());
        for window in &self.state.floating {
            referenced.extend(window.root.panes());
        }
        self.state.panes.retain(|id, _| referenced.contains(id));
        self.refresh_active_pane();
    }

    fn refresh_active_pane(&mut self) {
        if !self.state.panes.contains_key(&self.state.active_pane) {
            self.state.active_pane = self.first_main_pane();
        }
    }

    fn first_main_pane(&self) -> PaneId {
        self.state
            .root
            .panes()
            .first()
            .copied()
            .expect("the main tree always contains at least one pane")
    }

    fn active_pane_id_is_inside(&self, window: &FloatingWindow) -> bool {
        window.root.panes().contains(&self.state.active_pane)
    }
}

// ---------------------------------------------------------------------------
// Tree helpers
// ---------------------------------------------------------------------------

/// Temporary identity used only while moving a container root out of place during a
/// split; it never survives past the same function call.
const PLACEHOLDER_PANE: PaneId = PaneId(0);

fn max_split_id(node: &WorkspaceNode) -> u64 {
    match node {
        WorkspaceNode::Leaf(_) => 0,
        WorkspaceNode::Split {
            id, first, second, ..
        } => id.0.max(max_split_id(first)).max(max_split_id(second)),
    }
}

fn advance_id(value: u64, kind: &'static str) -> Result<u64, WorkspaceError> {
    value
        .checked_add(1)
        .ok_or(WorkspaceError::IdentityExhausted(kind))
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
    InvalidIdentity(&'static str),
    InvalidAllocator(&'static str),
    IdentityExhausted(&'static str),
    OrphanPane(PaneId),
    InvalidPaneKey(PaneId),
    InvalidActivePane(PaneId),
    InvalidActiveItem(PaneId),
    InvalidSplitRatio(SplitId),
    InvalidWindowPlacement(FloatingWindowId),
    DuplicateWindow(FloatingWindowId),
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
            Self::InvalidIdentity(kind) => write!(formatter, "{kind} identity zero is reserved"),
            Self::InvalidAllocator(kind) => write!(
                formatter,
                "{kind} allocator does not follow existing identities"
            ),
            Self::IdentityExhausted(kind) => {
                write!(formatter, "{kind} identity space is exhausted")
            }
            Self::OrphanPane(id) => write!(formatter, "pane {} is not owned by a container", id.0),
            Self::InvalidPaneKey(id) => write!(
                formatter,
                "pane map key {} does not match its identity",
                id.0
            ),
            Self::InvalidActivePane(id) => {
                write!(formatter, "active pane {} is not in a container", id.0)
            }
            Self::InvalidActiveItem(id) => {
                write!(formatter, "pane {} has no valid active item", id.0)
            }
            Self::InvalidSplitRatio(id) => write!(
                formatter,
                "split {} ratio is outside the finite admitted range",
                id.0
            ),
            Self::InvalidWindowPlacement(id) => {
                write!(formatter, "window {} placement has invalid bounds", id.0)
            }
            Self::DuplicateWindow(id) => write!(formatter, "window id {} is duplicated", id.0),
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
