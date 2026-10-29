//! Grapheme-safe user navigation over the existing Parley editor.
use super::*;

impl TextInputState {
    /// Keep Parley's visual movement and valid affinities. A font's shaped
    /// cluster can split a Unicode grapheme, so only interior endpoints snap
    /// outward in the direction of travel. This never changes the source text.
    pub(super) fn snap_navigation_selection(
        &mut self,
        context: &mut TextInputContext,
        previous_focus: Option<usize>,
    ) {
        let selection = self.editor.raw_selection();
        let anchor = selection.anchor().index();
        let focus = selection.focus().index();
        let edge = |index: usize, previous: Option<usize>| {
            for mark in &self.marks {
                if mark.range.start < index && index < mark.range.end {
                    return match previous {
                        Some(previous) if index > previous => mark.range.end,
                        Some(previous) if index < previous => mark.range.start,
                        _ if index - mark.range.start <= mark.range.end - index => mark.range.start,
                        _ => mark.range.end,
                    };
                }
            }
            for (start, grapheme) in self.text().grapheme_indices(true) {
                let end = start + grapheme.len();
                if start < index && index < end {
                    return match previous {
                        Some(previous) if index > previous => end,
                        Some(previous) if index < previous => start,
                        _ => {
                            if index - start <= end - index {
                                start
                            } else {
                                end
                            }
                        }
                    };
                }
            }
            index
        };
        let snapped_focus = edge(focus, previous_focus);
        let snapped_anchor = if anchor == focus {
            snapped_focus
        } else {
            edge(anchor, None)
        };
        if (anchor, focus) != (snapped_anchor, snapped_focus) {
            self.editor
                .driver(&mut context.fonts, &mut context.layout)
                .select_byte_range(snapped_anchor, snapped_focus);
        }
    }
}
