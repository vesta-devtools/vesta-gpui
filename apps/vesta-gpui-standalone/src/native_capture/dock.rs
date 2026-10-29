//! Observes native Tab drag cancellation through existing Workspace commands.
use super::*;

mod lifecycle;

pub(super) struct Probe(Kind);

enum Kind {
    Mutation(MutationProbe),
    Lifecycle(lifecycle::Probe),
}

impl Probe {
    pub(super) fn from_case(case: &str, source: Source) -> Option<Self> {
        lifecycle::Probe::from_case(case, source)
            .map(Kind::Lifecycle)
            .or_else(|| MutationProbe::from_case(case, source).map(Kind::Mutation))
            .map(Self)
    }

    pub(super) fn single_tab(&self) -> bool {
        match &self.0 {
            Kind::Mutation(probe) => probe.single_tab(),
            Kind::Lifecycle(_) => false,
        }
    }

    pub(super) fn observe(
        &mut self,
        app: &mut App,
        event_loop: &ActiveEventLoop,
        window: UiWindowId,
        event: Event,
        sequence: u64,
    ) -> Result<bool, String> {
        match &mut self.0 {
            Kind::Mutation(probe) => probe.observe(app, event_loop, window, event, sequence),
            Kind::Lifecycle(probe) => probe.observe(app, window, event, sequence),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Mutation {
    Remove,
    Reset,
    Restore,
}

struct Pending {
    window: UiWindowId,
    item: ItemId,
    origin: ContainerRef,
    pane: PaneId,
    after: Option<vesta_workspace::WorkspaceModel>,
}

struct MutationProbe {
    source: Source,
    mutation: Mutation,
    pending: Option<Pending>,
    single_tab: bool,
}

impl MutationProbe {
    pub(super) fn from_case(case: &str, source: Source) -> Option<Self> {
        let mutation = if case.ends_with("-remove") {
            Mutation::Remove
        } else if case.ends_with("-reset") {
            Mutation::Reset
        } else if case.ends_with("-restore") {
            Mutation::Restore
        } else {
            return None;
        };
        Some(Self {
            source,
            mutation,
            pending: None,
            single_tab: case == "main-pane-remove",
        })
    }

    pub(super) fn single_tab(&self) -> bool {
        self.single_tab
    }

    pub(super) fn observe(
        &mut self,
        app: &mut App,
        event_loop: &ActiveEventLoop,
        window: UiWindowId,
        event: Event,
        sequence: u64,
    ) -> Result<bool, String> {
        match event {
            Event::Press => {
                if self.pending.is_some() {
                    return Err("native_dock_interrupted: a new press preceded release".into());
                }
                if (self.source == Source::Main) != (window == STANDALONE_WINDOW) {
                    return Ok(false);
                }
                let Some(drag) = app
                    .workspace_drag
                    .filter(|drag| app.window_container(window) == Some(drag.origin_container))
                else {
                    return Ok(false);
                };
                if drag.started {
                    return Err("native_dock_order: drag was already started at press".into());
                }
                let pane = container_panes(&app.demo.workspace, drag.origin_container)
                    .into_iter()
                    .find(|id| {
                        app.demo
                            .workspace
                            .pane(*id)
                            .is_some_and(|pane| pane.items.iter().any(|item| item.id == drag.item))
                    })
                    .ok_or("native_dock_identity: pressed item has no source pane")?;
                if self.single_tab
                    && app
                        .demo
                        .workspace
                        .pane(pane)
                        .is_none_or(|pane| pane.items.len() != 1)
                {
                    return Err("native_dock_setup: last-tab source is not singular".into());
                }
                self.pending = Some(Pending {
                    window,
                    item: drag.item,
                    origin: drag.origin_container,
                    pane,
                    after: None,
                });
                eprintln!(
                    "native_dock_fixture press window={} sequence={} item={} origin={:?} started=false mutation={:?}",
                    window.0, sequence, drag.item.0, drag.origin_container, self.mutation
                );
            }
            Event::Move => {
                let Some(pending) = self
                    .pending
                    .as_mut()
                    .filter(|pending| pending.after.is_none() && pending.window == window)
                else {
                    return Ok(false);
                };
                let drag = app
                    .workspace_drag
                    .ok_or("native_dock_interrupted: drag vanished before threshold motion")?;
                if drag.item != pending.item || drag.origin_container != pending.origin {
                    return Err(
                        "native_dock_identity: another gesture replaced the pressed Tab".into(),
                    );
                }
                if !drag.started {
                    return Ok(false);
                }
                eprintln!(
                    "native_dock_fixture motion window={} sequence={} item={} started=true target_present={}",
                    window.0,
                    sequence,
                    pending.item.0,
                    drag.target.is_some()
                );
                match self.mutation {
                    Mutation::Remove => {
                        app.apply_workspace_action(WorkspaceViewAction::CloseTab(pending.item))
                    }
                    Mutation::Reset => app.apply_demo_action(DemoAction::WorkspaceReset),
                    Mutation::Restore => {
                        let ContainerRef::Floating(id) = pending.origin else {
                            return Err(
                                "native_dock_setup: restore requires a floating source".into()
                            );
                        };
                        app.apply_demo_action(DemoAction::RestoreFloating(id));
                    }
                }
                let item_present = app.demo.workspace.item(pending.item).is_some();
                if item_present != matches!(self.mutation, Mutation::Reset | Mutation::Restore) {
                    return Err(
                        "native_dock_mutation: item removal or Reset identity reuse failed".into(),
                    );
                }
                if self.single_tab && app.demo.workspace.pane(pending.pane).is_some() {
                    return Err("native_dock_mutation: last source pane survived removal".into());
                }
                if let ContainerRef::Floating(id) = pending.origin
                    && app.demo.workspace.floating_window(id).is_some()
                {
                    return Err("native_dock_mutation: old floating source survived removal".into());
                }
                app.sync_native_windows(event_loop);
                pending.after = Some(app.demo.workspace.clone());
                Self::verify(app, pending)?;
                app.request_all_redraw();
                eprintln!(
                    "native_dock_fixture canceled window={} sequence={} item={} item_present={} drag=false capture=false mutation={:?}",
                    window.0, sequence, pending.item.0, item_present, self.mutation
                );
            }
            Event::Release => {
                let Some(pending) = &self.pending else {
                    return Ok(false);
                };
                if pending.after.is_none() {
                    return Err(
                        "native_dock_order: release preceded threshold motion and mutation".into(),
                    );
                }
                Self::verify(app, pending)?;
                if app
                    .fixture_input(window)
                    .is_some_and(|input| input.capture().is_some())
                {
                    return Err("native_dock_release: release window retained capture".into());
                }
                eprintln!(
                    "native_dock_fixture passed source={} release_window={} foreign_release={} sequence={} item={} mutation={:?} workspace_preserved=true drag=false late_commit=false",
                    pending.window.0,
                    window.0,
                    pending.window != window,
                    sequence,
                    pending.item.0,
                    self.mutation
                );
                return Ok(true);
            }
            Event::Blur
            | Event::Other
            | Event::Resized(_)
            | Event::WindowMoved(_)
            | Event::Focus
            | Event::ThemeKey { .. }
            | Event::Ime { .. } => {}
        }
        Ok(false)
    }

    fn verify(app: &App, pending: &Pending) -> Result<(), String> {
        if pending.after.as_ref() != Some(&app.demo.workspace)
            || app.workspace_drag.is_some()
            || app.floating_move.is_some()
            || app.workspace_resize.is_some()
            || app
                .fixture_input(pending.window)
                .is_some_and(|input| input.capture().is_some())
            || app.dock_drag_visual(ContainerRef::Main).is_some()
        {
            return Err("native_dock_cancellation: old gesture, preview or late Workspace mutation survived".into());
        }
        Ok(())
    }
}
