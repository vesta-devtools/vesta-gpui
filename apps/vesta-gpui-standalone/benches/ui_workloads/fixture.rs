use super::*;
use vesta_gpui::{
    ActionRoute, Axis, Dimension, Element, ElementId, ElementKey, HitTestTree, PaintCommand,
    PaintList, Point, RetainedLayoutEngine, Style, TextEditKind, TextInputAction, TextInputContext,
    TextInputState, TextRasterizer, Viewport, text_input_wrap_width,
};
use vesta_gpui_vulkan::{RenderOptions, prepare_paint};
use vesta_gpui_widgets::{
    VirtualListIds, VirtualListRange, Widget, WidgetIdentity, WidgetTheme, virtual_list,
};

const TEXT: &str = "A reproducible frame 中文排版 and ordinary editor content. ";
const INPUT: u64 = 900;
pub const WARMUP: usize = 4;

#[derive(Clone, Copy)]
pub enum Kind {
    Static,
    Resize,
    Edit,
    Collection,
}
impl Kind {
    pub const ALL: [Self; 4] = [Self::Static, Self::Resize, Self::Edit, Self::Collection];
    pub fn name(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Resize => "resize",
            Self::Edit => "edit",
            Self::Collection => "collection",
        }
    }
}

pub struct Fixture {
    kind: Kind,
    context: TextInputContext,
    layout: RetainedLayoutEngine,
    rasterizer: TextRasterizer,
    input: TextInputState,
    items: Vec<u64>,
}

pub struct Frame {
    pub ms: f64,
    pub signature: String,
    pub nodes: usize,
    pub rows: usize,
    pub passes: usize,
    pub batches: usize,
    pub vertices: usize,
}
impl Frame {
    pub fn json(&self, step: usize) -> Value {
        json!({"step":step,"wall_ms":self.ms,"signature":self.signature,
            "layout_nodes":self.nodes,"projected_rows":self.rows,"layout_passes":self.passes,
            "cpu_batches":self.batches,"cpu_vertices":self.vertices})
    }
}

fn id(value: u64) -> ElementId {
    ElementId::new(value)
}
fn key(value: u64) -> ElementKey {
    ElementKey::new(format!("workload.{value}"))
}
fn identity(value: u64) -> WidgetIdentity {
    WidgetIdentity::new(id(value), format!("workload.{value}"))
}

impl Fixture {
    pub fn new(kind: Kind, fonts: &FontSet) -> AnyResult<Self> {
        let mut context = TextInputContext::default();
        for (_, bytes) in &fonts.blobs {
            assert!(context.register_font(bytes.clone()), "font registration");
        }
        let mut input = TextInputState::new_multiline(14.0, "Segoe UI", 880.0);
        input.reset(&mut context, &TEXT.repeat(12))?;
        Ok(Self {
            kind,
            context,
            layout: RetainedLayoutEngine::default(),
            rasterizer: TextRasterizer::default(),
            input,
            items: (1..=100_000).map(|i| 1_000_000 + i).collect(),
        })
    }

    fn tree(&self, step: usize, width: f32) -> AnyResult<(Element, usize)> {
        let theme = WidgetTheme::dark();
        let mut root = Element::container(id(1), key(1)).styled(
            Style::default()
                .size(Dimension::Px(width), Dimension::Px(480.0))
                .axis(Axis::Column)
                .background(theme.palette.canvas),
        );
        match self.kind {
            Kind::Static => {
                for row in 0..64 {
                    let value = 10 + row;
                    root = root.child(
                        Element::text(
                            id(value),
                            key(value),
                            format!("Stable row {row} — 中文 and text"),
                        )
                        .styled(
                            Style::default()
                                .size(Dimension::Percent(1.0), Dimension::Px(24.0))
                                .text(14.0, theme.palette.text)
                                .flex(0.0, 0.0),
                        ),
                    );
                }
                Ok((root, 0))
            }
            Kind::Resize | Kind::Edit => Ok((
                root.child(
                    Element::text_input(id(INPUT), key(INPUT), self.input.snapshot(), true).styled(
                        Style::default().size(Dimension::Percent(1.0), Dimension::Px(320.0)),
                    ),
                ),
                0,
            )),
            Kind::Collection => {
                let offset = [0.0, 24_000.0, 720_000.0, 1_440_000.0, 240.0][step % 5];
                let range = VirtualListRange::compute(self.items.len(), 24.0, 480.0, offset, 3)?;
                let mut count = 0;
                let list: Widget<()> = virtual_list(
                    VirtualListIds {
                        root: identity(2),
                        content: identity(3),
                    },
                    Dimension::Percent(1.0),
                    range,
                    Some(1),
                    |index| {
                        count += 1;
                        let value = self.items[index];
                        Widget::from_element(
                            Element::text(id(value), key(value), format!("Row {value} — 中文"))
                                .styled(Style::default().text(14.0, theme.palette.text)),
                        )
                    },
                );
                assert!(
                    count <= 28,
                    "projection must be bounded by viewport plus one retained item"
                );
                assert!(count > 0);
                Ok((root.child(list.element), count))
            }
        }
    }

    pub fn frame(&mut self, step: usize) -> AnyResult<Frame> {
        let start = Instant::now();
        let frame_guard = trace::frame("Workload.Main");
        if matches!(self.kind, Kind::Edit) {
            if step % 2 == 0 {
                self.input
                    .insert(&mut self.context, "增", TextEditKind::Typing)?;
            } else {
                assert!(
                    self.input.act(&mut self.context, TextInputAction::Undo),
                    "real undo"
                );
            }
        }
        let width = if matches!(self.kind, Kind::Resize) {
            [900.0, 720.0, 480.0, 720.0, 900.0][step % 5]
        } else {
            900.0
        };
        let viewport = Viewport::new(width, 480.0, 1.0);
        let mut passes = 0;
        let (root, layout, rows) = loop {
            passes += 1;
            assert!(passes <= 8, "finite UI reconciliation");
            let (root, rows) = {
                let _build = trace::span("workload.host", "build.tree");
                self.tree(step, width)?
            };
            let layout =
                self.layout
                    .compute_with_text_context(&root, viewport, &mut self.context)?;
            if matches!(self.kind, Kind::Resize | Kind::Edit) {
                let bounds = layout.node(id(INPUT)).expect("input layout").rect;
                if self
                    .input
                    .set_wrap_width(&mut self.context, text_input_wrap_width(bounds))
                {
                    trace::instant("workload.host", "layout.changed");
                    continue;
                }
            }
            break (root, layout, rows);
        };
        let route = ActionRoute::<()>::build(&root, std::iter::empty())?;
        let hit = HitTestTree::build(&root, &layout);
        let paint = PaintList::build(&root, &layout);
        let theme = WidgetTheme::dark();
        let prepared = prepare_paint(
            viewport,
            &paint,
            &mut self.context,
            &mut self.rasterizer,
            None,
            4096,
            RenderOptions {
                clear: [0.0, 0.0, 0.0, 1.0],
                selection: theme.palette.selection,
                caret: theme.palette.focus,
                caret_visible: false,
            },
        )?;
        trace::counter("workload.host", "layout.nodes", layout.nodes().len() as f64);
        trace::counter("workload.host", "projected.rows", rows as f64);
        trace::counter("workload.host", "layout.passes", passes as f64);
        drop(frame_guard);
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        // Check/serialize after the timed frame, without recording extra work.
        assert!(ms.is_finite() && ms >= 0.0);
        assert!(hit.hit(Point::new(4.0, 4.0)).is_some());
        std::hint::black_box(&route);
        assert!(prepared.batch_count() > 0 && prepared.vertex_count() > 0);
        if matches!(self.kind, Kind::Collection) {
            assert!(layout.nodes().len() <= 31);
        }
        if matches!(self.kind, Kind::Resize | Kind::Edit) {
            let expected = paint
                .commands()
                .iter()
                .find_map(|command| match command {
                    PaintCommand::TextInput {
                        element,
                        bounds,
                        clip,
                        snapshot,
                        ..
                    } if *element == id(INPUT) => {
                        Some(snapshot.viewport(*bounds, *clip, Point::default()))
                    }
                    _ => None,
                })
                .expect("CPU input projection");
            assert_eq!(
                prepared
                    .output()
                    .text_inputs
                    .iter()
                    .find(|(e, _)| *e == id(INPUT))
                    .map(|(_, v)| *v),
                Some(expected)
            );
        }
        let mut hash = 0xcbf29ce484222325_u64;
        for node in layout.nodes() {
            for value in [
                node.id.get(),
                u64::from(node.rect.x.to_bits()),
                u64::from(node.rect.y.to_bits()),
                u64::from(node.rect.width.to_bits()),
                u64::from(node.rect.height.to_bits()),
            ] {
                for byte in value.to_le_bytes() {
                    hash ^= u64::from(byte);
                    hash = hash.wrapping_mul(0x100000001b3);
                }
            }
        }
        for value in [
            paint.commands().len(),
            prepared.batch_count(),
            prepared.vertex_count(),
            rows,
            self.input.text().len(),
        ] {
            hash ^= value as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        Ok(Frame {
            ms,
            signature: format!("{hash:016x}"),
            nodes: layout.nodes().len(),
            rows,
            passes,
            batches: prepared.batch_count(),
            vertices: prepared.vertex_count(),
        })
    }
}
