//! Controlled native raster sample; no pointer capture or Dock gesture is created.
use super::*;

pub(super) const CASE: &str = "observe-dock-visual";

pub(super) fn sample(
    workspace: &WorkspaceModel,
    container: ContainerRef,
) -> Option<DockDragVisual> {
    let item = container_panes(workspace, container)
        .into_iter()
        .find_map(|pane| workspace.pane(pane).and_then(|pane| pane.active))?;
    Some(DockDragVisual {
        target: None,
        cursor: Point::new(70.0, 110.0),
        title: workspace.item(item)?.title.clone(),
    })
}
