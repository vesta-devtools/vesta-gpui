//! Shared cooperative analysis control; no timer thread or hidden executor.
use crate::session::TraceError;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub struct Control<'a> {
    cancel: &'a AtomicBool,
    started: Instant,
    deadline: Duration,
    max_work: u64,
    pub work: u64,
}
impl<'a> Control<'a> {
    pub fn cancellation(&self) -> &AtomicBool {
        self.cancel
    }
    pub fn new(cancel: &'a AtomicBool, max_work: u64, deadline_ms: u64) -> Self {
        Self {
            cancel,
            started: Instant::now(),
            deadline: Duration::from_millis(deadline_ms),
            max_work,
            work: 0,
        }
    }
    pub fn elapsed_us(&self) -> u64 {
        self.started.elapsed().as_micros().min(u64::MAX as u128) as u64
    }
    pub fn check(&mut self, work: u64) -> Result<(), TraceError> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(TraceError::new("trace_cancelled", "analysis cancelled"));
        }
        self.work = self.work.saturating_add(work);
        if self.work > self.max_work {
            return Err(TraceError::new(
                "trace_work_budget",
                "query exceeded its event/child work budget; narrow the query or raise its budget",
            ));
        }
        if self.started.elapsed() >= self.deadline {
            return Err(TraceError::new(
                "trace_deadline",
                "query exceeded its cooperative deadline; no partial totals were returned",
            ));
        }
        Ok(())
    }
}
