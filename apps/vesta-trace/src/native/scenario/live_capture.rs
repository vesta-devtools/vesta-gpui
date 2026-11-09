//! Actual capture controls, target SDK and import/analysis publication, without desktop input.
use super::*;
use crate::query::protocol::Data;

impl App {
    fn live_frame_evidence(&self) -> Result<()> {
        if self.panel != Panel::Frames {
            return Err("trace_live_default_frame_panel_missing".into());
        }
        let session = self.session.as_ref().ok_or("trace_live_session_missing")?;
        if session.frames.is_empty() || session.gpu.is_none() {
            return Err("trace_live_host_cpu_gpu_evidence_missing".into());
        }
        if !matches!(self.report.as_ref().and_then(|r|r.inspection.table.as_ref()),Some(Data::FrameTimings{items,..}) if !items.is_empty())
        {
            return Err("trace_live_frame_comparison_missing".into());
        }
        println!(
            "trace_live_capture_evidence {}",
            serde_json::json!({"source":session.source,"path":self.source_path,
            "frames":session.frames.len(),"gpu_records":session.gpu.as_ref().map(|g|g.submissions.len())})
        );
        Ok(())
    }
    pub(in crate::native) fn live_capture_presented(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<()> {
        if self.live_status["message"].is_string() {
            return Err(
                format!("trace_live_control_error: {}", self.live_status["message"]).into(),
            );
        }
        if self.live_pending || self.busy() {
            return Ok(());
        }
        match self.smoke_step {
            0 => {
                self.click_element(vesta_gpui::ElementId::new(160))?;
            }
            1 => {
                if self.live_status["state"] != "idle" {
                    return Ok(());
                }
                self.click_element(vesta_gpui::ElementId::new(164))?;
            }
            2 => {
                if self.live_status["state"] != "idle" || self.session.is_none() {
                    return Ok(());
                }
                self.live_frame_evidence()?;
                self.smoke_source = self.session.as_ref().map(|s| s.source.sha256.clone());
                println!("trace_live_check command_origin=true auto_stop_save_analyze=true");
                self.click_element(vesta_gpui::ElementId::new(164))?;
            }
            3 => {
                if self.live_status["state"] != "recording"
                    || self.live_status["events"].as_u64().unwrap_or(0) < 20
                {
                    return Ok(());
                }
                self.click_element(vesta_gpui::ElementId::new(168))?;
            }
            4 => {
                if self.live_status["state"] != "idle"
                    || self
                        .session
                        .as_ref()
                        .is_none_or(|s| Some(&s.source.sha256) == self.smoke_source.as_ref())
                {
                    return Ok(());
                }
                self.live_frame_evidence()?;
                println!(
                    "trace_live_check command_origin=true repeated_stop_save_analyze=true default_frame_comparison=true"
                );
                self.click_element(vesta_gpui::ElementId::new(164))?;
            }
            5 => {
                if self.live_status["state"] != "recording"
                    || self.live_status["events"].as_u64().unwrap_or(0) < 20
                {
                    return Ok(());
                }
                println!(
                    "trace_live_complete command_origin=true steps=6 close_with_owned_recording=true"
                );
                event_loop.exit();
            }
            _ => return Err("trace_live_step_overrun".into()),
        }
        self.smoke_step += 1;
        self.redraw();
        Ok(())
    }
}
