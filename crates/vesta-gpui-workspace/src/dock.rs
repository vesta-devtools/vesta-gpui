//! Shared dock geometry, drop-target resolution and drag transactions.
//!
//! Hosts own windows, pointer capture, painting and the workspace model they
//! mutate. This module owns the part that must not differ between hosts: which
//! [`DockAction`] a pointer position means, the drag threshold, the container
//! the pointer currently occupies, and the rectangles a host paints as a drop
//! preview. Keyboard mapping stays host-side because `vesta-gpui` has no
//! windowing dependency; hosts translate keys into the numeric helpers here.
//!
//! Both the GPUI acceptance host and the developer-agent desktop consume this,
//! so dragging a tab behaves the same in either window.

use vesta_gpui::{
    Color, Dimension, Element, ElementKey, ElementScope, FlowDirection, LayoutTree, Point, Rect,
    Size, Style,
};
use vesta_workspace::{
    ContainerRef, DockAction, FloatingWindowId, ItemId, PaneId, Side, SplitAxis, SplitId,
    WindowPlacement, WorkspaceModel, WorkspaceNode,
};

use crate::{WorkspaceViewRegions, geometry};

#[cfg(test)]
pub(crate) mod overflow_tests;
#[cfg(test)]
mod overlay_tests;
#[cfg(test)]
mod ownership_tests;

/// Pointer travel below this distance keeps a press a click instead of a drag.
pub const DOCK_DRAG_THRESHOLD: f32 = 6.0;

#[derive(Clone, Copy, Debug)]
pub struct WorkspaceTabDrag {
    pub item: ItemId,
    pub origin_container: ContainerRef,
    pub origin: Point,
    pub started: bool,
    pub pointer_container: Option<ContainerRef>,
    pub cursor: Point,
    pub target: Option<DockDropTarget>,
}

#[derive(Clone, Copy, Debug)]
pub struct FloatingMoveDrag {
    pub window: FloatingWindowId,
    pub origin: Point,
    pub placement: WindowPlacement,
}

#[derive(Clone, Copy, Debug)]
pub struct DockDropTarget {
    pub action: DockAction,
    pub preview: Rect,
    pub insertion: Option<Rect>,
}

#[derive(Clone, Copy, Debug)]
pub struct DockTabSplitRegion {
    pub side: Side,
    pub rect: Rect,
}

#[derive(Clone, Debug)]
pub struct DockDragVisual {
    pub target: Option<DockDropTarget>,
    pub cursor: Point,
    pub title: String,
}

#[derive(Clone, Copy, Debug)]
pub struct DockPaneFrame {
    pub pane: PaneId,
    pub container: ContainerRef,
    pub flow_direction: FlowDirection,
    pub root: Rect,
    pub tab_strip: Rect,
    pub body: Rect,
}

#[derive(Clone, Copy, Debug)]
pub struct DockTabFrame {
    pub pane: PaneId,
    /// Full current rectangle, including offscreen tabs needed for insertion order.
    pub rect: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DockSplitFrame {
    pub container: ContainerRef,
    pub split: SplitId,
    pub axis: SplitAxis,
    pub flow_direction: FlowDirection,
    pub rect: Rect,
}

/// Host-side rectangles for one window, resolved from a completed layout pass.
#[derive(Clone, Debug, Default)]
pub struct DockFrame {
    pub panes: Vec<DockPaneFrame>,
    pub tabs: Vec<DockTabFrame>,
    pub splits: Vec<DockSplitFrame>,
    pub workspace: Option<Rect>,
}

impl DockFrame {
    /// Require a complete matching layout; missing nodes never become a partial frame.
    pub fn from_layout(
        regions: &WorkspaceViewRegions,
        layout: &LayoutTree,
        workspace: Option<Rect>,
    ) -> Result<Self, vesta_gpui::LayoutError> {
        use vesta_gpui::LayoutError;
        if workspace.is_some_and(|rect| !geometry::valid_rect(rect)) {
            return Err(LayoutError::InvalidConstraint);
        }
        let node = |id| {
            let node = layout.node(id).ok_or(LayoutError::MissingLayout(id))?;
            if !geometry::valid_rect(node.rect) || !geometry::valid_rect(node.inherited_clip) {
                return Err(LayoutError::InvalidConstraint);
            }
            Ok(node)
        };
        let visible = |rect: Rect, clip: Rect| {
            rect.intersection(clip)
                .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0))
        };
        let panes = regions
            .panes
            .iter()
            .map(|region| {
                let root = node(region.root)?;
                node(region.tab_strip)?;
                let list = node(region.tab_list)?;
                let body = node(region.body)?;
                let viewport = list.scroll_viewport();
                if !geometry::valid_rect(viewport) {
                    return Err(LayoutError::InvalidConstraint);
                }
                Ok(DockPaneFrame {
                    pane: region.pane,
                    container: region.container,
                    flow_direction: root.flow_direction,
                    root: visible(root.rect, root.inherited_clip),
                    tab_strip: visible(viewport, list.inherited_clip),
                    body: visible(body.rect, body.inherited_clip),
                })
            })
            .collect::<Result<Vec<_>, LayoutError>>()?;
        let tabs = regions
            .tabs
            .iter()
            .map(|region| {
                Ok(DockTabFrame {
                    pane: region.pane,
                    rect: node(region.tab)?.rect,
                })
            })
            .collect::<Result<Vec<_>, LayoutError>>()?;
        let splits = regions
            .splits
            .iter()
            .map(|region| {
                Ok(DockSplitFrame {
                    container: region.container,
                    split: region.split,
                    axis: region.axis,
                    flow_direction: node(region.root)?.flow_direction,
                    rect: node(region.root)?.rect,
                })
            })
            .collect::<Result<Vec<_>, LayoutError>>()?;
        Ok(Self {
            panes,
            tabs,
            splits,
            workspace,
        })
    }

    #[must_use]
    pub fn split_rect(
        &self,
        container: ContainerRef,
        split: SplitId,
        axis: SplitAxis,
    ) -> Option<Rect> {
        self.split_frame(container, split, axis)
            .map(|frame| frame.rect)
    }

    #[must_use]
    pub fn split_frame(
        &self,
        container: ContainerRef,
        split: SplitId,
        axis: SplitAxis,
    ) -> Option<DockSplitFrame> {
        self.splits
            .iter()
            .find(|frame| {
                frame.container == container && frame.split == split && frame.axis == axis
            })
            .copied()
    }

    /// Resolve the drop a pointer position means inside this window.
    #[must_use]
    pub fn target(&self, point: Point, model: &WorkspaceModel) -> Option<DockDropTarget> {
        let workspace = self.workspace?;
        if !geometry::hittable_rect(workspace) || !geometry::finite_point(point) {
            return None;
        }
        for tab in self.tabs.iter().rev() {
            let Some(pane) = self.panes.iter().find(|pane| pane.pane == tab.pane) else {
                continue;
            };
            if !geometry::hittable_rect(tab.rect)
                || !geometry::hittable_rect(pane.tab_strip)
                || !geometry::hittable_rect(pane.root)
            {
                continue;
            }
            let split_regions = dock_tab_split_regions(tab.rect);
            if let Some(side) = split_regions
                .iter()
                .find(|region| {
                    region
                        .rect
                        .intersection(pane.tab_strip)
                        .is_some_and(|visible| visible.contains(point))
                })
                .map(|region| region.side)
            {
                return Some(DockDropTarget {
                    action: DockAction::Split {
                        pane: pane.pane,
                        side: authored_side(side, pane.flow_direction),
                    },
                    preview: dock_edge_preview(pane.root, side, 0.42),
                    insertion: None,
                });
            }
        }
        if !workspace.contains(point) {
            let width = 360.0_f32.min((workspace.width - 16.0).max(120.0));
            let height = 220.0_f32.min((workspace.height - 16.0).max(100.0));
            let x =
                (point.x - workspace.x - 60.0).clamp(8.0, (workspace.width - width - 8.0).max(8.0));
            let y = (point.y - workspace.y - 24.0)
                .clamp(8.0, (workspace.height - height - 8.0).max(8.0));
            return Some(DockDropTarget {
                action: DockAction::Float {
                    placement: WindowPlacement::new(x, y, width, height),
                },
                preview: Rect::new(workspace.x + x, workspace.y + y, width, height),
                insertion: None,
            });
        }
        if let Some(side) = dock_edge_band(workspace, point, 12.0) {
            return Some(DockDropTarget {
                action: DockAction::WindowSplit {
                    container: self.panes.first()?.container,
                    side: authored_side(side, self.panes.first()?.flow_direction),
                },
                preview: dock_edge_preview(workspace, side, 0.25),
                insertion: None,
            });
        }
        for pane in self.panes.iter().rev() {
            if geometry::hittable_rect(pane.tab_strip) && pane.tab_strip.contains(point) {
                let pane_tabs = self
                    .tabs
                    .iter()
                    .filter(|tab| tab.pane == pane.pane)
                    .collect::<Vec<_>>();
                let reversed = pane_tabs
                    .first()
                    .zip(pane_tabs.last())
                    .is_some_and(|(first, last)| first.rect.x > last.rect.x);
                let slot = pane_tabs
                    .iter()
                    .position(|tab| {
                        let midpoint = tab.rect.x + tab.rect.width * 0.5;
                        if reversed {
                            point.x > midpoint
                        } else {
                            point.x < midpoint
                        }
                    })
                    .unwrap_or(pane_tabs.len());
                let marker_x = pane_tabs
                    .get(slot)
                    .map_or_else(
                        || {
                            pane_tabs.last().map_or(pane.tab_strip.x + 4.0, |tab| {
                                if reversed {
                                    tab.rect.x
                                } else {
                                    tab.rect.right()
                                }
                            })
                        },
                        |tab| {
                            if reversed {
                                tab.rect.right()
                            } else {
                                tab.rect.x
                            }
                        },
                    )
                    .max(pane.tab_strip.x)
                    .min(pane.tab_strip.right());
                return Some(DockDropTarget {
                    action: DockAction::Merge {
                        pane: pane.pane,
                        slot,
                    },
                    preview: pane.tab_strip.inset(1.0),
                    insertion: Some(Rect::new(
                        marker_x - 2.0,
                        pane.tab_strip.y + 2.0,
                        4.0,
                        (pane.tab_strip.height - 4.0).max(0.0),
                    )),
                });
            }
        }
        for pane in self.panes.iter().rev() {
            if !geometry::hittable_rect(pane.root) || !pane.root.contains(point) {
                continue;
            }
            if pane.container != ContainerRef::Main
                && let Some(side) = dock_edge_band(pane.root, point, 18.0)
            {
                return Some(DockDropTarget {
                    action: DockAction::WindowSplit {
                        container: pane.container,
                        side: authored_side(side, pane.flow_direction),
                    },
                    preview: dock_edge_preview(pane.root, side, 0.25),
                    insertion: None,
                });
            }
            if let Some(side) = dock_edge(pane.body, point, 0.25) {
                return Some(DockDropTarget {
                    action: DockAction::Split {
                        pane: pane.pane,
                        side: authored_side(side, pane.flow_direction),
                    },
                    preview: dock_edge_preview(pane.root, side, 0.42),
                    insertion: None,
                });
            }
            if geometry::hittable_rect(pane.body) && pane.body.contains(point) {
                return Some(DockDropTarget {
                    action: DockAction::Merge {
                        pane: pane.pane,
                        slot: model.pane(pane.pane).map_or(0, |pane| pane.items.len()),
                    },
                    preview: pane.root.inset(5.0),
                    insertion: None,
                });
            }
        }
        None
    }
}

/// Split zones hugging each tab, so dropping near a tab edge splits its pane.
#[must_use]
pub fn dock_tab_split_regions(tab: Rect) -> [DockTabSplitRegion; 4] {
    let rect = Rect::new(tab.x, tab.y - 8.0, tab.width, tab.height + 16.0);
    let side_width = (rect.width * 0.25).clamp(24.0, 38.0).min(rect.width * 0.5);
    let middle_width = (rect.width - side_width * 2.0).max(0.0);
    let half_height = rect.height * 0.5;
    [
        DockTabSplitRegion {
            side: Side::Left,
            rect: Rect::new(rect.x, rect.y, side_width, rect.height),
        },
        DockTabSplitRegion {
            side: Side::Top,
            rect: Rect::new(rect.x + side_width, rect.y, middle_width, half_height),
        },
        DockTabSplitRegion {
            side: Side::Right,
            rect: Rect::new(rect.right() - side_width, rect.y, side_width, rect.height),
        },
        DockTabSplitRegion {
            side: Side::Bottom,
            rect: Rect::new(
                rect.x + side_width,
                rect.y + half_height,
                middle_width,
                rect.height - half_height,
            ),
        },
    ]
}

#[must_use]
pub fn dock_split_ratio(
    model: &WorkspaceModel,
    container: ContainerRef,
    split: SplitId,
) -> Option<f32> {
    fn find(node: &WorkspaceNode, split: SplitId) -> Option<f32> {
        match node {
            WorkspaceNode::Leaf(_) => None,
            WorkspaceNode::Split {
                id,
                ratio,
                first,
                second,
                ..
            } => (*id == split)
                .then_some(*ratio)
                .or_else(|| find(first, split))
                .or_else(|| find(second, split)),
        }
    }
    let root = match container {
        ContainerRef::Main => model.root(),
        ContainerRef::Floating(window) => {
            &model
                .floating_windows()
                .iter()
                .find(|floating| floating.id == window)?
                .root
        }
    };
    find(root, split)
}

/// Nearest edge when the pointer sits within a fraction of the rectangle.
#[must_use]
pub fn dock_edge(rect: Rect, point: Point, fraction: f32) -> Option<Side> {
    if !geometry::hittable_rect(rect)
        || !geometry::finite_point(point)
        || !fraction.is_finite()
        || !rect.contains(point)
    {
        return None;
    }
    let distances = [
        (Side::Top, (point.y - rect.y) / rect.height),
        (Side::Bottom, (rect.bottom() - point.y) / rect.height),
        (Side::Left, (point.x - rect.x) / rect.width),
        (Side::Right, (rect.right() - point.x) / rect.width),
    ];
    distances
        .into_iter()
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .and_then(|(side, distance)| (distance <= fraction).then_some(side))
}

/// Nearest edge when the pointer sits within a fixed band of the rectangle.
#[must_use]
pub fn dock_edge_band(rect: Rect, point: Point, band: f32) -> Option<Side> {
    if !geometry::hittable_rect(rect)
        || !geometry::finite_point(point)
        || !band.is_finite()
        || !rect.contains(point)
    {
        return None;
    }
    [
        (Side::Top, point.y - rect.y),
        (Side::Bottom, rect.bottom() - point.y),
        (Side::Left, point.x - rect.x),
        (Side::Right, rect.right() - point.x),
    ]
    .into_iter()
    .min_by(|left, right| left.1.total_cmp(&right.1))
    .and_then(|(side, distance)| (distance <= band).then_some(side))
}

#[must_use]
pub fn dock_edge_preview(rect: Rect, side: Side, fraction: f32) -> Rect {
    match side {
        Side::Left => Rect::new(rect.x, rect.y, rect.width * fraction, rect.height),
        Side::Right => Rect::new(
            rect.right() - rect.width * fraction,
            rect.y,
            rect.width * fraction,
            rect.height,
        ),
        Side::Top => Rect::new(rect.x, rect.y, rect.width, rect.height * fraction),
        Side::Bottom => Rect::new(
            rect.x,
            rect.bottom() - rect.height * fraction,
            rect.width,
            rect.height * fraction,
        ),
    }
}

/// Desktop physical bounds of one host window, with DPI applied only when a
/// client area is entered. Hosts build this from their own window system.
#[derive(Clone, Copy, Debug)]
pub struct DockWindowGeometry {
    pub container: ContainerRef,
    pub outer: Rect,
    pub client: Rect,
    pub scale: f32,
}

fn authored_side(side: Side, direction: FlowDirection) -> Side {
    match (side, direction) {
        (Side::Left, FlowDirection::RightToLeft) => Side::Right,
        (Side::Right, FlowDirection::RightToLeft) => Side::Left,
        _ => side,
    }
}

/// Start a tab drag. The press activates the tab; a drag begins only once the
/// pointer leaves the threshold while still inside the origin container.
pub fn begin_tab_drag(
    drag: &mut Option<WorkspaceTabDrag>,
    item: ItemId,
    origin_container: ContainerRef,
    origin: Point,
) {
    *drag = Some(WorkspaceTabDrag {
        item,
        origin_container,
        origin,
        started: false,
        pointer_container: Some(origin_container),
        cursor: origin,
        target: None,
    });
}

/// Feed a pointer position and its resolved target. Returns whether the drag
/// state changed and the host should repaint.
pub fn update_tab_drag(
    drag: &mut Option<WorkspaceTabDrag>,
    container: ContainerRef,
    cursor: Point,
    target: Option<DockDropTarget>,
) -> bool {
    let Some(mut current) = *drag else {
        return false;
    };
    let dx = cursor.x - current.origin.x;
    let dy = cursor.y - current.origin.y;
    if !current.started
        && container == current.origin_container
        && dx * dx + dy * dy < DOCK_DRAG_THRESHOLD * DOCK_DRAG_THRESHOLD
    {
        return false;
    }
    current.started = true;
    current.pointer_container = Some(container);
    current.cursor = cursor;
    current.target = target;
    *drag = Some(current);
    true
}

/// The pointer left a container; an active drag keeps running without a target
/// so it can enter another window. Returns whether the drag had started.
pub fn leave_tab_drag(drag: &mut Option<WorkspaceTabDrag>, container: ContainerRef) -> bool {
    let Some(mut current) = *drag else {
        return false;
    };
    if current.pointer_container != Some(container) {
        return false;
    }
    current.pointer_container = None;
    current.target = None;
    *drag = Some(current);
    current.started
}

/// Finish a drag. The host applies the returned action, and only when the drag
/// actually started and its source and resolved target still belong to the
/// containers that produced the gesture. A late release cannot reverse a move.
pub fn finish_tab_drag(
    drag: &mut Option<WorkspaceTabDrag>,
    model: &WorkspaceModel,
) -> Option<(ItemId, DockAction)> {
    let drag = drag.take()?;
    if drag.started
        && model.container_of_item(drag.item) == Some(drag.origin_container)
        && let Some(target) = drag.target
        && drag
            .pointer_container
            .is_some_and(|container| current_dock_target(target, container, model))
    {
        return Some((drag.item, target.action));
    }
    None
}

fn current_dock_target(
    target: DockDropTarget,
    container: ContainerRef,
    model: &WorkspaceModel,
) -> bool {
    let exists = match container {
        ContainerRef::Main => true,
        ContainerRef::Floating(id) => model.floating_window(id).is_some(),
    };
    exists
        && match target.action {
            DockAction::Merge { pane, .. } | DockAction::Split { pane, .. } => {
                model.container_of_pane(pane) == Some(container)
            }
            DockAction::WindowSplit {
                container: target, ..
            } => target == container,
            DockAction::Float { .. } => true,
        }
}

/// The preview a container paints while it currently holds a started drag.
#[must_use]
pub fn tab_drag_visual(
    drag: &Option<WorkspaceTabDrag>,
    model: &WorkspaceModel,
    container: ContainerRef,
) -> Option<DockDragVisual> {
    let drag = drag.filter(|drag| {
        drag.started && model.container_of_item(drag.item) == Some(drag.origin_container)
    })?;
    if drag.pointer_container != Some(container) {
        return None;
    }
    let item = model.item(drag.item)?;
    Some(DockDragVisual {
        target: drag
            .target
            .filter(|target| current_dock_target(*target, container, model)),
        cursor: drag.cursor,
        title: item.title.clone(),
    })
}

/// Project a drag preview: the pending drop rectangle, the tab insertion marker
/// and the dragged title following the cursor. Hosts parent it in their own
/// overlay scope so element identities stay inside one window.
#[must_use]
pub fn dock_drag_overlay(
    scope: ElementScope,
    drag: &DockDragVisual,
    theme: vesta_gpui_widgets::WidgetTheme,
    viewport: Size,
) -> Element {
    let mut overlay = Element::container(scope.element(100), ElementKey::new("dock.drag.overlay"))
        .styled(
            Style::default()
                .absolute(0.0, 0.0)
                .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                .pointer_events(false),
        );
    if let Some(target) = drag.target {
        let preview = target.preview;
        overlay = overlay.child(
            Element::container(scope.element(101), ElementKey::new("dock.drag.preview")).styled(
                Style::default()
                    .absolute(preview.x, preview.y)
                    .size(Dimension::Px(preview.width), Dimension::Px(preview.height))
                    .background(Color::srgba(
                        theme.palette.focus.red,
                        theme.palette.focus.green,
                        theme.palette.focus.blue,
                        0.16,
                    ))
                    .border(2.0, theme.palette.focus)
                    .radius(3.0)
                    .z_index(140)
                    .pointer_events(false),
            ),
        );
        if let Some(insertion) = target.insertion {
            overlay = overlay.child(
                Element::container(scope.element(107), ElementKey::new("dock.drag.insertion"))
                    .styled(
                        Style::default()
                            .absolute(insertion.x, insertion.y)
                            .size(
                                Dimension::Px(insertion.width),
                                Dimension::Px(insertion.height),
                            )
                            .background(theme.palette.focus)
                            .z_index(150)
                            .pointer_events(false),
                    ),
            );
        }
    }
    let ghost_x = (drag.cursor.x + 10.0).clamp(0.0, (viewport.width - 150.0).max(0.0));
    let ghost_y = (drag.cursor.y + 12.0).clamp(0.0, (viewport.height - 28.0).max(0.0));
    overlay.child(
        Element::text(
            scope.element(102),
            ElementKey::new("dock.drag.ghost"),
            format!("  {}", drag.title),
        )
        .styled(
            Style::default()
                .absolute(ghost_x, ghost_y)
                .size(Dimension::Px(150.0), Dimension::Px(28.0))
                .background(theme.palette.overlay)
                .border(1.0, theme.palette.focus)
                .radius(3.0)
                .text(12.0, theme.palette.text)
                .z_index(200)
                .pointer_events(false),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cancel_tab_drag, finish_tab_drag_from, finish_tab_drag_with_admission, route_dock_pointer,
        route_dock_pointer_with_admission,
    };
    use vesta_workspace::{Pane, PanelTypeId};

    const PANE: PaneId = PaneId(1);
    const FLOATING: FloatingWindowId = FloatingWindowId(1);

    fn model(items: usize) -> (WorkspaceModel, Vec<ItemId>) {
        let mut model =
            WorkspaceModel::with_tree(WorkspaceNode::leaf(PANE), [Pane::new(PANE)]).expect("model");
        let ids = (0..items)
            .map(|index| {
                model
                    .add_item(PANE, format!("Tab {index}"), PanelTypeId::new("test.panel"))
                    .expect("item")
            })
            .collect();
        (model, ids)
    }

    /// One 800x600 pane: 28px tab strip with two 120px tabs, body below.
    fn frame() -> DockFrame {
        DockFrame {
            panes: vec![DockPaneFrame {
                flow_direction: vesta_gpui::FlowDirection::LeftToRight,
                pane: PANE,
                container: ContainerRef::Main,
                root: Rect::new(0.0, 0.0, 800.0, 600.0),
                tab_strip: Rect::new(0.0, 0.0, 800.0, 28.0),
                body: Rect::new(0.0, 28.0, 800.0, 572.0),
            }],
            tabs: vec![
                DockTabFrame {
                    pane: PANE,
                    rect: Rect::new(0.0, 0.0, 120.0, 28.0),
                },
                DockTabFrame {
                    pane: PANE,
                    rect: Rect::new(120.0, 0.0, 120.0, 28.0),
                },
            ],
            splits: Vec::new(),
            workspace: Some(Rect::new(0.0, 0.0, 800.0, 600.0)),
        }
    }

    #[test]
    fn drop_zones_resolve_split_float_window_split_and_merge() {
        overlay_tests::qualify();
        let (model, _) = model(2);
        let frame = frame();
        let action = |point: Point| frame.target(point, &model).map(|target| target.action);

        // Tab edges split that tab's pane; the tab centre merges into its slot.
        assert_eq!(
            action(Point::new(4.0, 14.0)),
            Some(DockAction::Split {
                pane: PANE,
                side: Side::Left
            })
        );
        assert_eq!(
            action(Point::new(116.0, 14.0)),
            Some(DockAction::Split {
                pane: PANE,
                side: Side::Right
            })
        );
        // A tab's middle column splits above/below; empty strip space merges.
        assert_eq!(
            action(Point::new(180.0, 6.0)),
            Some(DockAction::Split {
                pane: PANE,
                side: Side::Top
            })
        );
        assert_eq!(
            action(Point::new(300.0, 14.0)),
            Some(DockAction::Merge {
                pane: PANE,
                slot: 2
            })
        );
        let insertion = frame
            .target(Point::new(300.0, 14.0), &model)
            .and_then(|target| target.insertion);
        assert!(
            insertion.is_some(),
            "the tab strip shows an insertion marker"
        );

        // The pane body merges at the end, its edges split the pane.
        assert_eq!(
            action(Point::new(400.0, 300.0)),
            Some(DockAction::Merge {
                pane: PANE,
                slot: 2
            })
        );
        assert_eq!(
            action(Point::new(150.0, 300.0)),
            Some(DockAction::Split {
                pane: PANE,
                side: Side::Left
            })
        );

        // The workspace edge band splits the window; outside it becomes a float.
        assert_eq!(
            action(Point::new(4.0, 300.0)).map(|action| match action {
                DockAction::WindowSplit { container, side } => Some((container, side)),
                _ => None,
            }),
            Some(Some((ContainerRef::Main, Side::Left)))
        );
        assert!(
            matches!(
                action(Point::new(-40.0, 300.0)),
                Some(DockAction::Float { .. })
            ),
            "a pointer outside the workspace tears the tab off"
        );
        assert!(frame.target(Point::new(-40.0, 300.0), &model).is_some());
    }

    #[test]
    fn a_press_only_becomes_a_drag_after_travel_and_then_tracks_the_pointer() {
        let (model, items) = model(2);
        let item = items[1];
        let mut drag = None;
        begin_tab_drag(&mut drag, item, ContainerRef::Main, Point::new(100.0, 14.0));
        let started = drag.expect("drag");
        assert_eq!(started.pointer_container, Some(ContainerRef::Main));
        assert!(!started.started);

        // A small move inside the origin container stays a click.
        assert!(!update_tab_drag(
            &mut drag,
            ContainerRef::Main,
            Point::new(103.0, 15.0),
            None
        ));
        assert!(!drag.expect("drag").started);
        assert!(tab_drag_visual(&drag, &model, ContainerRef::Main).is_none());

        // Crossing the threshold starts the drag and carries the resolved target.
        let target = frame().target(Point::new(400.0, 300.0), &model);
        assert!(update_tab_drag(
            &mut drag,
            ContainerRef::Main,
            Point::new(400.0, 300.0),
            target
        ));
        let visual = tab_drag_visual(&drag, &model, ContainerRef::Main).expect("visual");
        assert_eq!(visual.title, "Tab 1");
        assert!(visual.target.is_some());
        assert!(tab_drag_visual(&drag, &model, ContainerRef::Floating(FLOATING)).is_none());

        let accepted = drag;
        assert!(
            finish_tab_drag_from(
                &mut drag,
                ContainerRef::Floating(FLOATING),
                &model,
                |_| panic!("foreign release must not consult the owning display"),
            )
            .is_none()
        );
        assert!(
            drag.expect("foreign release retains source")
                .target
                .is_some()
        );
        assert!(finish_tab_drag_from(&mut drag, ContainerRef::Main, &model, |_| true).is_some());
        assert!(finish_tab_drag_from(&mut drag, ContainerRef::Main, &model, |_| true).is_none());
        drag = accepted;
        assert!(!cancel_tab_drag(
            &mut drag,
            ContainerRef::Floating(FLOATING)
        ));
        assert!(drag.expect("unrelated source remains").target.is_some());
        assert!(cancel_tab_drag(&mut drag, ContainerRef::Main));
        assert!(
            finish_tab_drag(&mut drag, &model).is_none(),
            "retired source cannot commit"
        );
        assert!(!cancel_tab_drag(&mut drag, ContainerRef::Main));
        drag = accepted;

        assert!(finish_tab_drag_with_admission(&mut drag, &model, |_| false).is_none());
        assert!(
            drag.is_none(),
            "blocked release consumes the source without an action"
        );
        drag = accepted;
        drag.as_mut().expect("source").pointer_container = Some(ContainerRef::Floating(FLOATING));
        assert!(
            finish_tab_drag_with_admission(&mut drag, &model, |container| container
                == ContainerRef::Main)
            .is_none()
        );
        assert!(
            drag.is_none(),
            "destination suspension after the last move rejects release"
        );
        drag = accepted;
        assert!(finish_tab_drag_with_admission(&mut drag, &model, |_| true).is_some());
        assert!(drag.is_none(), "admitted release is once-only");
        drag = accepted;
        drag.as_mut().expect("live source").pointer_container =
            Some(ContainerRef::Floating(FLOATING));
        assert!(cancel_tab_drag(&mut drag, ContainerRef::Floating(FLOATING)));
        let retained = drag.expect("source remains active after destination suspension");
        assert_eq!(retained.origin_container, ContainerRef::Main);
        assert_eq!(retained.pointer_container, None);
        assert!(retained.target.is_none());
        assert!(
            finish_tab_drag(&mut drag, &model).is_none(),
            "stale destination cannot commit"
        );
        drag = accepted;

        // Leaving the container keeps the drag alive without a target.
        assert!(leave_tab_drag(&mut drag, ContainerRef::Main));
        assert!(tab_drag_visual(&drag, &model, ContainerRef::Main).is_none());
        // Entering another container starts immediately, even without travel.
        assert!(update_tab_drag(
            &mut drag,
            ContainerRef::Floating(FLOATING),
            Point::new(106.0, 16.0),
            None
        ));
        assert_eq!(
            drag.expect("drag").pointer_container,
            Some(ContainerRef::Floating(FLOATING))
        );
        let target = frame().target(Point::new(400.0, 300.0), &model);
        let (model, target) = ownership_tests::floating_destination(
            &model,
            items[0],
            target.expect("resolved destination"),
        );
        assert!(update_tab_drag(
            &mut drag,
            ContainerRef::Floating(FLOATING),
            Point::new(206.0, 216.0),
            Some(target)
        ));
        let (finished, action) = finish_tab_drag(&mut drag, &model).expect("drop");
        assert_eq!(finished, item);
        assert!(matches!(action, DockAction::Merge { .. }));
        assert!(drag.is_none());
    }

    #[test]
    fn a_drop_without_travel_target_or_a_live_item_is_cancelled() {
        ownership_tests::qualify();
        let (model, items) = model(1);
        let mut drag = None;
        begin_tab_drag(
            &mut drag,
            items[0],
            ContainerRef::Main,
            Point::new(50.0, 14.0),
        );
        assert!(
            finish_tab_drag(&mut drag, &model).is_none(),
            "a press without travel never docks"
        );
        assert!(drag.is_none(), "the finished transaction is consumed");

        begin_tab_drag(
            &mut drag,
            items[0],
            ContainerRef::Main,
            Point::new(50.0, 14.0),
        );
        assert!(update_tab_drag(
            &mut drag,
            ContainerRef::Main,
            Point::new(400.0, 300.0),
            None
        ));
        assert!(
            finish_tab_drag(&mut drag, &model).is_none(),
            "a drag without a resolved target docks nothing"
        );

        let mut drag = None;
        begin_tab_drag(
            &mut drag,
            ItemId(999),
            ContainerRef::Main,
            Point::new(50.0, 14.0),
        );
        let target = frame().target(Point::new(400.0, 300.0), &model);
        assert!(update_tab_drag(
            &mut drag,
            ContainerRef::Main,
            Point::new(400.0, 300.0),
            target
        ));
        assert!(
            finish_tab_drag(&mut drag, &model).is_none(),
            "a vanished item is not docked"
        );
    }

    #[test]
    fn routing_prefers_the_front_window_and_respects_native_decoration() {
        let (model, _) = model(1);
        let source = DockWindowGeometry {
            container: ContainerRef::Main,
            outer: Rect::new(0.0, 0.0, 800.0, 600.0),
            client: Rect::new(0.0, 0.0, 800.0, 600.0),
            scale: 1.0,
        };
        // The floating window sits in front, shifted right and scaled.
        let floating = DockWindowGeometry {
            container: ContainerRef::Floating(FLOATING),
            outer: Rect::new(500.0, 50.0, 400.0, 300.0),
            client: Rect::new(508.0, 82.0, 384.0, 260.0),
            scale: 1.25,
        };
        let frame = frame();
        let windows = vec![(floating, &frame), (source, &frame)];

        for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(
                route_dock_pointer(
                    DockWindowGeometry { scale, ..source },
                    Point::new(600.0, 150.0),
                    &windows,
                    &model,
                )
                .is_none(),
                "invalid source scale {scale} cannot choose a Dock destination"
            );
            assert!(
                route_dock_pointer(
                    source,
                    Point::new(600.0, 150.0),
                    &[
                        (DockWindowGeometry { scale, ..floating }, &frame),
                        (source, &frame)
                    ],
                    &model,
                )
                .is_none(),
                "invalid foreground scale {scale} must not fall through to Main"
            );
        }
        for rect in [
            Rect::new(508.0, 82.0, 0.0, 260.0),
            Rect::new(508.0, 82.0, -1.0, 260.0),
            Rect::new(508.0, 82.0, f32::INFINITY, 260.0),
            Rect::new(f32::NAN, 82.0, 384.0, 260.0),
        ] {
            assert!(
                route_dock_pointer(
                    DockWindowGeometry {
                        client: rect,
                        ..source
                    },
                    Point::new(600.0, 150.0),
                    &windows,
                    &model,
                )
                .is_none(),
                "invalid source client geometry cannot route or use tear-off fallback"
            );
            assert!(
                route_dock_pointer(
                    source,
                    Point::new(600.0, 150.0),
                    &[
                        (
                            DockWindowGeometry {
                                outer: rect,
                                ..floating
                            },
                            &frame
                        ),
                        (source, &frame)
                    ],
                    &model,
                )
                .is_none(),
                "unknown foreground bounds must not route through to Main"
            );
        }
        assert!(
            route_dock_pointer(
                source,
                Point::new(600.0, 150.0),
                &[(
                    DockWindowGeometry {
                        scale: f32::MIN_POSITIVE,
                        ..floating
                    },
                    &frame
                )],
                &model,
            )
            .is_none(),
            "a positive scale that overflows logical coordinates cannot publish a target"
        );

        // A pointer over the floating client resolves to that container and scale.
        let route =
            route_dock_pointer(source, Point::new(600.0, 150.0), &windows, &model).expect("route");
        assert_eq!(route.0, ContainerRef::Floating(FLOATING));
        let expected = Point::new((600.0 - 508.0) / 1.25, (150.0 - 82.0) / 1.25);
        assert!((route.1.x - expected.x).abs() < 0.01, "{:?}", route.1);
        assert!((route.1.y - expected.y).abs() < 0.01, "{:?}", route.1);

        let shifted = |mut geometry: DockWindowGeometry| {
            geometry.outer.x -= 1200.0;
            geometry.outer.y -= 800.0;
            geometry.client.x -= 1200.0;
            geometry.client.y -= 800.0;
            geometry
        };
        let route = route_dock_pointer(
            shifted(source),
            Point::new(600.0, 150.0),
            &[(shifted(floating), &frame), (shifted(source), &frame)],
            &model,
        )
        .expect("negative desktop origins remain valid");
        assert_eq!(route.0, ContainerRef::Floating(FLOATING));
        assert_eq!(route.1, expected);
        let route = route_dock_pointer(
            DockWindowGeometry {
                scale: 1.5,
                ..source
            },
            Point::new(400.0, 100.0),
            &[(
                DockWindowGeometry {
                    scale: 2.0,
                    ..floating
                },
                &frame,
            )],
            &model,
        )
        .expect("positive mixed-DPI transform");
        assert_eq!(route.0, ContainerRef::Floating(FLOATING));
        assert_eq!(route.1, Point::new(46.0, 34.0));

        // A pointer on the native title bar is occluded instead of routing.
        assert!(
            route_dock_pointer(source, Point::new(502.0, 60.0), &windows, &model).is_none(),
            "native decoration does not fall through to the window behind it"
        );

        // With no window under the pointer the source container still resolves.
        let route = route_dock_pointer(source, Point::new(400.0, 300.0), &windows, &model)
            .expect("fallback");
        assert_eq!(route.0, ContainerRef::Main);
        assert_eq!(route.1, Point::new(400.0, 300.0));

        assert!(
            route_dock_pointer_with_admission(
                source,
                Point::new(600.0, 150.0),
                &windows,
                &model,
                |container| container == ContainerRef::Main,
            )
            .is_none(),
            "a blocked foreground display cannot route through to Main"
        );
        assert!(
            route_dock_pointer_with_admission(
                source,
                Point::new(600.0, 150.0),
                &windows,
                &model,
                |container| container != ContainerRef::Main,
            )
            .is_none(),
            "a blocked source cannot route into an admitted destination"
        );
        assert!(
            route_dock_pointer_with_admission(
                source,
                Point::new(-200.0, 900.0),
                &windows,
                &model,
                |_| false,
            )
            .is_none(),
            "a blocked source cannot use the off-window fallback"
        );
        assert!(
            route_dock_pointer_with_admission(
                source,
                Point::new(-200.0, 900.0),
                &windows,
                &model,
                |container| container == ContainerRef::Main,
            )
            .is_some(),
            "an admitted source keeps the existing tear-off fallback"
        );
    }
}
