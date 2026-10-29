//! Controlled tree interactions over the original host preorder (ADR-0657).

use vesta_gpui::{KeyChord, KeyStroke};

use crate::{
    ListSelectionState, SelectionModifiers, TreeExpansionState, TreeKeyAction, TreeViewEntry,
    TreeViewError, VisibleTree, collection_focus_after_change,
};

/// A focus intent for the calling view, never a platform focus mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeViewUpdate<K> {
    pub focus: K,
    pub changed: bool,
}

/// Borrows existing UI state. Entries and each window's focus remain host-owned.
pub struct TreeViewController<'a, K: Ord> {
    entries: &'a [TreeViewEntry<K>],
    expansion: &'a mut TreeExpansionState<K>,
    selection: &'a mut ListSelectionState<K>,
}

impl<'a, K: Ord + Clone> TreeViewController<'a, K> {
    /// Validate the entire source before pruning stale expansion/selection keys.
    /// Collapsed, still-live descendants deliberately keep their selection.
    pub fn new(
        entries: &'a [TreeViewEntry<K>],
        expansion: &'a mut TreeExpansionState<K>,
        selection: &'a mut ListSelectionState<K>,
    ) -> Result<Self, TreeViewError> {
        VisibleTree::build(entries, expansion)?;
        expansion.reconcile(entries);
        selection.reconcile(
            &entries
                .iter()
                .map(|entry| entry.id.clone())
                .collect::<Vec<_>>(),
        );
        Ok(Self {
            entries,
            expansion,
            selection,
        })
    }

    #[must_use]
    pub fn visible(&self) -> VisibleTree<'_, K> {
        // Neither immutable entries nor validated depths can change while borrowed.
        VisibleTree::build(self.entries, self.expansion).expect("validated tree source")
    }

    #[must_use]
    pub fn expansion(&self) -> &TreeExpansionState<K> {
        self.expansion
    }

    #[must_use]
    pub fn selection(&self) -> &ListSelectionState<K> {
        self.selection
    }

    /// A stale or hidden row cannot mutate selection. Ranges follow visible order.
    pub fn select(&mut self, id: K, modifiers: SelectionModifiers) -> Option<TreeViewUpdate<K>> {
        let visible = self.visible();
        visible.index_of(&id)?;
        let order = (0..visible.len())
            .map(|index| visible.entry(index).expect("visible index").id.clone())
            .collect::<Vec<_>>();
        let changed = self.selection.select(id.clone(), &order, modifiers);
        Some(TreeViewUpdate { focus: id, changed })
    }

    /// Set rather than toggle, so a repeated command cannot accidentally reopen.
    pub fn set_expanded(&mut self, id: &K, expanded: bool) -> bool {
        let visible = self.visible();
        let Some(entry) = visible.index_of(id).and_then(|index| visible.entry(index)) else {
            return false;
        };
        if !entry.has_children || self.expansion.contains(id) == expanded {
            return false;
        }
        self.expansion.toggle(id.clone());
        true
    }

    /// Includes currently hidden branches; selection is unaffected.
    pub fn set_all_expanded(&mut self, expanded: bool) -> bool {
        let mut changed = false;
        for entry in self.entries.iter().filter(|entry| entry.has_children) {
            if self.expansion.contains(&entry.id) != expanded {
                self.expansion.toggle(entry.id.clone());
                changed = true;
            }
        }
        changed
    }

    /// Open only the target's ancestors, without selecting it or opening siblings.
    pub fn reveal(&mut self, id: &K) -> Option<TreeViewUpdate<K>> {
        let mut ancestors = Vec::new();
        let source = self.entries.iter().position(|entry| &entry.id == id)?;
        let mut depth = self.entries[source].depth;
        for entry in self.entries[..source].iter().rev() {
            if depth > 0 && entry.depth == depth - 1 {
                ancestors.push(entry.id.clone());
                depth -= 1;
            }
        }
        let mut changed = false;
        for ancestor in ancestors {
            if !self.expansion.contains(&ancestor) {
                self.expansion.toggle(ancestor);
                changed = true;
            }
        }
        Some(TreeViewUpdate {
            focus: id.clone(),
            changed,
        })
    }

    /// Unmodified navigation selects; Control moves focus only; Shift extends.
    /// Alt/Super and unsupported chords are left to the host's existing router.
    pub fn key(
        &mut self,
        focused: &K,
        chord: KeyChord,
        page_size: usize,
    ) -> Option<TreeViewUpdate<K>> {
        let modifiers = chord.modifiers;
        if modifiers.alt || modifiers.super_key {
            return None;
        }
        if chord.key == KeyStroke::Space && modifiers.control && !modifiers.shift {
            return self.select(
                focused.clone(),
                SelectionModifiers {
                    additive: true,
                    range: false,
                },
            );
        }
        let action = self
            .visible()
            .key_action(self.expansion, focused, chord.key, page_size)?;
        match action {
            TreeKeyAction::SetExpanded { id, expanded } => Some(TreeViewUpdate {
                changed: self.set_expanded(&id, expanded),
                focus: id,
            }),
            TreeKeyAction::Focus(id) if modifiers.control && !modifiers.shift => {
                Some(TreeViewUpdate {
                    focus: id,
                    changed: false,
                })
            }
            TreeKeyAction::Focus(id) => self.select(
                id,
                SelectionModifiers {
                    additive: modifiers.control,
                    range: modifiers.shift,
                },
            ),
        }
    }
}

/// Repair one calling view after collapse or replacement. Both sources must be
/// valid; an unknown old focus is not permission to focus the first tree row.
pub fn tree_focus_after_change<K: Ord + Clone>(
    previous: &[TreeViewEntry<K>],
    current: &[TreeViewEntry<K>],
    expansion: &TreeExpansionState<K>,
    focused: &K,
) -> Result<Option<K>, TreeViewError> {
    let old = VisibleTree::build(previous, expansion)?;
    let next = VisibleTree::build(current, expansion)?;
    if !previous.iter().any(|entry| &entry.id == focused) {
        return Ok(None);
    }
    let visible_ancestor = |id: &K| {
        let mut candidate = Some(id.clone());
        while let Some(id) = candidate {
            if next.index_of(&id).is_some() {
                return Some(id);
            }
            candidate = next.parent_of(&id).map(|entry| entry.id.clone());
        }
        None
    };
    if let Some(id) = visible_ancestor(focused) {
        return Ok(Some(id));
    }
    let mut parent = old.parent_of(focused);
    while let Some(entry) = parent {
        if let Some(id) = visible_ancestor(&entry.id) {
            return Ok(Some(id));
        }
        parent = old.parent_of(&entry.id);
    }
    Ok(collection_focus_after_change(
        &previous
            .iter()
            .map(|entry| entry.id.clone())
            .collect::<Vec<_>>(),
        &(0..next.len())
            .map(|index| next.entry(index).expect("visible index").id.clone())
            .collect::<Vec<_>>(),
        focused,
        None,
    ))
}
