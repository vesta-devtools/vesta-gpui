//! Focused view scrolling over the same published geometry as paint and picking.
use super::*;

impl App {
    pub(super) fn set_table_scroll(&mut self, requested: usize) {
        self.table_scroll = requested.min(self.table_rows.saturating_sub(self.table_page_rows));
        self.redraw();
    }

    fn focus_inside(&self, root: vesta_gpui::ElementId, area: vesta_gpui::Rect) -> bool {
        self.ownership.focus().is_some_and(|target| {
            target.element == root
                || self
                    .layout_tree
                    .as_ref()
                    .and_then(|l| l.node(target.element))
                    .and_then(|n| n.rect.intersection(n.inherited_clip))
                    .is_some_and(|r| {
                        area.contains(Point::new(r.x + r.width * 0.5, r.y + r.height * 0.5))
                    })
        })
    }

    pub(super) fn content_scroll_key(&mut self, key: &Key) -> bool {
        if self.layout_tree.is_none() || self.busy() || self.search_focused() {
            return false;
        }
        if self.focus_inside(view::TABLE, self.table_area) {
            let requested = match key {
                Key::Named(NamedKey::PageDown) => {
                    self.table_scroll.saturating_add(self.table_page_rows)
                }
                Key::Named(NamedKey::PageUp) => {
                    self.table_scroll.saturating_sub(self.table_page_rows)
                }
                Key::Named(NamedKey::ArrowDown) => self.table_scroll.saturating_add(1),
                Key::Named(NamedKey::ArrowUp) => self.table_scroll.saturating_sub(1),
                Key::Named(NamedKey::End) => usize::MAX,
                Key::Named(NamedKey::Home) => 0,
                _ => return false,
            };
            self.cancel_drag();
            self.set_table_scroll(requested);
            self.set_focus(Some(InputTarget {
                window: WINDOW,
                element: view::TABLE,
            }));
            return true;
        }
        if self.focus_inside(view::TRACK, self.area) {
            let Some(timeline) = &mut self.timeline else {
                return false;
            };
            let offset = match key {
                Key::Named(NamedKey::PageDown) => timeline.scroll + self.area.height,
                Key::Named(NamedKey::PageUp) => timeline.scroll - self.area.height,
                Key::Named(NamedKey::ArrowDown) => timeline.scroll + 25.0,
                Key::Named(NamedKey::ArrowUp) => timeline.scroll - 25.0,
                Key::Named(NamedKey::End) => self.track_scroll_max,
                _ => return false, // Home keeps the existing timeline Fit action.
            };
            self.cancel_drag();
            if let Some(timeline) = &mut self.timeline {
                timeline.scroll = offset.clamp(0.0, self.track_scroll_max);
            }
            self.set_focus(Some(InputTarget {
                window: WINDOW,
                element: view::TRACK,
            }));
            self.redraw();
            return true;
        }
        false
    }
}
