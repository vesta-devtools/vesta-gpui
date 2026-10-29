use super::*;
use vesta_gpui::{ActionRoute, LayoutEngine, Viewport};

fn id(value: u64) -> WidgetIdentity {
    WidgetIdentity::new(ElementId::new(value), format!("table.{value}"))
}

#[test]
fn windowed_table_projects_bounded_rows_and_retains_offscreen_identity() {
    table_columns_keep_declared_geometry_in_constrained_viewports();
    scrolled_table_keeps_compound_input_and_header_tracks_aligned();
    let range =
        VirtualListRange::compute(10_000, 28.0, 140.0, 140_000.0, 2).expect("bounded table range");
    let mut projected = Vec::new();
    let table = table_view_windowed(
        TableWindowIds {
            root: id(1),
            header: id(2),
            viewport: id(7),
            content: id(8),
        },
        vec![TableColumn {
            header: id(3),
            title: "Name".into(),
            width: 180.0,
            sort_action: Some(10_001),
            resize: None,
        }],
        range,
        Some(9_999),
        |index| {
            projected.push(index);
            TableRow {
                identity: id(100 + index as u64 * 2),
                cells: vec![TableCell {
                    identity: id(101 + index as u64 * 2),
                    value: format!("Object {index}"),
                    selected: index == 9_999,
                    select_action: Some(index),
                }],
                selected: index == 9_999,
                select_action: index,
            }
        },
        WidgetTheme::dark(),
    )
    .expect("windowed table");
    assert_eq!(projected.len(), range.len() + 1);
    assert_eq!(projected.last(), Some(&9_999));
    assert_eq!(table.element.children[0].id, ElementId::new(2));
    let route = ActionRoute::build(&table.element, table.actions).expect("table actions");
    assert_eq!(
        route.action_at(ElementId::new(101 + 9_999 * 2)),
        Some(&9_999)
    );
    assert_eq!(
        route.action_at(ElementId::new(100 + range.first as u64 * 2)),
        Some(&range.first)
    );
    let layout = LayoutEngine
        .compute(&table.element, Viewport::new(200.0, 180.0, 1.0))
        .expect("windowed layout");
    assert_eq!(
        layout
            .node(ElementId::new(7))
            .expect("viewport")
            .scroll_content,
        Some((180.0, 280_000.0))
    );
    let viewport = layout.node(ElementId::new(7)).expect("scroll viewport");
    let (Some(track), _, horizontal, _) = viewport.scrollbar_geometry(Point::default()) else {
        panic!("windowed table reserves its overflowing vertical axis");
    };
    assert!(
        horizontal.is_none(),
        "the gutter must not reduce the declared column widths"
    );
    assert!(track.intersection(viewport.scroll_viewport()).is_none());
    let narrow = LayoutEngine
        .compute(&table.element, Viewport::new(180.0, 180.0, 1.0))
        .expect("constrained table");
    let node = narrow
        .node(ElementId::new(7))
        .expect("constrained viewport");
    let (Some(vertical), _, Some(horizontal), _) = node.scrollbar_geometry(Point::default()) else {
        panic!("columns that do not fit beside the gutter need both axes");
    };
    assert!(narrow.viewport().logical_rect().contains_rect(vertical));
    assert!(narrow.viewport().logical_rect().contains_rect(horizontal));
    // The bounded path must retain a focused child editor, not just text shells.
    let mut content_projected = Vec::new();
    let custom = table_view_windowed_with_content(
        TableWindowIds {
            root: id(1),
            header: id(2),
            viewport: id(7),
            content: id(8),
        },
        vec![TableColumnContent {
            header: id(3),
            content: WidgetContent::new(
                "Name",
                Widget::from_element(
                    Element::container(
                        ElementId::new(4),
                        vesta_gpui::ElementKey::new("header.content"),
                    )
                    .styled(Style::default().focusable(true)),
                )
                .on_click(20_000),
            ),
            width: 180.0,
            sort_action: Some(10_001),
            resize: None,
        }],
        range,
        Some(9_999),
        |index| {
            content_projected.push(index);
            let base = 100 + index as u64 * 4;
            TableRowContent {
                identity: id(base),
                cells: vec![TableCellContent {
                    identity: id(base + 1),
                    content: WidgetContent::new(
                        format!("Object {index}"),
                        Widget::from_element(Element::container(
                            ElementId::new(base + 2),
                            vesta_gpui::ElementKey::new(format!("cell.{base}.content")),
                        ))
                        .child(
                            Widget::from_element(
                                Element::container(
                                    ElementId::new(base + 3),
                                    vesta_gpui::ElementKey::new(format!("cell.{base}.editor")),
                                )
                                .styled(
                                    Style::default()
                                        .size(Dimension::Px(300.0), Dimension::Px(28.0))
                                        .focusable(true),
                                ),
                            )
                            .on_click(30_000 + index),
                        ),
                    ),
                    selected: false,
                    select_action: Some(40_000 + index),
                }],
                selected: index == 9_999,
                select_action: index,
            }
        },
        WidgetTheme::dark(),
    )
    .expect("windowed content table");
    assert_eq!(content_projected, projected);
    let route = ActionRoute::build(&custom.element, custom.actions)
        .expect("independent table content actions");
    assert_eq!(route.action_at(ElementId::new(4)), Some(&20_000));
    assert_eq!(route.action_at(ElementId::new(3)), Some(&10_001));
    let child = ElementId::new(103 + 9_999 * 4);
    assert_eq!(route.action_at(child), Some(&39_999));
    assert!(route.focus_order().contains(&child));
    assert_eq!(
        custom
            .element
            .find(ElementId::new(101 + 9_999 * 4))
            .expect("retained cell")
            .style
            .clip,
        vesta_gpui::ClipMode::Clip
    );
    assert_eq!(
        custom
            .element
            .find(ElementId::new(101 + 9_999 * 4))
            .and_then(|cell| cell.semantic.as_ref())
            .map(|s| s.label.as_str()),
        Some("Object 9999")
    );

    let duplicate = table_view_with_content(
        id(1),
        id(2),
        vec![TableColumnContent {
            header: id(3),
            content: WidgetContent::new(
                "Duplicate",
                Widget::<usize>::from_element(Element::text(
                    ElementId::new(1),
                    vesta_gpui::ElementKey::new("duplicate"),
                    "Header",
                )),
            ),
            width: 100.0,
            sort_action: None,
            resize: None,
        }],
        vec![],
        WidgetTheme::dark(),
    );
    assert_eq!(
        duplicate,
        Err(TableViewError::DuplicateElement(ElementId::new(1)))
    );
    let malformed = VirtualListRange {
        first: 10,
        end: 9,
        ..range
    };
    assert_eq!(
        table_view_windowed(
            TableWindowIds {
                root: id(1),
                header: id(2),
                viewport: id(7),
                content: id(8),
            },
            vec![TableColumn {
                header: id(3),
                title: "Name".into(),
                width: 180.0,
                sort_action: None::<usize>,
                resize: None,
            }],
            malformed,
            None,
            |_| unreachable!(),
            WidgetTheme::dark(),
        )
        .err(),
        Some(TableViewError::InvalidViewport),
    );
}

fn scrolled_table_keeps_compound_input_and_header_tracks_aligned() {
    let theme = WidgetTheme::dark();
    let ids = || TableWindowIds {
        root: id(1),
        header: id(2),
        viewport: id(3),
        content: id(4),
    };
    let columns = || {
        [180.0, 120.0]
            .into_iter()
            .enumerate()
            .map(|(column, width)| {
                let base = 10 + column as u64 * 3;
                TableColumnContent {
                    header: id(base),
                    content: WidgetContent::new(
                        format!("Column {column}"),
                        crate::button(
                            id(base + 1),
                            id(base + 2),
                            "Header",
                            100 + column,
                            crate::ControlState::Idle,
                            theme,
                        ),
                    ),
                    width,
                    sort_action: Some(200 + column),
                    resize: None,
                }
            })
            .collect()
    };
    let range = VirtualListRange::compute(100, 28.0, 84.0, 28.0, 0).expect("visible rows");
    for flow in [
        vesta_gpui::FlowDirection::LeftToRight,
        vesta_gpui::FlowDirection::RightToLeft,
    ] {
        for scale in [1.0, 2.0] {
            for offset in [0.0, 30.0, 80.0] {
                let mut built = Vec::new();
                let mut table = table_view_windowed_scrolled_with_content(
                    ids(),
                    columns(),
                    range,
                    offset,
                    Some(99),
                    |index| {
                        built.push(index);
                        let base = 1000 + index as u64 * 10;
                        TableRowContent {
                            identity: id(base),
                            cells: (0..2)
                                .map(|column| TableCellContent {
                                    identity: id(base + 1 + column),
                                    content: WidgetContent::new(
                                        format!("Cell {column}"),
                                        crate::button(
                                            id(base + 3 + column * 2),
                                            id(base + 4 + column * 2),
                                            "Cell",
                                            1000 + index * 2 + column as usize,
                                            crate::ControlState::Idle,
                                            theme,
                                        ),
                                    ),
                                    selected: false,
                                    select_action: Some(2000 + index * 2 + column as usize),
                                })
                                .collect(),
                            selected: index == 99,
                            select_action: 3000 + index,
                        }
                    },
                    theme,
                )
                .expect("scrolled compound table");
                assert_eq!(built, [1, 2, 3, 99]);
                table.element.style.max_width = Dimension::Px(180.0);
                table.element.style.flow_direction = Some(flow);
                let layout = LayoutEngine
                    .compute(&table.element, Viewport::new(600.0, 160.0, scale))
                    .expect("scrolled table layout");
                let route =
                    ActionRoute::build(&table.element, table.actions).expect("compound actions");
                let hits = vesta_gpui::HitTestTree::build(&table.element, &layout);
                let mut child_hits = 0;
                for column in 0..2 {
                    let header = layout
                        .node(ElementId::new(10 + column * 3))
                        .expect("header");
                    let cell = layout.node(ElementId::new(1011 + column)).expect("cell");
                    assert!((header.rect.x - cell.rect.x).abs() < 0.01);
                    assert_eq!(header.rect.height, 28.0);
                    for (element, action) in [
                        (ElementId::new(11 + column * 3), 100 + column as usize),
                        (ElementId::new(1013 + column * 2), 1002 + column as usize),
                    ] {
                        let node = layout.node(element).expect("child geometry");
                        if let Some(clip) = node.rect.intersection(node.inherited_clip) {
                            let hit = hits
                                .hit(Point::new(
                                    clip.x + clip.width / 2.0,
                                    clip.y + clip.height / 2.0,
                                ))
                                .expect("visible child");
                            assert_eq!(route.action_for_hit(hit), Some(&action));
                            child_hits += 1;
                        }
                    }
                }
                assert!(
                    child_hits >= 2,
                    "a live header/cell pair must remain reachable"
                );
                let retained = ElementId::new(1993);
                assert!(route.focus_order().contains(&retained));
                let retained_rect = layout.node(retained).expect("retained child").rect;
                assert!(
                    hits.hit(Point::new(retained_rect.x + 1.0, retained_rect.y + 1.0))
                        .is_none()
                );
                assert_eq!(
                    table
                        .element
                        .find(ElementId::new(2))
                        .expect("header viewport")
                        .style
                        .scrollbar
                        .gutter,
                    vesta_gpui::ScrollbarGutter::Overlay
                );
                println!(
                    "GPV05-SCROLLED-CONTENT flow={flow:?} scale={scale} offset={offset} built={} child_hits={child_hits}",
                    built.len()
                );
            }
        }
    }
    for offset in [f32::NAN, f32::INFINITY, -1.0, 301.0] {
        assert_eq!(
            table_view_windowed_scrolled_with_content::<usize>(
                ids(),
                columns(),
                range,
                offset,
                None,
                |_| panic!("invalid horizontal geometry reached row factory"),
                theme
            )
            .err(),
            Some(TableViewError::InvalidViewport)
        );
        assert_eq!(
            table_view_windowed_scrolled::<usize>(
                ids(),
                vec![TableColumn {
                    header: id(10),
                    title: "Name".into(),
                    width: 300.0,
                    sort_action: None,
                    resize: None,
                }],
                range,
                offset,
                None,
                |_| panic!("invalid text geometry reached row factory"),
                theme
            )
            .err(),
            Some(TableViewError::InvalidViewport)
        );
    }
}

fn table_columns_keep_declared_geometry_in_constrained_viewports() {
    let theme = WidgetTheme::dark();
    for (flow, width, scale) in [
        (vesta_gpui::FlowDirection::LeftToRight, 600.0, 1.0),
        (vesta_gpui::FlowDirection::LeftToRight, 180.0, 1.0),
        (vesta_gpui::FlowDirection::RightToLeft, 600.0, 1.0),
        (vesta_gpui::FlowDirection::RightToLeft, 180.0, 1.0),
        (vesta_gpui::FlowDirection::LeftToRight, 600.0, 2.0),
        (vesta_gpui::FlowDirection::LeftToRight, 180.0, 2.0),
        (vesta_gpui::FlowDirection::RightToLeft, 600.0, 2.0),
        (vesta_gpui::FlowDirection::RightToLeft, 180.0, 2.0),
    ] {
        let range =
            VirtualListRange::compute(100, 28.0, 84.0, 28.0, 0).expect("multi-column visible rows");
        let mut built = Vec::new();
        let mut table = table_view_windowed_with_content(
            TableWindowIds {
                root: id(1),
                header: id(2),
                viewport: id(3),
                content: id(4),
            },
            vec![
                TableColumnContent {
                    header: id(10),
                    content: WidgetContent::new(
                        "Name",
                        crate::button(
                            id(13),
                            id(14),
                            "Filter",
                            200,
                            crate::ControlState::Idle,
                            theme,
                        ),
                    ),
                    width: 180.0,
                    sort_action: Some(100),
                    resize: Some(TableColumnResize {
                        handle: id(12),
                        min_width: 36.0,
                        max_width: 264.0,
                        keyboard_step: 8.0,
                    }),
                },
                TableColumnContent {
                    header: id(11),
                    content: WidgetContent::new(
                        "Kind",
                        Widget::from_element(Element::text(
                            ElementId::new(15),
                            vesta_gpui::ElementKey::new("kind.header"),
                            "Kind",
                        )),
                    ),
                    width: 120.0,
                    sort_action: Some(101),
                    resize: None,
                },
            ],
            range,
            Some(99),
            |index| {
                built.push(index);
                let base = 1000 + index as u64 * 10;
                TableRowContent {
                    identity: id(base),
                    cells: vec![
                        TableCellContent {
                            identity: id(base + 1),
                            content: WidgetContent::new(
                                format!("Object {index}"),
                                crate::button(
                                    id(base + 3),
                                    id(base + 4),
                                    "Open",
                                    300 + index,
                                    crate::ControlState::Idle,
                                    theme,
                                ),
                            ),
                            selected: false,
                            select_action: Some(2000 + index),
                        },
                        TableCellContent {
                            identity: id(base + 2),
                            content: WidgetContent::new(
                                "Image",
                                Widget::from_element(
                                    Element::image(
                                        ElementId::new(base + 5),
                                        vesta_gpui::ElementKey::new(format!("image.{index}")),
                                        "example://thumbnail",
                                    )
                                    .styled(
                                        Style::default()
                                            .size(Dimension::Px(240.0), Dimension::Px(28.0)),
                                    ),
                                ),
                            ),
                            selected: false,
                            select_action: Some(3000 + index),
                        },
                    ],
                    selected: index == 99,
                    select_action: 1000 + index,
                }
            },
            theme,
        )
        .expect("multi-column compound table");
        assert_eq!(built.len(), range.len() + 1);
        table.element.style.flow_direction = Some(flow);
        table.element.style.max_width = Dimension::Px(width);
        let layout = LayoutEngine
            .compute(&table.element, Viewport::new(600.0, 160.0, scale))
            .expect("multi-column layout");
        let first = 1000 + range.first as u64 * 10;
        let header = layout.node(ElementId::new(10)).expect("first header").rect;
        let handle = layout.node(ElementId::new(12)).expect("resize handle").rect;
        let cell = layout
            .node(ElementId::new(first + 1))
            .expect("first cell")
            .rect;
        assert!(
            (header.width + handle.width - 180.0).abs() < 0.01,
            "declared first column shrank at viewport {width}: header={header:?}, handle={handle:?}"
        );
        assert!((cell.width - 180.0).abs() < 0.01);
        let second_header = layout.node(ElementId::new(11)).expect("second header").rect;
        let second_cell = layout
            .node(ElementId::new(first + 2))
            .expect("second cell")
            .rect;
        assert!(
            (second_header.x - second_cell.x).abs() < 0.01,
            "header and cell disagree at viewport {width}: header={second_header:?}, cell={second_cell:?}"
        );
        let route = ActionRoute::build(&table.element, table.actions)
            .expect("compound header and cell actions");
        let hits = vesta_gpui::HitTestTree::build(&table.element, &layout);
        let bounds = layout.node(ElementId::new(1)).expect("table bounds").rect;
        for element in [10, 11, 12] {
            let node = layout.node(ElementId::new(element)).expect("header track");
            assert!(
                bounds.contains_rect(node.inherited_clip),
                "constrained header must not paint or hit outside the table: {bounds:?} / {:?}",
                node.inherited_clip
            );
        }
        for (element, action) in [
            (ElementId::new(13), 200),
            (ElementId::new(first + 3), 300 + range.first),
        ] {
            let node = layout.node(element).expect("live child geometry");
            let visible = node
                .rect
                .intersection(node.inherited_clip)
                .expect("compound child remains visible");
            let hit = hits
                .hit(Point::new(
                    visible.x + visible.width / 2.0,
                    visible.y + visible.height / 2.0,
                ))
                .expect("visible child hit");
            assert_eq!(
                route.action_for_hit(hit),
                Some(&action),
                "compound child must win over header sort or cell/row select"
            );
        }
        let image = layout
            .node(ElementId::new(first + 5))
            .expect("oversized image");
        if image.inherited_clip.width > 0.0 && image.inherited_clip.height > 0.0 {
            assert!(
                second_cell.contains_rect(image.inherited_clip),
                "oversized content must remain clipped to its own cell"
            );
        }
        let retained = ElementId::new(1000 + 99 * 10 + 3);
        assert!(route.focus_order().contains(&retained));
        let retained_node = layout.node(retained).expect("offscreen retained child");
        assert!(
            hits.hit(Point::new(
                retained_node.rect.x + 1.0,
                retained_node.rect.y + 1.0
            ))
            .is_none()
        );
        assert_eq!(route.action_at(ElementId::new(10)), Some(&100));
        assert_eq!(
            route.action_at(ElementId::new(first + 1)),
            Some(&(2000 + range.first))
        );
        let mut plain = table_view(
            id(20),
            id(21),
            [180.0, 120.0]
                .into_iter()
                .enumerate()
                .map(|(index, width)| TableColumn {
                    header: id(10 + index as u64),
                    title: format!("Column {index}"),
                    width,
                    sort_action: Some(100 + index),
                    resize: (index == 0).then(|| TableColumnResize {
                        handle: id(12),
                        min_width: 36.0,
                        max_width: 264.0,
                        keyboard_step: 8.0,
                    }),
                })
                .collect(),
            vec![TableRow {
                identity: id(50),
                cells: (0..2)
                    .map(|index| TableCell {
                        identity: id(51 + index as u64),
                        value: format!("Value {index}"),
                        selected: false,
                        select_action: Some(200 + index),
                    })
                    .collect(),
                selected: false,
                select_action: 300,
            }],
            theme,
        )
        .expect("original text constructor");
        plain.element.style.max_width = Dimension::Px(width);
        plain.element.style.flow_direction = Some(flow);
        let plain_layout = LayoutEngine
            .compute(&plain.element, Viewport::new(600.0, 160.0, scale))
            .expect("plain table geometry");
        let first_cell = plain_layout
            .node(ElementId::new(51))
            .expect("plain first cell")
            .rect;
        let plain_second_cell = plain_layout
            .node(ElementId::new(52))
            .expect("plain second cell")
            .rect;
        let second_column = plain_layout
            .node(ElementId::new(11))
            .expect("plain second header")
            .rect;
        assert!((first_cell.width - 180.0).abs() < 0.01);
        assert!((plain_second_cell.width - 120.0).abs() < 0.01);
        assert!(
            (second_column.x - plain_second_cell.x).abs() < 0.01,
            "text wrapper and content constructor must share declared tracks"
        );
        println!(
            "GPV05-TABLE-COLUMNS flow={flow:?} viewport={width} scale={scale} header={second_header:?} cell={second_cell:?} built={} retained=99 child_actions=2",
            built.len()
        );
    }
}
