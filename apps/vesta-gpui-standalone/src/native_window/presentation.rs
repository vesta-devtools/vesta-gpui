//! Presentation of an owned logical floating window.
use super::*;

impl App {
    pub(crate) fn redraw_native_window(&mut self, window_id: WindowId) {
        if self
            .native_windows
            .get(&window_id)
            .is_none_or(|host| !host.surface.available())
        {
            return;
        }
        let _trace_frame = vesta_gpui::trace::frame("Standalone.Float");
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
                if let Some(host) = self.native_windows.get(&window_id) {
                    host.window.request_redraw();
                }
                return;
            }
            layout_passes += 1;
            let layout_pass = vesta_gpui::trace::span("standalone.frame", "layout.pass");
            if let Some(window) = self
                .native_windows
                .get(&window_id)
                .map(|host| host.ui_window)
            {
                self.reconcile_window_menu(window);
                self.prepare_suggestions(window);
            }
            let password_snapshot = self.password_display_snapshot();
            let table_note_snapshot = self.table_note_state.snapshot();
            let tooltip_focus_root = self
                .native_windows
                .get(&window_id)
                .and_then(|host| self.focus_root_for_window(host.ui_window));
            let Some(mut host) = self.native_windows.remove(&window_id) else {
                return;
            };
            let Some(title) = self.native_window_title(host.logical_id) else {
                return;
            };
            let size = host.window.inner_size();
            if size.width == 0 || size.height == 0 {
                self.native_windows.insert(window_id, host);
                return;
            }
            let scale = host.window.scale_factor() as f32;
            let logical = size.to_logical::<f32>(host.window.scale_factor());
            let viewport = Viewport::new(logical.width, logical.height, scale);
            let focused_element = host.input.focus().map(|focus| focus.element);
            let focused_field = TextFieldKind::ALL
                .into_iter()
                .find(|field| Some(field.element(host.scopes.gallery)) == focused_element);
            let hovered = host.hit_test.hit(host.cursor);
            let dock_drag = self.dock_drag_visual(ContainerRef::Floating(host.logical_id));
            let collection_focus = collection_projection_focus(
                &self.demo,
                &host.actions,
                &host.input,
                host.ui_window,
                host.scopes.gallery,
                tooltip_focus_root,
            );
            host.area_layout
                .prepare(&mut self.area_state, &mut self.text_context);
            let gallery = build_gallery(&UiBuildState {
                documents: &host.documents,
                selected_feedback: native_capture::selected_feedback_enabled(self),
                ui_window: host.ui_window,
                menu_open: host.pane_menu.is_some(),
                scopes: host.scopes,
                workspace_container: ContainerRef::Floating(host.logical_id),
                mode: AcceptanceMode::Gallery2d,
                maximized: host.window.is_maximized(),
                theme: self.theme,
                scroll_offset: Point::default(),
                viewport_size: Size::new(logical.width, (logical.height - CHROME_HEIGHT).max(0.0)),
                snapshot: self.text_state.snapshot(),
                focused: focused_field == Some(TextFieldKind::Main),
                search_snapshot: self.search_state.snapshot(),
                search_query: self.search_state.text(),
                search_focused: focused_field == Some(TextFieldKind::Search),
                area_snapshot: self.area_state.snapshot().with_viewport_scroll(
                    Point::new(0.0, host.area_scroll_y),
                    host.area_follow_caret,
                ),
                area_focused: focused_field == Some(TextFieldKind::Area),
                number_snapshot: self.number_state.snapshot(),
                number_focused: focused_field == Some(TextFieldKind::Number),
                vector_snapshots: std::array::from_fn(|axis| self.vector_states[axis].snapshot()),
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
                virtual_focused: match collection_focus {
                    Some(CollectionFocus::List { index, .. }) => Some(index),
                    _ => None,
                },
                virtual_table_focused: match collection_focus {
                    Some(CollectionFocus::Table(index)) => Some(index),
                    _ => None,
                },
                virtual_table_viewport_height: window_collections::table_viewport_height(
                    &host.scroll_geometry,
                    host.scopes.gallery,
                ),
                tree_focused: match collection_focus {
                    Some(CollectionFocus::Tree { index, .. }) => Some(index),
                    _ => None,
                },
                demo: &self.demo,
                hovered,
                focused_element,
                dock_drag: dock_drag.clone(),
                popover_anchor: host.popover_anchor,
                combo_anchor: host.combo_anchor,
                gizmo_mode: self.scene.gizmo.mode,
                frame_stats: self.frame_meter.display,
                overlay_owner: self.overlay_owner,
            });
            host.tile_columns = gallery.tile_columns;
            let workspace_regions = gallery.workspace_regions.clone();
            let ramp_track = gallery.ramp_track;
            let slider = gallery.slider;
            let (mut root, mut action_pairs) = native_window::build_native_floating_root(
                host.chrome_scope,
                gallery.element,
                gallery.actions,
                self.theme,
                native_window::NativeChrome {
                    scope: host.title_scope,
                    title: &display_text(&title, self.demo.locale),
                    state: vesta_gpui_widgets::TitleBarState {
                        maximized: host.window.is_maximized(),
                        hovered: host.hit_test.hit(host.cursor),
                    },
                },
            );
            if let Some(menu) = host.pane_menu.clone() {
                if menu.context.is_live(&self.demo) {
                    root = root.child(build_window_menu_popup(
                        host.scopes.menu,
                        self.theme,
                        &self.demo,
                        &menu,
                        hovered,
                        Size::new(logical.width, logical.height),
                        &mut action_pairs,
                        &mut self.text_context,
                    ));
                } else {
                    host.close_pane_menu();
                }
            }
            if let Some(drag) = &dock_drag {
                root = root.child(dock_drag_overlay(
                    host.scopes.root,
                    drag,
                    self.theme,
                    Size::new(logical.width, logical.height),
                ));
            }
            if self.owns_overlay(host.ui_window)
                && let Some(overlays) = build_owned_overlays(
                    host.scopes,
                    &self.demo,
                    self.search_state.text(),
                    &self.search_state.snapshot(),
                    self.text_session_field(host.ui_window) == Some(TextFieldKind::Search),
                    Size::new(logical.width, logical.height),
                    self.theme,
                    &mut action_pairs,
                )
            {
                root = root.child(overlays);
            }
            localize_tree(&mut root, self.demo.locale);
            frame_trace.mark(FrameStage::Build);
            let Ok(actions) = ActionRoute::build(&root, action_pairs.iter().copied()) else {
                self.native_windows.insert(window_id, host);
                return;
            };
            frame_trace.mark(FrameStage::Route);
            let window_active = host.window.has_focus();
            let base_children = root.children.len();
            let Ok(layout) = self.layout_floating_tooltips(
                &mut host,
                &mut root,
                &actions,
                viewport,
                window_active,
                tooltip_focus_root,
            ) else {
                self.native_windows.insert(window_id, host);
                return;
            };
            let Ok(actions) =
                window_tooltips::route_after_tooltip(&root, base_children, actions, action_pairs)
            else {
                self.native_windows.insert(window_id, host);
                return;
            };
            frame_trace.mark(FrameStage::Layout);
            let wrap_changed = host.area_layout.sync(
                &mut self.area_state,
                &mut self.text_context,
                host.scopes.gallery,
                &layout,
            );
            match host.documents.sync_layout(
                host.scopes.markdown,
                &layout,
                &mut self.text_context,
                self.theme,
            ) {
                Ok(true) => {
                    self.native_windows.insert(window_id, host);
                    continue;
                }
                Ok(false) => {}
                Err(error) => {
                    self.fatal_error = Some(error.to_string());
                    self.native_windows.insert(window_id, host);
                    return;
                }
            }
            if host.documents.sync_input(
                host.ui_window,
                host.scopes.markdown,
                &layout,
                &mut host.input,
            ) {
                self.native_windows.insert(window_id, host);
                continue;
            }
            let workspace_rect = layout
                .node(host.scopes.workspace.element(1))
                .map(|node| node.rect);
            host.dock_frame = DockFrame::from_layout(&workspace_regions, &layout, workspace_rect);
            self.sync_palette_layout(host.ui_window, host.scopes.root, &layout);
            self.sync_suggestion_layout(host.ui_window, host.scopes.gallery, &layout);
            host.ramp_track_rect = layout.node(ramp_track).map(|node| node.rect);
            host.slider_rect = layout.node(slider).map(|node| node.rect);
            host.table_rect = layout
                .node(host.scopes.gallery.element(TABLE_ROOT))
                .map(|node| node.rect);
            host.combo_anchor = layout
                .node(host.scopes.gallery.element(COMBO_TRIGGER))
                .map(|node| node.rect);
            host.popover_anchor = layout
                .node(host.scopes.gallery.element(POPOVER_TRIGGER))
                .map(|node| node.rect);
            let page_items = workspace_regions
                .panes
                .iter()
                .filter_map(|region| {
                    self.demo
                        .workspace
                        .pane(region.pane)
                        .and_then(Pane::active_item)
                        .map(|item| item.id)
                })
                .collect::<Vec<_>>();
            host.page_scroll_item = page_items.first().copied();
            let previous_table_height = window_collections::table_viewport_height(
                &host.scroll_geometry,
                host.scopes.gallery,
            );
            host.scroll_geometry = ScrollGeometry::from_layout(
                host.scopes.gallery,
                &layout,
                page_items.iter().copied(),
            )
            .with_tabs(&layout, &workspace_regions);
            let table_viewport_changed = previous_table_height
                != window_collections::table_viewport_height(
                    &host.scroll_geometry,
                    host.scopes.gallery,
                );
            let mut scroll_changed = (ScrollStates {
                tree: &mut self.demo.tree_scroll,
                virtual_list: &mut self.demo.virtual_scroll,
                virtual_table: &mut self.demo.virtual_table_scroll,
                nested: &mut self.demo.nested_scroll,
                horizontal: &mut self.demo.horizontal_scroll,
                gallery: None,
                pages: &mut self.demo.page_scroll,
                tabs: &mut self.demo.tab_scroll,
            })
            .sync_layout(host.scopes.gallery, &host.scroll_geometry, &layout)
                || table_viewport_changed;
            // Tab strips and the TextArea viewport belong to the window that shows them, so their
            // scroll extents are synced from this window's own layout.
            host.tab_scroll_rects.clear();
            let pane_width_changed =
                self.demo
                    .sync_pane_widths(workspace_regions.panes.iter().filter_map(|region| {
                        layout
                            .node(region.root)
                            .map(|root| (region.pane, root.rect.width))
                    }));
            for region in &workspace_regions.panes {
                if let Some(list) = layout.node(region.tab_list) {
                    host.tab_scroll_rects
                        .insert(region.pane, list.scroll_viewport());
                    scroll_changed |= self
                        .demo
                        .tab_scroll
                        .entry(region.pane)
                        .or_default()
                        .sync_layout(list);
                }
            }
            let tab_changed = reveal_active_tabs(
                &workspace_regions,
                &layout,
                &self.demo.workspace,
                &mut self.demo.tab_scroll,
                &mut host.tab_reveal,
            );
            host.focus_rects = actions
                .focus_order()
                .iter()
                .copied()
                .filter_map(|id| layout.node(id).map(|node| (id, node.rect)))
                .collect();
            let focus_root = self
                .owns_overlay(host.ui_window)
                .then(|| {
                    self.demo
                        .dialog_stack
                        .top()
                        .map(|id| dialog_element(host.scopes.root, id, 0))
                        .or_else(|| {
                            self.demo
                                .palette_session
                                .is_open()
                                .then(|| host.scopes.root.element(PALETTE_ROOT))
                        })
                })
                .flatten();
            let popup_scope = FocusScope::new(host.ui_window, &actions, None);
            let combo_closed = self
                .demo
                .combo
                .reconcile(host.ui_window, &popup_scope)
                .is_some();
            let popover_closed = self
                .demo
                .popover
                .reconcile(host.ui_window, &popup_scope)
                .is_some();
            if combo_closed || popover_closed {
                self.native_windows.insert(window_id, host);
                self.request_all_redraw();
                continue;
            }
            let scope = FocusScope::new(host.ui_window, &actions, focus_root);
            let focus_native = DockFocusTransfer::request(
                &mut self.dock_focus,
                &self.demo.workspace,
                ContainerRef::Floating(host.logical_id),
                host.ui_window,
                &actions,
                focus_root,
                &mut host.input,
            );
            if let Some(restored) = scope.resolve_request(&mut host.input) {
                let owner = host.ui_window;
                if focus_native && restored.is_some() {
                    host.window.focus_window();
                }
                host.actions = actions;
                self.native_windows.insert(window_id, host);
                self.set_window_focus(owner, restored.map(|target| target.element));
                if let Some(target) = restored {
                    self.reveal_window_focus(owner, target.element);
                }
                self.request_all_redraw();
                continue;
            }
            let reconciled = scope.reconcile(&mut host.input);
            if reconciled.lost_focus || reconciled.lost_capture {
                let owner = host.ui_window;
                host.actions = actions;
                host.pointer_click.cancel();
                self.native_windows.insert(window_id, host);
                if reconciled.lost_focus {
                    self.cancel_window_editors_for(owner, PopupCloseReason::OwnerRemoved);
                } else {
                    self.cancel_window_workspace_drag(owner);
                    self.cancel_window_value_edits(owner);
                }
                self.request_all_redraw();
                continue;
            }
            host.actions = actions;
            frame_trace.mark(FrameStage::Reconcile);
            if wrap_changed || scroll_changed || tab_changed || pane_width_changed {
                self.native_windows.insert(window_id, host);
                continue;
            }
            drop(layout_pass);
            vesta_gpui::trace::counter(
                "standalone.frame",
                "layout.passes",
                f64::from(layout_passes),
            );
            let paint = PaintList::build_with_focus_and_interaction(
                &root,
                &layout,
                host.input.visible_focus().map(|focus| focus.element),
                self.theme.palette.focus,
                self.window_paint_interaction(
                    host.cursor,
                    host.scrollbar_drag.map(|(element, _)| element),
                ),
            );
            if complete_caret_reveal(
                &mut self.caret_reveal,
                self.text_session,
                CaretRevealFrame {
                    window: host.ui_window,
                    gallery: host.scopes.gallery,
                    actions: &host.actions,
                    focus: host.input.focus(),
                    focus_root,
                    geometry: &host.scroll_geometry,
                    paint: &paint,
                    layout_stable: !(wrap_changed || scroll_changed || tab_changed),
                },
                &mut ScrollStates {
                    tree: &mut self.demo.tree_scroll,
                    virtual_list: &mut self.demo.virtual_scroll,
                    virtual_table: &mut self.demo.virtual_table_scroll,
                    nested: &mut self.demo.nested_scroll,
                    horizontal: &mut self.demo.horizontal_scroll,
                    gallery: None,
                    pages: &mut self.demo.page_scroll,
                    tabs: &mut self.demo.tab_scroll,
                },
            ) {
                self.native_windows.insert(window_id, host);
                continue;
            }
            frame_trace.mark(FrameStage::Paint);
            host.hit_test = HitTestTree::build(&root, &layout);
            frame_trace.mark(FrameStage::Hit);
            host.caret_blink.update(
                focused_field
                    .filter(|_| window_active)
                    .map(|field| field.element(host.scopes.gallery)),
                Instant::now(),
            );
            if self.capture_geometry_changed(
                host.ui_window,
                host.scopes.gallery,
                &root,
                &layout,
                &host.actions,
            ) {
                if let Ok(renderer) = host.surface.capture_renderer() {
                    renderer.cancel_capture();
                }
                self.ui_query.cancel_capture_binding(host.ui_window);
            }
            let Some(gpu) = self.gpu.as_mut() else {
                self.native_windows.insert(window_id, host);
                return;
            };
            let profile_context = FrameContext {
                window: host.ui_window,
                gallery: true,
                text_focused: focused_field.is_some(),
                active: window_active,
            };
            let output = match gpu.render(
                Some(&mut host.surface),
                RenderFrame {
                    viewport,
                    paint: &paint,
                    context: &mut self.text_context,
                    rasterizer: &mut self.text_rasterizer,
                    images: Some(&DemoImages::for_frame(host.smoke_frames)),
                    options: RenderOptions {
                        clear: Some(theme_clear(self.theme, false)),
                        selection: self.theme.palette.selection,
                        caret: self.theme.palette.focus,
                        caret_visible: host.caret_blink.visible(),
                    },
                    scene: None,
                    finish: self.smoke && !render_backend::retirement_probe_enabled(),
                    cpu_profile: frame_trace.renderer_profile(),
                    gpu_profile: &mut self.gpu_profile,
                    profile_context,
                },
            ) {
                Ok(Some(output)) => output,
                Ok(None) => {
                    self.native_windows.insert(window_id, host);
                    if self.display_blocked(window_id) {
                        self.cancel_gpu_interactions(&std::collections::HashSet::from([window_id]));
                    }
                    return;
                }
                Err(error) => {
                    self.native_windows.insert(window_id, host);
                    self.gpu_fault(window_id, error);
                    return;
                }
            };
            frame_trace.mark(FrameStage::Renderer);
            self.lifecycle_profile
                .presented(host.ui_window, input_watermark);
            self.gpu_recovery.presented(window_id);
            if self.smoke {
                host.smoke_frames += 1;
            }
            host.text_views =
                match text_views_for(host.ui_window, host.scopes.gallery, &output.text_inputs) {
                    Ok(views) => views,
                    Err(error) => {
                        self.fatal_error = Some(format!("text_input_views: {error:?}"));
                        self.native_windows.insert(window_id, host);
                        return;
                    }
                };
            if let Some(view) = host.text_views.get(TextFieldKind::Area) {
                host.area_scroll_y = view.scroll_y;
            }
            frame_trace.mark(FrameStage::TextViews);
            // A modal dialog owned by this window isolates the semantic tree the same way the main
            // window does, so assistive technology cannot reach the background (ADR-0254).
            let isolated = if self.overlay_owner == Some(host.ui_window) {
                self.demo
                    .dialog_stack
                    .top()
                    .map(|id| dialog_element(host.scopes.root, id, 0))
            } else {
                None
            };
            let mut update = host.semantics.build(
                accessibility_root(&root, isolated),
                &layout,
                host.input.focus().map(|focus| focus.element),
                &format!("{} — Vesta", display_text(&title, self.demo.locale)),
            );
            enrich_window_text(
                &mut host.semantics,
                host.scopes.gallery,
                &host.text_views,
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
            if let Err(error) = host
                .documents
                .enrich_accessibility(&mut host.semantics, &mut update)
            {
                eprintln!("document accessibility: {error}");
            }
            host.accessibility.update_if_active(|| {
                self.frame_profile.observe_native(|| {
                    let owner = host.input.focus().map(|target| target.element);
                    NativeObservation::Projection(NativeProjection::from_update(
                        host.ui_window,
                        host.window.has_focus(),
                        host.window.scale_factor(),
                        owner,
                        owner.and_then(|element| host.semantics.node_for_element(element)),
                        &update,
                    ))
                });
                update
            });
            frame_trace.mark(FrameStage::Accessibility);
            if let Some(field) = focused_field
                && let Some(bridge) = self.native_text_bridges.get_mut(host.ui_window)
            {
                bridge.sync_ime(
                    &host.window,
                    field != TextFieldKind::Label,
                    host.text_views.get(field),
                );
            }
            frame_trace.mark(FrameStage::Ime);
            host.busy_visible = gallery
                .busy_roots
                .iter()
                .any(|id| window_idle::visible_geometry(&layout, *id));
            self.frame_profile.submitted(frame_trace, profile_context);
            if let Some(snapshot) = self.capture_query_snapshot(
                host.ui_window,
                host.scopes.gallery,
                &root,
                &layout,
                &host.actions,
                host.input.focus(),
                focus_root,
            ) {
                self.ui_query.publish_root(
                    snapshot,
                    root,
                    &host.actions,
                    &layout,
                    self.ui_input_context(host.ui_window, &host.window),
                );
            }
            self.native_windows.insert(window_id, host);
            return;
        }
    }
}
