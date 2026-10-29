//! Isolated real-owner transition fixtures; never enabled in ordinary builds.
use super::*;
use std::sync::Weak;
use vesta_gpui_vulkan::{GpuContext, RecoveryFixture};

#[derive(Default)]
pub(super) struct Probe {
    flag: Option<String>,
    at_frame: u64,
    snapshot: Option<([String; 9], Weak<GpuContext>)>,
    scripted: bool,
    complete: bool,
}
impl Probe {
    pub fn pending(&self) -> bool {
        self.flag.is_some() && !self.complete
    }
}
impl App {
    pub(super) fn start_target_probe(&mut self) {
        if self.rendered_frames != 3 || self.target_probe.flag.is_some() {
            return;
        }
        let Some(flag) = std::env::args().find(|a| a.starts_with("--smoke-target-")) else {
            return;
        };
        let snapshot = self.recovery_snapshot_parts();
        let (Some(gpu), Some(window)) = (&mut self.gpu, &self.window) else {
            return;
        };
        let Some(context) = gpu.gpu_context() else {
            return;
        };
        let fixture = match flag.as_str() {
            "--smoke-target-failure" => RecoveryFixture::ReconfigurationFailure,
            "--smoke-target-out-of-date" => RecoveryFixture::ReconfigurationOutOfDate,
            "--smoke-target-device-loss" => RecoveryFixture::ReconfigurationDeviceLoss,
            "--smoke-target-surface-loss" => RecoveryFixture::ReconfigurationSurfaceLoss,
            _ => RecoveryFixture::DelayedReconfiguration,
        };
        gpu.inject_update_fixture(fixture, flag == "--smoke-target-timeout");
        self.target_probe.flag = Some(flag);
        self.target_probe.at_frame = self.rendered_frames;
        self.target_probe.snapshot = Some((snapshot, Arc::downgrade(&context)));
        let _ = window.request_inner_size(LogicalSize::new(1020.0, 730.0));
        eprintln!("UI-R11 requested: after_valid_frame=true real_owner=true");
    }
    /// True requests ordinary event-loop exit for the close fixture only.
    pub(super) fn poll_target_probe(&mut self) -> bool {
        if !self.target_probe.pending() {
            return false;
        }
        let Some(flag) = self.target_probe.flag.clone() else {
            return false;
        };
        let prepared = self.gpu.as_ref().is_some_and(GpuState::update_prepared);
        if prepared && !self.target_probe.scripted {
            self.target_probe.scripted = true;
            let blocked = self
                .window
                .as_ref()
                .is_some_and(|window| self.display_blocked(window.id()));
            if !blocked {
                self.fatal_error =
                    Some("target fixture allowed input before a current frame".into());
                return false;
            }
            if flag == "--smoke-target-close" {
                eprintln!(
                    "UI-R11 close: prepared=true input_blocked=true renderer_published=false"
                );
                return true;
            }
            if flag == "--smoke-target-cancel" {
                if let Some(gpu) = &mut self.gpu {
                    gpu.cancel_update();
                }
                eprintln!("UI-R11 cancel: prepared=true input_blocked=true");
            }
            if flag == "--smoke-target-delay" {
                if let Some(window) = &self.window {
                    let _ = window.request_inner_size(LogicalSize::new(940.0, 660.0));
                }
                self.mode = match self.mode {
                    AcceptanceMode::Gallery2d => AcceptanceMode::Viewport3d,
                    AcceptanceMode::Viewport3d => AcceptanceMode::Gallery2d,
                };
                eprintln!(
                    "UI-R11 coalesced: prepared=true input_blocked=true new_extent_and_mode=true"
                );
            }
        }
        if flag == "--smoke-target-failure" || flag == "--smoke-target-cancel" {
            self.retry_gpu();
        }
        let ready = self
            .window
            .as_ref()
            .is_some_and(|window| !self.display_blocked(window.id()))
            && self.rendered_frames > self.target_probe.at_frame;
        if ready {
            let Some((snapshot, old)) = &self.target_probe.snapshot else {
                return false;
            };
            let after = self.recovery_snapshot_parts();
            let components: [bool; 9] =
                std::array::from_fn(|index| snapshot[index] == after[index]);
            let mode_changed = flag == "--smoke-target-delay";
            let preserved =
                components[..8].iter().all(|same| *same) && (components[8] || mode_changed);
            eprintln!(
                "UI-R11 CPU components: workspace_camera_text_selection_area_selection_search_selection_focus={components:?}"
            );
            eprintln!(
                "UI-R11 focus: preserved={} explicit_mode_change={mode_changed}",
                components[8]
            );
            let context = self.gpu.as_ref().and_then(GpuState::gpu_context);
            let same = context.is_some_and(|context| Weak::ptr_eq(old, &Arc::downgrade(&context)));
            let replace = matches!(
                flag.as_str(),
                "--smoke-target-device-loss"
                    | "--smoke-target-timeout"
                    | "--smoke-target-failure"
                    | "--smoke-target-cancel"
            );
            let released = old.upgrade().is_none();
            eprintln!(
                "UI-R11 accepted_frame: cpu_state_preserved={preserved} context_replaced={} old_context_released={released} input_resumed=true",
                !same
            );
            if !preserved || same == replace || (replace && !released) {
                self.fatal_error =
                    Some("target transition changed CPU state or context scope".into());
            }
            self.target_probe.complete = true;
        }
        false
    }
}
