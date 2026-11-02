//! Counter tracks retain actual raw points and extrema, never interpolated facts.
use crate::{
    plot::{RangeTree, Rank, start_bins},
    session::*,
};
use std::{collections::BTreeMap, sync::atomic::AtomicBool};
#[derive(Debug)]
pub struct Series {
    pub events: Vec<usize>,
    minimum: RangeTree,
    maximum: RangeTree,
}
pub struct Envelope {
    pub points: Vec<usize>,
    pub count: usize,
    pub min: f64,
    pub max: f64,
    pub minimum: usize,
    pub maximum: usize,
}
pub fn value(event: &Event) -> f64 {
    if let EventKind::Counter(value) = event.kind {
        value
    } else {
        0.0
    }
}
impl Series {
    pub fn for_lane(
        indices: &[usize],
        events: &[Event],
        cancel: &AtomicBool,
    ) -> Result<Vec<Self>, TraceError> {
        let mut groups = BTreeMap::<(&str, &str), Vec<usize>>::new();
        for (at, index) in indices.iter().enumerate() {
            if at % 1024 == 0 {
                crate::plot::cancelled(cancel)?;
            }
            let event = &events[*index];
            if matches!(event.kind, EventKind::Counter(_)) {
                groups
                    .entry((&event.category, &event.name))
                    .or_default()
                    .push(*index);
            }
        }
        groups
            .into_values()
            .map(|mut indices| {
                indices.sort_unstable_by_key(|i| (events[*i].start, events[*i].key));
                Self::new(indices, events, cancel)
            })
            .collect()
    }
    pub fn new(
        indices: Vec<usize>,
        events: &[Event],
        cancel: &AtomicBool,
    ) -> Result<Self, TraceError> {
        let minimum = RangeTree::new(&indices, events, Rank::ValueMin, cancel)?;
        let maximum = RangeTree::new(&indices, events, Rank::ValueMax, cancel)?;
        Ok(Self {
            events: indices,
            minimum,
            maximum,
        })
    }
    pub fn envelope(&self, events: &[Event], range: TimeRange, columns: usize) -> Option<Envelope> {
        let start = self
            .events
            .partition_point(|i| events[*i].start < range.start);
        let end = self
            .events
            .partition_point(|i| events[*i].start < range.end);
        let minimum = self.minimum.query(&self.events, events, start, end)?;
        let maximum = self.maximum.query(&self.events, events, start, end)?;
        let min = value(&events[minimum]);
        let max = value(&events[maximum]);
        let mut points = Vec::with_capacity(columns.clamp(1, 8192) * 4);
        for span in start_bins(&self.events, events, range, columns) {
            points.push(self.events[span.start]);
            points.push(self.events[span.end - 1]);
            if let Some(i) = self
                .minimum
                .query(&self.events, events, span.start, span.end)
            {
                points.push(i);
            }
            if let Some(i) = self
                .maximum
                .query(&self.events, events, span.start, span.end)
            {
                points.push(i);
            }
        }
        points.sort_unstable_by_key(|i| (events[*i].start, events[*i].key));
        points.dedup();
        Some(Envelope {
            minimum,
            maximum,
            points,
            count: end - start,
            min,
            max,
        })
    }
}
/// Normalize without overflowing opposite-sign finite extremes or losing tiny ranges.
pub fn fraction(value: f64, min: f64, max: f64) -> f32 {
    if min == max {
        return 0.5;
    }
    let half = max * 0.5 - min * 0.5;
    let result = if half == 0.0 {
        (value - min) / (max - min)
    } else {
        (value * 0.5 - min * 0.5) / half
    };
    result.clamp(0.0, 1.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn decimation_keeps_extrema_and_exact_counts_without_filling_gaps() {
        let mut input = crate::tests::fixture();
        let events=(0..1000).map(|id|json!({"pid":17,"tid":1,"ts":id*3,"ph":"C","cat":"synthetic","name":"queue","args":{"event_id":id+1,"parent_id":null,"frame_id":null,"frame":false,"value":if id==501{-999.0}else if id==505{10000.0}else{(id%9)as f64}}})).collect::<Vec<_>>();
        input["traceEvents"] = json!(events);
        input["gpui_trace"]["events"] = json!(1000);
        input["gpui_trace"]["unframed_events"] = json!(1000);
        let session = crate::import::parse(input.to_string().as_bytes(), &AtomicBool::new(false))
            .expect("counter session");
        let series = &session.lanes[0].counters[0];
        for columns in [1, 7, 1000] {
            let envelope = series
                .envelope(&session.events, session.bounds, columns)
                .expect("observations");
            assert_eq!(envelope.count, 1000);
            assert_eq!(envelope.min, -999.0);
            assert_eq!(envelope.max, 10000.0);
            assert!(envelope.points.len() <= columns * 4);
            assert!(envelope.points.contains(&501) && envelope.points.contains(&505));
            assert!(
                envelope
                    .points
                    .windows(2)
                    .all(|pair| session.events[pair[0]].start <= session.events[pair[1]].start)
            );
        }
        assert!(
            series
                .envelope(&session.events, TimeRange::new(1, 2).expect("gap"), 2)
                .is_none()
        );
        let one = series
            .envelope(
                &session.events,
                TimeRange::new(0, 1).expect("first point"),
                1,
            )
            .expect("point");
        assert_eq!(one.count, 1);
        assert_eq!(fraction(0.0, -f64::MAX, f64::MAX), 0.5);
        assert_eq!(fraction(f64::MAX, -f64::MAX, f64::MAX), 1.0);
        assert_eq!(fraction(f64::from_bits(1), 0.0, f64::from_bits(1)), 1.0);
        assert_eq!(fraction(7.0, 7.0, 7.0), 0.5);
    }
}
