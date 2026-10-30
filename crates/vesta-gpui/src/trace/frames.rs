//! One analysis authority for typed inspection and the exported offline report.
use super::*;

#[derive(Clone, Debug)]
pub struct FrameCall {
    pub event_id: u64,
    pub parent_id: Option<u64>,
    pub thread: u32,
    pub category: &'static str,
    pub name: &'static str,
    pub start: Duration,
    pub inclusive: Duration,
    /// Elapsed wall time outside recorded direct children on the same thread.
    /// Includes waits and uninstrumented work; not exclusive CPU utilization.
    pub self_time: Duration,
    pub unfinished: bool,
}

#[derive(Clone, Debug)]
pub struct FrameMarker {
    pub name: &'static str,
    pub at: Duration,
    pub value: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct FrameTimer {
    pub category: &'static str,
    pub name: &'static str,
    pub calls: usize,
    pub inclusive: Duration,
    pub self_time: Duration,
}

/// A recorded CPU frame attempt. Completeness concerns captured scopes, not
/// GPU execution or physical presentation. Capture losses apply to every view.
#[derive(Clone, Debug)]
pub struct FrameProfile {
    pub ordinal: usize,
    pub frame_id: u64,
    pub name: &'static str,
    pub thread: u32,
    pub start: Duration,
    pub duration: Duration,
    pub unfinished: bool,
    pub calls: Vec<FrameCall>,
    pub timers: Vec<FrameTimer>,
    pub markers: Vec<FrameMarker>,
}

fn end_of(event: &Event, fallback: Duration) -> Duration {
    match event.kind {
        EventKind::Span { end, .. } => end.unwrap_or(fallback).max(event.at),
        _ => event.at,
    }
}

fn covered_time(mut ranges: Vec<(Duration, Duration)>) -> Duration {
    ranges.sort_unstable_by_key(|range| range.0);
    let mut covered = Duration::ZERO;
    let mut end = Duration::ZERO;
    for (start, stop) in ranges {
        let start = start.max(end);
        covered = covered.saturating_add(stop.saturating_sub(start));
        end = end.max(stop);
    }
    covered
}

impl TraceSnapshot {
    /// Build frame profiles from recorded parent/frame identities. No inference
    /// from coincident timestamps, no symbol sampling and no file I/O.
    pub fn frames(&self) -> Vec<FrameProfile> {
        let mut children: HashMap<usize, Vec<(Duration, Duration)>> = HashMap::new();
        for event in &self.events {
            let Some(parent_index) = event.parent else {
                continue;
            };
            let Some(parent) = self.events.get(parent_index) else {
                continue;
            };
            if event.thread != parent.thread
                || event.frame != parent.frame
                || !matches!(event.kind, EventKind::Span { .. })
            {
                continue;
            }
            let start = event.at.max(parent.at);
            let end = end_of(event, self.stats.elapsed).min(end_of(parent, self.stats.elapsed));
            if end > start {
                children.entry(parent_index).or_default().push((start, end));
            }
        }
        let mut frames = Vec::new();
        let mut by_index = HashMap::new();
        for (index, event) in self.events.iter().enumerate().filter(|(_, e)| e.is_frame) {
            let EventKind::Span { unfinished, .. } = event.kind else {
                continue;
            };
            by_index.insert(index, frames.len());
            frames.push(FrameProfile {
                ordinal: frames.len() + 1,
                frame_id: index as u64 + 1,
                name: event.name,
                thread: event.thread,
                start: event.at,
                duration: end_of(event, self.stats.elapsed).saturating_sub(event.at),
                unfinished,
                calls: Vec::new(),
                timers: Vec::new(),
                markers: Vec::new(),
            });
        }
        for (index, event) in self.events.iter().enumerate() {
            let Some(frame_index) = event.frame.and_then(|id| by_index.get(&id)).copied() else {
                continue;
            };
            let frame = &mut frames[frame_index];
            match event.kind {
                EventKind::Span { unfinished, .. } => {
                    let inclusive = end_of(event, self.stats.elapsed).saturating_sub(event.at);
                    let self_time = inclusive
                        .saturating_sub(covered_time(children.remove(&index).unwrap_or_default()));
                    frame.calls.push(FrameCall {
                        event_id: index as u64 + 1,
                        parent_id: event
                            .parent
                            .filter(|parent| self.events[*parent].frame == event.frame)
                            .map(|id| id as u64 + 1),
                        thread: event.thread,
                        category: event.category,
                        name: event.name,
                        start: event.at,
                        inclusive,
                        self_time,
                        unfinished,
                    });
                }
                EventKind::Instant => frame.markers.push(FrameMarker {
                    name: event.name,
                    at: event.at,
                    value: None,
                }),
                EventKind::Counter(value) => frame.markers.push(FrameMarker {
                    name: event.name,
                    at: event.at,
                    value: Some(value),
                }),
            }
        }
        for frame in &mut frames {
            let mut timers: HashMap<(&'static str, &'static str), FrameTimer> = HashMap::new();
            for call in &frame.calls {
                let timer = timers
                    .entry((call.category, call.name))
                    .or_insert(FrameTimer {
                        category: call.category,
                        name: call.name,
                        calls: 0,
                        inclusive: Duration::ZERO,
                        self_time: Duration::ZERO,
                    });
                timer.calls += 1;
                timer.inclusive = timer.inclusive.saturating_add(call.inclusive);
                timer.self_time = timer.self_time.saturating_add(call.self_time);
            }
            frame.timers = timers.into_values().collect();
            frame.timers.sort_unstable_by(|a, b| {
                b.inclusive
                    .cmp(&a.inclusive)
                    .then_with(|| (a.category, a.name).cmp(&(b.category, b.name)))
            });
        }
        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_accounting_clips_and_unions_children_and_preserves_unframed_work() {
        let ms = Duration::from_millis;
        assert_eq!(
            covered_time(vec![(ms(1), ms(6)), (ms(4), ms(8)), (ms(2), ms(3))]),
            ms(7)
        );
        assert_eq!(covered_time(vec![(ms(1), ms(2)), (ms(5), ms(8))]), ms(4));
        let event = |name, start, end, parent, frame, is_frame| Event {
            category: "contract",
            name,
            thread: 1,
            at: ms(start),
            kind: EventKind::Span {
                end: Some(ms(end)),
                unfinished: false,
            },
            parent,
            frame,
            is_frame,
        };
        let snapshot = TraceSnapshot {
            process_id: 1,
            config: TraceConfig::default(),
            detail: TraceDetail::Detailed,
            stats: TraceStats {
                events: 5,
                dropped_events: 0,
                unfinished_spans: 0,
                elapsed: ms(15),
                expired: false,
                invalid_nesting: 1,
            },
            events: vec![
                event("Frame", 0, 10, None, Some(0), true),
                event("Work", 1, 6, Some(0), Some(0), false),
                event("Work", 4, 12, Some(0), Some(0), false),
                event("Nested", 2, 3, Some(1), Some(0), false),
                event("Outside", 0, 15, None, None, false),
            ],
        };
        let frames = snapshot.frames();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].duration, ms(10));
        assert_eq!(frames[0].calls.len(), 4);
        assert_eq!(
            frames[0].calls[0].self_time,
            ms(1),
            "overlap is counted once and clipped at frame end"
        );
        assert_eq!(frames[0].calls[1].self_time, ms(4));
        let work = frames[0]
            .timers
            .iter()
            .find(|timer| timer.name == "Work")
            .expect("aggregated timer");
        assert_eq!(
            (work.calls, work.inclusive, work.self_time),
            (2, ms(13), ms(12))
        );
    }
}
