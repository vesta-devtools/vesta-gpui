//! Stable-identity selection state for list-like widget projections.

use std::collections::BTreeSet;

/// Selection modifiers already normalized by the host input adapter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SelectionModifiers {
    pub additive: bool,
    pub range: bool,
}

/// UI selection state; author documents and list data remain host-owned.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListSelectionState<K: Ord> {
    selected: BTreeSet<K>,
    anchor: Option<K>,
}

impl<K: Ord + Clone> ListSelectionState<K> {
    #[must_use]
    pub fn single(id: K) -> Self {
        Self {
            selected: BTreeSet::from([id.clone()]),
            anchor: Some(id),
        }
    }

    #[must_use]
    pub fn contains(&self, id: &K) -> bool {
        self.selected.contains(id)
    }

    pub fn selected_ids(&self) -> impl Iterator<Item = &K> {
        self.selected.iter()
    }

    /// Drop identities removed by a host data refresh and keep a live range anchor.
    /// Remaining selections keep their stable identities across reordering.
    pub fn reconcile(&mut self, order: &[K]) -> bool {
        let before = self.clone();
        let live = order.iter().collect::<BTreeSet<_>>();
        self.selected.retain(|id| live.contains(id));
        if self
            .anchor
            .as_ref()
            .is_some_and(|anchor| !live.contains(anchor))
        {
            self.anchor = order.iter().find(|id| self.selected.contains(*id)).cloned();
        }
        *self != before
    }

    /// Apply a click or keyboard selection in the current visible item order.
    /// Returns false when `id` is absent from that order or the state is unchanged.
    pub fn select(&mut self, id: K, order: &[K], modifiers: SelectionModifiers) -> bool {
        let Some(end) = order.iter().position(|candidate| *candidate == id) else {
            return false;
        };
        let before = self.clone();
        if modifiers.range {
            let anchor = self
                .anchor
                .as_ref()
                .and_then(|anchor| order.iter().position(|candidate| candidate == anchor))
                .unwrap_or(end);
            if !modifiers.additive {
                self.selected.clear();
            }
            for item in &order[anchor.min(end)..=anchor.max(end)] {
                self.selected.insert(item.clone());
            }
            if self
                .anchor
                .as_ref()
                .is_none_or(|anchor| !order.contains(anchor))
            {
                self.anchor = Some(id);
            }
        } else if modifiers.additive {
            if !self.selected.remove(&id) {
                self.selected.insert(id.clone());
            }
            self.anchor = Some(id);
        } else {
            self.selected.clear();
            self.selected.insert(id.clone());
            self.anchor = Some(id);
        }
        *self != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_uses_stable_ids_across_toggle_range_and_reorder() {
        let mut state = ListSelectionState::single("b");
        let order = ["a", "b", "c", "d"];
        assert!(state.select(
            "d",
            &order,
            SelectionModifiers {
                range: true,
                ..Default::default()
            }
        ));
        assert_eq!(
            state.selected_ids().copied().collect::<Vec<_>>(),
            ["b", "c", "d"]
        );
        assert!(state.select(
            "a",
            &order,
            SelectionModifiers {
                additive: true,
                ..Default::default()
            }
        ));
        assert_eq!(state.selected_ids().copied().collect::<Vec<_>>(), order);
        assert!(state.select(
            "b",
            &order,
            SelectionModifiers {
                additive: true,
                ..Default::default()
            }
        ));
        assert!(!state.contains(&"b"));

        let reordered = ["d", "c", "b", "a"];
        assert!(state.select(
            "c",
            &reordered,
            SelectionModifiers {
                range: true,
                ..Default::default()
            }
        ));
        assert_eq!(
            state.selected_ids().copied().collect::<Vec<_>>(),
            ["b", "c"]
        );
        assert!(state.select(
            "d",
            &reordered,
            SelectionModifiers {
                additive: true,
                range: true,
            }
        ));
        assert_eq!(
            state.selected_ids().copied().collect::<Vec<_>>(),
            ["b", "c", "d"]
        );
        assert!(!state.select("missing", &reordered, SelectionModifiers::default()));
        assert!(state.reconcile(&["c", "b"]));
        assert_eq!(
            state.selected_ids().copied().collect::<Vec<_>>(),
            ["b", "c"]
        );
        assert!(state.reconcile(&["c"]));
        assert!(state.select(
            "new",
            &["c", "new"],
            SelectionModifiers {
                range: true,
                ..Default::default()
            }
        ));
        assert_eq!(
            state.selected_ids().copied().collect::<Vec<_>>(),
            ["c", "new"]
        );
    }
}
