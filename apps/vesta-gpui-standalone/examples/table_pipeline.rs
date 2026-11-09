//! Real-font dense-table CPU pipeline. No window, GPU or native latency claim.
use serde_json::{Value, json};
use std::{error::Error, fs, hint::black_box, sync::Arc, time::Instant};
use vesta_gpui::{
    AccessibilityTree, ActionRoute, ElementId, HitTestTree, PaintCommand, PaintList, Point,
    RetainedLayoutEngine, TextInputContext, TextRasterizer, UiQueryLimits, UiSnapshot,
    UiSnapshotFrame, UiSnapshotId, UiWindowId, Viewport,
};
use vesta_gpui_vulkan::{RenderOptions, prepare_paint};
use vesta_gpui_widgets::{
    TableCell, TableColumn, TableRow, Widget, WidgetIdentity, WidgetTheme, table_view,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
#[path = "table_pipeline/gpu.rs"]
mod gpu;
const ROWS: usize = 32;
const WARMUP: usize = 4;
const SAMPLES: usize = 32;

fn identity(value: usize) -> WidgetIdentity {
    WidgetIdentity::new(ElementId::new(value as u64), format!("table.probe.{value}"))
}

fn tree(columns: usize, step: usize) -> Result<Widget<usize>> {
    let cols = (0..columns)
        .map(|column| TableColumn {
            header: identity(10 + column),
            title: format!("Column {column}"),
            width: 96.0,
            sort_action: Some(column),
            resize: None,
        })
        .collect();
    let rows = (0..ROWS)
        .map(|row| TableRow {
            identity: identity(1000 + row * 100),
            cells: (0..columns)
                .map(|column| TableCell {
                    identity: identity(1001 + row * 100 + column),
                    value: format!("{row}:{column} 中文 item"),
                    selected: row == step % ROWS && column == 1,
                    select_action: (column == 1).then_some(1000 + row),
                })
                .collect(),
            selected: row == step % ROWS,
            select_action: 100 + row,
        })
        .collect();
    Ok(table_view(
        identity(1),
        identity(2),
        cols,
        rows,
        WidgetTheme::dark(),
    )?)
}

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn measure(columns: usize, fonts: &[Arc<Vec<u8>>]) -> Result<Value> {
    let theme = WidgetTheme::dark();
    let viewport = Viewport::new(
        columns as f32 * 96.0 + 2.0,
        (ROWS + 1) as f32 * theme.metrics.control_height + 2.0,
        1.5,
    );
    let mut context = TextInputContext::default();
    for bytes in fonts {
        assert!(context.register_font(bytes.clone()));
    }
    let mut layout_engine = RetainedLayoutEngine::default();
    let mut rasterizer = TextRasterizer::default();
    let mut accessibility = AccessibilityTree::default();
    let mut samples = Vec::new();
    for step in 0..WARMUP + SAMPLES {
        let total = Instant::now();
        let start = Instant::now();
        let widget = tree(columns, step)?;
        let build_ms = ms(start);
        let start = Instant::now();
        let layout =
            layout_engine.compute_with_text_context(&widget.element, viewport, &mut context)?;
        let layout_ms = ms(start);
        let start = Instant::now();
        let route = ActionRoute::build(&widget.element, widget.actions)?;
        let route_ms = ms(start);
        let start = Instant::now();
        let hit = HitTestTree::build(&widget.element, &layout);
        let hit_ms = ms(start);
        let start = Instant::now();
        let accessible = accessibility.build(&widget.element, &layout, None, "Table probe");
        let accessibility_ms = ms(start);
        let start = Instant::now();
        let snapshot = UiSnapshot::capture(
            UiSnapshotFrame {
                id: UiSnapshotId {
                    instance: 1,
                    window: UiWindowId(1),
                    revision: step as u64 + 1,
                },
                root: &widget.element,
                layout: &layout,
                route: &route,
                focus: None,
                modal_root: None,
            },
            UiQueryLimits::default(),
            |_| None,
            |_| false,
        )?;
        let inspection_ms = ms(start);
        let start = Instant::now();
        let paint = PaintList::build(&widget.element, &layout);
        let paint_ms = ms(start);
        let start = Instant::now();
        let prepared = prepare_paint(
            viewport,
            &paint,
            &mut context,
            &mut rasterizer,
            None,
            4096,
            RenderOptions {
                clear: [0.0, 0.0, 0.0, 1.0],
                selection: theme.palette.selection,
                caret: theme.palette.focus,
                caret_visible: false,
            },
        )?;
        let prepare_ms = ms(start);
        let total_ms = ms(total);
        // Verify the realized presentation after measuring, with the real cell identity.
        let id = identity(1002).element;
        let rect = layout.node(id).expect("cell layout").rect;
        assert_eq!(hit.hit(Point::new(rect.x + 2.0, rect.y + 2.0)), Some(id));
        assert_eq!(route.action_at(id), Some(&1000));
        assert!(route.focus_order().contains(&id));
        assert!(accessibility.node_for_element(id).is_some());
        assert!(
            snapshot
                .nodes()
                .iter()
                .any(|node| node.id == id && node.name.value.as_deref() == Some("0:1 中文 item"))
        );
        assert!(!snapshot.node_limit_reached && !snapshot.text_truncated);
        assert_eq!(layout.nodes().len(), 2 + columns + ROWS * (1 + columns));
        black_box(accessible);
        if step >= WARMUP {
            samples.push(json!({
                "step":step,"build_ms":build_ms,"layout_ms":layout_ms,"route_ms":route_ms,
                "hit_ms":hit_ms,"accessibility_ms":accessibility_ms,"inspection_ms":inspection_ms,
                "paint_ms":paint_ms,"prepare_ms":prepare_ms,"total_ms":total_ms,
                "nodes":layout.nodes().len(),"paint_commands":paint.commands().len(),
                "text_commands":paint.commands().iter().filter(|c|matches!(c,PaintCommand::Text{..})).count(),
                "cpu_batches":prepared.batch_count(),"cpu_vertices":prepared.vertex_count(),
            }));
        }
    }
    Ok(json!({"rows":ROWS,"columns":columns,"scale":viewport.scale_factor,"samples":samples}))
}

fn main() -> Result<()> {
    let mut fonts = Vec::new();
    let mut manifest = Vec::new();
    for path in [
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\msyh.ttc",
    ] {
        let bytes = fs::read(path)?;
        manifest.push(json!({"path":path,"bytes":bytes.len()}));
        fonts.push(Arc::new(bytes));
    }
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.get(1).is_some_and(|arg| arg == "--gpu") {
        return gpu::run(&fonts, std::path::Path::new(&args[0]));
    }
    let records = [8, 24]
        .into_iter()
        .map(|columns| measure(columns, &fonts))
        .collect::<Result<Vec<_>>>()?;
    let output = serde_json::to_string_pretty(&json!({
        "scope":"warm CPU stages, real fonts; excludes upload, submission, GPU and display",
        "warmup":WARMUP,"fonts":manifest,"records":records,
    }))?;
    if let Some(path) = args.first() {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        std::io::Write::write_all(&mut file, output.as_bytes())?;
    } else {
        println!("{output}");
    }
    Ok(())
}
