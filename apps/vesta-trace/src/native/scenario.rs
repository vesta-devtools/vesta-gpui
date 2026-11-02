//! Finite command-origin acceptance using the current native view and input path.
mod accessibility;
pub(super) mod focus;
pub(super) mod lifecycle;
pub(super) mod navigation;
use super::*;
use vesta_gpui_winit::{UiInputEvent as Event, UiInputOrigin, UiKeyEvent};
use winit::{
    event::DeviceId,
    keyboard::{KeyCode, KeyLocation, ModifiersState, PhysicalKey},
};
impl App {
    fn check_range_cancel(&mut self) -> Result<()> {
        let before = self.timeline.as_ref().map(|t| (t.view, t.selection));
        let generation = self.generation;
        let scale = self
            .window
            .as_ref()
            .ok_or("trace_window_missing")?
            .scale_factor();
        let point = |fraction: f32| Event::CursorMoved {
            device_id: DeviceId::dummy(),
            position: winit::dpi::PhysicalPosition::new(
                f64::from(self.area.x + self.area.width * fraction) * scale,
                f64::from(self.area.y + self.area.height * 0.8) * scale,
            ),
        };
        let start = point(0.25);
        let end = point(0.65);
        self.input(&start, UiInputOrigin::Command)?;
        self.input(
            &Event::MouseInput {
                device_id: DeviceId::dummy(),
                state: ElementState::Pressed,
                button: MouseButton::Left,
            },
            UiInputOrigin::Command,
        )?;
        self.input(&end, UiInputOrigin::Command)?;
        if self.timeline.as_ref().is_none_or(|t| t.selection.is_none())
            || self.ownership.capture().is_none()
        {
            return Err("trace_range_preview_missing".into());
        }
        self.logical_key(Key::Named(NamedKey::Escape), None, KeyCode::Escape)?;
        self.input(
            &Event::MouseInput {
                device_id: DeviceId::dummy(),
                state: ElementState::Released,
                button: MouseButton::Left,
            },
            UiInputOrigin::Command,
        )?;
        if self.timeline.as_ref().map(|t| (t.view, t.selection)) != before
            || self.ownership.capture().is_some()
            || self.generation != generation
        {
            return Err("trace_range_cancel_changed_committed_evidence".into());
        }
        println!(
            "trace_inspection_check command_origin=true range_cancel_restores=true capture_released=true"
        );
        Ok(())
    }
    pub(super) fn smoke_ready(&self, viewport: Viewport) -> bool {
        let resized = match self.scene {
            Scene::Basic => false,
            Scene::Inspection => self.smoke_step == 9,
            Scene::Navigation => self.smoke_step == 8,
            Scene::Focus | Scene::Lifecycle => false,
        };
        !resized || (viewport.width - 980.0).abs() < 1.0
    }
    pub(super) fn smoke_steps(&self) -> usize {
        if self.scene == Scene::Lifecycle {
            7
        } else if self.scene == Scene::Focus {
            10
        } else if self.scene == Scene::Navigation {
            9
        } else if self.scene == Scene::Basic {
            5
        } else if self
            .session
            .as_ref()
            .and_then(|s| s.gpu.as_ref())
            .is_some_and(|g| !g.submissions.is_empty())
        {
            19
        } else {
            14
        }
    }
    fn logical_key(&mut self, logical: Key, text: Option<&str>, physical: KeyCode) -> Result<()> {
        self.input(
            &Event::KeyboardInput {
                device_id: DeviceId::dummy(),
                is_synthetic: false,
                event: UiKeyEvent {
                    physical_key: PhysicalKey::Code(physical),
                    logical_key: logical,
                    text: text.map(str::to_owned),
                    location: KeyLocation::Standard,
                    state: ElementState::Pressed,
                    repeat: false,
                },
            },
            UiInputOrigin::Command,
        )?;
        Ok(())
    }
    fn scroll_gpu_evidence(&mut self, lines: f32) -> Result<()> {
        let rect = self
            .layout_tree
            .as_ref()
            .and_then(|l| l.node(view::GPU_DETAIL))
            .ok_or("trace_gpu_detail_geometry_missing")?
            .scroll_viewport();
        let generation = self.generation;
        let scale = self
            .window
            .as_ref()
            .ok_or("trace_window_missing")?
            .scale_factor();
        self.input(
            &Event::CursorMoved {
                device_id: DeviceId::dummy(),
                position: winit::dpi::PhysicalPosition::new(
                    f64::from(rect.x + rect.width * 0.5) * scale,
                    f64::from(rect.y + rect.height * 0.5) * scale,
                ),
            },
            UiInputOrigin::Command,
        )?;
        self.input(
            &Event::MouseWheel {
                device_id: DeviceId::dummy(),
                delta: winit::event::MouseScrollDelta::LineDelta(0.0, lines),
                phase: winit::event::TouchPhase::Moved,
            },
            UiInputOrigin::Command,
        )?;
        if self.generation != generation {
            return Err("trace_gpu_scroll_restarted_analysis".into());
        }
        Ok(())
    }
    fn click_element(&mut self, element: vesta_gpui::ElementId) -> Result<()> {
        let node = self
            .layout_tree
            .as_ref()
            .and_then(|l| l.node(element))
            .ok_or("trace_inspection_target_not_realized")?;
        let rect = node
            .rect
            .intersection(node.inherited_clip)
            .ok_or("trace_inspection_target_clipped")?;
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return Err("trace_inspection_target_empty".into());
        }
        let scale = self
            .window
            .as_ref()
            .ok_or("trace_window_missing")?
            .scale_factor();
        self.input(
            &Event::CursorMoved {
                device_id: DeviceId::dummy(),
                position: winit::dpi::PhysicalPosition::new(
                    f64::from(rect.x + rect.width * 0.5) * scale,
                    f64::from(rect.y + rect.height * 0.5) * scale,
                ),
            },
            UiInputOrigin::Command,
        )?;
        for state in [ElementState::Pressed, ElementState::Released] {
            self.input(
                &Event::MouseInput {
                    device_id: DeviceId::dummy(),
                    state,
                    button: MouseButton::Left,
                },
                UiInputOrigin::Command,
            )?;
        }
        Ok(())
    }
    pub(super) fn smoke_presented(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        println!(
            "trace_view_presented step={} command_origin=true panel={:?} events={} collapsed={} range={:?} generation={} search_geometry=true elapsed_ms={}",
            self.smoke_step,
            self.panel,
            self.drawn,
            self.collapsed,
            self.timeline.as_ref().map(|t| t.view),
            self.generation,
            self.started.elapsed().as_millis()
        );
        let step = self.smoke_step;
        self.smoke_step += 1;
        if self.scene == Scene::Lifecycle {
            return self.lifecycle_presented(event_loop, step);
        }
        if self.scene == Scene::Focus {
            return self.focus_presented(event_loop, step);
        }
        if self.scene == Scene::Navigation {
            return self.navigation_presented(event_loop, step);
        }
        if self.scene == Scene::Basic {
            if self.smoke_step >= 5 {
                event_loop.exit();
            } else {
                self.command(match self.smoke_step {
                    1 => Command::Worst,
                    2 | 3 => Command::ZoomOut,
                    _ => Command::Fit,
                })?;
            }
            return Ok(());
        }
        match step {
            0 => {
                self.check_accessibility_initial()?;
                let session = self.session.as_ref().ok_or("trace_session_missing")?;
                self.smoke_source = Some(session.source.sha256.clone());
                let text = session
                    .events
                    .iter()
                    .find(|e| e.is_span() && e.parent.is_some() && e.end > e.start)
                    .ok_or("trace_inspection_child_required")?
                    .name
                    .clone();
                let original = self.timeline.as_ref().ok_or("trace_timeline_missing")?.view;
                self.input(
                    &Event::ModifiersChanged(ModifiersState::CONTROL.into()),
                    UiInputOrigin::Command,
                )?;
                self.logical_key(Key::Character("f".into()), None, KeyCode::KeyF)?;
                self.input(
                    &Event::ModifiersChanged(ModifiersState::empty().into()),
                    UiInputOrigin::Command,
                )?;
                self.input(&Event::Text(text.clone()), UiInputOrigin::Command)?;
                self.search.input.end_group();
                self.logical_key(Key::Character("w".into()), Some("w"), KeyCode::KeyW)?;
                if self.timeline.as_ref().is_none_or(|t| t.view != original)
                    || self.search.input.text() != format!("{text}w")
                {
                    return Err("trace_search_shortcut_ownership".into());
                }
                if !self
                    .search
                    .input
                    .act(&mut self.context, vesta_gpui::TextInputAction::Undo)
                    || self.search.input.text() != text
                {
                    return Err("trace_search_undo".into());
                }
                self.smoke_filter = Some(text);
                self.apply_search()?;
                println!(
                    "trace_inspection_check command_origin=true text_owns_w=true undo_restores_filter=true"
                );
            }
            1 => {
                if !self.search_focused() || self.smoke_filter.as_ref() != Some(&self.filter) {
                    return Err("trace_search_filter_not_applied".into());
                }
                self.check_range_cancel()?;
                self.check_accessibility_selection()?;
                self.check_pan_focus_loss()?;
                let route = self.route.as_ref().ok_or("trace_route_missing")?;
                let element = self
                    .layout_tree
                    .as_ref()
                    .ok_or("trace_layout_missing")?
                    .nodes()
                    .iter()
                    .find_map(|node| {
                        if let Some(Command::Scope(key)) = route.action_at(node.id) {
                            let session = self.session.as_ref()?;
                            let event = &session.events[*session.indices.get(key)?];
                            if event.is_span() && event.parent.is_some() && event.end > event.start
                            {
                                return Some(node.id);
                            }
                        }
                        None
                    })
                    .ok_or("trace_filtered_scope_not_realized")?;
                self.click_element(element)?;
                if self.timeline.as_ref().and_then(|t| t.selected).is_none() {
                    return Err("trace_pointer_scope_not_selected".into());
                }
            }
            2 => self.check_accessibility_activation()?,
            3 => {
                if !matches!(self.report.as_ref().and_then(|r|r.inspection.table.as_ref()),Some(crate::query::protocol::Data::Relations {items,..})if !items.is_empty())
                {
                    return Err("trace_native_caller_evidence_missing".into());
                }
                self.logical_key(Key::Named(NamedKey::Tab), None, KeyCode::Tab)?;
                if self
                    .ownership
                    .focus()
                    .is_none_or(|t| t.element != vesta_gpui::ElementId::new(104))
                {
                    return Err("trace_native_tab_target".into());
                }
                self.input(
                    &Event::ModifiersChanged(ModifiersState::SHIFT.into()),
                    UiInputOrigin::Command,
                )?;
                self.logical_key(Key::Named(NamedKey::Tab), None, KeyCode::Tab)?;
                if self
                    .ownership
                    .focus()
                    .is_none_or(|t| t.element != vesta_gpui::ElementId::new(102))
                {
                    return Err("trace_native_reverse_tab_target".into());
                }
                self.input(
                    &Event::ModifiersChanged(ModifiersState::empty().into()),
                    UiInputOrigin::Command,
                )?;
                self.logical_key(Key::Named(NamedKey::Tab), None, KeyCode::Tab)?;
                self.logical_key(Key::Named(NamedKey::Enter), None, KeyCode::Enter)?;
                if self.panel != Panel::Callees {
                    return Err("trace_native_keyboard_activation".into());
                }
                println!(
                    "trace_inspection_check command_origin=true tab_forward_reverse=true enter_activates=true"
                );
            }
            4 => self.click_element(vesta_gpui::ElementId::new(106))?,
            5 => {
                if self
                    .report
                    .as_ref()
                    .and_then(|r| r.inspection.page())
                    .is_none_or(|p| p.total == 0)
                {
                    return Err("trace_native_occurrences_missing".into());
                }
                self.click_element(vesta_gpui::ElementId::new(108))?;
            }
            6 => {
                self.command(Command::ClearSearch)?;
                self.command(Command::Fit)?;
                self.command(Command::Panel(Panel::Counters))?;
                println!(
                    "trace_inspection_check command_origin=true superseded_generations_queued=true"
                );
            }
            7 => {
                if self.panel != Panel::Counters
                    || !self.filter.is_empty()
                    || self.report.as_ref().is_none_or(|r| {
                        r.analysis.range
                            != self.session.as_ref().map_or(r.analysis.range, |s| s.bounds)
                    })
                {
                    return Err("trace_stale_report_published".into());
                }
                let marker = self.layout_tree.as_ref().zip(self.route.as_ref()).and_then(
                    |(layout, route)| {
                        layout.nodes().iter().find_map(|node| {
                            if let Some(Command::Scope(key)) = route.action_at(node.id) {
                                let session = self.session.as_ref()?;
                                let event = &session.events[*session.indices.get(key)?];
                                if matches!(event.kind, EventKind::Counter(_)) {
                                    return Some(node.id);
                                }
                            }
                            None
                        })
                    },
                );
                if let Some(marker) = marker {
                    self.click_element(marker)?;
                    self.smoke_counter = self.timeline.as_ref().and_then(|t| t.selected);
                }
                self.command(Command::Panel(Panel::Diagnostics))?;
            }
            8 => {
                if let Some(key) = self.smoke_counter {
                    let detail = self
                        .report
                        .as_ref()
                        .and_then(|r| r.inspection.detail.as_ref())
                        .ok_or("trace_counter_detail_missing")?;
                    if detail.item.key != key || detail.item.counter_value.is_none() {
                        return Err("trace_counter_pick_lost_original_evidence".into());
                    }
                    println!(
                        "trace_inspection_check command_origin=true counter_raw_pick=true key={key:?}"
                    );
                }
                let resized = self
                    .window
                    .as_ref()
                    .ok_or("trace_window_missing")?
                    .request_inner_size(LogicalSize::new(980.0, 720.0));
                println!("trace_inspection_resize requested_logical=980x720 immediate={resized:?}");
                self.redraw();
            }
            9 => {
                if !self.filter.is_empty() || !self.search.input.text().is_empty() {
                    return Err("trace_resize_changed_search".into());
                }
                self.click_element(vesta_gpui::ElementId::new(106))?;
            }
            10 => {
                self.smoke_page = self
                    .report
                    .as_ref()
                    .and_then(|r| r.inspection.page())
                    .is_some_and(|p| p.has_more);
                if self.smoke_page {
                    self.click_element(vesta_gpui::ElementId::new(122))?;
                } else {
                    self.command(Command::Panel(Panel::Diagnostics))?;
                }
            }
            11 => {
                if self.smoke_page {
                    if self.page.as_ref().is_none_or(|c| c.offset != 100)
                        || self.previous_pages.len() != 1
                    {
                        return Err("trace_native_page_cursor".into());
                    }
                    self.click_element(vesta_gpui::ElementId::new(120))?;
                } else {
                    self.command(Command::Fit)?;
                }
            }
            12 => {
                if self.smoke_page && (!self.previous_pages.is_empty() || self.page.is_some()) {
                    return Err("trace_native_previous_page".into());
                }
                self.command(Command::Panel(Panel::Diagnostics))?;
                let path = self
                    .source_path
                    .clone()
                    .ok_or("trace_source_path_missing")?;
                self.input(&Event::DroppedFile(path), UiInputOrigin::Command)?;
            }
            13 => {
                if self
                    .session
                    .as_ref()
                    .is_none_or(|s| Some(&s.source.sha256) != self.smoke_source.as_ref())
                    || !self.filter.is_empty()
                    || self.timeline.as_ref().is_none_or(|t| t.selected.is_some())
                    || self.panel != Panel::Diagnostics
                {
                    return Err("trace_reload_identity_or_state_changed".into());
                }
                if self.smoke_steps() > 14 {
                    self.click_element(vesta_gpui::ElementId::new(114))?;
                } else {
                    println!(
                        "trace_inspection_complete command_origin=true steps=14 source_unchanged=true pagination_exercised={}",
                        self.smoke_page
                    );
                    event_loop.exit();
                }
            }
            14 => {
                let item = match self
                    .report
                    .as_ref()
                    .and_then(|r| r.inspection.table.as_ref())
                {
                    Some(crate::query::protocol::Data::GpuSubmissions { items, .. }) => {
                        items.first().ok_or("trace_gpu_rows_absent")?
                    }
                    _ => return Err("trace_gpu_table_absent".into()),
                };
                let index = self
                    .session
                    .as_ref()
                    .and_then(|s| {
                        s.gpu_indices
                            .get(&(item.key.generation, item.key.submission))
                    })
                    .ok_or("trace_gpu_source_key_missing")?;
                self.click_element(vesta_gpui::ElementId::new(20_000_000 + *index as u64))?;
            }
            15 => {
                let detail = self
                    .report
                    .as_ref()
                    .and_then(|r| r.inspection.gpu.as_ref())
                    .ok_or("trace_gpu_detail_absent")?;
                if Some(detail.key) != self.gpu || detail.frame.is_none() {
                    return Err("trace_gpu_detail_key_mismatch".into());
                }
                println!(
                    "trace_gpu_check command_origin=true raw_key_pick=true key={:?} ticks={:?} elapsed_ns={:?} cpu_clock_mapping=false",
                    detail.key, detail.raw_ticks, detail.elapsed_ns
                );
                self.scroll_gpu_evidence(-10000.0)?;
            }
            16 => {
                if self.gpu_scroll.max_y() <= 0.0
                    || (self.gpu_scroll.offset_y() - self.gpu_scroll.max_y()).abs() > 0.1
                {
                    return Err("trace_gpu_detail_bottom_unreachable".into());
                }
                self.scroll_gpu_evidence(10000.0)?;
            }
            17 => {
                if self.gpu_scroll.offset_y() != 0.0 {
                    return Err("trace_gpu_detail_top_unreachable".into());
                }
                println!(
                    "trace_gpu_check command_origin=true detail_scroll_bottom_top=true analysis_unchanged=true"
                );
                self.check_gpu_keyboard_scroll()?;
                self.click_element(vesta_gpui::ElementId::new(134))?;
            }
            18 => {
                let detail = self
                    .report
                    .as_ref()
                    .and_then(|r| r.inspection.gpu.as_ref())
                    .ok_or("trace_gpu_detail_absent")?;
                if self.timeline.as_ref().and_then(|t| t.selected) != detail.frame {
                    return Err("trace_gpu_frame_join_failed".into());
                }
                println!(
                    "trace_inspection_complete command_origin=true steps=19 source_unchanged=true gpu_frame_join=true gpu_detail_scroll=true"
                );
                event_loop.exit();
            }
            _ => return Err("trace_inspection_step_overrun".into()),
        }
        self.redraw();
        Ok(())
    }
}
