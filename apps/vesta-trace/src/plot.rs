//! Immutable presentation indexes over original event identities, never analysis facts.
mod range;
pub(crate) use range::{RangeTree, Rank};
#[cfg(test)]
mod tests;
use crate::session::{Event, EventRow, Session, TimeRange, TraceError};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[derive(Debug)]
pub struct PlotIndex {
    longest: RangeTree,
    latest: RangeTree,
    // end*2 + point keeps half-open positive spans and boundary points distinct.
    ends: Vec<u64>,
}
#[derive(Clone, Copy, Debug)]
pub struct Bucket {
    pub index: usize,
    pub count: usize,
}
pub(crate) fn cancelled(cancel: &AtomicBool) -> Result<(), TraceError> {
    if cancel.load(Ordering::Relaxed) {
        Err(TraceError::new(
            "trace_cancelled",
            "plot indexing cancelled",
        ))
    } else {
        Ok(())
    }
}
pub(crate) fn start_bins<'a>(
    indices: &'a [usize],
    events: &'a [Event],
    range: TimeRange,
    columns: usize,
) -> impl Iterator<Item = std::ops::Range<usize>> + 'a {
    let columns = columns.clamp(1, 8192);
    let mut start = indices.partition_point(|i| events[*i].start < range.start);
    (1..=columns).filter_map(move |column| {
        let boundary =
            range.start + (u128::from(range.duration()) * column as u128 / columns as u128) as u64;
        let end = start + indices[start..].partition_point(|i| events[*i].start < boundary);
        let span = start..end;
        start = end;
        (!span.is_empty()).then_some(span)
    })
}
impl PlotIndex {
    pub fn new(
        indices: &[usize],
        events: &[Event],
        cancel: &AtomicBool,
    ) -> Result<Self, TraceError> {
        let longest = RangeTree::new(indices, events, Rank::Duration, cancel)?;
        let latest = RangeTree::new(indices, events, Rank::End, cancel)?;
        let mut ends = Vec::with_capacity(indices.len());
        for (at, i) in indices.iter().enumerate() {
            if at % 1024 == 0 {
                cancelled(cancel)?;
            }
            let e = &events[*i];
            ends.push(e.end * 2 + u64::from(e.start == e.end));
        }
        ends.sort_unstable();
        cancelled(cancel)?;
        Ok(Self {
            longest,
            latest,
            ends,
        })
    }
    pub fn longest(&self, indices: &[usize]) -> Option<usize> {
        self.longest.first(indices)
    }
    /// Exact visible counts and stable actual representatives, O(columns*log(row)).
    pub fn buckets(
        &self,
        indices: &[usize],
        events: &[Event],
        range: TimeRange,
        columns: usize,
    ) -> Vec<Bucket> {
        if indices.is_empty() {
            return Vec::new();
        }
        let mut result = Vec::with_capacity(columns.clamp(1, 8192) + 1);
        let start = indices.partition_point(|i| events[*i].start < range.start);
        let ended = self
            .ends
            .partition_point(|end| *end <= range.start.saturating_mul(2));
        let carry = start.saturating_sub(ended);
        if carry > 0
            && let Some(index) = self.latest.query(indices, events, 0, start)
        {
            result.push(Bucket {
                index,
                count: carry,
            });
        }
        for span in start_bins(indices, events, range, columns) {
            if let Some(index) = self.longest.query(indices, events, span.start, span.end) {
                result.push(Bucket {
                    index,
                    count: span.len(),
                });
            }
        }
        result
    }
}

#[derive(Debug)]
pub struct FilteredTracks {
    pub gpu: Vec<crate::gpu::plot::Track>,
    pub source_sha256: String,
    pub text: String,
    pub lanes: Vec<Vec<EventRow>>,
    pub counters: Vec<Vec<usize>>,
}
impl FilteredTracks {
    pub fn prepare(
        session: &Session,
        text: &str,
        cached: Option<Arc<Self>>,
        cancel: &AtomicBool,
    ) -> Result<Option<Arc<Self>>, TraceError> {
        cancelled(cancel)?;
        if text.is_empty() {
            return Ok(None);
        }
        let text = text.to_lowercase();
        if let Some(cached) = cached
            && cached.source_sha256 == session.source.sha256
            && cached.text == text
        {
            return Ok(Some(cached));
        }
        let selection = crate::analysis::Selection {
            text: text.clone(),
            ..Default::default()
        };
        let mut lanes = Vec::with_capacity(session.lanes.len());
        let mut counters = Vec::with_capacity(session.lanes.len());
        let mut work = 0;
        for lane in &session.lanes {
            counters.push(
                lane.counters
                    .iter()
                    .enumerate()
                    .filter_map(|(index, series)| {
                        selection
                            .matches(&session.events[series.events[0]], session.bounds)
                            .then_some(index)
                    })
                    .collect(),
            );
            let mut rows = Vec::with_capacity(lane.rows.len());
            for row in &lane.rows {
                let mut indices = Vec::new();
                for index in &row.events {
                    work += 1;
                    if work % 1024 == 0 {
                        cancelled(cancel)?;
                    }
                    if selection.matches(&session.events[*index], session.bounds) {
                        indices.push(*index);
                    }
                }
                rows.push(EventRow::indexed(
                    row.depth,
                    indices,
                    &session.events,
                    cancel,
                )?);
            }
            lanes.push(rows);
        }
        cancelled(cancel)?;
        Ok(Some(Arc::new(Self {
            gpu: crate::gpu::plot::filtered(session, &text, cancel)?,
            counters,
            source_sha256: session.source.sha256.clone(),
            text,
            lanes,
        })))
    }
}
