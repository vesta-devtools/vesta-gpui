//! Actual keyboard reachability, async focus and source-node retirement.
use super::*;
use accesskit::{Action, ActionRequest, NodeId};

pub(in crate::native) struct State {
    alternate: Option<PathBuf>,
    source_a: Option<String>,
    old_node: Option<NodeId>,
    old_element: Option<vesta_gpui::ElementId>,
    old_timer: Option<TimerKey>,
    before_scroll: Option<Arc<crate::analysis::Analysis>>,
    page: Option<(String, String, usize)>,
    previous: usize,
}
impl State {
    pub(in crate::native) fn new(alternate: Option<PathBuf>) -> Self {
        Self {
            alternate,
            source_a: None,
            old_node: None,
            old_element: None,
            old_timer: None,
            before_scroll: None,
            page: None,
            previous: 0,
        }
    }
}
impl App {
    fn focus_by_tab(&mut self, element: vesta_gpui::ElementId) -> Result<()> {
        if self
            .layout_tree
            .as_ref()
            .is_none_or(|l| l.node(element).is_none())
        {
            return Err("trace_focus_target_not_realized".into());
        }
        for _ in 0..4096 {
            if self.ownership.focus().is_some_and(|t| t.element == element) {
                return Ok(());
            }
            self.logical_key(Key::Named(NamedKey::Tab), None, KeyCode::Tab)?;
        }
        Err("trace_focus_target_not_keyboard_reachable".into())
    }
    fn first_timer_target(&self) -> Result<(vesta_gpui::ElementId, TimerKey)> {
        self.layout_tree
            .as_ref()
            .ok_or("trace_layout_missing")?
            .nodes()
            .iter()
            .filter_map(|node| {
                let Some(Command::Timer(category, name)) = self.route.as_ref()?.action_at(node.id)
                else {
                    return None;
                };
                let rect = node.rect.intersection(node.inherited_clip)?;
                self.table_area
                    .contains(Point::new(
                        rect.x + rect.width * 0.5,
                        rect.y + rect.height * 0.5,
                    ))
                    .then(|| {
                        (
                            node.rect.y,
                            node.id,
                            TimerKey {
                                category: category.clone(),
                                name: name.clone(),
                            },
                        )
                    })
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id, timer)| (id, timer))
            .ok_or_else(|| "trace_focus_timer_not_realized".into())
    }
    fn cursor_identity(&self) -> Option<(String, String, usize)> {
        self.page
            .as_ref()
            .map(|p| (p.source_sha256.clone(), p.query_sha256.clone(), p.offset))
    }
    fn timer_is(&self, key: &TimerKey) -> bool {
        self.timer
            .as_ref()
            .is_some_and(|t| t.category == key.category && t.name == key.name)
    }
    fn focus_key(&mut self, key: NamedKey, code: KeyCode) -> Result<()> {
        self.logical_key(Key::Named(key), None, code)
    }
    pub(super) fn focus_presented(
        &mut self,
        event_loop: &ActiveEventLoop,
        step: usize,
    ) -> Result<()> {
        match step {
            0 => {
                self.focus_probe.source_a = self.session.as_ref().map(|s| s.source.sha256.clone());
                let generation = self.generation;
                self.command(Command::FocusSearch)?;
                self.focus_key(NamedKey::PageDown, KeyCode::PageDown)?;
                if self.table_scroll != 0 || self.generation != generation || !self.search_focused()
                {
                    return Err("trace_focus_search_keys_stolen".into());
                }
                self.focus_by_tab(view::TABLE)?;
                for (key, code, positive) in [
                    (NamedKey::PageDown, KeyCode::PageDown, true),
                    (NamedKey::PageUp, KeyCode::PageUp, false),
                    (NamedKey::ArrowDown, KeyCode::ArrowDown, true),
                    (NamedKey::ArrowUp, KeyCode::ArrowUp, false),
                    (NamedKey::End, KeyCode::End, true),
                ] {
                    self.focus_key(key, code)?;
                    if (self.table_scroll > 0) != positive || self.generation != generation {
                        return Err("trace_focus_statistics_key_or_analysis".into());
                    }
                }
                println!(
                    "trace_focus_check command_origin=true search_owns_keys=true statistics_tab_page_arrows_end=true analysis_unchanged=true"
                );
            }
            1 => {
                if self.table_scroll != self.table_rows.saturating_sub(self.table_page_rows)
                    || self.table_scroll == 0
                {
                    return Err("trace_focus_statistics_bottom_missing".into());
                }
                self.focus_key(NamedKey::Home, KeyCode::Home)?;
                if self.table_scroll != 0 {
                    return Err("trace_focus_statistics_home".into());
                }
            }
            2 => {
                let (element, timer) = self.first_timer_target()?;
                self.focus_by_tab(element)?;
                self.focus_probe.old_element = Some(element);
                self.focus_probe.old_timer = Some(timer.clone());
                self.focus_key(NamedKey::Enter, KeyCode::Enter)?;
                if !self.timer_is(&timer) {
                    return Err("trace_focus_keyboard_row_activation".into());
                }
                // Materialize the real busy frame before the queued worker message
                // is published. The finite scene cannot advance while busy.
                self.draw(event_loop)?;
                if self.ownership.focus().is_none_or(|t| t.element != element) {
                    return Err("trace_focus_busy_projection_lost_logical_target".into());
                }
                println!(
                    "trace_focus_check command_origin=true row_keyboard_enter=true busy_keeps_logical_focus=true"
                );
            }
            3 => {
                let timer = self
                    .focus_probe
                    .old_timer
                    .clone()
                    .ok_or("trace_focus_old_timer_missing")?;
                if !self.timer_is(&timer) {
                    return Err("trace_focus_row_analysis_not_published".into());
                }
                self.focus_by_tab(vesta_gpui::ElementId::new(106))?;
                self.focus_key(NamedKey::Enter, KeyCode::Enter)?;
            }
            4 => {
                if self
                    .report
                    .as_ref()
                    .and_then(|r| r.inspection.page())
                    .is_none_or(|p| !p.has_more)
                {
                    return Err("trace_focus_paging_fixture_required".into());
                }
                self.focus_by_tab(vesta_gpui::ElementId::new(122))?;
                self.focus_key(NamedKey::Enter, KeyCode::Enter)?;
            }
            5 => {
                if self.page.as_ref().is_none_or(|p| p.offset != 100)
                    || self.previous_pages.len() != 1
                {
                    return Err("trace_focus_next_page_missing".into());
                }
                self.focus_probe.page = self.cursor_identity();
                self.focus_probe.previous = self.previous_pages.len();
                self.focus_probe.before_scroll = self.report.as_ref().map(|r| r.analysis.clone());
                let generation = self.generation;
                self.focus_by_tab(view::TRACK)?;
                self.focus_key(NamedKey::PageDown, KeyCode::PageDown)?;
                if self.timeline.as_ref().is_none_or(|t| t.scroll <= 0.0) {
                    return Err("trace_focus_track_page_missing".into());
                }
                self.focus_key(NamedKey::End, KeyCode::End)?;
                self.nav_point(0.5)?;
                self.input(
                    &Event::MouseWheel {
                        device_id: DeviceId::dummy(),
                        delta: winit::event::MouseScrollDelta::LineDelta(0.0, -1.0),
                        phase: winit::event::TouchPhase::Moved,
                    },
                    UiInputOrigin::Command,
                )?;
                if self.generation != generation
                    || self.cursor_identity() != self.focus_probe.page
                    || self.previous_pages.len() != self.focus_probe.previous
                {
                    return Err("trace_focus_track_scroll_changed_page_or_analysis".into());
                }
            }
            6 => {
                if self
                    .timeline
                    .as_ref()
                    .is_none_or(|t| (t.scroll - self.track_scroll_max).abs() > 0.1)
                    || self.focus_probe.before_scroll.as_ref().is_none_or(|a| {
                        self.report
                            .as_ref()
                            .is_none_or(|r| !Arc::ptr_eq(a, &r.analysis))
                    })
                    || self.cursor_identity() != self.focus_probe.page
                    || self.previous_pages.len() != self.focus_probe.previous
                {
                    return Err("trace_focus_scrolled_evidence_changed".into());
                }
                for _ in 0..64 {
                    if self.timeline.as_ref().is_some_and(|t| t.scroll == 0.0) {
                        break;
                    }
                    self.focus_key(NamedKey::PageUp, KeyCode::PageUp)?;
                }
                if self.timeline.as_ref().is_none_or(|t| t.scroll != 0.0) {
                    return Err("trace_focus_track_top_missing".into());
                }
                println!(
                    "trace_focus_check command_origin=true tracks_tab_page_end=true wheel_keeps_page_history=true analysis_retained=true"
                );
                self.focus_by_tab(vesta_gpui::ElementId::new(100))?;
                self.focus_key(NamedKey::Enter, KeyCode::Enter)?;
            }
            7 => {
                let (element, timer) = self.first_timer_target()?;
                self.focus_probe.old_element = Some(element);
                self.focus_probe.old_timer = Some(timer);
                self.focus_probe.old_node = self.semantics.node_for_element(element);
                if self.focus_probe.old_node.is_none() {
                    return Err("trace_focus_old_semantics_missing".into());
                }
                // Search is application intent and intentionally retains focus
                // across a load; it must not keep another view's old capture.
                self.command(Command::FocusSearch)?;
                self.nav_point(0.3)?;
                self.nav_button(MouseButton::Right, ElementState::Pressed)?;
                self.nav_point(0.5)?;
                if self.ownership.capture().is_none() {
                    return Err("trace_focus_source_preview_capture_missing".into());
                }
                let path = self
                    .focus_probe
                    .alternate
                    .clone()
                    .ok_or("trace_focus_alternate_missing")?;
                self.input(&Event::DroppedFile(path), UiInputOrigin::Command)?;
            }
            8 => {
                let source = self
                    .session
                    .as_ref()
                    .ok_or("trace_session_missing")?
                    .source
                    .sha256
                    .clone();
                if Some(&source) == self.focus_probe.source_a.as_ref() {
                    return Err("trace_focus_source_not_replaced".into());
                }
                let old = self
                    .focus_probe
                    .old_node
                    .ok_or("trace_focus_old_node_missing")?;
                let (element, timer) = self.first_timer_target()?;
                let new = self
                    .semantics
                    .node_for_element(element)
                    .ok_or("trace_focus_new_node_missing")?;
                if Some(element) != self.focus_probe.old_element {
                    return Err("trace_focus_reused_slot_fixture_required".into());
                }
                let leaked_capture = self.ownership.capture().is_some();
                let accepted = self.accessible_action(ActionRequest {
                    action: Action::Click,
                    target: old,
                    data: None,
                })?;
                println!(
                    "trace_focus_source {}",
                    serde_json::json!({"schema_version":1,"analysis_version":crate::query::protocol::ANALYSIS_VERSION,
                    "source_a":self.focus_probe.source_a,"source_b":source,"old_node":old.0.to_string(),"new_node":new.0.to_string(),
                    "reused_element":element.get().to_string(),"old_target_accepted":accepted,"old_capture_retained":leaked_capture})
                );
                if accepted
                    || leaked_capture
                    || old == new
                    || self.drag.is_some()
                    || self.page.is_some()
                    || !self.previous_pages.is_empty()
                {
                    return Err("trace_source_epoch_or_capture_not_retired".into());
                }
                self.nav_button(MouseButton::Right, ElementState::Released)?;
                if !self.accessible_action(ActionRequest {
                    action: Action::Click,
                    target: new,
                    data: None,
                })? || !self.timer_is(&timer)
                {
                    return Err("trace_focus_new_source_action_rejected".into());
                }
                self.focus_probe.old_timer = Some(timer);
            }
            9 => {
                let timer = self
                    .focus_probe
                    .old_timer
                    .as_ref()
                    .ok_or("trace_focus_current_timer_missing")?;
                if !self.timer_is(timer) || self.ownership.capture().is_some() {
                    return Err("trace_focus_current_source_report_missing".into());
                }
                let source = self.session.as_ref().ok_or("trace_session_missing")?;
                println!(
                    "trace_focus_data {}",
                    serde_json::json!({"schema_version":1,
                    "analysis_version":crate::query::protocol::ANALYSIS_VERSION,"source_sha256":source.source.sha256,
                    "timer":{"category":timer.category,"name":timer.name}})
                );
                println!(
                    "trace_focus_complete command_origin=true steps=10 keyboard_views=true busy_focus=true paging_retained=true old_source_targets_retired=true capture_retired=true new_source_action=true"
                );
                event_loop.exit();
            }
            _ => return Err("trace_focus_step_overrun".into()),
        }
        self.redraw();
        Ok(())
    }
}
