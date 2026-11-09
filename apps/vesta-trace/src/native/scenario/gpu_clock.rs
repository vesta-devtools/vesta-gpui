//! Actual estimated GPU bars, original raw evidence and shared CPU time selection.
use super::*;
impl App {
    pub(in crate::native) fn gpu_clock_presented(
        &mut self,
        event_loop: &ActiveEventLoop,
        step: usize,
    ) -> Result<()> {
        match step {
            0 => {
                let session = self.session.as_ref().ok_or("trace_clock_source_missing")?;
                if session.gpu_counts.mapped == 0 || session.gpu_tracks.is_empty() {
                    return Err("trace_clock_real_mapped_evidence_required".into());
                }
                self.smoke_source = Some(session.source.sha256.clone());
                self.command(Command::Panel(Panel::Gpu))?;
            }
            1 => {
                let route = self.route.as_ref().ok_or("trace_clock_route_missing")?;
                let element = self
                    .layout_tree
                    .as_ref()
                    .ok_or("trace_clock_layout_missing")?
                    .nodes()
                    .iter()
                    .find_map(|node| {
                        matches!(route.action_at(node.id), Some(Command::GpuExecution(_)))
                            .then_some(node.id)
                    })
                    .ok_or("trace_clock_gpu_bar_not_realized")?;
                self.click_element(element)?;
            }
            2 => {
                let item = self
                    .report
                    .as_ref()
                    .and_then(|r| r.inspection.gpu.as_ref())
                    .ok_or("trace_clock_detail_missing")?;
                let evidence = item
                    .execution
                    .as_ref()
                    .ok_or("trace_clock_execution_missing")?;
                let expected = self
                    .session
                    .as_ref()
                    .and_then(|s| {
                        s.gpu_execution
                            .get(&(item.key.generation, item.key.submission))
                    })
                    .ok_or("trace_clock_original_key_missing")?;
                if evidence.estimated_start_ns != expected.start_ns
                    || evidence.estimated_end_ns != expected.end_ns
                    || item.calibration.is_none()
                    || evidence.drift_bound_known
                {
                    return Err("trace_clock_projection_differs_from_raw_evidence".into());
                }
                let range = self
                    .timeline
                    .as_ref()
                    .ok_or("trace_clock_timeline_missing")?
                    .analysis_range();
                let full = self
                    .timeline
                    .as_ref()
                    .ok_or("trace_clock_timeline_missing")?
                    .full;
                let expected_start =
                    (expected.start_ns.max(0) as u64).clamp(full.start, full.end - 1);
                let expected_end = (expected.end_ns.max(0) as u64)
                    .max(expected_start + 1)
                    .min(full.end);
                if range.start != expected_start || range.end != expected_end {
                    return Err("trace_clock_gpu_bar_did_not_select_execution_range".into());
                }
                println!(
                    "trace_clock_check command_origin=true key={:?} start={} end={} anchor_uncertainty={} drift_bound_known=false",
                    item.key,
                    evidence.estimated_start_ns,
                    evidence.estimated_end_ns,
                    evidence.anchor_uncertainty_ns
                );
                if !matches!(self.report.as_ref().and_then(|r|r.inspection.table.as_ref()),Some(crate::query::protocol::Data::GpuExecution{items,..}) if items.iter().any(|r|r.key==item.key))
                {
                    return Err("trace_clock_selected_bar_missing_from_execution_table".into());
                }
                self.scroll_gpu_evidence(-9.0)?;
                self.command(Command::Fit)?;
            }
            3 => {
                let item = self
                    .report
                    .as_ref()
                    .and_then(|r| r.inspection.gpu.as_ref())
                    .ok_or("trace_clock_detail_missing")?;
                let session = self.session.as_ref().ok_or("trace_clock_source_missing")?;
                let name = item
                    .frame
                    .and_then(|k| session.indices.get(&k))
                    .map(|i| session.events[*i].name.clone())
                    .ok_or("trace_clock_real_frame_association_required")?;
                self.command(Command::FocusSearch)?;
                self.input(&Event::Text(name.clone()), UiInputOrigin::Command)?;
                self.smoke_filter = Some(name);
                self.apply_search()?;
            }
            4 => {
                if self
                    .report
                    .as_ref()
                    .and_then(|r| r.tracks.as_ref())
                    .is_none_or(|t| t.gpu.is_empty())
                {
                    return Err("trace_clock_shared_frame_text_filter_missing".into());
                }
                self.command(Command::ClearSearch)?;
            }
            5 => {
                let key = self.gpu.ok_or("trace_clock_selected_key_missing")?;
                self.command(Command::GpuExecution(key))?;
            }
            6 => {
                self.command(Command::Fit)?;
                let _ = self
                    .window
                    .as_ref()
                    .ok_or("trace_window_missing")?
                    .request_inner_size(LogicalSize::new(980.0, 720.0));
            }
            7 => {
                if self.session.as_ref().map(|s| &s.source.sha256) != self.smoke_source.as_ref() {
                    return Err("trace_clock_source_mutated".into());
                }
                if self
                    .report
                    .as_ref()
                    .and_then(|r| r.inspection.gpu.as_ref())
                    .and_then(|g| g.execution.as_ref())
                    .is_none()
                {
                    return Err("trace_clock_estimate_lost_after_filter_resize".into());
                }
                println!(
                    "trace_clock_complete command_origin=true steps=8 original_keys=true gpu_bar_selection=true raw_qpc=true frame_text_filter=true minimum_layout=true source_unchanged=true"
                );
                event_loop.exit();
            }
            _ => return Err("trace_clock_step_overrun".into()),
        }
        self.redraw();
        Ok(())
    }
}
