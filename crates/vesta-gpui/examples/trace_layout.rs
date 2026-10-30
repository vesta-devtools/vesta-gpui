//! Real headless layout/text/paint work with synthetic width steps, not native latency evidence.
use std::{error::Error, fs::OpenOptions, io::Write, path::PathBuf};
use vesta_gpui::{
    Dimension, Element, ElementId, ElementKey, HitTestTree, LayoutEngine, PaintList, Style,
    TextEditKind, TextInputContext, TextInputState, Viewport, text_input_wrap_width,
    trace::{self, TraceCapture, TraceConfig},
};

fn main() -> Result<(), Box<dyn Error>> {
    let output = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Usage: trace_layout <new-output.json|html>")?,
    );
    let mut context = TextInputContext::default();
    let mut input = TextInputState::new_multiline(14.0, "sans-serif", 300.0);
    input
        .insert(
            &mut context,
            "A local tracing example. Document text is not a trace payload.",
            TextEditKind::Typing,
        )
        .map_err(|error| format!("Example text preparation failed: {error:?}"))?;
    let capture = TraceCapture::start(TraceConfig::default())?;
    for width in [300.0, 240.0, 180.0, 240.0, 300.0] {
        for _ in 0..2 {
            let _frame = trace::frame("Headless.Main");
            trace::counter("example.host", "synthetic.width", f64::from(width));
            let root = {
                let _build = trace::span("example.host", "build.tree");
                Element::container(ElementId::new(1), ElementKey::new("example.root"))
                    .styled(Style::default().size(Dimension::Px(width), Dimension::Px(100.0)))
            };
            let layout = LayoutEngine.compute_with_text_context(
                &root,
                Viewport::new(width, 100.0, 1.0),
                &mut context,
            )?;
            let bounds = layout.node(root.id).ok_or("missing root layout")?.rect;
            if input.set_wrap_width(&mut context, text_input_wrap_width(bounds)) {
                trace::instant("example.host", "frame.skipped.wrap_changed");
                continue;
            }
            let _paint = PaintList::build(&root, &layout);
            let _hit = HitTestTree::build(&root, &layout);
            trace::instant("example.host", "headless.frame.ready");
        }
    }
    let snapshot = capture.stop();
    let content = if output
        .extension()
        .is_some_and(|extension| extension == "html")
    {
        snapshot.to_html_report()?
    } else {
        snapshot.to_chrome_json()?
    };
    // This headless command owns its explicit destination, after capture stopped.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(content.as_bytes())?;
    println!(
        "Headless trace: {} frames, {} events, {} dropped; no native/GPU timing",
        snapshot.frames().len(),
        snapshot.stats().events,
        snapshot.stats().dropped_events
    );
    Ok(())
}
