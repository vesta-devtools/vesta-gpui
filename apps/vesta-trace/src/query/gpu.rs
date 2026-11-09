//! Source-addressed GPU evidence, not invented CPU-clock timeline intervals.
use super::*;
use crate::gpu::{GpuOutcome, GpuSubmission};

fn reason(reason: impl serde::Serialize) -> String {
    // Project the authoritative capture enum spelling, not a second code catalog.
    serde_json::to_value(reason)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".into())
}
pub(super) fn coverage(session: &Session) -> Option<GpuCoverage> {
    let capture = session.gpu.as_ref()?;
    Some(GpuCoverage {
        records: capture.submissions.len(),
        dropped_records: capture.dropped_records,
        samples: session.gpu_counts.samples,
        pending: session.gpu_counts.pending,
        unavailable: session.gpu_counts.unavailable,
        cpu_clock_mapping: session.gpu_counts.mapped != 0,
        execution_estimates: session.gpu_counts.mapped,
        calibration_failed: session.gpu_counts.calibration_failed,
        unmapped_samples: session.gpu_counts.samples - session.gpu_counts.mapped,
        drift_bound_known: false,
    })
}
fn key(process: u64, s: &GpuSubmission) -> GpuKey {
    GpuKey {
        process,
        generation: s.generation,
        submission: s.submission,
    }
}
fn row(session: &Session, process: u64, s: &GpuSubmission) -> GpuRecord {
    let (status, ticks, elapsed, bound, unavailable) = match s.outcome {
        GpuOutcome::Recording => (GpuStatus::Recording, None, None, None, None),
        GpuOutcome::PendingCompletion => (GpuStatus::PendingCompletion, None, None, None, None),
        GpuOutcome::Sample {
            raw_ticks,
            elapsed_ns,
            completion_upper_bound_ns,
        } => (
            GpuStatus::Sample,
            Some(raw_ticks),
            Some(elapsed_ns),
            Some(completion_upper_bound_ns),
            None,
        ),
        GpuOutcome::Unavailable {
            reason: why,
            raw_ticks,
        } => (
            GpuStatus::Unavailable,
            raw_ticks,
            None,
            None,
            Some(reason(why)),
        ),
    };
    GpuRecord {
        execution: session
            .gpu_execution
            .get(&(s.generation, s.submission))
            .map(|e| GpuExecutionRecord {
                estimated_start_ns: e.start_ns,
                estimated_end_ns: e.end_ns,
                anchor_cpu_ns: e.anchor_cpu_ns,
                anchor_uncertainty_ns: e.anchor_uncertainty_ns,
                anchor_distance_ns: e.anchor_distance_ns,
                drift_bound_known: false,
            }),
        calibration: s.calibration.as_ref().map(|c| GpuCalibrationRecord {
            device_ticks: c.device_ticks.to_string(),
            host_ticks: c.host_ticks.to_string(),
            host_before_ticks: c.host_before_ticks.to_string(),
            host_after_ticks: c.host_after_ticks.to_string(),
            host_frequency_hz: c.host_frequency_hz.to_string(),
            cpu_before_ns: c.cpu_before_ns,
            cpu_after_ns: c.cpu_after_ns,
            max_deviation_ns: c.max_deviation_ns,
        }),
        calibration_error: s.calibration_error.map(reason),
        key: key(process, s),
        window: s.window,
        lane: LaneKey {
            process,
            thread: u64::from(s.thread),
        },
        frame: s.frame_id.map(|event| EventKey { process, event }),
        recorded_cpu_ns: s.recorded_cpu_ns,
        status,
        raw_ticks: ticks.map(|ticks| ticks.map(|value| value.to_string())),
        elapsed_ns: elapsed,
        completion_upper_bound_ns: bound,
        unavailable,
    }
}
pub(super) fn run(
    session: &Session,
    request: &Request,
    range: TimeRange,
    hash: &str,
    control: &mut Control<'_>,
) -> Result<Data, TraceError> {
    let capture = session.gpu.as_ref();
    let process = capture.map_or(0, |c| u64::from(c.process_id));
    if matches!(request.query, Query::GpuClocks) {
        let mut items = Vec::new();
        for c in capture.map_or(&[][..], |c| c.clocks.as_slice()) {
            control.check(1)?;
            items.push(GpuClockRecord {
                calibration_support: reason(c.calibration_support),
                process,
                generation: c.generation,
                device_name: c.device_name.clone(),
                vendor_id: c.vendor_id,
                device_id: c.device_id,
                driver_version: c.driver_version,
                queue_family: c.queue_family,
                valid_bits: c.valid_bits,
                period_ns: c.period_ns,
                unavailable: c.unavailable.map(reason),
            });
        }
        items.sort_unstable_by_key(|c| c.generation);
        let (items, page) = page(items, session, request, hash)?;
        return Ok(Data::GpuClocks {
            attached: capture.is_some(),
            clock_domain: if session.gpu_counts.mapped == 0 {
                "device_timestamp_ticks_uncalibrated"
            } else {
                "device_ticks_with_qpc_execution_estimates"
            }
            .into(),
            cpu_clock_mapping: session.gpu_counts.mapped != 0,
            items,
            page,
        });
    }
    let text = request.selection.text.to_lowercase();
    let mut records = Vec::new();
    let all = capture.map_or(&[][..], |c| c.submissions.as_slice());
    let candidates = if let Query::GpuSubmission { key } = request.query {
        if key.process != process {
            &[][..]
        } else {
            session
                .gpu_indices
                .get(&(key.generation, key.submission))
                .and_then(|i| all.get(*i..*i + 1))
                .unwrap_or(&[])
        }
    } else {
        all
    };
    for s in candidates {
        control.check(1)?;
        if let Query::GpuSubmission { key: wanted } = &request.query
            && key(process, s) != *wanted
        {
            continue;
        }
        let lane = LaneKey {
            process,
            thread: u64::from(s.thread),
        };
        let frame = s.frame_id.map(|event| EventKey { process, event });
        let frame_event = frame
            .and_then(|f| session.indices.get(&f))
            .map(|i| &session.events[*i]);
        let time_matches = if matches!(request.query, Query::GpuExecution) {
            request.selection.range_ns.is_none()
                || session
                    .gpu_execution
                    .get(&(s.generation, s.submission))
                    .is_none_or(|e| {
                        let start = i128::from(e.start_ns);
                        let end = i128::from(e.end_ns);
                        if start == end {
                            i128::from(range.start) <= start && start < i128::from(range.end)
                        } else {
                            start < i128::from(range.end) && end > i128::from(range.start)
                        }
                    })
        } else {
            range.contains(s.recorded_cpu_ns)
        };
        if !time_matches
            || request.selection.lane.is_some_and(|l| l != lane)
            || request.selection.frame.is_some_and(|f| Some(f) != frame)
            || (!text.is_empty()
                && !frame_event.is_some_and(|e| {
                    e.category.to_lowercase().contains(&text)
                        || e.name.to_lowercase().contains(&text)
                }))
        {
            continue;
        }
        records.push(s);
    }
    if matches!(request.query, Query::GpuSubmission { .. }) {
        return records
            .first()
            .map(|s| Data::GpuSubmission {
                item: row(session, process, s),
            })
            .ok_or_else(|| {
                TraceError::new(
                    "trace_gpu_not_found",
                    "submission absent or outside selection",
                )
            });
    }
    if matches!(request.query, Query::GpuExecution) {
        let mut ordered = Vec::with_capacity(records.len());
        let mut matched_estimates = 0;
        for record in records {
            control.check(1)?;
            let estimate = session
                .gpu_execution
                .get(&(record.generation, record.submission));
            matched_estimates += usize::from(estimate.is_some());
            ordered.push((
                (
                    estimate.is_none(),
                    estimate.map_or(i64::MAX, |e| e.start_ns),
                    record.generation,
                    record.submission,
                ),
                record,
            ));
        }
        ordered.sort_unstable_by_key(|(rank, _)| *rank);
        let unplaced_records = ordered.len() - matched_estimates;
        control.check(0)?;
        let (items, page) = page(ordered, session, request, hash)?;
        control.check(items.len() as u64)?;
        return Ok(Data::GpuExecution {items:items.into_iter().map(|(_,s)|row(session,process,s)).collect(),page,matched_estimates,unplaced_records,selection_basis:"estimated_execution_interval_overlap_with_unplaced_records_retained; anchor_uncertainty_excludes_drift".into()});
    }
    if let Query::GpuSubmissions { sort } = request.query {
        records.sort_unstable_by(|a, b| {
            let duration = |s: &GpuSubmission| match s.outcome {
                GpuOutcome::Sample { elapsed_ns, .. } => Some(elapsed_ns),
                _ => None,
            };
            match sort {
                GpuSort::DurationDesc => duration(b).cmp(&duration(a)),
                GpuSort::CpuRecordingAsc => a.recorded_cpu_ns.cmp(&b.recorded_cpu_ns),
            }
            .then_with(|| (a.generation, a.submission).cmp(&(b.generation, b.submission)))
        });
    }
    control.check(0)?;
    let (records, page) = page(records, session, request, hash)?;
    Ok(Data::GpuSubmissions {
        items: records
            .into_iter()
            .map(|s| row(session, process, s))
            .collect(),
        page,
    })
}
