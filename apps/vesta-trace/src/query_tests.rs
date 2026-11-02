//! Protect the external Agent evidence, paging and bounded-transport contracts.
use crate::{
    analysis, import,
    query::{self, protocol::*, transport},
    tests::fixture,
};
use serde_json::{Value, json};
use std::{io::Cursor as Input, sync::atomic::AtomicBool};

fn session() -> crate::session::Session {
    import::parse(fixture().to_string().as_bytes(), &AtomicBool::new(false)).expect("fixture")
}
fn ask(session: &crate::session::Session, query: Value, selection: Value, page: Value) -> Value {
    let bytes=json!({"schema_version":1,"request_id":"test","query":query,"selection":selection,"page":page}).to_string();
    serde_json::from_slice(&transport::process(
        session,
        bytes.as_bytes(),
        &AtomicBool::new(false),
    ))
    .expect("wire JSON")
}
fn code(response: &Value) -> &str {
    response["result"]["code"]
        .as_str()
        .expect("structured error")
}

#[test]
fn machine_pages_are_complete_deterministic_and_bound_to_source_and_query() {
    let session = session();
    let mut page = json!({"limit":2});
    let mut keys = Vec::new();
    let mut first_cursor = None;
    loop {
        let response = ask(&session, json!({"op":"events"}), json!({}), page.clone());
        assert_eq!(response["result"]["status"], "ok");
        assert_eq!(response["source"]["sha256"], session.source.sha256);
        let data = &response["result"]["data"];
        assert_eq!(data["page"]["total"], 4);
        keys.extend(
            data["items"]
                .as_array()
                .expect("items")
                .iter()
                .map(|e| e["key"]["event"].as_u64().expect("key")),
        );
        let next = &data["page"]["next_cursor"];
        if next.is_null() {
            break;
        }
        first_cursor.get_or_insert(next.clone());
        page["cursor"] = next.clone();
    }
    assert_eq!(
        keys,
        vec![1, 2, 4, 3],
        "time order retains stable identities rather than array positions"
    );
    let cursor = first_cursor.expect("second page");
    let stale = ask(
        &session,
        json!({"op":"events"}),
        json!({"text":"scope2"}),
        json!({"limit":2,"cursor":cursor}),
    );
    assert_eq!(code(&stale), "trace_cursor_stale");
    let stale = ask(
        &session,
        json!({"op":"events"}),
        json!({}),
        json!({"limit":3,"cursor":cursor}),
    );
    assert_eq!(code(&stale), "trace_cursor_stale");
    let mut other = fixture();
    other["traceEvents"][0]["name"] = json!("other input");
    let other =
        import::parse(other.to_string().as_bytes(), &AtomicBool::new(false)).expect("other source");
    assert_eq!(
        code(&ask(
            &other,
            json!({"op":"events"}),
            json!({}),
            json!({"limit":2,"cursor":cursor})
        )),
        "trace_cursor_stale"
    );
}

#[test]
fn agent_drilldown_reuses_clipped_analysis_and_original_parent_frame_evidence() {
    let session = session();
    let selection = json!({"range_ns":[25000,60000],"text":"scope1"});
    let response = ask(&session, json!({"op":"hotspots"}), selection, json!({}));
    let timer = &response["result"]["data"]["items"][0];
    assert_eq!(timer["inclusive_ns"], 35000);
    assert_eq!(timer["self_ns"], 0);
    let expected = analysis::analyze(
        &session,
        crate::session::TimeRange::new(25000, 60000).expect("range"),
        "scope1",
        &AtomicBool::new(false),
    )
    .expect("native analysis");
    assert_eq!(timer["inclusive_ns"], expected.timers[0].inclusive);
    let response = ask(
        &session,
        json!({"op":"events","timer":{"category":"fixture","name":"scope2"}}),
        json!({}),
        json!({}),
    );
    let event = &response["result"]["data"]["items"][0];
    assert_eq!(event["key"]["event"], 2);
    assert_eq!(event["parent"]["event"], 1);
    assert_eq!(event["frame"]["event"], 1);
    let response = ask(
        &session,
        json!({"op":"event","key":event["key"]}),
        json!({}),
        json!({}),
    );
    assert_eq!(response["result"]["data"]["self_ns"], 30000);
    let response = ask(
        &session,
        json!({"op":"relations","timer":{"category":"fixture","name":"scope2"},"direction":"callers"}),
        json!({}),
        json!({}),
    );
    assert_eq!(
        response["result"]["data"]["items"][0]["inclusive_ns"],
        40000
    );
    let response = ask(
        &session,
        json!({"op":"tree","root":{"process":17,"event":1}}),
        json!({}),
        json!({}),
    );
    assert_eq!(response["result"]["data"]["page"]["total"], 4);
    let response = ask(
        &session,
        json!({"op":"frames"}),
        json!({"range_ns":[25000,60000]}),
        json!({}),
    );
    assert_eq!(
        response["result"]["data"]["items"][0]["duration_ns"],
        100000
    );
    assert_eq!(response["result"]["data"]["items"][0]["overlap_ns"], 35000);
    assert_eq!(response["result"]["data"]["statistics"]["p95_ns"], 100000);
    assert_eq!(
        response["quality"]["hierarchy_reliable"], false,
        "forged clean nesting header remains visible to the Agent"
    );
    assert_eq!(response["quality"]["full_function_coverage"], false);
    assert_eq!(response["quality"]["gpu_execution_timing"], false);
    let html = format!(
        "<script id=\"gpui-data\" type=\"application/json\">{}</script>",
        fixture()
    );
    let html = import::parse(html.as_bytes(), &AtomicBool::new(false)).expect("HTML data");
    assert_ne!(
        html.source.sha256, session.source.sha256,
        "hash identifies original bytes including wrapper, not normalized data"
    );
    assert_eq!(html.events[1].key, session.events[1].key);
    let absent = ask(
        &session,
        json!({"op":"frames"}),
        json!({"text":"not-recorded"}),
        json!({}),
    );
    assert_eq!(absent["result"]["data"]["statistics"]["count"], 0);
    assert!(
        absent["result"]["data"]["statistics"]["p50_ns"].is_null(),
        "missing samples cannot masquerade as measured zero"
    );
    let mut zero = fixture();
    zero["traceEvents"]
        .as_array_mut()
        .expect("events")
        .truncate(1);
    zero["traceEvents"][0]["dur"] = json!(0);
    zero["gpui_trace"]["events"] = json!(1);
    let zero = import::parse(zero.to_string().as_bytes(), &AtomicBool::new(false))
        .expect("recorded zero-duration frame");
    let measured = ask(&zero, json!({"op":"frames"}), json!({}), json!({}));
    assert_eq!(measured["result"]["data"]["statistics"]["count"], 1);
    assert_eq!(measured["result"]["data"]["statistics"]["p50_ns"], 0);
}

#[test]
fn wire_failures_never_publish_partial_totals_and_jsonl_recovers_after_bad_requests() {
    let session = session();
    let base = json!({"schema_version":1,"request_id":"bounded","query":{"op":"hotspots"}});
    let mut limited = base.clone();
    limited["budget"] = json!({"max_work":1});
    let response: Value = serde_json::from_slice(&transport::process(
        &session,
        limited.to_string().as_bytes(),
        &AtomicBool::new(false),
    ))
    .expect("error JSON");
    assert_eq!(code(&response), "trace_work_budget");
    assert!(response["result"].get("data").is_none());
    let request: Request = serde_json::from_value(base.clone()).expect("request");
    let response = serde_json::to_value(query::execute(&session, &request, &AtomicBool::new(true)))
        .expect("response");
    assert_eq!(code(&response), "trace_cancelled");
    let mut invalid = base.clone();
    invalid["schema_version"] = json!(2);
    let mut unknown = base.clone();
    unknown["query"]["path"] = json!("must not open another file");
    let lines = format!("{}\n{}\n{}", invalid, unknown, base);
    let mut output = Vec::new();
    transport::serve(
        &session,
        &mut Input::new(lines),
        &mut output,
        &AtomicBool::new(false),
    )
    .expect("stdio");
    let responses = String::from_utf8(output)
        .expect("UTF-8")
        .lines()
        .map(|s| serde_json::from_str::<Value>(s).expect("one JSON per line"))
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 3);
    assert_eq!(code(&responses[0]), "trace_query_schema");
    assert_eq!(code(&responses[1]), "trace_request_json");
    assert_eq!(responses[1]["request_id"], "bounded");
    assert_eq!(responses[2]["result"]["status"], "ok");
    let response = ask(
        &session,
        json!({"op":"events"}),
        json!({"range_ns":[0,100001]}),
        json!({}),
    );
    assert_eq!(code(&response), "trace_query_range");
    let oversized = [vec![b' '; MAX_REQUEST_BYTES + 1], b"\n{}\n".to_vec()].concat();
    let mut output = Vec::new();
    transport::serve(
        &session,
        &mut Input::new(oversized),
        &mut output,
        &AtomicBool::new(false),
    )
    .expect("bounded transport");
    assert_eq!(
        output.iter().filter(|b| **b == b'\n').count(),
        1,
        "oversized tail cannot become another request"
    );
    let response: Value = serde_json::from_slice(&output).expect("budget JSON");
    assert_eq!(code(&response), "trace_request_budget");
}

#[test]
fn output_budget_discards_oversized_success_without_mutating_captured_facts() {
    let mut input = fixture();
    input["traceEvents"][0]["name"] = json!("x".repeat(MAX_RESPONSE_BYTES));
    let session = import::parse(input.to_string().as_bytes(), &AtomicBool::new(false))
        .expect("large but admitted source");
    let response = ask(
        &session,
        json!({"op":"event","key":{"process":17,"event":1}}),
        json!({}),
        json!({}),
    );
    assert_eq!(code(&response), "trace_response_budget");
    assert!(response["result"].get("data").is_none());
    assert_eq!(session.events[0].name.len(), MAX_RESPONSE_BYTES);
    assert_eq!(response["source"]["sha256"], session.source.sha256);
}

#[test]
fn agent_counter_envelopes_retain_spike_ids_counts_and_exact_frame_filtering() {
    let mut input = fixture();
    for (offset, value) in [1.0, 99.0, -3.0, 7.0].into_iter().enumerate() {
        input["traceEvents"].as_array_mut().expect("events").push(json!({"pid":17,"tid":1,"ts":20+offset*10,"ph":"C","cat":"metrics","name":"queue","args":{"event_id":5+offset,"parent_id":null,"frame_id":if offset%2==0{Some(1)}else{None},"frame":false,"value":value}}));
    }
    input["gpui_trace"]["events"] = json!(8);
    input["gpui_trace"]["unframed_events"] = json!(2);
    let session = import::parse(input.to_string().as_bytes(), &AtomicBool::new(false))
        .expect("counter source");
    let query =
        json!({"op":"counter_series","columns":1,"timer":{"category":"metrics","name":"queue"}});
    let all = ask(&session, query.clone(), json!({}), json!({}));
    let track = &all["result"]["data"]["items"][0];
    assert_eq!(track["count"], 4);
    assert_eq!(track["minimum"]["event"]["event"], 7);
    assert_eq!(track["maximum"]["event"]["event"], 6);
    assert_eq!(track["minimum"]["value"], -3.0);
    assert_eq!(track["maximum"]["value"], 99.0);
    let selected = ask(
        &session,
        query,
        json!({"frame":{"process":17,"event":1}}),
        json!({}),
    );
    let track = &selected["result"]["data"]["items"][0];
    assert_eq!(track["count"], 2);
    assert_eq!(track["maximum"]["value"], 1.0);
    assert_eq!(track["minimum"]["value"], -3.0);
    assert_eq!(
        code(&ask(
            &session,
            json!({"op":"counter_series","columns":0}),
            json!({}),
            json!({})
        )),
        "trace_counter_columns"
    );
    let bounded = json!({"schema_version":1,"request_id":"counter-budget","query":{"op":"counter_series"},"budget":{"max_work":1}});
    let response: Value = serde_json::from_slice(&transport::process(
        &session,
        bounded.to_string().as_bytes(),
        &AtomicBool::new(false),
    ))
    .expect("budget response");
    assert_eq!(code(&response), "trace_work_budget");
    assert!(response["result"].get("data").is_none());
}

#[test]
fn overflowing_elapsed_totals_are_errors_instead_of_plausible_saturated_hotspots() {
    let mut input = fixture();
    let events=(1..=2400).map(|id|json!({"pid":17,"tid":1,"ts":0.0,"dur":8_000_000_000_000.0,"ph":"X","cat":"synthetic","name":"same","args":{"event_id":id,"parent_id":null,"frame_id":null,"frame":false}})).collect::<Vec<_>>();
    input["traceEvents"] = json!(events);
    input["gpui_trace"]["events"] = json!(2400);
    input["gpui_trace"]["unframed_events"] = json!(2400);
    let session = import::parse(input.to_string().as_bytes(), &AtomicBool::new(false))
        .expect("numeric-valid long capture");
    assert_eq!(
        code(&ask(
            &session,
            json!({"op":"hotspots"}),
            json!({}),
            json!({})
        )),
        "trace_total_overflow"
    );
}
