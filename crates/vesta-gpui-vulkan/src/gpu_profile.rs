//! Opt-in command timestamps; completion and scheduling remain Submission-owned.
use crate::{GpuContext, RendererError};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use vulkano::query::{QueryPool, QueryResultFlags};

const CAPACITY: usize = 256;
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

/// Process-local observation identity, independent of author IDs and presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GpuProfileKey {
    pub generation: u64,
    pub submission: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuTimingUnavailable {
    UnsupportedQueue,
    InvalidMetadata,
    ResultsUnavailable,
    InvalidTimestampRange,
    AmbiguousCounterWrap,
}

#[derive(Clone, Copy, Debug)]
pub struct GpuTimestampCapabilities {
    pub generation: u64,
    pub valid_bits: Option<u32>,
    pub period_ns: f64,
    pub unavailable: Option<GpuTimingUnavailable>,
}

/// Timestamp span bracketing upload and scene/UI commands. Includes device
/// dependency/scheduling stalls between the marks; not presentation completion.
#[derive(Clone, Copy, Debug)]
pub struct FrameGpuProfile {
    pub key: GpuProfileKey,
    pub raw_ticks: [u64; 2],
    pub valid_bits: u32,
    pub period_ns: f64,
    pub elapsed: Duration,
    /// Submission creation through observed completion, used only as an upper
    /// bound to reject an ambiguous counter wrap, not as a GPU/CPU clock offset.
    pub completion_upper_bound: Duration,
}

#[derive(Clone, Copy, Debug)]
pub enum GpuProfileObservation {
    Sample(FrameGpuProfile),
    Unavailable {
        key: GpuProfileKey,
        reason: GpuTimingUnavailable,
        raw_ticks: Option<[u64; 2]>,
    },
}

/// Clones share one consuming queue. No background task or redraw is requested.
#[derive(Clone)]
pub struct GpuProfileStream {
    capabilities: GpuTimestampCapabilities,
    queue: Arc<Mutex<VecDeque<GpuProfileObservation>>>,
    dropped: Arc<AtomicU64>,
}

impl GpuProfileStream {
    pub fn capabilities(&self) -> GpuTimestampCapabilities {
        self.capabilities
    }

    /// None means busy/poisoned telemetry, not a zero-duration GPU observation.
    pub fn try_drain(&self) -> Option<Vec<GpuProfileObservation>> {
        self.queue
            .try_lock()
            .ok()
            .map(|mut queue| queue.drain(..).collect())
    }

    pub fn dropped_observations(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    fn publish(&self, observation: GpuProfileObservation) {
        let Ok(mut queue) = self.queue.try_lock() else {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        };
        if queue.len() == CAPACITY {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        } else {
            queue.push_back(observation);
        }
    }
}

pub(super) struct GpuTimestampState {
    pub stream: GpuProfileStream,
    submission: u64,
}

impl GpuTimestampState {
    pub fn new(gpu: &GpuContext) -> Result<Self, RendererError> {
        let generation = NEXT_GENERATION
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| {
                RendererError::at("gpu_profile_identity", "profiling generations exhausted")
            })?;
        let valid_bits = gpu
            .device
            .physical_device()
            .queue_family_properties()
            .get(gpu.queue.queue_family_index() as usize)
            .and_then(|family| family.timestamp_valid_bits);
        let period_ns = f64::from(gpu.device.physical_device().properties().timestamp_period);
        let unavailable = match valid_bits {
            None => Some(GpuTimingUnavailable::UnsupportedQueue),
            Some(bits)
                if !(1..=64).contains(&bits) || !period_ns.is_finite() || period_ns <= 0.0 =>
            {
                Some(GpuTimingUnavailable::InvalidMetadata)
            }
            Some(_) => None,
        };
        Ok(Self {
            stream: GpuProfileStream {
                capabilities: GpuTimestampCapabilities {
                    generation,
                    valid_bits,
                    period_ns,
                    unavailable,
                },
                queue: Arc::new(Mutex::new(VecDeque::with_capacity(CAPACITY))),
                dropped: Arc::new(AtomicU64::new(0)),
            },
            submission: 0,
        })
    }

    pub fn key(&self) -> Option<GpuProfileKey> {
        (self.submission != 0).then_some(GpuProfileKey {
            generation: self.stream.capabilities.generation,
            submission: self.submission,
        })
    }

    pub fn next(&mut self) -> Result<Option<GpuTimestampTicket>, RendererError> {
        if self.stream.capabilities.unavailable.is_some() {
            return Ok(None);
        }
        self.submission = self.submission.checked_add(1).ok_or_else(|| {
            RendererError::at("gpu_profile_identity", "profiling submissions exhausted")
        })?;
        Ok(Some(GpuTimestampTicket {
            key: GpuProfileKey {
                generation: self.stream.capabilities.generation,
                submission: self.submission,
            },
            stream: self.stream.clone(),
        }))
    }
}

pub(super) struct GpuTimestampTicket {
    key: GpuProfileKey,
    stream: GpuProfileStream,
}

impl GpuTimestampTicket {
    pub fn submitted(self, pool: Arc<QueryPool>) -> PendingGpuTimestamp {
        PendingGpuTimestamp { pool, ticket: self }
    }
}

/// Held alongside the command future until the existing completion fence signals.
pub(super) struct PendingGpuTimestamp {
    pool: Arc<QueryPool>,
    ticket: GpuTimestampTicket,
}

impl PendingGpuTimestamp {
    pub fn complete(self, maximum_elapsed: Duration) -> Result<(), RendererError> {
        let mut values = [0_u64; 4];
        let available = self
            .pool
            .get_results(0..2, &mut values, QueryResultFlags::WITH_AVAILABILITY)
            .map_err(|error| RendererError::caused_by("gpu_profile_results", error))?;
        let caps = self.ticket.stream.capabilities;
        let observation = decode(
            self.ticket.key,
            values,
            available,
            caps.valid_bits,
            caps.period_ns,
            maximum_elapsed,
        );
        self.ticket.stream.publish(observation);
        Ok(())
    }
}

fn decode(
    key: GpuProfileKey,
    values: [u64; 4],
    available: bool,
    bits: Option<u32>,
    period: f64,
    maximum_elapsed: Duration,
) -> GpuProfileObservation {
    if !available || values[1] == 0 || values[3] == 0 {
        return GpuProfileObservation::Unavailable {
            key,
            reason: GpuTimingUnavailable::ResultsUnavailable,
            raw_ticks: None,
        };
    }
    let Some(bits) = bits.filter(|bits| (1..=64).contains(bits)) else {
        return GpuProfileObservation::Unavailable {
            key,
            reason: GpuTimingUnavailable::InvalidMetadata,
            raw_ticks: Some([values[0], values[2]]),
        };
    };
    if !period.is_finite() || period <= 0.0 {
        return GpuProfileObservation::Unavailable {
            key,
            reason: GpuTimingUnavailable::InvalidMetadata,
            raw_ticks: Some([values[0], values[2]]),
        };
    }
    let wrap_seconds = 2_f64.powi(bits as i32) * period / 1_000_000_000.0;
    if maximum_elapsed.as_secs_f64() >= wrap_seconds {
        return GpuProfileObservation::Unavailable {
            key,
            reason: GpuTimingUnavailable::AmbiguousCounterWrap,
            raw_ticks: Some([values[0], values[2]]),
        };
    }
    let mask = if bits == 64 {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    };
    let ticks = values[2].wrapping_sub(values[0]) & mask;
    let Ok(elapsed) = Duration::try_from_secs_f64(ticks as f64 * period / 1_000_000_000.0) else {
        return GpuProfileObservation::Unavailable {
            key,
            reason: GpuTimingUnavailable::InvalidTimestampRange,
            raw_ticks: Some([values[0], values[2]]),
        };
    };
    if elapsed > maximum_elapsed {
        return GpuProfileObservation::Unavailable {
            key,
            reason: GpuTimingUnavailable::InvalidTimestampRange,
            raw_ticks: Some([values[0], values[2]]),
        };
    }
    GpuProfileObservation::Sample(FrameGpuProfile {
        key,
        raw_ticks: [values[0], values[2]],
        valid_bits: bits,
        period_ns: period,
        elapsed,
        completion_upper_bound: maximum_elapsed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_preserve_wrap_availability_and_bounded_nonblocking_delivery() {
        let key = GpuProfileKey {
            generation: 7,
            submission: 1,
        };
        let upper_bound = Duration::from_nanos(100);
        let GpuProfileObservation::Sample(sample) =
            decode(key, [250, 1, 3, 1], true, Some(8), 2.0, upper_bound)
        else {
            panic!("available wrapped ticks must produce an observation")
        };
        assert_eq!(sample.elapsed, Duration::from_nanos(18));
        let GpuProfileObservation::Sample(sample) = decode(
            key,
            [u64::MAX - 2, 1, 4, 1],
            true,
            Some(64),
            1.0,
            upper_bound,
        ) else {
            panic!("64-bit wrap must not overflow")
        };
        assert_eq!(sample.elapsed, Duration::from_nanos(7));
        for (values, available, bits, period) in [
            ([0, 0, 0, 0], false, Some(64), 1.0),
            ([1, 1, 2, 0], true, Some(64), 1.0),
            ([1, 1, 2, 1], true, None, 1.0),
            ([1, 1, 2, 1], true, Some(65), 1.0),
            ([1, 1, 2, 1], true, Some(8), f64::NAN),
        ] {
            assert!(matches!(
                decode(key, values, available, bits, period, upper_bound),
                GpuProfileObservation::Unavailable { .. }
            ));
        }
        assert!(
            matches!(
                decode(
                    key,
                    [250, 1, 3, 1],
                    true,
                    Some(8),
                    2.0,
                    Duration::from_nanos(1024)
                ),
                GpuProfileObservation::Unavailable {
                    reason: GpuTimingUnavailable::AmbiguousCounterWrap,
                    ..
                }
            ),
            "a delayed observation cannot prove how many wraps elapsed"
        );
        assert!(matches!(
            decode(key, [1, 1, 200, 1], true, Some(64), 1.0, upper_bound),
            GpuProfileObservation::Unavailable {
                reason: GpuTimingUnavailable::InvalidTimestampRange,
                ..
            }
        ));
        assert!(
            matches!(
                decode(key, [0, 1, 0, 1], true, Some(64), 1.0, upper_bound),
                GpuProfileObservation::Sample(sample) if sample.elapsed.is_zero()
            ),
            "an available zero span is distinct from missing query data"
        );
        let stream = GpuProfileStream {
            capabilities: GpuTimestampCapabilities {
                generation: 7,
                valid_bits: Some(8),
                period_ns: 2.0,
                unavailable: None,
            },
            queue: Arc::new(Mutex::new(VecDeque::new())),
            dropped: Arc::new(AtomicU64::new(0)),
        };
        for _ in 0..300 {
            stream.publish(GpuProfileObservation::Unavailable {
                key,
                reason: GpuTimingUnavailable::ResultsUnavailable,
                raw_ticks: None,
            });
        }
        assert_eq!(stream.dropped_observations(), 44);
        {
            let _guard = stream.queue.lock().expect("telemetry fixture lock");
            assert!(
                stream.try_drain().is_none(),
                "the UI must not wait for a producer"
            );
            stream.publish(GpuProfileObservation::Unavailable {
                key,
                reason: GpuTimingUnavailable::ResultsUnavailable,
                raw_ticks: None,
            });
            assert_eq!(
                stream.dropped_observations(),
                45,
                "a producer must not wait for a reader"
            );
        }
        assert_eq!(stream.try_drain().expect("available queue").len(), CAPACITY);
        assert!(stream.try_drain().expect("consumed queue").is_empty());
    }
}
