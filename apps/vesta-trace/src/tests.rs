use crate::{analysis, import, session::*};
use serde_json::json;
use std::sync::atomic::AtomicBool;

pub(crate) fn fixture() -> serde_json::Value {
    let event = |id, parent, start, dur, frame| json!({"pid":17,"tid":1,"ts":start,"dur":dur,"ph":"X","cat":"fixture","name":format!("scope{id}"),"args":{"event_id":id,"parent_id":parent,"frame_id":1,"frame":frame,"unfinished":false}});
    json!({"gpui_trace":{"schema_version":2,"clock":"monotonic_elapsed_wall_time","thread_ids":"capture_local","gpu_execution_timing":false,"detail":"detailed","events":4,"dropped_events":0,"unfinished_spans":0,"invalid_nesting":0,"unframed_events":0,"expired":false},"traceEvents":[event(1,None,0.0,100.0,true),event(2,Some(1),10.0,40.0,false),event(3,Some(1),30.0,40.0,false),event(4,Some(2),20.0,10.0,false)]})
}
fn load(value: &serde_json::Value) -> Session {
    import::parse(value.to_string().as_bytes(), &AtomicBool::new(false)).expect("fixture imported")
}

#[test]
fn clipped_self_time_uses_child_union_and_filter_does_not_change_ownership() {
    let session = load(&fixture());
    assert!(
        !session.hierarchy_reliable,
        "overlapping siblings must be disclosed even when the header declares zero errors"
    );
    let range = TimeRange::new(25_000, 60_000).expect("range");
    let result =
        analysis::analyze(&session, range, "scope1", &AtomicBool::new(false)).expect("analyzed");
    assert_eq!(result.timers.len(), 1);
    assert_eq!(result.timers[0].inclusive, 35_000);
    assert_eq!(result.timers[0].self_time, 0);
    let whole =
        analysis::analyze(&session, session.bounds, "", &AtomicBool::new(false)).expect("analyzed");
    assert_eq!(
        whole
            .timers
            .iter()
            .find(|t| t.name == "scope1")
            .expect("root")
            .self_time,
        40_000
    );
    let (callers, callees) = analysis::relations(
        &session,
        session.bounds,
        "fixture",
        "scope2",
        &analysis::Selection::default(),
        &mut crate::control::Control::new(&AtomicBool::new(false), u64::MAX, 10_000),
    )
    .expect("relations");
    assert_eq!(callers[0].name, "scope1");
    assert_eq!(
        callers[0].inclusive, 40_000,
        "caller edges attribute selected work, not the whole parent for each occurrence"
    );
    assert_eq!(callees[0].name, "scope4");
}

#[test]
fn json_and_html_preserve_ids_while_corrupt_identity_clock_and_cycles_fail() {
    let value = fixture();
    let mut unsupported = value.clone();
    unsupported["gpui_trace"]["schema_version"] = json!(3);
    assert_eq!(
        import::parse(unsupported.to_string().as_bytes(), &AtomicBool::new(false))
            .expect_err("schema")
            .code,
        "trace_schema_unsupported"
    );
    let mut negative = value.clone();
    negative["traceEvents"][1]["dur"] = json!(-1.0);
    assert_eq!(
        import::parse(negative.to_string().as_bytes(), &AtomicBool::new(false))
            .expect_err("negative time")
            .code,
        "trace_numeric_range"
    );
    let html = format!(
        "<script>throw Error('must never run')</script><script id=\"gpui-data\" type=\"application/json\">{value}</script>"
    );
    let session = import::parse(html.as_bytes(), &AtomicBool::new(false)).expect("data only");
    assert_eq!(
        session.events[3].key,
        EventKey {
            process: 17,
            event: 4
        }
    );
    assert_eq!(session.events[3].depth, 2);
    let mut legacy = value.clone();
    legacy["gpui_trace"]
        .as_object_mut()
        .expect("header")
        .remove("detail");
    let legacy = load(&legacy);
    assert_eq!(legacy.detail, "unspecified (legacy)");
    assert!(
        legacy
            .diagnostics
            .iter()
            .any(|d| d.code == "trace_detail_unspecified")
    );
    let mut bad = value.clone();
    bad["traceEvents"][1]["args"]["event_id"] = json!(1);
    assert_eq!(
        import::parse(bad.to_string().as_bytes(), &AtomicBool::new(false))
            .expect_err("duplicate")
            .code,
        "trace_event_identity"
    );
    let mut bad = value.clone();
    bad["gpui_trace"]["clock"] = json!("gpu_ticks");
    assert_eq!(
        import::parse(bad.to_string().as_bytes(), &AtomicBool::new(false))
            .expect_err("clock")
            .code,
        "trace_clock_unsupported"
    );
    let mut bad = value.clone();
    bad["traceEvents"][1]["args"]["parent_id"] = json!(3);
    bad["traceEvents"][2]["args"]["parent_id"] = json!(2);
    assert_eq!(
        import::parse(bad.to_string().as_bytes(), &AtomicBool::new(false))
            .expect_err("cycle")
            .code,
        "trace_parent_cycle"
    );
}

#[test]
fn lane_queries_keep_zero_duration_boundaries_and_unframed_loss_visible() {
    let mut value = fixture();
    value["traceEvents"][3] = json!({"pid":17,"tid":2,"ts":20.0,"ph":"C","cat":"fixture","name":"queue.depth","args":{"event_id":4,"parent_id":null,"frame_id":null,"frame":false,"value":3.0}});
    value["gpui_trace"]["unframed_events"] = json!(1);
    value["gpui_trace"]["dropped_events"] = json!(8);
    let session = load(&value);
    assert_eq!(session.lanes.len(), 2);
    let lane = session
        .lanes
        .iter()
        .find(|l| l.key.thread == 2)
        .expect("worker");
    assert_eq!(
        lane.visible(&session, TimeRange::new(20_000, 20_001).expect("range"))
            .count(),
        1
    );
    assert_eq!(
        lane.visible(&session, TimeRange::new(19_999, 20_000).expect("range"))
            .count(),
        0
    );
    assert!(
        session
            .diagnostics
            .iter()
            .any(|d| d.code == "trace_events_dropped")
    );
    assert!(
        session
            .diagnostics
            .iter()
            .any(|d| d.code == "trace_unframed_events")
    );
    assert!(analysis::analyze(&session, session.bounds, "", &AtomicBool::new(true)).is_err());
}

#[test]
fn native_inspection_keeps_range_work_separate_from_whole_event_and_parent_evidence() {
    use crate::inspection::{self, Panel, Selection};
    use crate::query::protocol::{Data, TimerKey};
    let session = load(&fixture());
    let cancel = AtomicBool::new(false);
    let range = TimeRange::new(25000, 60000).expect("selection");
    let mut selection = Selection {
        memory: Default::default(),
        frame_sort: Default::default(),
        gpu: None,
        panel: Panel::Callers,
        timer: Some(TimerKey {
            category: "fixture".into(),
            name: "scope2".into(),
        }),
        event: Some(EventKey {
            process: 17,
            event: 2,
        }),
        cursor: None,
    };
    let callers = inspection::analyze(&session, range, "", &selection, &cancel, None)
        .expect("native callers");
    let Some(Data::Relations { items, .. }) = &callers.inspection.table else {
        panic!("caller table");
    };
    assert_eq!(
        items[0].inclusive_ns, 25000,
        "edge work is clipped to the native selection"
    );
    let detail = callers.inspection.detail.expect("selected evidence");
    assert_eq!(
        detail.item.duration_ns, 40000,
        "whole-event evidence is not truncated by the selection"
    );
    assert_eq!(detail.self_ns, Some(30000));
    assert_eq!(
        detail.item.parent,
        Some(EventKey {
            process: 17,
            event: 1
        })
    );
    selection.panel = Panel::Callees;
    let callees = inspection::analyze(&session, range, "", &selection, &cancel, None)
        .expect("native callees");
    let Some(Data::Relations { items, .. }) = &callees.inspection.table else {
        panic!("callee table");
    };
    assert_eq!(items[0].timer.name, "scope4");
    assert_eq!(items[0].inclusive_ns, 5000);
    selection.panel = Panel::Tree;
    let tree =
        inspection::analyze(&session, range, "", &selection, &cancel, None).expect("native tree");
    let Some(Data::Tree { items, page, .. }) = &tree.inspection.table else {
        panic!("tree table");
    };
    assert_eq!(page.total, 2);
    assert_eq!(
        items[0].parent,
        Some(EventKey {
            process: 17,
            event: 1
        }),
        "missing parent row cannot rewrite the original key"
    );
    assert!(
        inspection::analyze(
            &session,
            range,
            "",
            &selection,
            &AtomicBool::new(true),
            None
        )
        .is_err()
    );
}

#[test]
fn filtered_native_rows_keep_focus_bound_to_the_same_recorded_scope_group() {
    use crate::{
        inspection::Panel,
        view::{self, Command},
    };
    use vesta_gpui::{
        ActionRoute, ElementId, FocusScope, InputOwnership, InputTarget, TextInputSnapshot,
        UiWindowId,
    };
    let session = load(&fixture());
    let cancel = AtomicBool::new(false);
    let full = analysis::analyze(&session, session.bounds, "", &cancel).expect("full analysis");
    let filtered =
        analysis::analyze(&session, session.bounds, "scope2", &cancel).expect("filtered analysis");
    let timeline = crate::timeline::Timeline::new(session.bounds);
    let projection = |analysis| {
        view::build(view::View {
            session: Some(&session),
            analysis: Some(analysis),
            timeline: Some(&timeline),
            inspection: None,
            tracks: None,
            width: 1480.0,
            height: 900.0,
            status: "fixture",
            filter: "",
            search: TextInputSnapshot::default(),
            search_focused: false,
            panel: Panel::Timers,
            table_scroll: 0,
            gpu_scroll: vesta_gpui::Point::default(),
            previous_page: false,
            busy: false,
        })
    };
    let full = projection(&full);
    let filtered = projection(&filtered);
    let full = ActionRoute::build(&full.widget.element, full.widget.actions).expect("full route");
    let filtered = ActionRoute::build(&filtered.widget.element, filtered.widget.actions)
        .expect("filtered route");
    let element = ElementId::new(6_000_001);
    for route in [&full, &filtered] {
        assert!(
            matches!(route.action_at(element),Some(Command::Timer(category,name))if category=="fixture" && name=="scope2")
        );
    }
    let mut ownership = InputOwnership::default();
    ownership.set_focus(Some(InputTarget {
        window: UiWindowId(1),
        element,
    }));
    assert!(
        !FocusScope::new(UiWindowId(1), &filtered, None)
            .reconcile(&mut ownership)
            .lost_focus
    );
    assert!(filtered.focus_order().contains(&view::SEARCH));
    assert!(filtered.focus_order().contains(&view::TRACK));
}
