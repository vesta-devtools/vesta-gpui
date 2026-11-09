//! Independent lifetime, coverage, identity and query boundary contracts.
use crate::{
    import,
    query::{self, protocol::*},
    session::*,
};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

fn fixture() -> Value {
    let operations = [
        (
            10,
            1,
            Some(1),
            json!({"op":"alloc","address":"0x100","size":100}),
        ),
        (
            20,
            1,
            Some(1),
            json!({"op":"alloc","address":"0x200","size":200}),
        ),
        (
            30,
            1,
            Some(1),
            json!({"op":"realloc","old_address":"0x100","old_size":100,"address":"0x100","size":150}),
        ),
        (
            40,
            2,
            None,
            json!({"op":"free","address":"0x200","size":200}),
        ),
        (
            45,
            1,
            Some(1),
            json!({"op":"realloc_failed","address":"0x100","old_size":150,"requested_size":900}),
        ),
        (
            50,
            1,
            Some(1),
            json!({"op":"alloc","address":"0x200","size":40}),
        ),
        (
            60,
            2,
            None,
            json!({"op":"free","address":"0x900","size":90}),
        ),
        (
            65,
            2,
            Some(2),
            json!({"op":"realloc","old_address":"0xa00","old_size":60,"address":"0xb00","size":70}),
        ),
        (
            70,
            2,
            Some(2),
            json!({"op":"alloc","address":"0x300","size":10}),
        ),
        (
            80,
            2,
            None,
            json!({"op":"free","address":"0x300","size":10}),
        ),
        (
            90,
            2,
            None,
            json!({"op":"free","address":"0x200","size":40}),
        ),
    ];
    let threads = json!([{"thread":1,"name":"Main"},{"thread":2,"name":"Worker"}]);
    json!({"gpui_trace":{"schema_version":2,"clock":"monotonic_elapsed_wall_time","thread_ids":"capture_local","gpu_execution_timing":false,"detail":"stages","events":1,"dropped_events":0,"unfinished_spans":0,"invalid_nesting":0,"unframed_events":0,"expired":false},
        "traceEvents":[{"pid":17,"tid":1,"ts":0,"dur":0.1,"ph":"X","cat":"frames","name":"Frame","args":{"event_id":1,"parent_id":null,"frame_id":1,"frame":true,"unfinished":false}}],
        "gpui_threads":{"schema_version":1,"process_id":17,"threads":threads},
        "gpui_memory":{"schema_version":1,"process_id":17,"elapsed_ns":100,"max_events":100,"provider":"rust_system_global_allocator","baseline_known":false,"event_limit_reached":false,"hook_errors":0,"stack_frames":4,"symbols_resolved":false,"threads":threads,
            "stacks":[{"id":1,"potentially_truncated":false,"frames":[{"address":"0xabc","function":"main_allocate","file":"host.rs","line":12}]},{"id":2,"potentially_truncated":true,"frames":[{"address":"0xdef","function":null,"file":null,"line":null}]}],
            "events":operations.into_iter().enumerate().map(|(i,(time,thread,stack,operation))|json!({"id":i+1,"time_ns":time,"thread":thread,"stack":stack,"operation":operation})).collect::<Vec<_>>()}})
}
fn load(v: &Value) -> Session {
    import::parse(v.to_string().as_bytes(), &AtomicBool::new(false)).expect("fixture")
}
fn ask(s: &Session, mut v: Value) -> Value {
    v["schema_version"] = json!(1);
    v["request_id"] = json!("heap-contract");
    let r: Request = serde_json::from_value(v).expect("admitted memory fixture");
    serde_json::to_value(query::execute(s, &r, &AtomicBool::new(false)))
        .expect("admitted memory fixture")
}
#[test]
fn memory_reuse_realloc_failure_and_cross_thread_free_preserve_lifetimes() {
    let s = load(&fixture());
    let m = s.memory.as_ref().expect("admitted memory fixture");
    assert_eq!(m.allocations.len(), 6);
    assert_eq!(m.peak_bytes, 350);
    assert_eq!(m.peak_count, 4);
    assert_eq!(m.observed_allocated_bytes, 570);
    assert_eq!(m.points.last().expect("admitted memory fixture").bytes, 220);
    assert_eq!(m.realloc_failures, 1);
    assert_eq!(m.unknown_frees, 1);
    assert_eq!(m.unknown_reallocations, 1);
    let r = ask(
        &s,
        json!({"query":{"op":"memory_allocation","key":{"process":17,"allocation":3}}}),
    );
    let a = &r["result"]["data"]["item"];
    assert_eq!(a["address"], "0x100");
    assert_eq!(a["size_bytes"], 150);
    assert_eq!(a["reallocated_from"]["allocation"], 1);
    assert_eq!(a["freed_ns"], Value::Null);
    let r = ask(
        &s,
        json!({"query":{"op":"memory_allocation","key":{"process":17,"allocation":2}}}),
    );
    assert_eq!(r["result"]["data"]["item"]["free_lane"]["thread"], 2);
    let r = ask(&s, json!({"query":{"op":"lanes"}}));
    assert_eq!(r["result"]["data"]["items"][0]["name"], "Main");
    let r = ask(&s, json!({"query":{"op":"memory_summary"}}));
    assert_eq!(r["analysis_version"], ANALYSIS_VERSION);
    let c = &r["quality"]["memory"];
    assert_eq!(c["baseline_known"], false);
    assert_eq!(c["process_wide"], false);
    assert_eq!(c["resolved_frames"], 1);
    assert_eq!(c["unresolved_frames"], 1);
}
#[test]
fn memory_comparison_cohorts_have_exact_queryable_evidence_and_tie_semantics() {
    let s = load(&fixture());
    let r = ask(
        &s,
        json!({"query":{"op":"memory_compare","start_ns":25,"end_ns":85}}),
    );
    let c = &r["result"]["data"]["comparison"];
    for (field, count, bytes) in [
        ("observed_live_at_start", 2, 300),
        ("observed_live_at_end", 3, 260),
        ("new_survivors", 3, 260),
        ("released", 2, 300),
        ("common", 0, 0),
        ("transient", 1, 10),
    ] {
        assert_eq!(c[field]["allocations"], count, "{field}");
        assert_eq!(c[field]["bytes"], bytes, "{field}");
        let state = match field {
            "observed_live_at_start" | "observed_live_at_end" => continue,
            _ => field,
        };
        let q = ask(
            &s,
            json!({"query":{"op":"memory_allocations","lifetime":{"state":state,"start_ns":25,"end_ns":85}}}),
        );
        assert_eq!(q["result"]["data"]["page"]["total"], count);
    }
    assert_eq!(c["observed_change_bytes"], "-40");
    let r = ask(
        &s,
        json!({"query":{"op":"memory_allocations","lifetime":{"state":"live_at","time_ns":40}}}),
    );
    let rows = r["result"]["data"]["items"]
        .as_array()
        .expect("admitted memory fixture");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["key"]["allocation"], 3);
    let mut f = fixture();
    f["gpui_memory"]["events"][9]["time_ns"] = json!(70);
    let s = load(&f);
    let r = ask(
        &s,
        json!({"query":{"op":"memory_allocations","lifetime":{"state":"live_at","time_ns":70}}}),
    );
    assert!(
        !r["result"]["data"]["items"]
            .as_array()
            .expect("admitted memory fixture")
            .iter()
            .any(|a| a["key"]["allocation"] == 9)
    );
}
#[test]
fn memory_groups_paging_stacks_filters_and_series_are_bounded_and_source_bound() {
    let s = load(&fixture());
    let r = ask(
        &s,
        json!({"query":{"op":"memory_stack_groups"},"page":{"limit":1}}),
    );
    assert_eq!(r["result"]["data"]["items"][0]["allocated_bytes"], 490);
    let cursor = r["result"]["data"]["page"]["next_cursor"].clone();
    let r = ask(
        &s,
        json!({"query":{"op":"memory_stack_groups"},"page":{"limit":1,"cursor":cursor}}),
    );
    assert_eq!(r["result"]["data"]["items"][0]["allocated_bytes"], 80);
    let r = ask(
        &s,
        json!({"query":{"op":"memory_allocations"},"page":{"limit":1,"cursor":cursor}}),
    );
    assert_eq!(r["result"]["code"], "trace_cursor_stale");
    let r = ask(
        &s,
        json!({"query":{"op":"memory_stack","key":{"process":17,"stack":1}}}),
    );
    assert_eq!(
        r["result"]["data"]["item"]["frames"][0]["function"],
        "main_allocate"
    );
    let r = ask(
        &s,
        json!({"query":{"op":"memory_allocations"},"selection":{"text":"MAIN_ALLOCATE"}}),
    );
    assert_eq!(r["result"]["data"]["page"]["total"], 4);
    let r = ask(
        &s,
        json!({"query":{"op":"memory_allocations"},"selection":{"lane":{"process":17,"thread":2}}}),
    );
    assert_eq!(r["result"]["data"]["page"]["total"], 2);
    let r = ask(&s, json!({"query":{"op":"memory_series","columns":1}}));
    let d = &r["result"]["data"];
    assert_eq!(d["observations"], 11);
    let points = d["items"].as_array().expect("admitted memory fixture");
    for (operation, bytes, count) in [(1, 100, 1), (3, 350, 2), (9, 270, 4), (11, 220, 2)] {
        assert!(points.iter().any(|p| p["operation"] == operation
            && p["observed_live_bytes"] == bytes
            && p["observed_live_count"] == count));
    }
    assert_eq!(
        d["omitted_observations"]
            .as_u64()
            .expect("admitted memory fixture") as usize,
        11 - points.len()
    );
}
#[test]
fn memory_unknown_absent_partial_and_query_budgets_never_fabricate_coverage() {
    let mut f = fixture();
    f["gpui_memory"]["event_limit_reached"] = json!(true);
    let s = load(&f);
    for q in [
        json!({"op":"memory_allocations","lifetime":{"state":"live_at","time_ns":101}}),
        json!({"op":"memory_compare","start_ns":10,"end_ns":101}),
        json!({"op":"memory_compare","start_ns":50,"end_ns":50}),
    ] {
        let r = ask(&s, json!({"query":q}));
        assert_eq!(r["result"]["status"], "error");
    }
    let r = ask(
        &s,
        json!({"query":{"op":"memory_allocations"},"budget":{"max_work":1}}),
    );
    assert_eq!(r["result"]["status"], "error");
    let r = ask(&s, json!({"query":{"op":"memory_series","columns":257}}));
    assert_eq!(r["result"]["status"], "error");
    let r = ask(
        &s,
        json!({"query":{"op":"memory_series"},"selection":{"text":"main"}}),
    );
    assert_eq!(r["result"]["code"], "trace_selection_unsupported");
    f.as_object_mut()
        .expect("admitted memory fixture")
        .remove("gpui_memory");
    let s = load(&f);
    let r = ask(&s, json!({"query":{"op":"memory_summary"}}));
    assert_eq!(r["result"]["data"]["coverage"], Value::Null);
    let r = ask(&s, json!({"query":{"op":"memory_allocations"}}));
    assert_eq!(r["result"]["code"], "trace_memory_not_attached");
}
#[test]
fn memory_malformed_operations_and_metadata_are_rejected() {
    for (pointer, value) in [
        ("/gpui_memory/events/0/id", json!(2)),
        ("/gpui_memory/events/1/time_ns", json!(9)),
        ("/gpui_memory/events/0/thread", json!(999)),
        ("/gpui_memory/events/0/stack", json!(999)),
        ("/gpui_memory/events/1/operation/address", json!("0x100")),
        ("/gpui_memory/events/1/operation/address", json!("0x110")),
        (
            "/gpui_memory/events/6/operation/address",
            json!("0xfffffffffffffff0"),
        ),
        ("/gpui_memory/events/0/operation/address", json!("0x+1")),
        ("/gpui_memory/events/0/operation/size", json!(0)),
        ("/gpui_memory/events/3/operation/size", json!(199)),
        ("/gpui_memory/events/4/operation/old_size", json!(151)),
        ("/gpui_memory/elapsed_ns", json!(89)),
        ("/gpui_memory/baseline_known", json!(true)),
        ("/gpui_memory/provider", json!("rss")),
        ("/gpui_memory/threads/0/name", json!("Different")),
        ("/gpui_memory/stacks/0/frames/0/address", json!("0x0")),
        ("/gpui_memory/symbols_resolved", json!(true)),
        ("/gpui_threads/process_id", json!(18)),
    ] {
        let mut f = fixture();
        *f.pointer_mut(pointer).expect("admitted memory fixture") = value;
        assert!(
            import::parse(f.to_string().as_bytes(), &AtomicBool::new(false)).is_err(),
            "{pointer}"
        );
    }
    let mut f = fixture();
    f["gpui_trace"]["events"] = json!(0);
    f["traceEvents"] = json!([]);
    f.as_object_mut()
        .expect("admitted memory fixture")
        .remove("gpui_memory");
    assert!(
        import::parse(f.to_string().as_bytes(), &AtomicBool::new(false)).is_ok(),
        "empty CPU capture may still have named thread metadata"
    );
}
