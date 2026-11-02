//! Directed native projection/action checks; no OS input or reader activation.
use super::*;
use accesskit::{Action, ActionData, ActionRequest, NodeId};

impl App {
    pub(super) fn check_gpu_keyboard_scroll(&mut self) -> Result<()> {
        let target = self
            .semantics
            .node_for_element(view::GPU_DETAIL)
            .ok_or("trace_gpu_scroll_semantics_missing")?;
        if !self.accessible_action(ActionRequest {
            action: Action::Focus,
            target,
            data: None,
        })? {
            return Err("trace_gpu_scroll_not_focusable".into());
        }
        let generation = self.generation;
        let selected = self
            .timeline
            .as_ref()
            .map(|t| (t.view, t.selected, t.selection));
        for (key, code, bottom) in [
            (NamedKey::End, KeyCode::End, true),
            (NamedKey::Home, KeyCode::Home, false),
            (NamedKey::PageDown, KeyCode::PageDown, true),
            (NamedKey::PageUp, KeyCode::PageUp, false),
            (NamedKey::ArrowDown, KeyCode::ArrowDown, true),
            (NamedKey::ArrowUp, KeyCode::ArrowUp, false),
        ] {
            self.logical_key(Key::Named(key), None, code)?;
            if (self.gpu_scroll.offset_y() > 0.0) != bottom
                || self.generation != generation
                || self
                    .timeline
                    .as_ref()
                    .map(|t| (t.view, t.selected, t.selection))
                    != selected
            {
                return Err("trace_gpu_keyboard_scroll_or_evidence_changed".into());
            }
        }
        println!(
            "trace_accessibility_check command_origin=true gpu_scroll_focus=true keyboard_page_home_end_arrows=true analysis_unchanged=true"
        );
        Ok(())
    }

    pub(super) fn check_accessibility_initial(&mut self) -> Result<()> {
        let update = self
            .smoke_semantics
            .as_ref()
            .ok_or("trace_semantics_missing")?;
        let nodes = update.nodes.len();
        if self.accessibility.is_none()
            || !update
                .nodes
                .iter()
                .any(|(_, n)| n.role() == accesskit::Role::Label)
        {
            return Err("trace_accessibility_host_or_evidence_labels_missing".into());
        }
        let target = self
            .semantics
            .node_for_element(view::SEARCH)
            .ok_or("trace_search_semantics_missing")?;
        let old_focus = self.ownership.focus();
        if self.accessible_action(ActionRequest {
            action: Action::Focus,
            target: NodeId(u64::MAX),
            data: None,
        })? || self.ownership.focus() != old_focus
        {
            return Err("trace_foreign_accessibility_target_accepted".into());
        }
        if !self.accessible_action(ActionRequest {
            action: Action::Focus,
            target,
            data: None,
        })? || !self.search_focused()
        {
            return Err("trace_accessibility_search_focus".into());
        }
        let original = self.search.input.text().to_owned();
        let generation = self.generation;
        let replacement = "布局 e\u{301}";
        if !self.accessible_action(ActionRequest {
            action: Action::SetValue,
            target,
            data: Some(ActionData::Value(replacement.into())),
        })? || self.search.input.text() != replacement
            || self.generation != generation
        {
            return Err("trace_accessibility_replacement_or_debounce".into());
        }
        if !self
            .search
            .input
            .act(&mut self.context, vesta_gpui::TextInputAction::Undo)
            || self.search.input.text() != original
        {
            return Err("trace_accessibility_replacement_not_undoable".into());
        }
        if self.accessible_action(ActionRequest {
            action: Action::SetValue,
            target,
            data: Some(ActionData::Value("x".repeat(257).into())),
        })? || self.search.input.text() != original
        {
            return Err("trace_accessibility_query_budget_bypassed".into());
        }
        self.search.refresh_view();
        self.search.due = None;
        println!(
            "trace_accessibility_check command_origin=true projected_nodes={nodes} adapter_installed=true focus=true foreign_target_rejected=true value_undo=true query_budget=true"
        );
        Ok(())
    }

    pub(super) fn check_accessibility_selection(&mut self) -> Result<()> {
        let target = self
            .semantics
            .node_for_element(view::SEARCH)
            .ok_or("trace_search_semantics_missing")?;
        let node = self
            .smoke_semantics
            .as_ref()
            .and_then(|u| u.nodes.iter().find(|(id, _)| *id == target))
            .map(|(_, n)| n)
            .ok_or("trace_search_semantic_projection_missing")?;
        if node.value() != Some(self.search.input.text()) {
            return Err("trace_search_semantic_value_not_current".into());
        }
        let selection = *node
            .text_selection()
            .ok_or("trace_search_text_runs_missing")?;
        let before = self.search.input.inspection_selection();
        let generation = self.generation;
        if !self.accessible_action(ActionRequest {
            action: Action::SetTextSelection,
            target,
            data: Some(ActionData::SetTextSelection(selection)),
        })? {
            return Err("trace_accessibility_selection_rejected".into());
        }
        let mut foreign = selection;
        foreign.anchor.node = NodeId(u64::MAX);
        if self.accessible_action(ActionRequest {
            action: Action::SetTextSelection,
            target,
            data: Some(ActionData::SetTextSelection(foreign)),
        })? || self.search.input.inspection_selection() != before
            || self.generation != generation
        {
            return Err("trace_accessibility_foreign_selection_or_analysis".into());
        }
        println!(
            "trace_accessibility_check command_origin=true actual_text_runs=true selection_roundtrip=true foreign_selection_rejected=true analysis_unchanged=true"
        );
        Ok(())
    }

    pub(super) fn check_accessibility_activation(&mut self) -> Result<()> {
        let target = self
            .semantics
            .node_for_element(vesta_gpui::ElementId::new(102))
            .ok_or("trace_callers_semantics_missing")?;
        if !self.accessible_action(ActionRequest {
            action: Action::Click,
            target,
            data: None,
        })? || self.panel != Panel::Callers
        {
            return Err("trace_accessibility_activation_failed".into());
        }
        if self.accessible_action(ActionRequest {
            action: Action::Focus,
            target,
            data: None,
        })? {
            return Err("trace_accessibility_invalidated_geometry_accepted".into());
        }
        println!(
            "trace_accessibility_check command_origin=true click_shared_command=true invalidated_target_rejected=true"
        );
        Ok(())
    }

    pub(super) fn check_pan_focus_loss(&mut self) -> Result<()> {
        let before = self.timeline.as_ref().map(|t| (t.view, t.selection));
        let generation = self.generation;
        let active = self.active;
        let scale = self
            .window
            .as_ref()
            .ok_or("trace_window_missing")?
            .scale_factor();
        let point = |fraction: f32| Event::CursorMoved {
            device_id: DeviceId::dummy(),
            position: winit::dpi::PhysicalPosition::new(
                f64::from(self.area.x + self.area.width * fraction) * scale,
                f64::from(self.area.y + self.area.height * 0.8) * scale,
            ),
        };
        let start = point(0.3);
        let end = point(0.6);
        self.input(&start, UiInputOrigin::Command)?;
        self.input(
            &Event::MouseInput {
                device_id: DeviceId::dummy(),
                state: ElementState::Pressed,
                button: MouseButton::Right,
            },
            UiInputOrigin::Command,
        )?;
        self.input(&end, UiInputOrigin::Command)?;
        if self.ownership.capture().is_none() {
            return Err("trace_pan_capture_missing".into());
        }
        self.input(&Event::Focused(false), UiInputOrigin::Command)?;
        self.input(
            &Event::MouseInput {
                device_id: DeviceId::dummy(),
                state: ElementState::Released,
                button: MouseButton::Right,
            },
            UiInputOrigin::Command,
        )?;
        if self.ownership.capture().is_some()
            || self.drag.is_some()
            || self.timeline.as_ref().map(|t| (t.view, t.selection)) != before
            || self.generation != generation
        {
            return Err("trace_pan_focus_loss_retained_capture_or_changed_evidence".into());
        }
        self.input(&Event::Focused(active), UiInputOrigin::Command)?;
        println!(
            "trace_navigation_check command_origin=true secondary_capture_focus_loss=true late_release_no_commit=true analysis_unchanged=true"
        );
        Ok(())
    }
}
