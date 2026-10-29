//! Extends the existing Workspace overflow composite; no duplicate test entry.
use super::*;
use crate::{
    PaneHeader, PaneHeaderCommand, WorkspaceView, WorkspaceViewInteraction,
    workspace_view_with_header,
};
use vesta_gpui::{ElementScope, LayoutEngine, ScrollbarGutter, ScrollbarStyle, Viewport};
use vesta_gpui_widgets::WidgetTheme;

fn frame(
    model: &WorkspaceModel,
    scope: ElementScope,
    states: &HashMap<PaneId, ScrollAreaState>,
    direction: FlowDirection,
    width: f32,
) -> (WorkspaceView, LayoutTree) {
    let mut view = workspace_view_with_header(
        scope,
        model,
        WidgetTheme::dark(),
        WorkspaceViewInteraction::default(),
        None,
        |pane| PaneHeader {
            navigation: Some((false, false)),
            commands: vec![PaneHeaderCommand {
                id: 1,
                caption: "+".into(),
                name: "New tab".into(),
                enabled: true,
            }],
            tab_scroll_x: states.get(&pane.id).map_or(0.0, |s| s.offset().x),
            tab_scrollbar: Some(ScrollbarStyle {
                gutter: ScrollbarGutter::Reserve,
                ..ScrollbarStyle::default()
            }),
            ..PaneHeader::default()
        },
        |_| None,
    )
    .expect("workspace");
    view.element.style = view.element.style.flow_direction(direction);
    let layout = LayoutEngine
        .compute(&view.element, Viewport::new(width, 240.0, 1.0))
        .expect("layout");
    (view, layout)
}

pub(crate) fn qualify(model: &WorkspaceModel, scope: ElementScope) {
    for direction in [FlowDirection::LeftToRight, FlowDirection::RightToLeft] {
        let mut model = model.clone();
        let mut states = HashMap::<PaneId, ScrollAreaState>::new();
        let mut previous = HashMap::<PaneId, TabRevealGeometry>::new();
        for width in [320.0, 180.0, 440.0] {
            let (view, layout) = frame(&model, scope, &states, direction, width);
            let region = view.regions.panes[0];
            states
                .entry(region.pane)
                .or_default()
                .sync_layout(layout.node(region.tab_list).expect("tab scroll viewport"));
            reveal_active_tabs(
                &view.regions,
                &layout,
                &model,
                &mut states,
                &mut previous,
                |p| p.pane,
            );
            let (view, layout) = frame(&model, scope, &states, direction, width);
            let visible = layout
                .node(region.tab_list)
                .expect("tab list")
                .scroll_viewport();
            let active = model
                .pane(region.pane)
                .expect("pane")
                .active
                .expect("active");
            let tab = view
                .regions
                .tabs
                .iter()
                .find(|r| r.item == active)
                .expect("active tab");
            let rect = layout.node(tab.tab).expect("active layout").rect;
            assert!(
                rect.x < visible.right() && rect.right() > visible.x,
                "active tab must be visible after selection/reflow: {direction:?}"
            );
            if rect.width < visible.width {
                assert!(visible.contains_rect(rect));
            } else {
                let difference = match direction {
                    FlowDirection::LeftToRight => rect.x - visible.x,
                    FlowDirection::RightToLeft => rect.right() - visible.right(),
                };
                assert!(
                    difference.abs() < 0.001,
                    "overwide leading edge: {difference}"
                );
            }
            states
                .get_mut(&region.pane)
                .expect("state")
                .scroll_by_point(Point::new(direction.horizontal(-20.0), 0.0));
            let manual = states[&region.pane].offset();
            let (view, layout) = frame(&model, scope, &states, direction, width);
            assert!(!reveal_active_tabs(
                &view.regions,
                &layout,
                &model,
                &mut states,
                &mut previous,
                |p| p.pane
            ));
            assert_eq!(
                states[&region.pane].offset(),
                manual,
                "unchanged selection/geometry preserves manual movement"
            );
        }
        let first = model.pane(PaneId(1)).expect("pane").items[0].id;
        model.activate(first).expect("select earlier tab");
        let (view, layout) = frame(&model, scope, &states, direction, 440.0);
        assert!(reveal_active_tabs(
            &view.regions,
            &layout,
            &model,
            &mut states,
            &mut previous,
            |p| p.pane
        ));
        assert_eq!(states[&PaneId(1)].offset().x, 0.0);
        previous.insert(PaneId(99), previous[&PaneId(1)]);
        let mut unrelated = ScrollAreaState::new(0.0);
        unrelated.sync_content_size(Size::new(1000.0, 50.0), Size::new(100.0, 50.0));
        unrelated.scroll_by_point(Point::new(37.0, 0.0));
        states.insert(PaneId(99), unrelated);
        let before = states[&PaneId(99)].offset();
        reveal_active_tabs(
            &view.regions,
            &layout,
            &model,
            &mut states,
            &mut previous,
            |p| p.pane,
        );
        assert!(!previous.contains_key(&PaneId(99)));
        assert_eq!(
            states[&PaneId(99)].offset(),
            before,
            "another host/window's retained scroll is not reset"
        );
    }
}
