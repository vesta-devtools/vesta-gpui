//! Window-scoped keyboard focus, text ownership and scroll reveal (ADR-0259).

use super::*;

pub(super) type TextViews = vesta_gpui::TextInputViews<TextFieldKind>;

impl App {
    pub(super) fn open_demo_dialog(&mut self, id: u8) {
        if self.demo.palette_session.is_open() {
            self.close_command_palette();
        }
        let owner = self.overlay_owner.unwrap_or(self.action_window);
        let return_focus = self.window_focus(owner);
        self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
        self.close_popover_for(self.action_window, PopupCloseReason::FocusTransfer);
        self.close_combo_for(self.action_window, PopupCloseReason::FocusTransfer);
        self.demo.status_overflow_open = false;
        let Some(scopes) = self.window_scopes(owner) else {
            return;
        };
        if self
            .demo
            .dialog_stack
            .open_in(
                owner,
                id,
                return_focus.map(|element| InputTarget {
                    window: owner,
                    element,
                }),
                Default::default(),
            )
            .is_ok()
        {
            self.cancel_window_workspace_drag(owner);
            self.set_window_focus(owner, None);
            self.overlay_owner = Some(owner);
            self.set_window_focus(owner, Some(dialog_element(scopes.root, id, 5)));
        }
    }

    pub(super) fn close_demo_dialog(&mut self, id: u8, confirmed: bool) {
        let reason = if confirmed {
            PopupCloseReason::Confirm
        } else {
            PopupCloseReason::Explicit
        };
        if let Some(dismissal) = self
            .demo
            .dialog_stack
            .dismiss(self.action_window, id, reason)
        {
            if confirmed {
                self.demo.number = self.demo.number.saturating_add(1);
                self.reset_number_draft();
            }
            let owner = self.overlay_owner.unwrap_or(STANDALONE_WINDOW);
            if self.demo.dialog_stack.is_empty() {
                self.overlay_owner = None;
            }
            if let Some(target) = dismissal.focus_request() {
                self.restore_after_rebuild(owner, Some(target.element));
            }
        }
    }

    pub(super) fn open_command_palette(&mut self) {
        if self.demo.palette_session.is_open() || !self.demo.dialog_stack.is_empty() {
            return;
        }
        if let Some(owner) = self.demo.inline_session.owner() {
            self.finish_inline_edit_for(owner, PopupCloseReason::FocusTransfer);
        }
        let owner = self.overlay_owner.unwrap_or(self.action_window);
        let opener = self.window_focus(owner).map(|element| InputTarget {
            window: owner,
            element,
        });
        self.end_borrowed_text_session();
        if !self.demo.palette_session.exchange_in(
            owner,
            opener,
            &mut self.search_state,
            TextInputState::default(),
        ) {
            return;
        }
        self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
        self.close_popover_for(self.action_window, PopupCloseReason::FocusTransfer);
        self.close_combo_for(self.action_window, PopupCloseReason::FocusTransfer);
        self.demo.palette = Default::default();
        self.demo.palette_scroll_node = None;
        self.demo.palette_drag = None;
        self.overlay_owner = Some(owner);
        self.cancel_window_workspace_drag(owner);
        self.begin_text_selection(owner, TextFieldKind::Search, None);
    }

    pub(super) fn close_command_palette(&mut self) {
        if !self.demo.palette_session.is_open() {
            return;
        }
        let owner = self
            .demo
            .palette_session
            .owner()
            .unwrap_or(STANDALONE_WINDOW);
        self.finish_command_palette(owner, PopupCloseReason::Explicit);
    }
}

impl App {
    /// Retire a closing page's owners before another queued event sees the old route.
    pub(super) fn retire_workspace_page_input(&mut self, item: ItemId) {
        if self.demo.workspace.item(item).is_none() {
            return;
        }
        let owners = std::iter::once(STANDALONE_WINDOW)
            .chain(self.native_windows.values().map(|host| host.ui_window))
            .collect::<Vec<_>>();
        for owner in owners {
            if let Some(gallery) = self.gallery_scopes(owner) {
                self.retire_window_subtree_input(owner, page_scroll_id(gallery, item));
            }
        }
    }

    /// Model changes retire owners before queued input can reuse the old route.
    pub(super) fn retire_window_subtree_input(&mut self, owner: UiWindowId, root: ElementId) {
        let affected = self.window_actions(owner).map(|actions| {
            let focus = self
                .window_focus(owner)
                .is_some_and(|element| actions.is_descendant_of(element, root));
            let capture = self.window_pointer_capture(owner).is_some_and(|capture| {
                capture.target.window == owner
                    && actions.is_descendant_of(capture.target.element, root)
            });
            (focus, capture)
        });
        if owner == STANDALONE_WINDOW {
            self.pointer_click.retain_live(owner, |element| {
                !self.ui_actions.is_descendant_of(element, root)
            });
        } else if let Some(host) = self
            .native_windows
            .values_mut()
            .find(|host| host.ui_window == owner)
        {
            host.pointer_click.retain_live(owner, |element| {
                !host.actions.is_descendant_of(element, root)
            });
        }
        match affected {
            Some((true, _)) => {
                self.cancel_window_editors_for(owner, PopupCloseReason::OwnerRemoved)
            }
            Some((false, true)) => {
                self.cancel_window_workspace_drag(owner);
                self.cancel_window_value_edits(owner);
                if owner == STANDALONE_WINDOW {
                    self.input_ownership.cancel_capture();
                } else if let Some(host) = self
                    .native_windows
                    .values_mut()
                    .find(|host| host.ui_window == owner)
                {
                    host.input.cancel_capture();
                }
            }
            _ => {}
        }
    }

    fn window_pointer_capture(&self, window: UiWindowId) -> Option<vesta_gpui::PointerCapture> {
        if window == STANDALONE_WINDOW {
            return self.input_ownership.capture();
        }
        self.native_windows
            .values()
            .find(|host| host.ui_window == window)
            .and_then(|host| host.input.capture())
    }

    fn cancel_focus_capture_preview(
        &mut self,
        window: UiWindowId,
        captured: Option<vesta_gpui::PointerCapture>,
        next: Option<ElementId>,
    ) {
        if captured.is_some_and(|capture| {
            capture.target.window == window && Some(capture.target.element) != next
        }) {
            // Restore before the old field's exit can commit its last drag draft.
            self.cancel_window_value_edits(window);
        }
    }

    /// Text selection replaces any old gesture, even for the same field.
    pub(super) fn prepare_text_selection_capture(&mut self, window: UiWindowId) {
        let captured = self.window_pointer_capture(window);
        self.cancel_focus_capture_preview(window, captured, None);
    }

    /// The caller has admitted the destination window and field. A caret transfer
    /// ends the old capture before the shared field transition can publish a draft.
    pub(super) fn transition_window_text_session(
        &mut self,
        window: UiWindowId,
        field: Option<TextFieldKind>,
    ) {
        if let Some(previous) = self.text_session.filter(|session| session.window != window) {
            let captured = self.window_pointer_capture(previous.window);
            self.cancel_focus_capture_preview(previous.window, captured, None);
            if previous.window == STANDALONE_WINDOW {
                self.input_ownership.cancel_capture();
            } else if let Some(host) = self
                .native_windows
                .values_mut()
                .find(|host| host.ui_window == previous.window)
            {
                host.input.cancel_capture();
            }
        }
        let next = field.map(|field| TextSession { window, field });
        if self.text_session != next {
            self.caret_reveal = None;
        }
        self.transition_text_field(self.text_session, next);
        self.text_session = next;
    }

    pub(super) fn set_focus_target(&mut self, element: Option<ElementId>) {
        self.cancel_inline_for_focus(STANDALONE_WINDOW, element);
        let captured = self.window_pointer_capture(STANDALONE_WINDOW);
        self.cancel_focus_capture_preview(STANDALONE_WINDOW, captured, element);
        self.text_bridge.clear_text_pointer();
        if !self.is_password_control(STANDALONE_WINDOW, element) {
            self.demo.password_revealed = false;
        }
        let next = self.text_field_at(STANDALONE_WINDOW, element);
        self.transition_window_text_session(STANDALONE_WINDOW, next);
        self.input_ownership
            .set_focus(element.map(|element| InputTarget {
                window: STANDALONE_WINDOW,
                element,
            }));
        self.sync_text_ime();
    }

    /// Moves keyboard focus inside one window; other windows are left untouched.
    pub(super) fn set_window_focus(&mut self, window: UiWindowId, element: Option<ElementId>) {
        if window == STANDALONE_WINDOW {
            self.set_focus_target(element);
            return;
        }
        if self.window_scopes(window).is_none() {
            return;
        }
        self.cancel_inline_for_focus(window, element);
        let captured = self.window_pointer_capture(window);
        self.cancel_focus_capture_preview(window, captured, element);
        if let Some(bridge) = self.native_text_bridges.get_mut(window) {
            bridge.clear_text_pointer();
        }
        let next = self.text_field_at(window, element);
        self.transition_window_text_session(window, next);
        if !self.is_password_control(window, element) {
            self.demo.password_revealed = false;
        }
        if let Some(host) = self
            .native_windows
            .values_mut()
            .find(|host| host.ui_window == window)
        {
            host.input
                .set_focus(element.map(|element| InputTarget { window, element }));
        }
        self.sync_text_ime_for(window);
    }

    /// Resolve field identity through the authoritative window projection.
    pub(super) fn gallery_scopes(&self, window: UiWindowId) -> Option<ElementScope> {
        self.window_scopes(window).map(|scopes| scopes.gallery)
    }
    pub(super) fn window_scopes(&self, window: UiWindowId) -> Option<UiScopes> {
        if window == STANDALONE_WINDOW {
            return Some(self.scopes);
        }
        self.native_windows
            .values()
            .find(|host| host.ui_window == window)
            .map(|host| host.scopes)
    }
    pub(super) fn observe_window_focus_input(
        &mut self,
        window: UiWindowId,
        input: vesta_gpui::FocusInput,
    ) -> bool {
        if self.window_scopes(window).is_some() {
            self.dock_focus = None;
        }
        if window == STANDALONE_WINDOW {
            return self.input_ownership.observe_focus_input(input);
        }
        self.native_windows
            .values_mut()
            .find(|host| host.ui_window == window)
            .is_some_and(|host| host.input.observe_focus_input(input))
    }

    pub(super) fn sync_text_ime(&mut self) {
        self.sync_text_ime_for(STANDALONE_WINDOW);
    }

    /// Ends the IME session of one window without touching the other windows.
    pub(super) fn end_window_ime_session(&mut self, window: UiWindowId) {
        if window == STANDALONE_WINDOW {
            self.text_bridge.clear_ime_session();
            if let Some(window) = &self.window {
                self.text_bridge.sync_ime(window, false, None);
            }
            return;
        }
        if let Some(bridge) = self.native_text_bridges.get_mut(window) {
            bridge.clear_ime_session();
            if let Some(host) = self
                .native_windows
                .values()
                .find(|host| host.ui_window == window)
            {
                bridge.sync_ime(&host.window, false, None);
            }
        }
    }

    /// Syncs the OS IME request for the window that owns the caret.
    pub(super) fn sync_text_ime_for(&mut self, window: UiWindowId) {
        if self.text_session_field(window) != Some(TextFieldKind::Search) {
            self.dismiss_suggestions(window);
        }
        let view = self
            .text_session_field(window)
            .filter(|field| *field != TextFieldKind::Label)
            .and_then(|field| self.text_view_for(window, field));
        if window == STANDALONE_WINDOW {
            if let Some(window) = &self.window {
                self.text_bridge.sync_ime(window, view.is_some(), view);
            }
            return;
        }
        if let Some(host) = self
            .native_windows
            .values()
            .find(|host| host.ui_window == window)
            && let Some(bridge) = self.native_text_bridges.get_mut(window)
        {
            bridge.sync_ime(&host.window, view.is_some(), view);
        }
    }
}

/// Input geometry belongs to the same prepared CPU projection as hit/scroll.
/// Renderer submission may defer without replacing that projection.
pub(super) fn text_views_from_paint(
    window: UiWindowId,
    gallery: ElementScope,
    paint: &PaintList,
) -> Result<TextViews, vesta_gpui::TextInputViewError> {
    let entries = paint
        .commands()
        .iter()
        .filter_map(|command| match command {
            vesta_gpui::PaintCommand::TextInput {
                element,
                bounds,
                clip,
                snapshot,
                ..
            } => Some((
                *element,
                snapshot.viewport(*bounds, *clip, Point::default()),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    text_views_for(window, gallery, &entries)
}

pub(super) fn text_views_for(
    window: UiWindowId,
    gallery: ElementScope,
    entries: &[(ElementId, vesta_gpui::TextInputViewport)],
) -> Result<TextViews, vesta_gpui::TextInputViewError> {
    TextViews::from_layout(window, entries, |element| {
        TextFieldKind::ALL
            .into_iter()
            .find(|field| field.input_element(gallery) == element)
    })
}

#[derive(Clone, Copy)]
pub(super) enum TextSessionExit {
    FocusChange,
    WindowBlur(UiWindowId),
}

pub(super) fn reveal_scroll_path(
    gallery: ElementScope,
    geometry: &ScrollGeometry,
    states: &mut ScrollStates<'_>,
    path: &[ElementId],
    target: Rect,
) -> bool {
    let viewports = geometry.targets(gallery);
    let steps = scroll_reveal_path(path, target, |element| {
        let (_, viewport, kind) = viewports.iter().find(|(id, _, _)| *id == element)?;
        states.for_target(*kind)?;
        Some(ScrollRevealViewport {
            rect: geometry
                .unclipped
                .get(&element)
                .copied()
                .unwrap_or(*viewport),
            margin: match kind {
                ScrollTarget::Tree | ScrollTarget::VirtualList | ScrollTarget::VirtualTable => 2.0,
                _ => 8.0,
            },
        })
    });
    let mut changed = false;
    for step in steps {
        let Some((_, _, kind)) = viewports.iter().find(|(id, _, _)| *id == step.element) else {
            continue;
        };
        let Some(state) = states.for_target(*kind) else {
            continue;
        };
        changed |= state.reveal(step.target, step.viewport.rect, step.viewport.margin);
    }
    changed
}

/// One source's current layout/paint projection; no cached caret rectangle is an input.
pub(super) struct CaretRevealFrame<'a> {
    pub window: UiWindowId,
    pub gallery: ElementScope,
    pub actions: &'a ActionRoute<DemoAction>,
    pub focus: Option<InputTarget>,
    pub focus_root: Option<ElementId>,
    pub geometry: &'a ScrollGeometry,
    pub paint: &'a PaintList,
    pub layout_stable: bool,
}

/// A later manual scroll in the same source supersedes an older edit reveal.
pub(super) fn retire_caret_reveal(pending: &mut Option<InputTarget>, window: UiWindowId) {
    *pending = pending.filter(|target| target.window != window);
}

pub(super) fn complete_caret_reveal(
    pending: &mut Option<InputTarget>,
    session: Option<TextSession>,
    frame: CaretRevealFrame<'_>,
    states: &mut ScrollStates<'_>,
) -> bool {
    let Some(target) = *pending else {
        return false;
    };
    let Some(session) = session.filter(|session| session.window == target.window) else {
        *pending = None;
        return false;
    };
    if target.window != frame.window || !frame.layout_stable {
        return false;
    }
    *pending = None;
    if frame.focus != Some(target) || session.field.element(frame.gallery) != target.element {
        return false;
    }
    let scope = FocusScope::new(frame.window, frame.actions, frame.focus_root);
    if !scope.accepts(target) {
        return false;
    }
    let Some(path) = scope.path_to(target) else {
        return false;
    };
    let input = session.field.input_element(frame.gallery);
    let caret = frame
        .paint
        .commands()
        .iter()
        .find_map(|command| match command {
            vesta_gpui::PaintCommand::TextInput {
                element,
                bounds,
                clip,
                snapshot,
                focused: true,
                ..
            } if *element == input => snapshot.viewport(*bounds, *clip, Point::default()).caret,
            _ => None,
        });
    caret.is_some_and(|caret| {
        reveal_scroll_path(frame.gallery, frame.geometry, states, &path, caret)
    })
}

fn focus_reveal_target(
    gallery: ElementScope,
    geometry: &ScrollGeometry,
    rects: &HashMap<ElementId, Rect>,
    actions: &ActionRoute<DemoAction>,
    element: ElementId,
    explicit_rect: Option<Rect>,
) -> Option<(Rect, Vec<ElementId>)> {
    let mut path = actions.path_to(element)?;
    if let Some(rect) = explicit_rect.or_else(|| rects.get(&element).copied()) {
        return Some((rect, path));
    }
    // Virtual rows can be new to this frame. Reveal their live viewport through its
    // outer ancestors, without changing the inner offset that selected the row.
    let (_, clipped, _) = geometry
        .targets(gallery)
        .into_iter()
        .find(|(id, _, _)| *id == element)?;
    let rect = geometry.unclipped.get(&element).copied().unwrap_or(clipped);
    path.retain(|ancestor| *ancestor != element);
    Some((rect, path))
}

impl App {
    fn reset_window_text_bridge(&mut self, window: UiWindowId) {
        if window == STANDALONE_WINDOW {
            self.text_bridge.reset();
        } else if let Some(bridge) = self.native_text_bridges.get_mut(window) {
            bridge.reset();
        }
    }

    pub(super) fn finish_window_text_session(&mut self, window: UiWindowId) {
        self.reset_window_text_bridge(window);
        if self.text_session.is_none() {
            return;
        }
        if !self
            .text_session
            .is_some_and(|session| session.window == window)
        {
            return;
        }
        self.transition_text_field(self.text_session, None);
        self.clear_text_session();
    }

    pub(super) fn complete_window_text_event(
        &mut self,
        exit: Option<TextSessionExit>,
        number_dirty: bool,
        vector_axis: Option<u8>,
    ) {
        // A cancelled scrub has already restored the value. Publish that restored
        // draft before the field transition can commit the previous drag draft.
        if number_dirty {
            self.reset_number_draft();
        }
        if let Some(axis) = vector_axis {
            self.reset_vector_draft(axis);
        }
        match exit {
            Some(TextSessionExit::FocusChange) => {
                self.transition_text_field(self.text_session, None);
                self.clear_text_session();
            }
            Some(TextSessionExit::WindowBlur(window)) => self.finish_window_text_session(window),
            None => {}
        }
    }

    pub(super) fn focus_root_for_window(&self, window: UiWindowId) -> Option<ElementId> {
        if !self.owns_overlay(window) {
            return None;
        }
        let scopes = self.window_scopes(window)?;
        self.demo
            .dialog_stack
            .top()
            .map(|id| dialog_element(scopes.root, id, 0))
            .or_else(|| {
                self.demo
                    .palette_session
                    .is_open()
                    .then(|| scopes.root.element(PALETTE_ROOT))
            })
    }

    pub(super) fn request_window_focus_traversal(&mut self, window: UiWindowId, backwards: bool) {
        let captured = self.window_pointer_capture(window);
        self.cancel_focus_capture_preview(window, captured, None);
        if window == STANDALONE_WINDOW {
            self.input_ownership
                .request_focus_traversal(window, backwards);
        } else if let Some(host) = self
            .native_windows
            .values_mut()
            .find(|host| host.ui_window == window)
        {
            host.input.request_focus_traversal(window, backwards);
        }
    }

    pub(super) fn traverse_window_focus(&mut self, window: UiWindowId, backwards: bool) {
        let captured = self.window_pointer_capture(window);
        let root = self.focus_root_for_window(window);
        if window == STANDALONE_WINDOW {
            FocusScope::new(window, &self.ui_actions, root)
                .traverse(&mut self.input_ownership, backwards);
        } else if let Some(host) = self
            .native_windows
            .values_mut()
            .find(|host| host.ui_window == window)
        {
            FocusScope::new(window, &host.actions, root).traverse(&mut host.input, backwards);
        } else {
            return;
        }
        let element = self.window_focus(window);
        self.cancel_focus_capture_preview(window, captured, element);
        if !self.is_password_control(window, element) {
            self.demo.password_revealed = false;
        }
        let next = self.text_field_at(window, element);
        self.transition_window_text_session(window, next);
        if next == Some(TextFieldKind::Area) {
            self.follow_area_caret(window);
        }
        if let Some(element) = element {
            self.reveal_window_focus(window, element);
        }
        self.sync_text_ime_for(window);
    }

    pub(super) fn reveal_window_focus(&mut self, window: UiWindowId, element: ElementId) {
        let document_focus = self.window_document_reveal_rect(window, element);
        self.reveal_window_target(window, element, document_focus);
    }

    /// Restore against the admitted frame without publishing its geometry before paint.
    pub(super) fn reveal_current_layout_focus(
        &mut self,
        window: UiWindowId,
        gallery: ElementScope,
        layout: &LayoutTree,
        actions: &ActionRoute<DemoAction>,
        regions: &WorkspaceViewRegions,
        element: ElementId,
    ) {
        let target = InputTarget { window, element };
        if self.gallery_scopes(window) != Some(gallery)
            || !FocusScope::new(window, actions, self.focus_root_for_window(window)).accepts(target)
        {
            return;
        }
        let Some(node) = layout.node(element) else {
            return;
        };
        let target_rect = self
            .window_document_reveal_rect(window, element)
            .unwrap_or(node.rect);
        let pages = regions
            .panes
            .iter()
            .filter_map(|region| {
                self.demo
                    .workspace
                    .pane(region.pane)?
                    .active_item()
                    .map(|item| item.id)
            })
            .collect::<Vec<_>>();
        let geometry = ScrollGeometry::from_layout(gallery, layout, pages.iter().copied())
            .with_tabs(layout, regions);
        let Some((rect, path)) = focus_reveal_target(
            gallery,
            &geometry,
            &HashMap::new(),
            actions,
            element,
            Some(target_rect),
        ) else {
            return;
        };
        for item in geometry.pages.keys() {
            self.demo.page_scroll.entry(*item).or_default();
        }
        for (pane, _) in geometry.tabs.values() {
            self.demo.tab_scroll.entry(*pane).or_default();
        }
        self.apply_focus_reveal(window, gallery, &geometry, rect, &path, true);
    }

    pub(super) fn reveal_window_rect(
        &mut self,
        window: UiWindowId,
        element: ElementId,
        rect: Rect,
    ) {
        self.reveal_window_target(window, element, Some(rect));
    }

    fn reveal_window_target(&mut self, window: UiWindowId, element: ElementId, rect: Option<Rect>) {
        let data = if window == STANDALONE_WINDOW {
            focus_reveal_target(
                self.scopes.gallery,
                &self.scroll_geometry,
                &self.focus_rects,
                &self.ui_actions,
                element,
                rect,
            )
            .map(|(rect, path)| {
                (
                    self.scopes.gallery,
                    self.scroll_geometry.clone(),
                    rect,
                    path,
                )
            })
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .and_then(|host| {
                    focus_reveal_target(
                        host.scopes.gallery,
                        &host.scroll_geometry,
                        &host.focus_rects,
                        &host.actions,
                        element,
                        rect,
                    )
                    .map(|(rect, path)| {
                        (
                            host.scopes.gallery,
                            host.scroll_geometry.clone(),
                            rect,
                            path,
                        )
                    })
                })
        };
        let Some((gallery, geometry, target, path)) = data else {
            return;
        };
        self.apply_focus_reveal(window, gallery, &geometry, target, &path, false);
    }

    fn apply_focus_reveal(
        &mut self,
        window: UiWindowId,
        gallery: ElementScope,
        geometry: &ScrollGeometry,
        target: Rect,
        path: &[ElementId],
        refresh_metrics: bool,
    ) {
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
        if refresh_metrics {
            for (element, _, kind) in geometry.targets(gallery) {
                if path.contains(&element)
                    && let Some(node) = geometry.nodes.get(&element)
                    && let Some(state) = states.for_target(kind)
                {
                    state.sync_layout(node);
                }
            }
        }
        reveal_scroll_path(gallery, geometry, &mut states, path, target);
    }
}

#[cfg(test)]
mod tests;

impl App {
    pub(super) fn focused_text_field(&self) -> Option<TextFieldKind> {
        self.text_session_field(STANDALONE_WINDOW)
    }

    /// The focused element of one window.
    pub(super) fn window_focus(&self, window: UiWindowId) -> Option<ElementId> {
        if window == STANDALONE_WINDOW {
            return self.input_ownership.focus().map(|focus| focus.element);
        }
        self.native_windows
            .values()
            .find(|host| host.ui_window == window)
            .and_then(|host| host.input.focus())
            .map(|focus| focus.element)
    }

    /// Reveal remains active while focus moves among the field's own controls.
    pub(super) fn is_password_control(
        &self,
        window: UiWindowId,
        element: Option<ElementId>,
    ) -> bool {
        self.gallery_scopes(window).is_some_and(|gallery| {
            [PASSWORD_ROOT, PASSWORD_ROOT + 3, PASSWORD_ROOT + 5]
                .into_iter()
                .any(|id| element == Some(gallery.element(id)))
        })
    }

    /// Whether this window should project the palette or a modal dialog it opened.
    pub(super) fn owns_overlay(&self, window: UiWindowId) -> bool {
        self.overlay_owner == Some(window)
    }

    /// The text field a window's element id names, using that window's own scopes.
    pub(super) fn text_field_at(
        &self,
        window: UiWindowId,
        element: Option<ElementId>,
    ) -> Option<TextFieldKind> {
        let element = element?;
        let scopes = self.gallery_scopes(window)?;
        TextFieldKind::ALL
            .into_iter()
            .find(|field| field.element(scopes) == element)
    }

    /// The caret owner for one window, if that window holds the text session.
    pub(super) fn text_session_field(&self, window: UiWindowId) -> Option<TextFieldKind> {
        self.text_session
            .filter(|session| session.window == window)
            .map(|session| session.field)
    }

    pub(super) fn set_text_focus(&mut self, field: Option<TextFieldKind>) {
        self.set_focus_target(field.map(|field| field.element(self.scopes.gallery)));
    }

    #[cfg(test)]
    pub(super) fn traverse_focus(&mut self, backwards: bool) {
        self.traverse_window_focus(STANDALONE_WINDOW, backwards);
    }

    pub(super) fn accessibility_action(&mut self, request: accesskit::ActionRequest) {
        self.window_accessibility_action(STANDALONE_WINDOW, request);
    }

    /// Press on a text field of the main window; native floating windows call
    /// [`App::begin_text_selection`] with their own window and pointer.
    pub(super) fn start_text_selection(&mut self, field: TextFieldKind) {
        self.begin_text_selection(STANDALONE_WINDOW, field, Some(self.cursor));
    }
}
