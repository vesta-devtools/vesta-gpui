//! One read-only evidence engine for machine clients and native analysis views.
mod counters;
mod gpu;
pub mod protocol;
pub mod transport;

use crate::{analysis, control::Control, session::*};
use protocol::*;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::atomic::AtomicBool;

pub fn quality(session: &Session) -> Quality {
    let gpu = gpu::coverage(session);
    Quality {
        gpu_execution_timing: gpu.as_ref().is_some_and(|g| g.samples != 0),
        gpu,
        detail: session.detail.clone(),
        dropped_events: session.dropped_events,
        unfinished_spans: session.unfinished_spans,
        unframed_events: session.unframed_events,
        hierarchy_reliable: session.hierarchy_reliable,
        capture_expired: session.capture_expired,
        diagnostics: session.diagnostics.clone(),
        full_function_coverage: false,
        symbol_resolution: false,
        relationships: if session.gpu.is_some() {
            "recorded_same_thread_parent_frame_and_gpu_submission_context_keys"
        } else {
            "recorded_same_thread_parent_and_frame_keys_only"
        }
        .into(),
        self_time_semantics:
            "clipped_elapsed_wall_minus_direct_child_union_includes_waits_and_uninstrumented_work"
                .into(),
    }
}
pub fn failure(
    session: Option<&Session>,
    request_id: Option<String>,
    error: TraceError,
) -> Response {
    Response {
        schema_version: PROTOCOL_VERSION,
        analysis_version: ANALYSIS_VERSION,
        request_id,
        source: session.map(|s| s.source.clone()),
        quality: session.map(quality),
        cost: Cost::default(),
        result: Outcome::Error {
            code: error.code.into(),
            message: error.message,
        },
    }
}

pub fn execute(session: &Session, request: &Request, cancel: &AtomicBool) -> Response {
    let mut response = failure(
        Some(session),
        Some(request.request_id.clone()),
        TraceError::new("trace_internal", "query not executed"),
    );
    // Validate before constructing a deadline from untrusted limits.
    if let Err(error) = validate(request) {
        response.result = Outcome::Error {
            code: error.code.into(),
            message: error.message,
        };
        return response;
    }
    let mut control = Control::new(cancel, request.budget.max_work, request.budget.deadline_ms);
    response.result = match run(session, request, &mut control) {
        Ok((range, hash, data)) => Outcome::Ok {
            range_ns: [range.start, range.end],
            query_sha256: hash,
            data: Box::new(data),
        },
        Err(error) => Outcome::Error {
            code: error.code.into(),
            message: error.message,
        },
    };
    response.cost = Cost {
        work: control.work,
        elapsed_us: control.elapsed_us(),
    };
    response
}

fn validate(request: &Request) -> Result<(), TraceError> {
    if request.schema_version != PROTOCOL_VERSION {
        return Err(TraceError::new(
            "trace_query_schema",
            "unsupported query schema; use --schema",
        ));
    }
    if request.request_id.is_empty() || request.request_id.len() > 128 {
        return Err(TraceError::new(
            "trace_request_id",
            "request_id must contain 1..128 UTF-8 bytes",
        ));
    }
    if !(1..=1000).contains(&request.page.limit) {
        return Err(TraceError::new(
            "trace_page_limit",
            "page limit must be 1..1000",
        ));
    }
    if !(1..=10_000_000).contains(&request.budget.max_work)
        || !(1..=10_000).contains(&request.budget.deadline_ms)
    {
        return Err(TraceError::new(
            "trace_query_budget",
            "max_work must be 1..10000000 and deadline_ms 1..10000",
        ));
    }
    if request.selection.text.len() > 256 {
        return Err(TraceError::new(
            "trace_filter_budget",
            "selection text exceeds 256 UTF-8 bytes",
        ));
    }
    let labels = match &request.query {
        Query::Relations { timer, .. }
        | Query::CounterSeries {
            timer: Some(timer), ..
        }
        | Query::Events {
            timer: Some(timer), ..
        } => Some((&timer.category, &timer.name)),
        Query::Counters {
            category: Some(category),
            name: Some(name),
        } => Some((category, name)),
        _ => None,
    };
    if labels.is_some_and(|(category, name)| category.len() > 4096 || name.len() > 4096) {
        return Err(TraceError::new(
            "trace_filter_budget",
            "exact timer labels exceed 4096 UTF-8 bytes",
        ));
    }
    if matches!(
        request.query,
        Query::Session | Query::Lanes | Query::GpuClocks
    ) && (request.selection.range_ns.is_some()
        || request.selection.lane.is_some()
        || request.selection.frame.is_some()
        || !request.selection.text.is_empty())
    {
        return Err(TraceError::new(
            "trace_selection_unsupported",
            "session/lanes describe the whole source; query events/frames/hotspots to filter",
        ));
    }
    if matches!(
        request.query,
        Query::Session | Query::Event { .. } | Query::GpuSubmission { .. }
    ) && request.page.cursor.is_some()
    {
        return Err(TraceError::new(
            "trace_cursor_unsupported",
            "this operation is not paginated",
        ));
    }
    Ok(())
}

fn event_index(session: &Session, key: EventKey) -> Result<usize, TraceError> {
    session.indices.get(&key).copied().ok_or_else(|| {
        TraceError::new(
            "trace_event_not_found",
            format!(
                "no recorded event {}:{} in this source",
                key.process, key.event
            ),
        )
    })
}
fn fingerprint(request: &Request) -> Result<String, TraceError> {
    #[derive(Serialize)]
    struct Identity<'a> {
        analysis_version: u32,
        schema_version: u32,
        query: &'a Query,
        selection: &'a protocol::Selection,
        limit: usize,
    }
    let bytes = serde_json::to_vec(&Identity {
        analysis_version: ANALYSIS_VERSION,
        schema_version: PROTOCOL_VERSION,
        query: &request.query,
        selection: &request.selection,
        limit: request.page.limit,
    })
    .map_err(|e| TraceError::caused("trace_query_identity", e))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn page<T>(
    items: Vec<T>,
    session: &Session,
    request: &Request,
    hash: &str,
) -> Result<(Vec<T>, PageInfo), TraceError> {
    let total = items.len();
    let start = request.page.cursor.as_ref().map_or(0, |c| c.offset);
    if start > total {
        return Err(TraceError::new(
            "trace_cursor_offset",
            "cursor lies beyond this result",
        ));
    }
    let end = start.saturating_add(request.page.limit).min(total);
    let has_more = end < total;
    let items = items.into_iter().skip(start).take(end - start).collect();
    Ok((
        items,
        PageInfo {
            total,
            returned: end - start,
            has_more,
            next_cursor: has_more.then(|| Cursor {
                source_sha256: session.source.sha256.clone(),
                query_sha256: hash.into(),
                offset: end,
            }),
        },
    ))
}
fn kind(event: &Event) -> Kind {
    match event.kind {
        EventKind::Span { .. } => Kind::Span,
        EventKind::Marker => Kind::Marker,
        EventKind::Counter(_) => Kind::Counter,
    }
}
fn record(event: &Event, range: TimeRange) -> EventRecord {
    EventRecord {
        key: event.key,
        lane: event.lane,
        parent: event.parent,
        frame: event.frame,
        is_frame: event.is_frame,
        kind: kind(event),
        category: event.category.clone(),
        name: event.name.clone(),
        start_ns: event.start,
        end_ns: event.end,
        duration_ns: event.end - event.start,
        overlap_ns: range.overlap(event.start, event.end),
        depth: event.depth,
        unfinished: matches!(event.kind, EventKind::Span { unfinished: true }),
        counter_value: if let EventKind::Counter(value) = event.kind {
            Some(value)
        } else {
            None
        },
    }
}
fn timer_record(timer: analysis::Timer) -> TimerRecord {
    TimerRecord {
        timer: TimerKey {
            category: timer.category,
            name: timer.name,
        },
        calls: timer.calls,
        inclusive_ns: timer.inclusive,
        self_ns: timer.self_time,
    }
}
fn sort_timers(items: &mut [analysis::Timer], sort: TimerSort) {
    let value = |t: &analysis::Timer| match sort {
        TimerSort::Inclusive => t.inclusive,
        TimerSort::SelfTime => t.self_time,
        TimerSort::Calls => t.calls as u64,
    };
    items.sort_by(|a, b| {
        value(b)
            .cmp(&value(a))
            .then_with(|| a.category.cmp(&b.category))
            .then_with(|| a.name.cmp(&b.name))
    });
}
fn event_page(
    session: &Session,
    mut indices: Vec<usize>,
    range: TimeRange,
    request: &Request,
    hash: &str,
    control: &mut Control<'_>,
) -> Result<(Vec<EventRecord>, PageInfo), TraceError> {
    control.check(0)?;
    indices.sort_unstable_by_key(|i| {
        let e = &session.events[*i];
        (e.start, e.key)
    });
    control.check(0)?;
    let (indices, page) = page(indices, session, request, hash)?;
    let mut items = Vec::with_capacity(indices.len());
    for i in indices {
        control.check(1)?;
        items.push(record(&session.events[i], range));
    }
    Ok((items, page))
}

fn run(
    session: &Session,
    request: &Request,
    control: &mut Control<'_>,
) -> Result<(TimeRange, String, Data), TraceError> {
    control.check(0)?;
    let range = match request.selection.range_ns {
        None=>session.bounds,
        Some([start,end])=>TimeRange::new(start,end).filter(|r|r.start>=session.bounds.start && r.end<=session.bounds.end)
            .ok_or_else(||TraceError::new("trace_query_range", "range must be nonempty and contained in the session's half-open nanosecond bounds"))?,
    };
    if request.selection.lane.is_some_and(|lane| {
        session
            .lanes
            .binary_search_by_key(&lane, |l| l.key)
            .is_err()
            && !session.gpu_lanes.contains(&lane)
    }) {
        return Err(TraceError::new(
            "trace_lane_not_found",
            "capture-local lane is not present in this source",
        ));
    }
    if let Some(frame) = request.selection.frame
        && !session.events[event_index(session, frame)?].is_frame
    {
        return Err(TraceError::new(
            "trace_frame_identity",
            "selection.frame must identify a recorded frame attempt",
        ));
    }
    let selection = analysis::Selection {
        text: request.selection.text.to_lowercase(),
        lane: request.selection.lane,
        frame: request.selection.frame,
    };
    let hash = fingerprint(request)?;
    if request
        .page
        .cursor
        .as_ref()
        .is_some_and(|c| c.source_sha256 != session.source.sha256 || c.query_sha256 != hash)
    {
        return Err(TraceError::new(
            "trace_cursor_stale",
            "cursor belongs to a different source/query/page size or analysis version",
        ));
    }
    let data = match &request.query {
        Query::GpuClocks | Query::GpuSubmissions { .. } | Query::GpuSubmission { .. } => {
            gpu::run(session, request, range, &hash, control)?
        }
        Query::Session => Data::Session {
            bounds_ns: [session.bounds.start, session.bounds.end],
            events: session.events.len(),
            frames: session.frames.len(),
            lanes: session.lanes.len(),
        },
        Query::Lanes => {
            let mut indices = Vec::with_capacity(session.lanes.len());
            for i in 0..session.lanes.len() {
                control.check(1)?;
                indices.push(i);
            }
            let (lanes, page) = page(indices, session, request, &hash)?;
            let mut items = Vec::with_capacity(lanes.len());
            for i in lanes {
                let lane = &session.lanes[i];
                let mut count = 0;
                for row in &lane.rows {
                    control.check(1)?;
                    count += row.events.len();
                }
                items.push(LaneRecord {
                    key: lane.key,
                    events: count,
                    max_depth: lane.max_depth,
                });
            }
            Data::Lanes { items, page }
        }
        Query::Event { key } => {
            let i = event_index(session, *key)?;
            let event = &session.events[i];
            control.check(1)?;
            if !selection.matches(event, range) {
                return Err(TraceError::new(
                    "trace_event_outside_selection",
                    "event exists but does not match this selection",
                ));
            }
            Data::Event {
                item: record(event, range),
                direct_children: session.children[i].len(),
                self_ns: if event.is_span() {
                    Some(analysis::self_time(session, i, range, control)?)
                } else {
                    None
                },
            }
        }
        Query::Hotspots { sort } => {
            let mut result = analysis::analyze_selected(session, range, &selection, control)?;
            sort_timers(&mut result.timers, *sort);
            control.check(0)?;
            let (items, page) = page(result.timers, session, request, &hash)?;
            Data::Hotspots {
                items: items.into_iter().map(timer_record).collect(),
                page,
            }
        }
        Query::Relations { timer, direction } => {
            let (callers, callees) = analysis::relations(
                session,
                range,
                &timer.category,
                &timer.name,
                &selection,
                control,
            )?;
            let mut items = match direction {
                Direction::Callers => callers,
                Direction::Callees => callees,
            };
            sort_timers(&mut items, TimerSort::Inclusive);
            control.check(0)?;
            let (items, page) = page(items, session, request, &hash)?;
            Data::Relations {
                timer: timer.clone(),
                direction: *direction,
                items: items.into_iter().map(timer_record).collect(),
                page,
            }
        }
        Query::Frames { sort } => {
            let frames = analysis::analyze_frames(session, range, &selection, control)?;
            let statistics = FrameStatistics {
                count: frames.frames.len(),
                p50_ns: (!frames.frames.is_empty()).then_some(frames.percentile_ns[0]),
                p95_ns: (!frames.frames.is_empty()).then_some(frames.percentile_ns[1]),
                p99_ns: (!frames.frames.is_empty()).then_some(frames.percentile_ns[2]),
                worst: frames.max_frame.map(|(key, _)| key),
            };
            let mut indices = Vec::with_capacity(frames.frames.len());
            for key in frames.frames {
                control.check(1)?;
                indices.push(session.indices[&key]);
            }
            match sort {
                FrameSort::DurationDesc => indices.sort_unstable_by_key(|i| {
                    let e = &session.events[*i];
                    (std::cmp::Reverse(e.end - e.start), e.key)
                }),
                FrameSort::StartAsc => indices.sort_unstable_by_key(|i| {
                    let e = &session.events[*i];
                    (e.start, e.key)
                }),
            }
            control.check(0)?;
            let (indices, page) = page(indices, session, request, &hash)?;
            control.check(indices.len() as u64)?;
            let items = indices
                .into_iter()
                .map(|i| record(&session.events[i], range))
                .collect();
            Data::Frames {
                items,
                statistics,
                page,
            }
        }
        Query::Tree { root } => {
            let mut stack = vec![event_index(session, *root)?];
            let mut indices = Vec::new();
            while let Some(i) = stack.pop() {
                control.check(1)?;
                if selection.matches(&session.events[i], range) {
                    indices.push(i);
                }
                for child in &session.children[i] {
                    control.check(1)?;
                    stack.push(*child);
                }
            }
            let (items, page) = event_page(session, indices, range, request, &hash, control)?;
            Data::Tree {
                root: *root,
                items,
                page,
            }
        }
        Query::Events {
            kind: wanted,
            timer,
        } => {
            let mut indices = Vec::new();
            for (i, event) in session.events.iter().enumerate() {
                control.check(1)?;
                if selection.matches(event, range)
                    && wanted.is_none_or(|k| k == kind(event))
                    && timer
                        .as_ref()
                        .is_none_or(|t| event.category == t.category && event.name == t.name)
                {
                    indices.push(i);
                }
            }
            let (items, page) = event_page(session, indices, range, request, &hash, control)?;
            Data::Events { items, page }
        }
        Query::Counters { category, name } => {
            let mut indices = Vec::new();
            for (i, event) in session.events.iter().enumerate() {
                control.check(1)?;
                if matches!(event.kind, EventKind::Counter(_))
                    && selection.matches(event, range)
                    && category.as_ref().is_none_or(|c| event.category == *c)
                    && name.as_ref().is_none_or(|n| event.name == *n)
                {
                    indices.push(i);
                }
            }
            let (items, page) = event_page(session, indices, range, request, &hash, control)?;
            Data::Counters { items, page }
        }
        Query::CounterSeries { columns, timer } => counters::run(
            counters::Context {
                session,
                request,
                range,
                selection: &selection,
                hash: &hash,
                columns: *columns,
                timer: timer.as_ref(),
            },
            control,
        )?,
    };
    control.check(0)?;
    Ok((range, hash, data))
}
