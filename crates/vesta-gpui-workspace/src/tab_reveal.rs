//! Active-tab reveal policy shared by Workspace hosts, using their existing scroll states.
use std::collections::HashMap;
use std::hash::Hash;

use vesta_gpui::{FlowDirection, LayoutTree, Point, Size};
use vesta_gpui_widgets::ScrollAreaState;
use vesta_workspace::{ItemId, PaneId, WorkspaceModel};

use crate::{WorkspacePaneRegion, WorkspaceViewRegions};

#[cfg(test)]
pub(crate) mod tests;

/// Transient checkpoint for one pane in one owning host/window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TabRevealGeometry {
    active: ItemId,
    position: usize,
    visible: Size,
    content_width: f32,
}

/// Reveal after selection or reflow, without undoing a user's wheel/thumb movement.
/// `state_key` adapts an existing host map keyed by PaneId or scoped ElementId.
pub fn reveal_active_tabs<K: Eq + Hash>(
    regions: &WorkspaceViewRegions,
    layout: &LayoutTree,
    model: &WorkspaceModel,
    states: &mut HashMap<K, ScrollAreaState>,
    previous: &mut HashMap<PaneId, TabRevealGeometry>,
    mut state_key: impl FnMut(&WorkspacePaneRegion) -> K,
) -> bool {
    previous.retain(|pane, _| regions.panes.iter().any(|region| region.pane == *pane));
    let mut changed = false;
    for region in &regions.panes {
        let Some(active) = model.pane(region.pane).and_then(|pane| pane.active) else {
            continue;
        };
        let Some(list) = layout.node(region.tab_list) else {
            continue;
        };
        let visible = list.scroll_viewport();
        let geometry = TabRevealGeometry {
            active,
            position: model
                .pane(region.pane)
                .expect("live pane")
                .items
                .iter()
                .position(|item| item.id == active)
                .expect("active item"),
            visible: Size::new(visible.width, visible.height),
            content_width: list.scroll_content.unwrap_or_default().0,
        };
        let Some(tab) = regions
            .tabs
            .iter()
            .find(|tab| tab.pane == region.pane && tab.item == active)
            .and_then(|tab| layout.node(tab.tab))
        else {
            continue;
        };
        if previous.insert(region.pane, geometry) == Some(geometry) {
            continue;
        }
        let state = states.entry(state_key(region)).or_default();
        if tab.rect.width + 4.0 >= visible.width {
            let old = state.offset();
            let delta = match list.flow_direction {
                FlowDirection::LeftToRight => tab.rect.x - visible.x,
                FlowDirection::RightToLeft => tab.rect.right() - visible.right(),
            };
            state.scroll_by_point(Point::new(delta, 0.0));
            changed |= old != state.offset();
        } else {
            changed |= state.reveal(tab.rect, visible, 2.0);
        }
    }
    changed
}
