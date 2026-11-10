//! Host-owned records, column order, focus and scroll over the public 2D projector.
//! Pass Host font paths; this consumer opens no native window or clipboard.
use std::{error::Error, sync::Arc};
use vesta_gpui::{
    AccessibilityTree, ActionRoute, Dimension, Element, ElementId, FlowDirection, HitTestTree,
    InputTarget, Point, RetainedLayoutEngine, Semantic, SemanticRole, Style, TextInputContext,
    UiQueryLimits, UiSnapshot, UiSnapshotFrame, UiSnapshotId, UiWindowId, Viewport,
};
use vesta_gpui_widgets::{
    ScrollAreaState, TableBandIds, TableCellContent, TableColumnContent, TableColumnMetrics,
    TableRetainedFocus, TableRowContent, TableWindowIds, TableWindowRow, VirtualListRange,
    WideTableIds, Widget, WidgetContent, WidgetIdentity, WidgetTheme, table_view_columns,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RecordId(u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ColumnId(u16);
struct Record {
    id: RecordId,
    name: String,
}
struct Column {
    id: ColumnId,
    width: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Open(RecordId, ColumnId),
    Select(RecordId),
    Sort(ColumnId),
}

fn identity(value: u64) -> WidgetIdentity {
    WidgetIdentity::new(ElementId::new(value), format!("wide.{value}"))
}
fn bands(base: u64) -> TableBandIds {
    TableBandIds {
        leading: identity(base),
        scrolling: identity(base + 1),
        trailing: identity(base + 2),
    }
}
fn row_base(id: RecordId) -> u64 {
    100_000 + u64::from(id.0) * 10_000
}
fn cell_base(row: RecordId, column: ColumnId) -> u64 {
    row_base(row) + 10 + u64::from(column.0) * 10
}
fn value(row: &Record, column: ColumnId) -> String {
    match column.0 {
        0 => row.name.clone(),
        _ => format!("{}:{} 中文", row.id.0, column.0),
    }
}

struct Host {
    records: Vec<Record>,
    columns: Vec<Column>,
    focus: Option<(RecordId, ColumnId)>,
    scroll: ScrollAreaState,
    width: f32,
    flow: FlowDirection,
    theme: WidgetTheme,
    context: TextInputContext,
    layout: RetainedLayoutEngine,
    a11y: AccessibilityTree,
    revision: u64,
}

impl Host {
    fn geometry(&self) -> TableColumnMetrics {
        TableColumnMetrics::new(
            &self.columns.iter().map(|c| c.width).collect::<Vec<_>>(),
            1,
            1,
        )
        .expect("live Host table geometry and identity")
    }

    fn reveal_focus(&mut self) {
        let Some((row, column)) = self.focus else {
            return;
        };
        let Some(ri) = self.records.iter().position(|r| r.id == row) else {
            return;
        };
        let Some(ci) = self.columns.iter().position(|c| c.id == column) else {
            return;
        };
        let geometry = self.geometry();
        let window = geometry
            .window(self.width, self.scroll.offset().x, 1, self.flow)
            .expect("live Host table geometry and identity");
        let offset = window
            .offset_to_reveal(ci)
            .expect("live Host table geometry and identity");
        let y = ri as f32 * 28.0;
        self.scroll.scroll_by_point(Point::new(
            self.flow.horizontal(offset - self.scroll.offset().x),
            y - self.scroll.offset_y(),
        ));
    }

    // Host source-order copying across unrealized cells. This fixture's values
    // contain no TSV delimiters; the Host owns clipboard I/O and export policy.
    fn copy_rectangle(
        &self,
        rows: std::ops::Range<usize>,
        columns: std::ops::Range<usize>,
    ) -> String {
        self.records[rows]
            .iter()
            .map(|row| {
                self.columns[columns.clone()]
                    .iter()
                    .map(|column| value(row, column.id))
                    .collect::<Vec<_>>()
                    .join("\t")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn frame(&mut self, label: &str) -> Result<bool> {
        let geometry = self.geometry();
        let window = geometry.window(self.width, self.scroll.offset().x, 1, self.flow)?;
        let range =
            VirtualListRange::compute(self.records.len(), 28.0, 168.0, self.scroll.offset_y(), 1)?;
        let retained = self.focus.and_then(|(record, column)| {
            Some(TableRetainedFocus {
                row: Some(self.records.iter().position(|r| r.id == record)?),
                column: Some(self.columns.iter().position(|c| c.id == column)?),
            })
        });
        let theme = self.theme;
        let mut header_count = 0;
        let mut row_count = 0;
        let mut cell_count = 0;
        let table = table_view_columns(
            WideTableIds {
                table: TableWindowIds {
                    root: identity(1),
                    header: identity(2),
                    viewport: identity(3),
                    content: identity(4),
                },
                header_bands: bands(5),
            },
            &window,
            range,
            retained,
            |index| {
                header_count += 1;
                let column = &self.columns[index];
                let base = 100 + u64::from(column.id.0) * 10;
                let caption = format!("Field {}", column.id.0);
                let text = identity(base + 1);
                TableColumnContent {
                    header: identity(base),
                    content: WidgetContent::new(
                        &caption,
                        Widget::from_element(
                            Element::text(text.element, text.key, &caption).styled(
                                Style::default()
                                    .text(13.0, theme.palette.text)
                                    .pointer_events(false),
                            ),
                        ),
                    ),
                    width: column.width,
                    sort_action: Some(Action::Sort(column.id)),
                    resize: None,
                }
            },
            |index, columns| {
                row_count += 1;
                cell_count += columns.len();
                let record = &self.records[index];
                TableWindowRow {
                    bands: bands(row_base(record.id) + 1),
                    label: record.name.clone(),
                    row: TableRowContent {
                        identity: identity(row_base(record.id)),
                        selected: self.focus.is_some_and(|f| f.0 == record.id),
                        select_action: Action::Select(record.id),
                        cells: columns
                            .iter()
                            .map(|&column| {
                                let column = self.columns[column].id;
                                let base = cell_base(record.id, column);
                                let button = identity(base + 1);
                                let caption = value(record, column);
                                TableCellContent {
                                    identity: identity(base),
                                    content: WidgetContent::new(
                                        &caption,
                                        Widget::from_element(
                                            Element::text(button.element, button.key, &caption)
                                                .semantic(Semantic::new(
                                                    SemanticRole::Button,
                                                    &caption,
                                                ))
                                                .styled(
                                                    Style::default()
                                                        .size(
                                                            Dimension::Percent(1.0),
                                                            Dimension::Percent(1.0),
                                                        )
                                                        .focusable(true)
                                                        .text(13.0, theme.palette.text),
                                                ),
                                        )
                                        .on_click(Action::Open(record.id, column)),
                                    ),
                                    selected: self.focus == Some((record.id, column)),
                                    select_action: None,
                                }
                            })
                            .collect(),
                    },
                }
            },
            theme,
        )?;
        assert_eq!(header_count, window.indices().len());
        assert!(
            row_count <= 10 && header_count <= 14 && cell_count <= row_count * header_count + 1
        );
        let viewport = Viewport::new(self.width, 196.0, 1.5);
        let layout =
            self.layout
                .compute_with_text_context(&table.element, viewport, &mut self.context)?;
        let route = ActionRoute::build(&table.element, table.actions)?;
        let hits = HitTestTree::build(&table.element, &layout);
        self.scroll.sync_layout(
            layout
                .node(ElementId::new(3))
                .expect("live Host table geometry and identity"),
        );
        let focus = self
            .focus
            .map(|(record, column)| identity(cell_base(record, column) + 1).element);
        let update = self
            .a11y
            .build(&table.element, &layout, focus, "Wide records");
        self.revision += 1;
        let snapshot = UiSnapshot::capture(
            UiSnapshotFrame {
                id: UiSnapshotId {
                    instance: 1,
                    window: UiWindowId(1),
                    revision: self.revision,
                },
                root: &table.element,
                layout: &layout,
                route: &route,
                focus: focus.map(|element| InputTarget {
                    window: UiWindowId(1),
                    element,
                }),
                modal_root: None,
            },
            UiQueryLimits::default(),
            |_| None,
            |_| false,
        )?;
        assert!(!snapshot.node_limit_reached && !snapshot.text_truncated);
        let mut focused_visible = false;
        if let Some((record, column)) = self.focus {
            let child = identity(cell_base(record, column) + 1).element;
            assert!(route.focus_order().contains(&child));
            assert_eq!(route.action_at(child), Some(&Action::Open(record, column)));
            assert_eq!(
                update.focus,
                self.a11y
                    .node_for_element(child)
                    .expect("live Host table geometry and identity")
            );
            assert!(
                snapshot
                    .nodes()
                    .iter()
                    .any(|node| node.id == child && node.focused)
            );
            let record_name = &self
                .records
                .iter()
                .find(|row| row.id == record)
                .expect("focused record remains in Host source")
                .name;
            assert!(
                snapshot
                    .nodes()
                    .iter()
                    .any(|node| node.id == identity(row_base(record)).element
                        && node.name.value.as_deref() == Some(record_name.as_str()))
            );
            let node = layout.node(child).expect("retained focused child");
            if let Some(clip) = node
                .rect
                .intersection(node.inherited_clip)
                .filter(|clip| clip.width > 1.0 && clip.height > 1.0)
            {
                let hit = hits
                    .hit(Point::new(
                        clip.x + clip.width / 2.0,
                        clip.y + clip.height / 2.0,
                    ))
                    .expect("revealed focused child is hittable");
                assert_eq!(
                    route.action_for_hit(hit),
                    Some(&Action::Open(record, column))
                );
                focused_visible = true;
            }
        }
        if let Some(anchor) = range.anchor() {
            let record = &self.records[anchor.index()];
            for column in [
                &self.columns[0],
                self.columns
                    .last()
                    .expect("live Host table geometry and identity"),
            ] {
                let id = identity(cell_base(record.id, column.id) + 1).element;
                let node = layout
                    .node(id)
                    .expect("live Host table geometry and identity");
                let clip = node
                    .rect
                    .intersection(node.inherited_clip)
                    .expect("live Host table geometry and identity");
                let hit = hits
                    .hit(Point::new(
                        clip.x + clip.width / 2.0,
                        clip.y + clip.height / 2.0,
                    ))
                    .expect("live Host table geometry and identity");
                assert_eq!(
                    route.action_for_hit(hit),
                    Some(&Action::Open(record.id, column.id))
                );
            }
        }
        assert!((self.scroll.offset().x - window.offset()).abs() < 1.0);
        println!(
            "{label}: source={}x{} projected={row_count}x{header_count} cells={cell_count} offset={:?} flow={:?}",
            self.records.len(),
            self.columns.len(),
            self.scroll.offset(),
            self.flow
        );
        Ok(focused_visible)
    }
}

fn main() -> Result<()> {
    let mut context = TextInputContext::default();
    let paths = std::env::args().skip(1).collect::<Vec<_>>();
    if paths.is_empty() {
        return Err("pass one or more Host font paths".into());
    }
    for path in paths {
        let bytes = Arc::new(std::fs::read(path)?);
        assert!(context.register_font(bytes));
    }
    let mut host = Host {
        records: (0..10_000)
            .map(|id| Record {
                id: RecordId(id),
                name: format!("Asset {id}"),
            })
            .collect(),
        columns: (0..512)
            .map(|id| Column {
                id: ColumnId(id),
                width: 96.0,
            })
            .collect(),
        focus: Some((RecordId(9999), ColumnId(250))),
        scroll: ScrollAreaState::default(),
        width: 720.0,
        flow: FlowDirection::LeftToRight,
        theme: WidgetTheme::dark(),
        context,
        layout: RetainedLayoutEngine::default(),
        a11y: AccessibilityTree::default(),
        revision: 0,
    };
    host.frame("initial retained cell")?;
    host.scroll.scroll_by_point(Point::new(4030.5, 2800.0));
    host.frame("scroll both axes")?;
    host.reveal_focus();
    assert!(host.frame("reveal focused cell")?);
    let copied = host.copy_rectangle(9997..10_000, 249..252);
    assert_eq!(copied.lines().count(), 3);
    assert!(copied.contains("9999:250 中文"));
    host.records.reverse();
    host.columns.swap(250, 400);
    host.frame("reorder source retains child identity")?;
    host.reveal_focus();
    assert!(host.frame("reveal after reorder")?);
    host.columns[400].width = 143.5;
    host.width = 420.5;
    host.frame("resize column and viewport")?;
    host.flow = FlowDirection::RightToLeft;
    host.frame("RTL frozen logical ends")?;
    host.focus = Some((RecordId(42), ColumnId(125)));
    host.reveal_focus();
    assert!(host.frame("reveal in RTL")?);
    host.theme = WidgetTheme::light();
    host.frame("light theme")?;
    host.focus = None;
    host.records.clear();
    host.frame("empty rows")?;
    host.records.push(Record {
        id: RecordId(10001),
        name: "Restart".into(),
    });
    host.frame("restart")?;
    Ok(())
}
