//! Heap evidence uses operation identities, never addresses as lifetime keys.
use super::*;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct MemoryAllocationKey {
    pub process: u64,
    pub allocation: u64,
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct MemoryStackKey {
    pub process: u64,
    pub stack: u64,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MemorySort {
    #[default]
    SizeDesc,
    AllocatedAsc,
    LifetimeDesc,
}
/// At time t, apply all recorded operations with time <= t. An allocation freed
/// at t is absent. Nanosecond ties retain raw operation order in memory_series.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum MemoryPredicate {
    #[default]
    All,
    LiveAt {
        time_ns: u64,
    },
    NewSurvivors {
        start_ns: u64,
        end_ns: u64,
    },
    Released {
        start_ns: u64,
        end_ns: u64,
    },
    Common {
        start_ns: u64,
        end_ns: u64,
    },
    Transient {
        start_ns: u64,
        end_ns: u64,
    },
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct MemoryCoverage {
    pub process: u64,
    pub provider: String,
    /// Inclusive last known instant; coverage may end before the CPU trace.
    pub through_ns: u64,
    pub baseline_known: bool,
    pub process_wide: bool,
    pub recorder_allocations_excluded: bool,
    pub event_limit_reached: bool,
    pub hook_errors: u64,
    pub operations: usize,
    pub stacks: usize,
    pub allocation_records: usize,
    pub allocations_with_stacks: usize,
    pub stack_frames_requested: usize,
    pub potentially_truncated_stacks: usize,
    pub stack_frame_budget: usize,
    pub stack_observations_dropped: usize,
    pub symbol_budget_exhausted: bool,
    pub resolved_frames: usize,
    pub unresolved_frames: usize,
    pub unknown_frees: usize,
    pub unknown_reallocations: usize,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct MemoryStatistics {
    pub allocations: usize,
    pub observed_allocated_bytes: u64,
    pub observed_live_bytes_at_end: u64,
    pub observed_live_count_at_end: usize,
    pub peak_observed_live_bytes: u64,
    pub peak_observed_live_count: usize,
    pub failed_reallocations: usize,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct MemoryAllocationRecord {
    pub key: MemoryAllocationKey,
    /// Hexadecimal pointer; reused addresses are distinct allocation keys.
    pub address: String,
    pub size_bytes: u64,
    pub allocated_ns: u64,
    pub freed_ns: Option<u64>,
    pub observed_lifetime_ns: u64,
    pub allocation_lane: LaneKey,
    pub free_lane: Option<LaneKey>,
    pub stack: Option<MemoryStackKey>,
    pub end_operation: Option<u64>,
    pub reallocated_from: Option<MemoryAllocationKey>,
    pub unknown_prior_reallocation: bool,
    pub live_at_coverage_end: bool,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct MemoryStackFrameRecord {
    pub address: String,
    pub function: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct MemoryStackRecord {
    pub key: MemoryStackKey,
    pub potentially_truncated: bool,
    pub frames: Vec<MemoryStackFrameRecord>,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct MemoryStackGroup {
    pub stack: Option<MemoryStackKey>,
    pub allocations: usize,
    pub allocated_bytes: u64,
    pub observed_live_bytes_at_end: u64,
    pub representative: MemoryAllocationKey,
}
#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct MemoryCohort {
    pub allocations: usize,
    pub bytes: u64,
    pub representative: Option<MemoryAllocationKey>,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct MemoryComparison {
    pub start_ns: u64,
    pub end_ns: u64,
    pub observed_live_at_start: MemoryCohort,
    pub observed_live_at_end: MemoryCohort,
    pub common: MemoryCohort,
    pub new_survivors: MemoryCohort,
    pub released: MemoryCohort,
    pub transient: MemoryCohort,
    /// Exact signed decimal change in observed bytes, never a leak diagnosis.
    pub observed_change_bytes: String,
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct MemoryPoint {
    pub operation: u64,
    pub time_ns: u64,
    pub observed_live_bytes: u64,
    pub observed_live_count: usize,
}
