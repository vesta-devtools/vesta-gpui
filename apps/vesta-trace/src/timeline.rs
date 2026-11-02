//! Navigation in the capture's integer nanosecond domain.
use crate::session::{EventKey, TimeRange};

#[derive(Clone, Debug)]
pub struct Timeline {
    pub full: TimeRange,
    pub view: TimeRange,
    pub selection: Option<TimeRange>,
    pub selected: Option<EventKey>,
    pub scroll: f32,
}
impl Timeline {
    pub fn new(full: TimeRange) -> Self {
        Self {
            full,
            view: full,
            selection: None,
            selected: None,
            scroll: 0.0,
        }
    }
    pub fn at(&self, fraction: f64) -> u64 {
        self.view.start + (fraction.clamp(0.0, 1.0) * self.view.duration() as f64).round() as u64
    }
    pub fn fraction(&self, time: u64) -> f64 {
        (time as f64 - self.view.start as f64) / self.view.duration() as f64
    }
    pub fn zoom(&mut self, anchor: f64, factor: f64) {
        if !factor.is_finite() || factor <= 0.0 || !anchor.is_finite() {
            return;
        }
        let anchor = anchor.clamp(0.0, 1.0);
        let span = (self.view.duration() as f64 * factor)
            .round()
            .clamp(1.0, self.full.duration() as f64) as u64;
        let start = (self.at(anchor) as f64 - span as f64 * anchor)
            .round()
            .clamp(self.full.start as f64, (self.full.end - span) as f64)
            as u64;
        self.view = TimeRange {
            start,
            end: start + span,
        };
    }
    pub fn pan(&mut self, fraction: f64) {
        if !fraction.is_finite() {
            return;
        }
        let span = self.view.duration();
        let start = (self.view.start as f64 + span as f64 * fraction)
            .round()
            .clamp(self.full.start as f64, (self.full.end - span) as f64)
            as u64;
        self.view = TimeRange {
            start,
            end: start + span,
        };
    }
    pub fn select(&mut self, start: u64, end: u64) {
        self.selection = TimeRange::new(start.min(end), start.max(end));
    }
    pub fn fit(&mut self) {
        self.view = self.full;
    }
    pub fn analysis_range(&self) -> TimeRange {
        self.selection.unwrap_or(self.view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_preserves_anchor_limits_and_reversed_selection() {
        let mut timeline = Timeline::new(TimeRange::new(1000, 100_000).expect("full"));
        let anchor = timeline.at(0.25);
        timeline.zoom(0.25, 0.25);
        assert!(timeline.at(0.25).abs_diff(anchor) <= 1);
        timeline.pan(-1e12);
        assert_eq!(timeline.view.start, 1000);
        timeline.pan(1e12);
        assert_eq!(timeline.view.end, 100_000);
        timeline.zoom(0.5, 1e-20);
        assert_eq!(timeline.view.duration(), 1);
        timeline.select(30_000, 20_000);
        assert_eq!(
            timeline.analysis_range(),
            TimeRange::new(20_000, 30_000).expect("selection")
        );
        timeline.select(0, 0);
        timeline.fit();
        assert_eq!(timeline.analysis_range(), timeline.full);
    }
}
