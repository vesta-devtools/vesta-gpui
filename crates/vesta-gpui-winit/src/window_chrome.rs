//! Platform window edge hit testing for hosts using a custom title bar.

use vesta_gpui::{Point, Size};
use winit::window::ResizeDirection;

#[must_use]
pub fn window_resize_direction(cursor: Point, size: Size, edge: f32) -> Option<ResizeDirection> {
    if cursor.x < 0.0 || cursor.y < 0.0 || cursor.x > size.width || cursor.y > size.height {
        return None;
    }
    let west = cursor.x <= edge;
    let east = cursor.x >= size.width - edge;
    let north = cursor.y <= edge;
    let south = cursor.y >= size.height - edge;
    match (west, east, north, south) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (_, true, true, _) => Some(ResizeDirection::NorthEast),
        (true, _, _, true) => Some(ResizeDirection::SouthWest),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        (true, _, _, _) => Some(ResizeDirection::West),
        (_, true, _, _) => Some(ResizeDirection::East),
        (_, _, true, _) => Some(ResizeDirection::North),
        (_, _, _, true) => Some(ResizeDirection::South),
        _ => None,
    }
}
