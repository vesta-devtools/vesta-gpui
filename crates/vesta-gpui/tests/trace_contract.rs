//! Capture lifecycle, cross-thread ownership, and ordinary public-stage integration.
use std::{sync::Mutex, time::Duration};
use vesta_gpui::{
    Dimension, Element, ElementId, ElementKey, HitTestTree, LayoutEngine, PaintList,
    RetainedLayoutEngine, Style, TextEditKind, TextInputContext, TextInputState, Viewport,
    trace::{self, TraceCapture, TraceConfig, TraceDetail, TraceError},
};

static CAPTURE: Mutex<()> = Mutex::new(());
fn config(max_events: usize) -> TraceConfig {
    TraceConfig {
        max_events,
        max_duration: Duration::from_secs(5),
    }
}

#[test]
fn automatic_stages_and_host_scopes_export_without_document_content() {
    let _serial = CAPTURE.lock().expect("capture contract serialization");
    assert!(!trace::is_recording());
    let disabled = trace::span("host", "disabled");
    drop(disabled);
    let capture = TraceCapture::start(config(2000)).expect("explicit capture");
    assert!(matches!(
        TraceCapture::start(config(10)),
        Err(TraceError::AlreadyRecording)
    ));
    {
        let _frame_entry = trace::frame("Contract.Main");
        let _frame = trace::span("host", "frame\"quoted");
        trace::instant("host", "</script><script>must_stay_data</script>");
        let mut context = TextInputContext::default();
        let root = Element::container(ElementId::new(1), ElementKey::new("trace.fixture"))
            .styled(Style::default().size(Dimension::Px(300.0), Dimension::Px(80.0)));
        let viewport = Viewport::new(300.0, 80.0, 1.0);
        let layout = LayoutEngine
            .compute_with_text_context(&root, viewport, &mut context)
            .expect("ordinary layout");
        let _paint = PaintList::build(&root, &layout);
        let _hit = HitTestTree::build(&root, &layout);
        RetainedLayoutEngine::default()
            .compute_with_text_context(&root, viewport, &mut context)
            .expect("retained layout");
        let mut input = TextInputState::new_multiline(14.0, "sans-serif", 200.0);
        input
            .insert(
                &mut context,
                "PRIVATE_DOCUMENT_CONTENT_012345",
                TextEditKind::Typing,
            )
            .expect("real text input");
        input.set_wrap_width(&mut context, 100.0);
        trace::instant("host", "frame.skipped.wrap_changed");
        std::thread::spawn(|| {
            let _work = trace::span("host", "worker");
            trace::counter("host", "items", 12.0);
        })
        .join()
        .expect("worker joined");
        let _ = std::panic::catch_unwind(|| {
            let _scope = trace::span("host", "unwind");
            panic!("disposable unwind contract");
        });
    }
    let snapshot = capture.stop();
    assert!(!trace::is_recording());
    assert_eq!(snapshot.stats().dropped_events, 0);
    assert_eq!(snapshot.stats().unfinished_spans, 0);
    assert_eq!(snapshot.stats().invalid_nesting, 0);
    let frames = snapshot.frames();
    assert_eq!(frames.len(), 1);
    let frame = &frames[0];
    assert_eq!(frame.name, "Contract.Main");
    let host = frame
        .calls
        .iter()
        .find(|c| c.name == "frame\"quoted")
        .expect("host scope");
    let layout = frame
        .calls
        .iter()
        .find(|c| c.name == "layout.compute")
        .expect("automatic layout");
    assert_eq!(host.parent_id, Some(frame.frame_id));
    assert_eq!(layout.parent_id, Some(host.event_id));
    assert!(frame.calls.iter().all(|c| c.self_time <= c.inclusive));
    assert!(
        !frame.calls.iter().any(|c| c.name == "worker"),
        "worker without a frame is not guessed into the main frame"
    );
    let json = snapshot.to_chrome_json().expect("trace serialization");
    assert!(!json.contains("PRIVATE_DOCUMENT_CONTENT"));
    assert!(!json.contains("trace.fixture"));
    let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    let events = value["traceEvents"].as_array().expect("event list");
    for name in [
        "frame\"quoted",
        "layout.compute",
        "layout.retained",
        "paint.build",
        "hit_test.build",
        "text.input.refresh",
        "text.wrap_width.changed",
        "frame.skipped.wrap_changed",
        "worker",
        "unwind",
    ] {
        assert!(
            events.iter().any(|e| e["name"] == name),
            "missing automatic/host stage {name}"
        );
    }
    let main = events
        .iter()
        .find(|e| e["name"] == "frame\"quoted")
        .expect("main frame");
    let worker = events
        .iter()
        .find(|e| e["name"] == "worker")
        .expect("worker");
    assert_ne!(main["tid"], worker["tid"]);
    assert!(
        events
            .iter()
            .filter(|e| e["ph"] == "X")
            .all(|e| e["dur"].as_f64().is_some_and(|d| d >= 0.0))
    );
    assert_eq!(value["gpui_trace"]["gpu_execution_timing"], false);
    assert_eq!(value["gpui_trace"]["unframed_events"], 2);
    assert_eq!(value["gpui_frames"][0]["frame_id"], frame.frame_id);
    assert_eq!(main["args"]["frame_id"], frame.frame_id);
    assert_eq!(worker["args"]["frame_id"], serde_json::Value::Null);
    let html = snapshot.to_html_report().expect("offline report");
    assert!(!html.contains("<script>must_stay_data"));
    assert!(!html.contains("PRIVATE_DOCUMENT_CONTENT"));
    let embedded = html
        .split("<script id=\"gpui-data\" type=\"application/json\">")
        .nth(1)
        .expect("embedded data")
        .split("</script>")
        .next()
        .expect("data close");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(embedded).expect("safe embedded JSON"),
        value
    );
    for detail in [TraceDetail::Stages, TraceDetail::Detailed] {
        let capture =
            TraceCapture::start_with_detail(config(2000), detail).expect("selected detail");
        {
            let _frame = trace::frame("Detail.Contract");
            let mut context = TextInputContext::default();
            let root = Element::text(
                ElementId::new(5),
                ElementKey::new("private.detail.label"),
                "PRIVATE_DETAIL_TEXT",
            );
            LayoutEngine
                .compute_with_text_context(&root, Viewport::new(300.0, 80.0, 1.0), &mut context)
                .expect("text layout");
        }
        let json = capture.stop().to_chrome_json().expect("selected export");
        let value: serde_json::Value = serde_json::from_str(&json).expect("selected JSON");
        assert_eq!(
            value["gpui_trace"]["detail"],
            if detail == TraceDetail::Stages {
                "stages"
            } else {
                "detailed"
            }
        );
        let calls = value["gpui_frames"][0]["calls"]
            .as_array()
            .expect("frame calls");
        assert!(calls.iter().any(|call| call["name"] == "layout.compute"));
        assert_eq!(
            calls.iter().any(|call| call["name"] == "text.shape_static"),
            detail == TraceDetail::Detailed
        );
        assert!(!json.contains("PRIVATE_DETAIL_TEXT"));
        assert_eq!(value["gpui_trace"]["invalid_nesting"], 0);
    }
}

#[test]
fn capacity_stop_and_late_guards_preserve_capture_ownership() {
    let _serial = CAPTURE.lock().expect("capture contract serialization");
    let capture = TraceCapture::start(config(2)).expect("first capture");
    let late = trace::frame("old.open");
    trace::counter("host", "finite", 1.0);
    trace::counter("host", "overflow", 2.0);
    trace::counter("host", "invalid", f64::NAN);
    let first = capture.stop();
    assert_eq!(first.stats().events, 2);
    assert_eq!(first.stats().dropped_events, 2);
    assert_eq!(first.stats().unfinished_spans, 1);
    assert!(first.frames()[0].unfinished);
    let frozen = first.to_chrome_json().expect("first export");
    let successor = TraceCapture::start(config(10)).expect("successor capture");
    late.finish_with_name("late.must_not_enter_successor");
    trace::instant("host", "successor.only");
    let second = successor.stop();
    assert_eq!(second.stats().events, 1);
    assert_eq!(first.to_chrome_json().expect("frozen export"), frozen);
    assert!(
        !second
            .to_chrome_json()
            .expect("second export")
            .contains("late.must_not")
    );
    let dropped = TraceCapture::start(config(10)).expect("drop-only capture");
    drop(dropped);
    assert!(!trace::is_recording());
    let malformed = TraceCapture::start(config(10)).expect("nesting diagnostic capture");
    let parent = trace::frame("invalid.frame");
    let child = trace::span("host", "still.open");
    drop(parent);
    drop(child);
    assert_eq!(malformed.stop().stats().invalid_nesting, 1);
}

#[test]
fn deadline_budget_and_invalid_counters_are_explicit() {
    let _serial = CAPTURE.lock().expect("capture contract serialization");
    for settings in [
        config(0),
        config(200_001),
        TraceConfig {
            max_events: 1,
            max_duration: Duration::ZERO,
        },
        TraceConfig {
            max_events: 1,
            max_duration: Duration::from_secs(301),
        },
    ] {
        assert!(matches!(
            TraceCapture::start(settings),
            Err(TraceError::InvalidBudget)
        ));
    }
    let capture = TraceCapture::start(TraceConfig {
        max_events: 10,
        max_duration: Duration::from_millis(10),
    })
    .expect("bounded capture");
    let scope = trace::frame("clipped");
    trace::counter("host", "nan", f64::NAN);
    std::thread::sleep(Duration::from_millis(15));
    scope.finish_with_name("late.completed");
    assert!(!capture.is_recording());
    let successor = TraceCapture::start(config(10)).expect("expired capture releases recorder");
    let snapshot = capture.stop();
    assert!(
        successor.is_recording(),
        "old owner must not disable its successor"
    );
    assert!(snapshot.stats().expired);
    assert_eq!(snapshot.stats().elapsed, Duration::from_millis(10));
    assert_eq!(snapshot.stats().unfinished_spans, 1);
    assert_eq!(snapshot.stats().dropped_events, 1);
    assert_eq!(snapshot.stats().events, 1);
    assert!(snapshot.frames()[0].unfinished);
    assert_eq!(
        snapshot.frames()[0].name,
        "clipped",
        "late completion must not relabel an expired stage"
    );
    assert!(snapshot.frames()[0].duration <= Duration::from_millis(10));
    drop(successor);
}
