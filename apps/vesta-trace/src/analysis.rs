//! Deterministic interval analysis over captured facts; no renderer or file I/O.
use crate::control::Control;
use crate::session::*;
use std::{collections::BTreeMap, sync::atomic::AtomicBool};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Timer {
    pub category: String,
    pub name: String,
    pub calls: usize,
    pub inclusive: u64,
    pub self_time: u64,
}
#[derive(Debug)]
pub struct Analysis {
    pub range: TimeRange,
    pub timers: Vec<Timer>,
    pub frames: Vec<EventKey>,
    pub max_frame: Option<(EventKey, u64)>,
    pub percentile_ns: [u64; 3],
    pub hierarchy_reliable: bool,
}

pub struct FrameAnalysis {
    pub frames: Vec<EventKey>,
    pub max_frame: Option<(EventKey, u64)>,
    pub percentile_ns: [u64; 3],
}

fn accumulate(
    timer: &mut Timer,
    session: &Session,
    index: usize,
    range: TimeRange,
    control: &mut Control<'_>,
) -> Result<(), TraceError> {
    let event = &session.events[index];
    let inclusive = range.overlap(event.start, event.end);
    let own = self_time(session, index, range, control)?;
    timer.calls += 1;
    timer.inclusive = timer.inclusive.checked_add(inclusive).ok_or_else(|| {
        TraceError::new(
            "trace_total_overflow",
            "inclusive nanosecond total exceeds u64",
        )
    })?;
    timer.self_time = timer.self_time.checked_add(own).ok_or_else(|| {
        TraceError::new("trace_total_overflow", "self nanosecond total exceeds u64")
    })?;
    Ok(())
}

#[derive(Default)]
pub struct Selection {
    pub text: String,
    pub lane: Option<LaneKey>,
    pub frame: Option<EventKey>,
}
impl Selection {
    pub fn matches(&self, event: &Event, range: TimeRange) -> bool {
        event.visible_in(range)
            && self.lane.is_none_or(|lane| event.lane == lane)
            && self.frame.is_none_or(|frame| event.frame == Some(frame))
            && (self.text.is_empty()
                || event.name.to_lowercase().contains(&self.text)
                || event.category.to_lowercase().contains(&self.text))
    }
}

pub fn self_time(
    session: &Session,
    index: usize,
    range: TimeRange,
    control: &mut Control<'_>,
) -> Result<u64, TraceError> {
    let event = &session.events[index];
    let start = event.start.max(range.start);
    let end = event.end.min(range.end);
    if start >= end {
        return Ok(0);
    }
    let mut intervals = Vec::new();
    for i in &session.children[index] {
        control.check(1)?;
        let child = &session.events[*i];
        let a = child.start.max(start);
        let b = child.end.min(end);
        if child.is_span() && a < b {
            intervals.push((a, b));
        }
    }
    control.check(0)?;
    intervals.sort_unstable();
    control.check(0)?;
    let mut covered = 0;
    let mut previous = start;
    for (a, b) in intervals {
        control.check(1)?;
        covered += b.saturating_sub(a.max(previous));
        previous = previous.max(b);
    }
    Ok((end - start).saturating_sub(covered))
}

pub fn analyze(
    session: &Session,
    range: TimeRange,
    filter: &str,
    cancel: &AtomicBool,
) -> Result<Analysis, TraceError> {
    analyze_selected(
        session,
        range,
        &Selection {
            text: filter.to_lowercase(),
            ..Selection::default()
        },
        &mut Control::new(cancel, u64::MAX, u64::MAX),
    )
}

pub fn analyze_selected(
    session: &Session,
    range: TimeRange,
    selection: &Selection,
    control: &mut Control<'_>,
) -> Result<Analysis, TraceError> {
    control.check(0)?;
    let mut timers = BTreeMap::<(&str, &str), Timer>::new();
    for (index, event) in session.events.iter().enumerate() {
        control.check(1)?;
        if !event.is_span() || !selection.matches(event, range) {
            continue;
        }
        let timer = timers
            .entry((&event.category, &event.name))
            .or_insert_with(|| Timer {
                category: event.category.clone(),
                name: event.name.clone(),
                ..Timer::default()
            });
        accumulate(timer, session, index, range, control)?;
    }
    let mut timers: Vec<_> = timers.into_values().collect();
    timers.sort_by(|a, b| {
        b.inclusive
            .cmp(&a.inclusive)
            .then_with(|| a.category.cmp(&b.category))
            .then_with(|| a.name.cmp(&b.name))
    });
    control.check(0)?;
    let frames = analyze_frames(session, range, selection, control)?;
    Ok(Analysis {
        range,
        timers,
        frames: frames.frames,
        max_frame: frames.max_frame,
        percentile_ns: frames.percentile_ns,
        hierarchy_reliable: session.hierarchy_reliable,
    })
}

pub fn analyze_frames(
    session: &Session,
    range: TimeRange,
    selection: &Selection,
    control: &mut Control<'_>,
) -> Result<FrameAnalysis, TraceError> {
    control.check(0)?;
    let mut frames = Vec::new();
    for index in &session.frames {
        control.check(1)?;
        let event = &session.events[*index];
        if selection.matches(event, range) {
            frames.push(event.key);
        }
    }
    let mut durations = Vec::with_capacity(frames.len());
    for key in &frames {
        control.check(1)?;
        let e = &session.events[session.indices[key]];
        durations.push((e.key, e.end - e.start));
    }
    let max_frame = durations
        .iter()
        .max_by_key(|(key, ns)| (*ns, std::cmp::Reverse(*key)))
        .copied();
    durations.sort_by_key(|(key, ns)| (*ns, *key));
    control.check(0)?;
    let percentile_ns = [50, 95, 99].map(|p| {
        if durations.is_empty() {
            0
        } else {
            durations[((durations.len() * p).div_ceil(100)).saturating_sub(1)].1
        }
    });
    Ok(FrameAnalysis {
        frames,
        max_frame,
        percentile_ns,
    })
}

/// Immediate captured caller/callee edges for one selected timer and time range.
/// Counts describe actual occurrences, not edges invented from overlapping time.
pub fn relations<'a>(
    session: &'a Session,
    range: TimeRange,
    category: &str,
    name: &str,
    selection: &Selection,
    control: &mut Control<'_>,
) -> Result<(Vec<Timer>, Vec<Timer>), TraceError> {
    control.check(0)?;
    let mut callers = BTreeMap::<(&'a str, &'a str), Timer>::new();
    let mut callees = BTreeMap::<(&'a str, &'a str), Timer>::new();
    let add = |table: &mut BTreeMap<(&'a str, &'a str), Timer>,
               label: usize,
               value: usize,
               control: &mut Control<'_>|
     -> Result<(), TraceError> {
        control.check(1)?;
        let e = &session.events[value];
        let owner = &session.events[label];
        if !e.is_span() || !e.visible_in(range) {
            return Ok(());
        }
        let timer = table
            .entry((&owner.category, &owner.name))
            .or_insert_with(|| Timer {
                category: owner.category.clone(),
                name: owner.name.clone(),
                ..Timer::default()
            });
        accumulate(timer, session, value, range, control)?;
        Ok(())
    };
    for (index, event) in session.events.iter().enumerate() {
        control.check(1)?;
        if event.category != category
            || event.name != name
            || !event.is_span()
            || !selection.matches(event, range)
        {
            continue;
        }
        if let Some(parent) = event.parent.and_then(|key| session.indices.get(&key)) {
            add(&mut callers, *parent, index, control)?;
        }
        for child in &session.children[index] {
            add(&mut callees, *child, *child, control)?;
        }
    }
    control.check(0)?;
    Ok((
        callers.into_values().collect(),
        callees.into_values().collect(),
    ))
}
