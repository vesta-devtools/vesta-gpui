//! Immutable captured facts. Native presentation and file I/O are separate owners.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, error::Error, fmt};

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct EventKey {
    pub process: u64,
    pub event: u64,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct LaneKey {
    pub process: u64,
    /// Capture-local, not an operating-system thread identifier.
    pub thread: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeRange {
    pub start: u64,
    pub end: u64,
}

impl TimeRange {
    pub fn new(start: u64, end: u64) -> Option<Self> {
        (start < end).then_some(Self { start, end })
    }
    pub fn duration(self) -> u64 {
        self.end - self.start
    }
    pub fn overlap(self, start: u64, end: u64) -> u64 {
        end.min(self.end).saturating_sub(start.max(self.start))
    }
    pub fn contains(self, instant: u64) -> bool {
        self.start <= instant && instant < self.end
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EventKind {
    Span { unfinished: bool },
    Marker,
    Counter(f64),
}

#[derive(Clone, Debug)]
pub struct Event {
    pub key: EventKey,
    pub lane: LaneKey,
    pub parent: Option<EventKey>,
    pub frame: Option<EventKey>,
    pub is_frame: bool,
    pub name: String,
    pub category: String,
    pub start: u64,
    pub end: u64,
    pub kind: EventKind,
    pub depth: usize,
}

impl Event {
    pub fn visible_in(&self, range: TimeRange) -> bool {
        if self.start == self.end {
            range.contains(self.start)
        } else {
            self.start < range.end && range.start < self.end
        }
    }
    pub fn is_span(&self) -> bool {
        matches!(self.kind, EventKind::Span { .. })
    }
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug)]
pub struct TraceError {
    pub code: &'static str,
    pub message: String,
    source: Option<Box<dyn Error + Send + Sync>>,
}
impl TraceError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            source: None,
        }
    }
    pub fn caused(code: &'static str, source: impl Error + Send + Sync + 'static) -> Self {
        Self {
            code,
            message: source.to_string(),
            source: Some(Box::new(source)),
        }
    }
}
impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl Error for TraceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_deref().map(|e| e as &(dyn Error + 'static))
    }
}

#[derive(Debug)]
pub struct Lane {
    pub key: LaneKey,
    pub rows: Vec<EventRow>,
    pub max_depth: usize,
    pub counters: Vec<crate::counters::Series>,
}
#[derive(Debug)]
pub struct EventRow {
    pub depth: usize,
    /// Sorted by start, descending end, then stable identity.
    pub events: Vec<usize>,
    pub prefix_end: Vec<u64>,
    pub plot: crate::plot::PlotIndex,
}
impl Lane {
    pub fn visible<'a>(
        &'a self,
        session: &'a Session,
        range: TimeRange,
    ) -> impl Iterator<Item = usize> + 'a {
        self.rows
            .iter()
            .flat_map(move |row| row.visible(session, range))
    }
}
impl EventRow {
    pub fn indexed(
        depth: usize,
        events: Vec<usize>,
        source: &[Event],
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<Self, TraceError> {
        let mut end = 0;
        let prefix_end = events
            .iter()
            .map(|i| {
                end = end.max(source[*i].end);
                end
            })
            .collect();
        let plot = crate::plot::PlotIndex::new(&events, source, cancel)?;
        Ok(Self {
            depth,
            events,
            prefix_end,
            plot,
        })
    }
    pub fn visible<'a>(
        &'a self,
        session: &'a Session,
        range: TimeRange,
    ) -> impl Iterator<Item = usize> + 'a {
        let first = self.prefix_end.partition_point(|end| *end < range.start);
        let end = self
            .events
            .partition_point(|i| session.events[*i].start < range.end);
        self.events[first.min(end)..end]
            .iter()
            .copied()
            .filter(move |i| session.events[*i].visible_in(range))
    }
}

#[derive(Debug)]
pub struct Session {
    pub gpu_tracks: Vec<crate::gpu::plot::Track>,
    pub gpu_execution: BTreeMap<(u64, u64), vesta_gpui::trace::gpu::GpuExecutionEstimate>,
    pub memory: Option<crate::memory::MemorySession>,
    pub thread_names: BTreeMap<LaneKey, Option<String>>,
    pub gpu: Option<crate::gpu::GpuCapture>,
    pub gpu_counts: crate::gpu::Counts,
    pub gpu_lanes: std::collections::BTreeSet<LaneKey>,
    pub gpu_indices: BTreeMap<(u64, u64), usize>,
    pub source: Source,
    pub events: Vec<Event>,
    pub indices: BTreeMap<EventKey, usize>,
    /// First captured span of each label group; stable native row identity.
    pub timer_anchors: BTreeMap<(String, String), usize>,
    pub children: Vec<Vec<usize>>,
    pub frames: Vec<usize>,
    pub frame_plot: crate::plot::PlotIndex,
    pub lanes: Vec<Lane>,
    pub bounds: TimeRange,
    pub diagnostics: Vec<Diagnostic>,
    pub detail: String,
    pub dropped_events: u64,
    pub unframed_events: usize,
    pub unfinished_spans: usize,
    pub capture_expired: bool,
    pub hierarchy_reliable: bool,
}

/// Identity of the complete, unchanged input file (including an HTML wrapper).
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct Source {
    pub capture_extension_version: Option<u32>,
    pub sha256: String,
    pub bytes: usize,
    pub format: SourceFormat,
    pub producer_schema_version: u32,
    pub clock: String,
    pub thread_ids: String,
    pub source_time_unit: String,
    pub analysis_time_unit: String,
}
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    GpuiJson,
    GpuiHtmlData,
}
