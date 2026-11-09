//! Shared observed heap diagnostics. No baseline, leak or post-coverage inference.
use super::*;
use crate::memory::{Allocation, MemorySession};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn coverage(session: &Session) -> Option<MemoryCoverage> {
    session.memory.as_ref().map(|m| MemoryCoverage {
        process: u64::from(m.capture.process_id),
        provider: m.capture.provider.clone(),
        through_ns: m.capture.elapsed_ns,
        baseline_known: false,
        process_wide: false,
        recorder_allocations_excluded: true,
        event_limit_reached: m.capture.event_limit_reached,
        hook_errors: m.capture.hook_errors,
        operations: m.points.len(),
        stacks: m.stacks.len(),
        allocation_records: m.allocations.len(),
        allocations_with_stacks: m.allocations_with_stacks,
        stack_frames_requested: m.capture.stack_frames,
        potentially_truncated_stacks: m.potentially_truncated_stacks,
        stack_frame_budget: m.capture.stack_frame_budget,
        stack_observations_dropped: m.capture.stack_observations_dropped,
        symbol_budget_exhausted: m.capture.symbol_budget_exhausted,
        resolved_frames: m.resolved_frames,
        unresolved_frames: m.unresolved_frames,
        unknown_frees: m.unknown_frees,
        unknown_reallocations: m.unknown_reallocations,
    })
}
fn key(m: &MemorySession, id: u64) -> MemoryAllocationKey {
    MemoryAllocationKey {
        process: u64::from(m.capture.process_id),
        allocation: id,
    }
}
fn stack_key(m: &MemorySession, id: u64) -> MemoryStackKey {
    MemoryStackKey {
        process: u64::from(m.capture.process_id),
        stack: id,
    }
}
fn point(m: &MemorySession, time: u64) -> Result<(), TraceError> {
    if time > m.capture.elapsed_ns {
        Err(TraceError::new(
            "trace_memory_coverage",
            "Requested instant lies after known memory coverage",
        ))
    } else {
        Ok(())
    }
}
fn interval(m: &MemorySession, start: u64, end: u64) -> Result<(), TraceError> {
    point(m, end)?;
    if start >= end {
        return Err(TraceError::new(
            "trace_memory_interval",
            "Comparison requires start_ns < end_ns within known memory coverage",
        ));
    }
    Ok(())
}
fn validate_predicate(m: &MemorySession, p: MemoryPredicate) -> Result<(), TraceError> {
    match p {
        MemoryPredicate::All => Ok(()),
        MemoryPredicate::LiveAt { time_ns } => point(m, time_ns),
        MemoryPredicate::NewSurvivors { start_ns, end_ns }
        | MemoryPredicate::Released { start_ns, end_ns }
        | MemoryPredicate::Common { start_ns, end_ns }
        | MemoryPredicate::Transient { start_ns, end_ns } => interval(m, start_ns, end_ns),
    }
}
fn matches(a: &Allocation, p: MemoryPredicate) -> bool {
    match p {
        MemoryPredicate::All => true,
        MemoryPredicate::LiveAt { time_ns } => a.live_at(time_ns),
        MemoryPredicate::NewSurvivors { start_ns, end_ns } => {
            !a.live_at(start_ns) && a.live_at(end_ns)
        }
        MemoryPredicate::Released { start_ns, end_ns } => a.live_at(start_ns) && !a.live_at(end_ns),
        MemoryPredicate::Common { start_ns, end_ns } => a.live_at(start_ns) && a.live_at(end_ns),
        MemoryPredicate::Transient { start_ns, end_ns } => {
            a.allocated_ns > start_ns
                && a.allocated_ns <= end_ns
                && a.freed_ns.is_some_and(|t| t <= end_ns)
        }
    }
}
fn record(m: &MemorySession, a: &Allocation) -> MemoryAllocationRecord {
    let process = u64::from(m.capture.process_id);
    MemoryAllocationRecord {
        key: key(m, a.id),
        address: format!("0x{:x}", a.address),
        size_bytes: a.size,
        allocated_ns: a.allocated_ns,
        freed_ns: a.freed_ns,
        observed_lifetime_ns: a
            .freed_ns
            .unwrap_or(m.capture.elapsed_ns)
            .saturating_sub(a.allocated_ns),
        allocation_lane: LaneKey {
            process,
            thread: u64::from(a.thread),
        },
        free_lane: a.free_thread.map(|t| LaneKey {
            process,
            thread: u64::from(t),
        }),
        stack: a.stack.map(|id| stack_key(m, id)),
        end_operation: a.end_operation,
        reallocated_from: a.reallocated_from.map(|id| key(m, id)),
        unknown_prior_reallocation: a.unknown_prior_reallocation,
        live_at_coverage_end: a.live_at(m.capture.elapsed_ns),
    }
}
fn cohort(c: &mut MemoryCohort, m: &MemorySession, a: &Allocation) {
    c.allocations += 1;
    c.bytes += a.size;
    c.representative.get_or_insert_with(|| key(m, a.id));
}

pub(super) fn run(
    session: &Session,
    request: &Request,
    range: TimeRange,
    hash: &str,
    control: &mut Control<'_>,
) -> Result<Data, TraceError> {
    let unsupported = || {
        TraceError::new(
            "trace_selection_unsupported",
            "Memory queries use allocation lifetimes and allocation threads; this operation does not support these selection fields",
        )
    };
    let no_filter = matches!(
        request.query,
        Query::MemorySummary | Query::MemoryAllocation { .. } | Query::MemoryStack { .. }
    );
    if request.selection.frame.is_some()
        || no_filter
            && (request.selection.range_ns.is_some()
                || request.selection.lane.is_some()
                || !request.selection.text.is_empty())
        || matches!(request.query, Query::MemoryCompare { .. })
            && request.selection.range_ns.is_some()
        || matches!(request.query, Query::MemorySeries { .. })
            && (request.selection.lane.is_some() || !request.selection.text.is_empty())
    {
        return Err(unsupported());
    }
    if matches!(
        request.query,
        Query::MemorySummary
            | Query::MemoryAllocation { .. }
            | Query::MemoryStack { .. }
            | Query::MemoryCompare { .. }
    ) && request.page.cursor.is_some()
    {
        return Err(TraceError::new(
            "trace_cursor_unsupported",
            "This memory operation is not paginated",
        ));
    }
    if matches!(request.query, Query::MemorySummary) {
        return Ok(Data::MemorySummary {
            coverage: coverage(session),
            statistics: session.memory.as_ref().map(|m| MemoryStatistics {
                allocations: m.allocations.len(),
                observed_allocated_bytes: m.observed_allocated_bytes,
                observed_live_bytes_at_end: m.points.last().map_or(0, |p| p.bytes),
                observed_live_count_at_end: m.points.last().map_or(0, |p| p.count),
                peak_observed_live_bytes: m.peak_bytes,
                peak_observed_live_count: m.peak_count,
                failed_reallocations: m.realloc_failures,
            }),
        });
    }
    let m = session.memory.as_ref().ok_or_else(|| {
        TraceError::new(
            "trace_memory_not_attached",
            "This source has no recorded heap operations",
        )
    })?;
    if request.selection.range_ns.is_some() && range.end > m.capture.elapsed_ns.saturating_add(1) {
        return Err(TraceError::new(
            "trace_memory_coverage",
            "Selected range exceeds known memory coverage",
        ));
    }
    match request.query {
        Query::MemoryAllocation { key: wanted } => {
            let a = (wanted.process == u64::from(m.capture.process_id))
                .then(|| m.indices.get(&wanted.allocation))
                .flatten()
                .map(|i| &m.allocations[*i])
                .ok_or_else(|| {
                    TraceError::new(
                        "trace_memory_allocation_not_found",
                        "Allocation key is absent from this source",
                    )
                })?;
            control.check(1)?;
            return Ok(Data::MemoryAllocation { item: record(m, a) });
        }
        Query::MemoryStack { key: wanted } => {
            let s = (wanted.process == u64::from(m.capture.process_id))
                .then(|| m.stacks.get(&wanted.stack))
                .flatten()
                .map(|i| &m.capture.stacks[*i])
                .ok_or_else(|| {
                    TraceError::new(
                        "trace_memory_stack_not_found",
                        "Stack key is absent from this source",
                    )
                })?;
            control.check(s.frames.len() as u64)?;
            return Ok(Data::MemoryStack {
                item: MemoryStackRecord {
                    key: wanted,
                    potentially_truncated: s.potentially_truncated,
                    frames: s
                        .frames
                        .iter()
                        .map(|f| MemoryStackFrameRecord {
                            address: f.address.clone(),
                            function: f.function.clone(),
                            file: f.file.clone(),
                            line: f.line,
                        })
                        .collect(),
                },
            });
        }
        Query::MemorySeries { columns } => {
            return series(session, m, request, range, hash, columns, control);
        }
        _ => {}
    }
    let predicate = match request.query {
        Query::MemoryAllocations { lifetime, .. } | Query::MemoryStackGroups { lifetime } => {
            lifetime
        }
        _ => MemoryPredicate::All,
    };
    validate_predicate(m, predicate)?;
    let wanted_stack = match request.query {
        Query::MemoryAllocations { stack, .. } => stack,
        _ => None,
    };
    if wanted_stack.is_some_and(|s| {
        s.process != u64::from(m.capture.process_id) || !m.stacks.contains_key(&s.stack)
    }) {
        return Err(TraceError::new(
            "trace_memory_stack_not_found",
            "Stack filter is absent from this source",
        ));
    }
    let text = request.selection.text.to_lowercase();
    let mut matching_stacks = BTreeSet::new();
    if !text.is_empty() {
        for s in &m.capture.stacks {
            let mut matched = false;
            for f in &s.frames {
                control.check(1)?;
                matched |= [
                    &f.address,
                    f.function.as_ref().unwrap_or(&f.address),
                    f.file.as_ref().unwrap_or(&f.address),
                ]
                .iter()
                .any(|s| s.to_lowercase().contains(&text));
            }
            if matched {
                matching_stacks.insert(s.id);
            }
        }
    }
    let mut indices = Vec::new();
    for (i, a) in m.allocations.iter().enumerate() {
        control.check(1)?;
        if a.overlaps(range)
            && matches(a, predicate)
            && wanted_stack.is_none_or(|s| a.stack == Some(s.stack))
            && request.selection.lane.is_none_or(|l| {
                l.process == u64::from(m.capture.process_id) && l.thread == u64::from(a.thread)
            })
            && (text.is_empty()
                || a.stack.is_some_and(|s| matching_stacks.contains(&s))
                || format!("0x{:x}", a.address).contains(&text))
        {
            indices.push(i);
        }
    }
    match request.query {
        Query::MemoryAllocations { sort, .. } => {
            indices.sort_unstable_by_key(|i| {
                let a = &m.allocations[*i];
                match sort {
                    MemorySort::SizeDesc => (std::cmp::Reverse(a.size), a.id),
                    MemorySort::AllocatedAsc => {
                        (std::cmp::Reverse(u64::MAX - a.allocated_ns), a.id)
                    }
                    MemorySort::LifetimeDesc => (
                        std::cmp::Reverse(
                            a.freed_ns
                                .unwrap_or(m.capture.elapsed_ns)
                                .saturating_sub(a.allocated_ns),
                        ),
                        a.id,
                    ),
                }
            });
            control.check(0)?;
            let (indices, page) = page(indices, session, request, hash)?;
            control.check(indices.len() as u64)?;
            Ok(Data::MemoryAllocations {
                items: indices
                    .into_iter()
                    .map(|i| record(m, &m.allocations[i]))
                    .collect(),
                page,
            })
        }
        Query::MemoryStackGroups { .. } => {
            let mut groups = BTreeMap::<Option<u64>, MemoryStackGroup>::new();
            for i in indices {
                control.check(1)?;
                let a = &m.allocations[i];
                let group = groups.entry(a.stack).or_insert_with(|| MemoryStackGroup {
                    stack: a.stack.map(|s| stack_key(m, s)),
                    allocations: 0,
                    allocated_bytes: 0,
                    observed_live_bytes_at_end: 0,
                    representative: key(m, a.id),
                });
                group.allocations += 1;
                group.allocated_bytes += a.size;
                if a.live_at(m.capture.elapsed_ns) {
                    group.observed_live_bytes_at_end += a.size;
                }
            }
            let mut groups: Vec<_> = groups.into_values().collect();
            groups.sort_unstable_by_key(|g| (std::cmp::Reverse(g.allocated_bytes), g.stack));
            control.check(0)?;
            let (items, page) = page(groups, session, request, hash)?;
            Ok(Data::MemoryStackGroups { items, page })
        }
        Query::MemoryCompare { start_ns, end_ns } => {
            interval(m, start_ns, end_ns)?;
            let mut c = MemoryComparison {
                start_ns,
                end_ns,
                observed_live_at_start: MemoryCohort::default(),
                observed_live_at_end: MemoryCohort::default(),
                common: MemoryCohort::default(),
                new_survivors: MemoryCohort::default(),
                released: MemoryCohort::default(),
                transient: MemoryCohort::default(),
                observed_change_bytes: String::new(),
            };
            for i in indices {
                control.check(1)?;
                let a = &m.allocations[i];
                let start = a.live_at(start_ns);
                let end = a.live_at(end_ns);
                if start {
                    cohort(&mut c.observed_live_at_start, m, a);
                }
                if end {
                    cohort(&mut c.observed_live_at_end, m, a);
                }
                if start && end {
                    cohort(&mut c.common, m, a);
                } else if start {
                    cohort(&mut c.released, m, a);
                } else if end {
                    cohort(&mut c.new_survivors, m, a);
                } else if matches(a, MemoryPredicate::Transient { start_ns, end_ns }) {
                    cohort(&mut c.transient, m, a);
                }
            }
            c.observed_change_bytes = (i128::from(c.observed_live_at_end.bytes)
                - i128::from(c.observed_live_at_start.bytes))
            .to_string();
            Ok(Data::MemoryCompare { comparison: c })
        }
        _ => unreachable!("memory query dispatch"),
    }
}
fn series(
    session: &Session,
    m: &MemorySession,
    request: &Request,
    range: TimeRange,
    hash: &str,
    columns: usize,
    control: &mut Control<'_>,
) -> Result<Data, TraceError> {
    if !(1..=256).contains(&columns) {
        return Err(TraceError::new(
            "trace_memory_columns",
            "Memory series columns must be 1..256",
        ));
    }
    // Actual post-operation observations only. Six representatives per bin:
    // first, last, min/max bytes and min/max count, then original operation order.
    let mut bins = vec![[None::<usize>; 6]; columns];
    let mut observations = 0;
    let duration = range.duration();
    for (i, p) in m.points.iter().enumerate() {
        control.check(1)?;
        if !range.contains(p.time_ns) {
            continue;
        }
        observations += 1;
        let bin = ((u128::from(p.time_ns - range.start) * columns as u128 / u128::from(duration))
            as usize)
            .min(columns - 1);
        let b = &mut bins[bin];
        b[0].get_or_insert(i);
        b[1] = Some(i);
        for (slot, maximum, bytes) in [
            (2, false, true),
            (3, true, true),
            (4, false, false),
            (5, true, false),
        ] {
            let value = |n: usize| {
                if bytes {
                    m.points[n].bytes
                } else {
                    m.points[n].count as u64
                }
            };
            if b[slot].is_none_or(|n| {
                if maximum {
                    value(i) > value(n)
                } else {
                    value(i) < value(n)
                }
            }) {
                b[slot] = Some(i);
            }
        }
    }
    let indices: BTreeSet<_> = bins.into_iter().flatten().flatten().collect();
    let selected = indices.len();
    let (indices, page) = page(indices.into_iter().collect(), session, request, hash)?;
    control.check(indices.len() as u64)?;
    Ok(Data::MemorySeries {
        columns,
        observations,
        omitted_observations: observations - selected,
        page,
        items: indices
            .into_iter()
            .map(|i| {
                let p = m.points[i];
                MemoryPoint {
                    operation: p.operation,
                    time_ns: p.time_ns,
                    observed_live_bytes: p.bytes,
                    observed_live_count: p.count,
                }
            })
            .collect(),
    })
}
