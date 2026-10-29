//! Stable collection focus repair and virtual-row reveal, independent of Gallery slots.
use crate::ScrollAreaState;

/// Keep a live key; otherwise prefer an explicit ancestor, then the nearest surviving
/// following/preceding key in the previous order. Never retain a removed array position.
pub fn collection_focus_after_change<K: Clone + Eq>(
    previous: &[K],
    current: &[K],
    focused: &K,
    preferred: Option<&K>,
) -> Option<K> {
    if current.contains(focused) {
        return Some(focused.clone());
    }
    if let Some(key) = preferred.filter(|key| current.contains(key)) {
        return Some(key.clone());
    }
    let index = previous.iter().position(|key| key == focused)?;
    previous[index + 1..]
        .iter()
        .chain(previous[..index].iter().rev())
        .find(|key| current.contains(key))
        .cloned()
        .or_else(|| current.first().cloned())
}

pub fn reveal_collection_row(
    scroll: &mut ScrollAreaState,
    index: usize,
    row_height: f32,
    viewport_height: f32,
) {
    if !row_height.is_finite()
        || row_height <= 0.0
        || !viewport_height.is_finite()
        || viewport_height <= 0.0
    {
        return;
    }
    let top = index as f32 * row_height;
    let offset = scroll.offset_y();
    if top < offset {
        scroll.scroll_by(top - offset);
    } else if top + row_height > offset + viewport_height {
        scroll.scroll_by(top + row_height - offset - viewport_height);
    }
}
