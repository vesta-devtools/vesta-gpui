//! Committed native intervals, camera gestures and resize cancellation.
//! Only finite acceptance uses this state; input and analysis remain production paths.
use super::*;

#[derive(Default)]
pub(in crate::native) struct State {
    range: Option<TimeRange>,
    preview: Option<TimeRange>,
    analysis_before_resize: Option<Arc<crate::analysis::Analysis>>,
    focus_before_resize: Option<InputTarget>,
}

impl App {
    pub(super) fn nav_point(&mut self, fraction: f32) -> Result<()> {
        let scale = self
            .window
            .as_ref()
            .ok_or("trace_window_missing")?
            .scale_factor();
        self.input(
            &Event::CursorMoved {
                device_id: DeviceId::dummy(),
                position: winit::dpi::PhysicalPosition::new(
                    f64::from(self.area.x + self.area.width * fraction) * scale,
                    f64::from(self.area.y + self.area.height * 0.8) * scale,
                ),
            },
            UiInputOrigin::Command,
        )?;
        Ok(())
    }
    pub(super) fn nav_button(&mut self, button: MouseButton, state: ElementState) -> Result<()> {
        self.input(
            &Event::MouseInput {
                device_id: DeviceId::dummy(),
                button,
                state,
            },
            UiInputOrigin::Command,
        )?;
        Ok(())
    }
    fn nav_wheel(&mut self, modifiers: ModifiersState) -> Result<()> {
        self.input(
            &Event::ModifiersChanged(modifiers.into()),
            UiInputOrigin::Command,
        )?;
        self.input(
            &Event::MouseWheel {
                device_id: DeviceId::dummy(),
                delta: winit::event::MouseScrollDelta::LineDelta(0.0, 1.0),
                phase: winit::event::TouchPhase::Moved,
            },
            UiInputOrigin::Command,
        )?;
        self.input(
            &Event::ModifiersChanged(ModifiersState::empty().into()),
            UiInputOrigin::Command,
        )?;
        Ok(())
    }
    fn nav_time_at_pointer(&self) -> Result<u64> {
        let timeline = self.timeline.as_ref().ok_or("trace_timeline_missing")?;
        Ok(timeline.at(f64::from((self.pointer.x - self.area.x) / self.area.width)))
    }
    fn nav_preview_range(&mut self, from: f32, to: f32) -> Result<TimeRange> {
        let generation = self.generation;
        self.nav_point(from)?;
        let start = self.nav_time_at_pointer()?;
        self.nav_button(MouseButton::Left, ElementState::Pressed)?;
        self.nav_point(to)?;
        let end = self.nav_time_at_pointer()?;
        let expected =
            TimeRange::new(start.min(end), start.max(end)).ok_or("trace_navigation_empty_range")?;
        if self.timeline.as_ref().and_then(|t| t.selection) != Some(expected)
            || self.ownership.capture().is_none()
            || self.generation != generation
        {
            return Err("trace_navigation_range_preview_not_deferred".into());
        }
        Ok(expected)
    }
    fn nav_release_range(&mut self, expected: TimeRange) -> Result<()> {
        let generation = self.generation;
        self.nav_button(MouseButton::Left, ElementState::Released)?;
        if self.ownership.capture().is_some()
            || self.drag.is_some()
            || self.generation != generation + 1
        {
            return Err("trace_navigation_range_not_committed_once".into());
        }
        self.navigation.range = Some(expected);
        Ok(())
    }
    fn nav_commit_range(&mut self, from: f32, to: f32) -> Result<()> {
        let expected = self.nav_preview_range(from, to)?;
        self.nav_release_range(expected)
    }
    fn nav_evidence(&self, step: usize, name: &str) -> Result<()> {
        let report = self
            .report
            .as_ref()
            .ok_or("trace_navigation_report_missing")?;
        let session = self.session.as_ref().ok_or("trace_session_missing")?;
        let timeline = self.timeline.as_ref().ok_or("trace_timeline_missing")?;
        let range = self.navigation.range.unwrap_or(session.bounds);
        if report.analysis.range != range
            || self.drag.is_none() && timeline.analysis_range() != range
            || Some(&session.source.sha256) != self.smoke_source.as_ref()
        {
            return Err("trace_navigation_presented_stale_analysis".into());
        }
        if self.displayed_analysis_range != Some(range) {
            return Err("trace_navigation_header_relabels_previous_statistics".into());
        }
        let timers = report
            .analysis
            .timers
            .iter()
            .take(8)
            .map(|timer| {
                serde_json::json!({
                    "timer":{"category":timer.category,"name":timer.name},"calls":timer.calls,
                    "inclusive_ns":timer.inclusive,"self_ns":timer.self_time,
                })
            })
            .collect::<Vec<_>>();
        println!(
            "trace_navigation_evidence {}",
            serde_json::json!({
                "schema_version":1,"step":step,"name":name,"source_sha256":session.source.sha256,
                "analysis_version":crate::query::protocol::ANALYSIS_VERSION,
                "range_ns":[range.start,range.end],"view_ns":[timeline.view.start,timeline.view.end],
                "intent_range_ns":[timeline.analysis_range().start,timeline.analysis_range().end],
                "frames_count":report.analysis.frames.len(),"percentile_ns":report.analysis.percentile_ns,
                "timers_total":report.analysis.timers.len(),"timers":timers,
            })
        );
        Ok(())
    }
    pub(super) fn navigation_presented(
        &mut self,
        event_loop: &ActiveEventLoop,
        step: usize,
    ) -> Result<()> {
        match step {
            0 => {
                if !self.filter.is_empty() {
                    return Err("trace_navigation_fixture_requires_unfiltered_source".into());
                }
                self.smoke_source = self.session.as_ref().map(|s| s.source.sha256.clone());
                self.nav_evidence(step, "initial")?;
                self.navigation.preview = Some(self.nav_preview_range(0.2, 0.6)?);
            }
            1 => {
                self.nav_evidence(step, "range_preview")?;
                let range = self
                    .navigation
                    .preview
                    .take()
                    .ok_or("trace_navigation_preview_missing")?;
                self.nav_release_range(range)?;
            }
            2 => {
                self.nav_evidence(step, "forward_range")?;
                self.nav_commit_range(0.7, 0.3)?;
            }
            3 => {
                self.nav_evidence(step, "reverse_range")?;
                self.nav_point(0.75)?;
                let anchor = self.nav_time_at_pointer()?;
                let before = self.timeline.as_ref().ok_or("trace_timeline_missing")?.view;
                let generation = self.generation;
                self.nav_wheel(ModifiersState::CONTROL)?;
                let after = self.timeline.as_ref().ok_or("trace_timeline_missing")?;
                if after.view.duration() >= before.duration()
                    || self.nav_time_at_pointer()?.abs_diff(anchor) > 1
                    || after.selection != self.navigation.range
                    || self.generation != generation + 1
                {
                    return Err("trace_navigation_zoom_anchor_or_range".into());
                }
            }
            4 => {
                self.nav_evidence(step, "anchored_zoom")?;
                let before = self.timeline.as_ref().ok_or("trace_timeline_missing")?.view;
                let generation = self.generation;
                self.nav_point(0.5)?;
                self.nav_button(MouseButton::Right, ElementState::Pressed)?;
                self.nav_point(0.6)?;
                if self.generation != generation {
                    return Err("trace_navigation_pan_preview_committed_early".into());
                }
                self.nav_button(MouseButton::Right, ElementState::Released)?;
                let after = self.timeline.as_ref().ok_or("trace_timeline_missing")?;
                if after.view.start >= before.start
                    || after.view.duration() != before.duration()
                    || after.selection != self.navigation.range
                    || self.generation != generation + 1
                    || self.ownership.capture().is_some()
                    || self.drag.is_some()
                {
                    return Err("trace_navigation_pan_commit".into());
                }
            }
            5 => {
                self.nav_evidence(step, "pointer_pan")?;
                let before = self.timeline.as_ref().ok_or("trace_timeline_missing")?.view;
                let generation = self.generation;
                self.nav_point(0.5)?;
                self.nav_wheel(ModifiersState::SHIFT)?;
                let after = self.timeline.as_ref().ok_or("trace_timeline_missing")?;
                if after.view.start >= before.start
                    || after.view.duration() != before.duration()
                    || after.selection != self.navigation.range
                    || self.generation != generation + 1
                {
                    return Err("trace_navigation_shift_wheel_pan".into());
                }
            }
            6 => {
                self.nav_evidence(step, "wheel_pan")?;
                self.logical_key(Key::Named(NamedKey::Home), None, KeyCode::Home)?;
                self.navigation.range = None;
            }
            7 => {
                self.nav_evidence(step, "fit")?;
                self.navigation.analysis_before_resize =
                    self.report.as_ref().map(|r| r.analysis.clone());
                self.nav_point(0.2)?;
                self.nav_button(MouseButton::Left, ElementState::Pressed)?;
                self.nav_point(0.5)?;
                if self.ownership.capture().is_none() {
                    return Err("trace_navigation_resize_preview_missing".into());
                }
                self.navigation.focus_before_resize = self.ownership.focus();
                let resized = self
                    .window
                    .as_ref()
                    .ok_or("trace_window_missing")?
                    .request_inner_size(LogicalSize::new(980.0, 720.0));
                println!("trace_navigation_resize requested_logical=980x720 immediate={resized:?}");
                self.redraw();
            }
            8 => {
                self.nav_evidence(step, "resize_cancel")?;
                let report = self
                    .report
                    .as_ref()
                    .ok_or("trace_navigation_report_missing")?;
                if self.ownership.capture().is_some()
                    || self.drag.is_some()
                    || self
                        .navigation
                        .analysis_before_resize
                        .as_ref()
                        .is_none_or(|a| !Arc::ptr_eq(a, &report.analysis))
                {
                    return Err("trace_navigation_resize_committed_preview".into());
                }
                let old = self.navigation.focus_before_resize;
                let now = self.ownership.focus();
                if now != old
                    && (now.is_some()
                        || old.is_some_and(|t| {
                            self.route
                                .as_ref()
                                .is_some_and(|r| FocusScope::new(WINDOW, r, None).accepts(t))
                        }))
                {
                    return Err("trace_navigation_resize_focus_redirected".into());
                }
                let generation = self.generation;
                self.nav_button(MouseButton::Left, ElementState::Released)?;
                if self.generation != generation {
                    return Err("trace_navigation_late_release_committed".into());
                }
                println!(
                    "trace_navigation_complete command_origin=true steps=9 preview_keeps_statistics_range=true forward_reverse_range=true anchored_zoom=true pointer_wheel_pan=true resize_cancel=true source_unchanged=true"
                );
                event_loop.exit();
            }
            _ => return Err("trace_navigation_step_overrun".into()),
        }
        self.redraw();
        Ok(())
    }
}
