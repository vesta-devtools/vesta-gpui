//! Real slider capture followed by normal source or focus ownership transitions.
use super::*;

#[derive(Clone, Copy, Debug)]
enum Mutation {
    Remove,
    Reset,
    Hide,
    Disable,
    Focus,
    Modal,
}

struct Pending {
    source: UiWindowId,
    target: ElementId,
    initial: f32,
    after: WorkspaceModel,
    released: Option<UiWindowId>,
}

pub(super) struct Probe {
    source: Source,
    mutation: Mutation,
    pending: Option<Pending>,
}

impl Probe {
    pub(super) fn inspection_grace(&self) -> Duration {
        Duration::from_secs(
            if matches!(
                self.mutation,
                Mutation::Disable | Mutation::Focus | Mutation::Modal
            ) {
                90
            } else {
                30
            },
        )
    }

    pub(super) fn from_case(case: &str, source: Source) -> Option<Self> {
        let mutation = match case {
            "main-remove-slider" | "float-remove-slider" => Mutation::Remove,
            "main-reset-slider" | "float-reset-slider" => Mutation::Reset,
            "main-hide-slider" | "float-hide-slider" => Mutation::Hide,
            "main-disable-slider" | "float-disable-slider" => Mutation::Disable,
            "main-focus-slider" | "float-focus-slider" => Mutation::Focus,
            "main-modal-slider" | "float-modal-slider" => Mutation::Modal,
            _ => return None,
        };
        Some(Self {
            source,
            mutation,
            pending: None,
        })
    }

    pub(super) fn observe(
        &mut self,
        app: &mut App,
        window: UiWindowId,
        event: Event,
        sequence: u64,
    ) -> Result<bool, String> {
        if matches!(event, Event::Press) {
            if self.pending.is_some() {
                return Err("native_value_loss_interrupted: another press preceded release".into());
            }
            if (self.source == Source::Main) != (window == STANDALONE_WINDOW) {
                return Ok(false);
            }
            let Some((initial, capture)) = app.fixture_slider_capture(window) else {
                return Ok(false);
            };
            if initial.to_bits() == app.demo.slider.to_bits() {
                return Ok(false);
            }
            if app
                .window_actions(window)
                .and_then(|route| route.action_at(capture.target.element))
                != Some(&DemoAction::SetSlider)
            {
                return Err(
                    "native_value_loss_target: captured owner is not the live slider".into(),
                );
            }
            let container = app
                .window_container(window)
                .ok_or("native_value_loss_source: missing container")?;
            let item = container_panes(&app.demo.workspace, container)
                .into_iter()
                .filter_map(|pane| app.demo.workspace.pane(pane))
                .flat_map(|pane| pane.items.iter())
                .find(|item| item.panel_type.as_str() == "gallery.controls")
                .map(|item| item.id)
                .ok_or("native_value_loss_source: controls page missing")?;
            eprintln!(
                "native_value_loss_fixture press window={} sequence={} target={} item={} initial={} preview={} capture=true mutation={:?}",
                window.0,
                sequence,
                capture.target.element.get(),
                item.0,
                initial,
                app.demo.slider,
                self.mutation,
            );
            match self.mutation {
                Mutation::Remove => app.apply_window_action(
                    window,
                    DemoAction::Workspace(WorkspaceViewAction::CloseTab(item)),
                ),
                Mutation::Reset => app.apply_window_action(window, DemoAction::WorkspaceReset),
                Mutation::Hide => app.apply_window_action(
                    window,
                    DemoAction::ToggleGallerySection(GallerySectionId::Feedback),
                ),
                Mutation::Disable => {
                    app.apply_window_action(window, DemoAction::ToggleSliderEnabled)
                }
                Mutation::Focus => {
                    let scope = app
                        .gallery_scopes(window)
                        .ok_or("native_value_loss_source: focus window missing")?;
                    app.set_window_focus(window, Some(slider_demo::enable_target(scope)));
                }
                Mutation::Modal => app.apply_window_action(window, DemoAction::OpenDemoDialog),
            }
            let item_present = app.demo.workspace.item(item).is_some();
            if item_present == matches!(self.mutation, Mutation::Remove) {
                return Err(
                    "native_value_loss_mutation: source removal or Reset identity reuse failed"
                        .into(),
                );
            }
            if matches!(self.mutation, Mutation::Hide)
                && !app
                    .demo
                    .collapsed_gallery_sections
                    .contains(&GallerySectionId::Feedback)
            {
                return Err("native_value_loss_mutation: slider section was not hidden".into());
            }
            if matches!(self.mutation, Mutation::Disable) && app.demo.slider_enabled {
                return Err("native_value_loss_mutation: slider was not disabled".into());
            }
            self.pending = Some(Pending {
                source: window,
                target: capture.target.element,
                initial,
                after: app.demo.workspace.clone(),
                released: None,
            });
            app.request_all_redraw();
            eprintln!(
                "native_value_loss_fixture mutated window={} sequence={} item={} item_present={} mutation={:?} fixture_cancel=false fixture_rebuild=false",
                window.0, sequence, item.0, item_present, self.mutation,
            );
            return Ok(false);
        }
        let Some(pending) = &mut self.pending else {
            return Ok(false);
        };
        if matches!(event, Event::Release) {
            if pending.released.replace(window).is_some() {
                return Err("native_value_loss_release: multiple late releases".into());
            }
            eprintln!(
                "native_value_loss_fixture release source={} window={} sequence={} value={}",
                pending.source.0, window.0, sequence, app.demo.slider,
            );
        }
        if app.demo.workspace != pending.after {
            return Err(
                "native_value_loss_workspace: late input changed the replacement Workspace".into(),
            );
        }
        let Some(released) = pending.released else {
            return Ok(false);
        };
        if app.demo.slider.to_bits() != pending.initial.to_bits() {
            return Err(
                "native_value_loss_commit: late release committed a retired slider preview".into(),
            );
        }
        let editing = if pending.source == STANDALONE_WINDOW {
            app.slider_drag.is_some()
        } else {
            app.native_windows
                .values()
                .any(|host| host.ui_window == pending.source && host.slider_drag.is_some())
        };
        if editing
            || app
                .fixture_input(pending.source)
                .is_some_and(|input| input.capture().is_some())
            || app
                .fixture_input(released)
                .is_some_and(|input| input.capture().is_some())
        {
            return Ok(false);
        }
        if matches!(self.mutation, Mutation::Focus | Mutation::Modal) {
            let scopes = app
                .window_scopes(pending.source)
                .ok_or("native_value_loss_source: focus destination window missing")?;
            let expected = if matches!(self.mutation, Mutation::Modal) {
                if app.demo.dialog_stack.top() != Some(1)
                    || app.overlay_owner != Some(pending.source)
                {
                    return Err(
                        "native_value_loss_modal: late input changed the source modal".into(),
                    );
                }
                dialog_element(scopes.root, 1, 5)
            } else {
                slider_demo::enable_target(scopes.gallery)
            };
            // Require ordinary publication; the fixture cannot rebuild the route.
            if app
                .window_actions(pending.source)
                .and_then(|route| route.action_at(expected))
                .is_none()
            {
                return Ok(false);
            }
            if app
                .fixture_input(pending.source)
                .and_then(InputOwnership::focus)
                != Some(InputTarget {
                    window: pending.source,
                    element: expected,
                })
            {
                return Err("native_value_loss_focus: late input changed replacement focus".into());
            }
            eprintln!(
                "native_value_loss_fixture replacement source={} sequence={} target={} focus=true modal={} fixture_cancel=false fixture_rebuild=false",
                pending.source.0,
                sequence,
                expected.get(),
                matches!(self.mutation, Mutation::Modal),
            );
        }
        if matches!(self.mutation, Mutation::Hide) {
            let scope = app
                .gallery_scopes(pending.source)
                .ok_or("native_value_loss_source: hidden source window missing")?;
            let route = app
                .window_actions(pending.source)
                .ok_or("native_value_loss_source: hidden source route missing")?;
            // Wait for the ordinary redraw; the fixture cannot rebuild or focus it.
            if route.action_at(pending.target).is_some() {
                return Ok(false);
            }
            if app
                .fixture_input(pending.source)
                .and_then(InputOwnership::focus)
                != Some(InputTarget {
                    window: pending.source,
                    element: GallerySectionId::Feedback.element(scope, 1),
                })
            {
                return Err("native_value_loss_focus: hidden control did not return to its own section header".into());
            }
            eprintln!(
                "native_value_loss_fixture hidden source={} sequence={} action=false header_focus=true fixture_focus=false",
                pending.source.0, sequence
            );
        }
        if matches!(self.mutation, Mutation::Disable) {
            let scope = app
                .gallery_scopes(pending.source)
                .ok_or("native_value_loss_source: disabled source window missing")?;
            let route = app
                .window_actions(pending.source)
                .ok_or("native_value_loss_source: disabled source route missing")?;
            if route.action_at(pending.target).is_some() {
                return Ok(false);
            }
            if app.demo.slider_enabled
                || app
                    .fixture_input(pending.source)
                    .and_then(InputOwnership::focus)
                    != Some(InputTarget {
                        window: pending.source,
                        element: slider_demo::enable_target(scope),
                    })
            {
                return Err("native_value_loss_focus: disabled slider did not return to its live enable control".into());
            }
            eprintln!(
                "native_value_loss_fixture disabled source={} sequence={} action=false enable_focus=true fixture_focus=false",
                pending.source.0, sequence
            );
        }
        eprintln!(
            "native_value_loss_fixture passed source={} release_window={} foreign_release={} sequence={} target={} initial={} value={} mutation={:?} edit=false capture=false workspace_preserved=true late_commit=false",
            pending.source.0,
            released.0,
            pending.source != released,
            sequence,
            pending.target.get(),
            pending.initial,
            app.demo.slider,
            self.mutation,
        );
        Ok(true)
    }
}
