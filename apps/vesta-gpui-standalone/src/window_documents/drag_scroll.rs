//! Schedule the shared gesture through each live window's retained scroll path.
use super::*;

impl App {
    pub(crate) fn drive_document_drag_scroll(&mut self, now: Instant) -> Option<Instant> {
        let windows = self
            .window
            .as_ref()
            .map(|window| (STANDALONE_WINDOW, window.id()))
            .into_iter()
            .chain(
                self.native_windows
                    .values()
                    .map(|host| (host.ui_window, host.window.id())),
            )
            .collect::<Vec<_>>();
        let mut deadline = None;
        for (window, native) in windows {
            if self.display_blocked(native) {
                self.cancel_window_document_input(window);
                continue;
            }
            deadline = deadline
                .into_iter()
                .chain(self.tick_window_document_drag(window, now))
                .min();
        }
        deadline
    }

    pub(crate) fn tick_window_document_drag(
        &mut self,
        window: UiWindowId,
        now: Instant,
    ) -> Option<Instant> {
        let target = self.with_document_input(window, |interaction, _, _| interaction.target())?;
        if self.window_text_composing(window)
            || !self.window_actions(window).is_some_and(|actions| {
                FocusScope::new(window, actions, self.focus_root_for_window(window)).accepts(target)
            })
        {
            self.cancel_window_document_input(window);
            return None;
        }
        let result = self.with_document_input(window, |interaction, surface, input| {
            let before = interaction.selection();
            let step = interaction.drag_scroll_step(&surface, now, input);
            (step, before != interaction.selection())
        })?;
        let (step, selection_changed) = result;
        let mut changed = selection_changed;
        match step {
            Ok(Some(movement)) => {
                if self.scroll_window_document(window, target, movement) {
                    changed = true;
                } else {
                    self.with_document_input(window, |interaction, _, _| {
                        interaction.stop_drag_scroll()
                    });
                }
            }
            Ok(None) => {}
            Err(error) => eprintln!("Markdown drag scroll: {error}"),
        }
        if changed {
            self.request_all_redraw();
        }
        self.with_document_input(window, |interaction, _, _| {
            interaction.drag_scroll_deadline()
        })?
    }

    fn scroll_window_document(
        &mut self,
        window: UiWindowId,
        target: InputTarget,
        movement: Point,
    ) -> bool {
        let root = self.focus_root_for_window(window);
        let data = if window == STANDALONE_WINDOW {
            vesta_gpui_widgets::ScrollInputPath::new(
                &FocusScope::new(window, &self.ui_actions, root),
                target,
            )
            .map(|path| (path, self.scopes.gallery, self.scroll_geometry.clone()))
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .and_then(|host| {
                    vesta_gpui_widgets::ScrollInputPath::new(
                        &FocusScope::new(window, &host.actions, root),
                        target,
                    )
                    .map(|path| (path, host.scopes.gallery, host.scroll_geometry.clone()))
                })
        };
        let Some((path, gallery, geometry)) = data else {
            return false;
        };
        let targets = geometry.targets(gallery);
        let mut states = ScrollStates {
            tree: &mut self.demo.tree_scroll,
            virtual_list: &mut self.demo.virtual_scroll,
            virtual_table: &mut self.demo.virtual_table_scroll,
            nested: &mut self.demo.nested_scroll,
            horizontal: &mut self.demo.horizontal_scroll,
            gallery: (window == STANDALONE_WINDOW).then_some(&mut self.gallery_scroll),
            pages: &mut self.demo.page_scroll,
            tabs: &mut self.demo.tab_scroll,
        };
        path.scroll(movement, |element, remaining| {
            targets
                .iter()
                .find(|(id, _, _)| *id == element)
                .and_then(|(_, _, kind)| states.for_target(*kind))
                .map_or(remaining, |state| state.scroll_by_point(remaining))
        }) != movement
    }
}
