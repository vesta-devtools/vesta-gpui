//! Calibration admission and the shared execution/context distinction.
use crate::{
    import,
    query::{self, protocol::*},
    session::*,
};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;
fn fixture() -> Value {
    let base = 9_007_199_254_741_999u64;
    let sample = json!({"device_ticks":base,"host_ticks":2_000_000_000_000_050u64,"host_before_ticks":2_000_000_000_000_000u64,"host_after_ticks":2_000_000_000_000_100u64,"host_frequency_hz":10000000,"cpu_before_ns":40000,"cpu_after_ns":50200,"max_deviation_ns":100});
    let event = |id, cal, error, outcome| json!({"generation":1,"submission":id,"window":1,"thread":1,"frame_id":1,"recorded_cpu_ns":20000,"outcome":outcome,"calibration":cal,"calibration_error":error});
    let ticks = json!({"state":"sample","raw_ticks":[base-5000,base-4000],"elapsed_ns":1000,"completion_upper_bound_ns":100000});
    json!({"gpui_trace":{"schema_version":2,"clock":"monotonic_elapsed_wall_time","thread_ids":"capture_local","gpu_execution_timing":false,"detail":"stages","events":1,"dropped_events":0,"unfinished_spans":0,"invalid_nesting":0,"unframed_events":0,"expired":false},
        "traceEvents":[{"pid":17,"tid":1,"ts":0,"dur":100,"ph":"X","name":"Frame","cat":"frames","args":{"event_id":1,"parent_id":null,"frame_id":1,"frame":true,"unfinished":false}}],
        "gpui_capture":{"schema_version":2,"process_id":17,"elapsed_ns":100000,"dropped_records":0,"windows":[1],"clocks":[{"generation":1,"device_name":"fixture","vendor_id":1,"device_id":1,"driver_version":1,"queue_family":0,"valid_bits":64,"period_ns":1.0,"unavailable":null,"calibration_support":"vulkan_khr_qpc"}],
            "submissions":[event(1,sample,Value::Null,ticks.clone()),event(2,Value::Null,Value::Null,ticks.clone()),event(3,Value::Null,json!("query_failed"),ticks),event(4,Value::Null,Value::Null,json!({"state":"pending_completion"}))]}})
}
fn load(v: &Value) -> Session {
    import::parse(v.to_string().as_bytes(), &AtomicBool::new(false)).expect("clock fixture")
}
fn ask(s: &Session, mut v: Value) -> Value {
    v["schema_version"] = json!(1);
    v["request_id"] = json!("gpu-clock");
    let r: Request = serde_json::from_value(v).expect("typed request");
    serde_json::to_value(query::execute(s, &r, &AtomicBool::new(false))).expect("response")
}
#[test]
fn execution_estimates_preserve_high_ticks_unknowns_and_context_semantics() {
    let s = load(&fixture());
    let r = ask(&s, json!({"query":{"op":"gpu_execution"}}));
    let d = &r["result"]["data"];
    assert_eq!(r["analysis_version"], 7);
    assert_eq!(d["matched_estimates"], 1);
    assert_eq!(d["unplaced_records"], 3);
    let a = &d["items"][0];
    assert_eq!(a["execution"]["estimated_start_ns"], 40100);
    assert_eq!(a["execution"]["estimated_end_ns"], 41100);
    assert_eq!(a["execution"]["drift_bound_known"], false);
    assert_eq!(a["calibration"]["device_ticks"], "9007199254741999");
    assert_eq!(r["quality"]["gpu"]["execution_estimates"], 1);
    assert_eq!(r["quality"]["gpu"]["unmapped_samples"], 2);
    let range = json!({"range_ns":[40000,42000]});
    let e = ask(
        &s,
        json!({"query":{"op":"gpu_execution"},"selection":range}),
    );
    assert_eq!(e["result"]["data"]["matched_estimates"], 1);
    let context = ask(
        &s,
        json!({"query":{"op":"gpu_submissions"},"selection":range}),
    );
    assert_eq!(
        context["result"]["data"]["page"]["total"], 0,
        "CPU recording context is not execution"
    );
    let other = ask(
        &s,
        json!({"query":{"op":"gpu_execution"},"selection":{"range_ns":[60000,70000]}}),
    );
    assert_eq!(other["result"]["data"]["matched_estimates"], 0);
    assert_eq!(other["result"]["data"]["unplaced_records"], 3);
    let bins = s.gpu_tracks[0].buckets(
        TimeRange {
            start: 40000,
            end: 42000,
        },
        1,
    );
    assert_eq!(bins.len(), 1);
    assert_eq!(bins[0].record, 0);
    assert_eq!(bins[0].count, 1);
    let filtered = crate::gpu::plot::filtered(&s, "missing", &AtomicBool::new(false))
        .expect("filtered GPU tracks");
    assert!(filtered.is_empty());
}
#[test]
fn malformed_calibration_schema_bounds_support_and_states_are_rejected() {
    for (pointer, value) in [
        ("/gpui_capture/schema_version", json!(1)),
        (
            "/gpui_capture/clocks/0/calibration_support",
            json!("not_recorded"),
        ),
        (
            "/gpui_capture/submissions/0/calibration/host_frequency_hz",
            json!(0),
        ),
        (
            "/gpui_capture/submissions/0/calibration/cpu_after_ns",
            json!(100001),
        ),
        (
            "/gpui_capture/submissions/0/calibration/max_deviation_ns",
            json!(0),
        ),
        (
            "/gpui_capture/submissions/0/calibration_error",
            json!("query_failed"),
        ),
    ] {
        let mut f = fixture();
        *f.pointer_mut(pointer).expect("fixture field") = value;
        assert!(
            import::parse(f.to_string().as_bytes(), &AtomicBool::new(false)).is_err(),
            "{pointer}"
        );
    }
    let s = load(&fixture());
    let r = ask(
        &s,
        json!({"query":{"op":"gpu_execution"},"budget":{"max_work":1}}),
    );
    assert_eq!(r["result"]["status"], "error");
    let r = ask(
        &s,
        json!({"query":{"op":"gpu_execution"},"page":{"limit":1}}),
    );
    let cursor = &r["result"]["data"]["page"]["next_cursor"];
    let next = ask(
        &s,
        json!({"query":{"op":"gpu_execution"},"page":{"limit":1,"cursor":cursor}}),
    );
    assert_eq!(next["result"]["data"]["items"][0]["key"]["submission"], 2);
    let stale = ask(
        &s,
        json!({"query":{"op":"gpu_submissions"},"page":{"limit":1,"cursor":cursor}}),
    );
    assert_eq!(stale["result"]["code"], "trace_cursor_stale");
}
