//! Pure popup placement in caller-provided logical bounds.

use vesta_gpui::{Point, Rect, Size};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupDirection {
    Below,
    Right,
}

/// Places a popup inside the screen bounds and flips it when the preferred edge would overflow.
/// Caller supplies valid Core rects and a valid popup size. Measured menu entry
/// points validate their complete geometry before invoking projection adapters.
#[must_use]
pub fn place_popup(anchor: Rect, popup: Size, bounds: Rect, direction: PopupDirection) -> Point {
    let (mut x, mut y) = match direction {
        PopupDirection::Below => {
            let below = anchor.bottom();
            let y = if below + popup.height <= bounds.bottom() {
                below
            } else {
                anchor.y - popup.height
            };
            (anchor.x, y)
        }
        PopupDirection::Right => {
            let right = anchor.right();
            let x = if right + popup.width <= bounds.right() {
                right
            } else {
                anchor.x - popup.width
            };
            (x, anchor.y)
        }
    };
    x = x.clamp(bounds.x, (bounds.right() - popup.width).max(bounds.x));
    y = y.clamp(bounds.y, (bounds.bottom() - popup.height).max(bounds.y));
    Point::new(x, y)
}
