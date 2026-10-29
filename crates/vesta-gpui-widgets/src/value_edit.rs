//! Host-owned drafts and pointer edit transactions (ADR-0306).

use vesta_gpui::{InputTarget, UiWindowId};

#[derive(Clone, Debug, PartialEq)]
pub enum DraftResolution<T> {
    Accepted { value: T, changed: bool },
    Rejected { restore: T },
}

/// Parse and validate through the caller's typed contract. Invalid input restores the
/// current model value; accepting the same value must not create a history entry.
#[must_use]
pub fn resolve_value_draft<T: Clone + PartialEq>(
    draft: &str,
    current: &T,
    parse: impl FnOnce(&str) -> Option<T>,
) -> DraftResolution<T> {
    parse(draft).map_or_else(
        || DraftResolution::Rejected {
            restore: current.clone(),
        },
        |value| DraftResolution::Accepted {
            changed: value != *current,
            value,
        },
    )
}

#[derive(Clone, Debug, PartialEq)]
pub enum PointerEditOutcome<T> {
    Click,
    Unchanged,
    Commit { before: T, after: T },
    Cancel { restore: T },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerEditError {
    InvalidGeometry,
    InvalidThreshold,
}

/// One preview/commit/cancel cycle. The host keeps the preview and typed validation;
/// this session keeps only the stable owner and the immutable value to restore.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ValueEditSession<T> {
    target: InputTarget,
    initial: T,
    ended: bool,
}

impl<T> ValueEditSession<T> {
    #[must_use]
    pub const fn new(target: InputTarget, initial: T) -> Self {
        Self {
            target,
            initial,
            ended: false,
        }
    }

    #[must_use]
    pub const fn target(&self) -> InputTarget {
        self.target
    }

    #[must_use]
    pub const fn initial(&self) -> &T {
        &self.initial
    }

    #[must_use]
    pub fn accepts(&self, source: UiWindowId) -> bool {
        !self.ended && source == self.target.window
    }
}

impl<T: Clone + PartialEq> ValueEditSession<T> {
    /// A foreign or repeated terminal event cannot complete this session.
    pub fn finish(
        &mut self,
        source: UiWindowId,
        current: &T,
        commit: bool,
    ) -> Option<PointerEditOutcome<T>> {
        if !self.accepts(source) {
            return None;
        }
        self.ended = true;
        Some(if !commit {
            PointerEditOutcome::Cancel {
                restore: self.initial.clone(),
            }
        } else if *current == self.initial {
            PointerEditOutcome::Unchanged
        } else {
            PointerEditOutcome::Commit {
                before: self.initial.clone(),
                after: current.clone(),
            }
        })
    }
}

/// One horizontal edit with an immutable start snapshot and a window-qualified target.
/// Hosts own pointer capture, geometry conversion, value arithmetic and model updates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HorizontalEditSession<T> {
    session: ValueEditSession<T>,
    origin_x: f32,
    threshold: f32,
    dragging: bool,
}

impl<T> HorizontalEditSession<T> {
    pub fn new(
        target: InputTarget,
        origin_x: f32,
        initial: T,
        threshold: f32,
    ) -> Result<Self, PointerEditError> {
        if !origin_x.is_finite() {
            return Err(PointerEditError::InvalidGeometry);
        }
        if !threshold.is_finite() || threshold < 0.0 {
            return Err(PointerEditError::InvalidThreshold);
        }
        Ok(Self {
            session: ValueEditSession::new(target, initial),
            origin_x,
            threshold,
            dragging: threshold == 0.0,
        })
    }

    #[must_use]
    pub const fn target(&self) -> InputTarget {
        self.session.target()
    }

    #[must_use]
    pub const fn initial(&self) -> &T {
        self.session.initial()
    }

    #[must_use]
    pub fn is_dragging(&self) -> bool {
        self.dragging && self.session.accepts(self.session.target().window)
    }

    /// Foreign windows and non-finite coordinates cannot advance the gesture.
    pub fn offset(&mut self, source: UiWindowId, pointer_x: f32) -> Option<f32> {
        if !self.session.accepts(source) || !pointer_x.is_finite() {
            return None;
        }
        let delta = pointer_x - self.origin_x;
        if !delta.is_finite() {
            return None;
        }
        if delta.abs() >= self.threshold {
            self.dragging = true;
        }
        self.dragging.then_some(delta)
    }
}

impl<T: Clone + PartialEq> HorizontalEditSession<T> {
    /// Emits at most one terminal intent. Cancellation never activates a pending click;
    /// a foreign release leaves the session intact.
    pub fn finish(
        &mut self,
        source: UiWindowId,
        current: &T,
        commit: bool,
    ) -> Option<PointerEditOutcome<T>> {
        self.session.finish(source, current, commit).map(|outcome| {
            if self.dragging || !commit {
                outcome
            } else {
                PointerEditOutcome::Click
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vesta_gpui::ElementId;

    #[test]
    fn edits_isolate_windows_and_emit_one_terminal_intent_with_the_original_snapshot() {
        let owner = InputTarget {
            window: UiWindowId(1),
            element: ElementId::new(10),
        };
        let mut session =
            HorizontalEditSession::new(owner, 10.0, Some(2.0_f32), 4.0).expect("finite gesture");
        assert_eq!(session.offset(UiWindowId(2), 40.0), None);
        assert_eq!(session.offset(owner.window, 12.0), None);
        assert_eq!(session.offset(owner.window, f32::NAN), None);
        assert_eq!(session.offset(owner.window, 18.0), Some(8.0));
        assert_eq!(session.finish(UiWindowId(2), &Some(3.0), false), None);
        assert_eq!(
            session.finish(owner.window, &Some(3.0), false),
            Some(PointerEditOutcome::Cancel { restore: Some(2.0) })
        );
        assert_eq!(session.finish(owner.window, &Some(3.0), true), None);
        assert_eq!(session.offset(owner.window, 30.0), None);
        let mut canceled_click =
            HorizontalEditSession::new(owner, 10.0, 2_i32, 4.0).expect("pending click");
        assert_eq!(canceled_click.offset(owner.window, 12.0), None);
        assert_eq!(canceled_click.finish(UiWindowId(2), &2, false), None);
        assert_eq!(
            canceled_click.finish(owner.window, &2, false),
            Some(PointerEditOutcome::Cancel { restore: 2 }),
            "canceling before the drag threshold must not become a click"
        );
        assert_eq!(canceled_click.finish(owner.window, &2, true), None);
        assert_eq!(canceled_click.offset(owner.window, 20.0), None);
        let mut ordinary_click =
            HorizontalEditSession::new(owner, 10.0, 2_i32, 4.0).expect("ordinary click");
        assert_eq!(
            ordinary_click.finish(owner.window, &2, true),
            Some(PointerEditOutcome::Click)
        );
        let mut mixed =
            HorizontalEditSession::new(owner, 0.0, None::<f32>, 4.0).expect("finite mixed gesture");
        mixed.offset(owner.window, 8.0);
        assert_eq!(
            mixed.finish(owner.window, &Some(0.2), true),
            Some(PointerEditOutcome::Commit {
                before: None,
                after: Some(0.2)
            })
        );
        assert_eq!(
            resolve_value_draft("bad", &2_i32, |raw| raw.parse().ok()),
            DraftResolution::Rejected { restore: 2 }
        );
        assert_eq!(
            resolve_value_draft("2", &2_i32, |raw| raw.parse().ok()),
            DraftResolution::Accepted {
                value: 2,
                changed: false
            }
        );
        let mut color = ValueEditSession::new(owner, [1_u8, 2, 3, 255]);
        assert_eq!(color.finish(UiWindowId(2), &[4, 5, 6, 255], true), None);
        assert!(color.accepts(owner.window));
        assert_eq!(
            color.finish(owner.window, &[4, 5, 6, 255], false),
            Some(PointerEditOutcome::Cancel {
                restore: [1, 2, 3, 255]
            })
        );
        assert!(!color.accepts(owner.window));
        assert_eq!(color.finish(owner.window, &[4, 5, 6, 255], true), None);
    }
}
