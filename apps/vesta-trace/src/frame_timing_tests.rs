//! Independent frame/GPU join, missing-data, paging and native projection contracts.
use crate::{
    import, inspection,
    query::{self, protocol::*},
    session::*,
};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

fn fixture() -> Value {
    let frame = |id, start, duration, name| {
        json!({"pid":17,"tid":1,"ts":start,"dur":duration,"ph":"X","cat":"frames","name":name,
        "args":{"event_id":id,"parent_id":null,"frame_id":id,"frame":true,"unfinished":false}})
    };
    let sample = |ns: u64| json!({"state":"sample","raw_ticks":[9007199254741992u64,9007199254741992u64+ns],"elapsed_ns":ns,"completion_upper_bound_ns":100000});
    let gpu = |id, frame, time, thread, window, outcome| json!({"generation":7,"submission":id,"window":window,"thread":thread,"frame_id":frame,"recorded_cpu_ns":time,"outcome":outcome});
    json!({"gpui_trace":{"schema_version":2,"clock":"monotonic_elapsed_wall_time","thread_ids":"capture_local","gpu_execution_timing":false,
        "detail":"stages","events":4,"dropped_events":0,"unfinished_spans":0,"invalid_nesting":0,"unframed_events":0,"expired":false},
        "traceEvents":[frame(1,0,100,"CPU slow"),frame(2,100,60,"GPU heavy"),frame(3,200,40,"Zero"),frame(4,300,50,"No GPU")],
        "gpui_capture":{"schema_version":1,"process_id":17,"elapsed_ns":350000,"dropped_records":0,"windows":[1,2],
            "clocks":[{"generation":7,"device_name":"synthetic GPU","vendor_id":1,"device_id":2,"driver_version":3,"queue_family":0,"valid_bits":64,"period_ns":1.0,"unavailable":null}],
            "submissions":[gpu(1,Some(1),10000,1,1,sample(40000)),gpu(2,Some(1),80000,1,2,sample(30000)),
                gpu(3,Some(1),90000,1,1,json!({"state":"pending_completion"})),gpu(4,Some(2),110000,1,1,sample(80000)),
                gpu(5,Some(3),210000,1,1,sample(0)),gpu(6,None::<u32>,250000,2,2,json!({"state":"pending_completion"}))]}})
}
fn load(value: &Value) -> Session {
    import::parse(value.to_string().as_bytes(), &AtomicBool::new(false)).expect("admitted source")
}
fn ask(session: &Session, mut value: Value) -> Value {
    value["schema_version"] = json!(1);
    value["request_id"] = json!("frame-timing-contract");
    let request: Request = serde_json::from_value(value).expect("typed request");
    serde_json::to_value(query::execute(session, &request, &AtomicBool::new(false)))
        .expect("response")
}

#[test]
fn frame_timings_join_whole_gpu_work_preserve_zero_missing_and_range_semantics() {
    let session = load(&fixture());
    let result = ask(
        &session,
        json!({"query":{"op":"frame_timings","sort":"gpu_work_desc"}}),
    );
    assert_eq!(result["result"]["status"], "ok");
    assert_eq!(result["quality"]["gpu"]["cpu_clock_mapping"], false);
    let data = &result["result"]["data"];
    let rows = &data["items"];
    assert_eq!(data["statistics"]["cpu"]["count"], 4);
    assert_eq!(data["statistics"]["cpu"]["p50_ns"], 50000);
    assert_eq!(data["statistics"]["cpu"]["p95_ns"], 100000);
    assert_eq!(data["statistics"]["frames_with_gpu_records"], 3);
    assert_eq!(
        data["statistics"]["frames_with_all_recorded_submissions_sampled"],
        2
    );
    assert_eq!(data["source_unframed_gpu_records"], 1);
    assert_eq!(
        data["statistics"]["gpu_pending"], 1,
        "source-unframed records are separate"
    );
    assert_eq!(rows[0]["frame"]["key"]["event"], 2);
    assert_eq!(rows[0]["gpu"]["sampled_work_ns"], 80000);
    assert_eq!(
        rows[0]["frame"]["duration_ns"], 60000,
        "GPU work may exceed CPU elapsed"
    );
    assert_eq!(rows[1]["gpu"]["sampled_work_ns"], 70000);
    assert_eq!(rows[1]["gpu"]["all_recorded_submissions_sampled"], false);
    assert_eq!(rows[1]["gpu"]["longest_submission"]["submission"], 1);
    assert_eq!(rows[2]["gpu"]["sampled_work_ns"], 0, "zero is measured");
    assert_eq!(
        rows[3]["gpu"]["sampled_work_ns"],
        Value::Null,
        "missing is unknown"
    );
    let clipped = ask(
        &session,
        json!({"query":{"op":"frame_timings"},"selection":{"range_ns":[50000,60000]}}),
    );
    let row = &clipped["result"]["data"]["items"][0];
    assert_eq!(row["frame"]["overlap_ns"], 10000);
    assert_eq!(row["frame"]["duration_ns"], 100000);
    assert_eq!(
        row["gpu"]["sampled_work_ns"], 70000,
        "whole associated submissions are not clipped by CPU range"
    );
    let contexts = ask(
        &session,
        json!({"query":{"op":"gpu_submissions"},"selection":{"range_ns":[50000,60000]}}),
    );
    assert_eq!(
        contexts["result"]["data"]["page"]["total"], 0,
        "CPU-context queries keep their original meaning"
    );
}

#[test]
fn frame_timings_page_filter_loss_and_native_analysis_share_original_evidence() {
    let mut value = fixture();
    value["gpui_capture"]["dropped_records"] = json!(2);
    let session = load(&value);
    let first = ask(
        &session,
        json!({"query":{"op":"frame_timings","sort":"gpu_work_desc"},"page":{"limit":2}}),
    );
    let data = &first["result"]["data"];
    assert_eq!(
        data["statistics"]["frames_with_all_recorded_submissions_sampled"], 0,
        "producer loss has no frame attribution"
    );
    let cursor = data["page"]["next_cursor"].clone();
    let second = ask(
        &session,
        json!({"query":{"op":"frame_timings","sort":"gpu_work_desc"},"page":{"limit":2,"cursor":cursor}}),
    );
    assert_eq!(
        second["result"]["data"]["items"][0]["frame"]["key"]["event"],
        3
    );
    let stale = ask(
        &session,
        json!({"query":{"op":"frame_timings","sort":"start_asc"},"page":{"limit":2,"cursor":cursor}}),
    );
    assert_eq!(stale["result"]["code"], "trace_cursor_stale");
    for selection in [
        json!({"frame":{"process":17,"event":2}}),
        json!({"text":"GPU heavy"}),
    ] {
        let result = ask(
            &session,
            json!({"query":{"op":"frame_timings"},"selection":selection}),
        );
        assert_eq!(result["result"]["data"]["page"]["total"], 1);
        assert_eq!(
            result["result"]["data"]["items"][0]["frame"]["key"]["event"],
            2
        );
    }
    let report = inspection::analyze(
        &session,
        session.bounds,
        "",
        &inspection::Selection {
            panel: inspection::Panel::Frames,
            frame_sort: FrameTimingSort::GpuWorkDesc,
            ..Default::default()
        },
        &AtomicBool::new(false),
        None,
    )
    .expect("native report");
    let native = serde_json::to_value(report.inspection.table).expect("native table");
    let machine = ask(
        &session,
        json!({"query":{"op":"frame_timings","sort":"gpu_work_desc"}}),
    );
    assert_eq!(native, machine["result"]["data"]);
    let mut cpu_only = fixture();
    cpu_only
        .as_object_mut()
        .expect("object")
        .remove("gpui_capture");
    let result = ask(&load(&cpu_only), json!({"query":{"op":"frame_timings"}}));
    assert_eq!(result["result"]["data"]["gpu_attached"], false);
    assert_eq!(
        result["result"]["data"]["items"][0]["gpu"]["sampled_work_ns"],
        Value::Null
    );
    let lane = ask(
        &session,
        json!({"query":{"op":"frame_timings"},"selection":{"lane":{"process":17,"thread":2}}}),
    );
    assert_eq!(lane["result"]["data"]["page"]["total"], 0);
    assert_eq!(lane["result"]["data"]["source_unframed_gpu_records"], 1);
    let mut unavailable = fixture();
    unavailable["gpui_capture"]["submissions"][4]["outcome"] =
        json!({"state":"unavailable","reason":"results_unavailable","raw_ticks":null});
    let unknown = ask(
        &load(&unavailable),
        json!({"query":{"op":"frame_timings"},"selection":{"frame":{"process":17,"event":3}}}),
    );
    assert_eq!(
        unknown["result"]["data"]["statistics"]["gpu_unavailable"],
        1
    );
    assert_eq!(
        unknown["result"]["data"]["items"][0]["gpu"]["sampled_work_ns"],
        Value::Null
    );
}

#[test]
fn frame_timings_budget_cancel_and_overflow_never_return_partial_success() {
    let session = load(&fixture());
    let limited = ask(
        &session,
        json!({"query":{"op":"frame_timings"},"budget":{"max_work":1}}),
    );
    assert_eq!(limited["result"]["status"], "error");
    let request: Request = serde_json::from_value(
        json!({"schema_version":1,"request_id":"cancel","query":{"op":"frame_timings"}}),
    )
    .expect("request");
    assert!(matches!(
        query::execute(&session, &request, &AtomicBool::new(true)).result,
        Outcome::Error { .. }
    ));
    let raw = [0, 9_000_000_000_000_000_000u64];
    let bound = 10_000_000_000_000_000_000u64;
    let duration = vesta_gpui::trace::gpu::timestamp_elapsed(
        raw,
        64,
        1.0,
        std::time::Duration::from_nanos(bound),
    )
    .expect("admissible large device span");
    let mut value = fixture();
    for i in 0..3 {
        value["gpui_capture"]["submissions"][i]["outcome"] = json!({"state":"sample","raw_ticks":raw,"elapsed_ns":duration.as_nanos() as u64,"completion_upper_bound_ns":bound});
    }
    let overflow = ask(&load(&value), json!({"query":{"op":"frame_timings"}}));
    assert_eq!(overflow["result"]["code"], "trace_total_overflow");
    assert!(overflow["result"].get("data").is_none());
}
