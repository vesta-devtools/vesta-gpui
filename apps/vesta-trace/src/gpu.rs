//! Strict portable capture admission; graphics API ownership stays in the renderer.
pub mod plot;
use crate::session::*;
use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
pub use vesta_gpui::trace::gpu::{GpuCapture, GpuOutcome, GpuSubmission, GpuUnavailable};
#[derive(Debug, Default)]
pub struct Counts {
    pub samples: usize,
    pub pending: usize,
    pub unavailable: usize,
    pub mapped: usize,
    pub calibration_failed: usize,
}

fn invalid(message: &str) -> TraceError {
    TraceError::new("trace_gpu_invalid", message)
}

pub fn admit(
    session: &mut Session,
    mut capture: GpuCapture,
    cancel: &AtomicBool,
) -> Result<(), TraceError> {
    if !matches!(capture.schema_version, 1 | 2) {
        return Err(TraceError::new(
            "trace_capture_schema",
            "unsupported gpui_capture extension",
        ));
    }
    if capture.elapsed_ns > (1_u64 << 53) - 1
        || capture.process_id == 0
        || capture.submissions.len() > 1_000_000
        || capture.windows.len() > capture.submissions.len()
        || capture.clocks.len() > capture.submissions.len()
    {
        return Err(invalid(
            "capture metadata exceeds its numeric/record budget",
        ));
    }
    let process = u64::from(capture.process_id);
    if !session.events.is_empty() && !session.events.iter().any(|e| e.key.process == process) {
        return Err(invalid("GPU process has no relationship to the CPU source"));
    }
    let mut clocks = std::collections::BTreeMap::new();
    for (n, clock) in capture.clocks.iter().enumerate() {
        if n % 1024 == 0 && cancel.load(Ordering::Relaxed) {
            return Err(TraceError::new(
                "trace_cancelled",
                "GPU admission cancelled",
            ));
        }
        if clock.generation == 0
            || clocks.insert(clock.generation, clock).is_some()
            || clock.device_name.is_empty()
            || clock.device_name.len() > 256
        {
            return Err(invalid(
                "clock generations must be unique and hardware names bounded",
            ));
        }
        let bits_ok = clock
            .valid_bits
            .is_some_and(|bits| (1..=64).contains(&bits));
        let period_ok = clock.period_ns.is_some_and(|p| p.is_finite() && p > 0.0);
        let valid = match clock.unavailable {
            None => bits_ok && period_ok,
            Some(GpuUnavailable::UnsupportedQueue) => clock.valid_bits.is_none(),
            Some(GpuUnavailable::InvalidMetadata) => !bits_ok || !period_ok,
            _ => false,
        };
        if !valid {
            return Err(invalid(
                "clock capabilities contradict their unavailable state",
            ));
        }
        if capture.schema_version == 1
            && clock.calibration_support
                != vesta_gpui::trace::gpu::GpuCalibrationSupport::NotRecorded
        {
            return Err(invalid("GPU schema 1 cannot assert a calibrated clock"));
        }
    }
    let windows: BTreeSet<_> = capture.windows.iter().copied().collect();
    if windows.len() != capture.windows.len() || windows.contains(&0) {
        return Err(invalid(
            "window identities must be unique, nonzero and capture-local",
        ));
    }
    let mut pending = 0;
    let mut unavailable = 0;
    let mut mapped = 0;
    let mut calibration_failed = 0;
    for (n, record) in capture.submissions.iter().enumerate() {
        if n % 1024 == 0 && cancel.load(Ordering::Relaxed) {
            return Err(TraceError::new(
                "trace_cancelled",
                "GPU admission cancelled",
            ));
        }
        let clock = clocks
            .get(&record.generation)
            .ok_or_else(|| invalid("unknown clock generation"))?;
        if record.submission == 0
            || session
                .gpu_indices
                .insert((record.generation, record.submission), n)
                .is_some()
            || record.thread == 0
            || !windows.contains(&record.window)
            || record.recorded_cpu_ns > capture.elapsed_ns
        {
            return Err(invalid(
                "submission identity, context time or window is invalid",
            ));
        }
        session.gpu_lanes.insert(LaneKey {
            process,
            thread: u64::from(record.thread),
        });
        if let Some(id) = record.frame_id {
            let key = EventKey { process, event: id };
            let event = session
                .indices
                .get(&key)
                .map(|i| &session.events[*i])
                .ok_or_else(|| invalid("submission frame is absent"))?;
            if !event.is_frame
                || event.lane.thread != u64::from(record.thread)
                || record.recorded_cpu_ns.saturating_add(1) < event.start
                || record.recorded_cpu_ns > event.end.saturating_add(1)
            {
                return Err(invalid(
                    "submission does not belong to its recorded CPU frame/thread",
                ));
            }
        }
        match &record.outcome {
            GpuOutcome::Sample {
                raw_ticks,
                elapsed_ns,
                completion_upper_bound_ns,
            } => {
                if clock.unavailable.is_some() {
                    return Err(invalid("sample uses an unsupported clock"));
                }
                let expected = vesta_gpui::trace::gpu::timestamp_elapsed(
                    *raw_ticks,
                    clock.valid_bits.unwrap_or(0),
                    clock.period_ns.unwrap_or(0.0),
                    Duration::from_nanos(*completion_upper_bound_ns),
                )
                .map_err(|_| invalid("sample timestamp arithmetic/wrap bound is invalid"))?;
                if expected.as_nanos() != u128::from(*elapsed_ns) {
                    return Err(invalid(
                        "sample duration differs from its raw timestamp evidence",
                    ));
                }
            }
            GpuOutcome::Recording | GpuOutcome::PendingCompletion => {
                if clock.unavailable.is_some() {
                    return Err(invalid("pending query uses unsupported capabilities"));
                }
                pending += 1;
            }
            GpuOutcome::Unavailable { reason, raw_ticks } => {
                match reason {
                    GpuUnavailable::UnsupportedQueue | GpuUnavailable::InvalidMetadata => {
                        if clock.unavailable != Some(*reason) {
                            return Err(invalid("unavailability contradicts capabilities"));
                        }
                    }
                    GpuUnavailable::InvalidTimestampRange
                    | GpuUnavailable::AmbiguousCounterWrap => {
                        if raw_ticks.is_none() || clock.unavailable.is_some() {
                            return Err(invalid(
                                "failed timestamp decoding lacks its raw evidence",
                            ));
                        }
                    }
                    _ => {
                        if raw_ticks.is_some() || clock.unavailable.is_some() {
                            return Err(invalid(
                                "query/recording failure has inconsistent metadata",
                            ));
                        }
                    }
                }
                unavailable += 1;
            }
        }
        if record.calibration.is_some() && record.calibration_error.is_some()
            || capture.schema_version == 1
                && (record.calibration.is_some() || record.calibration_error.is_some())
        {
            return Err(invalid("Calibration state contradicts its schema or error"));
        }
        if let Some(calibration) = &record.calibration {
            let GpuOutcome::Sample {
                raw_ticks,
                elapsed_ns,
                ..
            } = &record.outcome
            else {
                return Err(invalid(
                    "Uncompleted submission cannot have execution calibration",
                ));
            };
            if calibration.cpu_after_ns > capture.elapsed_ns {
                return Err(invalid("Calibration extends after the frozen CPU prefix"));
            }
            let estimate = vesta_gpui::trace::gpu::map_execution(
                clock,
                *raw_ticks,
                record.recorded_cpu_ns,
                *elapsed_ns,
                calibration,
            )
            .map_err(|_| invalid("Calibration arithmetic or host/device bounds are invalid"))?;
            session
                .gpu_execution
                .insert((record.generation, record.submission), estimate);
            session.bounds.end = session.bounds.end.max(calibration.cpu_after_ns + 1);
            mapped += 1;
        }
        if record.calibration_error.is_some() {
            if !clock.calibration_support.available()
                || !matches!(record.outcome, GpuOutcome::Sample { .. })
            {
                return Err(invalid(
                    "Calibration failure has no supported completed query",
                ));
            }
            calibration_failed += 1;
        }
        session.bounds.start = session.bounds.start.min(record.recorded_cpu_ns);
        session.bounds.end = session.bounds.end.max(record.recorded_cpu_ns + 1);
    }
    session.source.capture_extension_version = Some(capture.schema_version);
    session.gpu_counts = Counts {
        samples: capture.submissions.len() - pending - unavailable,
        pending,
        unavailable,
        mapped,
        calibration_failed,
    };
    if mapped == 0 {
        session.diagnostics.push(Diagnostic {
        code: "trace_gpu_clock_uncalibrated",
        message: "Device ticks have no calibrated CPU mapping. CPU recording context is an association, not GPU execution start or presentation latency.".into(),
    });
    } else {
        session.diagnostics.push(Diagnostic{code:"trace_gpu_execution_estimates",message:format!("{mapped} GPU submissions have fresh device/QPC execution estimates; anchor uncertainty excludes extrapolation clock drift and is not a whole-interval error guarantee.")});
    }
    if pending != 0 || unavailable != 0 || capture.dropped_records != 0 {
        session.diagnostics.push(Diagnostic {
            code: "trace_gpu_coverage",
            message: format!(
                "GPU records: {pending} pending at freeze, {unavailable} unavailable, {} dropped",
                capture.dropped_records
            ),
        });
    }
    capture.clocks.sort_unstable_by_key(|c| c.generation);
    session.gpu_tracks = plot::build(&capture, &session.gpu_execution, cancel)?;
    session.gpu = Some(capture);
    Ok(())
}
pub fn matches_text(session: &Session, record: &GpuSubmission, text: &str) -> bool {
    text.is_empty()
        || record
            .frame_id
            .and_then(|event| {
                session.indices.get(&EventKey {
                    process: u64::from(session.gpu.as_ref().map_or(0, |g| g.process_id)),
                    event,
                })
            })
            .map(|i| &session.events[*i])
            .is_some_and(|e| {
                e.category.to_lowercase().contains(text) || e.name.to_lowercase().contains(text)
            })
}
