//! Shared per-frame CPU/GPU work comparison. No CPU/device clock offset is invented.
use super::*;
use crate::gpu::GpuOutcome;
use std::collections::BTreeMap;

pub(super) fn run(
    session: &Session,
    request: &Request,
    range: TimeRange,
    selection: &analysis::Selection,
    sort: FrameTimingSort,
    hash: &str,
    control: &mut Control<'_>,
) -> Result<Data, TraceError> {
    let frames = analysis::analyze_frames(session, range, selection, control)?;
    let cpu = FrameStatistics {
        count: frames.frames.len(),
        p50_ns: (!frames.frames.is_empty()).then_some(frames.percentile_ns[0]),
        p95_ns: (!frames.frames.is_empty()).then_some(frames.percentile_ns[1]),
        p99_ns: (!frames.frames.is_empty()).then_some(frames.percentile_ns[2]),
        worst: frames.max_frame.map(|(key, _)| key),
    };
    let mut by_frame = BTreeMap::new();
    let mut rows = Vec::with_capacity(frames.frames.len());
    for key in frames.frames {
        control.check(1)?;
        by_frame.insert(key, rows.len());
        rows.push((session.indices[&key], FrameGpuWork::default()));
    }
    let capture = session.gpu.as_ref();
    let mut unframed = 0;
    let dropped = capture.map_or(0, |c| c.dropped_records);
    if let Some(capture) = capture {
        let process = u64::from(capture.process_id);
        for submission in &capture.submissions {
            control.check(1)?;
            let Some(frame) = submission.frame_id else {
                unframed += 1;
                continue;
            };
            let Some(index) = by_frame.get(&EventKey {
                process,
                event: frame,
            }) else {
                continue;
            };
            let work = &mut rows[*index].1;
            work.records += 1;
            match submission.outcome {
                GpuOutcome::Recording | GpuOutcome::PendingCompletion => work.pending += 1,
                GpuOutcome::Unavailable { .. } => work.unavailable += 1,
                GpuOutcome::Sample { elapsed_ns, .. } => {
                    work.samples += 1;
                    work.sampled_work_ns = Some(
                        work.sampled_work_ns
                            .unwrap_or(0)
                            .checked_add(elapsed_ns)
                            .ok_or_else(|| {
                                TraceError::new(
                                    "trace_total_overflow",
                                    "GPU sampled work exceeds u64 nanoseconds",
                                )
                            })?,
                    );
                    let key = GpuKey {
                        process,
                        generation: submission.generation,
                        submission: submission.submission,
                    };
                    if work.longest_sample_ns.is_none_or(|old| elapsed_ns > old)
                        || work.longest_sample_ns == Some(elapsed_ns)
                            && work.longest_submission.is_some_and(|old| {
                                (key.generation, key.submission) < (old.generation, old.submission)
                            })
                    {
                        work.longest_sample_ns = Some(elapsed_ns);
                        work.longest_submission = Some(key);
                    }
                }
            }
        }
    }
    let mut statistics = FrameTimingStatistics {
        cpu,
        frames_with_gpu_records: 0,
        frames_with_gpu_samples: 0,
        frames_with_all_recorded_submissions_sampled: 0,
        gpu_samples: 0,
        gpu_pending: 0,
        gpu_unavailable: 0,
        source_dropped_gpu_records: dropped,
    };
    for (_, work) in &mut rows {
        control.check(1)?;
        work.all_recorded_submissions_sampled =
            work.records != 0 && work.samples == work.records && dropped == 0;
        statistics.frames_with_gpu_records += usize::from(work.records != 0);
        statistics.frames_with_gpu_samples += usize::from(work.samples != 0);
        statistics.frames_with_all_recorded_submissions_sampled +=
            usize::from(work.all_recorded_submissions_sampled);
        statistics.gpu_samples += work.samples;
        statistics.gpu_pending += work.pending;
        statistics.gpu_unavailable += work.unavailable;
    }
    control.check(0)?;
    rows.sort_unstable_by(|(a, ag), (b, bg)| {
        let a = &session.events[*a];
        let b = &session.events[*b];
        match sort {
            FrameTimingSort::CpuDurationDesc => (b.end - b.start).cmp(&(a.end - a.start)),
            FrameTimingSort::GpuWorkDesc => bg.sampled_work_ns.cmp(&ag.sampled_work_ns),
            FrameTimingSort::StartAsc => a.start.cmp(&b.start),
        }
        .then_with(|| a.key.cmp(&b.key))
    });
    control.check(0)?;
    let (rows, page) = page(rows, session, request, hash)?;
    let mut items = Vec::with_capacity(rows.len());
    for (index, gpu) in rows {
        control.check(1)?;
        items.push(FrameTimingRecord {
            frame: record(&session.events[index], range),
            gpu,
        });
    }
    Ok(Data::FrameTimings {
        gpu_attached: capture.is_some(),
        source_unframed_gpu_records: unframed,
        items,
        statistics,
        sort,
        page,
    })
}
