//! Normal Float restoration during a real document gesture; never owns a Window Arc.
use super::*;

struct Pending {
    source: UiWindowId,
    native: WindowId,
    logical: vesta_workspace::FloatingWindowId,
    item: ItemId,
    before: State,
    values: String,
    after: Option<vesta_workspace::WorkspaceModel>,
    released: Option<(UiWindowId, u64)>,
}

#[derive(Default)]
pub(super) struct Probe {
    before: Option<State>,
    pending: Option<Pending>,
}

impl Probe {
    pub(super) fn retired_window(&self, native: WindowId) -> Option<UiWindowId> {
        self.pending
            .as_ref()
            .filter(|pending| pending.native == native && pending.after.is_some())
            .map(|pending| pending.source)
    }

    pub(super) fn observe(
        &mut self,
        app: &mut App,
        event_loop: &ActiveEventLoop,
        window: UiWindowId,
        event: Event,
        sequence: u64,
    ) -> Result<bool, String> {
        if self.pending.is_none() && window != STANDALONE_WINDOW {
            let Some(state) = State::read(app, window) else {
                return Ok(false);
            };
            if !state.dragging {
                self.before = Some(state);
                return Ok(false);
            }
            let before = self
                .before
                .take()
                .ok_or("native_document_restore_baseline")?;
            if !matches!(event, Event::Press)
                || before.dragging
                || before.capture.is_some()
                || before.scroll_pending
                || state.target != before.target
                || state.binding != before.binding
                || state.capture != Some((state.target, PointerButton::Primary))
            {
                return Err("native_document_restore_press: current native source required".into());
            }
            let (native, logical) = app
                .native_windows
                .iter()
                .find(|(_, host)| host.ui_window == window)
                .map(|(native, host)| (*native, host.logical_id))
                .ok_or("native_document_restore_window")?;
            let panes = app
                .demo
                .workspace
                .floating_window(logical)
                .ok_or("native_document_restore_model")?
                .root
                .panes();
            if panes.len() != 1 {
                return Err("native_document_restore_setup: one source pane required".into());
            }
            let pane = app
                .demo
                .workspace
                .pane(panes[0])
                .ok_or("native_document_restore_pane")?;
            let item = pane
                .active
                .filter(|item| {
                    pane.items.len() == 1
                        && app
                            .demo
                            .workspace
                            .item(*item)
                            .is_some_and(|item| item.panel_type.as_str() == "gallery.documents")
                })
                .ok_or("native_document_restore_setup: one active document required")?;
            eprintln!(
                "native_document_restore press source={} sequence={} item={} target={} capture=true",
                window.0,
                sequence,
                item.0,
                state.target.element.get()
            );
            self.pending = Some(Pending {
                source: window,
                native,
                logical,
                item,
                before,
                values: Self::values(app),
                after: None,
                released: None,
            });
            return Ok(false);
        }
        let Some(pending) = &mut self.pending else {
            return Ok(false);
        };
        match event {
            Event::Press => return Err("native_document_restore_interrupted: new press".into()),
            Event::Move if window == pending.source && pending.after.is_none() => {
                let state = State::read(app, window).ok_or("native_document_restore_source")?;
                if !state.dragging || state.capture != Some((state.target, PointerButton::Primary))
                {
                    return Err(
                        "native_document_restore_capture: old gesture lost its owner".into(),
                    );
                }
                if !state.noncollapsed {
                    return Ok(false);
                }
                eprintln!(
                    "native_document_restore motion source={} sequence={} selection_noncollapsed=true capture=true normal_action=RestoreFloating fixture_cancel=false fixture_rebuild=false",
                    window.0, sequence
                );
                app.apply_demo_action(DemoAction::RestoreFloating(pending.logical));
                app.sync_native_windows(event_loop);
                pending.after = Some(app.demo.workspace.clone());
                Self::verify_model(app, pending)?;
                eprintln!(
                    "native_document_restore retired source={} sequence={} item={} container=Main old_host=false old_route=false",
                    window.0, sequence, pending.item.0
                );
            }
            Event::Release => {
                if pending.after.is_none() {
                    return Err(
                        "native_document_restore_order: release preceded restoration".into(),
                    );
                }
                if pending.released.replace((window, sequence)).is_some() {
                    return Err("native_document_restore_release: duplicate native release".into());
                }
                eprintln!(
                    "native_document_restore release source={} release_window={} sequence={} foreign_release={}",
                    pending.source.0,
                    window.0,
                    sequence,
                    pending.source != window
                );
            }
            _ => {}
        }
        let Some((released, release_sequence)) = pending.released else {
            return Ok(false);
        };
        Self::verify_model(app, pending)?;
        let Some(main) = State::read(app, STANDALONE_WINDOW) else {
            return Ok(false); // The ordinary requested redraw must publish the returned page.
        };
        if main.target.window != STANDALONE_WINDOW
            || main.target.element == pending.before.target.element
            || main.selection.is_some()
            || main.dragging
            || main.capture.is_some()
            || main.scroll_pending
            || main.binding == pending.before.binding
        {
            return Err(format!(
                "native_document_restore_replacement: old gesture reached new page: {main:?}"
            ));
        }
        eprintln!(
            "native_document_restore passed source={} release_window={} release_sequence={} observed_sequence={} item={} container=Main old_host=false old_route=false workspace_preserved=true values_preserved=true replacement_selection=false gesture=false capture=false scroll_deadline=false fixture_cancel=false fixture_rebuild=false",
            pending.source.0, released.0, release_sequence, sequence, pending.item.0
        );
        Ok(true)
    }

    fn values(app: &App) -> String {
        format!("{:?}/{:?}", app.demo.slider, app.demo.number)
    }

    fn verify_model(app: &App, pending: &Pending) -> Result<(), String> {
        if pending.after.as_ref() != Some(&app.demo.workspace)
            || Self::values(app) != pending.values
            || app.demo.workspace.container_of_item(pending.item) != Some(ContainerRef::Main)
            || app
                .demo
                .workspace
                .floating_window(pending.logical)
                .is_some()
            || app
                .native_windows
                .values()
                .any(|host| host.ui_window == pending.source)
            || app.fixture_input(pending.source).is_some()
            || app.window_actions(pending.source).is_some()
            || app.input_ownership.capture().is_some()
        {
            return Err(
                "native_document_restore_late: retired source or late mutation survived".into(),
            );
        }
        Ok(())
    }
}
