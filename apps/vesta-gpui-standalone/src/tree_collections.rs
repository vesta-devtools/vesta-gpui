//! Host-owned sample hierarchy operations; shared interaction decisions (ADR-0657).
use super::*;
use vesta_gpui_widgets::{TreeViewController, tree_focus_after_change};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TreeDataAction {
    Reverse,
    RemoveSelected,
    Restore,
    ExpandAll,
    CollapseAll,
    RevealSelected,
}

pub(super) fn sample_entries() -> Vec<TreeViewEntry<usize>> {
    std::iter::once(TreeViewEntry {
        id: 0,
        label: "Scene".into(),
        depth: 0,
        has_children: true,
    })
    .chain((1..=1_000).map(|id| TreeViewEntry {
        id,
        label: format!("Object {id:05}"),
        depth: 1,
        has_children: false,
    }))
    .collect()
}

fn reverse_siblings(entries: &[TreeViewEntry<usize>]) -> Vec<TreeViewEntry<usize>> {
    let Some(root) = entries.first() else {
        return Vec::new();
    };
    let starts = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| (entry.depth == root.depth).then_some(index))
        .chain(std::iter::once(entries.len()))
        .collect::<Vec<_>>();
    let mut output = Vec::with_capacity(entries.len());
    for pair in starts.windows(2).rev() {
        output.push(entries[pair[0]].clone());
        output.extend(reverse_siblings(&entries[pair[0] + 1..pair[1]]));
    }
    output
}

impl App {
    fn tree_focuses(&self) -> Vec<(UiWindowId, usize)> {
        std::iter::once(STANDALONE_WINDOW)
            .chain(self.native_windows.values().map(|host| host.ui_window))
            .filter_map(|window| {
                let input = if window == STANDALONE_WINDOW {
                    &self.input_ownership
                } else {
                    &self
                        .native_windows
                        .values()
                        .find(|host| host.ui_window == window)?
                        .input
                };
                if self.focus_root_for_window(window).is_none()
                    && let Some(target) = input.pending_focus_target(window)
                    && let Some(scopes) = self.window_scopes(window)
                    && let Some(entry) = self.demo.tree_entries.iter().find(|entry| {
                        scopes.gallery.element(TREE_ROW_BASE + entry.id as u32) == target.element
                    })
                {
                    return Some((window, entry.id));
                }
                let scope = FocusScope::new(
                    window,
                    self.window_actions(window)?,
                    self.focus_root_for_window(window),
                );
                let id = scope.ancestor_action(
                    InputTarget {
                        window,
                        element: self.window_focus(window)?,
                    },
                    |action| match action {
                        DemoAction::SelectVirtualTree(id) | DemoAction::ToggleVirtualTree(id) => {
                            Some(*id)
                        }
                        _ => None,
                    },
                )?;
                Some((window, id))
            })
            .collect()
    }

    fn finish_tree_change(
        &mut self,
        previous: Option<&[TreeViewEntry<usize>]>,
        focused: Vec<(UiWindowId, usize)>,
    ) {
        let Ok(controller) = TreeViewController::new(
            &self.demo.tree_entries,
            &mut self.demo.tree_expansion,
            &mut self.demo.tree_selection,
        ) else {
            return;
        };
        let count = controller.visible().len();
        self.demo
            .tree_scroll
            .sync_content_height(count as f32 * VIRTUAL_ROW_HEIGHT, TREE_VIEWPORT_HEIGHT);
        for (window, id) in focused {
            let Ok(next) = tree_focus_after_change(
                previous.unwrap_or(&self.demo.tree_entries),
                &self.demo.tree_entries,
                &self.demo.tree_expansion,
                &id,
            ) else {
                continue;
            };
            if next != Some(id) {
                let target = next.and_then(|id| {
                    self.window_scopes(window)
                        .map(|scopes| scopes.gallery.element(TREE_ROW_BASE + id as u32))
                });
                if target.is_some_and(|target| {
                    self.window_actions(window)
                        .is_none_or(|actions| !actions.focus_order().contains(&target))
                }) {
                    // The repaired identity is already validated in the next tree.
                    // Preserve the existing immediate focus contract and stage it so
                    // a currently offscreen parent is retained in that next frame.
                    self.set_window_focus(window, target);
                    let target = InputTarget {
                        window,
                        element: target.expect("validated repair"),
                    };
                    if window == STANDALONE_WINDOW {
                        self.input_ownership.request_focus(target);
                    } else if let Some(host) = self
                        .native_windows
                        .values_mut()
                        .find(|host| host.ui_window == window)
                    {
                        host.input.request_focus(target);
                    }
                } else {
                    self.set_window_focus(window, target);
                }
            }
        }
        self.request_all_redraw();
    }

    pub(super) fn select_virtual_tree(&mut self, id: usize) {
        let modifiers = self.window_modifiers(self.action_window);
        if let Ok(mut controller) = TreeViewController::new(
            &self.demo.tree_entries,
            &mut self.demo.tree_expansion,
            &mut self.demo.tree_selection,
        ) {
            controller.select(
                id,
                SelectionModifiers {
                    additive: modifiers.control_key(),
                    range: modifiers.shift_key(),
                },
            );
        }
    }

    pub(super) fn toggle_virtual_tree(&mut self, id: usize) {
        let focused = self.tree_focuses();
        if let Ok(mut controller) = TreeViewController::new(
            &self.demo.tree_entries,
            &mut self.demo.tree_expansion,
            &mut self.demo.tree_selection,
        ) {
            let expanded = !controller.expansion().contains(&id);
            controller.set_expanded(&id, expanded);
        }
        self.finish_tree_change(None, focused);
    }

    pub(super) fn apply_tree_data(&mut self, action: TreeDataAction) {
        let focused = self.tree_focuses();
        let previous = self.demo.tree_entries.clone();
        let mut reveal = None;
        match action {
            TreeDataAction::Reverse => self.demo.tree_entries = reverse_siblings(&previous),
            TreeDataAction::RemoveSelected => {
                let mut removed_depth = None;
                self.demo.tree_entries.retain(|entry| {
                    if removed_depth.is_some_and(|depth| entry.depth > depth) {
                        return false;
                    }
                    removed_depth = self
                        .demo
                        .tree_selection
                        .contains(&entry.id)
                        .then_some(entry.depth);
                    removed_depth.is_none()
                });
            }
            TreeDataAction::Restore => self.demo.tree_entries = sample_entries(),
            TreeDataAction::RevealSelected => {
                let selected = self
                    .demo
                    .tree_entries
                    .iter()
                    .find(|entry| self.demo.tree_selection.contains(&entry.id))
                    .map(|entry| entry.id);
                if let Some(id) = selected
                    && let Ok(mut controller) = TreeViewController::new(
                        &self.demo.tree_entries,
                        &mut self.demo.tree_expansion,
                        &mut self.demo.tree_selection,
                    )
                {
                    reveal = controller.reveal(&id).map(|update| update.focus);
                }
            }
            TreeDataAction::ExpandAll | TreeDataAction::CollapseAll => {
                if let Ok(mut controller) = TreeViewController::new(
                    &self.demo.tree_entries,
                    &mut self.demo.tree_expansion,
                    &mut self.demo.tree_selection,
                ) {
                    controller.set_all_expanded(action == TreeDataAction::ExpandAll);
                }
            }
        }
        self.finish_tree_change(Some(&previous), focused);
        if let Some(id) = reveal {
            self.reveal_tree_row(self.action_window, id);
        }
    }

    fn reveal_tree_row(&mut self, window: UiWindowId, id: usize) {
        let Some(scopes) = self.window_scopes(window) else {
            return;
        };
        let Ok(tree) = VisibleTree::build(&self.demo.tree_entries, &self.demo.tree_expansion)
        else {
            return;
        };
        let Some(index) = tree.index_of(&id) else {
            return;
        };
        vesta_gpui_widgets::reveal_collection_row(
            &mut self.demo.tree_scroll,
            index,
            VIRTUAL_ROW_HEIGHT,
            TREE_VIEWPORT_HEIGHT,
        );
        let target = scopes.gallery.element(TREE_ROW_BASE + id as u32);
        if self
            .window_actions(window)
            .is_some_and(|actions| actions.focus_order().contains(&target))
        {
            self.set_window_focus(window, Some(target));
        } else {
            self.restore_after_rebuild(window, Some(target));
        }
        self.reveal_window_focus(window, scopes.gallery.element(TREE_SCROLL));
    }

    pub(super) fn handle_tree_key(&mut self, window: UiWindowId, id: usize, key: &Key) -> bool {
        // Disclosure remains an independent button, including its Space behavior.
        if key == &Key::Named(NamedKey::Space)
            && self
                .window_actions(window)
                .and_then(|actions| actions.action_at(self.window_focus(window)?))
                != Some(&DemoAction::SelectVirtualTree(id))
        {
            return false;
        }
        let Some(chord) = shortcut_chord(key.as_ref(), self.window_modifiers(window)) else {
            return false;
        };
        let focused = self.tree_focuses();
        let Ok(mut controller) = TreeViewController::new(
            &self.demo.tree_entries,
            &mut self.demo.tree_expansion,
            &mut self.demo.tree_selection,
        ) else {
            return false;
        };
        let page = window_collections::fixed_page_rows(
            controller.visible().len(),
            VIRTUAL_ROW_HEIGHT,
            TREE_VIEWPORT_HEIGHT,
            self.demo.tree_scroll.offset_y(),
        )
        .unwrap_or(1);
        let Some(update) = controller.key(&id, chord, page) else {
            return false;
        };
        self.finish_tree_change(None, focused);
        self.reveal_tree_row(window, update.focus);
        true
    }
}

#[cfg(test)]
mod tests;
