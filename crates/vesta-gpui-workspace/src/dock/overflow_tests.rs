//! Actual overflow layout must retain model order when choosing a drop slot.
use super::*;
use crate::{
    PaneHeader, PaneHeaderCommand, WorkspaceViewAction, WorkspaceViewInteraction,
    workspace_view_with_header,
};
use vesta_gpui::{FlowDirection, LayoutEngine, ScrollbarGutter, ScrollbarStyle, Viewport};
use vesta_gpui_widgets::{ScrollAreaState, WidgetTheme};

pub(crate) fn qualify(model: &WorkspaceModel, scope: ElementScope) {
    for direction in [FlowDirection::LeftToRight, FlowDirection::RightToLeft] {
        qualify_direction(model, scope, direction);
    }
}

fn qualify_direction(model: &WorkspaceModel, scope: ElementScope, direction: FlowDirection) {
    let project = |offset| {
        let mut view = workspace_view_with_header(
            scope,
            model,
            WidgetTheme::dark(),
            WorkspaceViewInteraction::default(),
            None,
            |_| PaneHeader {
                navigation: Some((false, false)),
                commands: vec![PaneHeaderCommand {
                    id: 1,
                    caption: "+".into(),
                    name: "New tab".into(),
                    enabled: true,
                }],
                tab_scroll_x: offset,
                tab_scrollbar: Some(ScrollbarStyle {
                    gutter: ScrollbarGutter::Reserve,
                    ..ScrollbarStyle::default()
                }),
                ..PaneHeader::default()
            },
            |_| None,
        )
        .expect("scrolled projection");
        view.element.style = view.element.style.flow_direction(direction);
        view
    };
    let initial = project(0.0);
    let initial_layout = LayoutEngine
        .compute(&initial.element, Viewport::new(320.0, 240.0, 1.0))
        .expect("initial layout");
    let mut scroll = ScrollAreaState::default();
    scroll.sync_layout(
        initial_layout
            .node(initial.regions.panes[0].tab_list)
            .expect("initial tabs"),
    );
    scroll.scroll_by_point(Point::new(direction.horizontal(f32::MAX), 0.0));
    let view = project(scroll.offset().x);
    let layout = LayoutEngine
        .compute(&view.element, Viewport::new(320.0, 240.0, 1.0))
        .expect("scrolled layout");
    let pane = model.pane(PaneId(1)).expect("pane");
    let list = layout
        .node(view.regions.panes[0].tab_list)
        .expect("tab list");
    let viewport = list.scroll_viewport();
    let close = view
        .actions
        .iter()
        .find(|(_, action)| {
            *action == WorkspaceViewAction::CloseTab(pane.items.last().expect("last item").id)
        })
        .map(|(element, _)| *element)
        .expect("last close action");
    let close = layout
        .node(close)
        .expect("close geometry")
        .rect
        .intersection(viewport)
        .expect("last close visible");
    let point = Point::new(close.x + close.width * 0.5, close.y + close.height * 0.5);
    let frame = DockFrame::from_layout(
        &view.regions,
        &layout,
        Some(Rect::new(0.0, 0.0, 320.0, 240.0)),
    )
    .expect("complete scrolled frame");
    assert!(
        layout
            .node(view.regions.tabs[0].tab)
            .expect("first tab")
            .rect
            .intersection(viewport)
            .is_none(),
        "fixture must exercise hidden preceding tabs"
    );
    assert_eq!(
        frame.target(point, model).map(|target| target.action),
        Some(DockAction::Merge {
            pane: pane.id,
            slot: pane.items.len(),
        }),
        "dropping after the last tab uses the full current order: {direction:?}"
    );
    let track = list
        .scrollbar_geometry(scroll.offset())
        .2
        .expect("scrollbar row");
    assert!(
        frame
            .target(
                Point::new(track.x + track.width * 0.5, track.y + track.height * 0.5),
                model
            )
            .is_none(),
        "tab split zones cannot claim the reserved scrollbar row: {direction:?}"
    );
    let first = pane.items[0].id;
    let mut changed = model.clone();
    changed
        .dock_item(
            first,
            frame.target(point, model).expect("drop target").action,
        )
        .expect("apply current target");
    let actual = changed
        .pane(pane.id)
        .expect("same pane")
        .items
        .iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    let expected = pane
        .items
        .iter()
        .skip(1)
        .map(|item| item.id)
        .chain([first])
        .collect::<Vec<_>>();
    assert_eq!(
        actual, expected,
        "one atomic Dock action preserves exact item order"
    );
    for (x, side) in [
        (1.0, Side::Left),
        (319.0, Side::Right),
        (20.0, Side::Left),
        (300.0, Side::Right),
    ] {
        let target = frame
            .target(Point::new(x, 120.0), model)
            .expect("physical side target");
        let mut changed = model.clone();
        changed
            .dock_item(first, target.action)
            .expect("dock at physical side");
        let moved = changed
            .root()
            .panes()
            .into_iter()
            .find(|id| {
                changed
                    .pane(*id)
                    .expect("pane")
                    .items
                    .iter()
                    .any(|item| item.id == first)
            })
            .expect("moved pane");
        let mut projected = crate::workspace_view(scope, &changed, WidgetTheme::dark())
            .expect("reproject moved model");
        projected.element.style = projected.element.style.flow_direction(direction);
        let projected_layout = LayoutEngine
            .compute(&projected.element, Viewport::new(320.0, 240.0, 1.0))
            .expect("moved layout");
        let bounds = |pane| {
            projected_layout
                .node(
                    projected
                        .regions
                        .panes
                        .iter()
                        .find(|r| r.pane == pane)
                        .expect("pane region")
                        .root,
                )
                .expect("pane layout")
                .rect
        };
        let moved_bounds = bounds(moved);
        let old_bounds = bounds(pane.id);
        let correct = match side {
            Side::Left => moved_bounds.right() <= old_bounds.x + 0.01,
            Side::Right => moved_bounds.x + 0.01 >= old_bounds.right(),
            _ => unreachable!(),
        };
        assert!(
            correct,
            "physical dock side must survive direction projection: {direction:?} {side:?}"
        );
    }
}
