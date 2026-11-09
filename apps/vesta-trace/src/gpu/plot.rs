//! Bounded presentation index over original calibrated submission identities.
use super::*;
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug)]
pub struct Interval {
    pub record: usize,
    pub start: u64,
    pub end: u64,
}
#[derive(Debug)]
pub struct Track {
    pub generation: u64,
    pub window: u64,
    pub intervals: Vec<Interval>,
    size: usize,
    longest: Vec<usize>,
    latest: Vec<usize>,
    ends: Vec<u64>,
}
#[derive(Clone, Copy, Debug)]
pub struct Bucket {
    pub record: usize,
    pub count: usize,
}
fn better(items: &[Interval], a: usize, b: usize, end: bool) -> usize {
    if a == usize::MAX {
        return b;
    }
    if b == usize::MAX {
        return a;
    }
    let rank = |i: usize| {
        if end {
            items[i].end
        } else {
            items[i].end - items[i].start
        }
    };
    if (rank(a), std::cmp::Reverse(items[a].record))
        >= (rank(b), std::cmp::Reverse(items[b].record))
    {
        a
    } else {
        b
    }
}
impl Track {
    fn new(
        generation: u64,
        window: u64,
        mut intervals: Vec<Interval>,
        cancel: &AtomicBool,
    ) -> Result<Self, TraceError> {
        intervals.sort_unstable_by_key(|i| (i.start, std::cmp::Reverse(i.end), i.record));
        let size = intervals.len().max(1).next_power_of_two();
        let mut longest = vec![usize::MAX; size * 2];
        let mut latest = longest.clone();
        let mut ends = Vec::with_capacity(intervals.len());
        for (n, item) in intervals.iter().enumerate() {
            if n % 1024 == 0 && cancel.load(Ordering::Relaxed) {
                return Err(TraceError::new(
                    "trace_cancelled",
                    "GPU track indexing cancelled",
                ));
            }
            longest[size + n] = n;
            latest[size + n] = n;
            ends.push(item.end * 2 + u64::from(item.start == item.end));
        }
        for n in (1..size).rev() {
            longest[n] = better(&intervals, longest[n * 2], longest[n * 2 + 1], false);
            latest[n] = better(&intervals, latest[n * 2], latest[n * 2 + 1], true);
        }
        ends.sort_unstable();
        Ok(Self {
            generation,
            window,
            intervals,
            size,
            longest,
            latest,
            ends,
        })
    }
    fn query(&self, mut start: usize, mut end: usize, latest: bool) -> Option<usize> {
        let tree = if latest { &self.latest } else { &self.longest };
        let mut best = usize::MAX;
        start += self.size;
        end += self.size;
        while start < end {
            if start % 2 == 1 {
                best = better(&self.intervals, best, tree[start], latest);
                start += 1;
            }
            if end % 2 == 1 {
                end -= 1;
                best = better(&self.intervals, best, tree[end], latest);
            }
            start /= 2;
            end /= 2;
        }
        (best != usize::MAX).then(|| self.intervals[best].record)
    }
    pub fn buckets(&self, range: TimeRange, columns: usize) -> Vec<Bucket> {
        let columns = columns.clamp(1, 4096);
        let mut output = Vec::new();
        let mut start = self.intervals.partition_point(|i| i.start < range.start);
        let ended = self.ends.partition_point(|e| *e <= range.start * 2);
        let carry = start.saturating_sub(ended);
        if carry > 0
            && let Some(record) = self.query(0, start, true)
        {
            output.push(Bucket {
                record,
                count: carry,
            });
        }
        for column in 1..=columns {
            let boundary = range.start
                + (u128::from(range.duration()) * column as u128 / columns as u128) as u64;
            let end = start + self.intervals[start..].partition_point(|i| i.start < boundary);
            if let Some(record) = self.query(start, end, false) {
                output.push(Bucket {
                    record,
                    count: end - start,
                });
            }
            start = end;
        }
        output
    }
}
pub fn build(
    capture: &GpuCapture,
    estimates: &BTreeMap<(u64, u64), vesta_gpui::trace::gpu::GpuExecutionEstimate>,
    cancel: &AtomicBool,
) -> Result<Vec<Track>, TraceError> {
    let mut groups = BTreeMap::<(u64, u64), Vec<Interval>>::new();
    for (n, record) in capture.submissions.iter().enumerate() {
        if n % 1024 == 0 && cancel.load(Ordering::Relaxed) {
            return Err(TraceError::new(
                "trace_cancelled",
                "GPU track indexing cancelled",
            ));
        }
        if let Some(e) = estimates.get(&(record.generation, record.submission))
            && e.end_ns >= 0
        {
            groups
                .entry((record.generation, record.window))
                .or_default()
                .push(Interval {
                    record: n,
                    start: e.start_ns.max(0) as u64,
                    end: e.end_ns as u64,
                });
        }
    }
    groups
        .into_iter()
        .map(|((generation, window), items)| Track::new(generation, window, items, cancel))
        .collect()
}
pub fn filtered(
    session: &Session,
    text: &str,
    cancel: &AtomicBool,
) -> Result<Vec<Track>, TraceError> {
    let Some(capture) = &session.gpu else {
        return Ok(Vec::new());
    };
    let mut output = Vec::new();
    for track in &session.gpu_tracks {
        let mut items = Vec::new();
        for (n, item) in track.intervals.iter().enumerate() {
            if n % 1024 == 0 && cancel.load(Ordering::Relaxed) {
                return Err(TraceError::new(
                    "trace_cancelled",
                    "GPU track filtering cancelled",
                ));
            }
            if super::matches_text(session, &capture.submissions[item.record], text) {
                items.push(*item);
            }
        }
        if !items.is_empty() {
            output.push(Track::new(track.generation, track.window, items, cancel)?);
        }
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn buckets_count_overlap_points_and_stable_actual_representatives() {
        let items = vec![
            Interval {
                record: 0,
                start: 50,
                end: 250,
            },
            Interval {
                record: 1,
                start: 90,
                end: 100,
            },
            Interval {
                record: 2,
                start: 100,
                end: 100,
            },
            Interval {
                record: 3,
                start: 100,
                end: 190,
            },
            Interval {
                record: 4,
                start: 200,
                end: 210,
            },
        ];
        let t = Track::new(1, 1, items, &AtomicBool::new(false)).expect("indexed GPU track");
        for start in (0..260).step_by(13) {
            for end in (start + 1..280).step_by(17) {
                let range = TimeRange { start, end };
                let expected = t
                    .intervals
                    .iter()
                    .filter(|i| {
                        if i.start == i.end {
                            range.contains(i.start)
                        } else {
                            i.start < end && i.end > start
                        }
                    })
                    .count();
                let buckets = t.buckets(range, 4);
                assert_eq!(buckets.iter().map(|b| b.count).sum::<usize>(), expected);
                assert!(
                    buckets
                        .iter()
                        .all(|b| t.intervals.iter().any(|i| i.record == b.record))
                );
            }
        }
        let b = t.buckets(
            TimeRange {
                start: 100,
                end: 200,
            },
            1,
        );
        assert_eq!(b.iter().map(|b| b.count).sum::<usize>(), 3);
        assert_eq!(b[0].record, 0);
        assert_eq!(b[1].record, 3);
    }
}
