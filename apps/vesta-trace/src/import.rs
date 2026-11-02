//! Bounded data import. HTML scripts are never evaluated.
use crate::session::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

pub const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_EVENTS: usize = 1_000_000;
const MAX_DEPTH: usize = 4096;
const MAX_EXACT_NS: u64 = (1_u64 << 53) - 1;

#[derive(Deserialize)]
struct Input {
    #[serde(rename = "traceEvents")]
    events: Vec<RawEvent>,
    gpui_trace: Header,
    gpui_capture: Option<crate::gpu::GpuCapture>,
}
#[derive(Deserialize)]
struct Header {
    schema_version: u32,
    clock: String,
    thread_ids: String,
    gpu_execution_timing: bool,
    detail: Option<String>,
    events: usize,
    dropped_events: u64,
    unfinished_spans: usize,
    invalid_nesting: u64,
    unframed_events: usize,
    expired: bool,
}
#[derive(Deserialize)]
struct RawEvent {
    pid: u64,
    tid: u64,
    ts: f64,
    dur: Option<f64>,
    ph: String,
    name: String,
    cat: String,
    args: Args,
}
#[derive(Deserialize)]
struct Args {
    event_id: u64,
    parent_id: Option<u64>,
    frame_id: Option<u64>,
    frame: bool,
    #[serde(default)]
    unfinished: bool,
    value: Option<f64>,
}

pub fn nanoseconds(us: f64) -> Result<u64, TraceError> {
    let ns = us * 1000.0;
    if !ns.is_finite() || ns < 0.0 || ns > MAX_EXACT_NS as f64 {
        return Err(TraceError::new(
            "trace_numeric_range",
            "time must be finite, nonnegative and exactly representable in nanoseconds",
        ));
    }
    Ok(ns.round() as u64)
}
fn cancelled(cancel: &AtomicBool) -> Result<(), TraceError> {
    if cancel.load(Ordering::Relaxed) {
        Err(TraceError::new("trace_cancelled", "session load cancelled"))
    } else {
        Ok(())
    }
}

pub fn read(path: &Path, cancel: &AtomicBool) -> Result<Session, TraceError> {
    cancelled(cancel)?;
    let file = File::open(path).map_err(|e| TraceError::caused("trace_open", e))?;
    if file
        .metadata()
        .map_err(|e| TraceError::caused("trace_metadata", e))?
        .len()
        > MAX_FILE_BYTES
    {
        return Err(TraceError::new(
            "trace_file_budget",
            "trace exceeds the 256 MiB input budget",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| TraceError::caused("trace_read", e))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(TraceError::new(
            "trace_file_budget",
            "trace grew beyond the input budget",
        ));
    }
    parse(&bytes, cancel)
}

pub fn parse(bytes: &[u8], cancel: &AtomicBool) -> Result<Session, TraceError> {
    cancelled(cancel)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(TraceError::new(
            "trace_file_budget",
            "trace exceeds the input budget",
        ));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|e| TraceError::caused("trace_utf8", e))?
        .trim_start_matches('\u{feff}')
        .trim_start();
    let json = if text.starts_with('<') {
        let marker = "id=\"gpui-data\"";
        let pos = text
            .find(marker)
            .ok_or_else(|| TraceError::new("trace_html_data", "not a GPUI data report"))?;
        if text[pos + marker.len()..].contains(marker) {
            return Err(TraceError::new(
                "trace_html_data",
                "ambiguous embedded trace data",
            ));
        }
        let open = text[..pos]
            .rfind("<script")
            .ok_or_else(|| TraceError::new("trace_html_data", "missing data script"))?;
        let body = text[pos..]
            .find('>')
            .map(|i| pos + i + 1)
            .ok_or_else(|| TraceError::new("trace_html_data", "unterminated data tag"))?;
        if !text[open..body].contains("type=\"application/json\"") {
            return Err(TraceError::new(
                "trace_html_data",
                "trace script must be application/json",
            ));
        }
        let end = text[body..]
            .find("</script>")
            .map(|i| body + i)
            .ok_or_else(|| TraceError::new("trace_html_data", "unterminated data block"))?;
        &text[body..end]
    } else {
        text
    };
    let input: Input =
        serde_json::from_str(json).map_err(|e| TraceError::caused("trace_json", e))?;
    cancelled(cancel)?;
    let source = Source {
        capture_extension_version: None,
        sha256: format!("{:x}", Sha256::digest(bytes)),
        bytes: bytes.len(),
        format: if text.starts_with('<') {
            SourceFormat::GpuiHtmlData
        } else {
            SourceFormat::GpuiJson
        },
        producer_schema_version: input.gpui_trace.schema_version,
        clock: input.gpui_trace.clock.clone(),
        thread_ids: input.gpui_trace.thread_ids.clone(),
        source_time_unit: "microseconds".into(),
        analysis_time_unit: "nanoseconds".into(),
    };
    let mut input = input;
    let gpu = input.gpui_capture.take();
    let mut session = admit(input, source, cancel)?;
    if let Some(gpu) = gpu {
        crate::gpu::admit(&mut session, gpu, cancel)?;
    }
    Ok(session)
}

fn admit(input: Input, source: Source, cancel: &AtomicBool) -> Result<Session, TraceError> {
    let header = input.gpui_trace;
    if header.schema_version != 2 {
        return Err(TraceError::new(
            "trace_schema_unsupported",
            format!("GPUI schema {} is unsupported", header.schema_version),
        ));
    }
    if header.clock != "monotonic_elapsed_wall_time"
        || header.thread_ids != "capture_local"
        || header.gpu_execution_timing
    {
        return Err(TraceError::new(
            "trace_clock_unsupported",
            "this importer requires the explicit GPUI v2 CPU clock domain",
        ));
    }
    if input.events.len() > MAX_EVENTS || input.events.len() != header.events {
        return Err(TraceError::new(
            "trace_event_budget",
            "event count exceeds the budget or differs from its header",
        ));
    }
    let mut events = Vec::with_capacity(input.events.len());
    let mut indices = BTreeMap::new();
    for raw in input.events {
        if events.len() % 1024 == 0 {
            cancelled(cancel)?;
        }
        let key = EventKey {
            process: raw.pid,
            event: raw.args.event_id,
        };
        if key.event == 0 || indices.insert(key, events.len()).is_some() {
            return Err(TraceError::new(
                "trace_event_identity",
                "event identities must be nonzero and unique within their process",
            ));
        }
        let start = nanoseconds(raw.ts)?;
        let (kind, duration) = match raw.ph.as_str() {
            "X" => (
                EventKind::Span {
                    unfinished: raw.args.unfinished,
                },
                nanoseconds(raw.dur.ok_or_else(|| {
                    TraceError::new("trace_duration_missing", "complete event has no duration")
                })?)?,
            ),
            "i" => (EventKind::Marker, 0),
            "C" => (
                EventKind::Counter(raw.args.value.filter(|v| v.is_finite()).ok_or_else(|| {
                    TraceError::new("trace_counter_invalid", "counter has no finite value")
                })?),
                0,
            ),
            _ => {
                return Err(TraceError::new(
                    "trace_phase_unsupported",
                    format!("unsupported event phase {}", raw.ph),
                ));
            }
        };
        let end = start
            .checked_add(duration)
            .filter(|end| *end <= MAX_EXACT_NS)
            .ok_or_else(|| {
                TraceError::new("trace_numeric_range", "event end exceeds the time domain")
            })?;
        let reference = |event| EventKey {
            process: raw.pid,
            event,
        };
        events.push(Event {
            key,
            lane: LaneKey {
                process: raw.pid,
                thread: raw.tid,
            },
            parent: raw.args.parent_id.map(reference),
            frame: raw.args.frame_id.map(reference),
            is_frame: raw.args.frame,
            name: raw.name,
            category: raw.cat,
            start,
            end,
            kind,
            depth: 0,
        });
    }
    let mut children = vec![Vec::new(); events.len()];
    let mut frames = Vec::new();
    let mut reliable = header.invalid_nesting == 0;
    for (index, event) in events.iter().enumerate() {
        if let Some(parent) = event.parent {
            let parent = *indices.get(&parent).ok_or_else(|| {
                TraceError::new("trace_parent_missing", "parent identity is absent")
            })?;
            if parent == index || events[parent].lane != event.lane || !events[parent].is_span() {
                return Err(TraceError::new(
                    "trace_parent_invalid",
                    "parent must be a different span on the same lane",
                ));
            }
            if event.start < events[parent].start || event.end > events[parent].end {
                reliable = false;
            }
            children[parent].push(index);
        }
        if let Some(frame) = event.frame {
            let frame = *indices.get(&frame).ok_or_else(|| {
                TraceError::new("trace_frame_missing", "frame identity is absent")
            })?;
            if !events[frame].is_frame || events[frame].lane != event.lane {
                return Err(TraceError::new(
                    "trace_frame_invalid",
                    "frame must identify its recorded CPU lane",
                ));
            }
        }
        if event.is_frame {
            if !event.is_span() || event.frame != Some(event.key) {
                return Err(TraceError::new(
                    "trace_frame_invalid",
                    "frame must be a span with its own stable frame identity",
                ));
            }
            frames.push(index);
        }
    }
    let mut depths = vec![None; events.len()];
    let mut visiting = vec![false; events.len()];
    for index in 0..events.len() {
        if index % 1024 == 0 {
            cancelled(cancel)?;
        }
        let mut chain = Vec::new();
        let mut cursor = Some(index);
        while let Some(at) = cursor {
            if depths[at].is_some() {
                break;
            }
            if visiting[at] {
                return Err(TraceError::new(
                    "trace_parent_cycle",
                    "cyclic parent hierarchy",
                ));
            }
            chain.push(at);
            if chain.len() > MAX_DEPTH {
                return Err(TraceError::new(
                    "trace_hierarchy_budget",
                    "parent hierarchy exceeds its depth budget",
                ));
            }
            visiting[at] = true;
            cursor = events[at].parent.and_then(|key| indices.get(&key).copied());
        }
        let mut depth = cursor.and_then(|i| depths[i]).map_or(0, |d| d + 1);
        for at in chain.into_iter().rev() {
            if depth >= MAX_DEPTH {
                return Err(TraceError::new(
                    "trace_hierarchy_budget",
                    "parent hierarchy exceeds its depth budget",
                ));
            }
            visiting[at] = false;
            depths[at] = Some(depth);
            events[at].depth = depth;
            depth += 1;
        }
    }
    let order = |i: &usize| {
        (
            events[*i].start,
            std::cmp::Reverse(events[*i].end),
            events[*i].key,
        )
    };
    frames.sort_by_key(order);
    let mut lane_map: BTreeMap<LaneKey, Vec<usize>> = BTreeMap::new();
    for (index, event) in events.iter().enumerate() {
        lane_map.entry(event.lane).or_default().push(index);
    }
    let lanes = lane_map
        .into_iter()
        .map(|(key, entries)| {
            let counters = crate::counters::Series::for_lane(&entries, &events, cancel)?;
            let max_depth = entries.iter().map(|i| events[*i].depth).max().unwrap_or(0);
            let mut bands = BTreeMap::<usize, Vec<usize>>::new();
            for i in entries {
                bands.entry(events[i].depth).or_default().push(i);
            }
            let rows = bands
                .into_iter()
                .map(|(depth, mut entries)| {
                    entries.sort_by_key(order);
                    let mut previous_span_end = 0;
                    for i in &entries {
                        let event = &events[*i];
                        if event.is_span() && event.end > event.start {
                            if event.start < previous_span_end {
                                reliable = false;
                            }
                            previous_span_end = previous_span_end.max(event.end);
                        }
                    }
                    EventRow::indexed(depth, entries, &events, cancel)
                })
                .collect::<Result<Vec<_>, TraceError>>()?;
            Ok(Lane {
                counters,
                key,
                rows,
                max_depth,
            })
        })
        .collect::<Result<Vec<_>, TraceError>>()?;
    let start = events.iter().map(|e| e.start).min().unwrap_or(0);
    let end = events
        .iter()
        .map(|e| e.end.saturating_add(u64::from(e.start == e.end)))
        .max()
        .unwrap_or(1)
        .max(start + 1);
    let unframed = events.iter().filter(|e| e.frame.is_none()).count();
    let unfinished = events
        .iter()
        .filter(|e| matches!(e.kind, EventKind::Span { unfinished: true }))
        .count();
    if unframed != header.unframed_events || unfinished != header.unfinished_spans {
        return Err(TraceError::new(
            "trace_loss_metadata",
            "unframed/clipped event counts differ from the recorded header",
        ));
    }
    let mut diagnostics = Vec::new();
    let mut warn = |code, message| diagnostics.push(Diagnostic { code, message });
    if header.detail.is_none() {
        warn(
            "trace_detail_unspecified",
            "Older schema-v2 capture does not declare detail; coverage is unspecified".into(),
        );
    }
    if header.dropped_events > 0 {
        warn(
            "trace_events_dropped",
            format!(
                "{} events were dropped; this is an incomplete capture",
                header.dropped_events
            ),
        );
    }
    if unfinished > 0 {
        warn(
            "trace_scopes_clipped",
            format!("{unfinished} scopes were clipped when collection ended"),
        );
    }
    if !reliable {
        warn("trace_nesting_invalid", "Recorded nesting is unreliable; self and relation analysis are qualified as incomplete".into());
    }
    if unframed > 0 {
        warn(
            "trace_unframed_events",
            format!("{unframed} events have no frame and remain visible on their own lane"),
        );
    }
    if header.expired {
        warn(
            "trace_deadline_expired",
            "Collection reached its deadline".into(),
        );
    }
    let mut timer_anchors = BTreeMap::new();
    for (index, event) in events.iter().enumerate() {
        if event.is_span() {
            timer_anchors
                .entry((event.category.clone(), event.name.clone()))
                .or_insert(index);
        }
    }
    let frame_plot = crate::plot::PlotIndex::new(&frames, &events, cancel)?;
    Ok(Session {
        gpu: None,
        gpu_counts: Default::default(),
        gpu_lanes: Default::default(),
        gpu_indices: Default::default(),
        frame_plot,
        timer_anchors,
        source,
        unfinished_spans: unfinished,
        capture_expired: header.expired,
        events,
        indices,
        children,
        frames,
        lanes,
        bounds: TimeRange { start, end },
        diagnostics,
        detail: header
            .detail
            .unwrap_or_else(|| "unspecified (legacy)".into()),
        dropped_events: header.dropped_events,
        unframed_events: unframed,
        hierarchy_reliable: reliable,
    })
}
