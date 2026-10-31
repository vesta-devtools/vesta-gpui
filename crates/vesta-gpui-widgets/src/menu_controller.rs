//! Stable descriptor path and window-qualified menu interaction state.

use crate::{PopupCloseReason, PopupDismissal};
use vesta_gpui::{InputTarget, KeyChord, KeyStroke, Menu, MenuId, MenuItemId, UiWindowId};

/// Keyboard intent from one open menu. The host resolves descriptors and executes actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKeyAction {
    Updated,
    OpenSubmenu(MenuId),
    Activate(MenuItemId),
    Close,
    TraverseFocus { backwards: bool },
    Consumed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuController {
    path: Vec<MenuId>,
    highlighted: Vec<Option<MenuItemId>>,
    return_focus: Option<InputTarget>,
    owner_window: Option<UiWindowId>,
}

impl MenuController {
    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.path.is_empty()
    }

    #[must_use]
    pub fn path(&self) -> &[MenuId] {
        &self.path
    }

    #[must_use]
    pub fn current(&self) -> Option<MenuId> {
        self.path.last().copied()
    }

    #[must_use]
    pub fn highlighted(&self) -> Option<MenuItemId> {
        self.highlighted.last().copied().flatten()
    }

    /// Parent menus retain their own highlighted submenu opener while a child is active.
    #[must_use]
    pub fn highlighted_in(&self, menu: MenuId) -> Option<MenuItemId> {
        self.path
            .iter()
            .position(|id| *id == menu)
            .and_then(|depth| self.highlighted.get(depth))
            .copied()
            .flatten()
    }

    #[must_use]
    pub fn return_focus(&self) -> Option<InputTarget> {
        self.return_focus
    }

    pub fn open(&mut self, menu: MenuId) {
        self.path.clear();
        self.highlighted.clear();
        self.return_focus = None;
        self.owner_window = None;
        self.path.push(menu);
        self.highlighted.push(None);
    }

    /// Save the opener's focus for keyboard dismissal. The host decides
    /// whether to restore it after `close`, since a pointer may focus another
    /// control while the menu is open.
    pub fn open_for(&mut self, menu: MenuId, return_focus: Option<InputTarget>) {
        self.open(menu);
        self.return_focus = return_focus;
        self.owner_window = return_focus.map(|target| target.window);
    }

    /// Explicit window ownership is required even if this menu has no opener.
    pub fn open_in(&mut self, window: UiWindowId, menu: MenuId, return_focus: Option<InputTarget>) {
        self.open(menu);
        self.owner_window = Some(window);
        self.return_focus = return_focus.filter(|target| target.window == window);
    }

    pub fn toggle(&mut self, menu: MenuId) {
        if self.path.first().copied() == Some(menu) {
            let _ = self.close();
        } else {
            self.open(menu);
        }
    }

    pub fn close(&mut self) -> Option<InputTarget> {
        self.path.clear();
        self.highlighted.clear();
        self.owner_window = None;
        self.return_focus.take()
    }

    /// Consume the owning session once, including a reconciled-away menu root.
    pub fn dismiss(
        &mut self,
        source_window: UiWindowId,
        reason: PopupCloseReason,
    ) -> Option<PopupDismissal> {
        if self.owner_window != Some(source_window) {
            return None;
        }
        Some(PopupDismissal {
            source_window,
            reason,
            opener: self.close(),
        })
    }

    /// A descriptor update cannot re-enter an ancestor under duplicate popup IDs.
    pub fn enter_submenu(&mut self, menu: MenuId) {
        if self.path.contains(&menu) {
            return;
        }
        self.path.push(menu);
        self.highlighted.push(None);
    }

    pub fn leave_submenu(&mut self) -> bool {
        if self.path.len() <= 1 {
            return false;
        }
        self.path.pop();
        self.highlighted.pop();
        true
    }

    pub fn highlight(&mut self, item: Option<MenuItemId>) {
        if let Some(highlighted) = self.highlighted.last_mut() {
            *highlighted = item;
        }
    }

    /// Revalidate the open path against the available descriptors after an update.
    /// Stable enabled selections survive reorder. Invalid selections clear without
    /// choosing another action, and invalid submenu links discard their descendants.
    /// A missing root empties the path; `close` still consumes the saved opener.
    pub fn reconcile<A>(&mut self, menus: &[Menu<A>]) -> bool {
        let previous_path = self.path.clone();
        let previous_highlights = self.highlighted.clone();
        for depth in 0..self.path.len() {
            let Some(menu) = menus.iter().find(|menu| menu.id == self.path[depth]) else {
                self.path.truncate(depth);
                self.highlighted.truncate(depth);
                break;
            };
            let selected = menu.items.iter().find(|item| {
                Some(item.id) == self.highlighted[depth] && item.enabled && !item.separator
            });
            self.highlighted[depth] = selected.map(|item| item.id);
            if let Some(child) = self.path.get(depth + 1)
                && selected.and_then(|item| item.submenu) != Some(*child)
            {
                self.path.truncate(depth + 1);
                self.highlighted.truncate(depth + 1);
                break;
            }
        }
        self.path != previous_path || self.highlighted != previous_highlights
    }

    /// Pointer selection uses the same item and path state as keyboard navigation.
    /// Returning to an ancestor item dismisses its open descendants.
    pub fn highlight_in<A>(&mut self, menu: &Menu<A>, item: MenuItemId) -> bool {
        if !menu
            .items
            .iter()
            .any(|candidate| candidate.id == item && candidate.enabled && !candidate.separator)
        {
            return false;
        }
        let Some(depth) = self.path.iter().rposition(|id| *id == menu.id) else {
            return false;
        };
        if self.path.len() > depth + 1 && self.highlighted[depth] == Some(item) {
            return false;
        }
        let changed = self.path.len() != depth + 1 || self.highlighted[depth] != Some(item);
        self.path.truncate(depth + 1);
        self.highlighted.truncate(depth + 1);
        self.highlighted[depth] = Some(item);
        changed
    }

    pub fn step<A>(&mut self, menu: &Menu<A>, delta: i32) {
        if self.current() != Some(menu.id) {
            return;
        }
        let enabled = menu
            .items
            .iter()
            .filter(|item| item.enabled && !item.separator)
            .map(|item| item.id)
            .collect::<Vec<_>>();
        if enabled.is_empty() {
            self.highlight(None);
            return;
        }
        let current = self
            .highlighted()
            .and_then(|id| enabled.iter().position(|candidate| *candidate == id));
        let next = match (current, delta.signum()) {
            (None, direction) if direction < 0 => enabled.len() - 1,
            (None, _) => 0,
            (Some(index), direction) if direction < 0 => {
                index.checked_sub(1).unwrap_or(enabled.len() - 1)
            }
            (Some(index), _) => (index + 1) % enabled.len(),
        };
        self.highlight(Some(enabled[next]));
    }

    pub fn select_edge<A>(&mut self, menu: &Menu<A>, end: bool) {
        if self.current() != Some(menu.id) {
            return;
        }
        let mut enabled = menu
            .items
            .iter()
            .filter(|item| item.enabled && !item.separator);
        self.highlight(if end {
            enabled.next_back().map(|item| item.id)
        } else {
            enabled.next().map(|item| item.id)
        });
    }

    /// Navigate the current descriptor using stable enabled item identities. A stale or
    /// absent highlight never activates a replacement item. All keys belong to the menu
    /// until the host closes it, preserving shortcut precedence for an open popup.
    #[must_use]
    pub fn key_action<A>(&mut self, menu: &Menu<A>, chord: KeyChord) -> MenuKeyAction {
        if self.current() != Some(menu.id) {
            return MenuKeyAction::Consumed;
        }
        match chord.key {
            KeyStroke::ArrowDown => self.step(menu, 1),
            KeyStroke::ArrowUp => self.step(menu, -1),
            KeyStroke::Home => self.select_edge(menu, false),
            KeyStroke::End => self.select_edge(menu, true),
            KeyStroke::ArrowLeft => {
                if !self.leave_submenu() {
                    return MenuKeyAction::Close;
                }
            }
            KeyStroke::Escape => return MenuKeyAction::Close,
            KeyStroke::Tab => {
                return MenuKeyAction::TraverseFocus {
                    backwards: chord.modifiers.shift,
                };
            }
            KeyStroke::ArrowRight | KeyStroke::Enter | KeyStroke::Space => {
                let Some(item) = menu.items.iter().find(|item| {
                    Some(item.id) == self.highlighted() && item.enabled && !item.separator
                }) else {
                    return MenuKeyAction::Consumed;
                };
                return match chord.key {
                    KeyStroke::ArrowRight => item
                        .submenu
                        .map_or(MenuKeyAction::Consumed, MenuKeyAction::OpenSubmenu),
                    _ => MenuKeyAction::Activate(item.id),
                };
            }
            _ => return MenuKeyAction::Consumed,
        }
        MenuKeyAction::Updated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vesta_gpui::{MenuItem, MenuItemId};

    #[test]
    fn menu_keyboard_step_skips_disabled_and_separator_items() {
        let menu = Menu::new(MenuId(1), "File")
            .item(MenuItem::section_header(MenuItemId(5), "Documents"))
            .item(MenuItem::new(MenuItemId(1), "Open", ()))
            .item(MenuItem::separator(MenuItemId(2)))
            .item(MenuItem::new(MenuItemId(3), "Unavailable", ()).enabled(false))
            .item(MenuItem::new(MenuItemId(4), "Save", ()));
        let mut state = MenuController::default();
        state.open(menu.id);
        state.step(&menu, 1);
        assert_eq!(state.highlighted(), Some(MenuItemId(1)));
        state.step(&menu, 1);
        assert_eq!(state.highlighted(), Some(MenuItemId(4)));
        state.select_edge(&menu, false);
        assert_eq!(state.highlighted(), Some(MenuItemId(1)));
        state.select_edge(&menu, true);
        assert_eq!(state.highlighted(), Some(MenuItemId(4)));

        let reordered = Menu::new(menu.id, "File")
            .item(MenuItem::new(MenuItemId(4), "Renamed", ()).checked(true))
            .item(MenuItem::new(MenuItemId(1), "Open", ()));
        assert!(!state.reconcile(&[reordered]));
        assert_eq!(state.highlighted(), Some(MenuItemId(4)));
        let disabled = Menu::new(menu.id, "File")
            .item(MenuItem::new(MenuItemId(4), "Renamed", ()).enabled(false))
            .item(MenuItem::new(MenuItemId(1), "Open", ()));
        assert!(state.reconcile(std::slice::from_ref(&disabled)));
        assert_eq!(
            state.highlighted(),
            None,
            "replacement cannot select another action"
        );
        state.step(&disabled, 1);
        assert_eq!(state.highlighted(), Some(MenuItemId(1)));
        assert!(state.reconcile(&[Menu::<()>::new(menu.id, "Empty")]));
        assert_eq!(state.highlighted(), None);
        let key = |key| KeyChord::new(key, vesta_gpui::KeyModifiers::default());
        assert_eq!(
            state.key_action(&disabled, key(KeyStroke::Enter)),
            MenuKeyAction::Consumed
        );
        assert_eq!(
            state.key_action(&disabled, key(KeyStroke::ArrowDown)),
            MenuKeyAction::Updated
        );
        assert_eq!(
            state.key_action(&disabled, key(KeyStroke::Enter)),
            MenuKeyAction::Activate(MenuItemId(1))
        );
        state.highlight(Some(MenuItemId(4)));
        assert_eq!(
            state.key_action(&disabled, key(KeyStroke::Enter)),
            MenuKeyAction::Consumed
        );
    }

    #[test]
    fn submenu_keeps_the_opener_focus_until_one_close_consumes_it() {
        let opener = InputTarget {
            window: vesta_gpui::UiWindowId(4),
            element: vesta_gpui::ElementId::new(12),
        };
        let mut state = MenuController::default();
        state.open_for(MenuId(1), Some(opener));
        let parent = Menu::new(MenuId(1), "File")
            .item(MenuItem::new(MenuItemId(10), "Reset", ()))
            .item(MenuItem::new(MenuItemId(11), "Disabled", ()).enabled(false))
            .item(MenuItem::new(MenuItemId(12), "Export", ()).submenu(MenuId(2)));
        assert!(state.highlight_in(&parent, MenuItemId(12)));
        state.enter_submenu(MenuId(2));
        assert_eq!(state.return_focus(), Some(opener));
        assert!(!state.highlight_in(&parent, MenuItemId(12)));
        assert_eq!(state.path(), &[MenuId(1), MenuId(2)]);
        let cyclic_child = Menu::new(MenuId(2), "Export")
            .item(MenuItem::new(MenuItemId(20), "Ancestor", ()).submenu(MenuId(1)));
        state.select_edge(&cyclic_child, false);
        for _ in 0..3 {
            let decision = state.key_action(
                &cyclic_child,
                KeyChord::new(KeyStroke::ArrowRight, Default::default()),
            );
            let MenuKeyAction::OpenSubmenu(target) = decision else {
                panic!("typed submenu decision");
            };
            state.enter_submenu(target);
            assert_eq!(
                state.path(),
                &[MenuId(1), MenuId(2)],
                "reject ancestor cycle"
            );
            // Hosts initialize the requested child's highlight after entry. A
            // refused ancestor descriptor must not replace the active highlight.
            state.select_edge(&parent, false);
            state.step(&parent, 1);
            assert_eq!(state.highlighted_in(MenuId(1)), Some(MenuItemId(12)));
            assert_eq!(
                state.highlighted(),
                Some(MenuItemId(20)),
                "a stale ancestor descriptor must not replace the active menu highlight"
            );
            assert_eq!(state.return_focus(), Some(opener));
        }
        assert!(!state.highlight_in(&parent, MenuItemId(11)));
        assert!(state.highlight_in(&parent, MenuItemId(10)));
        assert_eq!(state.path(), &[MenuId(1)]);
        assert_eq!(state.highlighted(), Some(MenuItemId(10)));
        let parent_state = state.clone();
        state.step(&cyclic_child, 1);
        assert_eq!(
            state, parent_state,
            "a retired child descriptor cannot mutate its parent"
        );
        state.highlight(Some(MenuItemId(12)));
        state.enter_submenu(MenuId(2));
        let child = Menu::new(MenuId(2), "Export").item(MenuItem::new(MenuItemId(20), "Image", ()));
        state.select_edge(&child, false);
        assert!(!state.reconcile(&[parent.clone(), child.clone()]));
        let mut retargeted = parent.clone();
        retargeted.items[2].submenu = Some(MenuId(3));
        assert!(state.reconcile(&[retargeted, child]));
        assert_eq!(state.path(), &[MenuId(1)]);
        assert_eq!(state.highlighted(), Some(MenuItemId(12)));
        assert_eq!(state.return_focus(), Some(opener));
        state.enter_submenu(MenuId(2));
        assert!(state.reconcile(&[parent]));
        assert_eq!(state.path(), &[MenuId(1)]);
        assert!(state.reconcile::<()>(&[]));
        assert!(!state.is_open());
        assert_eq!(state.return_focus(), Some(opener));
        assert_eq!(state.close(), Some(opener));
        assert_eq!(state.close(), None);

        // A context menu without an opener still has a source window. A foreign
        // close cannot consume it; a reconciled-away root still closes once.
        state.open_in(opener.window, MenuId(1), None);
        assert!(
            state
                .dismiss(UiWindowId(999), PopupCloseReason::Explicit)
                .is_none()
        );
        assert!(state.is_open());
        assert!(state.reconcile::<()>(&[]));
        assert_eq!(
            state
                .dismiss(opener.window, PopupCloseReason::OwnerRemoved)
                .expect("own vanished menu")
                .opener,
            None
        );
        assert!(
            state
                .dismiss(opener.window, PopupCloseReason::Explicit)
                .is_none()
        );
        state.open_in(UiWindowId(999), MenuId(1), Some(opener));
        assert_eq!(
            state.return_focus(),
            None,
            "foreign opener must not be retained"
        );
        assert!(
            state
                .dismiss(opener.window, PopupCloseReason::Explicit)
                .is_none()
        );
        assert!(
            state
                .dismiss(UiWindowId(999), PopupCloseReason::OutsidePress)
                .is_some()
        );
    }
}
