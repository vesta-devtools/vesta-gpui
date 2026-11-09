//! Public wire types. JSON Schema is generated from these actual serde types.
use crate::session::{Diagnostic, EventKey, LaneKey, Source};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
mod memory;
pub use memory::*;

pub const PROTOCOL_VERSION: u32 = 1;
pub const ANALYSIS_VERSION: u32 = 7;
pub const MAX_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Request {
    #[schemars(range(min = 1, max = 1))]
    pub schema_version: u32,
    /// Echoed verbatim; 1..128 UTF-8 bytes. Not part of query identity.
    pub request_id: String,
    pub query: Query,
    #[serde(default)]
    pub selection: Selection,
    #[serde(default)]
    pub page: Page,
    #[serde(default)]
    pub budget: Budget,
}

/// Read-only operations on the single explicitly opened input. No path fields.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Query {
    MemorySummary,
    MemoryAllocations {
        #[serde(default)]
        sort: MemorySort,
        #[serde(default)]
        lifetime: MemoryPredicate,
        stack: Option<MemoryStackKey>,
    },
    MemoryAllocation {
        key: MemoryAllocationKey,
    },
    MemoryStackGroups {
        #[serde(default)]
        lifetime: MemoryPredicate,
    },
    MemoryStack {
        key: MemoryStackKey,
    },
    MemoryCompare {
        start_ns: u64,
        end_ns: u64,
    },
    MemorySeries {
        #[serde(default = "counter_columns")]
        #[schemars(range(min = 1, max = 256))]
        columns: usize,
    },
    Session,
    Lanes,
    /// Whole-source device capability metadata; uncalibrated device clock domain.
    GpuClocks,
    /// Filter by the CPU recording context, never estimated GPU execution start.
    GpuSubmissions {
        #[serde(default)]
        sort: GpuSort,
    },
    GpuSubmission {
        key: GpuKey,
    },
    /// CPU-domain execution estimates, filtered by estimated interval overlap.
    /// Unmapped records matching the other filters remain explicitly unplaced.
    GpuExecution,
    Frames {
        #[serde(default)]
        sort: FrameSort,
    },
    /// Whole CPU frame attempts joined to whole GPU submissions by recorded
    /// frame identity. Selection intersects CPU frames, not GPU execution time.
    FrameTimings {
        #[serde(default)]
        sort: FrameTimingSort,
    },
    Events {
        kind: Option<Kind>,
        timer: Option<TimerKey>,
    },
    Event {
        key: EventKey,
    },
    /// Flat tree rows with original parent keys; no invented causal edges.
    Tree {
        root: EventKey,
    },
    Hotspots {
        #[serde(default)]
        sort: TimerSort,
    },
    /// Immediate captured edges, grouped by recorded category/name labels.
    Relations {
        timer: TimerKey,
        direction: Direction,
    },
    Counters {
        category: Option<String>,
        name: Option<String>,
    },
    /// Bounded actual counter representatives with exact counts and extrema.
    CounterSeries {
        #[serde(default = "counter_columns")]
        #[schemars(range(min = 1, max = 256))]
        columns: usize,
        timer: Option<TimerKey>,
    },
}
const fn counter_columns() -> usize {
    64
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GpuSort {
    #[default]
    CpuRecordingAsc,
    DurationDesc,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GpuKey {
    pub process: u64,
    pub generation: u64,
    pub submission: u64,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrameSort {
    #[default]
    DurationDesc,
    StartAsc,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FrameTimingSort {
    #[default]
    CpuDurationDesc,
    GpuWorkDesc,
    StartAsc,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimerSort {
    #[default]
    #[serde(rename = "inclusive_desc")]
    Inclusive,
    #[serde(rename = "self_desc")]
    SelfTime,
    #[serde(rename = "calls_desc")]
    Calls,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Callers,
    Callees,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Span,
    Marker,
    Counter,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimerKey {
    pub category: String,
    pub name: String,
}

#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    /// Half-open [start,end) integer nanoseconds in the original CPU clock.
    /// Must lie within session bounds; null means the whole recorded session.
    pub range_ns: Option<[u64; 2]>,
    /// Case-insensitive recorded category/name substring, at most 256 bytes.
    #[serde(default)]
    pub text: String,
    pub lane: Option<LaneKey>,
    pub frame: Option<EventKey>,
}
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Page {
    #[serde(default = "page_limit")]
    #[schemars(range(min = 1, max = 1000))]
    pub limit: usize,
    pub cursor: Option<Cursor>,
}
const fn page_limit() -> usize {
    100
}
impl Default for Page {
    fn default() -> Self {
        Self {
            limit: page_limit(),
            cursor: None,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Cursor {
    pub source_sha256: String,
    pub query_sha256: String,
    /// Position in this immutable deterministic result, never an event identity.
    pub offset: usize,
}
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    /// Cooperative event/child visits, plus result projection work.
    #[serde(default = "work_budget")]
    #[schemars(range(min = 1, max = 10000000))]
    pub max_work: u64,
    /// Cooperative; bounded sorting/serialization are not preemptible.
    #[serde(default = "deadline")]
    #[schemars(range(min = 1, max = 10000))]
    pub deadline_ms: u64,
}
const fn work_budget() -> u64 {
    5_000_000
}
const fn deadline() -> u64 {
    5000
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            max_work: work_budget(),
            deadline_ms: deadline(),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Response {
    pub schema_version: u32,
    pub analysis_version: u32,
    pub request_id: Option<String>,
    pub source: Option<Source>,
    pub quality: Option<Quality>,
    pub cost: Cost,
    pub result: Outcome,
}
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Outcome {
    Ok {
        range_ns: [u64; 2],
        query_sha256: String,
        data: Box<Data>,
    },
    Error {
        code: String,
        message: String,
    },
}
#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct Cost {
    pub work: u64,
    pub elapsed_us: u64,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct Quality {
    pub gpu: Option<GpuCoverage>,
    pub memory: Option<MemoryCoverage>,
    pub detail: String,
    pub dropped_events: u64,
    pub unfinished_spans: usize,
    pub unframed_events: usize,
    pub hierarchy_reliable: bool,
    pub capture_expired: bool,
    pub diagnostics: Vec<Diagnostic>,
    /// Coverage is of recorded scopes, not every function or CPU instruction.
    pub full_function_coverage: bool,
    pub gpu_execution_timing: bool,
    pub symbol_resolution: bool,
    pub relationships: String,
    pub self_time_semantics: String,
}
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Data {
    GpuExecution {
        items: Vec<GpuRecord>,
        matched_estimates: usize,
        unplaced_records: usize,
        selection_basis: String,
        page: PageInfo,
    },
    MemorySummary {
        coverage: Option<MemoryCoverage>,
        statistics: Option<MemoryStatistics>,
    },
    MemoryAllocations {
        items: Vec<MemoryAllocationRecord>,
        page: PageInfo,
    },
    MemoryAllocation {
        item: MemoryAllocationRecord,
    },
    MemoryStackGroups {
        items: Vec<MemoryStackGroup>,
        page: PageInfo,
    },
    MemoryStack {
        item: MemoryStackRecord,
    },
    MemoryCompare {
        comparison: MemoryComparison,
    },
    MemorySeries {
        columns: usize,
        items: Vec<MemoryPoint>,
        observations: usize,
        omitted_observations: usize,
        page: PageInfo,
    },
    GpuClocks {
        attached: bool,
        clock_domain: String,
        cpu_clock_mapping: bool,
        items: Vec<GpuClockRecord>,
        page: PageInfo,
    },
    GpuSubmissions {
        items: Vec<GpuRecord>,
        page: PageInfo,
    },
    GpuSubmission {
        item: GpuRecord,
    },
    Session {
        bounds_ns: [u64; 2],
        events: usize,
        frames: usize,
        lanes: usize,
    },
    Lanes {
        items: Vec<LaneRecord>,
        page: PageInfo,
    },
    Frames {
        items: Vec<EventRecord>,
        statistics: FrameStatistics,
        page: PageInfo,
    },
    FrameTimings {
        gpu_attached: bool,
        /// Source-wide records without a CPU frame association; never inferred.
        source_unframed_gpu_records: usize,
        items: Vec<FrameTimingRecord>,
        statistics: FrameTimingStatistics,
        sort: FrameTimingSort,
        page: PageInfo,
    },
    Events {
        items: Vec<EventRecord>,
        page: PageInfo,
    },
    Event {
        item: EventRecord,
        direct_children: usize,
        self_ns: Option<u64>,
    },
    Tree {
        root: EventKey,
        items: Vec<EventRecord>,
        page: PageInfo,
    },
    Hotspots {
        items: Vec<TimerRecord>,
        page: PageInfo,
    },
    Relations {
        timer: TimerKey,
        direction: Direction,
        items: Vec<TimerRecord>,
        page: PageInfo,
    },
    /// Actual raw points, never interpolated or filled with synthetic zeroes.
    Counters {
        items: Vec<EventRecord>,
        page: PageInfo,
    },
    CounterSeries {
        columns: usize,
        items: Vec<CounterTrack>,
        page: PageInfo,
    },
}
#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct GpuCoverage {
    pub records: usize,
    pub samples: usize,
    pub pending: usize,
    pub unavailable: usize,
    pub dropped_records: u64,
    pub cpu_clock_mapping: bool,
    pub execution_estimates: usize,
    pub calibration_failed: usize,
    pub unmapped_samples: usize,
    pub drift_bound_known: bool,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct GpuClockRecord {
    pub calibration_support: String,
    pub process: u64,
    pub generation: u64,
    pub device_name: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub driver_version: u32,
    pub queue_family: u32,
    pub valid_bits: Option<u32>,
    pub period_ns: Option<f64>,
    pub unavailable: Option<String>,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct GpuRecord {
    pub execution: Option<GpuExecutionRecord>,
    pub calibration: Option<GpuCalibrationRecord>,
    pub calibration_error: Option<String>,
    pub key: GpuKey,
    pub window: u64,
    pub lane: LaneKey,
    pub frame: Option<EventKey>,
    /// Association only: GPU execution is not calibrated to this CPU instant.
    pub recorded_cpu_ns: u64,
    pub status: GpuStatus,
    /// Exact base-10 u64 device ticks; strings preserve precision in JS clients.
    pub raw_ticks: Option<[String; 2]>,
    pub elapsed_ns: Option<u64>,
    pub completion_upper_bound_ns: Option<u64>,
    pub unavailable: Option<String>,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct GpuExecutionRecord {
    pub estimated_start_ns: i64,
    pub estimated_end_ns: i64,
    pub anchor_cpu_ns: i64,
    pub anchor_uncertainty_ns: u64,
    pub anchor_distance_ns: u64,
    pub drift_bound_known: bool,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct GpuCalibrationRecord {
    pub device_ticks: String,
    pub host_ticks: String,
    pub host_before_ticks: String,
    pub host_after_ticks: String,
    pub host_frequency_hz: String,
    pub cpu_before_ns: u64,
    pub cpu_after_ns: u64,
    pub max_deviation_ns: u64,
}
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GpuStatus {
    Recording,
    PendingCompletion,
    Sample,
    Unavailable,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct CounterPoint {
    pub event: EventKey,
    pub time_ns: u64,
    pub value: f64,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct CounterTrack {
    pub lane: LaneKey,
    pub timer: TimerKey,
    pub count: usize,
    pub minimum: CounterPoint,
    pub maximum: CounterPoint,
    /// First/last and extrema of time bins, all original observations.
    pub points: Vec<CounterPoint>,
    /// Display/query decimation, not producer capture loss.
    pub omitted_observations: usize,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct PageInfo {
    pub total: usize,
    pub returned: usize,
    pub next_cursor: Option<Cursor>,
    /// True only when more matching rows exist in this source/query.
    pub has_more: bool,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct LaneRecord {
    pub key: LaneKey,
    pub name: Option<String>,
    pub events: usize,
    pub max_depth: usize,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct EventRecord {
    pub key: EventKey,
    pub lane: LaneKey,
    pub parent: Option<EventKey>,
    pub frame: Option<EventKey>,
    pub is_frame: bool,
    pub kind: Kind,
    pub category: String,
    pub name: String,
    pub start_ns: u64,
    pub end_ns: u64,
    /// Whole recorded duration, not clipped selection work or presented latency.
    pub duration_ns: u64,
    pub overlap_ns: u64,
    pub depth: usize,
    pub unfinished: bool,
    pub counter_value: Option<f64>,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct TimerRecord {
    pub timer: TimerKey,
    pub calls: usize,
    /// Clipped recorded intervals; inclusive work overlaps across nesting/lanes.
    pub inclusive_ns: u64,
    /// Unattributed elapsed wall time, including waits and uninstrumented work.
    pub self_ns: u64,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct FrameStatistics {
    pub count: usize,
    /// Nearest-rank percentiles of whole recorded attempts intersecting selection.
    pub p50_ns: Option<u64>,
    pub p95_ns: Option<u64>,
    pub p99_ns: Option<u64>,
    pub worst: Option<EventKey>,
}

#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct FrameGpuWork {
    pub records: usize,
    pub samples: usize,
    pub pending: usize,
    pub unavailable: usize,
    /// Sum of whole sampled submission durations, including work outside the
    /// selected CPU interval. Overlap can make this exceed elapsed frame time.
    /// Null is missing timing; Some(0) is an actual zero-duration sample.
    pub sampled_work_ns: Option<u64>,
    pub longest_sample_ns: Option<u64>,
    pub longest_submission: Option<GpuKey>,
    /// Only covers recorded submissions; false if none or producer GPU loss.
    pub all_recorded_submissions_sampled: bool,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct FrameTimingRecord {
    pub frame: EventRecord,
    pub gpu: FrameGpuWork,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct FrameTimingStatistics {
    pub cpu: FrameStatistics,
    pub frames_with_gpu_records: usize,
    pub frames_with_gpu_samples: usize,
    pub frames_with_all_recorded_submissions_sampled: usize,
    pub gpu_samples: usize,
    pub gpu_pending: usize,
    pub gpu_unavailable: usize,
    pub source_dropped_gpu_records: u64,
}
