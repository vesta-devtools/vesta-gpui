//! Protect file-reference honesty and machine/native GPU evidence equivalence.
use crate::{import, query::transport, tests::fixture};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

fn capture() -> Value {
    let mut value = fixture();
    let clock = |generation, bits, unavailable| json!({"generation":generation,"device_name":"synthetic GPU","vendor_id":1,"device_id":2,"driver_version":3,"queue_family":0,"valid_bits":bits,"period_ns":2.0,"unavailable":unavailable});
    let record = |generation, id, time, outcome| json!({"generation":generation,"submission":id,"window":1,"thread":1,"frame_id":1,"recorded_cpu_ns":time,"outcome":outcome});
    value["gpui_capture"] = json!({"schema_version":1,"process_id":17,"elapsed_ns":100000,"dropped_records":2,"windows":[1],
    "clocks":[clock(7,Some(8),None::<String>),clock(8,None,Some("unsupported_queue".to_owned()))],
    "submissions":[
        record(7,1,20000,json!({"state":"sample","raw_ticks":[250,3],"elapsed_ns":18,"completion_upper_bound_ns":100})),
        record(7,2,30000,json!({"state":"sample","raw_ticks":[2,2],"elapsed_ns":0,"completion_upper_bound_ns":100})),
        record(7,3,40000,json!({"state":"pending_completion"})),
        record(7,4,50000,json!({"state":"unavailable","reason":"results_unavailable","raw_ticks":null})),
        record(8,1,60000,json!({"state":"unavailable","reason":"unsupported_queue","raw_ticks":null})),
    ]});
    value
}
fn load(value: &Value) -> Result<crate::session::Session, crate::session::TraceError> {
    import::parse(value.to_string().as_bytes(), &AtomicBool::new(false))
}
fn ask(session: &crate::session::Session, request: Value) -> Value {
    let mut request = request;
    request["schema_version"] = json!(1);
    request["request_id"] = json!("gpu.contract");
    serde_json::from_slice(&transport::process(
        session,
        request.to_string().as_bytes(),
        &AtomicBool::new(false),
    ))
    .expect("response")
}
#[test]
fn gpu_admission_rejects_forged_joins_capabilities_and_timestamp_math() {
    let original = capture();
    let session = load(&original).expect("GPU input");
    assert_eq!(session.gpu_counts.samples, 2);
    assert_eq!(session.gpu_counts.pending, 1);
    assert_eq!(session.gpu_counts.unavailable, 2);
    for (pointer, replacement) in [
        ("/gpui_capture/schema_version", json!(2)),
        ("/gpui_capture/process_id", json!(18)),
        ("/gpui_capture/submissions/1/submission", json!(1)),
        ("/gpui_capture/submissions/0/frame_id", json!(2)),
        ("/gpui_capture/submissions/0/thread", json!(2)),
        ("/gpui_capture/submissions/0/window", json!(2)),
        ("/gpui_capture/submissions/0/recorded_cpu_ns", json!(100002)),
        ("/gpui_capture/submissions/0/outcome/elapsed_ns", json!(19)),
        (
            "/gpui_capture/submissions/0/outcome/completion_upper_bound_ns",
            json!(1024),
        ),
        ("/gpui_capture/clocks/0/valid_bits", json!(65)),
        ("/gpui_capture/clocks/0/period_ns", json!(0)),
        ("/gpui_capture/clocks/1/unavailable", Value::Null),
        ("/gpui_capture/windows", json!([1, 1])),
    ] {
        let mut bad = original.clone();
        *bad.pointer_mut(pointer).expect("fixture pointer") = replacement;
        assert!(load(&bad).is_err(), "must reject {pointer}");
    }
    let html = format!(
        "<html><script id=\"gpui-data\" type=\"application/json\">{original}</script></html>"
    );
    let html =
        import::parse(html.as_bytes(), &AtomicBool::new(false)).expect("data-only HTML extension");
    assert_eq!(html.gpu_counts.samples, 2);
    assert_ne!(
        html.source.sha256, session.source.sha256,
        "whole input provenance"
    );
}

#[test]
fn gpu_queries_preserve_missing_zero_paging_frames_and_native_evidence() {
    let session = load(&capture()).expect("GPU input");
    let first = ask(
        &session,
        json!({"query":{"op":"gpu_submissions","sort":"duration_desc"},"page":{"limit":2}}),
    );
    assert_eq!(first["analysis_version"], 4);
    assert_eq!(first["quality"]["gpu"]["samples"], 2);
    assert_eq!(first["quality"]["gpu"]["pending"], 1);
    assert_eq!(first["quality"]["gpu"]["unavailable"], 2);
    assert_eq!(first["quality"]["gpu"]["dropped_records"], 2);
    assert_eq!(first["quality"]["gpu"]["cpu_clock_mapping"], false);
    assert_eq!(first["source"]["capture_extension_version"], 1);
    let data = &first["result"]["data"];
    assert_eq!(data["items"][0]["elapsed_ns"], 18);
    assert_eq!(data["items"][0]["raw_ticks"], json!(["250", "3"]));
    assert_eq!(data["items"][1]["elapsed_ns"], 0);
    assert_eq!(data["page"]["total"], 5);
    let cursor = data["page"]["next_cursor"].clone();
    let second = ask(
        &session,
        json!({"query":{"op":"gpu_submissions","sort":"duration_desc"},"page":{"limit":2,"cursor":cursor}}),
    );
    assert_eq!(
        second["result"]["data"]["items"][0]["status"],
        "pending_completion"
    );
    assert_eq!(
        second["result"]["data"]["items"][0]["elapsed_ns"],
        Value::Null
    );
    let stale = ask(
        &session,
        json!({"query":{"op":"gpu_submissions"},"page":{"limit":2,"cursor":cursor}}),
    );
    assert_eq!(stale["result"]["code"], "trace_cursor_stale");
    let key = data["items"][0]["key"].clone();
    let detail = ask(&session, json!({"query":{"op":"gpu_submission","key":key}}));
    assert_eq!(detail["result"]["data"]["item"], data["items"][0]);
    let filtered = ask(
        &session,
        json!({"query":{"op":"gpu_submissions"},"selection":{"range_ns":[20000,30000],"frame":{"process":17,"event":1},"text":"scope1"}}),
    );
    assert_eq!(
        filtered["result"]["data"]["page"]["total"], 1,
        "CPU anchor half-open selection"
    );
    let budget = ask(
        &session,
        json!({"query":{"op":"gpu_submissions"},"budget":{"max_work":1}}),
    );
    assert_eq!(budget["result"]["status"], "error");
    assert!(budget["result"].get("data").is_none());
    let clocks = ask(&session, json!({"query":{"op":"gpu_clocks"}}));
    assert_eq!(clocks["result"]["data"]["cpu_clock_mapping"], false);
    assert_eq!(
        clocks["result"]["data"]["items"][1]["unavailable"],
        "unsupported_queue"
    );
    let legacy = load(&fixture()).expect("legacy CPU");
    let legacy = ask(&legacy, json!({"query":{"op":"gpu_clocks"}}));
    assert_eq!(legacy["result"]["data"]["attached"], false);
    assert_eq!(legacy["quality"]["gpu"], Value::Null);
    let native = crate::inspection::analyze(
        &session,
        session.bounds,
        "",
        &crate::inspection::Selection {
            panel: crate::inspection::Panel::Gpu,
            gpu: Some(serde_json::from_value(key).expect("typed key")),
            ..Default::default()
        },
        &AtomicBool::new(false),
        None,
    )
    .expect("native shared evidence");
    assert_eq!(
        native.inspection.gpu.expect("GPU detail").elapsed_ns,
        Some(18)
    );
    let mut high = capture();
    high["gpui_capture"]["clocks"][0]["valid_bits"] = json!(64);
    high["gpui_capture"]["clocks"][0]["period_ns"] = json!(1.0);
    high["gpui_capture"]["submissions"][0]["outcome"]["raw_ticks"] = json!([u64::MAX - 2, 4]);
    high["gpui_capture"]["submissions"][0]["outcome"]["elapsed_ns"] = json!(7);
    let high = load(&high).expect("64-bit wrapped capture");
    let high = ask(
        &high,
        json!({"query":{"op":"gpu_submission","key":{"process":17,"generation":7,"submission":1}}}),
    );
    assert_eq!(
        high["result"]["data"]["item"]["raw_ticks"],
        json!([(u64::MAX - 2).to_string(), "4"]),
        "ordinary JS JSON parsing must not round device ticks"
    );
}
