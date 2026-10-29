use super::*;
use vesta_gpui::{ElementIdAllocator, LayoutEngine, Viewport};
use vesta_workspace::{Pane, PanelTypeId, WorkspaceNode};

#[test]
fn pane_header_routes_history_and_active_item_commands_without_mutating_dock_tree() {
    let mut model = WorkspaceModel::with_tree(
        WorkspaceNode::split(
            SplitId(1),
            SplitAxis::Row,
            0.5,
            WorkspaceNode::leaf(PaneId(1)),
            WorkspaceNode::leaf(PaneId(2)),
        ),
        [Pane::new(PaneId(1)), Pane::new(PaneId(2))],
    )
    .expect("workspace");
    model
        .add_item(PaneId(1), "First", PanelTypeId::new("demo.first"))
        .expect("first");
    let active = model
        .add_item(PaneId(1), "Second", PanelTypeId::new("demo.second"))
        .expect("second");
    model
        .add_item(PaneId(2), "Other", PanelTypeId::new("demo.other"))
        .expect("other");
    model.activate(active).expect("activate left pane");
    let before = model.clone();
    let scope = ElementIdAllocator.allocate_scope().expect("scope");
    let header = |_: &Pane| PaneHeader {
        tab_icons: vec![(active, '\u{e97a}')],
        navigation: Some((true, false)),
        commands: vec![
            PaneHeaderCommand {
                id: 1,
                caption: "+".into(),
                name: "New tab".into(),
                enabled: true,
            },
            PaneHeaderCommand {
                id: 2,
                caption: "◫".into(),
                name: "Split pane".into(),
                enabled: false,
            },
        ],
        tab_scroll_x: 0.0,
        tab_scrollbar: None,
    };
    let view = workspace_view_with_header(
        scope,
        &model,
        WidgetTheme::dark(),
        WorkspaceViewInteraction::default(),
        None,
        header,
        |_| None,
    )
    .expect("header projection");
    assert!(view.actions.iter().any(|(_, action)| {
        *action
            == WorkspaceViewAction::Navigate {
                pane: PaneId(1),
                active,
                direction: PaneNavigationDirection::Back,
            }
    }));
    assert!(!view.actions.iter().any(|(_, action)| {
        matches!(
            action,
            WorkspaceViewAction::Navigate {
                direction: PaneNavigationDirection::Forward,
                ..
            }
        )
    }));
    assert!(view.actions.iter().any(|(_, action)| {
        *action
            == WorkspaceViewAction::HeaderCommand {
                pane: PaneId(1),
                active,
                command: 1,
            }
    }));
    assert!(!view.actions.iter().any(|(_, action)| {
        matches!(
            action,
            WorkspaceViewAction::HeaderCommand { command: 2, .. }
        )
    }));
    let route = vesta_gpui::ActionRoute::build(&view.element, view.actions.clone())
        .expect("header actions route separately from tabs");
    let layout = LayoutEngine
        .compute(&view.element, Viewport::new(800.0, 500.0, 1.25))
        .expect("header layout");
    let icon_id = scope.element(stable_local(IdentityKind::ItemIcon, active.0).expect("icon id"));
    let icon = view.element.find(icon_id).expect("decorative tab icon");
    assert_eq!(icon.style.visual.font_role, FontRole::Icon);
    assert!(!icon.style.pointer_events);
    assert!(icon.semantic.is_none());
    assert!(!route.focus_order().contains(&icon_id));
    let icon_rect = layout.node(icon_id).expect("icon layout").rect;
    let tab = view
        .regions
        .tabs
        .iter()
        .find(|tab| tab.item == active)
        .expect("stable tab region");
    let hit = vesta_gpui::HitTestTree::build(&view.element, &layout)
        .hit(Point::new(
            icon_rect.x + icon_rect.width / 2.0,
            icon_rect.y + icon_rect.height / 2.0,
        ))
        .expect("icon hits its tab");
    assert_eq!(hit, tab.tab);
    assert_eq!(
        route.action_for_hit(hit),
        Some(&WorkspaceViewAction::Tab(active))
    );
    let without_icons = workspace_view_with_header(
        scope,
        &model,
        WidgetTheme::dark(),
        WorkspaceViewInteraction::default(),
        None,
        |pane| {
            let mut value = header(pane);
            value.tab_icons.clear();
            value
        },
        |_| None,
    )
    .expect("plain header");
    assert_eq!(
        view.actions, without_icons.actions,
        "icons add no actions or change action identity"
    );
    assert_eq!(
        view.element.find(tab.tab).expect("tab").semantic,
        without_icons
            .element
            .find(tab.tab)
            .expect("same tab")
            .semantic,
        "accessible names and selection stay independent of the icon font"
    );
    let back = layout
        .node(scope.element(stable_local(IdentityKind::PaneBack, 1).expect("back id")))
        .expect("back button");
    let forward = layout
        .node(scope.element(stable_local(IdentityKind::PaneForward, 1).expect("forward id")))
        .expect("forward button");
    assert_eq!(back.rect.width, 26.0);
    assert_eq!(forward.rect.x - back.rect.x, 26.0);
    assert!(
        view.element
            .find(scope.element(stable_local(IdentityKind::PaneBackLabel, 1).expect("icon id")))
            .is_some_and(|icon| icon.style.polyline.is_some())
    );
    let header_target = view
        .actions
        .iter()
        .find(|(_, action)| {
            matches!(
                action,
                WorkspaceViewAction::HeaderCommand { command: 1, .. }
            )
        })
        .map(|(element, _)| *element)
        .expect("new tab control");
    assert!(route.focus_order().contains(&header_target));
    let zoomed = workspace_view_with_header(
        scope,
        &model,
        WidgetTheme::dark(),
        WorkspaceViewInteraction::default(),
        Some(PaneId(1)),
        |pane| PaneHeader {
            navigation: pane.active.map(|_| (false, false)),
            ..PaneHeader::default()
        },
        |_| None,
    )
    .expect("zoom projection");
    assert_eq!(zoomed.regions.panes.len(), 1);
    assert_eq!(zoomed.regions.panes[0].pane, PaneId(1));
    assert_eq!(model, before);
}

#[test]
fn pane_tab_list_reports_horizontal_overflow_for_active_tab_reveal() {
    let mut model =
        WorkspaceModel::with_tree(WorkspaceNode::leaf(PaneId(1)), [Pane::new(PaneId(1))])
            .expect("workspace");
    for index in 0..10 {
        model
            .add_item(
                PaneId(1),
                format!("Gallery Panel {index}"),
                PanelTypeId::new("demo.panel"),
            )
            .expect("item");
    }
    let scope = ElementIdAllocator.allocate_scope().expect("scope");
    let view = workspace_view_with_header(
        scope,
        &model,
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
            tab_scroll_x: 0.0,
            tab_scrollbar: Some(ScrollbarStyle {
                gutter: ScrollbarGutter::Reserve,
                ..ScrollbarStyle::default()
            }),
            ..PaneHeader::default()
        },
        |_| None,
    )
    .expect("header projection");
    let layout = LayoutEngine
        .compute(&view.element, Viewport::new(320.0, 240.0, 1.0))
        .expect("layout");
    let tab_list = layout
        .node(view.regions.panes[0].tab_list)
        .expect("tab list");
    assert!(
        tab_list
            .scroll_content
            .is_some_and(|(width, _)| width > tab_list.rect.width),
        "tab list must expose overflow to the host scroll state"
    );
    let viewport = tab_list.scroll_viewport();
    assert_eq!(
        viewport.height, CHROME_TAB_HEIGHT,
        "the scrollbar does not consume the tab buttons"
    );
    assert!(
        tab_list.rect.height > viewport.height,
        "overflow adds a row beneath the tabs"
    );
    let (vertical, _, horizontal, _) = tab_list.scrollbar_geometry(Point::default());
    assert!(vertical.is_none(), "tab strip only overflows horizontally");
    let track = horizontal.expect("visible theme scrollbar");
    assert!(track.intersection(viewport).is_none());
    assert_eq!(
        vesta_gpui::HitTestTree::build(&view.element, &layout)
            .hit(Point::new(track.x + 2.0, track.y + 2.0)),
        Some(tab_list.id)
    );
    let wide = LayoutEngine
        .compute(&view.element, Viewport::new(3000.0, 240.0, 1.0))
        .expect("wide pane");
    let tabs = wide.node(tab_list.id).expect("same stable tabs");
    assert_eq!(
        tabs.rect,
        tabs.scroll_viewport(),
        "wide pane releases the reserved row"
    );
    tab_reveal::tests::qualify(&model, scope);
    dock::overflow_tests::qualify(&model, scope);
}

#[test]
fn nested_split_regions_resolve_their_own_layout_bounds() {
    let root = WorkspaceNode::split(
        SplitId(1),
        SplitAxis::Row,
        0.5,
        WorkspaceNode::split(
            SplitId(2),
            SplitAxis::Column,
            0.5,
            WorkspaceNode::leaf(PaneId(1)),
            WorkspaceNode::leaf(PaneId(2)),
        ),
        WorkspaceNode::leaf(PaneId(3)),
    );
    let model = WorkspaceModel::with_tree(
        root,
        [
            Pane::new(PaneId(1)),
            Pane::new(PaneId(2)),
            Pane::new(PaneId(3)),
        ],
    )
    .expect("nested workspace");
    let scope = ElementIdAllocator.allocate_scope().expect("identity scope");
    let view = workspace_view(scope, &model, WidgetTheme::dark()).expect("projection");
    let layout = LayoutEngine
        .compute(&view.element, Viewport::new(800.0, 500.0, 1.0))
        .expect("nested layout");
    let hits = vesta_gpui::HitTestTree::build(&view.element, &layout);
    for (element, action) in &view.actions {
        if let WorkspaceViewAction::BeginResize { axis, .. } = action {
            let handle = layout.node(*element).expect("splitter bounds").rect;
            for fraction in [0.25, 0.5, 0.75] {
                let pointer = match axis {
                    SplitAxis::Row => Point::new(
                        handle.x + handle.width * fraction,
                        handle.y + handle.height * 0.5,
                    ),
                    SplitAxis::Column => Point::new(
                        handle.x + handle.width * 0.5,
                        handle.y + handle.height * fraction,
                    ),
                };
                assert_eq!(hits.hit(pointer), Some(*element));
            }
        }
    }
    let outer = view
        .regions
        .splits
        .iter()
        .find(|region| region.split == SplitId(1))
        .and_then(|region| layout.node(region.root))
        .expect("outer split bounds");
    let inner = view
        .regions
        .splits
        .iter()
        .find(|region| region.split == SplitId(2))
        .and_then(|region| layout.node(region.root))
        .expect("inner split bounds");
    assert!(inner.rect.width < outer.rect.width);
    assert!(inner.rect.height <= outer.rect.height);
    let pointer = Point::new(inner.rect.right(), inner.rect.y);
    let ratio = split_ratio_at(inner.rect, SplitAxis::Row, pointer, MIN_DOCK_PANE_EXTENT);
    assert!(ratio <= 1.0 - MIN_DOCK_PANE_EXTENT / inner.rect.width);
    assert_eq!(
        split_ratio_at(
            inner.rect,
            SplitAxis::Row,
            Point::new(inner.rect.x + inner.rect.width * 0.5, inner.rect.y),
            MIN_DOCK_PANE_EXTENT,
        ),
        0.5,
    );
}
