//! Explicit smoke-only exercise of real floating-window GPU ownership.
use super::*;
use std::sync::OnceLock;

pub(super) fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::args().any(|arg| arg == "--smoke-gpu-lifecycle"))
}

#[derive(Default)]
pub(super) enum Smoke {
    #[default]
    Disabled,
    First {
        item: ItemId,
        resize_requested: bool,
    },
    #[cfg(feature = "gpu-fault-injection")]
    ObservingDestination {
        item: ItemId,
        floating: FloatingWindowId,
    },
    Recreated {
        closed: FloatingWindowId,
        context: Arc<vesta_gpui_vulkan::GpuContext>,
        closed_at_frame: u64,
    },
    #[cfg(feature = "gpu-fault-injection")]
    MainSuspension(Box<MainDockProbe>),
    Complete,
}

#[cfg(feature = "gpu-fault-injection")]
pub(super) struct MainDockProbe {
    floating: FloatingWindowId,
    source: ContainerRef,
    item: ItemId,
    context: Arc<vesta_gpui_vulkan::GpuContext>,
    workspace: vesta_workspace::WorkspaceModel,
    main_frames: u64,
    main_recreations: u32,
    floating_frames: u64,
    blocked_seen: bool,
}

impl App {
    pub(super) fn advance_gpu_lifecycle(&mut self) -> Result<bool, String> {
        let state = std::mem::take(&mut self.gpu_lifecycle);
        let Some(gpu) = &self.gpu else {
            self.gpu_lifecycle = state;
            return Ok(false);
        };
        self.gpu_lifecycle = match state {
            finished @ (Smoke::Disabled | Smoke::Complete) => {
                self.gpu_lifecycle = finished;
                return Ok(true);
            }
            Smoke::First {
                item,
                resize_requested,
            } => {
                let Some(host) = self.native_windows.values().next() else {
                    self.gpu_lifecycle = Smoke::First {
                        item,
                        resize_requested,
                    };
                    return Ok(false);
                };
                if host.smoke_frames >= 1 && !resize_requested {
                    let window = host.window.clone();
                    let floating = host.logical_id;
                    let before = self.demo.workspace.clone();
                    self.seed_lifecycle_drag(item, ContainerRef::Floating(floating))?;
                    if self.finish_workspace_drag(ContainerRef::Main)
                        || self.workspace_drag.is_none()
                        || self.demo.workspace != before
                    {
                        return Err(
                            "ui_gpu_lifecycle_failed: foreign release consumed the floating drag"
                                .into(),
                        );
                    }
                    eprintln!(
                        "UI-Dock foreign_release_retained=true workspace_preserved=true pointer_io=false"
                    );
                    #[cfg(feature = "gpu-fault-injection")]
                    if let Some(host) = self
                        .native_windows
                        .values_mut()
                        .find(|host| host.logical_id == floating)
                    {
                        host.surface.inject_update_fixture(
                            vesta_gpui_vulkan::RecoveryFixture::DelayedReconfiguration,
                            false,
                        );
                    }
                    let _ = window.request_inner_size(LogicalSize::new(560.0, 380.0));
                    eprintln!("UI-R02 resize_requested floating={}", floating.0);
                    #[cfg(feature = "gpu-fault-injection")]
                    {
                        Smoke::ObservingDestination { item, floating }
                    }
                    #[cfg(not(feature = "gpu-fault-injection"))]
                    {
                        Smoke::First {
                            item,
                            resize_requested: true,
                        }
                    }
                } else if host.smoke_frames >= 2 && host.surface.swapchain_recreations() > 0 {
                    let context = gpu.gpu_context().ok_or("main GPU unavailable")?;
                    if !Arc::ptr_eq(
                        &context,
                        &host
                            .surface
                            .gpu_context()
                            .ok_or("floating GPU unavailable")?,
                    ) {
                        return Err(
                            "ui_gpu_lifecycle_failed: first floating context differs".into()
                        );
                    }
                    let window_id = host.window.id();
                    let closed = host.logical_id;
                    if self.workspace_drag.is_some() {
                        return Err(
                            "ui_gpu_lifecycle_failed: floating resize retained its source drag"
                                .into(),
                        );
                    }
                    eprintln!(
                        "UI-Dock floating_resize origin_retired=true current_frame=true pointer_io=false"
                    );
                    #[cfg(feature = "gpu-fault-injection")]
                    self.verify_lifecycle_destination(closed, false)?;
                    self.seed_lifecycle_drag(item, ContainerRef::Floating(closed))?;
                    self.restore_native_window(window_id);
                    if self.native_windows.contains_key(&window_id)
                        || self.demo.workspace.floating_window(closed).is_some()
                        || self.workspace_drag.is_some()
                    {
                        return Err("ui_gpu_lifecycle_failed: floating teardown retained its window or drag".into());
                    }
                    eprintln!(
                        "UI-Dock floating_teardown origin_retired=true mapping_live_during_cleanup=true pointer_io=false"
                    );
                    eprintln!("UI-R02 closed floating={} after_resize=true", closed.0);
                    if !self.float_workspace_item(
                        item,
                        WindowPlacement::new(100.0, 100.0, 640.0, 480.0),
                    ) {
                        return Err(
                            "ui_gpu_lifecycle_failed: could not recreate floating item".into()
                        );
                    }
                    Smoke::Recreated {
                        closed,
                        context,
                        closed_at_frame: self.rendered_frames,
                    }
                } else {
                    Smoke::First {
                        item,
                        resize_requested,
                    }
                }
            }
            #[cfg(feature = "gpu-fault-injection")]
            Smoke::ObservingDestination { item, floating } => {
                let host = self
                    .native_windows
                    .values()
                    .find(|host| host.logical_id == floating)
                    .ok_or("resizing floating host missing")?;
                if host.surface.update_prepared() {
                    if !self.display_blocked(host.window.id()) || self.workspace_drag.is_some() {
                        return Err("ui_gpu_lifecycle_failed: prepared target exposed input or retained its source drag".into());
                    }
                    self.verify_lifecycle_destination(floating, true)?;
                    Smoke::First {
                        item,
                        resize_requested: true,
                    }
                } else if host.smoke_frames >= 2 && !self.display_blocked(host.window.id()) {
                    return Err(
                        "ui_gpu_lifecycle_failed: missed prepared blocked destination".into(),
                    );
                } else {
                    Smoke::ObservingDestination { item, floating }
                }
            }
            Smoke::Recreated {
                closed,
                context,
                closed_at_frame,
            } => {
                let ready = self
                    .native_windows
                    .values()
                    .any(|host| host.logical_id != closed && host.smoke_frames >= 2);
                if ready && self.rendered_frames > closed_at_frame {
                    if !Arc::ptr_eq(&context, &gpu.gpu_context().ok_or("main GPU unavailable")?)
                        || self.native_windows.values().any(|host| {
                            host.surface
                                .gpu_context()
                                .is_none_or(|gpu| !Arc::ptr_eq(&context, &gpu))
                        })
                    {
                        return Err(
                            "ui_gpu_lifecycle_failed: recreated window lost original context"
                                .into(),
                        );
                    }
                    eprintln!(
                        "UI-R02 recreated_presented=true main_continued=true original_gpu_context=true"
                    );
                    let floating = self
                        .native_windows
                        .values()
                        .next()
                        .ok_or("recreated floating host missing")?;
                    let foreign = ContainerRef::Floating(floating.logical_id);
                    let ui_window = floating.ui_window;
                    #[cfg(feature = "gpu-fault-injection")]
                    let floating_id = floating.logical_id;
                    let main_item = self
                        .demo
                        .workspace
                        .panes()
                        .iter()
                        .find_map(|pane| pane.items.first().map(|item| item.id))
                        .ok_or("main Dock item missing")?;
                    let before = self.demo.workspace.clone();
                    self.seed_lifecycle_drag(main_item, ContainerRef::Main)?;
                    self.cancel_window_editors(ui_window);
                    if self.workspace_drag.is_none()
                        || self.finish_workspace_drag(foreign)
                        || self.workspace_drag.is_none()
                        || self.demo.workspace != before
                    {
                        return Err("ui_gpu_lifecycle_failed: recreated floating cleanup or release consumed the main drag".into());
                    }
                    self.cancel_workspace_drag_for_container(ContainerRef::Main);
                    eprintln!(
                        "UI-Dock recreated foreign_cleanup_retained=true foreign_release_retained=true workspace_preserved=true pointer_io=false"
                    );
                    #[cfg(feature = "gpu-fault-injection")]
                    {
                        self.begin_main_dock_suspension(floating_id, ContainerRef::Main, context)?
                    }
                    #[cfg(not(feature = "gpu-fault-injection"))]
                    {
                        Smoke::Complete
                    }
                } else {
                    Smoke::Recreated {
                        closed,
                        context,
                        closed_at_frame,
                    }
                }
            }
            #[cfg(feature = "gpu-fault-injection")]
            Smoke::MainSuspension(mut probe) => {
                let window = self.window.as_ref().ok_or("main native window missing")?;
                let blocked = self.display_blocked(window.id());
                let prepared = gpu.update_prepared();
                let recreated = gpu.swapchain_recreations() > probe.main_recreations;
                let host = self
                    .native_windows
                    .values()
                    .find(|host| host.logical_id == probe.floating)
                    .ok_or("main suspension floating host missing")?;
                let retained = match probe.source {
                    ContainerRef::Main => self.workspace_drag.is_none(),
                    ContainerRef::Floating(_) => self.workspace_drag.is_some_and(|drag| {
                        drag.item == probe.item
                            && drag.origin_container == probe.source
                            && drag.pointer_container == Some(probe.source)
                            && drag.target.is_some()
                    }),
                };
                if self.demo.workspace != probe.workspace
                    || ((blocked || recreated || probe.blocked_seen) && !retained)
                {
                    return Err("ui_gpu_lifecycle_failed: Main suspension changed Workspace or another window's drag".into());
                }
                if prepared && !probe.blocked_seen {
                    if !blocked || self.display_blocked(host.window.id()) {
                        return Err("ui_gpu_lifecycle_failed: prepared Main did not isolate display admission".into());
                    }
                    probe.blocked_seen = true;
                    eprintln!(
                        "UI-Dock main_prepared source_main={} input_blocked=true own_origin_retired={} unrelated_floating_retained={} workspace_preserved=true pointer_io=false",
                        probe.source == ContainerRef::Main,
                        probe.source == ContainerRef::Main,
                        probe.source != ContainerRef::Main,
                    );
                }
                if !blocked && recreated && self.rendered_frames > probe.main_frames {
                    if !probe.blocked_seen
                        || host.smoke_frames <= probe.floating_frames
                        || !Arc::ptr_eq(
                            &probe.context,
                            &gpu.gpu_context().ok_or("main GPU unavailable")?,
                        )
                        || !Arc::ptr_eq(
                            &probe.context,
                            &host
                                .surface
                                .gpu_context()
                                .ok_or("floating GPU unavailable")?,
                        )
                    {
                        return Err("ui_gpu_lifecycle_failed: Main suspension missed its barrier, Floating presentation or original context".into());
                    }
                    eprintln!(
                        "UI-Dock main_restored source_main={} current_frame=true floating_continued=true original_gpu_context=true workspace_preserved=true pointer_io=false",
                        probe.source == ContainerRef::Main,
                    );
                    if probe.source == ContainerRef::Main {
                        self.begin_main_dock_suspension(
                            probe.floating,
                            ContainerRef::Floating(probe.floating),
                            probe.context,
                        )?
                    } else {
                        self.cancel_workspace_drag_for_container(probe.source);
                        Smoke::Complete
                    }
                } else {
                    Smoke::MainSuspension(probe)
                }
            }
        };
        self.request_all_redraw();
        Ok(matches!(self.gpu_lifecycle, Smoke::Complete))
    }

    #[cfg(feature = "gpu-fault-injection")]
    pub(super) fn poll_lifecycle_suspension(&mut self) -> Result<(), String> {
        if matches!(&self.gpu_lifecycle, Smoke::MainSuspension(probe) if !probe.blocked_seen)
            && self.gpu.as_ref().is_some_and(GpuState::update_prepared)
        {
            self.advance_gpu_lifecycle()?;
        }
        Ok(())
    }

    #[cfg(feature = "gpu-fault-injection")]
    fn begin_main_dock_suspension(
        &mut self,
        floating: FloatingWindowId,
        source: ContainerRef,
        context: Arc<vesta_gpui_vulkan::GpuContext>,
    ) -> Result<Smoke, String> {
        let host = self
            .native_windows
            .values()
            .find(|host| host.logical_id == floating)
            .ok_or("Main suspension floating host missing")?;
        let frame = if source == ContainerRef::Main {
            &self.dock_frame
        } else {
            &host.dock_frame
        };
        let pane = frame.panes.first().ok_or("source Dock frame missing")?;
        let item = self
            .demo
            .workspace
            .pane(pane.pane)
            .and_then(|pane| pane.items.first())
            .ok_or("source Dock item missing")?
            .id;
        let cursor = Point::new(
            pane.body.x + pane.body.width * 0.5,
            pane.body.y + pane.body.height * 0.5,
        );
        let target = frame
            .target(cursor, &self.demo.workspace)
            .ok_or("source Dock target missing")?;
        let floating_frames = host.smoke_frames;
        self.workspace_drag = Some(WorkspaceTabDrag {
            item,
            origin_container: source,
            origin: cursor,
            started: true,
            pointer_container: Some(source),
            cursor,
            target: Some(target),
        });
        let probe = MainDockProbe {
            floating,
            source,
            item,
            context,
            workspace: self.demo.workspace.clone(),
            main_frames: self.rendered_frames,
            main_recreations: self
                .gpu
                .as_ref()
                .ok_or("main GPU missing")?
                .swapchain_recreations(),
            floating_frames,
            blocked_seen: false,
        };
        self.gpu
            .as_mut()
            .ok_or("main GPU missing")?
            .inject_update_fixture(
                vesta_gpui_vulkan::RecoveryFixture::DelayedReconfiguration,
                false,
            );
        let extent = if source == ContainerRef::Main {
            LogicalSize::new(1010.0, 710.0)
        } else {
            LogicalSize::new(990.0, 690.0)
        };
        let _ = self
            .window
            .as_ref()
            .ok_or("main native window missing")?
            .request_inner_size(extent);
        eprintln!(
            "UI-Dock main_resize_requested source={source:?} real_owner=true pointer_io=false"
        );
        Ok(Smoke::MainSuspension(Box::new(probe)))
    }

    fn seed_lifecycle_drag(&mut self, item: ItemId, origin: ContainerRef) -> Result<(), String> {
        let pane = self
            .dock_frame
            .panes
            .first()
            .ok_or("main Dock frame missing")?;
        let cursor = Point::new(
            pane.body.x + pane.body.width * 0.5,
            pane.body.y + pane.body.height * 0.5,
        );
        let target = self
            .dock_frame
            .target(cursor, &self.demo.workspace)
            .ok_or("main Dock target missing")?;
        self.workspace_drag = Some(WorkspaceTabDrag {
            item,
            origin_container: origin,
            origin: cursor,
            started: true,
            pointer_container: Some(ContainerRef::Main),
            cursor,
            target: Some(target),
        });
        Ok(())
    }

    #[cfg(feature = "gpu-fault-injection")]
    fn verify_lifecycle_destination(
        &mut self,
        floating: FloatingWindowId,
        blocked: bool,
    ) -> Result<(), String> {
        let host = self
            .native_windows
            .values()
            .find(|host| host.logical_id == floating)
            .ok_or("floating Dock destination missing")?;
        let destination = ContainerRef::Floating(floating);
        let main = dock_drag::window_geometry(
            ContainerRef::Main,
            self.window.as_ref().ok_or("main native window missing")?,
        )
        .ok_or("main native geometry missing")?;
        let native = dock_drag::window_geometry(destination, &host.window)
            .ok_or("floating native geometry missing")?;
        let pane = host
            .dock_frame
            .panes
            .first()
            .ok_or("floating Dock frame missing")?;
        let cursor = Point::new(
            pane.body.x + pane.body.width * 0.5,
            pane.body.y + pane.body.height * 0.5,
        );
        let target = host
            .dock_frame
            .target(cursor, &self.demo.workspace)
            .ok_or("floating Dock target missing")?;
        let screen = Point::new(
            native.client.x + cursor.x * native.scale,
            native.client.y + cursor.y * native.scale,
        );
        if !native.outer.contains(screen) {
            return Err(
                "ui_gpu_lifecycle_failed: Dock probe was outside the occluding native window"
                    .into(),
            );
        }
        let source_cursor = Point::new(
            (screen.x - main.client.x) / main.scale,
            (screen.y - main.client.y) / main.scale,
        );
        let item = self
            .demo
            .workspace
            .panes()
            .iter()
            .find_map(|pane| pane.items.first().map(|item| item.id))
            .ok_or("main Dock item missing")?;
        let before = self.demo.workspace.clone();
        self.seed_lifecycle_drag(item, ContainerRef::Main)?;
        self.update_native_dock_pointer(ContainerRef::Main, source_cursor);
        let drag = self
            .workspace_drag
            .ok_or("live main drag unexpectedly retired")?;
        if blocked {
            if drag.target.is_some() || drag.pointer_container.is_some() {
                return Err("ui_gpu_lifecycle_failed: blocked native destination exposed stale geometry or fell through".into());
            }
            self.seed_lifecycle_drag(item, ContainerRef::Main)?;
            let drag = self
                .workspace_drag
                .as_mut()
                .ok_or("main release candidate missing")?;
            drag.pointer_container = Some(destination);
            drag.target = Some(target);
            if self.finish_workspace_drag(destination)
                || self.workspace_drag.is_none()
                || !self.finish_workspace_drag(ContainerRef::Main)
                || self.workspace_drag.is_some()
                || self.finish_workspace_drag(ContainerRef::Main)
                || self.demo.workspace != before
            {
                return Err("ui_gpu_lifecycle_failed: stale destination release changed Workspace or lost its origin".into());
            }
            eprintln!(
                "UI-Dock destination_blocked=true stale_geometry_rejected=true occlusion_preserved=true stale_release_consumed_once=true workspace_preserved=true pointer_io=false"
            );
        } else {
            if drag.pointer_container != Some(destination) || drag.target.is_none() {
                return Err(
                    "ui_gpu_lifecycle_failed: current floating frame did not resume Dock routing"
                        .into(),
                );
            }
            self.cancel_workspace_drag_for_container(ContainerRef::Main);
            if self.demo.workspace != before {
                return Err("ui_gpu_lifecycle_failed: restored routing changed Workspace".into());
            }
            eprintln!(
                "UI-Dock destination_restored=true current_frame=true routing_resumed=true workspace_preserved=true pointer_io=false"
            );
        }
        Ok(())
    }
}
