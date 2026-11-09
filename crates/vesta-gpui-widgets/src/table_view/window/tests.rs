use super::*;
use vesta_gpui::{ActionRoute, HitTestTree, LayoutEngine, Viewport};

fn id(value: u64) -> WidgetIdentity {
    WidgetIdentity::new(ElementId::new(value), format!("wide.{value}"))
}
fn band_ids(base: u64) -> TableBandIds {
    TableBandIds {
        leading: id(base),
        scrolling: id(base + 1),
        trailing: id(base + 2),
    }
}
fn ids() -> WideTableIds {
    WideTableIds {
        table: TableWindowIds {
            root: id(1),
            header: id(2),
            viewport: id(3),
            content: id(4),
        },
        header_bands: band_ids(5),
    }
}

#[test]
fn column_window_matches_full_scan_and_mirrors_physical_bounds() {
    let widths = (0..1000)
        .map(|i| 20.25 + (i * 37 % 93) as f32)
        .collect::<Vec<_>>();
    let geometry = TableColumnMetrics::new(&widths, 2, 3).expect("valid wide-table fixture");
    let total = widths.iter().map(|&x| f64::from(x)).sum::<f64>();
    for width in [400.5, 800.25, 2000.0] {
        for offset in [-99.0, 0.0, 20.0, 1000.25, 30111.0, f32::MAX] {
            for overscan in [0, 3, 99] {
                let ltr = geometry
                    .window(width, offset, overscan, FlowDirection::LeftToRight)
                    .expect("valid wide-table fixture");
                let rtl = geometry
                    .window(width, offset, overscan, FlowDirection::RightToLeft)
                    .expect("valid wide-table fixture");
                let leading = f64::from(widths[0] + widths[1]);
                let trailing = widths[997..].iter().map(|&x| f64::from(x)).sum::<f64>();
                let left = leading + f64::from(ltr.offset());
                let right = left + f64::from(width) - leading - trailing;
                let mut x = 0.0;
                let mut visible = Vec::new();
                for (index, &column_width) in widths.iter().enumerate() {
                    let end = x + f64::from(column_width);
                    if (2..997).contains(&index) && end > left && x < right {
                        visible.push(index);
                    }
                    x = end;
                }
                let first = visible[0].saturating_sub(overscan.min(32)).max(2);
                let last =
                    (visible.last().expect("valid wide-table fixture") + 1 + overscan.min(32))
                        .min(997);
                let expected = (0..2)
                    .chain(first..last)
                    .chain(997..1000)
                    .collect::<Vec<_>>();
                assert_eq!(ltr.indices(), expected);
                assert_eq!(rtl.indices(), expected);
                assert!((f64::from(ltr.max_offset()) - (total - f64::from(width))).abs() < 0.01);
                for index in expected {
                    let (lx, lw) = ltr.column_bounds(index).expect("valid wide-table fixture");
                    let (rx, rw) = rtl.column_bounds(index).expect("valid wide-table fixture");
                    assert_eq!(lw, widths[index]);
                    assert_eq!(lw, rw);
                    assert!((lx + rx + lw - width).abs() < 0.01);
                    for source in [&ltr, &rtl] {
                        let revealed_offset =
                            source.offset_to_reveal(index).expect("live reveal index");
                        let revealed = geometry
                            .window(width, revealed_offset, 0, source.flow)
                            .expect("valid revealed window");
                        assert!(revealed.indices().contains(&index));
                        if source.band(index) != 1 {
                            assert_eq!(revealed_offset, source.offset());
                        }
                    }
                }
            }
        }
    }
    assert_eq!(geometry.width(usize::MAX), None);
    assert_eq!(geometry.width(1000), None);
    for (widths, leading, trailing, error) in [
        (vec![], 0, 0, TableViewError::EmptyColumns),
        (vec![0.0], 0, 0, TableViewError::InvalidColumnWidth),
        (vec![f32::NAN], 0, 0, TableViewError::InvalidColumnWidth),
        (
            vec![16_777_216.0, 1.0],
            0,
            0,
            TableViewError::InvalidColumnWidth,
        ),
        (vec![10.0; 65], 65, 0, TableViewError::InvalidViewport),
        (vec![10.0], 1, 1, TableViewError::InvalidViewport),
    ] {
        assert_eq!(
            TableColumnMetrics::new(&widths, leading, trailing).err(),
            Some(error)
        );
    }
    for (width, offset) in [
        (0.0, 0.0),
        (100.0, 0.0),
        (400.0, f32::NAN),
        (f32::INFINITY, 0.0),
    ] {
        assert_eq!(
            geometry
                .window(width, offset, 0, FlowDirection::LeftToRight)
                .err(),
            Some(TableViewError::InvalidViewport)
        );
    }
    let dense = TableColumnMetrics::new(&[1.0; 1000], 0, 0).expect("valid wide-table fixture");
    assert_eq!(
        dense
            .window(400.0, 0.0, 0, FlowDirection::LeftToRight)
            .err(),
        Some(TableViewError::InvalidViewport)
    );
    let frozen = TableColumnMetrics::new(&[20.0, 30.0], 1, 1).expect("valid wide-table fixture");
    assert_eq!(
        frozen
            .window(50.0, 10.0, 32, FlowDirection::LeftToRight)
            .expect("valid wide-table fixture")
            .indices(),
        [0, 1]
    );
    assert_eq!(
        frozen
            .window(49.0, 0.0, 0, FlowDirection::LeftToRight)
            .err(),
        Some(TableViewError::InvalidViewport)
    );
}

fn column(index: usize) -> TableColumnContent<usize> {
    let base = 100 + index as u64 * 10;
    TableColumnContent {
        header: id(base),
        content: crate::WidgetContent::new(
            format!("Column {index}"),
            crate::button(
                id(base + 1),
                id(base + 2),
                "Sort",
                index,
                crate::ControlState::Idle,
                WidgetTheme::dark(),
            ),
        ),
        width: 80.25,
        sort_action: Some(1000 + index),
        resize: Some(TableColumnResize {
            handle: id(base + 3),
            min_width: 20.0,
            max_width: 200.0,
            keyboard_step: 5.0,
        }),
    }
}
fn row(index: usize, columns: &[usize]) -> TableWindowRow<usize> {
    let base = 100_000 + index as u64 * 10_000;
    TableWindowRow {
        bands: band_ids(base + 1),
        label: format!("Record {index}"),
        row: TableRowContent {
            identity: id(base),
            selected: index == 9999,
            select_action: 100_000 + index,
            cells: columns
                .iter()
                .map(|&column| {
                    let base = base + 10 + column as u64 * 10;
                    TableCellContent {
                        identity: id(base),
                        content: crate::WidgetContent::new(
                            format!("Row {index} column {column}"),
                            crate::button(
                                id(base + 1),
                                id(base + 2),
                                "Open",
                                index * 1000 + column,
                                crate::ControlState::Idle,
                                WidgetTheme::dark(),
                            ),
                        ),
                        selected: false,
                        select_action: Some(10_000_000 + index * 1000 + column),
                    }
                })
                .collect(),
        },
    }
}

#[test]
fn frozen_projection_preserves_routing_clips_scroll_focus_and_semantics() {
    let metrics = TableColumnMetrics::new(&[80.25; 500], 2, 1).expect("valid wide-table fixture");
    let range = VirtualListRange::compute(10_000, 28.0, 140.0, 2800.0, 1)
        .expect("valid wide-table fixture");
    for flow in [FlowDirection::LeftToRight, FlowDirection::RightToLeft] {
        for scale in [1.0, 1.5, 2.0] {
            for offset in [0.0, 4030.5, f32::MAX] {
                let window = metrics
                    .window(480.0, offset, 1, flow)
                    .expect("valid wide-table fixture");
                let mut headers = Vec::new();
                let mut rows = Vec::new();
                let table = table_view_columns(
                    ids(),
                    &window,
                    range,
                    Some(TableRetainedFocus {
                        row: Some(9999),
                        column: Some(250),
                    }),
                    |i| {
                        headers.push(i);
                        column(i)
                    },
                    |i, cols| {
                        rows.push((i, cols.to_vec()));
                        row(i, cols)
                    },
                    WidgetTheme::dark(),
                )
                .expect("valid wide-table fixture");
                assert_eq!(headers, window.indices());
                assert_eq!(rows.len(), range.len() + 1);
                assert!(headers.len() < 12);
                for (i, cols) in &rows {
                    assert!(cols.windows(2).all(|pair| pair[0] < pair[1]));
                    assert_eq!(
                        cols.contains(&250),
                        window.indices().contains(&250) || *i == 9999
                    );
                }
                let viewport = Viewport::new(480.0, 168.0, scale);
                let layout = LayoutEngine
                    .compute(&table.element, viewport)
                    .expect("valid wide-table fixture");
                let route = ActionRoute::build(&table.element, table.actions)
                    .expect("valid wide-table fixture");
                let hits = HitTestTree::build(&table.element, &layout);
                let scroll = layout
                    .node(ElementId::new(3))
                    .expect("valid wide-table fixture");
                assert_eq!(scroll.scroll_content, Some((40125.0, 280000.0)));
                assert_eq!(
                    scroll
                        .scroll_viewport
                        .expect("valid wide-table fixture")
                        .width,
                    480.0
                );
                let visible_row = range.anchor().expect("valid wide-table fixture").index();
                let row_base = 100_000 + visible_row as u64 * 10_000;
                let mut reached = 0;
                for &i in window.indices() {
                    let header = layout
                        .node(ElementId::new(100 + i as u64 * 10))
                        .expect("valid wide-table fixture");
                    let cell = layout
                        .node(ElementId::new(row_base + 10 + i as u64 * 10))
                        .expect("valid wide-table fixture");
                    if window.band(i) == 1 {
                        let (band_x, band_width) = window.band_bounds(1);
                        assert!(cell.inherited_clip.x >= (band_x + 0.5).floor());
                        assert!(cell.inherited_clip.right() <= (band_x + band_width + 0.5).floor());
                    }
                    // Resize handle consumes six pixels of the declared header width.
                    let inset = if flow == FlowDirection::RightToLeft {
                        6.0
                    } else {
                        0.0
                    };
                    assert!(
                        (header.rect.x - cell.rect.x - inset).abs() < 0.01,
                        "{flow:?} {offset} {i}: {:?} {:?}",
                        header.rect,
                        cell.rect
                    );
                    let (x, width) = window.column_bounds(i).expect("valid wide-table fixture");
                    let edge = |v: f32| (f64::from(v) + 0.5).floor() as f32;
                    assert_eq!(cell.rect.x, edge(x));
                    assert_eq!(cell.rect.width, edge(x + width) - edge(x));
                    let child = ElementId::new(row_base + 11 + i as u64 * 10);
                    let node = layout.node(child).expect("valid wide-table fixture");
                    if let Some(clip) = node
                        .rect
                        .intersection(node.inherited_clip)
                        .filter(|r| r.width > 1.0 && r.height > 1.0)
                    {
                        let point =
                            Point::new(clip.x + clip.width / 2.0, clip.y + clip.height / 2.0);
                        let hit = hits.hit(point).expect("valid wide-table fixture");
                        assert_eq!(route.action_for_hit(hit), Some(&(visible_row * 1000 + i)));
                        reached += 1;
                    }
                }
                assert!(reached >= 4);
                let retained = ElementId::new(100_000 + 9999 * 10_000 + 11 + 250 * 10);
                assert!(route.focus_order().contains(&retained));
                assert_eq!(route.action_at(retained), Some(&(9999 * 1000 + 250)));
                let node = layout.node(retained).expect("valid wide-table fixture");
                assert!(
                    node.rect
                        .intersection(node.inherited_clip)
                        .is_none_or(|r| r.width == 0.0 || r.height == 0.0)
                );
                let mut a11y = vesta_gpui::AccessibilityTree::default();
                let update = a11y.build(&table.element, &layout, Some(retained), "Wide table");
                assert_eq!(
                    update.focus,
                    a11y.node_for_element(retained)
                        .expect("valid wide-table fixture")
                );
            }
        }
    }
}

#[test]
fn header_retention_and_invalid_factories_obey_projection_contract() {
    let metrics = TableColumnMetrics::new(&[80.25; 500], 0, 0).expect("valid wide-table fixture");
    let window = metrics
        .window(320.0, 0.0, 0, FlowDirection::LeftToRight)
        .expect("valid wide-table fixture");
    let range = VirtualListRange::compute(3, 28.0, 84.0, 0.0, 0).expect("valid wide-table fixture");
    let mut headers = Vec::new();
    let table = table_view_columns(
        ids(),
        &window,
        range,
        Some(TableRetainedFocus {
            row: None,
            column: Some(250),
        }),
        |i| {
            headers.push(i);
            column(i)
        },
        |i, cols| {
            assert!(!cols.contains(&250));
            row(i, cols)
        },
        WidgetTheme::dark(),
    )
    .expect("valid wide-table fixture");
    assert_eq!(headers.last(), Some(&250));
    let route =
        ActionRoute::build(&table.element, table.actions).expect("valid wide-table fixture");
    assert!(route.focus_order().contains(&ElementId::new(2601)));
    let horizontal_only = table_view_columns(
        ids(),
        &window,
        range,
        Some(TableRetainedFocus {
            row: Some(1),
            column: Some(250),
        }),
        column,
        row,
        WidgetTheme::dark(),
    )
    .expect("visible row with offscreen focused cell");
    let layout = LayoutEngine
        .compute(&horizontal_only.element, Viewport::new(320.0, 112.0, 1.0))
        .expect("horizontal focus retention layout");
    let child = ElementId::new(100_000 + 10_000 + 11 + 250 * 10);
    let node = layout.node(child).expect("offscreen retained cell");
    assert!(
        node.rect
            .intersection(node.inherited_clip)
            .is_none_or(|r| r.width == 0.0)
    );
    let route = ActionRoute::build(&horizontal_only.element, horizontal_only.actions)
        .expect("horizontal retention route");
    assert!(route.focus_order().contains(&child));
    for focus in [
        TableRetainedFocus {
            row: Some(3),
            column: None,
        },
        TableRetainedFocus {
            row: None,
            column: Some(500),
        },
    ] {
        assert_eq!(
            table_view_columns::<usize>(
                ids(),
                &window,
                range,
                Some(focus),
                |_| panic!("invalid focus reached header"),
                |_, _| panic!("invalid focus reached row"),
                WidgetTheme::dark()
            )
            .err(),
            Some(TableViewError::InvalidViewport)
        );
    }
    let bad = table_view_columns(
        ids(),
        &window,
        range,
        None,
        |i| {
            let mut c = column(i);
            c.width = 80.0;
            c
        },
        |_, _| panic!("width mismatch reached rows"),
        WidgetTheme::dark(),
    );
    assert_eq!(bad.err(), Some(TableViewError::InvalidColumnWidth));
    let bad = table_view_columns(
        ids(),
        &window,
        range,
        None,
        column,
        |i, cols| {
            let mut r = row(i, cols);
            r.row.cells.pop();
            r
        },
        WidgetTheme::dark(),
    );
    assert_eq!(bad.err(), Some(TableViewError::CellCount));
    let bad = table_view_columns(
        ids(),
        &window,
        range,
        None,
        column,
        |i, cols| {
            let mut r = row(i, cols);
            r.bands.leading = id(1);
            r
        },
        WidgetTheme::dark(),
    );
    assert_eq!(
        bad.err(),
        Some(TableViewError::DuplicateElement(ElementId::new(1)))
    );
}
