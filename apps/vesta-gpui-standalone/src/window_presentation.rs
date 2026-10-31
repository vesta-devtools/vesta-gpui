//! Presentation of the main acceptance window.
use super::*;

impl App {
    pub(super) fn redraw(&mut self) {
        let _trace_frame = vesta_gpui::trace::frame("Standalone.Main");
        if self.gpu.as_ref().is_none_or(|gpu| !gpu.available()) {
            return;
        }
        // Initial floating-window activation can blur the main window. Start
        // the scripted popup sequence only after those asynchronous factories
        // have finished; real windows continue drawing during creation.
        if self.smoke
            && self.rendered_frames == 0
            && (std::env::args().any(|a| a == "--smoke-floating") || gpu_lifecycle::enabled())
            && self
                .demo
                .workspace
                .floating_windows()
                .iter()
                .any(|logical| {
                    !self
                        .native_windows
                        .values()
                        .any(|host| host.logical_id == logical.id && host.surface.available())
                })
        {
            return;
        }
        let mut frame_trace = self.frame_profile.begin();
        let input_watermark = self.lifecycle_profile.begin_frame();
        let mut layout_passes = 0;
        loop {
            if layout_passes == FRAME_LAYOUT_PASS_LIMIT {
                vesta_gpui::trace::counter(
                    "standalone.frame",
                    "layout.passes",
                    f64::from(layout_passes),
                );
                vesta_gpui::trace::instant("standalone.frame", "frame.skipped.layout_budget");
                self.request_redraw();
                return;
            }
            layout_passes += 1;
            let layout_pass = vesta_gpui::trace::span("standalone.frame", "layout.pass");
            self.reconcile_workspace_gestures();
            self.reconcile_window_menu(STANDALONE_WINDOW);
            self.prepare_suggestions(STANDALONE_WINDOW);
            let focus_root = self.focus_root_for_window(STANDALONE_WINDOW);
            let focused_field = self.focused_text_field();
            let virtual_focused = self.focused_virtual_index();
            let virtual_table_focused = self.focused_virtual_table_index();
            let tree_focused = self.focused_tree_index();
            let dock_drag = self.dock_drag_visual(ContainerRef::Main);
            let paint_interaction = self.paint_interaction();
            let isolated_accessibility_root = self
                .demo
                .dialog_stack
                .top()
                .map(|id| dialog_element(self.scopes.root, id, 0));
            let tooltip_focus_root = self.focus_root_for_window(STANDALONE_WINDOW);
            let password_snapshot = self.password_display_snapshot();
            let table_note_snapshot = self.table_note_state.snapshot();
            let Some(window) = self.window.clone() else {
                return;
            };
            let size = window.inner_size();
            if size.width == 0 || size.height == 0 {
                return;
            }
            let scale = window.scale_factor() as f32;
            let logical = size.to_logical::<f32>(window.scale_factor());
            let viewport = Viewport::new(logical.width, logical.height, scale);
            self.area_layout
                .prepare(&mut self.area_state, &mut self.text_context);
            let mut built = build_root(
                UiBuildState {
                    documents: &self.documents,
                    selected_feedback: native_capture::selected_feedback_enabled(self),
                    ui_window: STANDALONE_WINDOW,
                    menu_open: self.demo.menu.is_open(),
                    scopes: self.scopes,
                    workspace_container: ContainerRef::Main,
                    mode: self.mode,
                    maximized: window.is_maximized(),
                    theme: self.theme,
                    scroll_offset: self.gallery_scroll.offset(),
                    viewport_size: Size::new(logical.width, logical.height),
                    snapshot: self.text_state.snapshot(),
                    focused: focused_field == Some(TextFieldKind::Main),
                    search_snapshot: self.search_state.snapshot(),
                    search_query: self.search_state.text(),
                    search_focused: focused_field == Some(TextFieldKind::Search),
                    area_snapshot: self.area_state.snapshot().with_viewport_scroll(
                        Point::new(0.0, self.area_scroll_y),
                        self.area_follow_caret,
                    ),
                    area_focused: focused_field == Some(TextFieldKind::Area),
                    number_snapshot: self.number_state.snapshot(),
                    number_focused: focused_field == Some(TextFieldKind::Number),
                    vector_snapshots: std::array::from_fn(|axis| {
                        self.vector_states[axis].snapshot()
                    }),
                    vector_focused: std::array::from_fn(|axis| {
                        focused_field == Some(TextFieldKind::Vector(axis as u8))
                    }),
                    color_hex_snapshot: self.color_hex_state.snapshot(),
                    password_mask: self.password_state.masked_value(),
                    password_snapshot,
                    table_note_snapshot,
                    color_hex_focused: focused_field == Some(TextFieldKind::ColorHex),
                    table_note_focused: focused_field == Some(TextFieldKind::TableNote),
                    readonly_label: &self.readonly_label_state,
                    readonly_label_focused: focused_field == Some(TextFieldKind::Label),
                    virtual_focused,
                    virtual_table_focused,
                    virtual_table_viewport_height: window_collections::table_viewport_height(
                        &self.scroll_geometry,
                        self.scopes.gallery,
                    ),
                    tree_focused,
                    demo: &self.demo,
                    hovered: self.hovered,
                    focused_element: self.input_ownership.focus().map(|focus| focus.element),
                    dock_drag,
                    popover_anchor: self
                        .focus_rects
                        .get(&self.scopes.gallery.element(POPOVER_TRIGGER))
                        .copied(),
                    combo_anchor: self
                        .focus_rects
                        .get(&self.scopes.gallery.element(COMBO_TRIGGER))
                        .copied(),
                    gizmo_mode: self.scene.gizmo.mode,
                    frame_stats: self.frame_meter.display,
                    overlay_owner: self.overlay_owner,
                },
                &mut self.text_context,
            );
            self.tile_columns = built.tile_columns;
            if self.smoke && self.mode == AcceptanceMode::Gallery2d {
                demo_images::append_smoke_image(&mut built.root, self.scopes.gallery);
            }
            frame_trace.mark(FrameStage::Build);
            let Ok(actions) = ActionRoute::build(&built.root, built.actions.iter().copied()) else {
                eprintln!("failed to route UI actions: duplicate or unknown element identity");
                return;
            };
            frame_trace.mark(FrameStage::Route);
            let window_active = window.has_focus();
            let base_children = built.root.children.len();
            let Ok(layout) = self.layout_main_tooltips(
                &mut built.root,
                &actions,
                viewport,
                window_active,
                tooltip_focus_root,
            ) else {
                return;
            };
            let Ok(actions) = window_tooltips::route_after_tooltip(
                &built.root,
                base_children,
                actions,
                built.actions,
            ) else {
                eprintln!("failed to route final tooltip tree: invalid UI identity");
                return;
            };
            frame_trace.mark(FrameStage::Layout);
            match self.documents.sync_layout(
                self.scopes.markdown,
                &layout,
                &mut self.text_context,
                self.theme,
            ) {
                Ok(true) => {
                    continue;
                }
                Ok(false) => {}
                Err(error) => {
                    self.fatal_error = Some(error.to_string());
                    return;
                }
            }
            if self.documents.sync_input(
                STANDALONE_WINDOW,
                self.scopes.markdown,
                &layout,
                &mut self.input_ownership,
            ) {
                continue;
            }
            let popup_scope = FocusScope::new(STANDALONE_WINDOW, &actions, None);
            let combo_closed = self
                .demo
                .combo
                .reconcile(STANDALONE_WINDOW, &popup_scope)
                .is_some();
            let popover_closed = self
                .demo
                .popover
                .reconcile(STANDALONE_WINDOW, &popup_scope)
                .is_some();
            if combo_closed || popover_closed {
                continue;
            }
            let scope = FocusScope::new(STANDALONE_WINDOW, &actions, focus_root);
            let focus_native = DockFocusTransfer::request(
                &mut self.dock_focus,
                &self.demo.workspace,
                ContainerRef::Main,
                STANDALONE_WINDOW,
                &actions,
                focus_root,
                &mut self.input_ownership,
            );
            if let Some(restored) = scope.resolve_request(&mut self.input_ownership) {
                if focus_native && restored.is_some() {
                    window.focus_window();
                }
                if let Some(target) = restored {
                    self.reveal_current_layout_focus(
                        STANDALONE_WINDOW,
                        self.scopes.gallery,
                        &layout,
                        &actions,
                        &built.workspace_regions,
                        target.element,
                    );
                }
                self.ui_actions = actions;
                self.set_focus_target(restored.map(|target| target.element));
                continue;
            }
            let reconciled = scope.reconcile(&mut self.input_ownership);
            if reconciled.lost_focus || reconciled.lost_capture {
                self.ui_actions = actions;
                self.pointer_click.cancel();
                if reconciled.lost_focus {
                    self.cancel_window_editors_for(
                        STANDALONE_WINDOW,
                        PopupCloseReason::OwnerRemoved,
                    );
                } else {
                    self.cancel_window_workspace_drag(STANDALONE_WINDOW);
                    self.cancel_window_value_edits(STANDALONE_WINDOW);
                }
                continue;
            }
            self.ui_actions = actions;
            if self.smoke
                && !window_documents::smoke_enabled()
                && self.mode == AcceptanceMode::Gallery2d
                && self.rendered_frames == 1
                && self
                    .ui_actions
                    .action_at(self.scopes.popover.element(POPOVER_BUTTON))
                    != Some(&DemoAction::PopoverStep)
            {
                self.fatal_error = Some("smoke popover child action was not projected".into());
            }
            if self.smoke
                && !window_documents::smoke_enabled()
                && self.mode == AcceptanceMode::Gallery2d
                && self.rendered_frames == 2
                && self
                    .ui_actions
                    .action_at(self.scopes.gallery.element(COMBO_OPTION_BASE))
                    != Some(&DemoAction::SelectCombo(0))
            {
                self.fatal_error = Some("smoke combo options were not projected".into());
            }
            let scroll_id = self.scopes.gallery.element(3);
            self.gallery_scroll_rect = layout
                .node(scroll_id)
                .and_then(|node| node.rect.intersection(viewport.logical_rect()));
            self.page_scroll_rects.clear();
            let mut page_changed = false;
            let active_pages = self
                .demo
                .workspace
                .panes()
                .into_iter()
                .map(|pane| pane.id)
                .chain(
                    self.demo
                        .workspace
                        .floating_windows()
                        .iter()
                        .flat_map(|window| window.root.panes()),
                )
                .filter_map(|pane| {
                    self.demo
                        .workspace
                        .pane(pane)?
                        .active_item()
                        .map(|item| item.id)
                })
                .collect::<Vec<_>>();
            // One capture feeds both the legacy per-field rects and the shared geometry the
            // scrollbar and wheel paths use, so the two windows agree on viewports.
            let previous_table_height = window_collections::table_viewport_height(
                &self.scroll_geometry,
                self.scopes.gallery,
            );
            self.scroll_geometry = ScrollGeometry::from_layout(
                self.scopes.gallery,
                &layout,
                active_pages.iter().copied(),
            )
            .with_tabs(&layout, &built.workspace_regions);
            let table_viewport_changed = previous_table_height
                != window_collections::table_viewport_height(
                    &self.scroll_geometry,
                    self.scopes.gallery,
                );
            page_changed |= (ScrollStates {
                tree: &mut self.demo.tree_scroll,
                virtual_list: &mut self.demo.virtual_scroll,
                virtual_table: &mut self.demo.virtual_table_scroll,
                nested: &mut self.demo.nested_scroll,
                horizontal: &mut self.demo.horizontal_scroll,
                gallery: Some(&mut self.gallery_scroll),
                pages: &mut self.demo.page_scroll,
                tabs: &mut self.demo.tab_scroll,
            })
            .sync_layout(self.scopes.gallery, &self.scroll_geometry, &layout);
            for item in active_pages {
                if let Some(node) = layout.node(page_scroll_id(self.scopes.gallery, item)) {
                    let rect = node
                        .rect
                        .intersection(node.inherited_clip)
                        .unwrap_or(node.rect);
                    self.page_scroll_rects.insert(item, rect);
                    page_changed |= self
                        .demo
                        .page_scroll
                        .entry(item)
                        .or_default()
                        .sync_layout(node);
                }
            }
            let nested_id = self.scopes.gallery.element(NESTED_SCROLL);
            self.nested_scroll_rect = layout.node(nested_id).and_then(|node| {
                node.rect
                    .intersection(node.inherited_clip)
                    .or(Some(node.rect))
            });
            let horizontal_id = self.scopes.gallery.element(HORIZONTAL_SCROLL);
            self.horizontal_scroll_rect = layout.node(horizontal_id).and_then(|node| {
                node.rect
                    .intersection(node.inherited_clip)
                    .or(Some(node.rect))
            });
            let virtual_id = self.scopes.gallery.element(VIRTUAL_SCROLL);
            self.virtual_scroll_rect = layout.node(virtual_id).and_then(|node| {
                node.rect
                    .intersection(node.inherited_clip)
                    .or(Some(node.rect))
            });
            let virtual_table_id = self.scopes.gallery.element(TABLE_VIRTUAL_ROOT + 2);
            self.virtual_table_scroll_rect = layout.node(virtual_table_id).and_then(|node| {
                node.rect
                    .intersection(node.inherited_clip)
                    .or(Some(node.rect))
            });
            let tree_id = self.scopes.gallery.element(TREE_SCROLL);
            self.tree_scroll_rect = layout.node(tree_id).and_then(|node| {
                node.rect
                    .intersection(node.inherited_clip)
                    .or(Some(node.rect))
            });
            let nested_changed = layout
                .node(nested_id)
                .is_some_and(|node| self.demo.nested_scroll.sync_layout(node));
            let horizontal_changed = layout
                .node(horizontal_id)
                .is_some_and(|node| self.demo.horizontal_scroll.sync_layout(node));
            let virtual_changed = layout
                .node(virtual_id)
                .is_some_and(|node| self.demo.virtual_scroll.sync_layout(node));
            let virtual_table_changed = layout
                .node(virtual_table_id)
                .is_some_and(|node| self.demo.virtual_table_scroll.sync_layout(node));
            let tree_changed = layout
                .node(tree_id)
                .is_some_and(|node| self.demo.tree_scroll.sync_layout(node));
            let gallery_changed = layout
                .node(scroll_id)
                .is_some_and(|node| self.gallery_scroll.sync_layout(node));
            self.tab_scroll_rects.clear();
            let mut tab_changed = false;
            let pane_width_changed =
                self.demo
                    .sync_pane_widths(built.workspace_regions.panes.iter().filter_map(|region| {
                        layout
                            .node(region.root)
                            .map(|root| (region.pane, root.rect.width))
                    }));
            for region in &built.workspace_regions.panes {
                if let Some(list) = layout.node(region.tab_list) {
                    self.tab_scroll_rects
                        .insert(region.pane, list.scroll_viewport());
                    let scroll = self.demo.tab_scroll.entry(region.pane).or_default();
                    tab_changed |= scroll.sync_layout(list);
                }
            }
            tab_changed |= reveal_active_tabs(
                &built.workspace_regions,
                &layout,
                &self.demo.workspace,
                &mut self.demo.tab_scroll,
                &mut self.tab_reveal,
            );
            let layout_changed = nested_changed
                || page_changed
                || horizontal_changed
                || virtual_changed
                || virtual_table_changed
                || table_viewport_changed
                || tree_changed
                || gallery_changed
                || tab_changed
                || pane_width_changed;
            let wrap_changed = self.sync_area_wrap_layout(self.scopes.gallery, &layout);
            let old_combo_anchor = self
                .focus_rects
                .get(&self.scopes.gallery.element(COMBO_TRIGGER))
                .copied();
            let old_popover_anchor = self
                .focus_rects
                .get(&self.scopes.gallery.element(POPOVER_TRIGGER))
                .copied();
            self.focus_rects = self
                .ui_actions
                .focus_order()
                .iter()
                .copied()
                .filter(|id| self.ui_actions.is_descendant_of(*id, scroll_id))
                .filter_map(|id| layout.node(id).map(|node| (id, node.rect)))
                .collect();
            let mut anchor_changed = false;
            if self.demo.popover.is_open()
                && self
                    .focus_rects
                    .get(&self.scopes.gallery.element(POPOVER_TRIGGER))
                    .copied()
                    != old_popover_anchor
            {
                anchor_changed = true;
            }
            if self
                .demo
                .combo
                .opener()
                .is_some_and(|opener| opener.window == STANDALONE_WINDOW)
                && self
                    .focus_rects
                    .get(&self.scopes.gallery.element(COMBO_TRIGGER))
                    .copied()
                    != old_combo_anchor
            {
                anchor_changed = true;
            }
            self.slider_rect = built
                .slider
                .and_then(|id| layout.node(id).map(|node| node.rect));
            self.sync_palette_layout(STANDALONE_WINDOW, self.scopes.root, &layout);
            self.sync_suggestion_layout(STANDALONE_WINDOW, self.scopes.gallery, &layout);
            self.ramp_track_rect = built
                .ramp_track
                .and_then(|id| layout.node(id).map(|node| node.rect));
            self.table_rect = layout
                .node(self.scopes.gallery.element(TABLE_ROOT))
                .map(|node| node.rect);
            self.workspace_rect = layout
                .node(self.scopes.workspace.element(1))
                .map(|node| node.rect);
            self.dock_frame =
                DockFrame::from_layout(&built.workspace_regions, &layout, self.workspace_rect);
            let constrained_splits = self
                .dock_frame
                .splits
                .iter()
                .filter_map(|frame| {
                    let current =
                        dock_split_ratio(&self.demo.workspace, frame.container, frame.split)?;
                    let pointer = Point::new(
                        frame.rect.x + frame.rect.width * current,
                        frame.rect.y + frame.rect.height * current,
                    );
                    let constrained =
                        split_ratio_at(frame.rect, frame.axis, pointer, MIN_DOCK_PANE_EXTENT);
                    ((constrained - current).abs() > 0.001).then_some((
                        frame.container,
                        frame.split,
                        constrained,
                    ))
                })
                .collect::<Vec<_>>();
            let splits_changed = !constrained_splits.is_empty();
            if splits_changed {
                for (container, split, ratio) in constrained_splits {
                    let _ = self.demo.workspace.set_split_ratio(container, split, ratio);
                }
            }
            let paint_focus = self
                .input_ownership
                .visible_focus()
                .map(|focus| focus.element);
            frame_trace.mark(FrameStage::Reconcile);
            if layout_changed || wrap_changed || anchor_changed || splits_changed {
                continue;
            }
            drop(layout_pass);
            vesta_gpui::trace::counter(
                "standalone.frame",
                "layout.passes",
                f64::from(layout_passes),
            );
            let paint = PaintList::build_with_focus_and_interaction(
                &built.root,
                &layout,
                paint_focus,
                self.theme.palette.focus,
                paint_interaction,
            );
            if complete_caret_reveal(
                &mut self.caret_reveal,
                self.text_session,
                CaretRevealFrame {
                    window: STANDALONE_WINDOW,
                    gallery: self.scopes.gallery,
                    actions: &self.ui_actions,
                    focus: self.input_ownership.focus(),
                    focus_root,
                    geometry: &self.scroll_geometry,
                    paint: &paint,
                    layout_stable: !(layout_changed || wrap_changed),
                },
                &mut ScrollStates {
                    tree: &mut self.demo.tree_scroll,
                    virtual_list: &mut self.demo.virtual_scroll,
                    virtual_table: &mut self.demo.virtual_table_scroll,
                    nested: &mut self.demo.nested_scroll,
                    horizontal: &mut self.demo.horizontal_scroll,
                    gallery: Some(&mut self.gallery_scroll),
                    pages: &mut self.demo.page_scroll,
                    tabs: &mut self.demo.tab_scroll,
                },
            ) {
                continue;
            }
            frame_trace.mark(FrameStage::Paint);
            let text_views =
                match text_views_from_paint(STANDALONE_WINDOW, self.scopes.gallery, &paint) {
                    Ok(views) => views,
                    Err(error) => {
                        self.fatal_error = Some(format!("text_input_views: {error:?}"));
                        return;
                    }
                };
            self.text_view = text_views.get(TextFieldKind::Main);
            self.search_view = text_views.get(TextFieldKind::Search);
            self.area_view = text_views.get(TextFieldKind::Area);
            if let Some(view) = self.area_view {
                self.area_scroll_y = view.scroll_y;
            }
            self.number_view = text_views.get(TextFieldKind::Number);
            self.vector_views =
                std::array::from_fn(|axis| text_views.get(TextFieldKind::Vector(axis as u8)));
            self.color_hex_view = text_views.get(TextFieldKind::ColorHex);
            self.password_view = text_views.get(TextFieldKind::Password);
            self.table_note_view = text_views.get(TextFieldKind::TableNote);
            self.readonly_label_view = text_views.get(TextFieldKind::Label);
            frame_trace.mark(FrameStage::TextViews);
            self.hit_test = HitTestTree::build(&built.root, &layout);
            let hovered = self.hit_test.hit(self.cursor);
            if self.hovered != hovered {
                self.hovered = hovered;
                self.tooltip
                    .hover(hovered, &self.ui_actions, Instant::now());
                window.request_redraw();
            }
            frame_trace.mark(FrameStage::Hit);
            self.caret_blink.update(
                focused_field
                    .filter(|_| window_active)
                    .map(|field| field.element(self.scopes.gallery)),
                Instant::now(),
            );
            if self.capture_geometry_changed(
                STANDALONE_WINDOW,
                self.scopes.gallery,
                &built.root,
                &layout,
                &self.ui_actions,
            ) {
                if let Some(gpu) = &mut self.gpu
                    && let Ok(renderer) = gpu.capture_renderer()
                {
                    renderer.cancel_capture();
                }
                self.ui_query.cancel_capture_binding(STANDALONE_WINDOW);
            }
            let Some(gpu) = self.gpu.as_mut() else {
                return;
            };
            let profile_context = FrameContext {
                window: STANDALONE_WINDOW,
                gallery: self.mode == AcceptanceMode::Gallery2d,
                text_focused: focused_field.is_some(),
                active: window_active,
            };
            let output = match gpu.render(
                None,
                RenderFrame {
                    viewport,
                    paint: &paint,
                    context: &mut self.text_context,
                    rasterizer: &mut self.text_rasterizer,
                    images: Some(&DemoImages::for_frame(self.rendered_frames)),
                    options: RenderOptions {
                        clear: (self.mode == AcceptanceMode::Gallery2d)
                            .then(|| theme_clear(self.theme, false)),
                        selection: self.theme.palette.selection,
                        caret: self.theme.palette.focus,
                        caret_visible: self.caret_blink.visible(),
                    },
                    scene: (self.mode == AcceptanceMode::Viewport3d).then_some((
                        self.camera,
                        &self.scene,
                        theme_clear(self.theme, true),
                    )),
                    finish: self.smoke
                        && !render_backend::retirement_probe_enabled()
                        && self.rendered_frames + 1
                            >= render_backend::smoke_frame_limit(
                                self.mode == AcceptanceMode::Gallery2d,
                            ),
                    cpu_profile: frame_trace.renderer_profile(),
                    gpu_profile: &mut self.gpu_profile,
                    profile_context,
                },
            ) {
                Ok(Some(output)) => output,
                Ok(None) => {
                    if self.display_blocked(window.id()) {
                        self.cancel_gpu_interactions(&std::collections::HashSet::from([
                            window.id()
                        ]));
                    }
                    return;
                }
                Err(error) => {
                    self.gpu_fault(window.id(), error);
                    return;
                }
            };
            frame_trace.mark(FrameStage::Renderer);
            self.lifecycle_profile
                .presented(STANDALONE_WINDOW, input_watermark);
            if let Err(error) =
                demo_images::verify_frame(self.rendered_frames, &paint, viewport, &output)
            {
                self.fatal_error = Some(error);
                return;
            }
            if self.smoke
                && self.mode == AcceptanceMode::Gallery2d
                && self.rendered_frames == 2
                && !demo_images::faults_enabled()
                && output.bitmap_reuses == 0
            {
                self.fatal_error = Some("settled UI frame did not reuse any bitmap texture".into());
            }
            if self.smoke && self.mode == AcceptanceMode::Gallery2d {
                if self.rendered_frames == 0
                    && output.glyph_uploads == 0
                    && output.glyph_reuses == 0
                {
                    self.fatal_error =
                        Some("first UI frame has neither uploaded nor reused atlas glyphs".into());
                }
                if self.rendered_frames == 2 && output.glyph_reuses == 0 {
                    self.fatal_error = Some("settled UI frame did not reuse atlas glyphs".into());
                }
            }
            if let Err(error) = frame_reflow_probe::verify_rendered_text_views(
                STANDALONE_WINDOW,
                self.scopes.gallery,
                &text_views,
                &output.text_inputs,
            ) {
                self.fatal_error = Some(error);
                return;
            }
            if let Some(adapter) = &mut self.accessibility {
                let accessibility_root =
                    accessibility_root(&built.root, isolated_accessibility_root);
                let mut update = self.semantics.build(
                    accessibility_root,
                    &layout,
                    self.input_ownership.focus().map(|focus| focus.element),
                    self.demo.locale.window_title(),
                );
                enrich_window_text(
                    &mut self.semantics,
                    self.scopes.gallery,
                    &text_views,
                    TextFieldStates {
                        main: &mut self.text_state,
                        search: &mut self.search_state,
                        area: &mut self.area_state,
                        number: &mut self.number_state,
                        vector: &mut self.vector_states,
                        color: &mut self.color_hex_state,
                        password: &mut self.password_state,
                        table_note: &mut self.table_note_state,
                        readonly_label: &mut self.readonly_label_state,
                    },
                    &mut self.text_context,
                    &mut update,
                );
                if let Err(error) = self
                    .documents
                    .enrich_accessibility(&mut self.semantics, &mut update)
                {
                    eprintln!("document accessibility: {error}");
                }
                adapter.update_if_active(|| {
                    self.frame_profile.observe_native(|| {
                        let owner = self.input_ownership.focus().map(|target| target.element);
                        NativeObservation::Projection(NativeProjection::from_update(
                            STANDALONE_WINDOW,
                            window.has_focus(),
                            window.scale_factor(),
                            owner,
                            owner.and_then(|element| self.semantics.node_for_element(element)),
                            &update,
                        ))
                    });
                    update
                });
            }
            frame_trace.mark(FrameStage::Accessibility);
            if focused_field.is_some() {
                self.text_bridge.sync_ime(
                    &window,
                    focused_field != Some(TextFieldKind::Label),
                    match focused_field {
                        Some(TextFieldKind::Main) => self.text_view,
                        Some(TextFieldKind::Search) => self.search_view,
                        Some(TextFieldKind::Area) => self.area_view,
                        Some(TextFieldKind::Number) => self.number_view,
                        Some(TextFieldKind::Vector(axis)) => self.vector_views[usize::from(axis)],
                        Some(TextFieldKind::ColorHex) => self.color_hex_view,
                        Some(TextFieldKind::Password) => self.password_view,
                        Some(TextFieldKind::TableNote) => self.table_note_view,
                        Some(TextFieldKind::Label) => self.readonly_label_view,
                        None => None,
                    },
                );
            }
            frame_trace.mark(FrameStage::Ime);
            self.frame_profile.submitted(frame_trace, profile_context);
            self.frame_meter.submitted(Instant::now());
            self.busy_visible = built.busy_roots.is_some_and(|ids| {
                ids.iter()
                    .any(|id| window_idle::visible_geometry(&layout, *id))
            });
            self.gpu_recovery.presented(window.id());
            self.rendered_frames += 1;
            if let Some(snapshot) = self.capture_query_snapshot(
                STANDALONE_WINDOW,
                self.scopes.gallery,
                &built.root,
                &layout,
                &self.ui_actions,
                self.input_ownership.focus(),
                focus_root,
            ) {
                self.ui_query.publish_root(
                    snapshot,
                    built.root,
                    &self.ui_actions,
                    &layout,
                    self.ui_input_context(STANDALONE_WINDOW, &window),
                );
            }
            return;
        }
    }
}
