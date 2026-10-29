//! Keyboard navigation over caller-owned collection order (ADR-0306).

use vesta_gpui::KeyStroke;

/// The host resolves stable identity to its current visual index. Invalid or removed
/// indices are rejected, and page movement uses the caller's visible row count.
#[must_use]
pub fn linear_collection_index(
    current: usize,
    count: usize,
    page_size: usize,
    key: KeyStroke,
) -> Option<usize> {
    let last = count.checked_sub(1)?;
    if current >= count {
        return None;
    }
    Some(match key {
        KeyStroke::ArrowDown => current.saturating_add(1).min(last),
        KeyStroke::ArrowUp => current.saturating_sub(1),
        KeyStroke::PageDown => current.saturating_add(page_size.max(1)).min(last),
        KeyStroke::PageUp => current.saturating_sub(page_size.max(1)),
        KeyStroke::Home => 0,
        KeyStroke::End => last,
        _ => return None,
    })
}
