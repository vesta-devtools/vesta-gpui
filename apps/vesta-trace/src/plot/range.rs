//! One immutable range-selection tree for timeline and counter representatives.
use crate::session::{Event, EventKind, TraceError};
use std::{cmp::Ordering, sync::atomic::AtomicBool};
const NONE: u32 = u32::MAX;
#[derive(Clone, Copy, Debug)]
pub(crate) enum Rank {
    Duration,
    End,
    ValueMin,
    ValueMax,
}
#[derive(Debug)]
pub(crate) struct RangeTree {
    base: usize,
    nodes: Vec<u32>,
    rank: Rank,
}
fn value(event: &Event) -> f64 {
    if let EventKind::Counter(value) = event.kind {
        value
    } else {
        0.0
    }
}
fn choose(a: u32, b: u32, indices: &[usize], events: &[Event], rank: Rank) -> u32 {
    if a == NONE {
        return b;
    }
    if b == NONE {
        return a;
    }
    let (left, right) = (&events[indices[a as usize]], &events[indices[b as usize]]);
    let order = match rank {
        Rank::Duration => (left.end - left.start).cmp(&(right.end - right.start)),
        Rank::End => left.end.cmp(&right.end),
        Rank::ValueMax => value(left).total_cmp(&value(right)),
        Rank::ValueMin => value(right).total_cmp(&value(left)),
    }
    .then_with(|| right.key.cmp(&left.key));
    if order != Ordering::Less { a } else { b }
}
impl RangeTree {
    pub fn new(
        indices: &[usize],
        events: &[Event],
        rank: Rank,
        cancel: &AtomicBool,
    ) -> Result<Self, TraceError> {
        super::cancelled(cancel)?;
        let base = indices.len().max(1).next_power_of_two();
        let mut nodes = vec![NONE; base * 2];
        for (at, index) in indices.iter().enumerate() {
            if at % 1024 == 0 {
                super::cancelled(cancel)?;
            }
            if matches!(rank, Rank::ValueMin | Rank::ValueMax)
                && !matches!(events[*index].kind, EventKind::Counter(_))
            {
                return Err(TraceError::new(
                    "trace_counter_index_kind",
                    "counter index contains a non-counter event",
                ));
            }
            nodes[base + at] = u32::try_from(at).map_err(|_| {
                TraceError::new("trace_plot_budget", "plot row exceeds its index budget")
            })?;
        }
        for at in (1..base).rev() {
            if at % 1024 == 0 {
                super::cancelled(cancel)?;
            }
            nodes[at] = choose(nodes[at * 2], nodes[at * 2 + 1], indices, events, rank);
        }
        Ok(Self { base, nodes, rank })
    }
    pub fn query(
        &self,
        indices: &[usize],
        events: &[Event],
        mut start: usize,
        mut end: usize,
    ) -> Option<usize> {
        let mut found = NONE;
        start += self.base;
        end += self.base;
        while start < end {
            if start % 2 == 1 {
                found = choose(found, self.nodes[start], indices, events, self.rank);
                start += 1;
            }
            if end % 2 == 1 {
                end -= 1;
                found = choose(found, self.nodes[end], indices, events, self.rank);
            }
            start /= 2;
            end /= 2;
        }
        (found != NONE).then(|| indices[found as usize])
    }
    pub fn first(&self, indices: &[usize]) -> Option<usize> {
        (self.nodes[1] != NONE).then(|| indices[self.nodes[1] as usize])
    }
}
