//! Native projection for logical `WorkspaceModel::FloatingWindow` entries.

use super::*;
mod events;
mod presentation;
use vesta_gpui_widgets::{TitleBarActions, TitleBarLabels, TitleBarState, title_bar};

pub(super) fn placement_from_native_event(
    current: WindowPlacement,
    event: &WindowEvent,
    scale: f64,
    minimized: bool,
) -> Option<WindowPlacement> {
    if minimized {
        return None;
    }
    match event {
        WindowEvent::Resized(size) => {
            let logical = size.to_logical::<f32>(scale);
            Some(WindowPlacement::new(
                current.x,
                current.y,
                logical.width,
                logical.height,
            ))
        }
        WindowEvent::Moved(position) => {
            let logical = position.to_logical::<f32>(scale);
            Some(WindowPlacement::new(
                logical.x,
                logical.y,
                current.width,
                current.height,
            ))
        }
        _ => None,
    }
}

impl App {
    pub(super) fn native_placement_for_event(
        &self,
        native: WindowId,
        event: &WindowEvent,
    ) -> Option<(FloatingWindowId, WindowPlacement)> {
        let host = self.native_windows.get(&native)?;
        let current = self
            .demo
            .workspace
            .floating_window(host.logical_id)?
            .placement;
        placement_from_native_event(
            current,
            event,
            host.window.scale_factor(),
            host.window.is_minimized() == Some(true),
        )
        .map(|placement| (host.logical_id, placement))
    }
}

pub(crate) struct NativeFloatingHost {
    /// Smoke progress belongs to this window, not the main frame counter.
    pub(crate) smoke_frames: u64,
    pub(crate) logical_id: FloatingWindowId,
    pub(crate) return_pane: PaneId,
    pub(crate) ui_window: UiWindowId,
    pub(crate) window: Arc<Window>,
    pub(crate) surface: GpuState,
    pub(crate) accessibility: AccessibilityAdapter,
    pub(crate) semantics: AccessibilityTree,
    pub(crate) scopes: UiScopes,
    pub(crate) documents: GalleryDocuments,
    pub(crate) chrome_scope: ElementScope,
    pub(crate) title_scope: ElementScope,
    pub(crate) actions: ActionRoute<DemoAction>,
    pub(crate) hit_test: HitTestTree,
    pub(crate) dock_frame: DockFrame,
    pub(crate) input: InputOwnership,
    pub(crate) cursor: Point,
    pub(crate) pointer_click: PointerClick,
    pub(crate) busy_visible: bool,
    pub(crate) caret_blink: CaretBlink,
    pub(crate) page_scroll_item: Option<ItemId>,
    pub(crate) last_title_click: Option<(Instant, Point)>,
    pub(crate) pane_menu: Option<NativePaneMenu>,
    /// Laid-out rect of this window's ramp track, captured on every build so the shared
    /// ramp mapping can be used without duplicating track geometry per host.
    pub(crate) ramp_track_rect: Option<Rect>,
    pub(crate) ramp_drag: Option<RampDrag>,
    /// Trigger rects captured from this window's own layout. Without them the Gallery
    /// projects ComboBox and Popover overlays at their fixed fallback anchors.
    pub(crate) combo_anchor: Option<Rect>,
    pub(crate) popover_anchor: Option<Rect>,
    /// Text field viewports of this window's own layout (ADR-0252).
    pub(crate) text_views: TextViews,
    /// Per-window drag sessions: the main window keeps the same state, but each window must
    /// own its own geometry and pointer session (ADR-0253).
    pub(crate) slider_rect: Option<Rect>,
    pub(crate) slider_drag: Option<ValueEditSession<f32>>,
    pub(crate) number_scrub: Option<NumberScrub>,
    pub(crate) vector_scrub: Option<VectorScrub>,
    pub(crate) drawer_resize: Option<DrawerResizeDrag>,
    pub(crate) table_resize: Option<TableResizeDrag>,
    pub(crate) table_rect: Option<Rect>,
    /// TileGrid reflow captured from this window's projection (ADR-0265).
    pub(crate) tile_columns: usize,
    /// Scroll viewport rects of this window's own layout (ADR-0253 decision 7).
    pub(crate) scroll_geometry: ScrollGeometry,
    pub(crate) focus_rects: HashMap<ElementId, Rect>,
    pub(super) tooltip: WindowTooltip,
    pub(crate) scrollbar_drag: Option<(ElementId, ScrollbarDrag)>,
    /// Tab-strip viewports of this window's layout, so its tab strips scroll on their own.
    pub(crate) tab_scroll_rects: HashMap<PaneId, Rect>,
    pub(crate) tab_reveal: HashMap<PaneId, TabRevealGeometry>,
    /// TextArea inner scroll offset of this window; a caret in one window must not move the
    /// other window's text view.
    pub(crate) area_scroll_y: f32,
    pub(crate) area_follow_caret: bool,
    pub(crate) area_layout: AreaWrapLayout,
}

impl NativeFloatingHost {
    pub(crate) fn create(
        event_loop: &ActiveEventLoop,
        proxy: EventLoopProxy<UserEvent>,
        logical: &vesta_workspace::FloatingWindow,
        return_pane: PaneId,
        owner: &Window,
        gpu: Arc<vesta_gpui_vulkan::GpuContext>,
        documents: GalleryDocuments,
    ) -> Result<Self, String> {
        let placement = logical.placement;
        let attributes = Window::default_attributes()
            .with_title(format!("{} — Vesta", logical.title))
            .with_decorations(!cfg!(target_os = "windows"))
            .with_visible(false)
            .with_position(LogicalPosition::new(placement.x, placement.y))
            .with_inner_size(LogicalSize::new(
                placement.width.max(320.0),
                placement.height.max(220.0),
            ))
            .with_min_inner_size(LogicalSize::new(320.0, 220.0));
        #[cfg(target_os = "windows")]
        let attributes = {
            use winit::{
                platform::windows::{CornerPreference, WindowAttributesExtWindows},
                raw_window_handle::{HasWindowHandle, RawWindowHandle},
            };
            let handle = owner.window_handle().map_err(|error| error.to_string())?;
            let RawWindowHandle::Win32(handle) = handle.as_raw() else {
                return Err("main window did not expose a Win32 owner handle".into());
            };
            // Match the main window: DWM rounds an undecorated window only when the
            // preference is set explicitly, and the undecorated shadow is a separate opt-in.
            attributes
                .with_corner_preference(CornerPreference::Round)
                .with_undecorated_shadow(true)
                .with_owner_window(handle.hwnd.get())
        };
        let window = event_loop
            .create_window(attributes)
            .map(Arc::new)
            .map_err(|error| error.to_string())?;
        let accessibility = AccessibilityAdapter::with_event_loop_proxy(event_loop, &window, proxy);
        let surface = GpuState::with_gpu(window.clone(), gpu)?;
        let chrome_scope = ElementIdAllocator
            .allocate_scope()
            .map_err(|_| "native floating window exhausted UI identity scopes".to_owned())?;
        let title_scope = ElementIdAllocator
            .allocate_scope()
            .map_err(|_| "native floating title bar exhausted UI identity scopes".to_owned())?;
        let host = Self {
            smoke_frames: 0,
            logical_id: logical.id,
            return_pane,
            ui_window: UiWindowId(0x1000_0000_u64.saturating_add(logical.id.0)),
            window,
            surface,
            accessibility,
            semantics: AccessibilityTree::default(),
            scopes: UiScopes::new(),
            documents,
            chrome_scope,
            title_scope,
            actions: ActionRoute::default(),
            hit_test: HitTestTree::default(),
            dock_frame: DockFrame::default(),
            input: InputOwnership::default(),
            cursor: Point::new(0.0, 0.0),
            pointer_click: PointerClick::default(),
            busy_visible: false,
            caret_blink: CaretBlink::default(),
            page_scroll_item: None,
            last_title_click: None,
            pane_menu: None,
            ramp_track_rect: None,
            ramp_drag: None,
            combo_anchor: None,
            popover_anchor: None,
            text_views: TextViews::default(),
            slider_rect: None,
            slider_drag: None,
            number_scrub: None,
            vector_scrub: None,
            drawer_resize: None,
            table_resize: None,
            table_rect: None,
            tile_columns: 1,
            scroll_geometry: ScrollGeometry::default(),
            focus_rects: HashMap::new(),
            tooltip: WindowTooltip::new(TOOLTIP_DELAY),
            scrollbar_drag: None,
            tab_scroll_rects: HashMap::new(),
            tab_reveal: HashMap::new(),
            area_scroll_y: 0.0,
            area_follow_caret: true,
            area_layout: AreaWrapLayout::default(),
        };
        host.window.set_visible(true);
        host.window.request_redraw();
        Ok(host)
    }
}

pub(crate) struct NativeChrome<'a> {
    pub(crate) scope: ElementScope,
    pub(crate) title: &'a str,
    pub(crate) state: TitleBarState,
}

pub(crate) type NativePaneMenu = WindowMenuSession;

impl NativeFloatingHost {
    pub(crate) fn close_pane_menu(&mut self) {
        if let Some(mut menu) = self.pane_menu.take() {
            self.input.set_focus(
                menu.controller
                    .close()
                    .filter(|target| self.actions.focus_order().contains(&target.element)),
            );
        }
    }
}

impl App {
    pub(super) fn sync_native_windows(&mut self, event_loop: &ActiveEventLoop) {
        let desired = self.demo.workspace.floating_windows().to_vec();
        let desired_ids = desired
            .iter()
            .map(|window| window.id)
            .collect::<HashSet<_>>();
        let owned_overlay_dropped = self.overlay_owner.is_some_and(|owner| {
            self.native_windows
                .values()
                .any(|host| host.ui_window == owner && !desired_ids.contains(&host.logical_id))
        });
        let removed = self
            .native_windows
            .values()
            .filter(|host| !desired_ids.contains(&host.logical_id))
            .map(|host| host.ui_window)
            .collect::<Vec<_>>();
        for window in removed {
            self.ui_query.forget(window);
            self.cancel_window_editors(window);
            self.native_text_bridges.remove(window);
        }
        self.native_windows.retain(|_, host| {
            let keep = desired_ids.contains(&host.logical_id);
            if !keep {
                host.window.set_visible(false);
            }
            keep
        });
        self.native_window_order
            .retain(|id| self.native_windows.contains_key(id));
        if owned_overlay_dropped {
            self.close_owned_overlays();
        }
        let Some(gpu_context) = self.gpu.as_ref().and_then(GpuState::gpu_context) else {
            return;
        };
        if !gpu_context.can_create_renderer() {
            return;
        }
        let Some(proxy) = self.event_proxy.clone() else {
            return;
        };
        let Some(owner) = self.window.clone() else {
            return;
        };
        let Some(return_pane) = self.demo.workspace.root().panes().first().copied() else {
            return;
        };
        for logical in desired {
            if !gpu_context.can_create_renderer() {
                break;
            }
            if self
                .native_windows
                .values()
                .any(|host| host.logical_id == logical.id)
            {
                continue;
            }
            match self.new_window_documents().and_then(|documents| {
                NativeFloatingHost::create(
                    event_loop,
                    proxy.clone(),
                    &logical,
                    return_pane,
                    &owner,
                    Arc::clone(&gpu_context),
                    documents,
                )
            }) {
                Ok(host) => {
                    host.window.set_title(&format!(
                        "{} — Vesta",
                        display_text(&logical.title, self.demo.locale)
                    ));
                    self.native_window_order.push(host.window.id());
                    if let Err(error) = self.native_text_bridges.register(host.ui_window) {
                        self.fatal_error = Some(format!("text_bridge_registration: {error:?}"));
                        return;
                    }
                    self.native_windows.insert(host.window.id(), host);
                }
                Err(error) => {
                    eprintln!("native floating window {}: {error}", logical.id.0);
                }
            }
        }
    }
}

pub(crate) fn build_native_floating_root(
    scope: ElementScope,
    mut content: Element,
    mut actions: Vec<(ElementId, DemoAction)>,
    theme: WidgetTheme,
    chrome: NativeChrome<'_>,
) -> (Element, Vec<(ElementId, DemoAction)>) {
    content.style.height = Dimension::Auto;
    content.style.min_height = Dimension::Px(0.0);
    content.style.flex_basis = Some(Dimension::Px(0.0));
    content.style.flex_grow = 1.0;
    content.style.flex_shrink = 1.0;
    let title = title_bar(
        chrome.scope,
        "Vesta",
        chrome.title,
        chrome.state,
        theme,
        TitleBarLabels {
            minimize: "Minimize window",
            maximize: "Maximize or restore window",
            restore: "Maximize or restore window",
            close: "Close window",
        },
        TitleBarActions {
            minimize: DemoAction::WindowMinimize,
            maximize: DemoAction::WindowMaximize,
            close: DemoAction::WindowClose,
        },
    );
    actions.extend(title.actions);
    let root = Element::container(scope.element(1), ElementKey::new("native-floating.root"))
        .semantic(Semantic::new(SemanticRole::Group, "Floating workspace"))
        .styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                .axis(Axis::Column)
                .background(theme.palette.canvas)
                .gap(0.0)
                .pointer_events(false),
        )
        .child(title.element)
        .child(content);
    (root, actions)
}

/// Both hosts apply the same client chrome rules through safe winit APIs.
pub(crate) fn try_window_chrome_press(
    window: &Window,
    cursor: Point,
    hit_test: &HitTestTree,
    actions: &ActionRoute<DemoAction>,
    pointer_click: &mut PointerClick,
    last_title_click: &mut Option<(Instant, Point)>,
) -> bool {
    if !cfg!(target_os = "windows") {
        return false;
    }
    let size = window.inner_size().to_logical::<f32>(window.scale_factor());
    if !window.is_maximized()
        && let Some(direction) = chrome_resize_direction(cursor, Size::new(size.width, size.height))
    {
        pointer_click.cancel();
        *last_title_click = None;
        if let Err(error) = window.drag_resize_window(direction) {
            eprintln!("window resize: {error}");
        }
        return true;
    }
    if cursor.y < 0.0
        || cursor.y >= CHROME_HEIGHT
        || hit_test
            .hit(cursor)
            .is_some_and(|hit| actions.action_for_hit(hit).is_some())
    {
        return false;
    }
    pointer_click.cancel();
    let now = Instant::now();
    let double_click = last_title_click.is_some_and(|(at, position)| {
        now.duration_since(at) <= Duration::from_millis(450)
            && (cursor.x - position.x).abs() <= 5.0
            && (cursor.y - position.y).abs() <= 5.0
    });
    if double_click {
        *last_title_click = None;
        window.set_maximized(!window.is_maximized());
        window.request_redraw();
    } else {
        *last_title_click = Some((now, cursor));
        if let Err(error) = window.drag_window() {
            eprintln!("window drag: {error}");
        }
    }
    true
}

impl App {
    /// Native thumb ownership precedes chrome when their painted bounds overlap.
    pub(super) fn try_native_primary_gesture(&mut self) -> bool {
        if self.ui_query.input_origin == vesta_gpui_winit::UiInputOrigin::Command {
            return false;
        }
        self.pointer_click.cancel();
        if self.try_scrollbar_press() {
            self.request_redraw();
            return true;
        }
        let Some(window) = &self.window else {
            return false;
        };
        try_window_chrome_press(
            window,
            self.cursor,
            &self.hit_test,
            &self.ui_actions,
            &mut self.pointer_click,
            &mut self.last_title_click,
        )
    }

    pub(super) fn apply_native_action(&mut self, window_id: WindowId, action: DemoAction) {
        // Actions from this window own any overlay they open (ADR-0254).
        if let Some(host) = self.native_windows.get(&window_id) {
            self.action_window = host.ui_window;
        }
        if !self.prepare_window_menu_action(self.action_window, action) {
            return;
        }
        if let DemoAction::Workspace(WorkspaceViewAction::HeaderCommand {
            pane,
            active,
            command: HEADER_OVERFLOW,
        }) = action
        {
            if self.demo.workspace.pane(pane).and_then(|pane| pane.active) != Some(active) {
                return;
            }
            if let Some(host) = self.native_windows.get(&window_id) {
                let context = MenuContext::capture(
                    &self.demo,
                    ContainerRef::Floating(host.logical_id),
                    pane,
                    None,
                );
                self.open_window_menu(
                    host.ui_window,
                    MenuId(5),
                    Point::new(host.cursor.x, host.cursor.y + 12.0),
                    context,
                );
            }
            return;
        }
        if matches!(
            action,
            DemoAction::ToggleMenu | DemoAction::OpenMenu(_) | DemoAction::OpenSubmenu(_)
        ) {
            let Some(host) = self.native_windows.get(&window_id) else {
                return;
            };
            let window = host.ui_window;
            let anchor = Point::new(host.cursor.x, host.cursor.y + 12.0);
            if let DemoAction::OpenSubmenu(id) = action {
                self.enter_window_submenu(window, id);
            } else {
                let id = match action {
                    DemoAction::OpenMenu(id) => id,
                    _ => MenuId(1),
                };
                if self
                    .window_menu_snapshot(window)
                    .is_some_and(|menu| menu.controller.path().first() == Some(&id))
                {
                    self.close_window_menu(window, PopupCloseReason::Explicit);
                } else if let Some(context) = self.pane_context_at(window, None) {
                    self.open_window_menu(window, id, anchor, context);
                }
            }
            return;
        }
        if let Some(host) = self.native_windows.get_mut(&window_id) {
            host.close_pane_menu();
        }
        match action {
            DemoAction::WindowClose => self.restore_native_window(window_id),
            DemoAction::WindowMinimize | DemoAction::WindowMaximize => {
                if action == DemoAction::WindowMinimize
                    && let Some(owner) = self
                        .native_windows
                        .get(&window_id)
                        .map(|host| host.ui_window)
                {
                    self.cancel_window_workspace_drag(owner);
                }
                if let Some(host) = self.native_windows.get(&window_id) {
                    if action == DemoAction::WindowMinimize {
                        host.window.set_minimized(true);
                    } else {
                        host.window.set_maximized(!host.window.is_maximized());
                    }
                    host.window.request_redraw();
                }
            }
            _ => self.apply_demo_action(action),
        }
    }
}
