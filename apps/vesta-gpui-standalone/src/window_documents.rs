//! Per-window adaptation of the shared read-only Markdown input controller.
use super::*;
use vesta_gpui_widgets::{
    MarkdownInteraction, MarkdownKeyIntent, MarkdownLinkIntent, MarkdownPointerResult,
    MarkdownSurface,
};
mod drag_scroll;
mod smoke;
pub(super) use smoke::{Smoke, smoke_enabled, validate_smoke_args};

impl App {
    pub(super) fn new_window_documents(&self) -> Result<GalleryDocuments, String> {
        GalleryDocuments::for_sample(self.demo.long_document).map_err(|error| error.to_string())
    }

    fn replace_document_sample(
        &mut self,
        long: bool,
    ) -> Result<(), vesta_gpui_widgets::MarkdownError> {
        self.documents
            .replace_sample(long, &mut self.input_ownership)?;
        for host in self.native_windows.values_mut() {
            host.documents.replace_sample(long, &mut host.input)?;
        }
        self.demo.long_document = long;
        Ok(())
    }
    pub(super) fn show_documents(&mut self) {
        if let Some(item) = ensure_documents_tab(&mut self.demo.workspace) {
            let _ = self.demo.workspace.activate(item);
            self.demo.sync_workspace_ui();
        }
    }

    pub(super) fn apply_document_action(
        &mut self,
        window: UiWindowId,
        action: gallery::documents::DocumentAction,
    ) {
        let Some(scopes) = self.window_scopes(window) else {
            return;
        };
        let target = InputTarget {
            window,
            element: scopes.markdown.element(0),
        };
        if !self.window_actions(window).is_some_and(|actions| {
            FocusScope::new(window, actions, self.focus_root_for_window(window)).accepts(target)
        }) {
            return;
        }
        self.set_window_focus(window, Some(target.element));
        let paged = self
            .with_window_documents(window, |documents, _| documents.is_paged())
            .unwrap_or(false);
        let result = if action == gallery::documents::DocumentAction::NextSample
            && (!self.demo.long_document || paged)
        {
            let result = self.replace_document_sample(!self.demo.long_document);
            if result.is_ok() {
                self.with_window_documents(window, |documents, input| documents.defer_focus(input));
            }
            Some(result.map(|_| true))
        } else {
            self.with_window_documents(window, |documents, input| {
                Ok(documents.apply_action(action, input))
            })
        };
        match result {
            Some(Ok(true)) => {
                self.reset_document_page_scroll(window, target.element);
                self.request_all_redraw();
            }
            Some(Err(error)) => self.fatal_error = Some(error.to_string()),
            _ => {}
        }
    }

    pub(super) fn window_document_focus_rect(
        &mut self,
        window: UiWindowId,
        element: ElementId,
    ) -> Option<Rect> {
        self.with_document_input(window, |interaction, surface, _| {
            (interaction.target().element == element)
                .then(|| interaction.selection_focus_rect(&surface).ok().flatten())
                .flatten()
        })
        .flatten()
    }

    pub(super) fn window_document_reveal_rect(
        &mut self,
        window: UiWindowId,
        element: ElementId,
    ) -> Option<Rect> {
        let scope = self.window_scopes(window)?.markdown;
        self.window_document_focus_rect(window, element)
            .or_else(|| {
                self.with_document_input(window, |interaction, surface, _| {
                    if interaction.target().element != element {
                        return None;
                    }
                    let block = surface.document.block_id_at(surface.range.start)?;
                    let paragraph = surface.document.paragraph_id(scope, block).ok()?;
                    surface.layout.node(paragraph).map(|node| node.rect)
                })
                .flatten()
            })
    }

    fn reset_document_page_scroll(&mut self, window: UiWindowId, element: ElementId) {
        let Some(gallery) = self.gallery_scopes(window) else {
            return;
        };
        let Some(path) = self
            .window_actions(window)
            .and_then(|actions| actions.path_to(element))
        else {
            return;
        };
        for (item, state) in &mut self.demo.page_scroll {
            if path.contains(&page_scroll_id(gallery, *item)) {
                let offset = state.offset();
                state.scroll_by_point(Point::new(-offset.x, -offset.y));
            }
        }
    }

    fn with_window_documents<R>(
        &mut self,
        window: UiWindowId,
        callback: impl FnOnce(&mut GalleryDocuments, &mut InputOwnership) -> R,
    ) -> Option<R> {
        if window == STANDALONE_WINDOW {
            Some(callback(&mut self.documents, &mut self.input_ownership))
        } else {
            let host = self
                .native_windows
                .values_mut()
                .find(|host| host.ui_window == window)?;
            Some(callback(&mut host.documents, &mut host.input))
        }
    }
    pub(super) fn select_window_document_accessibility(
        &mut self,
        window: UiWindowId,
        selection: vesta_gpui::ReadOnlyDocumentSelection,
    ) -> bool {
        if self.window_text_composing(window) {
            return false;
        }
        if !matches!(
            self.with_document_input(window, |interaction, surface, _| interaction
                .accessible_selection(&surface, selection)),
            Some(Ok(_))
        ) {
            return false;
        }
        self.set_window_focus(window, Some(selection.root));
        let result = self.with_document_input(window, |interaction, surface, input| {
            interaction.accept_accessible_selection(&surface, selection, input)?;
            interaction.selection_focus_rect(&surface)
        });
        match result {
            Some(Ok(Some(rect))) => self.reveal_window_rect(window, selection.root, rect),
            Some(Ok(None)) => {}
            Some(Err(error)) => {
                eprintln!("document accessibility selection: {error}");
                return false;
            }
            None => return false,
        }
        self.request_all_redraw();
        true
    }

    fn activate_window_document_link(&mut self, window: UiWindowId, intent: MarkdownLinkIntent) {
        let result = self.with_document_input(window, |interaction, surface, _| {
            interaction
                .resolve_activation(&surface, intent)
                .map_err(|error| error.to_string())?
                .ok_or("document_link_not_projected".to_owned())
                .and_then(|link| {
                    vesta_gpui_winit::resolve_document_link(link.destination, None)
                        .map_err(|error| error.to_string())
                })
        });
        match result {
            Some(Ok(vesta_gpui_winit::DocumentLink::Web(url))) if !self.smoke => {
                if let Err(error) = vesta_gpui_winit::open_web_link(&url) {
                    eprintln!("opening Markdown link: {error}");
                }
            }
            Some(Err(error)) => eprintln!("opening Markdown link: {error}"),
            _ => {}
        }
    }

    pub(super) fn activate_window_document_target(
        &mut self,
        window: UiWindowId,
        target: InputTarget,
    ) -> bool {
        let intent = self
            .with_document_input(window, |interaction, surface, _| {
                interaction.activate_target(&surface, target).ok().flatten()
            })
            .flatten();
        if let Some(intent) = intent {
            self.set_window_focus(window, Some(target.element));
            self.activate_window_document_link(window, intent);
            true
        } else {
            false
        }
    }

    pub(super) fn with_document_input<R>(
        &mut self,
        window: UiWindowId,
        callback: impl FnOnce(&mut MarkdownInteraction, MarkdownSurface<'_>, &mut InputOwnership) -> R,
    ) -> Option<R> {
        self.with_window_documents(window, |documents, input| {
            let (interaction, surface) = documents.input_parts(window)?;
            Some(callback(interaction, surface, input))
        })
        .flatten()
    }

    pub(super) fn cancel_window_document_input(&mut self, window: UiWindowId) -> bool {
        if window == STANDALONE_WINDOW {
            self.documents.cancel_input(&mut self.input_ownership)
        } else {
            self.native_windows
                .values_mut()
                .find(|host| host.ui_window == window)
                .is_some_and(|host| host.documents.cancel_input(&mut host.input))
        }
    }

    pub(super) fn handle_window_document_key(&mut self, window: UiWindowId, key: &Key) -> bool {
        let Some(chord) = shortcut_chord(key.as_ref(), self.window_modifiers(window)) else {
            return false;
        };
        if self.window_text_composing(window) {
            return false;
        }
        let Some(target) =
            self.with_document_input(window, |interaction, _, _| interaction.target())
        else {
            return false;
        };
        if !self.window_actions(window).is_some_and(|actions| {
            FocusScope::new(window, actions, self.focus_root_for_window(window)).accepts(target)
        }) {
            return false;
        }
        let Some(result) = self.with_document_input(window, |interaction, surface, input| {
            interaction.key(&surface, chord, 64 * 1024, input)
        }) else {
            return false;
        };
        match result {
            Ok(Some(MarkdownKeyIntent::Copy(text))) => {
                if let Err(error) = self.clipboard.write_text(&text) {
                    eprintln!("copying Markdown selection: {error}");
                }
            }
            Ok(Some(MarkdownKeyIntent::Handled)) => {}
            Ok(Some(MarkdownKeyIntent::RevealSelection)) => {
                if matches!(
                    self.with_window_documents(window, |documents, input| documents
                        .follow_selection(input)),
                    Some(Ok(true))
                ) {
                    self.reset_document_page_scroll(window, target.element);
                    self.request_all_redraw();
                    return true;
                }
                match self.with_document_input(window, |interaction, surface, _| {
                    interaction.selection_focus_rect(&surface)
                }) {
                    Some(Ok(Some(rect))) => self.reveal_window_rect(window, target.element, rect),
                    Some(Err(error)) => eprintln!("revealing Markdown selection: {error}"),
                    _ => {}
                }
            }
            Ok(Some(MarkdownKeyIntent::Activate(intent))) => {
                self.activate_window_document_link(window, intent)
            }
            Ok(None) => return false,
            Err(error) => eprintln!("Markdown selection: {error}"),
        }
        true
    }

    pub(super) fn handle_window_document_event(
        &mut self,
        window: UiWindowId,
        event: &WindowEvent,
    ) -> bool {
        if let WindowEvent::MouseWheel { delta, .. } = event {
            let movement = match delta {
                MouseScrollDelta::LineDelta(x, y) => Point::new(*x, *y),
                MouseScrollDelta::PixelDelta(point) => Point::new(point.x as f32, point.y as f32),
            };
            if movement.x.is_finite()
                && movement.y.is_finite()
                && (movement.x.abs() > f32::EPSILON || movement.y.abs() > f32::EPSILON)
            {
                self.with_window_documents(window, |documents, input| {
                    documents.cancel_pending_focus(input);
                });
            }
        }
        if matches!(
            event,
            WindowEvent::Focused(false)
                | WindowEvent::CloseRequested
                | WindowEvent::Destroyed
                | WindowEvent::Resized(_)
                | WindowEvent::ScaleFactorChanged { .. }
        ) {
            self.cancel_window_document_input(window);
            return false;
        }
        let (cursor, hit, scale) = if window == STANDALONE_WINDOW {
            (
                self.cursor,
                self.hit_test.hit(self.cursor),
                self.window
                    .as_ref()
                    .map_or(1.0, |window| window.scale_factor()),
            )
        } else if let Some(host) = self
            .native_windows
            .values()
            .find(|host| host.ui_window == window)
        {
            (
                host.cursor,
                host.hit_test.hit(host.cursor),
                host.window.scale_factor(),
            )
        } else {
            return false;
        };
        let pressed = matches!(
            event,
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state: ElementState::Pressed,
                ..
            }
        );
        if pressed {
            let Some(target) = self
                .with_document_input(window, |interaction, _, input| {
                    input.capture().is_none().then(|| interaction.target())
                })
                .flatten()
            else {
                return false;
            };
            let accepted = self.window_actions(window).is_some_and(|actions| {
                hit.is_some_and(|hit| actions.is_descendant_of(hit, target.element))
                    && FocusScope::new(window, actions, self.focus_root_for_window(window))
                        .accepts(target)
            });
            if !accepted {
                return false;
            }
            self.set_window_focus(window, Some(target.element));
            if window == STANDALONE_WINDOW {
                self.pointer_click.cancel();
            } else if let Some(host) = self
                .native_windows
                .values_mut()
                .find(|host| host.ui_window == window)
            {
                host.pointer_click.cancel();
            }
        }
        let extend = self.window_modifiers(window).shift_key();
        let Some(result) =
            self.with_document_input(window, |interaction, surface, input| match event {
                WindowEvent::MouseInput {
                    button: MouseButton::Left,
                    state: ElementState::Pressed,
                    ..
                } => interaction.press(&surface, cursor, extend, input),
                WindowEvent::MouseInput {
                    button: MouseButton::Left,
                    state: ElementState::Released,
                    ..
                } => interaction.release(&surface, cursor, PointerButton::Primary, input),
                WindowEvent::CursorMoved { position, .. } => {
                    interaction.pointer_move(&surface, logical_point(*position, scale), input)
                }
                _ => Ok(MarkdownPointerResult::Ignored),
            })
        else {
            return false;
        };
        match result {
            Ok(MarkdownPointerResult::Ignored) => false,
            Ok(MarkdownPointerResult::Activate(intent)) => {
                self.activate_window_document_link(window, intent);
                true
            }
            Ok(MarkdownPointerResult::Selection) => {
                if let WindowEvent::CursorMoved { position, .. } = event {
                    let point = logical_point(*position, scale);
                    if window == STANDALONE_WINDOW {
                        self.cursor = point;
                    } else if let Some(host) = self
                        .native_windows
                        .values_mut()
                        .find(|host| host.ui_window == window)
                    {
                        host.cursor = point;
                    }
                }
                true
            }
            Err(error) => {
                eprintln!("Markdown selection: {error}");
                true
            }
        }
    }
}
