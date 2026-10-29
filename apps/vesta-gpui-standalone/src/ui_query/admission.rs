//! Exhaustive interaction classification over the current Gallery Actions.
use super::*;
impl App {
    pub(crate) fn command_input_allowed(&mut self, id: WindowId, event: &WindowEvent) -> bool {
        if self.ui_query.input_origin != vesta_gpui_winit::UiInputOrigin::Command {
            return true;
        }
        let (window, hit) = if self.window.as_ref().is_some_and(|window| window.id() == id) {
            (STANDALONE_WINDOW, self.hit_test.hit(self.cursor))
        } else if let Some(host) = self.native_windows.get(&id) {
            (host.ui_window, host.hit_test.hit(host.cursor))
        } else {
            return true;
        };
        let keyboard = matches!(
            event,
            WindowEvent::KeyboardInput { .. } | WindowEvent::Ime(_) | WindowEvent::Text(_)
        );
        if matches!(event,WindowEvent::KeyboardInput{event:key,..} if key.logical_key==Key::Named(NamedKey::F5))
            && self.display_blocked(id)
        {
            return true;
        }
        if keyboard && self.text_session_field(window) == Some(TextFieldKind::Password) {
            self.ui_query.input_error = Some("sensitive_target");
            return false;
        }
        let target = if keyboard {
            self.window_focus(window)
        } else if matches!(
            event,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                ..
            }
        ) {
            hit
        } else {
            None
        };
        if let Some(action) = target
            .and_then(|target| self.window_actions(window)?.action_for_hit(target))
            .copied()
            && let ("denied", Some(code)) = action_kind(action)
        {
            self.ui_query.input_error = Some(code);
            return false;
        }
        true
    }
}
pub(crate) fn action_kind(action: DemoAction) -> (&'static str, Option<&'static str>) {
    match action {
        DemoAction::Noop => ("none", Some("no_action")),
        DemoAction::FocusPassword
        | DemoAction::TogglePasswordReveal
        | DemoAction::ClearPassword => ("denied", Some("sensitive_target")),
        DemoAction::SetSlider
        | DemoAction::Workspace(
            WorkspaceViewAction::BeginResize { .. } | WorkspaceViewAction::BeginMoveFloating(_),
        ) => ("gesture", None),
        DemoAction::SelectMode(..)
        | DemoAction::ToggleTheme
        | DemoAction::ToggleLocale
        | DemoAction::WindowMinimize
        | DemoAction::WindowMaximize
        | DemoAction::WindowClose
        | DemoAction::ToggleSwitch
        | DemoAction::ToggleCompactButton
        | DemoAction::CancelBusy
        | DemoAction::RestartBusy
        | DemoAction::ToggleDrawer
        | DemoAction::SelectDrawerTab(..)
        | DemoAction::CycleCheckbox
        | DemoAction::SelectRadio(..)
        | DemoAction::SetSliderValue(..)
        | DemoAction::ToggleSliderEnabled
        | DemoAction::StepNumber(..)
        | DemoAction::StepVector(..)
        | DemoAction::BeginKeyCapture
        | DemoAction::ShowToast
        | DemoAction::DismissToast(..)
        | DemoAction::ToastDetails(..)
        | DemoAction::SortTable(..)
        | DemoAction::SelectTable(..)
        | DemoAction::SelectTableCell(..)
        | DemoAction::SelectVirtualTable(..)
        | DemoAction::CollectionData(..)
        | DemoAction::TreeData(..)
        | DemoAction::SelectTile(..)
        | DemoAction::RampSelect(..)
        | DemoAction::RampSetPosition(..)
        | DemoAction::RampAdd
        | DemoAction::RampStep(..)
        | DemoAction::RampRemove
        | DemoAction::RampUndo
        | DemoAction::RampRedo
        | DemoAction::RampCycleInterpolation
        | DemoAction::RampEditColor
        | DemoAction::ToggleStatusOverflow
        | DemoAction::FocusSearch
        | DemoAction::ClearSearch
        | DemoAction::ChooseSuggestion(..)
        | DemoAction::OpenDemoDialog
        | DemoAction::OpenNestedDialog
        | DemoAction::ConfirmDialog(..)
        | DemoAction::CancelDialog(..)
        | DemoAction::BeginInlineEdit
        | DemoAction::CommitInlineEdit
        | DemoAction::CancelInlineEdit
        | DemoAction::OpenCommandPalette
        | DemoAction::ExecutePalette(..)
        | DemoAction::ToggleBreadcrumbOverflow
        | DemoAction::BreadcrumbAncestor(..)
        | DemoAction::BreadcrumbNext
        | DemoAction::ToggleToolbarOverflow
        | DemoAction::ToggleHorizontalFlow
        | DemoAction::ToggleColorPicker
        | DemoAction::SelectColorMode(..)
        | DemoAction::StepColor(..)
        | DemoAction::PickColor
        | DemoAction::ApplyColor
        | DemoAction::CancelColor
        | DemoAction::ToggleCombo
        | DemoAction::SelectCombo(..)
        | DemoAction::TogglePopover
        | DemoAction::PopoverStep
        | DemoAction::ToggleMenu
        | DemoAction::ToggleApplicationMenu
        | DemoAction::OpenMenu(..)
        | DemoAction::OpenSubmenu(..)
        | DemoAction::MenuCommand(..)
        | DemoAction::SelectTab(..)
        | DemoAction::SelectList(..)
        | DemoAction::SelectVirtual(..)
        | DemoAction::SelectVirtualTree(..)
        | DemoAction::ToggleVirtualTree(..)
        | DemoAction::ToggleTree
        | DemoAction::ToggleAccordion(..)
        | DemoAction::ToggleGallerySection(..)
        | DemoAction::ShowDocuments
        | DemoAction::Document(..)
        | DemoAction::WorkspaceMove
        | DemoAction::WorkspaceSplit
        | DemoAction::WorkspaceFloat
        | DemoAction::TearOffTab(..)
        | DemoAction::RestoreFloating(..)
        | DemoAction::WorkspaceReset
        | DemoAction::ViewportTool(..)
        | DemoAction::ViewportFrame
        | DemoAction::ViewportProjection
        | DemoAction::Workspace(_) => ("semantic", None),
    }
}
