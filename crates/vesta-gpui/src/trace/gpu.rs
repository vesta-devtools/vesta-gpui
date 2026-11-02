//! Portable, capture-owned submission facts. No graphics objects or scheduling.
use super::{Session, active, lock};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    sync::Arc,
    time::Duration,
};

/// Optional extension to CPU trace schema 2. Keys are local to this input file.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GpuCapture {
    pub schema_version: u32,
    pub process_id: u32,
    pub elapsed_ns: u64,
    pub dropped_records: u64,
    pub clocks: Vec<GpuClock>,
    pub windows: Vec<u64>,
    pub submissions: Vec<GpuSubmission>,
}

/// One renderer timestamp stream, not a persistent physical-device identity.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GpuClock {
    pub generation: u64,
    pub device_name: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub driver_version: u32,
    pub queue_family: u32,
    pub valid_bits: Option<u32>,
    /// Null when the driver supplied invalid metadata, never a fabricated period.
    pub period_ns: Option<f64>,
    pub unavailable: Option<GpuUnavailable>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GpuUnavailable {
    UnsupportedQueue,
    InvalidMetadata,
    ResultsUnavailable,
    InvalidTimestampRange,
    AmbiguousCounterWrap,
    RecordingAbandoned,
    SubmissionAbandoned,
    ResultsError,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GpuSubmission {
    pub generation: u64,
    pub submission: u64,
    pub window: u64,
    pub thread: u32,
    pub frame_id: Option<u64>,
    /// CPU recording context, NOT GPU execution start or a clock calibration.
    pub recorded_cpu_ns: u64,
    pub outcome: GpuOutcome,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum GpuOutcome {
    Recording,
    PendingCompletion,
    Sample {
        raw_ticks: [u64; 2],
        elapsed_ns: u64,
        completion_upper_bound_ns: u64,
    },
    Unavailable {
        reason: GpuUnavailable,
        raw_ticks: Option<[u64; 2]>,
    },
}

/// One shared timestamp arithmetic contract for renderer and file admission.
/// The completion bound must exclude a whole wrap; no CPU/GPU offset is inferred.
pub fn timestamp_elapsed(
    raw: [u64; 2],
    bits: u32,
    period_ns: f64,
    upper_bound: Duration,
) -> Result<Duration, GpuUnavailable> {
    if !(1..=64).contains(&bits) || !period_ns.is_finite() || period_ns <= 0.0 {
        return Err(GpuUnavailable::InvalidMetadata);
    }
    let wrap_seconds = 2_f64.powi(bits as i32) * period_ns / 1_000_000_000.0;
    if upper_bound.as_secs_f64() >= wrap_seconds {
        return Err(GpuUnavailable::AmbiguousCounterWrap);
    }
    let mask = if bits == 64 {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    };
    let ticks = raw[1].wrapping_sub(raw[0]) & mask;
    let elapsed = Duration::try_from_secs_f64(ticks as f64 * period_ns / 1_000_000_000.0)
        .map_err(|_| GpuUnavailable::InvalidTimestampRange)?;
    if elapsed > upper_bound {
        return Err(GpuUnavailable::InvalidTimestampRange);
    }
    Ok(elapsed)
}

#[derive(Default)]
pub(super) struct State {
    native_windows: HashMap<u64, u64>,
    clock_generations: HashSet<u64>,
    pub capture: GpuCapture,
}

/// Original capture ownership survives renderer completion on another thread.
/// Dropping/finishing this token never records into a successor capture.
pub struct GpuSubmissionToken {
    session: Arc<Session>,
    index: usize,
    submitted: bool,
    completed: bool,
}

/// Renderer boundary. `native_window` is used only for an internal local mapping.
/// Disabled captures return immediately without allocating or retaining metadata.
pub fn begin_submission(
    native_window: u64,
    clock: &GpuClock,
    submission: u64,
) -> Option<GpuSubmissionToken> {
    let session = active()?;
    let mut state = lock(&session.state);
    session.expire(&mut state);
    if state.stopped.is_some() {
        return None;
    }
    if state.gpu.capture.submissions.len() == session.config.max_events {
        state.gpu.capture.dropped_records = state.gpu.capture.dropped_records.saturating_add(1);
        return None;
    }
    let next = state.threads.len() as u32 + 1;
    let thread = *state
        .threads
        .entry(std::thread::current().id())
        .or_insert(next);
    let frame_id = state
        .open
        .get(&thread)
        .and_then(|s| s.last())
        .and_then(|i| state.events[*i].frame)
        .map(|i| i as u64 + 1);
    let gpu = &mut state.gpu;
    let next_window = gpu.native_windows.len() as u64 + 1;
    let window = match gpu.native_windows.entry(native_window) {
        Entry::Occupied(entry) => *entry.get(),
        Entry::Vacant(entry) => {
            entry.insert(next_window);
            gpu.capture.windows.push(next_window);
            next_window
        }
    };
    if gpu.clock_generations.insert(clock.generation) {
        gpu.capture.clocks.push(clock.clone());
    }
    let index = gpu.capture.submissions.len();
    gpu.capture.submissions.push(GpuSubmission {
        generation: clock.generation,
        submission,
        window,
        thread,
        frame_id,
        recorded_cpu_ns: session.now().as_nanos() as u64,
        outcome: GpuOutcome::Recording,
    });
    drop(state);
    Some(GpuSubmissionToken {
        session,
        index,
        submitted: false,
        completed: false,
    })
}

impl GpuSubmissionToken {
    fn update(&self, outcome: GpuOutcome) {
        let mut state = lock(&self.session.state);
        self.session.expire(&mut state);
        if state.stopped.is_none()
            && let Some(record) = state.gpu.capture.submissions.get_mut(self.index)
        {
            record.outcome = outcome;
        }
    }
    /// Call only after the existing command flush succeeds.
    pub fn submitted(&mut self) {
        self.submitted = true;
        self.update(GpuOutcome::PendingCompletion);
    }
    pub fn complete(mut self, outcome: GpuOutcome) {
        self.completed = true;
        self.update(outcome);
    }
}
impl Drop for GpuSubmissionToken {
    fn drop(&mut self) {
        if !self.completed {
            self.update(GpuOutcome::Unavailable {
                reason: if self.submitted {
                    GpuUnavailable::SubmissionAbandoned
                } else {
                    GpuUnavailable::RecordingAbandoned
                },
                raw_ticks: None,
            });
        }
    }
}
