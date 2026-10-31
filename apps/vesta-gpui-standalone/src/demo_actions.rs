//! Gallery intents used by native and command dispatch.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum DemoAction {
    Noop,
    SelectMode(AcceptanceMode),
    ToggleTheme,
    ToggleLocale,
    WindowMinimize,
    WindowMaximize,
    WindowClose,
    ToggleSwitch,
    ToggleCompactButton,
    CancelBusy,
    RestartBusy,
    ToggleDrawer,
    SelectDrawerTab(u8),
    CycleCheckbox,
    SelectRadio(u8),
    SetSlider,
    SetSliderValue(f32),
    ToggleSliderEnabled,
    StepNumber(i32),
    StepVector(u8, i8),
    BeginKeyCapture,
    ShowToast,
    DismissToast(ToastId),
    ToastDetails(ToastId),
    SortTable(u8),
    SelectTable(u8),
    SelectTableCell(u8, u8),
    SelectVirtualTable(usize),
    CollectionData(CollectionDataAction),
    TreeData(tree_collections::TreeDataAction),
    SelectTile(usize),
    RampSelect(u32),
    RampSetPosition(u32, f32),
    RampAdd,
    RampStep(i8),
    RampRemove,
    RampUndo,
    RampRedo,
    RampCycleInterpolation,
    RampEditColor,
    ToggleStatusOverflow,
    FocusSearch,
    ClearSearch,
    ChooseSuggestion(vesta_gpui_widgets::SuggestionRequestId, u16),
    OpenDemoDialog,
    OpenNestedDialog,
    ConfirmDialog(u8),
    CancelDialog(u8),
    BeginInlineEdit,
    CommitInlineEdit,
    CancelInlineEdit,
    OpenCommandPalette,
    ExecutePalette(u8),
    ToggleBreadcrumbOverflow,
    BreadcrumbAncestor(u8),
    BreadcrumbNext,
    ToggleToolbarOverflow,
    ToggleHorizontalFlow,
    ToggleColorPicker,
    SelectColorMode(ColorPickerMode),
    StepColor(u8, i8),
    PickColor,
    ApplyColor,
    CancelColor,
    FocusPassword,
    TogglePasswordReveal,
    ClearPassword,
    ToggleCombo,
    SelectCombo(u8),
    TogglePopover,
    PopoverStep,
    ToggleMenu,
    ToggleApplicationMenu,
    OpenMenu(MenuId),
    OpenSubmenu(MenuId),
    MenuCommand(MenuItemId),
    SelectTab(u8),
    SelectList(u8),
    SelectVirtual(usize),
    SelectVirtualTree(usize),
    ToggleVirtualTree(usize),
    ToggleTree,
    ToggleAccordion(u8),
    ToggleGallerySection(GallerySectionId),
    ShowDocuments,
    Document(gallery::documents::DocumentAction),
    Workspace(WorkspaceViewAction),
    WorkspaceMove,
    WorkspaceSplit,
    WorkspaceFloat,
    TearOffTab(ItemId),
    RestoreFloating(FloatingWindowId),
    WorkspaceReset,
    ViewportTool(TransformMode),
    ViewportResetCamera,
    ViewportProjection,
}

impl Default for DemoAction {
    fn default() -> Self {
        Self::Noop
    }
}

impl DemoAction {
    pub(super) fn starts_pointer_drag(self) -> bool {
        matches!(
            self,
            Self::SetSlider
                | Self::Workspace(WorkspaceViewAction::Tab(_))
                | Self::Workspace(WorkspaceViewAction::BeginResize { .. })
                | Self::Workspace(WorkspaceViewAction::BeginMoveFloating(_))
        )
    }
}

impl App {
    pub(super) fn apply_demo_action(&mut self, action: DemoAction) {
        if self.ui_query.input_origin == vesta_gpui_winit::UiInputOrigin::Command
            && let ("denied", Some(code)) = ui_query::admission::action_kind(action)
        {
            self.ui_query.input_error = Some(code);
            return;
        }
        if !self.accepts_color_action(action) {
            return;
        }
        if !self.prepare_window_menu_action(self.action_window, action) {
            return;
        }
        self.prepare_ramp_action(action);
        if action != DemoAction::ToggleToolbarOverflow {
            self.close_toolbar_overflow(self.action_window);
        }
        if self.demo.status_overflow_open && action != DemoAction::ToggleStatusOverflow {
            self.demo.status_overflow_open = false;
            if self.input_ownership.focus().is_some_and(|focus| {
                self.ui_actions
                    .is_descendant_of(focus.element, self.scopes.status.element(40))
            }) {
                self.set_focus_target(Some(self.scopes.status.element(30)));
            }
        }
        match action {
            DemoAction::Noop => {}
            DemoAction::SelectMode(mode) => {
                if mode == AcceptanceMode::Viewport3d && !self.demo.viewport3d_available {
                    eprintln!(
                        "native_vulkan_3d_unsupported: 3D preview is unavailable in the native 2D renderer"
                    );
                    let id = ToastId(self.demo.next_toast_id);
                    self.demo.next_toast_id = self.demo.next_toast_id.saturating_add(1);
                    let now = Instant::now();
                    self.push_toast(
                        ToastNotice {
                            id,
                            message: "3D Viewport 尚未接入原生 Vulkan；当前后端仅支持 2D。".into(),
                            tone: StatusTone::Information,
                            action_label: None,
                            action: None,
                            expires_at: now + Duration::from_secs(6),
                        },
                        now,
                    );
                    self.request_all_redraw();
                    return;
                }
                if mode != self.mode {
                    self.cancel_transform_drag();
                    self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
                    self.close_popover_for(self.action_window, PopupCloseReason::FocusTransfer);
                    self.close_combo_for(self.action_window, PopupCloseReason::FocusTransfer);
                }
                self.mode = mode;
                if self.focused_text_field().is_some() {
                    self.set_text_focus(None);
                }
            }
            DemoAction::ToggleTheme => {
                self.theme = self.theme.toggled();
                self.request_all_redraw();
            }
            DemoAction::ToggleLocale => {
                self.demo.locale = self.demo.locale.toggled();
                if let Some(window) = &self.window {
                    window.set_title(self.demo.locale.window_title());
                }
                for host in self.native_windows.values() {
                    if let Some(logical) = self.demo.workspace.floating_window(host.logical_id) {
                        host.window.set_title(&format!(
                            "{} — Vesta",
                            display_text(&logical.title, self.demo.locale)
                        ));
                    }
                }
                self.request_all_redraw();
            }
            DemoAction::WindowMinimize => {
                if let Some(window) = &self.window {
                    window.set_minimized(true);
                }
            }
            DemoAction::WindowMaximize => {
                if let Some(window) = &self.window {
                    window.set_maximized(!window.is_maximized());
                }
            }
            DemoAction::WindowClose => self.exit_requested = true,
            DemoAction::ToggleSwitch | DemoAction::SelectRadio(_) => {
                self.apply_conditional_control_action(action);
            }
            DemoAction::ToggleCompactButton => self.demo.compact_toggle = !self.demo.compact_toggle,
            DemoAction::CancelBusy => self.demo.busy = false,
            DemoAction::RestartBusy => {
                self.demo.busy = true;
                self.next_busy_frame = Instant::now() + Duration::from_millis(80);
            }
            DemoAction::ToggleDrawer => {
                if self.demo.drawer_open {
                    self.cancel_drawer_resize();
                }
                if self.demo.drawer_open
                    && self.input_ownership.focus().is_some_and(|focus| {
                        focus.element == self.scopes.gallery.element(DRAWER_RESIZE)
                            || self.ui_actions.is_descendant_of(
                                focus.element,
                                self.scopes.gallery.element(DRAWER_BODY),
                            )
                    })
                {
                    self.set_focus_target(Some(self.scopes.gallery.element(DRAWER_TOGGLE)));
                }
                self.demo.drawer_open = !self.demo.drawer_open;
            }
            DemoAction::SelectDrawerTab(tab) => {
                if tab < 2 {
                    self.demo.drawer_tab = tab;
                }
            }
            DemoAction::CycleCheckbox => {
                self.demo.checkbox = match self.demo.checkbox {
                    BinaryState::Off => BinaryState::On,
                    BinaryState::On => BinaryState::Mixed,
                    BinaryState::Mixed => BinaryState::Off,
                }
            }
            DemoAction::ShowDocuments => self.show_documents(),
            DemoAction::Document(action) => self.apply_document_action(self.action_window, action),
            DemoAction::SetSlider => self.begin_slider_edit(),
            DemoAction::SetSliderValue(value) => self.set_slider_value(value),
            DemoAction::ToggleSliderEnabled => self.toggle_slider_enabled(),
            DemoAction::StepNumber(delta) => {
                self.demo.number = self
                    .demo
                    .number
                    .saturating_add(delta)
                    .clamp(NUMBER_MIN, NUMBER_MAX);
                self.reset_number_draft();
            }
            DemoAction::StepVector(axis, delta) => {
                if let Some(value) = self.demo.vector.get_mut(usize::from(axis)) {
                    *value = Some(
                        (value.unwrap_or(0.0) + f32::from(delta)).clamp(VECTOR_MIN, VECTOR_MAX),
                    );
                    self.reset_vector_draft(axis);
                }
            }
            DemoAction::BeginKeyCapture => {
                if let Some(opener) = self.editor_target(KEY_CHORD_TRIGGER) {
                    self.demo.key_capture.begin(opener);
                    self.set_window_focus(opener.window, Some(opener.element));
                }
            }
            DemoAction::ShowToast => {
                let id = ToastId(self.demo.next_toast_id);
                self.demo.next_toast_id = self.demo.next_toast_id.saturating_add(1);
                let now = Instant::now();
                self.push_toast(
                    ToastNotice {
                        id,
                        message: format!("Sample notice {}", id.0),
                        tone: StatusTone::Information,
                        action_label: Some("Increment".to_owned()),
                        action: Some(DemoAction::ToastDetails(id)),
                        expires_at: now + Duration::from_secs(4),
                    },
                    now,
                );
            }
            DemoAction::DismissToast(id) => {
                self.expire_toasts(Instant::now());
                if self.demo.toast_queue.dismiss(id) {
                    self.restore_toast_focus(id, self.action_window);
                    self.toast_focus.remove(&id);
                }
                self.request_all_redraw();
            }
            DemoAction::ToastDetails(id) => {
                self.expire_toasts(Instant::now());
                if self.demo.toast_queue.current().map(|notice| notice.id) == Some(id) {
                    self.demo.number = self.demo.number.saturating_add(1);
                    self.reset_number_draft();
                    self.demo.toast_queue.dismiss(id);
                    self.restore_toast_focus(id, self.action_window);
                    self.toast_focus.remove(&id);
                    self.request_all_redraw();
                }
            }
            DemoAction::SortTable(column) => {
                if column < 3 {
                    if self.demo.table_sort_column == column {
                        self.demo.table_sort_descending = !self.demo.table_sort_descending;
                    } else {
                        self.demo.table_sort_column = column;
                        self.demo.table_sort_descending = false;
                    }
                }
            }
            DemoAction::SelectTable(id) => {
                if (1..=3).contains(&id) {
                    self.demo.table_selected = id;
                    self.demo.table_selected_cell = None;
                }
            }
            DemoAction::SelectTableCell(row, column) => {
                if (1..=3).contains(&row) && column < 3 {
                    self.demo.table_selected = row;
                    self.demo.table_selected_cell = Some((row, column));
                }
            }
            DemoAction::SelectTile(id) => {
                if id < 9 {
                    self.demo.tile_selected = id;
                }
            }
            DemoAction::RampSelect(id) => self.demo.ramp.select(id),
            DemoAction::RampSetPosition(id, position) => {
                self.demo.ramp.set_position(id, position);
            }
            DemoAction::RampAdd => {
                self.cancel_ramp_color();
                self.demo.ramp.add();
            }
            DemoAction::RampStep(direction) => self.demo.ramp.step(direction),
            DemoAction::RampRemove => {
                self.cancel_ramp_color();
                self.demo.ramp.remove();
            }
            DemoAction::RampUndo => {
                self.cancel_ramp_color();
                self.demo.ramp.undo();
            }
            DemoAction::RampRedo => {
                self.cancel_ramp_color();
                self.demo.ramp.redo();
            }
            DemoAction::RampCycleInterpolation => self.demo.ramp.cycle_interpolation(),
            DemoAction::RampEditColor => self.open_ramp_color(),
            DemoAction::ToggleStatusOverflow => {
                self.demo.status_overflow_open = !self.demo.status_overflow_open;
            }
            DemoAction::FocusSearch => {
                self.demo.suggestion_dismissed = None;
                self.begin_text_selection(self.action_window, TextFieldKind::Search, None)
            }
            DemoAction::ClearSearch => {
                self.demo.suggestion_dismissed = None;
                let _ = self.search_state.reset(&mut self.text_context, "");
                self.begin_text_selection(self.action_window, TextFieldKind::Search, None);
            }
            DemoAction::ChooseSuggestion(ticket, id) => self.choose_suggestion(ticket, id),
            DemoAction::OpenDemoDialog => self.open_demo_dialog(1),
            DemoAction::OpenNestedDialog => {
                if self.demo.dialog_stack.top() == Some(1) {
                    self.open_demo_dialog(2);
                }
            }
            DemoAction::ConfirmDialog(id) => self.close_demo_dialog(id, true),
            DemoAction::CancelDialog(id) => self.close_demo_dialog(id, false),
            DemoAction::BeginInlineEdit => self.begin_inline_edit(),
            DemoAction::CommitInlineEdit => self.finish_inline_edit(true),
            DemoAction::CancelInlineEdit => self.finish_inline_edit(false),
            DemoAction::OpenCommandPalette => self.open_command_palette(),
            DemoAction::ExecutePalette(id) => self.execute_palette_command(id),
            DemoAction::ToggleCombo => self.toggle_combo(),
            DemoAction::SelectCombo(value) => {
                if self
                    .demo
                    .combo
                    .opener()
                    .is_some_and(|opener| opener.window == self.action_window)
                {
                    self.demo.combo_selected = value.min(2);
                    self.close_combo_for(self.action_window, PopupCloseReason::Explicit);
                }
            }
            DemoAction::TogglePopover => self.toggle_popover(),
            DemoAction::PopoverStep => {
                if self
                    .demo
                    .popover
                    .opener()
                    .is_some_and(|opener| opener.window == self.action_window)
                {
                    self.demo.number = self.demo.number.saturating_add(1);
                    self.reset_number_draft();
                    self.close_popover_for(self.action_window, PopupCloseReason::Explicit);
                }
            }
            DemoAction::ToggleMenu => {
                if self.demo.menu.is_open() {
                    self.close_window_menu(self.action_window, PopupCloseReason::Explicit);
                } else {
                    let anchor = if self.cursor.y < CHROME_HEIGHT {
                        Point::new(4.0, CHROME_HEIGHT)
                    } else {
                        Point::new(self.cursor.x, self.cursor.y + 8.0)
                    };
                    self.open_menu(anchor);
                }
            }
            DemoAction::ToggleApplicationMenu => {
                if self.demo.menu.is_open() && self.demo.menu_origin == MenuOrigin::ApplicationBar {
                    self.close_window_menu(STANDALONE_WINDOW, PopupCloseReason::Explicit);
                } else {
                    self.open_application_menu(MenuId(0));
                }
            }
            DemoAction::OpenMenu(menu) => {
                if self.demo.menu_origin == MenuOrigin::ApplicationBar
                    && self.demo.menu.path().first() == Some(&menu)
                {
                    self.close_window_menu(self.action_window, PopupCloseReason::Explicit);
                } else {
                    self.open_application_menu(menu);
                }
            }
            DemoAction::OpenSubmenu(menu) => self.enter_window_submenu(self.action_window, menu),
            DemoAction::MenuCommand(item) => {
                self.close_window_menu(self.action_window, PopupCloseReason::Explicit);
                let command = match item.0 {
                    10 => Some(DemoAction::WorkspaceReset),
                    11 => Some(DemoAction::WindowClose),
                    30 => Some(DemoAction::SelectMode(AcceptanceMode::Gallery2d)),
                    31 => Some(DemoAction::SelectMode(AcceptanceMode::Viewport3d)),
                    32 => Some(DemoAction::ToggleTheme),
                    40 => Some(DemoAction::WorkspaceMove),
                    41 => Some(DemoAction::WorkspaceSplit),
                    42 => Some(DemoAction::WorkspaceFloat),
                    43 => Some(DemoAction::WorkspaceReset),
                    _ => None,
                };
                if let Some(command) = command {
                    self.apply_window_action(self.action_window, command);
                }
            }
            DemoAction::SelectTab(value) => self.demo.selected_tab = value,
            DemoAction::ToggleBreadcrumbOverflow => {
                self.demo.breadcrumb_expanded = !self.demo.breadcrumb_expanded;
            }
            DemoAction::BreadcrumbAncestor(depth) => {
                self.demo.breadcrumb_depth = depth.min(3);
                self.demo.breadcrumb_expanded = false;
                self.focus_breadcrumb_navigation(self.action_window);
            }
            DemoAction::BreadcrumbNext => {
                self.demo.breadcrumb_depth = (self.demo.breadcrumb_depth + 1).min(3);
                self.demo.breadcrumb_expanded = false;
                self.focus_breadcrumb_navigation(self.action_window);
            }
            DemoAction::ToggleToolbarOverflow => {
                self.demo.toolbar_overflow_open = !self.demo.toolbar_overflow_open;
            }
            DemoAction::ToggleHorizontalFlow => {
                self.demo.horizontal_flow = match self.demo.horizontal_flow {
                    FlowDirection::LeftToRight => FlowDirection::RightToLeft,
                    FlowDirection::RightToLeft => FlowDirection::LeftToRight,
                };
                self.request_all_redraw();
            }
            DemoAction::ToggleColorPicker => self.toggle_swatch_color(),
            DemoAction::SelectColorMode(mode) => {
                if self.can_edit_color(self.action_window) {
                    self.demo.color_mode = mode;
                }
            }
            DemoAction::StepColor(channel, step) => {
                self.step_color_draft(channel, step);
            }
            DemoAction::PickColor => {
                if self.can_edit_color(self.action_window) {
                    self.demo.color_draft = [222, 126, 110, 255];
                    self.reset_color_hex_draft();
                }
            }
            DemoAction::ApplyColor => self.apply_color_draft(),
            DemoAction::CancelColor => self.cancel_color_draft(),
            DemoAction::FocusPassword => {
                if let Some(target) = self.editor_target(PASSWORD_ROOT) {
                    let pointer = if target.window == STANDALONE_WINDOW {
                        Some(self.cursor)
                    } else {
                        self.native_windows
                            .values()
                            .find(|host| host.ui_window == target.window)
                            .map(|host| host.cursor)
                    };
                    self.begin_text_selection(target.window, TextFieldKind::Password, pointer);
                }
            }
            DemoAction::TogglePasswordReveal => {
                self.demo.password_revealed = !self.demo.password_revealed;
                if let Some(target) = self.editor_target(PASSWORD_ROOT) {
                    self.set_window_focus(target.window, Some(target.element));
                }
            }
            DemoAction::ClearPassword => {
                self.password_state.act(
                    &mut self.text_context,
                    vesta_gpui::TextInputAction::SelectAll,
                );
                let _ = self.password_state.insert(
                    &mut self.text_context,
                    "",
                    vesta_gpui::TextEditKind::Paste,
                );
                if let Some(target) = self.editor_target(PASSWORD_ROOT) {
                    self.set_window_focus(target.window, Some(target.element));
                }
            }
            DemoAction::SelectList(value) => self.demo.selected_list = value,
            DemoAction::SelectVirtual(value) => {
                let modifiers = self.window_modifiers(self.action_window);
                self.demo.virtual_selection.select(
                    value,
                    &self.demo.virtual_ids,
                    SelectionModifiers {
                        additive: modifiers.control_key(),
                        range: modifiers.shift_key(),
                    },
                );
            }
            DemoAction::SelectVirtualTable(index) => {
                if index < TABLE_VIRTUAL_COUNT {
                    self.demo.virtual_table_selected = index;
                }
            }
            DemoAction::CollectionData(action) => self.apply_collection_data(action),
            DemoAction::TreeData(action) => self.apply_tree_data(action),
            DemoAction::SelectVirtualTree(id) => self.select_virtual_tree(id),
            DemoAction::ToggleVirtualTree(id) => self.toggle_virtual_tree(id),
            DemoAction::ToggleTree => self.demo.tree_expanded = !self.demo.tree_expanded,
            DemoAction::ToggleAccordion(index) => {
                self.demo.accordion_open = if self.demo.accordion_open == Some(index) {
                    None
                } else {
                    Some(index)
                };
            }
            DemoAction::ToggleGallerySection(section) => {
                self.toggle_gallery_section(section);
            }
            DemoAction::Workspace(action) => self.apply_workspace_action(action),
            DemoAction::WorkspaceMove => self.move_active_workspace_item(),
            DemoAction::WorkspaceSplit => self.split_active_workspace_item(),
            DemoAction::WorkspaceFloat => self.float_active_workspace_item(),
            DemoAction::TearOffTab(item) => {
                self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
                self.float_workspace_item(item, WindowPlacement::new(90.0, 60.0, 560.0, 420.0));
            }
            DemoAction::RestoreFloating(logical_id) => {
                if let Some(window_id) = self.native_windows.iter().find_map(|(window_id, host)| {
                    (host.logical_id == logical_id).then_some(*window_id)
                }) {
                    self.restore_native_window(window_id);
                }
            }
            DemoAction::WorkspaceReset => self.reset_workspace(),
            DemoAction::ViewportTool(mode) => self.scene.gizmo.mode = mode,
            DemoAction::ViewportResetCamera => {
                apply_action(&mut self.camera, ViewportAction::ResetCamera)
            }
            DemoAction::ViewportProjection => {
                apply_action(&mut self.camera, ViewportAction::ToggleProjection)
            }
        }
    }
}
