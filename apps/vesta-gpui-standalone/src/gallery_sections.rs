//! Shared section visibility transitions with window-owned input retirement.

use super::*;

impl App {
    pub(super) fn toggle_gallery_section(&mut self, section: GallerySectionId) {
        if !self.demo.collapsed_gallery_sections.insert(section) {
            self.demo.collapsed_gallery_sections.remove(&section);
            return;
        }
        let owners = std::iter::once(STANDALONE_WINDOW)
            .chain(self.native_windows.values().map(|host| host.ui_window))
            .collect::<Vec<_>>();
        for owner in owners {
            let Some(scope) = self.gallery_scopes(owner) else {
                continue;
            };
            let body = section.element(scope, 3);
            let header = section.element(scope, 1);
            let lost_focus = self.window_focus(owner).is_some_and(|element| {
                self.window_actions(owner)
                    .is_some_and(|route| route.is_descendant_of(element, body))
            });
            let close_combo = section == GallerySectionId::Selection
                && self
                    .demo
                    .combo
                    .opener()
                    .is_some_and(|opener| opener.window == owner);
            let close_popover = section == GallerySectionId::Overlays
                && self
                    .demo
                    .popover
                    .opener()
                    .is_some_and(|opener| opener.window == owner);
            // Retire before a queued release can use the last published route.
            self.retire_window_subtree_input(owner, body);
            if close_combo {
                self.close_combo_for(owner, PopupCloseReason::OwnerRemoved);
            }
            if close_popover {
                self.close_popover_for(owner, PopupCloseReason::OwnerRemoved);
            }
            if (lost_focus || close_combo || close_popover)
                && self.window_actions(owner).is_some_and(|route| {
                    route.action_at(header) == Some(&DemoAction::ToggleGallerySection(section))
                })
            {
                self.set_window_focus(owner, Some(header));
            }
        }
    }
}
