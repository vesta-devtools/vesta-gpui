//! Real SDK/allocator + native action-route acceptance, without desktop input.
use super::*;
use crate::{
    inspection::{MemoryInspection, MemoryMode},
    query::protocol::Data,
};
impl App {
    fn memory_evidence(&self) -> Result<&MemoryInspection> {
        if self.panel != Panel::Memory {
            return Err("trace_memory_panel_missing".into());
        }
        self.report
            .as_ref()
            .and_then(|r| r.inspection.memory.as_ref())
            .ok_or_else(|| "trace_memory_inspection_missing".into())
    }
    fn memory_cohort_rows(&self, mode: MemoryMode) -> Result<()> {
        let m = self.memory_evidence()?;
        if m.mode != mode {
            return Err("trace_memory_mode_mismatch".into());
        }
        let c = m
            .comparison
            .as_ref()
            .ok_or("trace_memory_comparison_missing")?;
        let expected = match mode {
            MemoryMode::LiveAtEnd => c.observed_live_at_end.allocations,
            MemoryMode::NewSurvivors => c.new_survivors.allocations,
            MemoryMode::Released => c.released.allocations,
            MemoryMode::Common => c.common.allocations,
            MemoryMode::Transient => c.transient.allocations,
            MemoryMode::All => return Ok(()),
        };
        let page = self
            .report
            .as_ref()
            .and_then(|r| r.inspection.page())
            .ok_or("trace_memory_page_missing")?;
        if page.total != expected {
            return Err(format!(
                "trace_memory_cohort_count_mismatch expected={expected} actual={}",
                page.total
            )
            .into());
        }
        println!("trace_memory_check command_origin=true cohort={mode:?} total={expected}");
        Ok(())
    }
    fn memory_range_drag(&mut self) -> Result<()> {
        let scale = self
            .window
            .as_ref()
            .ok_or("trace_window_missing")?
            .scale_factor();
        let points = &self
            .session
            .as_ref()
            .ok_or("trace_memory_session_missing")?
            .memory
            .as_ref()
            .ok_or("trace_memory_missing")?
            .points;
        let timeline = self
            .timeline
            .as_ref()
            .ok_or("trace_memory_timeline_missing")?;
        let fractions = [
            timeline.fraction(points[points.len() / 4].time_ns) as f32,
            timeline.fraction(points[points.len() * 3 / 4].time_ns) as f32,
        ];
        for (fraction, state) in [
            (fractions[0], ElementState::Pressed),
            (fractions[1], ElementState::Released),
        ] {
            self.input(
                &Event::CursorMoved {
                    device_id: DeviceId::dummy(),
                    position: winit::dpi::PhysicalPosition::new(
                        f64::from(self.area.x + self.area.width * fraction) * scale,
                        f64::from(self.area.y + self.area.height * 0.7) * scale,
                    ),
                },
                UiInputOrigin::Command,
            )?;
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
    pub(in crate::native) fn memory_capture_presented(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<()> {
        if let Some(error) = self.live_status["message"].as_str() {
            return Err(format!("trace_memory_control_error: {error}").into());
        }
        if self.live_pending
            || self.busy()
            || self.capture_waiting
            || self.session.is_some()
                && self.capture_dir.is_some()
                && self.captured_step != Some(self.smoke_step)
        {
            return Ok(());
        }
        match self.smoke_step {
            0 => self.click_element(vesta_gpui::ElementId::new(160))?,
            1 => {
                if self.live_status["state"] != "idle" {
                    return Ok(());
                }
                // SDK control becomes available before asynchronous Vulkan
                // initialization. This fixture targets a running host workload.
                let ready = self
                    .memory_warmup
                    .get_or_insert_with(|| Instant::now() + Duration::from_secs(2));
                if Instant::now() < *ready {
                    return Ok(());
                }
                if self.live_status["memory_supported"] != true {
                    return Err("trace_memory_allocator_not_installed".into());
                }
                self.click_element(vesta_gpui::ElementId::new(170))?;
            }
            2 => {
                if !self.live_memory {
                    return Err("trace_memory_toggle_not_applied".into());
                }
                self.click_element(vesta_gpui::ElementId::new(164))?;
            }
            3 => {
                if self.live_status["state"] != "idle" || self.session.is_none() {
                    return Ok(());
                }
                let s = self.session.as_ref().expect("admitted memory fixture");
                let m = s
                    .memory
                    .as_ref()
                    .ok_or("trace_memory_host_capture_missing")?;
                if m.allocations.is_empty()
                    || m.capture.stacks.is_empty()
                    || m.resolved_frames == 0
                    || s.frames.is_empty()
                    || s.gpu.is_none()
                {
                    return Err("trace_memory_real_host_evidence_missing".into());
                }
                println!(
                    "trace_memory_source {}",
                    serde_json::json!({"source":s.source,"path":self.source_path,"allocations":m.allocations.len(),"operations":m.points.len(),"resolved_frames":m.resolved_frames,"through_ns":m.capture.elapsed_ns})
                );
                self.smoke_source = Some(s.source.sha256.clone());
                self.click_element(vesta_gpui::ElementId::new(118))?;
            }
            4 => {
                let m = self.memory_evidence()?;
                if m.points.is_empty() {
                    return Err("trace_memory_curve_missing".into());
                }
                let s = self.session.as_ref().expect("admitted memory fixture");
                let raw = s.memory.as_ref().expect("admitted memory fixture");
                let Some(Data::MemoryAllocations { items, .. }) = self
                    .report
                    .as_ref()
                    .and_then(|r| r.inspection.table.as_ref())
                else {
                    return Err("trace_memory_allocations_missing".into());
                };
                let a = items
                    .iter()
                    .take(self.table_page_rows)
                    .find(|a| a.stack.is_some())
                    .ok_or("trace_memory_stack_row_missing")?;
                self.click_element(vesta_gpui::ElementId::new(
                    30_000_000 + raw.indices[&a.key.allocation] as u64,
                ))?;
            }
            5 => {
                let m = self.memory_evidence()?;
                if m.allocation.is_none() || m.stack.as_ref().is_none_or(|s| s.frames.is_empty()) {
                    return Err("trace_memory_detail_stack_missing".into());
                }
                self.scroll_gpu_evidence(-8.0)?;
                self.click_element(vesta_gpui::ElementId::new(2002))?;
            }
            6 => {
                self.memory_cohort_rows(MemoryMode::LiveAtEnd)?;
                self.click_element(vesta_gpui::ElementId::new(2004))?;
            }
            7 => {
                self.memory_cohort_rows(MemoryMode::NewSurvivors)?;
                self.click_element(vesta_gpui::ElementId::new(2006))?;
            }
            8 => {
                self.memory_cohort_rows(MemoryMode::Released)?;
                self.memory_range_drag()?;
            }
            9 => {
                if self.timeline.as_ref().is_none_or(|t| t.selection.is_none()) {
                    return Err("trace_memory_range_selection_missing".into());
                }
                self.memory_cohort_rows(MemoryMode::Released)?;
                if self
                    .memory_evidence()?
                    .comparison
                    .as_ref()
                    .is_none_or(|c| c.released.allocations == 0)
                {
                    return Err("trace_memory_selected_interval_has_no_released_evidence".into());
                }
                self.click_element(vesta_gpui::ElementId::new(2008))?;
            }
            10 => {
                self.memory_cohort_rows(MemoryMode::Common)?;
                self.click_element(vesta_gpui::ElementId::new(2010))?;
            }
            11 => {
                self.memory_cohort_rows(MemoryMode::Transient)?;
                if self
                    .memory_evidence()?
                    .comparison
                    .as_ref()
                    .is_none_or(|c| c.transient.allocations == 0)
                {
                    return Err("trace_memory_selected_interval_has_no_transient_evidence".into());
                }
                let _ = self
                    .window
                    .as_ref()
                    .expect("admitted memory fixture")
                    .request_inner_size(LogicalSize::new(980.0, 720.0));
            }
            12 => {
                self.memory_cohort_rows(MemoryMode::Transient)?;
                if self.session.as_ref().map(|s| &s.source.sha256) != self.smoke_source.as_ref() {
                    return Err("trace_memory_source_mutated".into());
                }
                println!(
                    "trace_memory_complete command_origin=true steps=13 real_allocator=true local_symbols=true memory_toggle=true range_cohorts=true resize=true source_unchanged=true"
                );
                event_loop.exit();
            }
            _ => return Err("trace_memory_step_overrun".into()),
        }
        self.smoke_step += 1;
        self.redraw();
        Ok(())
    }
}
