//! One recovery job per application; GPU retirement never joins the UI. ADR-0329.
use crate::{GpuContext, RendererError, RendererFault, WindowRenderer};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use vulkano::{device::physical::PhysicalDevice, instance::Instance, swapchain::Surface};
use winit::window::{Window, WindowId};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryScope {
    Surface,
    Device,
}
#[cfg(feature = "fault-injection")]
#[derive(Clone, Copy)]
pub enum RecoveryFixture {
    CreationFailure,
    DelayedRetirement,
    DelayedCreation,
    DelayedReconfiguration,
    ReconfigurationFailure,
    ReconfigurationOutOfDate,
    ReconfigurationDeviceLoss,
    ReconfigurationSurfaceLoss,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum JobKind {
    Recovery,
    Creation,
    Reconfiguration,
}
struct FactoryIntent {
    kind: JobKind,
    scope: RecoveryScope,
}
impl JobKind {
    fn stage(self) -> &'static str {
        match self {
            Self::Creation => "creation",
            Self::Reconfiguration => "reconfiguration",
            Self::Recovery => "recovery",
        }
    }
    fn timeout(self) -> RendererError {
        if self == Self::Recovery {
            return timeout();
        }
        RendererError {
            code: if self == Self::Creation {
                "native_vulkan_creation_timeout"
            } else {
                "native_vulkan_reconfiguration_timeout"
            },
            stage: self.stage(),
            fault: RendererFault::CompletionTimeout,
            message:
                "GPU factory exceeded its host observation budget; GPU owners remain on the worker"
                    .into(),
            element: None,
        }
    }
    fn cancelled(self) -> RendererError {
        if self == Self::Recovery {
            return cancelled();
        }
        RendererError {
            code: if self == Self::Creation {
                "native_vulkan_creation_cancelled"
            } else {
                "native_vulkan_reconfiguration_cancelled"
            },
            stage: self.stage(),
            fault: RendererFault::Other,
            message: "renderer creation cancelled; late resources are retired by the worker".into(),
            element: None,
        }
    }
}
/// No lost logical Device, Queue, submitted future or author state.
#[derive(Clone)]
pub struct RecoverySeed {
    pub(crate) instance: Arc<Instance>,
    pub(crate) physical: Arc<PhysicalDevice>,
    pub(crate) family: u32,
    pub(crate) worker: Arc<RecoveryWorker>,
}
pub struct RecoveryTarget {
    window: Arc<Window>,
    #[cfg(feature = "viewport")]
    scene: bool,
    renderer: Option<WindowRenderer>,
    surface: Option<Arc<Surface>>,
    configuration: Option<crate::TargetConfiguration>,
}
impl RecoveryTarget {
    pub(crate) fn prepared(window: Arc<Window>, surface: Arc<Surface>, scene: bool) -> Self {
        let mut target = Self::without_renderer(window, scene);
        target.surface = Some(surface);
        target
    }
    pub fn from_renderer(renderer: WindowRenderer) -> Self {
        Self {
            window: renderer.window.clone(),
            #[cfg(feature = "viewport")]
            scene: renderer.has_scene(),
            renderer: Some(renderer),
            surface: None,
            configuration: None,
        }
    }
    pub fn without_renderer(window: Arc<Window>, scene: bool) -> Self {
        #[cfg(not(feature = "viewport"))]
        let _ = scene;
        Self {
            window,
            #[cfg(feature = "viewport")]
            scene,
            renderer: None,
            surface: None,
            configuration: None,
        }
    }
    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }
    pub(crate) fn into_renderer(mut self) -> Option<WindowRenderer> {
        self.renderer.take()
    }
}
pub enum RecoveryPoll {
    Pending { deadline: Instant },
    Ready(Vec<WindowRenderer>),
    Failed(RendererError),
    Finished,
}
/// Rejection retains every GPU owner for the caller.
pub struct RecoveryRejected {
    pub error: RendererError,
    pub targets: Vec<RecoveryTarget>,
}
type Outcome = Result<Vec<WindowRenderer>, RendererError>;
#[derive(Default)]
struct ResultSlot {
    outcome: Mutex<Option<Outcome>>,
    changed: Condvar,
    cancel: AtomicBool,
    done: AtomicBool,
    #[cfg(feature = "fault-injection")]
    created: AtomicBool,
}
impl ResultSlot {
    fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
        self.changed.notify_one();
    }
    // Worker remains an owner until the host takes the result or cancellation
    // drains it here. Ticket Drop never disposes GPU objects on the event thread.
    fn deliver(&self, outcome: Outcome, deadline: Instant) {
        let mut slot = self.outcome.lock().unwrap_or_else(|e| e.into_inner());
        *slot = Some(outcome);
        while slot.is_some() && !self.cancel.load(Ordering::Acquire) && Instant::now() < deadline {
            slot = self
                .changed
                .wait_timeout(slot, deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        let abandoned = slot.take();
        drop(slot);
        if let Some(Ok(renderers)) = abandoned {
            #[cfg(feature = "fault-injection")]
            eprintln!(
                "UI-R10 abandoned result: worker={} renderers={}",
                thread::current().name() == Some("vesta-gpu-recovery"),
                renderers.len()
            );
            discard(renderers);
        }
        self.done.store(true, Ordering::Release);
    }
}
struct Request {
    kind: JobKind,
    scope: RecoveryScope,
    context: Option<Arc<GpuContext>>,
    seed: RecoverySeed,
    targets: Vec<RecoveryTarget>,
    result: Arc<ResultSlot>,
    deadline: Instant,
    #[cfg(feature = "fault-injection")]
    fixture: Option<RecoveryFixture>,
}
pub(crate) struct RecoveryWorker {
    sender: SyncSender<Request>,
    wake: SyncSender<()>,
    retirement: Arc<crate::retirement::Registry<crate::retirement::Owner>>,
    busy: Arc<AtomicBool>,
    _thread: JoinHandle<()>,
}
impl RecoveryWorker {
    pub fn on_worker(&self) -> bool {
        thread::current().id() == self._thread.thread().id()
    }
    pub fn register_retirement(&self) -> Arc<crate::retirement::Slot<crate::retirement::Owner>> {
        self.retirement.register()
    }
    pub fn retire(
        &self,
        slot: &crate::retirement::Slot<crate::retirement::Owner>,
        owner: crate::retirement::Owner,
    ) {
        self.retirement.handoff(slot, owner);
        let _ = self.wake.try_send(());
    }
    pub fn record_retirement_error(&self, error: RendererError) {
        self.retirement.record_error(error);
    }
    pub fn retirement_monitor(&self) -> crate::RetirementMonitor {
        crate::RetirementMonitor {
            status: self.retirement.status.clone(),
            busy: self.busy.clone(),
        }
    }
    pub fn new() -> Result<Arc<Self>, RendererError> {
        let (sender, receiver) = mpsc::sync_channel::<Request>(1);
        let (wake, awakened) = mpsc::sync_channel::<()>(1);
        let retirement = Arc::new(crate::retirement::Registry::default());
        let retiring = retirement.clone();
        let busy = Arc::new(AtomicBool::new(false));
        let working = busy.clone();
        let handle =
            thread::Builder::new()
                .name("vesta-gpu-recovery".into())
                .spawn(move || {
                    loop {
                        retiring.drain_gpu();
                        let request = match receiver.try_recv() {
                            Ok(request) => request,
                            Err(mpsc::TryRecvError::Disconnected) => {
                                // Slot payloads may outlive every sender.
                                retiring.drain_gpu();
                                break;
                            }
                            Err(mpsc::TryRecvError::Empty) => {
                                let _ = awakened.recv();
                                continue;
                            }
                        };
                        let Request {
                            kind,
                            scope,
                            context,
                            seed,
                            targets,
                            result,
                            deadline,
                            #[cfg(feature = "fault-injection")]
                            fixture,
                        } = request;
                        #[cfg(feature = "fault-injection")]
                        if matches!(fixture, Some(RecoveryFixture::DelayedRetirement)) {
                            thread::sleep(Duration::from_millis(150));
                        }
                        let outcome = catch_unwind(AssertUnwindSafe(|| {
                    recover(FactoryIntent { kind, scope }, context, &seed, targets, &result.cancel, deadline,
                        #[cfg(feature = "fault-injection")] fixture)
                    }))
                    .unwrap_or_else(|_| {
                        Err(RendererError::at(
                            kind.stage(),
                            "GPU dependency panicked during a factory job; DeviceLost is not inferred",
                        ))
                    });
                        #[cfg(feature = "fault-injection")]
                        if kind == JobKind::Creation {
                            result.created.store(outcome.is_ok(), Ordering::Release);
                            eprintln!("UI-R10 factory: worker={} created={} targets={}", seed.worker.on_worker(), outcome.is_ok(), outcome.as_ref().map_or(0, Vec::len));
                            if matches!(fixture, Some(RecoveryFixture::DelayedCreation)) {
                                thread::sleep(Duration::from_millis(200));
                            }
                        }
                        #[cfg(feature = "fault-injection")]
                        if kind == JobKind::Reconfiguration {
                            result.created.store(outcome.is_ok(), Ordering::Release);
                            eprintln!("UI-R11 factory: worker={} prepared={}", seed.worker.on_worker(), outcome.is_ok());
                            if matches!(fixture, Some(RecoveryFixture::DelayedReconfiguration)) {
                                thread::sleep(Duration::from_millis(200));
                            }
                        }
                        result.deliver(outcome, deadline);
                        #[cfg(feature = "fault-injection")]
                        if kind == JobKind::Creation {
                            eprintln!("UI-R10 factory delivery: worker={} cancelled={} result_slot_empty=true", seed.worker.on_worker(), result.cancel.load(Ordering::Acquire));
                        }
                        #[cfg(feature = "fault-injection")]
                        if kind == JobKind::Reconfiguration {
                            eprintln!("UI-R11 delivery: worker={} cancelled={} result_slot_empty=true", seed.worker.on_worker(), result.cancel.load(Ordering::Acquire));
                        }
                        working.store(false, Ordering::Release);
                    }
                })
                .map_err(|e| RendererError::caused_by("recovery_worker", e))?;
        Ok(Arc::new(Self {
            sender,
            wake,
            retirement,
            busy,
            _thread: handle,
        }))
    }
}
pub struct RecoveryJob {
    kind: JobKind,
    result: Arc<ResultSlot>,
    seed: RecoverySeed,
    deadline: Instant,
    reported_timeout: bool,
    terminal: bool,
}
impl RecoveryJob {
    pub fn start(
        context: Arc<GpuContext>,
        targets: Vec<RecoveryTarget>,
        scope: RecoveryScope,
    ) -> Result<Self, RecoveryRejected> {
        Self::start_with_seed(
            JobKind::Recovery,
            context.recovery_seed(),
            Some(context),
            targets,
            scope,
            Duration::from_secs(10),
            #[cfg(feature = "fault-injection")]
            None,
        )
    }
    pub fn restart(
        seed: RecoverySeed,
        targets: Vec<RecoveryTarget>,
    ) -> Result<Self, RecoveryRejected> {
        Self::start_with_seed(
            JobKind::Recovery,
            seed,
            None,
            targets,
            RecoveryScope::Device,
            Duration::from_secs(10),
            #[cfg(feature = "fault-injection")]
            None,
        )
    }
    #[cfg(feature = "fault-injection")]
    pub fn start_fixture(
        context: Arc<GpuContext>,
        targets: Vec<RecoveryTarget>,
        scope: RecoveryScope,
        fixture: RecoveryFixture,
        short_budget: bool,
    ) -> Result<Self, RecoveryRejected> {
        Self::start_with_seed(
            JobKind::Recovery,
            context.recovery_seed(),
            Some(context),
            targets,
            scope,
            if short_budget {
                Duration::from_millis(25)
            } else {
                Duration::from_secs(10)
            },
            Some(fixture),
        )
    }
    pub(crate) fn create(
        seed: RecoverySeed,
        context: Option<Arc<GpuContext>>,
        target: RecoveryTarget,
        budget: Duration,
        #[cfg(feature = "fault-injection")] fixture: Option<RecoveryFixture>,
    ) -> Result<Self, RecoveryRejected> {
        let scope = if context.is_some() {
            RecoveryScope::Surface
        } else {
            RecoveryScope::Device
        };
        Self::start_with_seed(
            JobKind::Creation,
            seed,
            context,
            vec![target],
            scope,
            budget,
            #[cfg(feature = "fault-injection")]
            fixture,
        )
    }
    pub(crate) fn reconfigure(
        renderer: WindowRenderer,
        configuration: crate::TargetConfiguration,
        #[cfg(feature = "fault-injection")] fixture: Option<RecoveryFixture>,
        #[cfg(feature = "fault-injection")] short_budget: bool,
    ) -> Result<Self, RecoveryRejected> {
        let seed = renderer.gpu.recovery_seed();
        let surface = renderer.swapchain.surface().clone();
        let mut target = RecoveryTarget::from_renderer(renderer);
        target.surface = Some(surface);
        target.configuration = Some(configuration);
        let budget = Duration::from_secs(10);
        #[cfg(feature = "fault-injection")]
        let budget = if short_budget {
            Duration::from_millis(25)
        } else {
            budget
        };
        Self::start_with_seed(
            JobKind::Reconfiguration,
            seed,
            None,
            vec![target],
            RecoveryScope::Surface,
            budget,
            #[cfg(feature = "fault-injection")]
            fixture,
        )
    }
    fn start_with_seed(
        kind: JobKind,
        seed: RecoverySeed,
        context: Option<Arc<GpuContext>>,
        targets: Vec<RecoveryTarget>,
        scope: RecoveryScope,
        budget: Duration,
        #[cfg(feature = "fault-injection")] fixture: Option<RecoveryFixture>,
    ) -> Result<Self, RecoveryRejected> {
        if seed
            .worker
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(RecoveryRejected {
                error: RendererError {
                    code: "native_vulkan_worker_busy",
                    stage: kind.stage(),
                    fault: RendererFault::Other,
                    message: "a GPU factory generation is still owned by the worker".into(),
                    element: None,
                },
                targets,
            });
        }
        // winit's Win32 window handle is available only on its event thread.
        // Create the device-independent Surface there, before handing GPU
        // retirement/Device/pipeline work to the worker. No queue/fence wait.
        let mut targets = targets;
        for target in &mut targets {
            if target.surface.is_some() {
                continue;
            }
            match Surface::from_window(seed.instance.clone(), target.window.clone()) {
                Ok(surface) => target.surface = Some(surface),
                Err(error) => {
                    seed.worker.busy.store(false, Ordering::Release);
                    return Err(RecoveryRejected {
                        error: RendererError::caused_by(kind.stage(), error),
                        targets,
                    });
                }
            }
        }
        let result = Arc::new(ResultSlot::default());
        let deadline = Instant::now() + budget;
        let request = Request {
            kind,
            scope,
            context,
            seed: seed.clone(),
            targets,
            result: result.clone(),
            deadline,
            #[cfg(feature = "fault-injection")]
            fixture,
        };
        if let Err(error) = seed.worker.sender.try_send(request) {
            seed.worker.busy.store(false, Ordering::Release);
            let request = match error {
                mpsc::TrySendError::Full(request) | mpsc::TrySendError::Disconnected(request) => {
                    request
                }
            };
            return Err(RecoveryRejected {
                error: RendererError::at(kind.stage(), "GPU worker unavailable"),
                targets: request.targets,
            });
        }
        let _ = seed.worker.wake.try_send(());
        Ok(Self {
            kind,
            result,
            seed,
            deadline,
            reported_timeout: false,
            terminal: false,
        })
    }
    pub fn seed(&self) -> RecoverySeed {
        self.seed.clone()
    }
    pub fn worker_finished(&self) -> bool {
        !self.seed.worker.retirement_monitor().pending() && self.result.done.load(Ordering::Acquire)
    }
    #[cfg(feature = "fault-injection")]
    pub(crate) fn creation_prepared(&self) -> bool {
        self.result.created.load(Ordering::Acquire)
    }
    pub fn cancel(&mut self) {
        self.result.cancel();
    }
    pub fn poll(&mut self) -> RecoveryPoll {
        self.poll_at(Instant::now())
    }
    fn poll_at(&mut self, now: Instant) -> RecoveryPoll {
        if self.terminal {
            return RecoveryPoll::Finished;
        }
        if now >= self.deadline || self.result.cancel.load(Ordering::Acquire) {
            self.result.cancel();
            if !self.reported_timeout {
                self.reported_timeout = true;
                return RecoveryPoll::Failed(if now >= self.deadline {
                    self.kind.timeout()
                } else {
                    self.kind.cancelled()
                });
            }
            return if self.result.done.load(Ordering::Acquire) {
                self.terminal = true;
                RecoveryPoll::Finished
            } else {
                RecoveryPoll::Pending {
                    deadline: now + Duration::from_millis(16),
                }
            };
        }
        let outcome = self
            .result
            .outcome
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(outcome) = outcome {
            self.result.changed.notify_one();
            self.terminal = true;
            match outcome {
                Ok(renderers) => RecoveryPoll::Ready(renderers),
                Err(error) => RecoveryPoll::Failed(error),
            }
        } else if self.result.done.load(Ordering::Acquire) {
            self.terminal = true;
            RecoveryPoll::Failed(self.kind.timeout())
        } else {
            RecoveryPoll::Pending {
                deadline: now + Duration::from_millis(16),
            }
        }
    }
}
impl Drop for RecoveryJob {
    fn drop(&mut self) {
        self.result.cancel();
    }
}
fn timeout() -> RendererError {
    RendererError {
        code: "native_vulkan_recovery_timeout",
        stage: "recovery",
        fault: RendererFault::CompletionTimeout,
        message:
            "display recovery exceeded its host observation budget; author/input state retained"
                .into(),
        element: None,
    }
}
fn cancelled() -> RendererError {
    RendererError { code: "native_vulkan_recovery_cancelled", stage: "recovery", fault: RendererFault::Other, message: "display recovery cancelled; retired resources remain owned by the worker until released".into(), element: None }
}
fn recover(
    intent: FactoryIntent,
    context: Option<Arc<GpuContext>>,
    seed: &RecoverySeed,
    mut targets: Vec<RecoveryTarget>,
    cancel: &AtomicBool,
    deadline: Instant,
    #[cfg(feature = "fault-injection")] fixture: Option<RecoveryFixture>,
) -> Outcome {
    let FactoryIntent { kind, scope } = intent;
    if kind == JobKind::Reconfiguration {
        return reconfigure_targets(
            targets,
            cancel,
            deadline,
            #[cfg(feature = "fault-injection")]
            fixture,
        );
    }
    let mut retirement_error = None;
    for target in &mut targets {
        if let Some(renderer) = target.renderer.take()
            && let Err(error) = retire(renderer)
        {
            retirement_error.get_or_insert(error);
        }
    }
    if let Some(error) = retirement_error {
        return Err(error);
    }
    let context = if scope == RecoveryScope::Surface {
        context
    } else {
        drop(context);
        None
    };
    let mut last_error = RendererError::at("recovery", "no recovery attempt completed");
    let attempts = if kind == JobKind::Creation { 1 } else { 3 };
    for attempt in 0..attempts {
        if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
            return Err(if Instant::now() >= deadline {
                kind.timeout()
            } else {
                kind.cancelled()
            });
        }
        let gpu = match scope {
            RecoveryScope::Surface => context
                .clone()
                .ok_or_else(|| RendererError::at("recovery", "surface recovery lost its context")),
            RecoveryScope::Device => {
                if kind == JobKind::Creation {
                    GpuContext::create(seed, "device")
                } else {
                    GpuContext::replacement(seed)
                }
            }
        };
        let outcome = gpu.and_then(|gpu| {
            #[cfg(feature = "fault-injection")]
            if matches!(fixture, Some(RecoveryFixture::CreationFailure)) {
                if kind == JobKind::Recovery {
                    eprintln!(
                        "UI-R05 fixture_create_attempt={} rejected=true",
                        attempt + 1
                    );
                }
                return Err(RendererError::at(
                    if kind == JobKind::Creation {
                        "creation_fixture"
                    } else {
                        "recovery_creation_fixture"
                    },
                    "intentional rejection after GPU context preparation",
                ));
            }
            let mut restored = Vec::with_capacity(targets.len());
            for target in &targets {
                let surface = target.surface.clone().ok_or_else(|| {
                    RendererError::at("recovery_surface", "missing event-thread surface")
                })?;
                if let Some(error) = gpu.failure() {
                    discard(restored);
                    return Err(error.clone());
                }
                let result =
                    WindowRenderer::from_surface(target.window.clone(), gpu.clone(), surface)
                        .inspect_err(|error| gpu.failure.record(error));
                #[cfg(feature = "viewport")]
                let result = result.and_then(|mut renderer| {
                    renderer.configure_scene(target.scene)?;
                    Ok(renderer)
                });
                match result {
                    Ok(renderer) => restored.push(renderer),
                    Err(error) => {
                        gpu.failure.record(&error);
                        discard(restored);
                        return Err(error);
                    }
                }
            }
            Ok(restored)
        });
        match outcome {
            Ok(renderers) => return Ok(renderers),
            Err(error) => last_error = error,
        }
        if scope == RecoveryScope::Surface && last_error.fault == RendererFault::DeviceLost {
            return Err(last_error);
        }
        if kind == JobKind::Recovery {
            thread::sleep(Duration::from_millis(50 << attempt));
        }
    }
    Err(last_error)
}
fn reconfigure_targets(
    mut targets: Vec<RecoveryTarget>,
    cancel: &AtomicBool,
    deadline: Instant,
    #[cfg(feature = "fault-injection")] fixture: Option<RecoveryFixture>,
) -> Outcome {
    let mut target = targets
        .pop()
        .ok_or_else(|| RendererError::at("reconfiguration", "missing renderer owner"))?;
    let mut renderer = target
        .renderer
        .take()
        .ok_or_else(|| RendererError::at("reconfiguration", "missing renderer owner"))?;
    let desired = target
        .configuration
        .ok_or_else(|| RendererError::at("reconfiguration", "missing target configuration"))?;
    let submission = renderer.submission.take();
    let acquired = renderer.acquired.take();
    #[cfg(feature = "fault-injection")]
    let owned = (submission.is_some(), acquired.is_some());
    retire_owned_futures(&renderer.gpu, submission, acquired)?;
    #[cfg(feature = "fault-injection")]
    {
        eprintln!(
            "UI-R11 retirement: worker={} submission={} acquired={} retirement_complete=true",
            renderer.gpu.worker.on_worker(),
            owned.0,
            owned.1
        );
        if renderer.presentation_probe == crate::PresentationProbe::AwaitCompletion {
            renderer.presentation_probe = crate::PresentationProbe::Complete;
            eprintln!("UI-R04 presentation owner: real_completion_before_rebuild=true");
        }
    }
    if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
        return Err(if Instant::now() >= deadline {
            JobKind::Reconfiguration.timeout()
        } else {
            JobKind::Reconfiguration.cancelled()
        });
    }
    if let Some(error) = renderer.gpu.failure().or(renderer.failed.as_ref()) {
        return Err(error.clone());
    }
    #[cfg(feature = "fault-injection")]
    if let Some(fixture) = fixture {
        let error = match fixture {
            RecoveryFixture::ReconfigurationFailure => Some(RendererError::at(
                "reconfiguration_fixture",
                "intentional target factory rejection after real retirement",
            )),
            RecoveryFixture::ReconfigurationDeviceLoss => Some(RendererError::caused_by(
                "reconfiguration_fixture",
                vulkano::VulkanError::DeviceLost,
            )),
            RecoveryFixture::ReconfigurationSurfaceLoss => Some(RendererError::caused_by(
                "reconfiguration_fixture",
                vulkano::VulkanError::SurfaceLost,
            )),
            _ => None,
        };
        if let Some(error) = error {
            renderer.gpu.failure.record(&error);
            return Err(error);
        }
    }
    let rebuilt = if renderer.recreate || renderer.swapchain.image_extent() != desired.extent {
        renderer.rebuild(desired.extent)
    } else {
        Ok(())
    };
    #[cfg(feature = "fault-injection")]
    let rebuilt = rebuilt.and_then(|()| {
        if matches!(fixture, Some(RecoveryFixture::ReconfigurationOutOfDate)) {
            // Use the real API to retire this target, then expose the typed
            // driver failure. Never try to resurrect a retired swapchain.
            let replacement = renderer
                .swapchain
                .recreate(renderer.swapchain.create_info())
                .map_err(|error| RendererError::caused_by("reconfiguration_fixture", error))?;
            drop(replacement);
            eprintln!("UI-R11 fixture: superseded_target_retired=true");
            return Err(RendererError::caused_by(
                "reconfiguration_fixture",
                vulkano::VulkanError::OutOfDate,
            ));
        }
        Ok(())
    });
    if let Err(error) = rebuilt {
        if error.code == "native_vulkan_target_unavailable" {
            renderer.recreate = true;
            renderer.retry.reset();
            return Ok(vec![renderer]);
        }
        if error.fault != RendererFault::SwapchainOutOfDate {
            renderer.gpu.failure.record(&error);
            return Err(error);
        }
        // Vulkano retires oldSwapchain even when recreation fails. Release
        // all of its owners here before constructing a fresh target on the
        // same Surface/context. CPU/author state is not part of this owner.
        let gpu = renderer.gpu.clone();
        let window = renderer.window.clone();
        let surface = renderer.swapchain.surface().clone();
        let recreations = renderer.recreations;
        let cpu_profiling = renderer.cpu_profiling;
        let gpu_profiling = renderer.gpu_profile.is_some();
        #[cfg(feature = "viewport")]
        let scene_frames = renderer.scene_frames;
        #[cfg(feature = "fault-injection")]
        let probes = (
            std::mem::take(&mut renderer.acquire_faults),
            std::mem::take(&mut renderer.presentation_probe),
            std::mem::take(&mut renderer.recording_probe),
            std::mem::take(&mut renderer.completion_probe),
            renderer.recovery_probe.take(),
        );
        drop(renderer);
        thread::sleep(Duration::from_millis(16));
        if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
            return Err(if Instant::now() >= deadline {
                JobKind::Reconfiguration.timeout()
            } else {
                JobKind::Reconfiguration.cancelled()
            });
        }
        renderer = WindowRenderer::from_surface(window, gpu.clone(), surface)
            .inspect_err(|cause| gpu.failure.record(cause))?;
        renderer.recreations = recreations.saturating_add(1);
        renderer.cpu_profiling = cpu_profiling;
        let _ = renderer.enable_gpu_profiling(gpu_profiling)?;
        #[cfg(feature = "viewport")]
        {
            renderer.scene_frames = scene_frames;
        }
        #[cfg(feature = "fault-injection")]
        {
            (
                renderer.acquire_faults,
                renderer.presentation_probe,
                renderer.recording_probe,
                renderer.completion_probe,
                renderer.recovery_probe,
            ) = probes;
        }
    }
    #[cfg(feature = "viewport")]
    if let Err(error) = renderer.configure_scene(desired.mode.is_scene()) {
        renderer.gpu.failure.record(&error);
        return Err(error);
    }
    renderer.retry.reset();
    renderer.cpu_profile = None;
    Ok(vec![renderer])
}
fn retire(mut renderer: WindowRenderer) -> Result<(), RendererError> {
    let submission = renderer.submission.take();
    let acquired = renderer.acquired.take();
    let result = retire_owned_futures(&renderer.gpu, submission, acquired);
    drop(renderer);
    result
}
pub(crate) fn retire_owned_futures(
    gpu: &GpuContext,
    mut submission: Option<crate::submission::Submission>,
    acquired: Option<vulkano::swapchain::SwapchainAcquireFuture>,
) -> Result<(), RendererError> {
    let mut loss = gpu.failure().cloned();
    let mut fault = None;
    if let Some(submission) = &mut submission {
        loop {
            match submission.poll_retirement() {
                Ok(true) => break,
                Ok(false) => thread::sleep(Duration::from_millis(1)),
                Err(error) => {
                    if error.fault == RendererFault::DeviceLost {
                        gpu.failure.record(&error);
                        loss = Some(error.clone());
                    } else {
                        fault = Some(error);
                    }
                    break;
                }
            }
        }
    }
    // Separate owners: a dependency unwind cannot strand the other future.
    if let Err(error) = retire_futures(submission, acquired, loss.is_some()) {
        fault.get_or_insert(error);
    }
    match fault {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
fn retire_futures(
    submission: impl Sized,
    acquired: impl Sized,
    confirmed_loss: bool,
) -> Result<(), RendererError> {
    let mut unknown_panic = false;
    for outcome in [
        catch_unwind(AssertUnwindSafe(|| drop(submission))),
        catch_unwind(AssertUnwindSafe(|| drop(acquired))),
    ] {
        if outcome.is_err() {
            if !confirmed_loss {
                unknown_panic = true;
            } else {
                eprintln!(
                    "native_vulkan_lost_generation_retirement_unwind: GPU owner discarded after typed DeviceLost"
                );
            }
        }
    }
    if unknown_panic {
        Err(RendererError { code: "native_vulkan_retirement_dependency_panicked", stage: "retirement", fault: RendererFault::Other, message: "GPU dependency panicked while retiring an unconfirmed generation; DeviceLost is not inferred".into(), element: None })
    } else {
        Ok(())
    }
}
fn discard(renderers: Vec<WindowRenderer>) {
    for renderer in renderers {
        if let Err(error) = retire(renderer) {
            eprintln!("{error}");
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dependency_drop_unwind_releases_both_owners_and_unknown_loss_stays_an_error() {
        use std::sync::atomic::AtomicUsize;
        struct Owner(Arc<AtomicUsize>);
        impl Drop for Owner {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Relaxed);
                panic!("owned dependency drop fixture");
            }
        }
        for confirmed in [false, true] {
            let dropped = Arc::new(AtomicUsize::new(0));
            let result = retire_futures(Owner(dropped.clone()), Owner(dropped.clone()), confirmed);
            assert_eq!(dropped.load(Ordering::Relaxed), 2);
            if confirmed {
                assert!(result.is_ok());
            } else {
                let error = result.expect_err("unknown panic must not sign successful retirement");
                assert_eq!(error.fault, RendererFault::Other);
                assert_eq!(error.code, "native_vulkan_retirement_dependency_panicked");
            }
        }
    }
    #[test]
    fn cancelled_result_is_drained_on_producer() {
        for cancel_before_delivery in [true, false] {
            let result = Arc::new(ResultSlot::default());
            if cancel_before_delivery {
                result.cancel();
            }
            let worker = result.clone();
            let (finished, completion) = mpsc::sync_channel(1);
            let task = thread::spawn(move || {
                worker.deliver(Err(timeout()), Instant::now() + Duration::from_secs(10));
                finished.send(()).expect("completion observer");
            });
            if !cancel_before_delivery {
                let deadline = Instant::now() + Duration::from_secs(1);
                loop {
                    if result.outcome.lock().expect("result mutex").is_some() {
                        break;
                    }
                    assert!(Instant::now() < deadline, "result must be published");
                    thread::yield_now();
                }
                assert!(
                    !result.done.load(Ordering::Acquire),
                    "producer retains an unaccepted result"
                );
                result.cancel();
            }
            // A missing cancellation wake must fail instead of silently
            // passing when the delivery deadline eventually drains the slot.
            completion
                .recv_timeout(Duration::from_secs(1))
                .expect("cancellation releases delivery before its deadline");
            task.join().expect("retirement producer must finish");
            assert!(result.done.load(Ordering::Acquire));
            assert!(result.outcome.lock().expect("result mutex").is_none());
        }
    }
    #[test]
    fn producer_waits_for_result_acceptance() {
        let result = Arc::new(ResultSlot::default());
        let worker = result.clone();
        let task = thread::spawn(move || {
            worker.deliver(Err(timeout()), Instant::now() + Duration::from_secs(1))
        });
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            if result
                .outcome
                .lock()
                .expect("result mutex")
                .take()
                .is_some()
            {
                break;
            }
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        result.changed.notify_one();
        task.join().expect("retirement producer must finish");
        assert!(result.done.load(Ordering::Acquire));
    }
}
