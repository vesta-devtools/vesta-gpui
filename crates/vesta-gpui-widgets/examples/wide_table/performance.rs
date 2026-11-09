//! Equal visible-work comparison. No frozen columns in either comparison path.
use super::*;
use std::{hint::black_box, time::Instant};
use vesta_gpui::{PaintCommand, PaintList, Rect};
use vesta_gpui_widgets::table_view_windowed_scrolled_with_content;

const COLUMNS: usize = 256;
const WARM: usize = 4;
const SAMPLES: usize = 16;
const FOCUS: RecordId = RecordId(9999);

#[derive(Debug, PartialEq)]
struct Signature {
    cells: Vec<(ElementId, Rect)>,
    text: Vec<PaintCommand>,
}

fn column(index: usize) -> TableColumnContent<Action> {
    let id = ColumnId(index as u16);
    let root = identity(100 + index as u64 * 10);
    let text = identity(101 + index as u64 * 10);
    let label = format!("Field {index}");
    TableColumnContent {
        header: root,
        width: 96.0,
        sort_action: Some(Action::Sort(id)),
        resize: None,
        content: WidgetContent::new(
            &label,
            Widget::from_element(
                Element::text(text.element, text.key, &label).styled(
                    Style::default()
                        .size(Dimension::Px(82.0), Dimension::Px(28.0))
                        .text(13.0, WidgetTheme::dark().palette.text)
                        .pointer_events(false),
                ),
            ),
        ),
    }
}
fn row(record: &Record, indices: &[usize], step: usize) -> TableRowContent<Action> {
    TableRowContent {
        identity: identity(row_base(record.id)),
        selected: record.id.0 as usize == 100 + step % 4,
        select_action: Action::Select(record.id),
        cells: indices
            .iter()
            .map(|&index| {
                let id = ColumnId(index as u16);
                let base = cell_base(record.id, id);
                let child = identity(base + 1);
                let label = value(record, id);
                TableCellContent {
                    identity: identity(base),
                    selected: false,
                    select_action: None,
                    content: WidgetContent::new(
                        &label,
                        Widget::from_element(
                            Element::text(child.element, child.key, &label)
                                .semantic(Semantic::new(SemanticRole::Button, &label))
                                .styled(
                                    Style::default()
                                        .size(Dimension::Px(82.0), Dimension::Px(28.0))
                                        .focusable(true)
                                        .text(13.0, WidgetTheme::dark().palette.text),
                                ),
                        )
                        .on_click(Action::Open(record.id, id)),
                    ),
                }
            })
            .collect(),
    }
}

struct Case {
    context: TextInputContext,
    layout: RetainedLayoutEngine,
    a11y: AccessibilityTree,
}
impl Case {
    fn new(fonts: &[Arc<Vec<u8>>]) -> Self {
        let mut context = TextInputContext::default();
        for font in fonts {
            assert!(context.register_font(font.clone()));
        }
        Self {
            context,
            layout: RetainedLayoutEngine::default(),
            a11y: AccessibilityTree::default(),
        }
    }

    fn frame(
        &mut self,
        records: &[Record],
        metrics: &TableColumnMetrics,
        lazy: bool,
        step: usize,
    ) -> Result<(f64, Signature)> {
        let start = Instant::now();
        let offset = 1920.0 + (step % 2) as f32 * 96.0;
        let range = VirtualListRange::compute(records.len(), 28.0, 112.0, 2800.0, 0)?;
        let ids = TableWindowIds {
            root: identity(1),
            header: identity(2),
            viewport: identity(3),
            content: identity(4),
        };
        let mut cells = 0;
        let mut table = if lazy {
            let window = metrics.window(720.0, offset, 1, FlowDirection::LeftToRight)?;
            table_view_columns(
                WideTableIds {
                    table: ids,
                    header_bands: bands(5),
                },
                &window,
                range,
                Some(TableRetainedFocus {
                    row: Some(9999),
                    column: Some(125),
                }),
                column,
                |index, indices| {
                    cells += indices.len();
                    TableWindowRow {
                        bands: bands(row_base(records[index].id) + 1),
                        label: records[index].name.clone(),
                        row: row(&records[index], indices, step),
                    }
                },
                WidgetTheme::dark(),
            )?
        } else {
            let all = (0..COLUMNS).collect::<Vec<_>>();
            let mut table = table_view_windowed_scrolled_with_content(
                ids,
                (0..COLUMNS).map(column).collect(),
                range,
                offset,
                Some(9999),
                |index| {
                    cells += all.len();
                    row(&records[index], &all, step)
                },
                WidgetTheme::dark(),
            )?;
            table.element.style.width = Dimension::Px(720.0);
            table.element.style.max_width = Dimension::Px(720.0);
            let viewport = &mut table.element.children[1];
            viewport.style.width = Dimension::Px(720.0);
            viewport.style.scrollbar.gutter = vesta_gpui::ScrollbarGutter::Overlay;
            table
        };
        // The same Host row name in both paths. The legacy projector otherwise
        // joins every cell label, which exceeds the bounded Agent text budget
        // for this source and is not a useful stable record name.
        for (index, shell) in (range.first..range.end)
            .chain(std::iter::once(9999))
            .zip(&mut table.element.children[1].children[0].children)
        {
            assert_eq!(shell.id, identity(row_base(records[index].id)).element);
            shell.semantic = Some(
                Semantic::new(SemanticRole::Row, &records[index].name)
                    .selected(records[index].id.0 as usize == 100 + step % 4),
            );
        }
        let built_ms = start.elapsed().as_secs_f64() * 1000.0;
        let viewport = Viewport::new(720.0, 140.0, 1.5);
        let layout =
            self.layout
                .compute_with_text_context(&table.element, viewport, &mut self.context)?;
        let layout_ms = start.elapsed().as_secs_f64() * 1000.0 - built_ms;
        let route = ActionRoute::build(&table.element, table.actions)?;
        let hits = HitTestTree::build(&table.element, &layout);
        let focus = identity(cell_base(FOCUS, ColumnId(125)) + 1).element;
        let accessible = self
            .a11y
            .build(&table.element, &layout, Some(focus), "Wide comparison");
        let snapshot = UiSnapshot::capture(
            UiSnapshotFrame {
                id: UiSnapshotId {
                    instance: 1,
                    window: UiWindowId(1),
                    revision: step as u64 + 1,
                },
                root: &table.element,
                layout: &layout,
                route: &route,
                focus: Some(InputTarget {
                    window: UiWindowId(1),
                    element: focus,
                }),
                modal_root: None,
            },
            UiQueryLimits::default(),
            |_| None,
            |_| false,
        )?;
        let paint = PaintList::build(&table.element, &layout);
        black_box((&accessible, &snapshot, &paint));
        let total_ms = start.elapsed().as_secs_f64() * 1000.0;
        // Validation is outside the timed interval, identical for both paths.
        assert!(
            !snapshot.node_limit_reached && !snapshot.text_truncated,
            "complete inspection required: lazy={lazy} nodes={} node_limit={} text_truncated={}",
            snapshot.nodes().len(),
            snapshot.node_limit_reached,
            snapshot.text_truncated
        );
        assert!(route.focus_order().contains(&focus));
        assert!(
            snapshot
                .nodes()
                .iter()
                .any(|node| node.id == focus && node.focused)
        );
        assert_eq!(
            accessible.focus,
            self.a11y.node_for_element(focus).expect("retained focus")
        );
        assert_eq!(
            layout
                .node(ElementId::new(3))
                .expect("body viewport")
                .scroll_content,
            Some((24576.0, 280000.0))
        );
        let mut visible = Vec::new();
        for record in &records[range.first..range.end] {
            for ci in 0..COLUMNS {
                let id = identity(cell_base(record.id, ColumnId(ci as u16)) + 1).element;
                if let Some(node) = layout.node(id)
                    && let Some(clip) = node.rect.intersection(node.inherited_clip)
                    && clip.width > 0.0
                    && clip.height > 0.0
                {
                    let hit = hits
                        .hit(Point::new(
                            clip.x + clip.width / 2.0,
                            clip.y + clip.height / 2.0,
                        ))
                        .expect("visible cell hit");
                    assert_eq!(
                        route.action_for_hit(hit),
                        Some(&Action::Open(record.id, ColumnId(ci as u16)))
                    );
                    visible.push((id, clip));
                }
            }
        }
        assert_eq!(visible.len(), 32);
        if step >= WARM {
            println!(
                "sample={} path={} cells={cells} nodes={} build_ms={built_ms:.3} layout_ms={layout_ms:.3} total_ms={total_ms:.3}",
                step - WARM,
                if lazy { "columns" } else { "all" },
                layout.nodes().len()
            );
        }
        let text = paint.commands().iter().filter(|command| {
            matches!(command, PaintCommand::Text {bounds, clip, ..}
                if bounds.intersection(*clip).is_some_and(|rect| rect.width > 0.0 && rect.height > 0.0))
        }).cloned().collect::<Vec<_>>();
        assert_eq!(text.len(), 40, "32 live cells and eight headers");
        Ok((
            total_ms,
            Signature {
                cells: visible,
                text,
            },
        ))
    }
}

pub(super) fn run(fonts: &[Arc<Vec<u8>>]) -> Result<()> {
    let records = (0..10_000)
        .map(|id| Record {
            id: RecordId(id),
            name: format!("Asset {id}"),
        })
        .collect::<Vec<_>>();
    let metrics = TableColumnMetrics::new(&[96.0; COLUMNS], 0, 0)?;
    let mut cases = [Case::new(fonts), Case::new(fonts)];
    let mut times: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
    for step in 0..WARM + SAMPLES {
        let mut signatures = [None, None];
        // Alternate first path to retain shared-machine effects in both samples.
        for which in [step % 2, 1 - step % 2] {
            let (ms, signature) = cases[which].frame(&records, &metrics, which == 1, step)?;
            signatures[which] = Some(signature);
            if step >= WARM {
                times[which].push(ms);
            }
        }
        assert_eq!(
            signatures[0], signatures[1],
            "equal visible text paint, geometry, identities and hit actions"
        );
    }
    for (which, samples) in times.iter_mut().enumerate() {
        samples.sort_by(f64::total_cmp);
        println!(
            "summary path={} samples={} total_ms_min={:.3} median={:.3} max={:.3}",
            if which == 0 { "all" } else { "columns" },
            samples.len(),
            samples[0],
            samples[SAMPLES / 2],
            samples[SAMPLES - 1]
        );
    }
    println!(
        "CPU dev profile only: build + real-font retained layout + routes/hits + AccessKit + complete bounded Agent snapshot + paint; excludes source/metrics creation, validation, renderer/GPU/native input and physical display latency. No frozen columns in this equal-output comparison."
    );
    Ok(())
}
