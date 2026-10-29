//! The actual Host call must not shadow shared Dock presentation with a stale copy.
use super::*;
use vesta_gpui::PaintList;

pub(super) fn host_overlay_matches_shared_authority() {
    let app = App::new();
    for scope in [app.scopes.root, UiScopes::new().root] {
        for theme in [WidgetTheme::light(), WidgetTheme::dark()] {
            let drag = DockDragVisual {
                target: None,
                cursor: Point::new(120.0, 90.0),
                title: "Controls".into(),
            };
            let size = Size::new(640.0, 480.0);
            let host = dock_drag_overlay(scope, &drag, theme, size);
            let shared = vesta_gpui_workspace::dock_drag_overlay(scope, &drag, theme, size);
            let viewport = Viewport::new(size.width, size.height, 1.25);
            let host_layout = LayoutEngine.compute(&host, viewport).expect("Host overlay");
            let shared_layout = LayoutEngine
                .compute(&shared, viewport)
                .expect("shared overlay");
            for node in host_layout.nodes() {
                assert_eq!(
                    node.rect,
                    shared_layout
                        .node(node.id)
                        .expect("same scoped identity")
                        .rect,
                    "Host presentation preserves shared Dock geometry"
                );
            }
            assert!(
                PaintList::build(&host, &host_layout) == PaintList::build(&shared, &shared_layout),
                "Host Dock overlay diverges from shared {:?} presentation",
                theme.mode
            );
        }
    }
}
