//! Public API CPU-stage probe; no OS window, renderer, or FPS assertion.

use std::{
    collections::HashMap,
    hint::black_box,
    sync::Arc,
    time::{Duration, Instant},
};
use vesta_gpui::{
    AccessibilityTree, ActionRoute, Axis, Color, Dimension, Element, ElementIdAllocator,
    ElementKey, ElementScope, FlowDirection, FontRole, HitTestTree, Insets, LayoutEngine,
    PaintList, Point, RetainedLayoutEngine, ScrollbarGutter, ScrollbarStyle, Semantic,
    SemanticRole, Style, TextInputContext, Viewport,
};
use vesta_gpui_widgets::{TooltipController, TooltipFrame, WidgetTheme};

const ROWS: u32 = 128;
const WARMUP: usize = 24;
const SAMPLES: usize = 160;
const BLOCKS: usize = 4;

#[derive(Clone, Copy, Debug)]
enum Projection {
    Stateless,
    Retained,
    Tooltip,
}

#[derive(Clone, Copy, Debug)]
enum Scenario {
    Steady,
    PaintAndScroll,
    TextAndResize,
}

fn tree(scope: ElementScope, scenario: Scenario, frame: usize) -> Element {
    let mut root = Element::scroll_box(
        scope.element(1),
        ElementKey::new("probe.scroll"),
        Point::new(
            0.0,
            if matches!(scenario, Scenario::PaintAndScroll) {
                (frame % 20) as f32 * 20.0
            } else {
                0.0
            },
        ),
    )
    .styled(
        Style::default()
            .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
            .scrollbar(ScrollbarStyle {
                gutter: ScrollbarGutter::Reserve,
                ..ScrollbarStyle::default()
            })
            .flow_direction(FlowDirection::LeftToRight),
    );
    for row in 0..ROWS {
        let part = 10 + row * 8;
        let name = if matches!(scenario, Scenario::TextAndResize) && row == 0 {
            format!("Object {row:05} revision {}", frame % 2)
        } else {
            format!("Object {row:05} · Vesta")
        };
        let mut item = Element::container(
            scope.element(part),
            ElementKey::new(format!("probe.row.{row}")),
        )
        .styled(
            Style::default()
                .axis(Axis::Row)
                .size(Dimension::Percent(1.0), Dimension::Px(44.0))
                .gap(8.0)
                .padding(Insets::all(4.0)),
        )
        .semantic(Semantic::new(SemanticRole::ListItem, &name))
        .tooltip("Open this stable object in the host");
        if matches!(scenario, Scenario::PaintAndScroll) && frame % 2 == 0 {
            item.style.visual.background = Some(Color::srgba(0.15, 0.2, 0.25, 1.0));
        }
        item = item
            .child(
                Element::image(
                    scope.element(part + 1),
                    ElementKey::new(format!("probe.image.{row}")),
                    "probe://image",
                )
                .styled(
                    Style::default()
                        .size(Dimension::Px(32.0), Dimension::Auto)
                        .aspect_ratio(1.0),
                ),
            )
            .child(
                Element::text(
                    scope.element(part + 2),
                    ElementKey::new(format!("probe.name.{row}")),
                    &name,
                )
                .styled(
                    Style::default()
                        .flex(1.0, 1.0)
                        .text(16.0, Color::rgb8(255, 255, 255)),
                ),
            )
            .child(
                Element::text(
                    scope.element(part + 3),
                    ElementKey::new(format!("probe.code.{row}")),
                    "Hg 0123",
                )
                .styled(
                    Style::default()
                        .font_role(FontRole::Mono)
                        .text(14.0, Color::rgb8(255, 255, 255)),
                ),
            );
        root.children.push(item);
    }
    root
}

fn viewport(scenario: Scenario, frame: usize) -> Viewport {
    Viewport::new(
        if matches!(scenario, Scenario::TextAndResize) && frame % 2 == 0 {
            1000.0
        } else {
            1280.0
        },
        800.0,
        1.0,
    )
}

fn run(
    scope: ElementScope,
    popup: ElementScope,
    context: &mut TextInputContext,
    projection: Projection,
    scenario: Scenario,
) -> Result<[f64; 6], Box<dyn std::error::Error>> {
    let mut retained = RetainedLayoutEngine::default();
    let mut tooltip = TooltipController::new(Duration::from_millis(500));
    let mut semantics = AccessibilityTree::default();
    let mut oracle_semantics = AccessibilityTree::default();
    let mut reference_semantics = AccessibilityTree::default();
    let mut total = [Duration::ZERO; 6];
    let mut counts = None;
    for frame in 0..WARMUP + SAMPLES {
        let start = Instant::now();
        let mut root = tree(scope, scenario, frame);
        let built = start.elapsed();
        let start = Instant::now();
        let route =
            ActionRoute::<u8>::build(&root, []).map_err(|error| format!("route: {error:?}"))?;
        let routed = start.elapsed();
        let bounds = viewport(scenario, frame);
        let start = Instant::now();
        let layout = match projection {
            Projection::Stateless => {
                LayoutEngine.compute_with_text_context(&root, bounds, context)?
            }
            Projection::Retained => retained.compute_with_text_context(&root, bounds, context)?,
            Projection::Tooltip => tooltip.layout(
                &mut root,
                TooltipFrame {
                    scope: popup,
                    viewport: bounds,
                    route: &route,
                    focus: None,
                    focus_root: None,
                    cursor: Point::new(-1.0, -1.0),
                    active: true,
                    theme: WidgetTheme::default(),
                    now: Instant::now(),
                },
                context,
            )?,
        };
        let arranged = start.elapsed();
        let start = Instant::now();
        let paint = PaintList::build(&root, &layout);
        let painted = start.elapsed();
        let start = Instant::now();
        let hits = HitTestTree::build(&root, &layout);
        black_box(hits.hit(Point::new(100.0, 60.0)));
        let hit = start.elapsed();
        let start = Instant::now();
        black_box(semantics.build(&root, &layout, None, "CPU stage probe"));
        let semantic = start.elapsed();
        // Check every transition outside all six timing intervals. The two
        // oracle semantic trees have identical histories, independently of the
        // measured tree; keep its construction/drop interval unchanged.
        let reference = LayoutEngine.compute_with_text_context(&root, bounds, context)?;
        if reference.nodes() != layout.nodes() || PaintList::build(&root, &reference) != paint {
            return Err(format!(
                "{scenario:?}/{projection:?} frame {frame}: geometry or paint mismatch"
            )
            .into());
        }
        if HitTestTree::build(&root, &reference) != hits {
            return Err(
                format!("{scenario:?}/{projection:?} frame {frame}: hit index mismatch").into(),
            );
        }
        let mut expected = reference_semantics.build(&root, &reference, None, "CPU stage probe");
        let actual = oracle_semantics.build(&root, &layout, None, "CPU stage probe");
        // Independent semantic trees intentionally own different platform IDs.
        // Align only those IDs through public ElementId identity; retain the
        // full node properties, child order, focus and TreeUpdate comparison.
        let node_ids = expected
            .nodes
            .iter()
            .map(|(id, _)| {
                if id.0 == 0 {
                    return Ok((*id, *id)); // The synthetic Window has no Element.
                }
                let element = reference_semantics
                    .element_for_node(*id)
                    .ok_or_else(|| format!("reference semantic node {id:?} has no element"))?;
                let target = oracle_semantics.node_for_element(element).ok_or_else(|| {
                    format!("semantic oracle is missing stable element {element:?}")
                })?;
                Ok((*id, target))
            })
            .collect::<Result<HashMap<_, _>, String>>()?;
        for (id, node) in &mut expected.nodes {
            *id = *node_ids.get(id).ok_or("semantic node mapping missing")?;
            let children = node
                .children()
                .iter()
                .map(|child| {
                    node_ids
                        .get(child)
                        .copied()
                        .ok_or("semantic child mapping missing")
                })
                .collect::<Result<Vec<_>, _>>()?;
            node.set_children(children);
        }
        expected.focus = *node_ids
            .get(&expected.focus)
            .ok_or("semantic focus mapping missing")?;
        if expected != actual {
            return Err(format!(
                "{scenario:?}/{projection:?} frame {frame}: semantic update mismatch"
            )
            .into());
        }
        counts = Some((layout.nodes().len(), paint.commands().len()));
        black_box((&root, &route, &layout, &paint, &hits));
        if frame >= WARMUP {
            for (sum, elapsed) in total
                .iter_mut()
                .zip([built, routed, arranged, painted, hit, semantic])
            {
                *sum += elapsed;
            }
        }
    }
    println!(
        "{scenario:?}/{projection:?}: nodes/commands={counts:?}; samples={SAMPLES}; full geometry/paint/hit/semantics oracle frames={}",
        WARMUP + SAMPLES,
    );
    Ok(total.map(|elapsed| elapsed.as_secs_f64() * 1000.0 / SAMPLES as f64))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("provide a host font path")?;
    let mut context = TextInputContext::default();
    context.register_font(Arc::new(std::fs::read(path)?));
    let scope = ElementIdAllocator
        .allocate_scope()
        .map_err(|error| format!("content scope: {error:?}"))?;
    let popup = ElementIdAllocator
        .allocate_scope()
        .map_err(|error| format!("popup scope: {error:?}"))?;
    let mut totals = [[0.0; 6]; 9];
    println!(
        "debug assertions={}; CPU ms: build, route, layout, paint, hit, semantics; no OS/GPU/IME",
        cfg!(debug_assertions)
    );
    for block in 0..BLOCKS {
        for (case, scenario) in [
            Scenario::Steady,
            Scenario::PaintAndScroll,
            Scenario::TextAndResize,
        ]
        .into_iter()
        .enumerate()
        {
            let order = if block % 2 == 0 {
                [
                    Projection::Stateless,
                    Projection::Retained,
                    Projection::Tooltip,
                ]
            } else {
                [
                    Projection::Tooltip,
                    Projection::Retained,
                    Projection::Stateless,
                ]
            };
            for projection in order {
                let measured = run(scope, popup, &mut context, projection, scenario)?;
                println!("block={block}: {measured:.6?}");
                let column = match projection {
                    Projection::Stateless => 0,
                    Projection::Retained => 1,
                    Projection::Tooltip => 2,
                };
                for (sum, value) in totals[case * 3 + column].iter_mut().zip(measured) {
                    *sum += value / BLOCKS as f64;
                }
            }
        }
    }
    for (case, stages) in totals.into_iter().enumerate() {
        println!("aggregate case={case}: {stages:.6?}");
    }
    let mut empty = RetainedLayoutEngine::default();
    let start = Instant::now();
    for _ in 0..100_000 {
        empty.clear();
        black_box(&empty);
    }
    println!(
        "empty retained clear mean ns: {:.1}",
        start.elapsed().as_nanos() as f64 / 100_000.0
    );
    Ok(())
}
