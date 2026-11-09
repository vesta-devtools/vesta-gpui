//! Budgeted Host measurement over the same geometry owner; no source text,
//! record identity, font discovery, scroll policy or background worker.
use super::*;
use vesta_gpui::TextMeasurementRevision;

/// Estimated/measured validity around one variable-height index. Old measured
/// heights remain provisional estimates after width/font/content invalidation.
/// Storage and initial construction are O(n); only requested rows are measured.
#[derive(Clone, Debug)]
pub struct LazyListMetrics {
    metrics: VariableListMetrics,
    measured: Vec<bool>,
    width: f32,
    text_revision: TextMeasurementRevision,
}

/// Outcome for the requested range, before the Host restores/recomputes its
/// current viewport. `pending` includes deferred callbacks and unattempted rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListMeasurement {
    pub attempted: usize,
    pub measured: usize,
    pub geometry_changed: bool,
    pub pending: usize,
}

impl LazyListMetrics {
    pub fn with_estimate(
        count: usize,
        estimated_height: f32,
        width: f32,
        text_revision: TextMeasurementRevision,
    ) -> Result<Self, VirtualListError> {
        Self::validate_width(width)?;
        Ok(Self {
            metrics: VariableListMetrics::filled(count, estimated_height)?,
            measured: vec![false; count],
            width,
            text_revision,
        })
    }

    fn validate_width(width: f32) -> Result<(), VirtualListError> {
        if !width.is_finite() || width <= 0.0 {
            Err(VirtualListError::InvalidGeometry)
        } else {
            Ok(())
        }
    }

    #[must_use]
    pub fn metrics(&self) -> &VariableListMetrics {
        &self.metrics
    }

    #[must_use]
    pub fn is_measured(&self, index: usize) -> Option<bool> {
        self.measured.get(index).copied()
    }

    /// Exact wrap width and the existing Core font/context revision define this
    /// measurement context. Invalid input is atomic; a no-op preserves validity.
    pub fn set_context(
        &mut self,
        width: f32,
        text_revision: TextMeasurementRevision,
    ) -> Result<bool, VirtualListError> {
        Self::validate_width(width)?;
        if self.width == width && self.text_revision == text_revision {
            return Ok(false);
        }
        self.width = width;
        self.text_revision = text_revision;
        self.measured.fill(false);
        Ok(true)
    }

    fn validate_range(&self, range: &Range<usize>) -> Result<(), VirtualListError> {
        if range.start > range.end || range.end > self.measured.len() {
            Err(VirtualListError::ItemIndexOutOfBounds)
        } else {
            Ok(())
        }
    }

    /// Mark changed Host content as provisional without moving scroll geometry
    /// before a real measurement. The caller supplies its current visual range.
    pub fn invalidate(&mut self, range: Range<usize>) -> Result<bool, VirtualListError> {
        self.validate_range(&range)?;
        let changed = self.measured[range.clone()].iter().any(|value| *value);
        self.measured[range].fill(false);
        Ok(changed)
    }

    /// Apply the Host's corresponding visual splice. Replacement rows are
    /// provisional even when their estimates equal the previous heights.
    pub fn splice(
        &mut self,
        range: Range<usize>,
        estimates: &[f32],
    ) -> Result<bool, VirtualListError> {
        self.validate_range(&range)?;
        let validity_changed = self.measured[range.clone()].iter().any(|value| *value);
        let changed = self.metrics.splice(range.clone(), estimates)?;
        self.measured
            .splice(range, std::iter::repeat_n(false, estimates.len()));
        Ok(changed || validity_changed)
    }

    /// Measure at most `budget` unknown rows in the requested current range.
    /// Choose that range from `metrics().range(...).visible_rows()` or one retained
    /// focused row. A callback returning None defers that row and spends one
    /// attempt. Invalid returned heights reject all geometry/validity changes.
    /// No call attempts more than one bounded visible-plus-overscan projection.
    pub fn measure(
        &mut self,
        range: Range<usize>,
        budget: usize,
        mut measure: impl FnMut(usize) -> Option<f32>,
    ) -> Result<ListMeasurement, VirtualListError> {
        self.validate_range(&range)?;
        if range.len() > MAX_VISIBLE_ROWS + MAX_OVERSCAN * 2 {
            return Err(VirtualListError::ViewportTooDense);
        }
        let pending = self.measured[range.clone()]
            .iter()
            .filter(|value| !**value)
            .count();
        if budget == 0 || pending == 0 {
            return Ok(ListMeasurement {
                attempted: 0,
                measured: 0,
                geometry_changed: false,
                pending,
            });
        }
        let mut heights = self.metrics.heights[range.clone()].to_vec();
        let mut ready = Vec::new();
        let mut attempted = 0;
        for index in range
            .clone()
            .filter(|index| !self.measured[*index])
            .take(budget)
        {
            attempted += 1;
            if let Some(height) = measure(index) {
                heights[index - range.start] = height;
                ready.push(index);
            }
        }
        let geometry_changed = self.metrics.splice(range.clone(), &heights)?;
        for index in &ready {
            self.measured[*index] = true;
        }
        Ok(ListMeasurement {
            attempted,
            measured: ready.len(),
            geometry_changed,
            pending: self.measured[range].iter().filter(|value| !**value).count(),
        })
    }
}

#[cfg(test)]
mod tests;
