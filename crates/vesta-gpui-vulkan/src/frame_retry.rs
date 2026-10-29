//! Delayed redraw after an unsuccessful, zero-timeout acquire. ADR-0325.
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameRetryReason {
    ImageUnavailable,
    SwapchainOutOfDate,
    GpuCompletion,
    TargetConfiguration,
}

/// A window-local wakeup. The host schedules it without discarding UI state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameRetry {
    pub reason: FrameRetryReason,
    pub attempts: u32,
    pub deadline: Instant,
}

#[derive(Default)]
pub(crate) struct AcquireRetry(Option<FrameRetry>);

impl AcquireRetry {
    pub fn pending(&self) -> Option<FrameRetry> {
        self.0
    }

    pub fn ready(&self, now: Instant) -> bool {
        self.0.is_none_or(|retry| now >= retry.deadline)
    }

    pub fn defer(&mut self, now: Instant, reason: FrameRetryReason) {
        let attempts = self.0.map_or(1, |retry| retry.attempts.saturating_add(1));
        let delay_ms = 1u64 << attempts.saturating_sub(1).min(4);
        self.0 = Some(FrameRetry {
            reason,
            attempts,
            deadline: now + Duration::from_millis(delay_ms),
        });
    }

    pub fn reset(&mut self) {
        self.0 = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Protect the event-loop contract: input before a wakeup must neither
    // trigger another acquire nor turn an unavailable surface into a busy loop.
    #[test]
    fn acquire_retry_is_delayed_capped_and_reset_by_window_progress() {
        let mut retry = AcquireRetry::default();
        let mut now = Instant::now();
        assert!(retry.ready(now));
        for delay_ms in [1, 2, 4, 8, 16, 16, 16] {
            retry.defer(now, FrameRetryReason::ImageUnavailable);
            let pending = retry.pending().expect("retry was scheduled");
            assert_eq!(pending.deadline - now, Duration::from_millis(delay_ms));
            for _ in 0..100 {
                assert!(!retry.ready(pending.deadline - Duration::from_nanos(1)));
                assert_eq!(retry.pending(), Some(pending));
            }
            assert!(retry.ready(pending.deadline));
            now = pending.deadline;
        }
        retry.defer(now, FrameRetryReason::SwapchainOutOfDate);
        assert_eq!(retry.pending().expect("pending").attempts, 8);
        retry.defer(now, FrameRetryReason::GpuCompletion);
        assert_eq!(retry.pending().expect("pending GPU").attempts, 9);
        assert_eq!(
            retry.pending().expect("pending GPU").reason,
            FrameRetryReason::GpuCompletion
        );
        retry.reset();
        assert_eq!(retry.pending(), None);
        assert!(retry.ready(now));
        retry.defer(now, FrameRetryReason::ImageUnavailable);
        assert_eq!(retry.pending().expect("fresh episode").attempts, 1);
        retry.0.as_mut().expect("pending").attempts = u32::MAX;
        retry.defer(now, FrameRetryReason::ImageUnavailable);
        assert_eq!(retry.pending().expect("saturated").attempts, u32::MAX);
        assert_eq!(
            retry.pending().expect("capped").deadline - now,
            Duration::from_millis(16)
        );
    }
}
