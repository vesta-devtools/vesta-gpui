//! Host catalog, borrowed text session and window authority for the command center.
use super::*;
use vesta_gpui_widgets::{
    CommandPaletteScrollIds, command_palette_geometry, command_palette_matches,
    command_palette_scrolled,
};

pub(super) const SHORTCUT_COMMANDS: [(char, DemoAction); 5] = [
    ('1', DemoAction::SelectMode(AcceptanceMode::Gallery2d)),
    ('2', DemoAction::SelectMode(AcceptanceMode::Viewport3d)),
    ('t', DemoAction::ToggleTheme),
    ('f', DemoAction::ViewportResetCamera),
    ('o', DemoAction::ViewportProjection),
];

pub(super) struct PaletteCommand {
    pub id: u8,
    pub label: &'static str,
    pub shortcut: String,
    pub enabled: bool,
    pub action: DemoAction,
}

pub(super) fn filtered_palette_commands(query: &str, demo: &DemoState) -> Vec<PaletteCommand> {
    let mut commands = Vec::new();
    let mut add = |id, label, keywords, enabled, action| {
        let shortcut = SHORTCUT_COMMANDS
            .iter()
            .find(|(_, candidate)| *candidate == action)
            .map_or_else(String::new, |(key, _)| key.to_uppercase().to_string());
        if command_palette_matches(query, label, &shortcut, keywords) {
            commands.push(PaletteCommand {
                id,
                label,
                shortcut,
                enabled,
                action,
            });
        }
    };
    // IDs 0–5 retain their original meanings. Catalog meaning belongs to this host.
    add(
        0,
        "Open 2D Gallery",
        "view 界面 画廊",
        true,
        DemoAction::SelectMode(AcceptanceMode::Gallery2d),
    );
    add(
        1,
        "Open 3D Viewport",
        "view 视口",
        demo.viewport3d_available,
        DemoAction::SelectMode(AcceptanceMode::Viewport3d),
    );
    add(
        2,
        "Toggle Theme",
        "appearance dark light 主题 外观",
        true,
        DemoAction::ToggleTheme,
    );
    add(
        3,
        "Reset Workspace",
        "layout 布局 重置",
        true,
        DemoAction::WorkspaceReset,
    );
    add(
        4,
        "Cancel Gallery Activity",
        "task progress stop 任务 取消",
        demo.busy,
        DemoAction::CancelBusy,
    );
    add(
        5,
        "Restart Gallery Activity",
        "task progress start 任务 重启",
        !demo.busy,
        DemoAction::RestartBusy,
    );
    add(
        6,
        "Switch Language",
        "locale 中文 english 语言",
        true,
        DemoAction::ToggleLocale,
    );
    add(
        7,
        "Toggle Sidebar",
        "navigation drawer 侧栏",
        true,
        DemoAction::ToggleDrawer,
    );
    add(
        8,
        "Show Notification",
        "feedback toast 通知",
        true,
        DemoAction::ShowToast,
    );
    add(
        9,
        "Toggle Switch",
        "controls boolean 开关",
        true,
        DemoAction::ToggleSwitch,
    );
    add(
        10,
        "Toggle Pinned Button",
        "controls compact 固定",
        true,
        DemoAction::ToggleCompactButton,
    );
    add(
        11,
        "Cycle Checkbox",
        "controls selection 复选",
        true,
        DemoAction::CycleCheckbox,
    );
    add(
        12,
        "Toggle Slider Availability",
        "controls range enabled 滑块",
        true,
        DemoAction::ToggleSliderEnabled,
    );
    add(
        13,
        "Reverse Horizontal Flow",
        "layout direction rtl ltr 方向",
        true,
        DemoAction::ToggleHorizontalFlow,
    );
    add(
        14,
        "Open Confirmation Dialog",
        "feedback modal 对话框",
        true,
        DemoAction::OpenDemoDialog,
    );
    add(
        15,
        "Open Documents",
        "view markdown 文档",
        true,
        DemoAction::ShowDocuments,
    );
    add(
        16,
        "Increase Sample Number",
        "controls numeric increment 增加 数值",
        true,
        DemoAction::StepNumber(1),
    );
    add(
        17,
        "Decrease Sample Number",
        "controls numeric decrement 减少 数值",
        true,
        DemoAction::StepNumber(-1),
    );
    add(
        18,
        "Select First Radio Option",
        "controls selection 单选",
        true,
        DemoAction::SelectRadio(0),
    );
    add(
        19,
        "Select Second Radio Option",
        "controls selection 单选",
        true,
        DemoAction::SelectRadio(1),
    );
    add(
        20,
        "Select Third Radio Option",
        "controls selection 单选",
        true,
        DemoAction::SelectRadio(2),
    );
    add(
        21,
        "Toggle Tree Branch",
        "navigation expand collapse 树",
        true,
        DemoAction::ToggleTree,
    );
    add(
        22,
        "Reset 3D Camera",
        "view camera 相机",
        demo.viewport3d_available,
        DemoAction::ViewportResetCamera,
    );
    add(
        23,
        "Switch 3D Projection",
        "view camera orthographic perspective 相机",
        demo.viewport3d_available,
        DemoAction::ViewportProjection,
    );
    commands
}

pub(super) fn projection(
    scopes: UiScopes,
    demo: &DemoState,
    snapshot: &vesta_gpui::TextInputSnapshot,
    query: &str,
    focused: bool,
    size: Size,
    theme: WidgetTheme,
) -> Widget<DemoAction> {
    let id = |slot, part: &str| {
        WidgetIdentity::new(
            scopes.root.element(PALETTE_ROOT + slot),
            format!("palette.{part}"),
        )
    };
    let commands = filtered_palette_commands(query, demo);
    let keys = commands
        .iter()
        .map(|c| (c.id, c.enabled))
        .collect::<Vec<_>>();
    let mut controller = demo.palette.clone();
    controller.reconcile(
        &keys,
        command_palette_geometry(size, commands.len(), theme).list_height,
    );
    let entries = commands
        .into_iter()
        .map(|c| CommandPaletteEntry {
            root: id(100 + u32::from(c.id), &format!("command.{}", c.id)),
            label: c.label.into(),
            shortcut: c.shortcut,
            highlighted: controller.highlighted() == Some(&c.id),
            action: c.enabled.then_some(DemoAction::ExecutePalette(c.id)),
        })
        .collect();
    let editor = text_field(WidgetIdentity::new(scopes.gallery.element(SEARCH_FIELD), "gallery.search.input"),
        snapshot.clone(), focused, theme)
        .semantic(Semantic::new(SemanticRole::TextInput, "Search commands")
            .description("Type words to filter; arrows, Home, End and Page keys select; Enter executes; Escape cancels"));
    command_palette_scrolled(
        CommandPaletteScrollIds {
            base: CommandPaletteIds {
                backdrop: id(0, "backdrop"),
                panel: id(1, "panel"),
                title: id(2, "title"),
                list: id(3, "list"),
                empty: id(4, "empty"),
            },
            content: id(5, "content"),
        },
        size,
        editor,
        entries,
        controller.scroll().offset_y(),
        theme,
    )
}

impl App {
    pub(super) fn finish_command_palette(&mut self, owner: UiWindowId, reason: PopupCloseReason) {
        if self.demo.palette_session.owner() != Some(owner) {
            return;
        }
        self.finish_window_text_session(owner);
        let Some(exit) =
            self.demo
                .palette_session
                .finish_into(owner, reason, &mut self.search_state)
        else {
            return;
        };
        if self.demo.palette_drag.take().is_some() {
            if owner == STANDALONE_WINDOW {
                self.input_ownership.cancel_capture();
            } else if let Some(host) = self
                .native_windows
                .values_mut()
                .find(|h| h.ui_window == owner)
            {
                host.input.cancel_capture();
            }
        }
        self.demo.palette_scroll_node = None;
        if self.demo.dialog_stack.is_empty() && self.overlay_owner == Some(owner) {
            self.overlay_owner = None;
        }
        if let Some(target) = exit.dismissal.focus_request() {
            self.restore_after_rebuild(owner, Some(target.element));
        }
    }

    pub(super) fn execute_palette_command(&mut self, id: u8) {
        let owner = self.action_window;
        if self.demo.palette_session.owner() != Some(owner)
            || !self.owns_overlay(owner)
            || self.search_state.is_composing()
        {
            return;
        }
        let command = filtered_palette_commands(self.search_state.text(), &self.demo)
            .into_iter()
            .find(|command| command.id == id && command.enabled)
            .map(|command| command.action);
        if let Some(command) = command {
            self.finish_command_palette(owner, PopupCloseReason::Confirm);
            self.apply_demo_action(command);
        }
    }

    fn palette_entries(&self) -> Vec<(u8, bool)> {
        filtered_palette_commands(self.search_state.text(), &self.demo)
            .iter()
            .map(|c| (c.id, c.enabled))
            .collect()
    }

    pub(super) fn handle_palette_key_for(&mut self, window: UiWindowId, key: &Key) -> bool {
        if self.demo.palette_session.owner() != Some(window)
            || !self.owns_overlay(window)
            || self.text_session_field(window) != Some(TextFieldKind::Search)
        {
            return false;
        }
        let Some(chord) = shortcut_chord(key.as_ref(), self.window_modifiers(window)) else {
            return false;
        };
        let entries = self.palette_entries();
        let height = self
            .demo
            .palette_scroll_node
            .as_ref()
            .map_or(240.0, |n| n.scroll_viewport().height);
        let action =
            self.demo
                .palette
                .key_action(&entries, chord, self.search_state.is_composing(), height);
        match action {
            Some(ChoiceKeyAction::Confirm(id)) => self.execute_palette_command(id),
            Some(ChoiceKeyAction::Close) => {
                self.finish_command_palette(window, PopupCloseReason::Explicit)
            }
            Some(ChoiceKeyAction::Highlight(_) | ChoiceKeyAction::Consumed) => {}
            _ => return false,
        }
        true
    }

    pub(super) fn sync_palette_layout(
        &mut self,
        owner: UiWindowId,
        scope: ElementScope,
        layout: &LayoutTree,
    ) {
        if self.demo.palette_session.owner() != Some(owner) {
            return;
        }
        if let Some(node) = layout.node(scope.element(PALETTE_ROOT + 3)) {
            let entries = self.palette_entries();
            self.demo
                .palette
                .reconcile(&entries, node.scroll_viewport().height);
            self.demo.palette_scroll_node = Some(node.clone());
        }
    }

    /// Intercept the owning popup's wheel and thumb before the page's normal scroll routing.
    /// Ordinary pointer presses still go through the original text/action hit paths.
    pub(super) fn handle_palette_scroll_event(
        &mut self,
        owner: UiWindowId,
        event: &WindowEvent,
    ) -> bool {
        if self.demo.palette_session.owner() != Some(owner) {
            return false;
        }
        let (point, scale) = if owner == STANDALONE_WINDOW {
            (
                self.cursor,
                self.window.as_ref().map_or(1.0, |w| w.scale_factor()),
            )
        } else if let Some(host) = self.native_windows.values().find(|h| h.ui_window == owner) {
            (host.cursor, host.window.scale_factor())
        } else {
            return false;
        };
        match event {
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y * 34.0,
                    MouseScrollDelta::PixelDelta(p) => -logical_point(*p, scale).y,
                };
                let entries = self.palette_entries();
                let height = self
                    .demo
                    .palette_scroll_node
                    .as_ref()
                    .map_or(240.0, |n| n.scroll_viewport().height);
                self.demo.palette.reconcile(&entries, height);
                self.demo.palette.wheel(delta);
                true
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let drag = self.demo.palette_scroll_node.as_ref().and_then(|n| {
                    self.demo
                        .palette
                        .scroll()
                        .begin_thumb_drag_in_layout(n, point)
                });
                if let Some(drag) = drag {
                    self.demo.palette_drag = Some(drag);
                    if let Some(scope) = self.window_scopes(owner) {
                        let target = InputTarget {
                            window: owner,
                            element: scope.root.element(PALETTE_ROOT + 3),
                        };
                        if owner == STANDALONE_WINDOW {
                            self.input_ownership
                                .capture_pointer(target, PointerButton::Primary);
                        } else if let Some(host) = self
                            .native_windows
                            .values_mut()
                            .find(|h| h.ui_window == owner)
                        {
                            host.input.capture_pointer(target, PointerButton::Primary);
                        }
                    }
                    true
                } else {
                    let hit = if owner == STANDALONE_WINDOW {
                        self.hit_test.hit(point)
                    } else {
                        self.native_windows
                            .values()
                            .find(|h| h.ui_window == owner)
                            .and_then(|h| h.hit_test.hit(point))
                    };
                    self.window_scopes(owner)
                        .zip(self.window_actions(owner))
                        .is_some_and(|(scopes, route)| {
                            hit.is_some_and(|hit| {
                                hit != scopes.gallery.element(SEARCH_FIELD)
                                    && route.is_descendant_of(
                                        hit,
                                        scopes.root.element(PALETTE_ROOT + 1),
                                    )
                                    && route.action_for_hit(hit).is_none()
                            })
                        })
                }
            }
            WindowEvent::CursorMoved { position, .. } if self.demo.palette_drag.is_some() => {
                let point = logical_point(*position, scale);
                if owner == STANDALONE_WINDOW {
                    self.cursor = point;
                } else if let Some(host) = self
                    .native_windows
                    .values_mut()
                    .find(|h| h.ui_window == owner)
                {
                    host.cursor = point;
                }
                if let Some(drag) = self.demo.palette_drag {
                    self.demo.palette.drag_scroll(drag, point);
                }
                true
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } if self.demo.palette_drag.take().is_some() => {
                if owner == STANDALONE_WINDOW {
                    self.input_ownership.cancel_capture();
                } else if let Some(host) = self
                    .native_windows
                    .values_mut()
                    .find(|h| h.ui_window == owner)
                {
                    host.input.cancel_capture();
                }
                true
            }
            _ => false,
        }
    }
}
