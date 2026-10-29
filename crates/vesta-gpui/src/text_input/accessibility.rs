//! Text-run accessibility and selection projected from the shared editor.
use super::*;

impl TextInputState {
    /// Project text runs and selection from the same editor used for painting and hit testing.
    pub fn accessibility(
        &mut self,
        context: &mut TextInputContext,
        update: &mut accesskit::TreeUpdate,
        node: &mut accesskit::Node,
        next_id: impl FnMut() -> accesskit::NodeId,
        origin: Point,
    ) {
        if self.password {
            return;
        }
        let mut projection = self.legacy_accessibility.take().unwrap_or_default();
        self.accessibility_projection(context, &mut projection, update, node, next_id, origin);
        if self.editor.cursor_geometry(1.0).is_none() {
            node.clear_text_selection();
        }
    }

    /// Compatibility entry point: returns whether a valid range changed selection.
    pub fn select_accessibility(
        &mut self,
        context: &mut TextInputContext,
        selection: &accesskit::TextSelection,
    ) -> bool {
        self.resolve_accessibility_selection(context, selection)
            .and_then(|selection| self.apply_accessibility_selection(context, selection))
            .unwrap_or(false)
    }

    /// Resolve the direct projection without mutating selection, including a valid
    /// unchanged range. Multi-window hosts use their owning AccessibilityTree instead.
    pub fn resolve_accessibility_selection(
        &mut self,
        context: &mut TextInputContext,
        selection: &accesskit::TextSelection,
    ) -> Option<TextInputSelection> {
        let projection = self.legacy_accessibility.clone()?;
        self.resolve_accessibility_projection(context, &projection, selection)
    }

    pub(crate) fn accessibility_projection(
        &mut self,
        context: &mut TextInputContext,
        projection: &mut parley::layout::LayoutAccessibility,
        update: &mut accesskit::TreeUpdate,
        node: &mut accesskit::Node,
        next_id: impl FnMut() -> accesskit::NodeId,
        origin: Point,
    ) {
        if self.password {
            return;
        }
        self.editor
            .refresh_layout(&mut context.fonts, &mut context.layout);
        let Some(layout) = self.editor.try_layout() else {
            return;
        };
        projection.build_nodes(
            self.editor.raw_text(),
            layout,
            update,
            node,
            next_id,
            f64::from(origin.x),
            f64::from(origin.y),
        );
        if let Some(selection) = self
            .editor
            .raw_selection()
            .to_access_selection(layout, projection)
        {
            node.set_text_selection(selection);
        }
        if !node.is_disabled() {
            node.add_action(accesskit::Action::SetTextSelection);
        }
        node.set_value(self.editor.raw_text());
        self.mark_read_only(node);
        self.legacy_accessibility = Some(projection.clone());
    }

    pub(crate) fn resolve_accessibility_projection(
        &mut self,
        context: &mut TextInputContext,
        projection: &parley::layout::LayoutAccessibility,
        selection: &accesskit::TextSelection,
    ) -> Option<TextInputSelection> {
        if self.password || self.is_composing() {
            return None;
        }
        self.editor
            .refresh_layout(&mut context.fonts, &mut context.layout);
        let layout = self.editor.try_layout()?;
        let selection =
            parley::editing::Selection::from_access_selection(selection, layout, projection)?;
        if self.read_only
            && (!self.read_only_boundary(selection.anchor().index())
                || !self.read_only_boundary(selection.focus().index()))
        {
            return None;
        }
        Some(TextInputSelection {
            anchor: selection.anchor().index(),
            focus: selection.focus().index(),
        })
    }

    /// Apply a resolved platform range after the host's field/focus transition.
    /// None rejects a composing/password buffer or offsets invalid after that transition.
    pub fn apply_accessibility_selection(
        &mut self,
        context: &mut TextInputContext,
        selection: TextInputSelection,
    ) -> Option<bool> {
        if self.password
            || self.is_composing()
            || !self.text().is_char_boundary(selection.anchor)
            || !self.text().is_char_boundary(selection.focus)
            || self.read_only
                && (!self.read_only_boundary(selection.anchor)
                    || !self.read_only_boundary(selection.focus))
        {
            return None;
        }
        let before = *self.editor.raw_selection();
        self.editor
            .driver(&mut context.fonts, &mut context.layout)
            .select_byte_range(selection.anchor, selection.focus);
        self.end_group();
        self.refresh(context);
        Some(before != *self.editor.raw_selection())
    }

    /// Assistive replacement is an ordinary, bounded, undoable text edit.
    pub fn replace_accessibility(
        &mut self,
        context: &mut TextInputContext,
        text: &str,
    ) -> Result<bool, TextInputError> {
        if self.read_only {
            return Err(TextInputError::ReadOnly);
        }
        if self.is_composing() {
            return Err(TextInputError::Composing);
        }
        if normalized_input(text, self.is_multiline())?.len() > self.max_bytes {
            return Err(TextInputError::TooLong);
        }
        let before = *self.editor.raw_selection();
        self.act(context, TextInputAction::SelectAll);
        let changed = self.insert(context, text, TextEditKind::Paste)?;
        let after = self.editor.raw_selection();
        Ok(changed || before.anchor() != after.anchor() || before.focus() != after.focus())
    }

    fn mark_read_only(&self, node: &mut accesskit::Node) {
        if self.read_only {
            node.set_read_only();
            node.remove_action(accesskit::Action::SetValue);
        }
    }
}
