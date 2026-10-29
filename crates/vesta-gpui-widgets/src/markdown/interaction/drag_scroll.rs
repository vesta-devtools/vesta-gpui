//! Edge scrolling is transient input state; the Host owns time and scroll offsets.
use super::*;
use std::time::{Duration, Instant};

const INTERVAL: Duration = Duration::from_millis(16);
const MAX_ELAPSED: Duration = Duration::from_millis(50);

pub(super) struct DragScroll {
    sampled_at: Instant,
    deadline: Instant,
    direction: f32,
}

impl MarkdownInteraction {
    /// Next owning-window wakeup, cleared with the ordinary gesture lifecycle.
    pub fn drag_scroll_deadline(&self) -> Option<Instant> {
        self.gesture
            .as_ref()?
            .scroll
            .as_ref()
            .map(|scroll| scroll.deadline)
    }

    /// Stop at a retained scroll boundary until a real pointer move rearms it.
    pub fn stop_drag_scroll(&mut self) {
        if let Some(gesture) = &mut self.gesture {
            gesture.scroll = None;
            gesture.scroll_stopped = true;
        }
    }

    /// Resolve the held pointer against current geometry and offer one bounded
    /// vertical step to the Host's existing ScrollInputPath. The Host requests
    /// redraw after consuming it; no page is loaded or offset changed here.
    pub fn drag_scroll_step(
        &mut self,
        surface: &MarkdownSurface<'_>,
        now: Instant,
        input: &mut InputOwnership,
    ) -> Result<Option<Point>, MarkdownProjectionError> {
        if surface.window != self.target.window {
            return Ok(None);
        }
        self.sync(surface, input);
        let Some(point) = self.gesture.as_ref().map(|gesture| gesture.pointer) else {
            return Ok(None);
        };
        // The last consumed step changed the layout. A stationary pointer must
        // extend into the newly visible text, using the same clamped hit path.
        self.pointer_move(surface, point, input)?;
        let visible = surface.layout.node(self.target.element).and_then(|node| {
            node.rect
                .intersection(node.inherited_clip)
                .map(|visible| (node.rect, visible))
        });
        let (distance, remaining) = visible
            .filter(|(_, rect)| rect.width > 0.0 && rect.height > 0.0)
            .filter(|_| point.x.is_finite() && point.y.is_finite())
            .map_or((0.0, 0.0), |(document, rect)| {
                // Do not scroll a following sibling (such as Diff) into view
                // once this document's own edge is already visible.
                if point.y < rect.y && document.y < rect.y {
                    (point.y - rect.y, rect.y - document.y)
                } else if point.y >= rect.bottom() && document.bottom() > rect.bottom() {
                    (
                        point.y - rect.bottom() + 0.01,
                        document.bottom() - rect.bottom(),
                    )
                } else {
                    (0.0, 0.0)
                }
            });
        let Some(gesture) = &mut self.gesture else {
            return Ok(None);
        };
        if distance == 0.0 {
            gesture.scroll = None;
            return Ok(None);
        }
        if gesture.scroll_stopped {
            return Ok(None);
        }
        let direction = distance.signum();
        let scroll = gesture.scroll.get_or_insert(DragScroll {
            sampled_at: now,
            deadline: now + INTERVAL,
            direction,
        });
        if scroll.direction != direction {
            *scroll = DragScroll {
                sampled_at: now,
                deadline: now + INTERVAL,
                direction,
            };
        }
        if now < scroll.deadline {
            return Ok(None);
        }
        let elapsed = now
            .saturating_duration_since(scroll.sampled_at)
            .min(MAX_ELAPSED);
        scroll.sampled_at = now;
        scroll.deadline = now + INTERVAL;
        let speed = (60.0 + distance.abs() * 12.0).min(600.0);
        Ok(Some(Point::new(
            0.0,
            direction * (speed * elapsed.as_secs_f32()).min(remaining),
        )))
    }
}
