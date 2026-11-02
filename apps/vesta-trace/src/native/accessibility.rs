//! Platform projection and shared realized-target/text admission.
use super::*;
use accesskit::{Action, ActionData, ActionRequest};

impl App {
    pub(super) fn publish_accessibility(&mut self, root: &vesta_gpui::Element) {
        let Some(layout) = &self.layout_tree else {
            return;
        };
        let focus = self
            .ownership
            .focus()
            .filter(|_| self.active && !self.occluded);
        let semantics = &mut self.semantics;
        let input = &mut self.search.input;
        let context = &mut self.context;
        let view = self.search.view;
        let mut build = || {
            let mut update = semantics.build(
                root,
                layout,
                focus.map(|t| t.element),
                "Vesta Trace — native frame analysis",
            );
            if let Some(view) = view {
                semantics.enrich_text_input(&mut update, view::SEARCH, input, context, view.origin);
            }
            update
        };
        if matches!(self.scene, Scene::Inspection | Scene::Focus) {
            let update = build();
            self.smoke_semantics = Some(update.clone());
            if let Some(adapter) = &mut self.accessibility {
                adapter.update_if_active(|| update);
            }
        } else if let Some(adapter) = &mut self.accessibility {
            // The adapter requests projection only while an assistive client is
            // active. Ordinary frames do not build an unused semantic tree.
            adapter.update_if_active(build);
        }
    }

    pub(super) fn accessible_action(
        &mut self,
        request: ActionRequest,
    ) -> std::result::Result<bool, TraceError> {
        if self.layout_tree.is_none() {
            return Ok(false);
        }
        let Some(target) = self.route.as_ref().and_then(|route| {
            FocusScope::new(WINDOW, route, None).accessible_target(
                &self.semantics,
                request.target,
                request.action,
            )
        }) else {
            return Ok(false);
        };
        if self.busy()
            && target.element != view::SEARCH
            && target.element != vesta_gpui::ElementId::new(82)
            && target.element != vesta_gpui::ElementId::new(86)
        {
            return Ok(false);
        }
        match (request.action, request.data) {
            (Action::Focus, _) => (),
            (Action::Click, _) => {
                if target.element != view::SEARCH {
                    let Some(command) = self
                        .route
                        .as_ref()
                        .and_then(|r| r.action_at(target.element))
                        .cloned()
                    else {
                        return Ok(false);
                    };
                    self.ownership
                        .observe_focus_input(vesta_gpui::FocusInput::Keyboard);
                    self.set_focus(Some(target));
                    self.command(command)?;
                    return Ok(true);
                }
            }
            (Action::SetValue, Some(ActionData::Value(value)))
                if target.element == view::SEARCH =>
            {
                if value.len() > 256 || self.search.input.is_composing() {
                    return Ok(false);
                }
                self.search
                    .input
                    .replace_accessibility(&mut self.context, &value)
                    .map_err(|e| TraceError::caused("trace_accessibility_text", e))?;
                self.search.due = Some(Instant::now() + Duration::from_millis(180));
            }
            (Action::SetTextSelection, Some(ActionData::SetTextSelection(selection)))
                if target.element == view::SEARCH =>
            {
                let Some(selection) = self.semantics.resolve_text_input_selection(
                    view::SEARCH,
                    &mut self.search.input,
                    &mut self.context,
                    &selection,
                ) else {
                    return Ok(false);
                };
                if self
                    .search
                    .input
                    .apply_accessibility_selection(&mut self.context, selection)
                    .is_none()
                {
                    return Ok(false);
                }
            }
            _ => return Ok(false),
        }
        self.ownership
            .observe_focus_input(vesta_gpui::FocusInput::Keyboard);
        self.set_focus(Some(target));
        self.search.refresh_view();
        self.redraw();
        Ok(true)
    }
}
