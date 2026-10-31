//! GPU generations change independently of editor/window state. ADR-0329.
use super::*;
use std::{collections::HashSet, sync::Weak};
use vesta_gpui_vulkan::{
    GpuContext, RecoveryJob, RecoveryPoll, RecoveryScope, RecoverySeed, RecoveryTarget,
    RendererError, RendererFault,
};

#[derive(Default)]
pub(super) struct State {
    pending: HashMap<WindowId, RendererError>,
    blocked: HashSet<WindowId>,
    job: Option<RecoveryJob>,
    seed: Option<RecoverySeed>,
    rejected: Vec<RecoveryTarget>,
    paused: Option<RendererError>,
    scope: Option<RecoveryScope>,
    titles: HashMap<WindowId, String>,
    snapshot: Option<(String, Weak<GpuContext>)>,
    completed: u32,
    fixture_started: bool,
    cancel_sent: bool,
    pause_observed: bool,
    retry_rejected_busy: bool,
    retries: u32,
    closed_fixture: bool,
    closed_results: u32,
}
impl State {
    pub fn blocked(&self, id: WindowId) -> bool {
        self.blocked.contains(&id) || self.pending.contains_key(&id)
    }
    pub fn presented(&mut self, id: WindowId) {
        if self.blocked.remove(&id) {
            eprintln!("UI-R05 recovered_frame window={id:?} input_resumed=true");
        }
    }
    pub fn ready(&self) -> bool {
        self.blocked.is_empty() && self.pending.is_empty() && self.paused.is_none()
    }
    pub fn report(&self) -> Result<(), String> {
        if render_backend::recovery_fault().is_some() {
            eprintln!(
                "UI-R05 recovery: generations={} input_resumed={}",
                self.completed,
                self.ready()
            );
            if self.completed != 1 || !self.ready() {
                return Err("recovery smoke did not accept all restored window frames".into());
            }
            if render_backend::recovery_retry_enabled() {
                eprintln!(
                    "UI-R05 retry: paused={} busy_retry_rejected={} explicit_retries={}",
                    self.pause_observed, self.retry_rejected_busy, self.retries
                );
                if !self.pause_observed
                    || self.retries != 1
                    || (!std::env::args().any(|a| a == "--smoke-recovery-create-failure")
                        && !self.retry_rejected_busy)
                {
                    return Err(
                        "recovery retry fixture did not preserve its pause/retirement gate".into(),
                    );
                }
            }
            if render_backend::recovery_close_enabled()
                && (!self.closed_fixture || self.closed_results != 1)
            {
                return Err("closed recovery window was not discarded exactly once".into());
            }
        }
        Ok(())
    }
}
impl App {
    pub(super) fn recovery_snapshot(&self) -> String {
        self.recovery_snapshot_parts().join("|")
    }
    pub(super) fn recovery_snapshot_parts(&self) -> [String; 9] {
        [
            format!("{:?}", self.demo.workspace),
            format!("{:?}", self.camera),
            self.text_state.text().into(),
            format!("{:?}", self.text_state.snapshot()),
            self.area_state.text().into(),
            format!("{:?}", self.area_state.snapshot()),
            self.search_state.text().into(),
            format!("{:?}", self.search_state.snapshot()),
            format!("{:?}", self.input_ownership.focus()),
        ]
    }
    pub(super) fn gpu_fault(&mut self, id: WindowId, error: RendererError) {
        if matches!(
            error.fault,
            RendererFault::SurfaceLost
                | RendererFault::DeviceLost
                | RendererFault::CompletionTimeout
        ) {
            eprintln!("{error}");
            let ids = if error.fault == RendererFault::SurfaceLost {
                HashSet::from([id])
            } else {
                self.recovery_windows()
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect()
            };
            self.cancel_unavailable_interactions(&ids);
            self.mark_recovery_windows(&ids);
            self.gpu_recovery.pending.insert(id, error);
            self.request_all_redraw();
            // Transfer a known failed generation before another close event
            // could drop the renderer on the event thread.
            self.drive_gpu_recovery();
        } else {
            self.fatal_error = Some(error.to_string());
        }
    }
    pub(super) fn gpu_update_fault(&mut self, id: WindowId, error: RendererError) {
        if matches!(
            error.fault,
            RendererFault::SurfaceLost
                | RendererFault::DeviceLost
                | RendererFault::CompletionTimeout
        ) {
            self.gpu_fault(id, error);
            return;
        }
        let context = if self.window.as_ref().is_some_and(|w| w.id() == id) {
            self.gpu.as_ref().and_then(GpuState::gpu_context)
        } else {
            self.native_windows
                .get(&id)
                .and_then(|host| host.surface.gpu_context())
        };
        self.gpu_recovery.seed = context.map(|context| context.recovery_seed());
        let ids = HashSet::from([id]);
        self.cancel_unavailable_interactions(&ids);
        self.mark_recovery_windows(&ids);
        if let Some((_, window)) = self
            .recovery_windows()
            .into_iter()
            .find(|(window, _)| *window == id)
        {
            window.set_title("Vesta — display paused; F5 retries after retirement");
        }
        eprintln!("{error}");
        self.gpu_recovery.paused = Some(error);
    }
    fn recovery_windows(&self) -> Vec<(WindowId, Arc<Window>)> {
        self.window
            .iter()
            .map(|window| (window.id(), window.clone()))
            .chain(
                self.native_windows
                    .iter()
                    .map(|(id, host)| (*id, host.window.clone())),
            )
            .collect()
    }
    fn take_recovery_targets(&mut self, ids: &HashSet<WindowId>) -> Vec<RecoveryTarget> {
        let mut targets = Vec::new();
        if let Some(window) = &self.window
            && ids.contains(&window.id())
        {
            let target = self
                .gpu
                .as_mut()
                .and_then(GpuState::take_target)
                .unwrap_or_else(|| {
                    RecoveryTarget::without_renderer(
                        window.clone(),
                        self.mode == AcceptanceMode::Viewport3d,
                    )
                });
            targets.push(target);
        }
        for (id, host) in &mut self.native_windows {
            if ids.contains(id) {
                targets.push(host.surface.take_target().unwrap_or_else(|| {
                    RecoveryTarget::without_renderer(host.window.clone(), false)
                }));
            }
        }
        targets
    }
    fn mark_recovery_windows(&mut self, ids: &HashSet<WindowId>) {
        for (id, window) in self.recovery_windows() {
            if ids.contains(&id) {
                self.gpu_recovery.blocked.insert(id);
                self.gpu_recovery
                    .titles
                    .entry(id)
                    .or_insert_with(|| window.title());
                window.set_title("Vesta — restoring display");
            }
        }
    }
    pub(super) fn retry_gpu(&mut self) {
        if self.gpu_recovery.paused.is_none()
            || self
                .gpu
                .as_ref()
                .is_some_and(|gpu| !gpu.update_worker_finished())
            || self
                .native_windows
                .values()
                .any(|host| !host.surface.update_worker_finished())
            || self
                .gpu_recovery
                .job
                .as_ref()
                .is_some_and(|job| !job.worker_finished())
        {
            return;
        }
        let Some(seed) = self.gpu_recovery.seed.clone() else {
            return;
        };
        let ids = self
            .recovery_windows()
            .into_iter()
            .map(|(id, _)| id)
            .collect::<HashSet<_>>();
        self.mark_recovery_windows(&ids);
        let mut targets = self.take_recovery_targets(&ids);
        // Rejection owns its renderers. Replace corresponding empty targets.
        for target in self.gpu_recovery.rejected.drain(..) {
            targets.retain(|other| other.window_id() != target.window_id());
            targets.push(target);
        }
        self.gpu_recovery.job = None;
        self.gpu_recovery.scope = Some(RecoveryScope::Device);
        match RecoveryJob::restart(seed, targets) {
            Ok(job) => {
                self.gpu_recovery.job = Some(job);
                self.gpu_recovery.paused = None;
                self.gpu_recovery.retries += 1;
                eprintln!("UI-R05 explicit_retry accepted=true previous_worker_finished=true");
            }
            Err(rejected) => {
                self.gpu_recovery.rejected = rejected.targets;
                self.gpu_recovery.paused = Some(rejected.error);
            }
        }
    }
    pub(super) fn drive_gpu_recovery(&mut self) -> Option<Instant> {
        let live = self
            .recovery_windows()
            .into_iter()
            .map(|(id, _)| id)
            .collect::<HashSet<_>>();
        self.gpu_recovery.blocked.retain(|id| live.contains(id));
        self.gpu_recovery.pending.retain(|id, _| live.contains(id));
        if self.gpu_recovery.paused.is_some()
            && render_backend::recovery_retry_enabled()
            && self
                .gpu_recovery
                .job
                .as_ref()
                .is_some_and(RecoveryJob::worker_finished)
        {
            self.retry_gpu();
        }
        if let Some(job) = &mut self.gpu_recovery.job {
            match job.poll() {
                RecoveryPoll::Pending { deadline } => {
                    if render_backend::recovery_cancel_enabled() && !self.gpu_recovery.cancel_sent {
                        job.cancel();
                        self.gpu_recovery.cancel_sent = true;
                    }
                    if render_backend::recovery_close_enabled()
                        && !self.gpu_recovery.closed_fixture
                        && let Some(id) = self.native_windows.keys().next().copied()
                    {
                        if let Some(host) = self.native_windows.get(&id) {
                            host.window.set_visible(false);
                        }
                        self.restore_native_window(id);
                        self.gpu_recovery.closed_fixture = true;
                        if let Some((_, old)) = self.gpu_recovery.snapshot.take() {
                            self.gpu_recovery.snapshot = Some((self.recovery_snapshot(), old));
                        }
                        eprintln!(
                            "UI-R05 closed_during_recovery window={id:?} logical_window_removed={}",
                            !self.native_windows.contains_key(&id)
                        );
                    }
                    return Some(deadline);
                }
                RecoveryPoll::Ready(renderers) => {
                    if render_backend::recovery_fault().is_some() {
                        let Some((snapshot, old)) = &self.gpu_recovery.snapshot else {
                            self.fatal_error = Some("missing recovery smoke snapshot".into());
                            return None;
                        };
                        let stable = *snapshot == self.recovery_snapshot();
                        let shared = renderers.first().is_some_and(|first| {
                            renderers.iter().all(|renderer| {
                                Arc::ptr_eq(&first.gpu_context(), &renderer.gpu_context())
                            })
                        });
                        let same = renderers.first().is_some_and(|renderer| {
                            Weak::ptr_eq(old, &Arc::downgrade(&renderer.gpu_context()))
                        });
                        let device = self.gpu_recovery.scope == Some(RecoveryScope::Device);
                        eprintln!(
                            "UI-R05 identity: cpu_state_preserved={stable} shared_context={shared} context_replaced={} old_context_released={}",
                            !same,
                            old.upgrade().is_none()
                        );
                        if !stable || !shared || same == device {
                            self.fatal_error = Some(
                                "recovery changed CPU state or used the wrong GPU context scope"
                                    .into(),
                            );
                        }
                    }
                    for renderer in renderers {
                        let id = renderer.window_id();
                        if self.window.as_ref().is_some_and(|window| window.id() == id) {
                            if let Some(gpu) = &mut self.gpu {
                                gpu.restore(renderer);
                            }
                        } else if let Some(host) = self.native_windows.get_mut(&id) {
                            host.surface.restore(renderer);
                        } else {
                            self.gpu_recovery.closed_results += 1;
                            eprintln!("UI-R05 closed_result window={id:?} discarded=true");
                        }
                        // Closed-window results have never submitted a frame.
                    }
                    for (id, window) in self.recovery_windows() {
                        if let Some(title) = self.gpu_recovery.titles.remove(&id) {
                            window.set_title(&title);
                        }
                    }
                    self.gpu_recovery.completed += 1;
                    self.gpu_recovery.paused = None;
                    self.request_all_redraw();
                }
                RecoveryPoll::Failed(error) => {
                    eprintln!("{error}");
                    if error.fault == RendererFault::DeviceLost
                        && self.gpu_recovery.scope == Some(RecoveryScope::Surface)
                        && let Some(window) = &self.window
                    {
                        self.gpu_recovery.pending.insert(window.id(), error.clone());
                    }
                    self.gpu_recovery.paused = Some(error);
                    self.gpu_recovery.pause_observed = true;
                    for (_, window) in self.recovery_windows() {
                        window.set_title("Vesta — display paused; F5 retries after retirement");
                    }
                    if self.smoke && !render_backend::recovery_retry_enabled() {
                        self.fatal_error = Some("GPU recovery smoke failed or timed out".into());
                    }
                    if render_backend::recovery_retry_enabled()
                        && self
                            .gpu_recovery
                            .job
                            .as_ref()
                            .is_some_and(|job| !job.worker_finished())
                    {
                        self.retry_gpu();
                        self.gpu_recovery.retry_rejected_busy = self.gpu_recovery.retries == 0;
                        eprintln!(
                            "UI-R05 paused=true worker_finished=false retry_rejected_busy={}",
                            self.gpu_recovery.retry_rejected_busy
                        );
                    }
                }
                RecoveryPoll::Finished => (),
            }
            if self
                .gpu_recovery
                .job
                .as_ref()
                .is_some_and(|job| !job.worker_finished())
            {
                return Some(Instant::now() + Duration::from_millis(16));
            }
            if self.gpu_recovery.paused.is_none() || !self.gpu_recovery.pending.is_empty() {
                self.gpu_recovery.job = None;
            }
        }
        if !self.gpu_recovery.pending.is_empty() && self.gpu_recovery.job.is_none() {
            let context = self
                .gpu
                .as_ref()
                .and_then(GpuState::gpu_context)
                .or_else(|| {
                    self.native_windows
                        .values()
                        .find_map(|host| host.surface.gpu_context())
                });
            let scope = if context
                .as_ref()
                .is_some_and(|context| !context.can_create_renderer())
            {
                return Some(Instant::now() + Duration::from_millis(16));
            } else if context
                .as_ref()
                .is_some_and(|context| context.failure().is_some())
                || self.gpu_recovery.pending.values().any(|error| {
                    matches!(
                        error.fault,
                        RendererFault::DeviceLost | RendererFault::CompletionTimeout
                    )
                })
            {
                RecoveryScope::Device
            } else {
                RecoveryScope::Surface
            };
            let ids = if scope == RecoveryScope::Device {
                self.recovery_windows()
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect()
            } else {
                self.gpu_recovery.pending.keys().copied().collect()
            };
            if render_backend::recovery_fault().is_some()
                && let Some(context) = &context
            {
                self.gpu_recovery.snapshot =
                    Some((self.recovery_snapshot(), Arc::downgrade(context)));
            }
            self.gpu_recovery.pending.clear();
            if let Some(context) = &context {
                self.gpu_recovery.seed = Some(context.recovery_seed());
            }
            self.gpu_recovery.scope = Some(scope);
            self.mark_recovery_windows(&ids);
            let targets = self.take_recovery_targets(&ids);
            eprintln!(
                "UI-R05 recovery_started scope={scope:?} windows={}",
                targets.len()
            );
            let start = if let Some(context) = context {
                #[cfg(feature = "gpu-fault-injection")]
                if (render_backend::recovery_retry_enabled()
                    || render_backend::recovery_close_enabled())
                    && !self.gpu_recovery.fixture_started
                {
                    self.gpu_recovery.fixture_started = true;
                    let creation = std::env::args().any(|a| a == "--smoke-recovery-create-failure");
                    RecoveryJob::start_fixture(
                        context,
                        targets,
                        scope,
                        if creation {
                            vesta_gpui_vulkan::RecoveryFixture::CreationFailure
                        } else {
                            vesta_gpui_vulkan::RecoveryFixture::DelayedRetirement
                        },
                        !creation
                            && !render_backend::recovery_cancel_enabled()
                            && !render_backend::recovery_close_enabled(),
                    )
                } else {
                    RecoveryJob::start(context, targets, scope)
                }
                #[cfg(not(feature = "gpu-fault-injection"))]
                {
                    self.gpu_recovery.fixture_started = true;
                    RecoveryJob::start(context, targets, scope)
                }
            } else if let Some(seed) = self.gpu_recovery.seed.clone() {
                RecoveryJob::restart(seed, targets)
            } else {
                return None;
            };
            match start {
                Ok(job) => {
                    self.gpu_recovery.job = Some(job);
                    self.gpu_recovery.paused = None;
                }
                Err(rejected) => {
                    eprintln!("{}", rejected.error);
                    self.gpu_recovery.rejected = rejected.targets;
                    self.gpu_recovery.paused = Some(rejected.error);
                    if self.smoke {
                        self.fatal_error =
                            Some("recovery could not prepare native surfaces".into());
                    }
                }
            }
            return Some(Instant::now() + Duration::from_millis(16));
        }
        None
    }
    pub(super) fn display_blocked(&self, id: WindowId) -> bool {
        self.gpu_recovery.blocked(id)
            || if self.window.as_ref().is_some_and(|window| window.id() == id) {
                self.gpu.as_ref().is_none_or(GpuState::display_blocked)
            } else {
                self.native_windows
                    .get(&id)
                    .is_none_or(|host| host.surface.display_blocked())
            }
    }
}
