//! Host-owned references tracked by the same edits and Undo as their visible text.
use super::*;
use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextInputMark {
    pub id: String,
    pub range: Range<usize>,
}

#[derive(Clone)]
pub(super) struct EditSnapshot {
    pub editor: PlainEditor<()>,
    pub marks: Vec<TextInputMark>,
}

impl TextInputState {
    /// Begin a new host edit boundary while retaining the committed text/references.
    pub fn clear_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.end_group();
    }

    pub fn marks(&self) -> &[TextInputMark] {
        &self.marks
    }

    /// Discard host-invalid references from every Undo state while keeping text.
    pub fn forget_marks(&mut self, ids: &[String]) {
        self.marks.retain(|mark| !ids.contains(&mark.id));
        for entry in self.undo.iter_mut().chain(&mut self.redo) {
            entry.marks.retain(|mark| !ids.contains(&mark.id));
        }
    }

    /// Retained Undo/Redo references let a host release resources safely.
    pub fn retained_mark_ids(&self) -> impl Iterator<Item = &str> {
        self.marks
            .iter()
            .chain(
                self.undo
                    .iter()
                    .chain(&self.redo)
                    .flat_map(|entry| &entry.marks),
            )
            .map(|mark| mark.id.as_str())
    }

    pub fn insert_mark(
        &mut self,
        context: &mut TextInputContext,
        id: &str,
        label: &str,
    ) -> Result<(), TextInputError> {
        if self.read_only {
            return Err(TextInputError::ReadOnly);
        }
        if self.is_composing() {
            return Err(TextInputError::Composing);
        }
        if self.password
            || id.is_empty()
            || id.len() > 64
            || label.is_empty()
            || label.len() > 128
            || label.contains(['\r', '\n', '\t'])
            || self.marks.len() >= 32
            || self.retained_mark_ids().any(|existing| existing == id)
        {
            return Err(TextInputError::InvalidMark);
        }
        let original = self.clone();
        self.end_group();
        self.expand_mark_selection(context);
        self.snap_navigation_selection(context, None);
        let start = self.editor.raw_selection().text_range().start;
        let before = self.edit_snapshot();
        let changed = match self.insert(context, label, TextEditKind::Paste) {
            Ok(changed) => changed,
            Err(error) => {
                *self = original;
                return Err(error);
            }
        };
        if !changed {
            self.undo.push(before);
            self.redo.clear();
            self.trim_history();
        }
        self.marks.push(TextInputMark {
            id: id.to_owned(),
            range: start..start + label.len(),
        });
        self.marks.sort_by_key(|mark| mark.range.start);
        self.end_group();
        Ok(())
    }

    /// Remove one reference without disturbing the user's current caret/selection.
    pub fn remove_mark(&mut self, context: &mut TextInputContext, id: &str) -> bool {
        if self.read_only || self.is_composing() {
            return false;
        }
        let Some(mark) = self.marks.iter().find(|mark| mark.id == id).cloned() else {
            return false;
        };
        let selection = *self.editor.raw_selection();
        self.editor
            .driver(&mut context.fonts, &mut context.layout)
            .select_byte_range(mark.range.start, mark.range.end);
        self.end_group();
        let _ = self.insert(context, "", TextEditKind::Paste);
        let remap = |index: usize| {
            if index <= mark.range.start {
                index
            } else if index >= mark.range.end {
                index - mark.range.len()
            } else {
                mark.range.start
            }
        };
        self.editor
            .driver(&mut context.fonts, &mut context.layout)
            .select_byte_range(
                remap(selection.anchor().index()),
                remap(selection.focus().index()),
            );
        self.refresh(context);
        true
    }

    pub(super) fn edit_snapshot(&self) -> EditSnapshot {
        EditSnapshot {
            editor: self.editor.clone(),
            marks: self.marks.clone(),
        }
    }

    fn expanded_mark_range(&self, mut range: Range<usize>) -> Range<usize> {
        for mark in &self.marks {
            if range.start < mark.range.end && range.end > mark.range.start {
                range.start = range.start.min(mark.range.start);
                range.end = range.end.max(mark.range.end);
            }
        }
        range
    }

    pub(super) fn expand_mark_selection(&mut self, context: &mut TextInputContext) {
        let range = self.editor.raw_selection().text_range();
        if range.is_empty() {
            return;
        }
        let expanded = self.expanded_mark_range(range.clone());
        if range != expanded {
            self.editor
                .driver(&mut context.fonts, &mut context.layout)
                .select_byte_range(expanded.start, expanded.end);
        }
    }

    pub(super) fn deleted_mark_range(
        &mut self,
        context: &mut TextInputContext,
        before: &EditSnapshot,
    ) -> Range<usize> {
        let start = self.editor.raw_selection().text_range().start;
        let removed = before.editor.raw_text().len() - self.editor.raw_text().len();
        if removed == 0 {
            return 0..0;
        }
        let range = start..start + removed;
        let expanded = self.expanded_mark_range(range.clone());
        if range != expanded {
            self.editor = before.editor.clone();
            let mut driver = self.editor.driver(&mut context.fonts, &mut context.layout);
            driver.select_byte_range(expanded.start, expanded.end);
            driver.delete_selection();
        }
        expanded
    }

    pub(super) fn move_marks(&mut self, range: Range<usize>, inserted: usize) {
        self.marks.retain_mut(|mark| {
            if range.start < mark.range.end && range.end > mark.range.start {
                return false;
            }
            if mark.range.start >= range.end {
                mark.range.start = mark.range.start - range.len() + inserted;
                mark.range.end = mark.range.end - range.len() + inserted;
            }
            true
        });
    }
}
