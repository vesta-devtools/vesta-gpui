//! Caller-clocked, window-local insertion-caret visibility and wakeups.
use std::time::{Duration, Instant};
use vesta_gpui::ElementId;

const PERIOD: Duration = Duration::from_millis(500);

#[derive(Default)]
pub struct CaretBlink {
    target: Option<ElementId>,
    visible: bool,
    next: Option<Instant>,
}

impl CaretBlink {
    /// A new live owner begins visibly; blur/removal cancels its wakeup.
    pub fn update(&mut self, target: Option<ElementId>, now: Instant) -> bool {
        if self.target != target {
            self.target = target;
            self.visible = target.is_some();
            self.next = target.map(|_| now + PERIOD);
            return true;
        }
        self.tick(now)
    }

    /// Advance at most once, so a delayed host never spins over missed ticks.
    pub fn tick(&mut self, now: Instant) -> bool {
        if self.next.is_some_and(|at| now >= at) {
            self.visible = !self.visible;
            self.next = Some(now + PERIOD);
            return true;
        }
        false
    }

    /// Input reveals the current caret immediately and starts a fresh period.
    pub fn restart(&mut self, now: Instant) -> bool {
        if self.target.is_none() {
            return false;
        }
        let changed = !self.visible;
        self.visible = true;
        self.next = Some(now + PERIOD);
        changed
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caret_deadlines_cancel_replace_and_advance_without_an_expired_loop() {
        let now = Instant::now();
        let first = ElementId::new(1);
        let second = ElementId::new(2);
        let mut blink = CaretBlink::default();
        assert!(!blink.update(None, now));
        assert!(!blink.restart(now));
        assert!(blink.update(Some(first), now));
        assert!(blink.visible());
        assert!(!blink.update(Some(first), now + PERIOD / 2));
        assert!(blink.tick(now + PERIOD));
        assert!(!blink.visible());
        assert_eq!(blink.deadline(), Some(now + PERIOD * 2));
        assert!(!blink.tick(now + PERIOD));
        assert!(blink.restart(now + PERIOD));
        assert!(blink.visible(), "input reveals the current caret");
        assert_eq!(blink.deadline(), Some(now + PERIOD * 2));
        assert!(blink.update(Some(second), now + PERIOD));
        assert!(blink.visible(), "new owner begins visibly");
        assert!(blink.update(None, now + PERIOD));
        assert_eq!(blink.deadline(), None, "blur/removal cancels future ticks");
        assert!(!blink.tick(now + PERIOD * 10));
        assert!(blink.update(Some(second), now + PERIOD * 10));
        assert_eq!(blink.deadline(), Some(now + PERIOD * 11));
        assert!(blink.tick(now + PERIOD * 100));
        assert_eq!(blink.deadline(), Some(now + PERIOD * 101));
    }
}
