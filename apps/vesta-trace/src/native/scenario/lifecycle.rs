//! Finite real-worker publication ordering. Synthetic files are not timing data.
use super::*;

pub(in crate::native) struct State {
    alternate: Option<PathBuf>,
    original: Option<PathBuf>,
    source_a: Option<String>,
    report_a: Option<Arc<crate::analysis::Analysis>>,
    expected: Option<(u64, &'static str)>,
    observed: Option<&'static str>,
    cancelled: Option<u64>,
    superseded: Option<u64>,
    pub(in crate::native) stale: Vec<u64>,
    pub(in crate::native) cancel_followup: bool,
    pub(in crate::native) cancelled_followup: Option<u64>,
}
impl State {
    pub(in crate::native) fn new(alternate: Option<PathBuf>) -> Self {
        Self {
            alternate,
            original: None,
            source_a: None,
            report_a: None,
            expected: None,
            observed: None,
            cancelled: None,
            superseded: None,
            stale: Vec::new(),
            cancel_followup: false,
            cancelled_followup: None,
        }
    }
}
impl App {
    pub(in crate::native) fn expected_work_error(
        &mut self,
        generation: u64,
        error: &TraceError,
    ) -> bool {
        if self.scene != Scene::Lifecycle
            || self.lifecycle.expected != Some((generation, error.code))
        {
            return false;
        }
        self.lifecycle.expected = None;
        self.lifecycle.observed = Some(error.code);
        println!(
            "trace_lifecycle_expected_error generation={generation} code={} retained_source={}",
            error.code,
            self.session.as_ref().map_or("none", |s| &s.source.sha256)
        );
        true
    }
    fn lifecycle_drop(&mut self, path: PathBuf) -> Result<()> {
        self.input(&Event::DroppedFile(path), UiInputOrigin::Command)?;
        Ok(())
    }
    fn lifecycle_draft(&mut self, event_loop: &ActiveEventLoop, text: &str) -> Result<()> {
        self.command(Command::FocusSearch)?;
        // Publish the real busy search geometry before delivering text.
        self.draw(event_loop)?;
        self.input(&Event::Text(text.into()), UiInputOrigin::Command)?;
        self.apply_search()?;
        if self.search.input.text() != text {
            return Err("trace_lifecycle_search_draft_missing".into());
        }
        Ok(())
    }
    fn lifecycle_a_retained(&self) -> Result<()> {
        if self.session.as_ref().map(|s| &s.source.sha256) != self.lifecycle.source_a.as_ref()
            || !self
                .report
                .as_ref()
                .zip(self.lifecycle.report_a.as_ref())
                .is_some_and(|(r, a)| Arc::ptr_eq(&r.analysis, a))
            || !self.filter.is_empty()
            || self.busy()
            || self.ownership.capture().is_some()
        {
            return Err("trace_lifecycle_validated_view_not_retained".into());
        }
        Ok(())
    }
    pub(super) fn lifecycle_presented(
        &mut self,
        event_loop: &ActiveEventLoop,
        step: usize,
    ) -> Result<()> {
        match step {
            0 => {
                self.lifecycle.original = self.source_path.clone();
                self.lifecycle.source_a = self.session.as_ref().map(|s| s.source.sha256.clone());
                self.lifecycle.report_a = self.report.as_ref().map(|r| r.analysis.clone());
                let directory = self
                    .capture_dir
                    .clone()
                    .ok_or("trace_lifecycle_capture_required")?;
                let malformed = directory.join("malformed.json");
                use std::io::Write;
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(malformed)?
                    .write_all(b"{\"schema_version\":")?;
                self.nav_point(0.3)?;
                self.nav_button(MouseButton::Right, ElementState::Pressed)?;
                self.nav_point(0.6)?;
                self.lifecycle_drop(directory.join("missing.json"))?;
                let generation = self.generation;
                self.lifecycle.expected = Some((generation, "trace_open"));
                self.command(Command::Next)?;
                self.command(Command::Panel(Panel::Events))?;
                self.lifecycle_draft(event_loop, "Common B")?;
                if self.generation != generation
                    || !self.importing()
                    || self.drag.is_some()
                    || self.ownership.capture().is_some()
                {
                    return Err("trace_lifecycle_old_source_replaced_import".into());
                }
                println!(
                    "trace_lifecycle_check command_origin=true import_exclusive=true preview_cancelled_on_start=true draft_deferred=true"
                );
            }
            1 => {
                self.lifecycle_a_retained()?;
                if self.lifecycle.observed.take() != Some("trace_open")
                    || self.search.input.text() != "Common B"
                    || !self.status.contains("missing.json")
                    || !self.status.contains(&self.source_name())
                {
                    return Err("trace_lifecycle_failed_import_context".into());
                }
                self.command(Command::ClearSearch)?;
                self.lifecycle_drop(
                    self.lifecycle
                        .alternate
                        .clone()
                        .ok_or("trace_lifecycle_alternate_missing")?,
                )?;
                self.lifecycle.cancelled = Some(self.generation);
                self.logical_key(Key::Named(NamedKey::Escape), None, KeyCode::Escape)?;
                self.lifecycle_a_retained()?;
            }
            2 => {
                self.lifecycle_a_retained()?;
                if !self
                    .lifecycle
                    .cancelled
                    .is_some_and(|g| self.lifecycle.stale.contains(&g))
                {
                    return Err("trace_lifecycle_cancel_stale_not_observed".into());
                }
                self.lifecycle_drop(
                    self.capture_dir
                        .as_ref()
                        .ok_or("trace_capture_missing")?
                        .join("malformed.json"),
                )?;
                self.lifecycle.expected = Some((self.generation, "trace_json"));
                println!(
                    "trace_lifecycle_check command_origin=true cancel_fenced_actual_reply=true previous_report_retained=true"
                );
            }
            3 => {
                self.lifecycle_a_retained()?;
                if self.lifecycle.observed.take() != Some("trace_json") {
                    return Err("trace_lifecycle_malformed_not_observed".into());
                }
                self.lifecycle_drop(
                    self.lifecycle
                        .alternate
                        .clone()
                        .ok_or("trace_lifecycle_alternate_missing")?,
                )?;
                self.lifecycle.superseded = Some(self.generation);
                self.lifecycle_drop(
                    self.lifecycle
                        .original
                        .clone()
                        .ok_or("trace_lifecycle_original_missing")?,
                )?;
            }
            4 => {
                if self.session.as_ref().map(|s| &s.source.sha256)
                    != self.lifecycle.source_a.as_ref()
                    || !self
                        .lifecycle
                        .superseded
                        .is_some_and(|g| self.lifecycle.stale.contains(&g))
                {
                    return Err("trace_lifecycle_replacement_not_latest".into());
                }
                let before = self
                    .report
                    .as_ref()
                    .ok_or("trace_report_missing")?
                    .analysis
                    .clone();
                let range = self.timeline.as_ref().ok_or("trace_timeline_missing")?.view;
                self.command(Command::ZoomIn)?;
                self.draw(event_loop)?;
                // Exercise the published Cancel button through current hit geometry.
                self.click_element(vesta_gpui::ElementId::new(86))?;
                if self.busy()
                    || self.timeline.as_ref().is_none_or(|t| t.view != range)
                    || !self
                        .report
                        .as_ref()
                        .is_some_and(|r| Arc::ptr_eq(&r.analysis, &before))
                {
                    return Err("trace_lifecycle_analysis_cancel_not_restored".into());
                }
                self.lifecycle_drop(
                    self.lifecycle
                        .alternate
                        .clone()
                        .ok_or("trace_lifecycle_alternate_missing")?,
                )?;
                self.lifecycle_draft(event_loop, "Common B")?;
                self.lifecycle.cancel_followup = true;
            }
            5 => {
                if self.session.as_ref().map(|s| &s.source.sha256)
                    == self.lifecycle.source_a.as_ref()
                    || self.busy()
                    || !self.filter.is_empty()
                    || self.search.input.text() != "Common B"
                    || !self
                        .lifecycle
                        .cancelled_followup
                        .is_some_and(|g| self.lifecycle.stale.contains(&g))
                {
                    return Err("trace_lifecycle_followup_cancel_crossed_source".into());
                }
                self.logical_key(Key::Named(NamedKey::Enter), None, KeyCode::Enter)?;
                println!(
                    "trace_lifecycle_check command_origin=true latest_import_wins=true cancel_button=true canonical_new_source_restored=true draft_retained=true"
                );
            }
            6 => {
                if self.filter != "Common B"
                    || !self.report.as_ref().is_some_and(|r| {
                        !r.analysis.timers.is_empty()
                            && r.analysis.timers.iter().all(|t| t.name == "Common B")
                    })
                {
                    return Err("trace_lifecycle_deferred_search_not_applied".into());
                }
                self.lifecycle_drop(
                    self.lifecycle
                        .original
                        .clone()
                        .ok_or("trace_lifecycle_original_missing")?,
                )?;
                if !self.importing() {
                    return Err("trace_lifecycle_close_not_pending".into());
                }
                println!(
                    "trace_lifecycle_complete command_origin=true steps=7 missing_and_malformed=true actual_stale_replies=true cancel_restores=true replacement=true deferred_search=true close_pending=true source_sha256={}",
                    self.session
                        .as_ref()
                        .ok_or("trace_source_missing")?
                        .source
                        .sha256
                );
                let id = self.window.as_ref().ok_or("trace_window_missing")?.id();
                self.window_event(event_loop, id, WindowEvent::CloseRequested);
            }
            _ => return Err("trace_lifecycle_step".into()),
        }
        self.redraw();
        Ok(())
    }
}
