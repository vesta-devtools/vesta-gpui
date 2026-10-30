//! Window menu contexts, projection and host dispatch (ADR-0275).

use super::*;
use vesta_gpui_widgets::PopupCloseReason;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MenuContext {
    pub container: ContainerRef,
    pub pane: PaneId,
    pub active: Option<ItemId>,
    pub tab: Option<ItemId>,
}

impl MenuContext {
    pub(super) fn capture(
        demo: &DemoState,
        container: ContainerRef,
        pane: PaneId,
        tab: Option<ItemId>,
    ) -> Self {
        Self {
            container,
            pane,
            active: demo.workspace.pane(pane).and_then(|pane| pane.active),
            tab,
        }
    }

    pub(super) fn main(demo: &DemoState) -> Self {
        let pane = demo
            .tab_menu_item
            .and_then(|item| {
                demo.workspace
                    .panes()
                    .into_iter()
                    .find(|pane| pane.items.iter().any(|candidate| candidate.id == item))
                    .map(|pane| pane.id)
            })
            .or_else(|| {
                let panes = container_panes(&demo.workspace, ContainerRef::Main);
                panes
                    .contains(&demo.workspace.active_pane())
                    .then_some(demo.workspace.active_pane())
                    .or_else(|| panes.first().copied())
            })
            .expect("main workspace retains a pane");
        Self::capture(demo, ContainerRef::Main, pane, demo.tab_menu_item)
    }

    pub(super) fn is_live(self, demo: &DemoState) -> bool {
        container_panes(&demo.workspace, self.container).contains(&self.pane)
            && demo.workspace.pane(self.pane).is_some_and(|pane| {
                pane.active == self.active
                    && self
                        .tab
                        .is_none_or(|item| pane.items.iter().any(|candidate| candidate.id == item))
            })
    }
}

pub(super) fn container_panes(workspace: &WorkspaceModel, container: ContainerRef) -> Vec<PaneId> {
    match container {
        ContainerRef::Main => workspace.root().panes(),
        ContainerRef::Floating(id) => workspace
            .floating_window(id)
            .map_or_else(Vec::new, |window| window.root.panes()),
    }
}

#[derive(Clone)]
pub(super) struct WindowMenuSession {
    pub controller: MenuController,
    pub context: MenuContext,
    pub anchor: Point,
    pub origin: MenuOrigin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MenuOrigin {
    Context,
    ApplicationBar,
}

impl App {
    pub(super) fn open_application_menu(&mut self, id: MenuId) {
        if !application_chrome::APPLICATION_MENUS
            .iter()
            .any(|header| header.id == id)
        {
            return;
        }
        let width = self.window.as_ref().map_or(1280.0, |window| {
            window
                .inner_size()
                .to_logical::<f32>(window.scale_factor())
                .width
        });
        let context = self
            .window_menu_snapshot(STANDALONE_WINDOW)
            .filter(|menu| menu.origin == MenuOrigin::ApplicationBar)
            .map_or_else(|| MenuContext::main(&self.demo), |menu| menu.context);
        self.open_window_menu(
            STANDALONE_WINDOW,
            id,
            application_chrome::application_menu_anchor(id, width),
            context,
        );
        if self.demo.menu.is_open() {
            self.demo.menu_origin = MenuOrigin::ApplicationBar;
        }
    }

    fn step_application_menu(&mut self, root: MenuId, backwards: bool) {
        let roots = application_chrome::APPLICATION_MENUS
            .iter()
            .map(|header| header.id)
            .collect::<Vec<_>>();
        if let Some(next) = vesta_gpui_widgets::menu_bar_next(&roots, root, backwards) {
            self.open_application_menu(next);
        }
    }

    pub(super) fn split_workspace_item_in_window(
        &mut self,
        window: UiWindowId,
        pane: PaneId,
        item: ItemId,
    ) {
        if !self.window_container(window).is_some_and(|container| {
            container_panes(&self.demo.workspace, container).contains(&pane)
        }) || self
            .demo
            .workspace
            .pane(pane)
            .is_none_or(|pane| pane.active != Some(item) || pane.items.len() < 2)
        {
            return;
        }
        let tab = self.window_actions(window).and_then(|actions| {
            actions.focus_order().iter().copied().find(|element| {
                matches!(actions.action_at(*element), Some(DemoAction::Workspace(WorkspaceViewAction::Tab(target))) if *target == item)
            })
        });
        if self
            .demo
            .workspace
            .dock_item(
                item,
                DockAction::Split {
                    pane,
                    side: Side::Right,
                },
            )
            .is_ok()
        {
            self.demo.sync_workspace_ui();
            if let Some(tab) = tab {
                self.set_window_focus(window, Some(tab));
            }
        }
    }

    pub(super) fn pane_context_at(
        &self,
        window: UiWindowId,
        tab: Option<ItemId>,
    ) -> Option<MenuContext> {
        let container = self.window_container(window)?;
        let panes = container_panes(&self.demo.workspace, container);
        let (frame, cursor) = if window == STANDALONE_WINDOW {
            (&self.dock_frame, self.cursor)
        } else {
            let host = self
                .native_windows
                .values()
                .find(|host| host.ui_window == window)?;
            (&host.dock_frame, host.cursor)
        };
        let pane = tab
            .and_then(|item| {
                panes.iter().copied().find(|pane| {
                    self.demo
                        .workspace
                        .pane(*pane)
                        .is_some_and(|pane| pane.items.iter().any(|candidate| candidate.id == item))
                })
            })
            .or_else(|| {
                frame
                    .panes
                    .iter()
                    .find(|pane| pane.root.contains(cursor))
                    .map(|pane| pane.pane)
            })
            .or_else(|| {
                panes
                    .contains(&self.demo.workspace.active_pane())
                    .then_some(self.demo.workspace.active_pane())
            })
            .or_else(|| panes.first().copied())?;
        Some(MenuContext::capture(&self.demo, container, pane, tab))
    }

    pub(super) fn window_container(&self, window: UiWindowId) -> Option<ContainerRef> {
        if window == STANDALONE_WINDOW {
            Some(ContainerRef::Main)
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .map(|host| ContainerRef::Floating(host.logical_id))
        }
    }

    pub(super) fn window_menu_snapshot(&self, window: UiWindowId) -> Option<WindowMenuSession> {
        if window == STANDALONE_WINDOW {
            self.demo.menu.is_open().then(|| WindowMenuSession {
                controller: self.demo.menu.clone(),
                context: self
                    .demo
                    .menu_context
                    .unwrap_or_else(|| MenuContext::main(&self.demo)),
                anchor: self.demo.menu_anchor,
                origin: self.demo.menu_origin,
            })
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .and_then(|host| host.pane_menu.clone())
        }
    }

    pub(super) fn store_window_menu(&mut self, window: UiWindowId, session: WindowMenuSession) {
        if window == STANDALONE_WINDOW {
            self.demo.menu = session.controller;
            self.demo.menu_context = Some(session.context);
            self.demo.menu_anchor = session.anchor;
            self.demo.menu_origin = session.origin;
        } else if let Some(host) = self
            .native_windows
            .values_mut()
            .find(|host| host.ui_window == window)
        {
            host.pane_menu = Some(session);
        }
    }

    pub(super) fn reconcile_window_menu(&mut self, window: UiWindowId) {
        let Some(mut session) = self.window_menu_snapshot(window) else {
            return;
        };
        if !session.context.is_live(&self.demo) {
            self.close_window_menu(window, PopupCloseReason::Explicit);
            return;
        }
        let menus = session
            .controller
            .path()
            .iter()
            .map(|id| menu_descriptor(*id, &self.demo, session.context))
            .collect::<Vec<_>>();
        if session.controller.reconcile(&menus) {
            let open = session.controller.is_open();
            let highlighted = session.controller.highlighted();
            self.store_window_menu(window, session);
            if !open {
                self.close_window_menu(window, PopupCloseReason::Explicit);
            } else if highlighted.is_some() {
                self.focus_window_menu_item(window);
            } else {
                self.set_window_focus(window, None);
            }
        }
    }

    pub(super) fn focus_window_menu_item(&mut self, window: UiWindowId) {
        if let Some(item) = self
            .window_menu_snapshot(window)
            .and_then(|menu| menu.controller.highlighted())
            && let Some(scopes) = self.window_scopes(window)
        {
            self.set_window_focus(window, Some(menu_item_element(scopes.menu, item)));
        }
    }

    pub(super) fn open_window_menu(
        &mut self,
        window: UiWindowId,
        id: MenuId,
        anchor: Point,
        context: MenuContext,
    ) {
        if self.window_container(window) != Some(context.container) || !context.is_live(&self.demo)
        {
            return;
        }
        let opener = self
            .window_menu_snapshot(window)
            .and_then(|menu| menu.controller.return_focus())
            .or_else(|| {
                self.window_focus(window)
                    .map(|element| InputTarget { window, element })
            });
        self.close_combo_for(window, PopupCloseReason::FocusTransfer);
        self.close_popover_for(window, PopupCloseReason::FocusTransfer);
        self.set_window_focus(window, None);
        let mut controller = MenuController::default();
        controller.open_in(window, id, opener);
        controller.select_edge(&menu_descriptor(id, &self.demo, context), false);
        self.store_window_menu(
            window,
            WindowMenuSession {
                controller,
                context,
                anchor,
                origin: MenuOrigin::Context,
            },
        );
        self.focus_window_menu_item(window);
    }

    pub(super) fn close_window_menu(&mut self, window: UiWindowId, reason: PopupCloseReason) {
        let dismissal = if window == STANDALONE_WINDOW {
            self.demo.menu_context = None;
            self.demo.tab_menu_item = None;
            self.demo.menu_origin = MenuOrigin::Context;
            self.demo.menu.dismiss(window, reason)
        } else {
            self.native_windows
                .values_mut()
                .find(|host| host.ui_window == window)
                .and_then(|host| host.pane_menu.take())
                .and_then(|mut menu| menu.controller.dismiss(window, reason))
        };
        if let Some(dismissal) = dismissal {
            self.restore_popup_dismissal(dismissal);
        }
    }

    pub(super) fn apply_window_action(&mut self, window: UiWindowId, action: DemoAction) {
        self.action_window = window;
        if window == STANDALONE_WINDOW {
            self.apply_demo_action(action);
        } else if let Some(id) = self
            .native_windows
            .iter()
            .find(|(_, host)| host.ui_window == window)
            .map(|(id, _)| *id)
        {
            self.apply_native_action(id, action);
        }
    }

    pub(super) fn prepare_window_menu_action(
        &mut self,
        window: UiWindowId,
        action: DemoAction,
    ) -> bool {
        let Some(session) = self.window_menu_snapshot(window) else {
            return true;
        };
        if !session.context.is_live(&self.demo) {
            self.close_window_menu(window, PopupCloseReason::Explicit);
            return false;
        }
        self.reconcile_window_menu(window);
        let Some(session) = self.window_menu_snapshot(window) else {
            return false;
        };
        let entry = session.controller.path().iter().find_map(|id| {
            menu_descriptor(*id, &self.demo, session.context)
                .items
                .into_iter()
                .find(|item| !item.separator && item.action == action)
        });
        let Some(entry) = entry else {
            return match action {
                DemoAction::OpenSubmenu(_) => false,
                // The legacy dismissal command is not a descriptor entry.
                DemoAction::MenuCommand(id) => id == MenuItemId(1),
                _ => true,
            };
        };
        if !entry.enabled {
            return false;
        }
        if !matches!(action, DemoAction::OpenSubmenu(_)) {
            self.close_window_menu(window, PopupCloseReason::Explicit);
        }
        true
    }

    pub(super) fn enter_window_submenu(&mut self, window: UiWindowId, id: MenuId) {
        self.reconcile_window_menu(window);
        if let Some(mut session) = self.window_menu_snapshot(window) {
            let Some(parent) = session.controller.current() else {
                return;
            };
            let descriptor = menu_descriptor(parent, &self.demo, session.context);
            let Some(item) = descriptor
                .items
                .iter()
                .find(|item| item.enabled && !item.separator && item.submenu == Some(id))
            else {
                return;
            };
            session.controller.highlight(Some(item.id));
            session.controller.enter_submenu(id);
            session
                .controller
                .select_edge(&menu_descriptor(id, &self.demo, session.context), false);
            self.store_window_menu(window, session);
            self.focus_window_menu_item(window);
        }
    }

    pub(super) fn handle_window_menu_key(&mut self, window: UiWindowId, key: &Key) -> bool {
        let was_open = self.window_menu_snapshot(window).is_some();
        self.reconcile_window_menu(window);
        let Some(mut session) = self.window_menu_snapshot(window) else {
            return was_open;
        };
        if !session.context.is_live(&self.demo) {
            self.close_window_menu(window, PopupCloseReason::Explicit);
            return true;
        }
        let descriptor = menu_descriptor(
            session.controller.current().unwrap_or(MenuId(1)),
            &self.demo,
            session.context,
        );
        let application_root = window == STANDALONE_WINDOW
            && session.origin == MenuOrigin::ApplicationBar
            && session.controller.path().len() == 1;
        if application_root && *key == Key::Named(NamedKey::ArrowLeft) {
            self.step_application_menu(descriptor.id, true);
            return true;
        }
        let decision = shortcut_chord(key.as_ref(), self.window_modifiers(window))
            .map_or(MenuKeyAction::Consumed, |chord| {
                session.controller.key_action(&descriptor, chord)
            });
        match decision {
            MenuKeyAction::OpenSubmenu(submenu) => {
                session.controller.enter_submenu(submenu);
                session.controller.select_edge(
                    &menu_descriptor(submenu, &self.demo, session.context),
                    false,
                );
            }
            MenuKeyAction::Close => {
                self.close_window_menu(window, PopupCloseReason::Explicit);
                return true;
            }
            MenuKeyAction::TraverseFocus { backwards } => {
                self.close_window_menu(window, PopupCloseReason::Explicit);
                self.request_window_focus_traversal(window, backwards);
                return true;
            }
            MenuKeyAction::Activate(id) => {
                if let Some(action) = descriptor
                    .items
                    .iter()
                    .find(|item| item.id == id)
                    .map(|item| item.action)
                {
                    self.apply_window_action(window, action);
                }
                return true;
            }
            MenuKeyAction::Updated => {}
            MenuKeyAction::Consumed => {
                if application_root && *key == Key::Named(NamedKey::ArrowRight) {
                    self.step_application_menu(descriptor.id, false);
                }
                return true;
            }
        }
        self.store_window_menu(window, session);
        self.focus_window_menu_item(window);
        true
    }

    pub(super) fn hover_window_menu_item(
        &mut self,
        window: UiWindowId,
        hovered: Option<ElementId>,
    ) {
        let Some((mut session, hovered, scopes)) = self
            .window_menu_snapshot(window)
            .zip(hovered)
            .zip(self.window_scopes(window))
            .map(|((session, hovered), scopes)| (session, hovered, scopes))
        else {
            return;
        };
        if window == STANDALONE_WINDOW
            && session.origin == MenuOrigin::ApplicationBar
            && let Some(header) = application_chrome::APPLICATION_MENUS
                .iter()
                .find(|header| scopes.header.element(header.element) == hovered)
        {
            if session.controller.path().first() != Some(&header.id) {
                self.open_application_menu(header.id);
                self.request_all_redraw();
            }
            return;
        }
        for id in session
            .controller
            .path()
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>()
        {
            let descriptor = menu_descriptor(id, &self.demo, session.context);
            if let Some(item) = descriptor
                .items
                .iter()
                .find(|item| menu_item_element(scopes.menu, item.id) == hovered)
            {
                if session.controller.highlight_in(&descriptor, item.id) {
                    self.store_window_menu(window, session);
                    self.set_window_focus(window, Some(hovered));
                    self.request_all_redraw();
                }
                return;
            }
        }
    }
}

pub(super) fn menu_item_local(item: MenuItemId, offset: u32) -> u32 {
    u32::try_from(item.0)
        .ok()
        .and_then(|id| id.checked_mul(5))
        .and_then(|id| id.checked_add(1000 + offset))
        .expect("demo menu item identity fits its scope")
}

pub(super) fn menu_item_element(scope: ElementScope, item: MenuItemId) -> ElementId {
    scope.element(menu_item_local(item, 0))
}

pub(super) fn menu_item_identity(
    scope: ElementScope,
    item: MenuItemId,
    offset: u32,
    name: &str,
) -> WidgetIdentity {
    let identity = scope.identity(
        menu_item_local(item, offset),
        format!("menu.{name}.{}", item.0),
    );
    WidgetIdentity {
        element: identity.id,
        key: identity.key,
    }
}

// Host adapter keeps the existing stable window inputs and owning text context.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_window_menu_popup(
    scope: ElementScope,
    theme: WidgetTheme,
    demo: &DemoState,
    session: &WindowMenuSession,
    hovered: Option<ElementId>,
    viewport_size: Size,
    actions: &mut Vec<(ElementId, DemoAction)>,
    text_context: &mut TextInputContext,
) -> Element {
    let mut overlay = Element::container(scope.element(1), ElementKey::new("menu.overlay")).styled(
        Style::default()
            .absolute(0.0, 0.0)
            .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
            .z_index(85)
            .pointer_events(false),
    );
    let bounds = Rect::new(
        8.0,
        CHROME_HEIGHT + 6.0,
        (viewport_size.width - 16.0).max(0.0),
        (viewport_size.height
            - CHROME_HEIGHT
            - if session.context.container == ContainerRef::Main {
                CHROME_STATUS_HEIGHT
            } else {
                0.0
            }
            - 14.0)
            .max(0.0),
    );
    let mut previous: Option<(Menu<DemoAction>, Rect)> = None;
    for (depth, menu_id) in session.controller.path().iter().copied().enumerate() {
        let mut menu = menu_descriptor(menu_id, demo, session.context);
        menu.label = display_text(&menu.label, demo.locale);
        for item in &mut menu.items {
            item.label = display_text(&item.label, demo.locale);
            if let Some(section) = &mut item.section_header {
                *section = display_text(section, demo.locale);
            }
        }
        if let Some(item) = menu.items.iter_mut().find(|item| item.id == MenuItemId(32)) {
            item.checked = Some(theme.mode == ThemeMode::Dark);
        }
        let (anchor, direction) = if let Some((parent, rect)) = &previous {
            (
                vesta_gpui_widgets::menu_submenu_anchor(
                    parent,
                    menu_id,
                    &session.controller,
                    *rect,
                    theme,
                )
                .unwrap_or(*rect),
                PopupDirection::Right,
            )
        } else {
            (
                Rect::new(session.anchor.x, session.anchor.y, 0.0, 0.0),
                PopupDirection::Below,
            )
        };
        let local = 100 + u32::try_from(menu_id.0).expect("demo menu identity fits its scope");
        let popup = vesta_gpui_widgets::menu_popup(
            &menu,
            &session.controller,
            vesta_gpui_widgets::MenuPopupSpec {
                root: WidgetIdentity::new(
                    scope.element(local),
                    scope
                        .identity(local, format!("menu.popup.{}", menu_id.0))
                        .key
                        .as_str(),
                ),
                content: WidgetIdentity::new(
                    scope.element(local + 200),
                    format!("menu.content.{}", menu_id.0),
                ),
                anchor,
                bounds,
                direction,
                z_index: 86 + depth as i32,
                hovered,
            },
            theme,
            text_context,
            |item| MenuItemIds {
                root: menu_item_identity(scope, item, 0, "item"),
                check: menu_item_identity(scope, item, 1, "check"),
                label: menu_item_identity(scope, item, 2, "label"),
                shortcut: menu_item_identity(scope, item, 3, "shortcut"),
                arrow: menu_item_identity(scope, item, 4, "arrow"),
            },
            |item| item.action,
        );
        match popup {
            Ok(popup) => {
                let style = &popup.element.style;
                let Dimension::Px(width) = style.width else {
                    unreachable!()
                };
                let Dimension::Px(height) = style.height else {
                    unreachable!()
                };
                previous = Some((
                    menu,
                    Rect::new(
                        style.left.unwrap_or(0.0),
                        style.top.unwrap_or(0.0),
                        width,
                        height,
                    ),
                ));
                overlay = overlay.child(widget_element(popup, actions));
            }
            Err(error) => {
                eprintln!("failed to build shared menu: {error}");
                break;
            }
        }
    }
    overlay
}

pub(super) fn menu_descriptor(
    id: MenuId,
    demo: &DemoState,
    context: MenuContext,
) -> Menu<DemoAction> {
    match id {
        MenuId(5) => {
            let pane = context.pane;
            let live = context.is_live(demo);
            let can_back = live
                && demo
                    .pane_history
                    .get(&pane)
                    .is_some_and(|history| history.target(PaneNavigationDirection::Back).is_some());
            let can_forward = live
                && demo.pane_history.get(&pane).is_some_and(|history| {
                    history.target(PaneNavigationDirection::Forward).is_some()
                });
            let can_split = live
                && demo
                    .workspace
                    .pane(pane)
                    .is_some_and(|pane| pane.items.len() > 1);
            let can_zoom = live && container_panes(&demo.workspace, context.container).len() > 1;
            let active = context.active;
            let navigate = |direction| {
                active
                    .map(|active| {
                        DemoAction::Workspace(WorkspaceViewAction::Navigate {
                            pane,
                            active,
                            direction,
                        })
                    })
                    .unwrap_or_default()
            };
            let command = |command| {
                active
                    .map(|active| {
                        DemoAction::Workspace(WorkspaceViewAction::HeaderCommand {
                            pane,
                            active,
                            command,
                        })
                    })
                    .unwrap_or_default()
            };
            let mut menu = Menu::new(MenuId(5), "Pane")
                .item(
                    MenuItem::new(
                        MenuItemId(50),
                        "Back in pane history",
                        navigate(PaneNavigationDirection::Back),
                    )
                    .enabled(can_back),
                )
                .item(
                    MenuItem::new(
                        MenuItemId(51),
                        "Forward in pane history",
                        navigate(PaneNavigationDirection::Forward),
                    )
                    .enabled(can_forward),
                )
                .item(MenuItem::separator(MenuItemId(55)))
                .item(
                    MenuItem::new(MenuItemId(52), "New Gallery tab", command(HEADER_NEW_TAB))
                        .enabled(live),
                )
                .item(
                    MenuItem::new(
                        MenuItemId(53),
                        "Move active tab into a pane on the right",
                        command(HEADER_SPLIT_RIGHT),
                    )
                    .enabled(can_split),
                )
                .item(
                    MenuItem::new(
                        MenuItemId(54),
                        if demo.zoomed_panes.get(&context.container) == Some(&pane) {
                            "Restore split panes"
                        } else {
                            "Zoom this pane"
                        },
                        command(HEADER_TOGGLE_ZOOM),
                    )
                    .checked(demo.zoomed_panes.get(&context.container) == Some(&pane))
                    .enabled(can_zoom),
                );
            if let Some(item) = active {
                menu = menu.item(
                    MenuItem::new(
                        MenuItemId(56),
                        "Tear off active tab",
                        DemoAction::TearOffTab(item),
                    )
                    .enabled(live),
                );
            }
            if let ContainerRef::Floating(id) = context.container {
                menu = menu.item(MenuItem::new(
                    MenuItemId(57),
                    "Dock back",
                    DemoAction::RestoreFloating(id),
                ));
            }
            menu
        }
        MenuId(6) => {
            let mut menu = Menu::new(MenuId(6), "Tab");
            if let Some(item) = context.tab {
                menu = menu.item(
                    MenuItem::new(MenuItemId(60), "Tear off tab", DemoAction::TearOffTab(item))
                        .enabled(context.is_live(demo)),
                );
            }
            if let ContainerRef::Floating(id) = context.container {
                menu = menu.item(MenuItem::new(
                    MenuItemId(61),
                    "Dock back",
                    DemoAction::RestoreFloating(id),
                ));
            }
            menu
        }
        MenuId(0) => Menu::new(MenuId(0), "Vesta")
            .item(MenuItem::section_header(MenuItemId(96), "Commands"))
            .item(
                MenuItem::new(
                    MenuItemId(90),
                    "Command palette",
                    DemoAction::OpenCommandPalette,
                )
                .shortcut("Ctrl+Shift+P"),
            )
            .item(MenuItem::section_header(MenuItemId(91), "Appearance"))
            .item(MenuItem::new(
                MenuItemId(92),
                "Switch theme",
                DemoAction::ToggleTheme,
            ))
            .item(MenuItem::new(
                MenuItemId(93),
                match demo.locale {
                    UiLocale::EnUs => "Switch to Chinese",
                    UiLocale::ZhCn => "切换为英文",
                },
                DemoAction::ToggleLocale,
            ))
            .item(MenuItem::section_header(MenuItemId(94), "Application"))
            .item(
                MenuItem::new(MenuItemId(95), "Exit", DemoAction::WindowClose).shortcut("Alt+F4"),
            ),
        MenuId(2) => Menu::new(MenuId(2), "Export")
            .item(
                MenuItem::new(
                    MenuItemId(20),
                    "PNG image (coming later)",
                    DemoAction::MenuCommand(MenuItemId(20)),
                )
                .enabled(false),
            )
            .item(
                MenuItem::new(
                    MenuItemId(21),
                    "Scene snapshot (coming later)",
                    DemoAction::MenuCommand(MenuItemId(21)),
                )
                .enabled(false),
            ),
        MenuId(3) => Menu::new(MenuId(3), "View")
            .item(MenuItem::section_header(MenuItemId(35), "Views"))
            .item(
                MenuItem::new(
                    MenuItemId(30),
                    "2D Widget Gallery",
                    DemoAction::MenuCommand(MenuItemId(30)),
                )
                .shortcut("1"),
            )
            .item(
                MenuItem::new(
                    MenuItemId(31),
                    "3D Viewport",
                    DemoAction::MenuCommand(MenuItemId(31)),
                )
                .shortcut("2")
                .enabled(demo.viewport3d_available),
            )
            .item(MenuItem::new(
                MenuItemId(34),
                "Documents · Markdown / Diff",
                DemoAction::ShowDocuments,
            ))
            .item(MenuItem::section_header(MenuItemId(33), "Appearance"))
            .item(
                MenuItem::new(
                    MenuItemId(32),
                    "Dark theme",
                    DemoAction::MenuCommand(MenuItemId(32)),
                )
                .shortcut("T"),
            ),
        MenuId(4) => Menu::new(MenuId(4), "Window")
            .item(MenuItem::section_header(MenuItemId(45), "Tabs"))
            .item(MenuItem::new(
                MenuItemId(40),
                "Move active tab",
                DemoAction::MenuCommand(MenuItemId(40)),
            ))
            .item(MenuItem::new(
                MenuItemId(41),
                "Split active tab",
                DemoAction::MenuCommand(MenuItemId(41)),
            ))
            .item(MenuItem::new(
                MenuItemId(42),
                "Float active tab",
                DemoAction::MenuCommand(MenuItemId(42)),
            ))
            .item(MenuItem::section_header(MenuItemId(44), "Workspace"))
            .item(MenuItem::new(
                MenuItemId(43),
                "Reset workspace",
                DemoAction::MenuCommand(MenuItemId(43)),
            )),
        _ => Menu::new(MenuId(1), "File")
            .item(MenuItem::section_header(MenuItemId(15), "Workspace"))
            .item(MenuItem::new(
                MenuItemId(10),
                "Reset workspace",
                DemoAction::MenuCommand(MenuItemId(10)),
            ))
            .item(MenuItem::section_header(MenuItemId(12), "Output"))
            .item(
                MenuItem::new(MenuItemId(13), "Export", DemoAction::OpenSubmenu(MenuId(2)))
                    .submenu(MenuId(2)),
            )
            .item(MenuItem::section_header(MenuItemId(14), "Application"))
            .item(
                MenuItem::new(
                    MenuItemId(11),
                    "Exit",
                    DemoAction::MenuCommand(MenuItemId(11)),
                )
                .shortcut("Alt+F4"),
            ),
    }
}

#[cfg(test)]
pub(super) fn demo_menu(id: MenuId, demo: &DemoState) -> Menu<DemoAction> {
    menu_descriptor(
        id,
        demo,
        demo.menu_context.unwrap_or_else(|| MenuContext::main(demo)),
    )
}

pub(super) fn build_menu_popup(
    scope: ElementScope,
    theme: WidgetTheme,
    demo: &DemoState,
    hovered: Option<ElementId>,
    viewport_size: Size,
    actions: &mut Vec<(ElementId, DemoAction)>,
    text_context: &mut TextInputContext,
) -> Element {
    let session = WindowMenuSession {
        controller: demo.menu.clone(),
        context: demo.menu_context.unwrap_or_else(|| MenuContext::main(demo)),
        anchor: demo.menu_anchor,
        origin: demo.menu_origin,
    };
    let mut session = session;
    if session.origin == MenuOrigin::ApplicationBar
        && let Some(root) = session.controller.path().first()
    {
        session.anchor = application_chrome::application_menu_anchor(*root, viewport_size.width);
    }
    build_window_menu_popup(
        scope,
        theme,
        demo,
        &session,
        hovered,
        viewport_size,
        actions,
        text_context,
    )
}

impl App {
    pub(super) fn open_menu_for(&mut self, menu: MenuId, anchor: Point) {
        if let Some(context) = self.pane_context_at(STANDALONE_WINDOW, self.demo.tab_menu_item) {
            self.open_window_menu(STANDALONE_WINDOW, menu, anchor, context);
        }
    }

    pub(super) fn hover_menu_item(&mut self, hovered: Option<ElementId>) {
        self.hover_window_menu_item(STANDALONE_WINDOW, hovered);
    }

    pub(super) fn open_menu(&mut self, anchor: Point) {
        self.open_menu_for(MenuId(1), anchor);
    }

    pub(super) fn open_context_menu(&mut self, anchor: Point) {
        self.demo.tab_menu_item = self
            .hit_test
            .hit(anchor)
            .and_then(|hit| self.ui_actions.action_for_hit(hit))
            .and_then(|action| match action {
                DemoAction::Workspace(WorkspaceViewAction::Tab(item)) => Some(*item),
                _ => None,
            })
            .filter(|item| self.demo.workspace.item(*item).is_some());
        self.open_menu_for(
            if self.demo.tab_menu_item.is_some() {
                MenuId(6)
            } else {
                MenuId(1)
            },
            anchor,
        );
    }

    pub(super) fn close_menu(&mut self, restore_focus: bool) {
        self.close_window_menu(
            STANDALONE_WINDOW,
            if restore_focus {
                PopupCloseReason::Explicit
            } else {
                PopupCloseReason::FocusTransfer
            },
        );
    }
}
