//! Stable identities and value types of the workspace model.
//!
//! Split from lib.rs (§5 file governance): the recursive dock tree, its pane/
//! item/split/window identities and the host-facing value types.

#![forbid(unsafe_code)]

use crate::model::sanitize_ratio;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PaneId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ItemId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SplitId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FloatingWindowId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PanelTypeId(String);

impl PanelTypeId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceItem {
    pub id: ItemId,
    pub title: String,
    pub panel_type: PanelTypeId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pane {
    pub id: PaneId,
    pub items: Vec<WorkspaceItem>,
    pub active: Option<ItemId>,
}

impl Pane {
    #[must_use]
    pub const fn new(id: PaneId) -> Self {
        Self {
            id,
            items: Vec::new(),
            active: None,
        }
    }

    #[must_use]
    pub fn active_item(&self) -> Option<&WorkspaceItem> {
        self.active
            .and_then(|id| self.items.iter().find(|item| item.id == id))
    }
}

/// The main-axis direction of a split. `Row` splits left/right, `Column` splits top/bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SplitAxis {
    Row,
    Column,
}

impl SplitAxis {
    #[must_use]
    pub const fn for_side(side: Side) -> Self {
        match side {
            Side::Left | Side::Right => Self::Row,
            Side::Top | Side::Bottom => Self::Column,
        }
    }
}

/// One node of the recursive workspace layout tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WorkspaceNode {
    Leaf(PaneId),
    Split {
        id: SplitId,
        axis: SplitAxis,
        /// Fraction of the container taken by `first` in its main axis, clamped to `(0, 1)`.
        ratio: f32,
        first: Box<WorkspaceNode>,
        second: Box<WorkspaceNode>,
    },
}

impl WorkspaceNode {
    #[must_use]
    pub fn leaf(pane: PaneId) -> Self {
        Self::Leaf(pane)
    }

    #[must_use]
    pub fn split(id: SplitId, axis: SplitAxis, ratio: f32, first: Self, second: Self) -> Self {
        Self::Split {
            id,
            axis,
            ratio: sanitize_ratio(ratio, 0.5),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Depth-first pane traversal in authored order.
    #[must_use]
    pub fn panes(&self) -> Vec<PaneId> {
        let mut output = Vec::new();
        self.collect_panes(&mut output);
        output
    }

    fn collect_panes(&self, output: &mut Vec<PaneId>) {
        match self {
            Self::Leaf(pane) => output.push(*pane),
            Self::Split { first, second, .. } => {
                first.collect_panes(output);
                second.collect_panes(output);
            }
        }
    }
}

/// Logical top-left origin and size of a floating window, in host logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowPlacement {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl WindowPlacement {
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// A floating OS window that owns its own workspace tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FloatingWindow {
    pub id: FloatingWindowId,
    pub title: String,
    pub placement: WindowPlacement,
    pub root: WorkspaceNode,
}

/// Which container a dock or resize operation targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ContainerRef {
    Main,
    Floating(FloatingWindowId),
}

/// One of the four pane edges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

/// A semantic dock operation resolved by the host from a drag hit zone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DockAction {
    /// Merge into an existing pane's tab list at a slot expressed in the pre-move tab order.
    Merge { pane: PaneId, slot: usize },
    /// Split the target pane so the dragged item lands in a new pane on `side`.
    Split { pane: PaneId, side: Side },
    /// Split the whole container so the item lands in a new pane against the window edge.
    WindowSplit { container: ContainerRef, side: Side },
    /// Detach the item into a new floating window at the given placement.
    Float { placement: WindowPlacement },
}
