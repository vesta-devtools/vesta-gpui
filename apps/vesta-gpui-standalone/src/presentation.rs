//! Stateless example UI composition. Application/input/GPU ownership stays with App.
use super::*;

pub(super) fn dialog_element(scope: ElementScope, id: u8, slot: u32) -> ElementId {
    scope.element(DIALOG_ROOT + u32::from(id) * 20 + slot)
}

pub(super) fn accessibility_root(root: &Element, isolated_root: Option<ElementId>) -> &Element {
    isolated_root.and_then(|id| root.find(id)).unwrap_or(root)
}

pub(super) fn table_resize_element(scope: ElementScope, column: usize) -> ElementId {
    scope.element(TABLE_ROOT + 20 + column as u32)
}

pub(super) fn dialog_ids(scope: ElementScope, id: u8) -> ModalIds {
    let identity = |slot, part| {
        WidgetIdentity::new(
            dialog_element(scope, id, slot),
            format!("dialog.{id}.{part}"),
        )
    };
    ModalIds {
        backdrop: identity(0, "backdrop"),
        panel: identity(1, "panel"),
        title: identity(2, "title"),
        body: identity(3, "body"),
        footer: identity(4, "footer"),
        confirm: identity(5, "confirm"),
        confirm_label: identity(6, "confirm.label"),
        cancel: identity(7, "cancel"),
        cancel_label: identity(8, "cancel.label"),
        secondary: identity(9, "secondary"),
        secondary_label: identity(10, "secondary.label"),
    }
}

pub(super) fn theme_clear(theme: WidgetTheme, viewport: bool) -> [f32; 4] {
    let color = if viewport {
        match theme.mode {
            ThemeMode::Dark => Color::rgb8(29, 31, 34),
            ThemeMode::Light => Color::rgb8(209, 212, 216),
        }
    } else {
        theme.palette.canvas
    };
    vesta_gpui::render::linear_color(color)
}

pub(super) struct BuiltUi {
    pub(super) root: Element,
    pub(super) actions: Vec<(ElementId, DemoAction)>,
    pub(super) busy_roots: Option<[ElementId; 2]>,
    pub(super) slider: Option<ElementId>,
    pub(super) ramp_track: Option<ElementId>,
    pub(super) workspace_regions: WorkspaceViewRegions,
    pub(super) tile_columns: usize,
}

pub(super) struct GalleryUi {
    pub(super) element: Element,
    pub(super) actions: Vec<(ElementId, DemoAction)>,
    pub(super) busy_roots: [ElementId; 2],
    pub(super) slider: ElementId,
    pub(super) ramp_track: ElementId,
    pub(super) workspace_regions: WorkspaceViewRegions,
    pub(super) tile_columns: usize,
}

pub(super) struct ViewportUi {
    pub(super) element: Element,
    pub(super) actions: Vec<(ElementId, DemoAction)>,
}

pub(super) struct UiBuildState<'a> {
    pub(super) documents: &'a GalleryDocuments,
    pub(super) selected_feedback: bool,
    pub(super) ui_window: UiWindowId,
    pub(super) menu_open: bool,
    pub(super) scopes: UiScopes,
    pub(super) workspace_container: ContainerRef,
    pub(super) mode: AcceptanceMode,
    pub(super) maximized: bool,
    pub(super) theme: WidgetTheme,
    pub(super) scroll_offset: Point,
    pub(super) viewport_size: Size,
    pub(super) snapshot: vesta_gpui::TextInputSnapshot,
    pub(super) focused: bool,
    pub(super) search_snapshot: vesta_gpui::TextInputSnapshot,
    pub(super) search_query: &'a str,
    pub(super) search_focused: bool,
    pub(super) area_snapshot: vesta_gpui::TextInputSnapshot,
    pub(super) area_focused: bool,
    pub(super) number_snapshot: vesta_gpui::TextInputSnapshot,
    pub(super) number_focused: bool,
    pub(super) vector_snapshots: [vesta_gpui::TextInputSnapshot; 3],
    pub(super) vector_focused: [bool; 3],
    pub(super) color_hex_snapshot: vesta_gpui::TextInputSnapshot,
    pub(super) password_mask: String,
    pub(super) password_snapshot: vesta_gpui::TextInputSnapshot,
    pub(super) table_note_snapshot: vesta_gpui::TextInputSnapshot,
    pub(super) color_hex_focused: bool,
    pub(super) table_note_focused: bool,
    pub(super) readonly_label: &'a TextInputState,
    pub(super) readonly_label_focused: bool,
    pub(super) virtual_focused: Option<usize>,
    pub(super) virtual_table_focused: Option<usize>,
    pub(super) virtual_table_viewport_height: f32,
    pub(super) tree_focused: Option<usize>,
    pub(super) demo: &'a DemoState,
    pub(super) hovered: Option<ElementId>,
    pub(super) focused_element: Option<ElementId>,
    pub(super) dock_drag: Option<DockDragVisual>,
    pub(super) popover_anchor: Option<Rect>,
    pub(super) combo_anchor: Option<Rect>,
    pub(super) gizmo_mode: TransformMode,
    pub(super) frame_stats: Option<(f32, f32)>,
    /// The window that owns the palette or modal dialog, if any (ADR-0254).
    pub(super) overlay_owner: Option<UiWindowId>,
}

pub(super) fn build_root(input: UiBuildState<'_>, text_context: &mut TextInputContext) -> BuiltUi {
    let scopes = input.scopes;
    let mode = input.mode;
    let theme = input.theme;
    let (header, header_actions) = build_header(scopes.header, &input);
    let (content, mut actions, slider, ramp_track, workspace_regions, tile_columns, busy_roots) =
        match mode {
            AcceptanceMode::Gallery2d => {
                let gallery = build_gallery(&input);
                (
                    gallery.element,
                    gallery.actions,
                    Some(gallery.slider),
                    Some(gallery.ramp_track),
                    gallery.workspace_regions,
                    gallery.tile_columns,
                    Some(gallery.busy_roots),
                )
            }
            AcceptanceMode::Viewport3d => {
                let viewport = build_viewport_overlays(scopes.viewport, theme, input.gizmo_mode);
                (
                    viewport.element,
                    viewport.actions,
                    None,
                    None,
                    WorkspaceViewRegions::default(),
                    1,
                    None,
                )
            }
        };
    actions.extend(header_actions);
    let mut status = build_status(scopes.status, &input);
    status.element.style = status.element.style.clone().absolute(
        0.0,
        (input.viewport_size.height - CHROME_STATUS_HEIGHT).max(0.0),
    );
    status.element.style.z_index = 110;
    actions.extend(status.actions);
    let mut root = Element::container(scopes.root.element(1), ElementKey::new("root"))
        .styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                .pointer_events(false),
        )
        .child(header)
        .child(content);
    if let Some(drag) = &input.dock_drag {
        root = root.child(dock_drag_overlay(
            scopes.root,
            drag,
            theme,
            input.viewport_size,
        ));
    }
    if input.demo.menu.is_open() {
        root = root.child(build_menu_popup(
            scopes.menu,
            theme,
            input.demo,
            input.hovered,
            input.viewport_size,
            &mut actions,
            text_context,
        ));
    }
    root = root.child(status.element);
    if input.overlay_owner == Some(STANDALONE_WINDOW)
        && let Some(overlays) = build_owned_overlays(
            scopes,
            input.demo,
            input.search_query,
            &input.search_snapshot,
            input.search_focused,
            input.viewport_size,
            theme,
            &mut actions,
        )
    {
        root = root.child(overlays);
    }
    localize_tree(&mut root, input.demo.locale);
    BuiltUi {
        root,
        actions,
        busy_roots,
        slider,
        ramp_track,
        workspace_regions,
        tile_columns,
    }
}

/// The command palette and the top modal dialog, projected into the window that opened them
/// (ADR-0254). Returns `None` when that window has no overlay to show.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_owned_overlays(
    scopes: UiScopes,
    demo: &DemoState,
    search_query: &str,
    search_snapshot: &vesta_gpui::TextInputSnapshot,
    search_focused: bool,
    viewport_size: Size,
    theme: WidgetTheme,
    actions: &mut Vec<(ElementId, DemoAction)>,
) -> Option<Element> {
    let mut overlay = Element::container(
        scopes.root.element(OWNED_OVERLAY_ROOT),
        ElementKey::new("owned.overlays"),
    )
    .styled(
        Style::default()
            .absolute(0.0, 0.0)
            .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
            .z_index(if demo.palette_session.is_open() {
                190
            } else {
                0
            })
            .pointer_events(false),
    );
    let mut built = false;
    if let Some(id) = demo.dialog_stack.top() {
        let dialog = modal_dialog(
            dialog_ids(scopes.root, id),
            viewport_size,
            ModalDialogSpec {
                title: if id == 1 {
                    "Gallery confirmation"
                } else {
                    "Nested confirmation"
                },
                body: if id == 1 {
                    "Confirm adds one to the sample value. The nested dialog closes first."
                } else {
                    "This dialog owns focus until it is confirmed or cancelled."
                },
                confirm: DemoAction::ConfirmDialog(id),
                cancel: DemoAction::CancelDialog(id),
                secondary: (id == 1).then_some(("Open nested", DemoAction::OpenNestedDialog)),
            },
            theme,
        );
        actions.extend(dialog.actions);
        overlay = overlay.child(dialog.element);
        built = true;
    }
    if demo.palette_session.is_open() {
        let palette = palette_demo::projection(
            scopes,
            demo,
            search_snapshot,
            search_query,
            search_focused,
            viewport_size,
            theme,
        );
        actions.extend(palette.actions);
        overlay = overlay.child(palette.element);
        built = true;
    }
    built.then_some(overlay)
}

pub(super) fn build_status(scope: ElementScope, input: &UiBuildState<'_>) -> Widget<DemoAction> {
    let button = |id, caption, name, action| {
        chrome_button(
            WidgetIdentity::new(scope.element(id), format!("status.control.{id}")),
            WidgetIdentity::new(scope.element(id + 1), format!("status.label.{id}")),
            caption,
            name,
            action,
            ChromeButtonStyle {
                role: ChromeButtonRole::Status,
                width: CHROME_STATUS_CONTROL_WIDTH,
                selected: false,
                hovered: input.hovered == Some(scope.element(id)),
                destructive: false,
            },
            input.theme,
        )
    };
    let mode = match input.mode {
        AcceptanceMode::Gallery2d => "2D Gallery",
        AcceptanceMode::Viewport3d => "3D Viewport",
    };
    let size = format!(
        "{} × {}",
        input.viewport_size.width.round() as u32,
        input.viewport_size.height.round() as u32
    );
    let frame_rate = input.frame_stats.map_or_else(
        || "UI -- FPS  --.- ms".to_owned(),
        |(fps, interval_ms)| format!("UI last {fps:.0} FPS  {interval_ms:.1} ms"),
    );
    let partition = partition_status_items(
        vec![
            StatusItem {
                widget: status_label(scope, 20, "status.mode", mode, input.theme),
                width: 98.0,
                priority: 0,
                overflow_label: mode.to_owned(),
                overflow_action: input
                    .demo
                    .viewport3d_available
                    .then_some(DemoAction::SelectMode(match input.mode {
                        AcceptanceMode::Gallery2d => AcceptanceMode::Viewport3d,
                        AcceptanceMode::Viewport3d => AcceptanceMode::Gallery2d,
                    })),
            },
            StatusItem {
                widget: status_label(scope, 21, "status.size", &size, input.theme),
                width: 94.0,
                priority: 1,
                overflow_label: size,
                overflow_action: None,
            },
            StatusItem {
                widget: status_label(scope, 22, "status.frame-rate", &frame_rate, input.theme),
                width: 144.0,
                priority: 3,
                overflow_label: frame_rate,
                overflow_action: None,
            },
        ],
        input.viewport_size.width - 88.0,
        CHROME_STATUS_CONTROL_WIDTH,
    );
    let mut right = partition.visible;
    if !partition.overflow.is_empty() {
        let mut overflow = button(
            30,
            "⋯",
            "More status items",
            DemoAction::ToggleStatusOverflow,
        );
        overflow.element.semantic = Some(
            Semantic::new(SemanticRole::Button, "More status items")
                .expanded(input.demo.status_overflow_open),
        );
        right.push(overflow);
    }
    let mut bar = status_bar(
        scope,
        vec![
            button(10, "↺", "Reset workspace", DemoAction::WorkspaceReset),
            status_separator(scope, 14, "status.group.divider", input.theme),
            button(
                12,
                "◐",
                "Toggle light and dark theme",
                DemoAction::ToggleTheme,
            ),
        ],
        right,
        input.theme,
    );
    if input.demo.status_overflow_open && !partition.overflow.is_empty() {
        let popup_height = partition.overflow.len() as f32 * CHROME_STATUS_HEIGHT;
        let mut popup = Widget::from_element(
            Element::container(scope.element(40), ElementKey::new("status.overflow"))
                .semantic(Semantic::new(SemanticRole::Menu, "More status items"))
                .styled(
                    Style::default()
                        .absolute(
                            (input.viewport_size.width - 196.0).max(0.0),
                            -popup_height - 4.0,
                        )
                        .size(Dimension::Px(190.0), Dimension::Px(popup_height))
                        .axis(Axis::Column)
                        .background(input.theme.palette.menu_surface)
                        .border(1.0, input.theme.palette.border)
                        .no_clip(),
                ),
        );
        popup.element.style.z_index = 120;
        for (index, entry) in partition.overflow.into_iter().enumerate() {
            let id = 50 + index as u32 * 2;
            let item = if let Some(action) = entry.action {
                let mut item = chrome_button(
                    WidgetIdentity::new(scope.element(id), format!("status.overflow.{id}")),
                    WidgetIdentity::new(
                        scope.element(id + 1),
                        format!("status.overflow.{id}.label"),
                    ),
                    &entry.label,
                    &entry.label,
                    action,
                    ChromeButtonStyle {
                        role: ChromeButtonRole::Status,
                        width: 190.0,
                        selected: false,
                        hovered: input.hovered == Some(scope.element(id)),
                        destructive: false,
                    },
                    input.theme,
                );
                item.element.semantic = Some(Semantic::new(SemanticRole::MenuItem, &entry.label));
                item
            } else {
                status_label(
                    scope,
                    id,
                    &format!("status.overflow.{id}"),
                    &entry.label,
                    input.theme,
                )
            };
            popup = popup.child(item);
        }
        bar = bar.child(popup);
    }
    bar
}

pub(super) fn build_combo_popup(
    scope: ElementScope,
    ids: &mut Ids,
    theme: WidgetTheme,
    selected: u8,
    anchor: Rect,
    bounds: Rect,
) -> Widget<DemoAction> {
    let mut content = Widget::from_element(
        Element::container(
            scope.element(COMBO_POPUP + 1),
            ElementKey::new("combo.options"),
        )
        .semantic(Semantic::new(SemanticRole::ListBox, "Projection"))
        .styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                .axis(Axis::Column)
                .gap(3.0)
                .pointer_events(false),
        ),
    );
    for (index, name) in COMBO_VALUES.into_iter().enumerate() {
        content = content.child(combo_option(
            WidgetIdentity::new(
                scope.element(COMBO_OPTION_BASE + index as u32),
                format!("combo.option.{index}"),
            ),
            ids.identity("combo.option.label"),
            name,
            selected == index as u8,
            DemoAction::SelectCombo(index as u8),
            ControlState::Idle,
            theme,
        ));
    }
    popover(
        WidgetIdentity::new(scope.element(COMBO_POPUP), "combo.popup"),
        content,
        anchor,
        bounds,
        Size::new(
            220.0,
            COMBO_VALUES.len() as f32 * (theme.metrics.control_height + 3.0) + 7.0,
        ),
        theme,
    )
}

pub(super) fn build_popover(
    scope: ElementScope,
    theme: WidgetTheme,
    number: i32,
    anchor: Rect,
    bounds: Rect,
) -> Widget<DemoAction> {
    let mut ids = Ids::new(scope, 2);
    let detail = Element::text(
        ids.id(),
        ids.key("popover.value"),
        format!("Current value: {number}"),
    )
    .styled(
        Style::default()
            .size(Dimension::Percent(1.0), Dimension::Px(30.0))
            .text(theme.typography.font_size, theme.palette.text)
            .pointer_events(false),
    );
    let button = button(
        WidgetIdentity::new(scope.element(POPOVER_BUTTON), "popover.increase"),
        WidgetIdentity::new(scope.element(POPOVER_BUTTON + 1), "popover.increase.label"),
        "Increase value",
        DemoAction::PopoverStep,
        ControlState::Idle,
        theme,
    );
    let content = Widget::from_element(
        Element::container(scope.element(6), ElementKey::new("popover.content"))
            .styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                    .axis(Axis::Column)
                    .gap(6.0)
                    .pointer_events(false),
            )
            .child(detail),
    )
    .child(button);
    popover(
        WidgetIdentity::new(scope.element(POPOVER_ROOT), "popover.root"),
        content,
        anchor,
        bounds,
        Size::new(220.0, 90.0),
        theme,
    )
}

pub(super) fn widget_element(
    widget: Widget<DemoAction>,
    actions: &mut Vec<(ElementId, DemoAction)>,
) -> Element {
    actions.extend(widget.actions);
    widget.element
}

pub(super) fn page_scroll_id(scope: ElementScope, item: ItemId) -> ElementId {
    scope.element(
        GALLERY_PAGE_SCROLL_BASE + u32::try_from(item.0).expect("bounded workspace item ID"),
    )
}

pub(super) fn build_viewport_overlays(
    scope: ElementScope,
    theme: WidgetTheme,
    gizmo_mode: TransformMode,
) -> ViewportUi {
    let mut actions = Vec::new();
    let mut card_style = Style::default()
        .size(Dimension::Px(360.0), Dimension::Px(130.0))
        .axis(Axis::Column)
        .gap(6.0)
        .padding(Insets::all(12.0))
        .background(theme.palette.overlay)
        .border(1.0, theme.palette.border)
        .radius(theme.metrics.radius);
    card_style.position = PositionMode::Absolute;
    card_style.left = Some(18.0);
    card_style.bottom = Some(18.0);
    let card = Element::container(scope.element(300), ElementKey::new("viewport.help"))
        .styled(card_style)
        .child(text(
            scope.element(301),
            ElementKey::new("viewport.help.title"),
            "Viewport Overlay",
            15.0,
            theme.palette.text,
        ))
        .child(text(
            scope.element(302),
            ElementKey::new("viewport.help.orbit"),
            "Left drag: Orbit   Middle drag: Pan",
            12.0,
            theme.palette.text_muted,
        ))
        .child(text(
            scope.element(303),
            ElementKey::new("viewport.help.camera"),
            "Wheel: Dolly   F: Frame   O: Projection",
            12.0,
            theme.palette.text_muted,
        ))
        .child(text(
            scope.element(304),
            ElementKey::new("viewport.help.layers"),
            "Grid → Selection/Gizmo → UI Overlay",
            12.0,
            theme.palette.text_muted,
        ));
    let mut cube_style = Style::default()
        .size(Dimension::Px(86.0), Dimension::Px(86.0))
        .align(Align::Center)
        .justify(Justify::Center)
        .background(theme.palette.overlay)
        .border(1.0, theme.palette.focus)
        .radius(theme.metrics.radius);
    cube_style.position = PositionMode::Absolute;
    cube_style.right = Some(18.0);
    cube_style.top = Some(70.0);
    let cube = Element::container(scope.element(310), ElementKey::new("viewport.view-cube"))
        .styled(cube_style)
        .child(text(
            scope.element(311),
            ElementKey::new("viewport.view-cube.label"),
            "TOP\nFRONT",
            11.0,
            theme.palette.text,
        ));
    let mut toolbar_inner =
        Element::container(scope.element(321), ElementKey::new("viewport.toolbar")).styled(
            Style::default()
                .size(Dimension::Px(520.0), Dimension::Px(40.0))
                .axis(Axis::Row)
                .align(Align::Center)
                .justify(Justify::Center)
                .gap(5.0)
                .padding(Insets::symmetric(5.0, 4.0))
                .background(theme.palette.overlay)
                .border(1.0, theme.palette.border)
                .radius(theme.metrics.radius)
                .pointer_events(false),
        );
    let specifications = [
        (
            "Select",
            DemoAction::ViewportTool(TransformMode::Select),
            gizmo_mode == TransformMode::Select,
        ),
        (
            "Move",
            DemoAction::ViewportTool(TransformMode::Translate),
            gizmo_mode == TransformMode::Translate,
        ),
        (
            "Rotate",
            DemoAction::ViewportTool(TransformMode::Rotate),
            gizmo_mode == TransformMode::Rotate,
        ),
        (
            "Scale",
            DemoAction::ViewportTool(TransformMode::Scale),
            gizmo_mode == TransformMode::Scale,
        ),
        ("Reset camera", DemoAction::ViewportResetCamera, false),
        ("Proj", DemoAction::ViewportProjection, false),
    ];
    for (index, (name, action, selected)) in specifications.into_iter().enumerate() {
        let local = 330 + index as u32 * 2;
        let widget = button(
            WidgetIdentity::new(
                scope.element(local),
                format!("viewport.toolbar.button.{index}"),
            ),
            WidgetIdentity::new(
                scope.element(local + 1),
                format!("viewport.toolbar.button.{index}.label"),
            ),
            name,
            action,
            if selected {
                ControlState::Pressed
            } else {
                ControlState::Idle
            },
            theme,
        );
        let mut element = widget_element(widget, &mut actions);
        element.style.width = Dimension::Px(76.0);
        toolbar_inner = toolbar_inner.child(element);
    }
    let toolbar = Element::container(scope.element(320), ElementKey::new("viewport.toolbar-row"))
        .styled(
            Style::default()
                .absolute(0.0, 70.0)
                .size(Dimension::Percent(1.0), Dimension::Px(38.0))
                .axis(Axis::Row)
                .align(Align::Center)
                .justify(Justify::Center)
                .pointer_events(false),
        )
        .child(toolbar_inner);
    ViewportUi {
        element: Element::container(scope.element(4), ElementKey::new("viewport.overlays"))
            .styled(
                Style::default()
                    .absolute(0.0, 0.0)
                    .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                    .pointer_events(false),
            )
            .child(card)
            .child(cube)
            .child(toolbar),
        actions,
    }
}

pub(super) fn section(
    ids: &mut Ids,
    heading: &str,
    controls: Vec<Element>,
    theme: WidgetTheme,
) -> Element {
    section_with_axis(ids, heading, controls, theme, Axis::Column)
}

pub(super) fn section_with_axis(
    ids: &mut Ids,
    heading: &str,
    controls: Vec<Element>,
    theme: WidgetTheme,
    axis: Axis,
) -> Element {
    let stacked = axis == Axis::Column;
    let mut section = Element::container(ids.id(), ids.key("section"))
        .semantic(Semantic::new(SemanticRole::Group, heading))
        .styled(
            Style::default()
                .size(
                    Dimension::Percent(1.0),
                    if stacked {
                        Dimension::Auto
                    } else {
                        Dimension::Px(104.0)
                    },
                )
                .axis(Axis::Column)
                .gap(8.0)
                .padding(Insets::all(4.0))
                .background(theme.palette.panel),
        )
        .child(
            Element::container(ids.id(), ids.key("section.row"))
                .styled(
                    Style::default()
                        .size(Dimension::Percent(1.0), Dimension::Auto)
                        .min_width(Dimension::Px(0.0))
                        .axis(axis)
                        .gap(8.0)
                        .align(if stacked { Align::Start } else { Align::Center }),
                )
                .children(controls),
        );
    section.style.clip = ClipMode::Inherit;
    section.children[0].style.clip = ClipMode::Inherit;
    section
}

pub(super) fn control(
    ids: &mut Ids,
    label_text: &str,
    background: Color,
    theme: WidgetTheme,
) -> Element {
    Element::container(ids.id(), ids.key("control"))
        .styled(
            Style::default()
                .size(Dimension::Auto, Dimension::Px(theme.metrics.control_height))
                .padding(Insets::symmetric(12.0, 0.0))
                .align(Align::Center)
                .justify(Justify::Center)
                .background(background)
                .border(1.0, theme.palette.border)
                .radius(theme.metrics.radius),
        )
        .child(label(ids, label_text, theme.palette.text, theme))
}

pub(super) fn label(ids: &mut Ids, value: &str, color: Color, theme: WidgetTheme) -> Element {
    text(
        ids.id(),
        ids.key("label"),
        value,
        theme.typography.font_size,
        color,
    )
}

pub(super) fn text(
    id: ElementId,
    key: ElementKey,
    value: &str,
    size: f32,
    color: Color,
) -> Element {
    Element::text(id, key, value).styled(
        Style::default()
            .size(Dimension::Auto, Dimension::Px((size * 1.5).max(18.0)))
            .text(size, color)
            .text_align(TextHAlign::Start, TextVAlign::Center)
            .pointer_events(false),
    )
}

pub(super) struct Ids {
    scope: ElementScope,
    next: u32,
}

impl Ids {
    pub(super) const fn new(scope: ElementScope, first: u32) -> Self {
        Self { scope, next: first }
    }

    pub(super) fn id(&mut self) -> ElementId {
        let id = self.scope.element(self.next);
        self.next += 1;
        id
    }

    pub(super) fn identity(&mut self, prefix: &str) -> WidgetIdentity {
        let local = self.next;
        self.next += 1;
        let identity = self.scope.identity(local, format!("{prefix}.{local}"));
        WidgetIdentity {
            element: identity.id,
            key: identity.key,
        }
    }

    pub(super) fn key(&self, prefix: &str) -> ElementKey {
        let local = self.next.saturating_sub(1);
        self.scope.identity(local, format!("{prefix}.{local}")).key
    }
}
