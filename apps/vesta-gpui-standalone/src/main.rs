#![forbid(unsafe_code)]

mod application_chrome;
mod application_run;
mod breadcrumb_demo;
mod color_demo;
mod conditional_controls;
mod demo_actions;
mod demo_images;
mod dock_drag;
mod frame_profile;
mod gallery;
mod gallery_sections;
mod gpu_lifecycle;
mod gpu_profile;
mod gpu_recovery;
mod host_fonts;
mod lifecycle_profile;
mod localization;
mod native_capture;
mod native_window;
mod numeric_demo;
mod palette_demo;
mod ramp_demo;
mod render_backend;
mod scroll_geometry;
mod slider_demo;
mod suggestion_demo;
#[cfg(feature = "gpu-fault-injection")]
mod target_reconfiguration;
mod temporary_text;
mod text_fields;
mod toolbar_demo;
mod transparent_fixture;
mod tree_collections;
mod ui_preferences;
mod ui_query;
mod window_accessibility;
mod window_collections;
mod window_controls;
use window_controls::{
    TableResizeDrag, table_column_fractions_after, table_column_widths, table_resize_width_bounds,
};
mod window_documents;
mod window_editors;
mod window_events;
mod window_focus;
mod window_idle;
mod window_keyboard;
mod window_menus;
mod window_notifications;
mod window_platform;
mod window_tooltips;

mod workspace_session;

use application_chrome::build_header;
use color_demo::build_color_picker;
use demo_actions::DemoAction;
use demo_images::DemoImages;
use dock_drag::*;
use frame_profile::{
    FrameContext, FrameMeter, FrameProfiler, FrameStage, NativeGeometryEvent, NativeObservation,
    NativeProjection,
};
use gallery::build_gallery;
use gallery::documents::GalleryDocuments;
use host_fonts::register_host_fonts;
use localization::{UiLocale, display_text, localize_tree};
use native_window::NativeFloatingHost;
use numeric_demo::{number_scrub_value, vector_scrub_value};
use ramp_demo::*;
use render_backend::{GpuState, RenderFrame, RenderOptions};
use scroll_geometry::{
    ScrollGeometry, ScrollStates, ScrollTarget, TabRevealGeometry, reveal_active_tabs,
};
use text_fields::{AreaWrapLayout, TextFieldKind, restore_area_view_layout};
use vesta_gpui::{TextSessionTransition, WindowTextSession};
use vesta_gpui_widgets::MenuKeyAction;
use vesta_gpui_widgets::{
    ChoiceBoundary, ChoiceKeyAction, PopupCloseReason, choice_key_action, format_rgba_hex,
    parse_rgba_hex, rgba_to_hsva,
};
use vesta_gpui_widgets::{
    DraftResolution, HorizontalEditSession, PointerEditOutcome, ValueEditSession,
    resolve_value_draft,
};
use vesta_gpui_widgets::{PopupKeyAction, popup_dismiss_for_hit, popup_key_action};
use vesta_gpui_widgets::{TooltipController as WindowTooltip, TooltipFrame};
use vesta_gpui_widgets::{TreeKeyAction, linear_collection_index};
use vesta_gpui_widgets::{WidgetContent, list_item_from};
use vesta_gpui_winit::UiInputEvent as WindowEvent;
use window_accessibility::{TextFieldStates, enrich_window_text};
#[cfg(test)]
use window_collections::collection_focus;
use window_collections::{CollectionDataAction, CollectionFocus, collection_projection_focus};
use window_focus::{CaretRevealFrame, complete_caret_reveal, retire_caret_reveal};
use window_focus::{TextSessionExit, TextViews, text_views_for};
use window_idle::CaretBlink;
use window_menus::*;

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    sync::Arc,
    time::{Duration, Instant},
};

use accesskit::{Action as AccessibilityAction, ActionData as AccessibilityActionData};

use vesta_gpui::{
    AccessibilityTree, ActionRoute, Align, Axis, ClipMode, Color, Dimension, Element, ElementId,
    ElementIdAllocator, ElementKey, ElementScope, FlowDirection, FocusScope, GridCell, GridTrack,
    HitTestTree, InputOwnership, InputTarget, Insets, Justify, KeyChord, KeyModifiers, KeyStroke,
    Keymap, LayoutNode, LayoutTree, Menu, MenuId, MenuItem, MenuItemId, PaintInteraction,
    PaintList, Point, PointerButton, PointerClick, PositionMode, Rect, ScrollbarStyle, Semantic,
    SemanticRole, Size, Style, TextHAlign, TextInputContext, TextInputState, TextRasterizer,
    TextVAlign, UiWindowId, Viewport, text_input_wrap_width,
};
#[cfg(test)]
use vesta_gpui::{FontRole, LayoutEngine};
use vesta_gpui_widgets::{
    BinaryState, BreadcrumbIds, BreadcrumbSegment, BusyIndicatorIds, CHROME_STATUS_CONTROL_WIDTH,
    CHROME_STATUS_HEIGHT, CHROME_TITLE_HEIGHT, CHROME_WINDOW_CONTROL_WIDTH, CheckboxIds,
    ChromeButtonRole, ChromeButtonStyle, CollapsibleIds, ColorPickerActions, ColorPickerIds,
    ColorPickerMode, ColorPickerView, ComboBoxIds, CommandPaletteEntry, CommandPaletteIds,
    CompactButtonKind, ControlState, EditableComboIds, ExpandableAreaIds, IconButtonSpec,
    InlineEditIds, InlineEditKeyAction, KeyCaptureAction, KeyChordCapture, KeyChordSelectorView,
    ListSelectionState, MenuController, MenuItemIds, ModalDialogSpec, ModalIds, ModalStack,
    NumberFieldIds, PasswordFieldActions, PasswordFieldIds, PasswordFieldView, PopoverState,
    PopupDirection, PropertyRowIds, PropertyRowState, PropertyValidation, RadioIds, RampIds,
    RampInterpolation, RampPoint, RampSpec, RichTextRun, RichTextTone, ScrollAreaState,
    ScrollRevealViewport, ScrollbarDrag, SearchFieldIds, SelectionModifiers, SidebarDrawerIds,
    SidebarDrawerResizeKey, SidebarDrawerState, SliderIds, SliderKey, StatusItem, StatusTone,
    TableCell, TableCellContent, TableColumn, TableColumnContent, TableColumnResize,
    TableResizeKey, TableRow, TableRowContent, TableWindowIds, TextContent, ThemeMode,
    TileGridContent, TileGridDirection, TileGridGeometry, TileGridIds, ToastId, ToastIds,
    ToastNotice, ToastQueue, ToggleIds, ToolbarIds, ToolbarItem, TreeExpansionState,
    TreeRowContentIds, TreeViewEntry, TreeViewRowActions, TreeViewRowState, ValidationSeverity,
    VectorComponentEdit, VectorComponentValue, VectorFieldEdit, VectorFieldIds, VectorFieldSpec,
    VirtualListIds, VirtualListRange, VisibleTree, Widget, WidgetAvailability, WidgetIdentity,
    WidgetTheme, breadcrumb, busy_indicator, button, checkbox, chrome_button, collapsible,
    color_picker, combo_box, combo_option, editable_number_field, editable_vector_field,
    expandable_area, icon_button, inline_edit_key_action, inline_editable_label,
    key_chord_selector, list_item, modal_dialog, partition_status_items, password_field, popover,
    popover_trigger, property_row, property_row_widget, ramp_editor, ramp_handle_x,
    ramp_position_for_pointer, rich_text, scroll_reveal_path, search_field, sidebar_drawer,
    sidebar_drawer_width_at, sidebar_drawer_width_for_key, slider, slider_value_at,
    slider_value_for_key, status_badge, status_bar, status_label, status_separator, tab,
    table_column_width_at, table_column_width_for_key, table_view_windowed_scrolled,
    table_view_with_content, text_area, text_field, text_label, tile_grid_next_index,
    tile_grid_windowed_with_content, toast_widget, toggle, toggle_button, toolbar,
    tree_view_row_from, tree_view_windowed, virtual_list_with_items,
};
use vesta_gpui_winit::{
    AccessibilityAdapter, AccessibilityEvent, AccessibilityWindowEvent, Clipboard, TextInputBridge,
    TextInputResponse, logical_point, shortcut_chord,
};
use vesta_gpui_workspace::{
    MIN_DOCK_PANE_EXTENT, PaneHeader, PaneHeaderCommand, PaneNavigationDirection,
    WorkspaceViewAction, WorkspaceViewInteraction, WorkspaceViewRegions, dock_drag_overlay,
    split_ratio_at, workspace_container_view_with_header,
};
use vesta_viewport::{
    Camera, TransformAction, TransformMode, TranslationDrag, ViewportAction, ViewportScene,
    apply_action, pick,
};
use vesta_workspace::{
    ContainerRef, DockAction, FloatingWindowId, ItemId, Pane, PaneId, PanelTypeId, Side, SplitAxis,
    SplitId, WindowPlacement, WorkspaceModel, WorkspaceNode,
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize},
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent as NativeWindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::{Key, NamedKey},
    window::{CursorIcon, ResizeDirection, Window, WindowAttributes, WindowId},
};

const MODE_2D: u32 = 10;
const MODE_3D: u32 = 11;
const THEME_TOGGLE: u32 = 12;
const LOCALE_TOGGLE: u32 = 17;
const APP_MENU: u32 = 13;
const WINDOW_MINIMIZE: u32 = 14;
const WINDOW_MAXIMIZE: u32 = 15;
const WINDOW_CLOSE: u32 = 16;
const HEADER_FILE_MENU: u32 = 31;
const HEADER_VIEW_MENU: u32 = 32;
const HEADER_WINDOW_MENU: u32 = 33;
const CHROME_HEIGHT: f32 = CHROME_TITLE_HEIGHT;
const HEADER_NEW_TAB: u8 = 1;
const HEADER_SPLIT_RIGHT: u8 = 2;
const HEADER_TOGGLE_ZOOM: u8 = 3;
const HEADER_OVERFLOW: u8 = 4;
const GALLERY_PAGE_SCROLL_BASE: u32 = 10_000_000;
const GALLERY_PAGE_CONTENT_BASE: u32 = 11_000_000;
const GALLERY_SECTION_BASE: u32 = 200_000;
const GALLERY_SECTION_DISCLOSURE_BASE: u32 = 201_000;
const RESIZE_EDGE: f32 = 6.0;
const TEXT_FIELD: u32 = 100;
const SEARCH_FIELD: u32 = 101;
const TEXT_AREA: u32 = 102;
const NUMBER_FIELD: u32 = 103;
const SELECTABLE_LABEL_FIELD: u32 = 104;
const SELECTABLE_LABEL_SOURCE: &str = "可选择文字 / Vesta 地形 مرحبا e\u{301} 👨‍👩‍👧‍👦";
const NUMBER_MIN: i32 = -999;
const NUMBER_MAX: i32 = 999;
const VECTOR_VALUE_BASE: u32 = 16_000_000;
const VECTOR_MIN: f32 = -999.0;
const VECTOR_MAX: f32 = 999.0;
const DRAWER_TOGGLE: u32 = 160;
const DRAWER_BODY: u32 = 161;
const DRAWER_RESIZE: u32 = 162;
const KEY_CHORD_TRIGGER: u32 = 170;
const TOAST_ROOT: u32 = 1_000_000;
const TOAST_SHOW: u32 = 186;
const TABLE_ROOT: u32 = 3_000_000;
const TABLE_NOTE_FIELD: u32 = TABLE_ROOT + 50;
const TABLE_VIRTUAL_ROOT: u32 = 17_000_000;
const BASIC_LIST_ROOT: u32 = 18_000_000;
const TABLE_VIRTUAL_COUNT: usize = 10_000;
const TABLE_VIRTUAL_ROW_HEIGHT: f32 = 28.0;
const TABLE_VIRTUAL_VIEWPORT_HEIGHT: f32 = 140.0;
const TILE_ROOT: u32 = 4_000_000;
const RAMP_ROOT: u32 = 5_000_000;
const SUGGESTION_ROOT: u32 = 6_000_000;
const DIALOG_ROOT: u32 = 8_000_000;
const INLINE_ROOT: u32 = 7_000_000;
const PALETTE_ROOT: u32 = 9_000_000;
/// Root-scope slot for the container that carries the palette and the top modal dialog of the
/// window that opened them (ADR-0254).
const OWNED_OVERLAY_ROOT: u32 = 9_500_000;
const BREADCRUMB_ROOT: u32 = 12_000_000;
const TOOLBAR_ROOT: u32 = 13_000_000;
const RADIO_GROUP_ROOT: u32 = 25_300_000;
const COLOR_PICKER_ROOT: u32 = 14_000_000;
const COLOR_HEX_FIELD: u32 = COLOR_PICKER_ROOT + 4;
const PASSWORD_ROOT: u32 = 15_000_000;
const COMBO_TRIGGER: u32 = 901;
const COMBO_POPUP: u32 = 902;
const COMBO_OPTION_BASE: u32 = 910;
const COMBO_VALUES: [&str; 3] = ["Perspective", "Orthographic", "Camera"];
const POPOVER_TRIGGER: u32 = 900;
const POPOVER_ROOT: u32 = 1;
const POPOVER_BUTTON: u32 = 4;
const NESTED_SCROLL: u32 = 5;
const HORIZONTAL_SCROLL: u32 = 6;
const VIRTUAL_SCROLL: u32 = 7;
const VIRTUAL_ROW_BASE: u32 = 20_000;
const VIRTUAL_LABEL_BASE: u32 = 40_000;
const VIRTUAL_ITEM_COUNT: usize = 10_000;
const VIRTUAL_ROW_HEIGHT: f32 = 28.0;
const VIRTUAL_VIEWPORT_HEIGHT: f32 = 140.0;
const TREE_SCROLL: u32 = 8;
const TREE_ROW_BASE: u32 = 60_000;
const TREE_DISCLOSURE_BASE: u32 = 80_000;
const TREE_LABEL_BASE: u32 = 100_000;
const TREE_VIEWPORT_HEIGHT: f32 = 140.0;
const STANDALONE_WINDOW: UiWindowId = UiWindowId(1);
const TOOLTIP_DELAY: Duration = Duration::from_millis(500);
#[cfg(test)]
const WINDOW_TITLE: &str = "Vesta GPUI Standalone — 2D Widget Gallery / 3D Viewport";

#[derive(Debug)]
enum UserEvent {
    Accessibility(AccessibilityEvent),
    UiQuery(Box<ui_query::Request>),
}

impl From<AccessibilityEvent> for UserEvent {
    fn from(event: AccessibilityEvent) -> Self {
        Self::Accessibility(event)
    }
}

#[derive(Clone, Copy)]
struct UiScopes {
    root: ElementScope,
    header: ElementScope,
    status: ElementScope,
    gallery: ElementScope,
    menu: ElementScope,
    viewport: ElementScope,
    workspace: ElementScope,
    popover: ElementScope,
    /// Scope for the ramp gradient band segments; one block per window so identity stays
    /// unique across the main window and every native floating window.
    ramp_band: ElementScope,
    tooltip: ElementScope,
    markdown: ElementScope,
    diff: ElementScope,
}

impl UiScopes {
    fn new() -> Self {
        let allocator = ElementIdAllocator;
        let next = || match allocator.allocate_scope() {
            Ok(scope) => scope,
            Err(_) => panic!("standalone exhausted UI identity scopes"),
        };
        Self {
            root: next(),
            header: next(),
            status: next(),
            gallery: next(),
            menu: next(),
            viewport: next(),
            workspace: next(),
            popover: next(),
            ramp_band: next(),
            tooltip: next(),
            markdown: next(),
            diff: next(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
enum GallerySectionId {
    Buttons = 0,
    Toggles = 1,
    Conditional = 2,
    Feedback = 3,
    Inputs = 4,
    Multiline = 5,
    Selection = 6,
    Property = 7,
    Navigation = 8,
    Nested = 9,
    Horizontal = 10,
    VirtualList = 11,
    VirtualTree = 12,
    Grid = 13,
    Accordion = 14,
    Overlays = 15,
}

impl GallerySectionId {
    fn element(self, scope: ElementScope, slot: u32) -> ElementId {
        scope.element(GALLERY_SECTION_BASE + self as u32 * 4 + slot)
    }

    fn disclosure(self, scope: ElementScope) -> ElementId {
        scope.element(GALLERY_SECTION_DISCLOSURE_BASE + self as u32)
    }
}

fn demo_keymap() -> Keymap<DemoAction> {
    let mut keymap = Keymap::default();
    for (key, action) in palette_demo::SHORTCUT_COMMANDS {
        keymap
            .bind(KeyChord::character(key, KeyModifiers::default()), action)
            .expect("unique demo shortcut");
        if key.is_ascii_alphabetic() {
            keymap
                .bind(
                    KeyChord::character(
                        key,
                        KeyModifiers {
                            shift: true,
                            ..KeyModifiers::default()
                        },
                    ),
                    action,
                )
                .expect("unique shifted demo shortcut");
        }
    }
    keymap
        .bind(
            KeyChord::character(
                'p',
                KeyModifiers {
                    control: true,
                    shift: true,
                    ..KeyModifiers::default()
                },
            ),
            DemoAction::OpenCommandPalette,
        )
        .expect("unique command palette shortcut");
    keymap
}

fn key_chord_caption(chord: KeyChord) -> String {
    let mut parts = Vec::new();
    if chord.modifiers.control {
        parts.push("Ctrl".to_owned());
    }
    if chord.modifiers.alt {
        parts.push("Alt".to_owned());
    }
    if chord.modifiers.shift {
        parts.push("Shift".to_owned());
    }
    if chord.modifiers.super_key {
        parts.push("Super".to_owned());
    }
    parts.push(match chord.key {
        KeyStroke::Character(key) => key.to_uppercase().to_string(),
        other => format!("{other:?}"),
    });
    parts.join("+")
}

fn dialog_element(scope: ElementScope, id: u8, slot: u32) -> ElementId {
    scope.element(DIALOG_ROOT + u32::from(id) * 20 + slot)
}

fn accessibility_root(root: &Element, isolated_root: Option<ElementId>) -> &Element {
    isolated_root.and_then(|id| root.find(id)).unwrap_or(root)
}

fn table_resize_element(scope: ElementScope, column: usize) -> ElementId {
    scope.element(TABLE_ROOT + 20 + column as u32)
}

fn dialog_ids(scope: ElementScope, id: u8) -> ModalIds {
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

struct DemoState {
    locale: UiLocale,
    long_document: bool,
    viewport3d_available: bool,
    toggle: bool,
    compact_toggle: bool,
    busy: bool,
    busy_phase: u8,
    drawer_open: bool,
    drawer_width: f32,
    drawer_tab: u8,
    checkbox: BinaryState,
    radio: u8,
    slider: f32,
    slider_enabled: bool,
    number: i32,
    vector: [Option<f32>; 3],
    key_capture: KeyChordCapture,
    key_chord: Option<KeyChord>,
    toast_queue: ToastQueue<DemoAction>,
    next_toast_id: u32,
    table_sort_column: u8,
    table_note: String,
    table_sort_descending: bool,
    table_selected: u8,
    table_selected_cell: Option<(u8, u8)>,
    virtual_table_selected: usize,
    virtual_table_scroll: ScrollAreaState,
    table_column_fractions: [f32; 3],
    tile_selected: usize,
    ramp: RampDemo,
    status_overflow_open: bool,
    combo: PopoverState,
    combo_selected: u8,
    suggestions: vesta_gpui_widgets::SuggestionState<u16>,
    suggestion_serial: u64,
    suggestion_dismissed: Option<(InputTarget, String)>,
    suggestion_scroll_node: Option<vesta_gpui::LayoutNode>,
    suggestion_drag: Option<vesta_gpui_widgets::ScrollbarDrag>,
    dialog_stack: ModalStack<u8>,
    inline_label: String,
    inline_session: vesta_gpui_widgets::TemporaryEditorState<TextInputState>,
    palette_session: vesta_gpui_widgets::TemporaryEditorState<TextInputState>,
    palette: vesta_gpui_widgets::CommandPaletteState<u8>,
    palette_scroll_node: Option<vesta_gpui::LayoutNode>,
    palette_drag: Option<ScrollbarDrag>,
    breadcrumb_depth: u8,
    breadcrumb_expanded: bool,
    toolbar_overflow_open: bool,
    color_committed: [u8; 4],
    color_draft: [u8; 4],
    color_mode: ColorPickerMode,
    color_open: bool,
    color_session: Option<ValueEditSession<[u8; 4]>>,
    color_target: ColorEditTarget,
    password_revealed: bool,
    popover: PopoverState,
    menu: MenuController,
    menu_origin: MenuOrigin,
    menu_context: Option<MenuContext>,
    menu_anchor: Point,
    tab_menu_item: Option<ItemId>,
    selected_tab: u8,
    selected_list: u8,
    virtual_ids: Vec<usize>,
    virtual_metrics: Option<vesta_gpui_widgets::VariableListMetrics>,
    virtual_selection: ListSelectionState<usize>,
    tree_entries: Vec<TreeViewEntry<usize>>,
    tree_expansion: TreeExpansionState<usize>,
    tree_selection: ListSelectionState<usize>,
    tree_expanded: bool,
    accordion_open: Option<u8>,
    collapsed_gallery_sections: HashSet<GallerySectionId>,
    nested_scroll: ScrollAreaState,
    horizontal_scroll: ScrollAreaState,
    horizontal_flow: FlowDirection,
    virtual_scroll: ScrollAreaState,
    tree_scroll: ScrollAreaState,
    page_scroll: HashMap<ItemId, ScrollAreaState>,
    tab_scroll: HashMap<PaneId, ScrollAreaState>,
    pane_widths: HashMap<PaneId, f32>,
    workspace: WorkspaceModel,
    pane_history: HashMap<PaneId, PaneHistory>,
    zoomed_panes: HashMap<ContainerRef, PaneId>,
}

#[derive(Clone, Debug, Default)]
struct PaneHistory {
    entries: Vec<ItemId>,
    cursor: usize,
}

impl PaneHistory {
    fn visit(&mut self, item: ItemId) {
        if self.entries.get(self.cursor) == Some(&item) {
            return;
        }
        self.entries.truncate(self.cursor.saturating_add(1));
        self.entries.push(item);
        self.cursor = self.entries.len() - 1;
    }

    fn target(&self, direction: PaneNavigationDirection) -> Option<(usize, ItemId)> {
        let index = match direction {
            PaneNavigationDirection::Back => self.cursor.checked_sub(1)?,
            PaneNavigationDirection::Forward => self.cursor.checked_add(1)?,
        };
        self.entries.get(index).copied().map(|item| (index, item))
    }

    fn retain_pane_items(&mut self, pane: &Pane) {
        let previous_cursor = self.cursor;
        let mut retained = Vec::with_capacity(self.entries.len());
        let mut cursor = 0;
        for (index, item) in self.entries.drain(..).enumerate() {
            if pane.items.iter().any(|candidate| candidate.id == item) {
                if index <= previous_cursor {
                    cursor = retained.len();
                }
                retained.push(item);
            }
        }
        self.entries = retained;
        self.cursor = cursor;
        if let Some(active) = pane.active {
            self.visit(active);
        }
    }
}

impl DemoState {
    fn sync_pane_widths(&mut self, widths: impl IntoIterator<Item = (PaneId, f32)>) -> bool {
        let mut changed = false;
        for (pane, width) in widths {
            let previous = self.pane_widths.insert(pane, width);
            changed |= previous.is_none_or(|previous| (previous - width).abs() > 1.0);
        }
        changed
    }

    fn sync_workspace_ui(&mut self) {
        self.pane_history
            .retain(|pane, _| self.workspace.pane(*pane).is_some());
        self.tab_scroll
            .retain(|pane, _| self.workspace.pane(*pane).is_some());
        self.pane_widths
            .retain(|pane, _| self.workspace.pane(*pane).is_some());
        let mut panes = self.workspace.panes();
        for window in self.workspace.floating_windows() {
            panes.extend(
                window
                    .root
                    .panes()
                    .into_iter()
                    .filter_map(|id| self.workspace.pane(id)),
            );
        }
        for pane in panes {
            self.pane_history
                .entry(pane.id)
                .or_default()
                .retain_pane_items(pane);
        }
        self.zoomed_panes
            .retain(|container, pane| container_panes(&self.workspace, *container).contains(pane));
    }
}

impl Default for DemoState {
    fn default() -> Self {
        let mut tree_expansion = TreeExpansionState::default();
        tree_expansion.toggle(0);
        let tree_entries = tree_collections::sample_entries();
        let mut state = Self {
            locale: UiLocale::EnUs,
            long_document: false,
            viewport3d_available: true,
            toggle: true,
            compact_toggle: false,
            busy: true,
            busy_phase: 0,
            drawer_open: true,
            drawer_width: 144.0,
            drawer_tab: 0,
            checkbox: BinaryState::On,
            radio: 0,
            slider: 0.52,
            slider_enabled: true,
            number: 128,
            vector: [Some(1.0), None, Some(3.0)],
            key_capture: KeyChordCapture::default(),
            key_chord: None,
            toast_queue: ToastQueue::default(),
            next_toast_id: 1,
            table_sort_column: 0,
            table_note: "Ready".to_owned(),
            table_sort_descending: false,
            table_selected: 1,
            table_selected_cell: Some((1, 0)),
            virtual_table_selected: 0,
            virtual_table_scroll: ScrollAreaState::default(),
            table_column_fractions: [118.0 / 284.0, 92.0 / 284.0, 74.0 / 284.0],
            tile_selected: 0,
            ramp: RampDemo::default(),
            status_overflow_open: false,
            combo: PopoverState::default(),
            combo_selected: 0,
            suggestions: Default::default(),
            suggestion_serial: 0,
            suggestion_dismissed: None,
            suggestion_scroll_node: None,
            suggestion_drag: None,
            dialog_stack: ModalStack::default(),
            inline_label: "Sample layer".to_owned(),
            inline_session: Default::default(),
            palette_session: Default::default(),
            palette: Default::default(),
            palette_scroll_node: None,
            palette_drag: None,
            breadcrumb_depth: 3,
            breadcrumb_expanded: false,
            toolbar_overflow_open: false,
            color_committed: [75, 142, 245, 204],
            color_draft: [75, 142, 245, 204],
            color_mode: ColorPickerMode::Rgb,
            color_open: false,
            color_session: None,
            color_target: ColorEditTarget::Swatch,
            password_revealed: false,
            popover: PopoverState::default(),
            menu: MenuController::default(),
            menu_origin: MenuOrigin::Context,
            menu_context: None,
            menu_anchor: Point::new(258.0, 202.0),
            tab_menu_item: None,
            selected_tab: 0,
            selected_list: 0,
            virtual_ids: (0..VIRTUAL_ITEM_COUNT).collect(),
            virtual_metrics: None,
            virtual_selection: ListSelectionState::single(0),
            tree_entries,
            tree_expansion,
            tree_selection: ListSelectionState::single(0),
            tree_expanded: true,
            accordion_open: Some(0),
            collapsed_gallery_sections: HashSet::new(),
            nested_scroll: ScrollAreaState::default(),
            horizontal_scroll: ScrollAreaState::default(),
            horizontal_flow: FlowDirection::LeftToRight,
            virtual_scroll: ScrollAreaState::default(),
            tree_scroll: ScrollAreaState::default(),
            page_scroll: HashMap::new(),
            tab_scroll: HashMap::new(),
            pane_widths: HashMap::new(),
            workspace: demo_workspace(),
            pane_history: HashMap::new(),
            zoomed_panes: HashMap::new(),
        };
        state.sync_workspace_ui();
        state
    }
}

type NumberScrub = HorizontalEditSession<i32>;

#[derive(Clone, Copy, Debug)]
struct VectorScrub {
    axis: u8,
    session: HorizontalEditSession<Option<f32>>,
}

impl std::ops::Deref for VectorScrub {
    type Target = HorizontalEditSession<Option<f32>>;
    fn deref(&self) -> &Self::Target {
        &self.session
    }
}

impl std::ops::DerefMut for VectorScrub {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.session
    }
}

type DrawerResizeDrag = HorizontalEditSession<f32>;

/// Starts a scrollbar thumb drag for the hit window, using that window's own viewport rects.
fn begin_scrollbar_drag(
    gallery: ElementScope,
    geometry: &ScrollGeometry,
    actions: &ActionRoute<DemoAction>,
    states: &mut ScrollStates<'_>,
    hit: Option<ElementId>,
    pointer: Point,
) -> Option<(ElementId, ScrollbarDrag)> {
    let hit = hit?;
    let scope = FocusScope::new(STANDALONE_WINDOW, actions, None);
    let path = vesta_gpui_widgets::ScrollInputPath::new(
        &scope,
        InputTarget {
            window: STANDALONE_WINDOW,
            element: hit,
        },
    )?;
    let targets = geometry.targets(gallery);
    path.begin_drag(pointer, |element, pointer| {
        let (_, rect, target) = targets
            .iter()
            .find(|(candidate, _, _)| *candidate == element)?;
        let state = states.for_target(*target)?;
        if let Some(node) = geometry.nodes.get(&element) {
            state.sync_layout(node);
            state.begin_thumb_drag_in_layout(node, pointer)
        } else {
            state.begin_thumb_drag(*rect, pointer)
        }
    })
}

/// Only the thumb's primary-button release can end its session.
fn finish_scrollbar_drag(
    session: &mut Option<(ElementId, ScrollbarDrag)>,
    button: MouseButton,
) -> bool {
    button == MouseButton::Left && session.take().is_some()
}

/// Applies one pointer move to a scrollbar drag through the window that owns it.
fn update_scrollbar_drag_state(
    gallery: ElementScope,
    geometry: &ScrollGeometry,
    states: &mut ScrollStates<'_>,
    element: ElementId,
    drag: ScrollbarDrag,
    pointer: Point,
) -> bool {
    for (candidate, _, target) in geometry.targets(gallery) {
        if candidate != element {
            continue;
        }
        let Some(state) = states.for_target(target) else {
            return false;
        };
        if let Some(node) = geometry.nodes.get(&element) {
            state.sync_layout(node);
        }
        return drag.update(state, pointer);
    }
    false
}

/// One wheel step, shared by every window: movement is consumed along the action path from
/// the innermost scroll element outwards, so an inner list with range scrolls before its
/// ancestors. Returns the movement no viewport consumed.
fn wheel_scroll_path(
    window: UiWindowId,
    actions: &ActionRoute<DemoAction>,
    states: &mut ScrollStates<'_>,
    hit: Option<ElementId>,
    geometry: &ScrollGeometry,
    gallery: ElementScope,
    movement: Point,
) -> Point {
    let root = gallery.element(3);
    let targets = geometry
        .targets(gallery)
        .into_iter()
        .map(|(element, _, target)| (element, target))
        .collect::<Vec<_>>();
    let target = hit
        .filter(|hit| actions.is_descendant_of(*hit, root))
        .unwrap_or(root);
    vesta_gpui_widgets::scroll_wheel_path(
        &FocusScope::new(window, actions, Some(root)),
        InputTarget {
            window,
            element: target,
        },
        movement,
        |element, mut remaining| {
            if let Some((_, scroll_target)) = targets.iter().find(|(id, _)| *id == element)
                && let Some(state) = states.for_target(*scroll_target)
            {
                remaining = state.scroll_by_point(remaining);
            }
            remaining
        },
    )
}

/// Which window currently owns the caret for a shared text field (ADR-0252).
///
/// The text state itself stays host-global; only the session - caret owner, pointer capture,
/// IME session and the geometry used to map the pointer - belongs to one window at a time.
type TextSession = WindowTextSession<TextFieldKind>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum AcceptanceMode {
    #[default]
    Gallery2d,
    Viewport3d,
}

impl AcceptanceMode {
    fn from_args() -> Self {
        match std::env::args()
            .skip(1)
            .find(|arg| arg.starts_with("--mode="))
        {
            Some(value) if value == "--mode=3d" => Self::Viewport3d,
            _ => Self::Gallery2d,
        }
    }
}

#[derive(Default)]
struct HostClipboard(Option<arboard::Clipboard>);

impl Clipboard for HostClipboard {
    type Error = String;

    fn read_text(&mut self) -> Result<String, Self::Error> {
        self.clipboard()?
            .get_text()
            .map_err(|error| error.to_string())
    }

    fn write_text(&mut self, text: &str) -> Result<(), Self::Error> {
        self.clipboard()?
            .set_text(text.to_owned())
            .map_err(|error| error.to_string())
    }
}

impl HostClipboard {
    fn clipboard(&mut self) -> Result<&mut arboard::Clipboard, String> {
        if self.0.is_none() {
            self.0 = Some(arboard::Clipboard::new().map_err(|error| error.to_string())?);
        }
        match self.0.as_mut() {
            Some(clipboard) => Ok(clipboard),
            None => Err("clipboard initialization returned no instance".to_owned()),
        }
    }
}

struct App {
    ui_query: ui_query::Store,
    scopes: UiScopes,
    documents: GalleryDocuments,
    mode: AcceptanceMode,
    theme: WidgetTheme,
    window: Option<Arc<Window>>,
    native_windows: HashMap<WindowId, NativeFloatingHost>,
    gpu_lifecycle: gpu_lifecycle::Smoke,
    native_capture: Option<native_capture::Fixture>,
    gpu_recovery: gpu_recovery::State,
    #[cfg(feature = "gpu-fault-injection")]
    target_probe: target_reconfiguration::Probe,
    native_text_bridges: vesta_gpui_winit::WindowTextInputBridges,
    native_window_order: Vec<WindowId>,
    event_proxy: Option<EventLoopProxy<UserEvent>>,
    accessibility: Option<AccessibilityAdapter>,
    semantics: AccessibilityTree,
    gpu: Option<GpuState>,
    text_context: TextInputContext,
    text_state: TextInputState,
    search_state: TextInputState,
    area_state: TextInputState,
    area_layout: AreaWrapLayout,
    number_state: TextInputState,
    vector_states: [TextInputState; 3],
    color_hex_state: TextInputState,
    password_state: TextInputState,
    table_note_state: TextInputState,
    readonly_label_state: TextInputState,
    text_rasterizer: TextRasterizer,
    text_bridge: TextInputBridge,
    input_ownership: InputOwnership,
    pointer_click: PointerClick,
    clipboard: HostClipboard,
    hit_test: HitTestTree,
    text_view: Option<vesta_gpui::TextInputViewport>,
    search_view: Option<vesta_gpui::TextInputViewport>,
    area_view: Option<vesta_gpui::TextInputViewport>,
    area_scroll_y: f32,
    area_follow_caret: bool,
    number_view: Option<vesta_gpui::TextInputViewport>,
    vector_views: [Option<vesta_gpui::TextInputViewport>; 3],
    color_hex_view: Option<vesta_gpui::TextInputViewport>,
    password_view: Option<vesta_gpui::TextInputViewport>,
    table_note_view: Option<vesta_gpui::TextInputViewport>,
    readonly_label_view: Option<vesta_gpui::TextInputViewport>,
    /// Which window owns the caret for the shared text fields (ADR-0252).
    text_session: Option<TextSession>,
    /// Coalesced presentation request; geometry is resolved only by the current owner frame.
    caret_reveal: Option<InputTarget>,
    /// The window that opened the command palette or a modal dialog (ADR-0254).
    overlay_owner: Option<UiWindowId>,
    /// The window whose input produced the action currently being applied.
    action_window: UiWindowId,
    toast_focus: vesta_gpui_widgets::NotificationFocusState,
    cursor: Point,
    gallery_scroll: ScrollAreaState,
    camera: Camera,
    scene: ViewportScene,
    camera_drag: bool,
    camera_pan: bool,
    transform_drag: Option<TranslationDrag>,
    last_cursor: Point,
    hovered: Option<ElementId>,
    tooltip: WindowTooltip,
    next_busy_frame: Instant,
    busy_visible: bool,
    caret_blink: CaretBlink,
    cursor_icon: CursorIcon,
    last_title_click: Option<(Instant, Point)>,
    exit_requested: bool,
    ui_actions: ActionRoute<DemoAction>,
    keymap: Keymap<DemoAction>,
    slider_rect: Option<Rect>,
    slider_drag: Option<ValueEditSession<f32>>,
    number_scrub: Option<NumberScrub>,
    vector_scrub: Option<VectorScrub>,
    drawer_resize: Option<DrawerResizeDrag>,
    table_resize: Option<TableResizeDrag>,
    ramp_drag: Option<RampDrag>,
    ramp_track_rect: Option<Rect>,
    table_rect: Option<Rect>,
    virtual_table_scroll_rect: Option<Rect>,
    workspace_rect: Option<Rect>,
    dock_frame: DockFrame,
    workspace_drag: Option<WorkspaceTabDrag>,
    dock_focus: Option<DockFocusTransfer>,
    floating_move: Option<FloatingMoveDrag>,
    gallery_scroll_rect: Option<Rect>,
    /// Scroll viewport rects of this window's last layout (ADR-0253 decision 7).
    scroll_geometry: ScrollGeometry,
    page_scroll_rects: HashMap<ItemId, Rect>,
    tab_scroll_rects: HashMap<PaneId, Rect>,
    tab_reveal: HashMap<PaneId, TabRevealGeometry>,
    nested_scroll_rect: Option<Rect>,
    horizontal_scroll_rect: Option<Rect>,
    virtual_scroll_rect: Option<Rect>,
    tree_scroll_rect: Option<Rect>,
    tile_columns: usize,
    scrollbar_drag: Option<(ElementId, ScrollbarDrag)>,
    focus_rects: HashMap<ElementId, Rect>,
    workspace_resize: Option<vesta_gpui_workspace::SplitResizeSession>,
    demo: DemoState,
    smoke: bool,
    document_smoke: window_documents::Smoke,
    rendered_frames: u64,
    frame_meter: FrameMeter,
    frame_profile: FrameProfiler,
    gpu_profile: gpu_profile::GpuProfiler,
    lifecycle_profile: lifecycle_profile::LifecycleProfiler,
    fatal_error: Option<String>,
}

impl App {
    fn new() -> Self {
        let mut text_context = TextInputContext::default();
        register_host_fonts(&mut text_context);
        let mut text_state = TextInputState::default();
        let _ = text_state.reset(&mut text_context, "中文 IME / Selection / Undo");
        let mut search_state = TextInputState::default();
        let _ = search_state.reset(&mut text_context, "");
        let mut area_state = TextInputState::new_multiline(13.0, "sans-serif", 344.0);
        let _ = area_state.reset(
            &mut text_context,
            "Vesta 地形 مرحبا 😀\nSelection / Undo / IME",
        );
        let mut number_state = TextInputState::default();
        let _ = number_state.reset(&mut text_context, "128");
        let vector_states = std::array::from_fn(|axis| {
            let mut state = TextInputState::default();
            let value = match axis {
                0 => "1",
                1 => "—",
                _ => "3",
            };
            let _ = state.reset(&mut text_context, value);
            state
        });
        let mut color_hex_state = TextInputState::default();
        let _ = color_hex_state.reset(&mut text_context, "#4B8EF5CC");
        let mut password_state = TextInputState::new_password(13.0, "sans-serif", 256);
        let _ = password_state.reset(&mut text_context, "vesta");
        let mut table_note_state = TextInputState::default();
        let _ = table_note_state.reset(&mut text_context, "Ready");
        let mut readonly_label_state = TextInputState::new_read_only(13.0, "sans-serif", None);
        readonly_label_state
            .reset(&mut text_context, SELECTABLE_LABEL_SOURCE)
            .expect("bounded label source");
        Self {
            scopes: UiScopes::new(),
            ui_query: ui_query::Store::new(
                std::env::args()
                    .any(|arg| matches!(arg.as_str(), "--ui-query-stdio" | "--ui-control-stdio")),
            ),
            documents: GalleryDocuments::default(),
            mode: AcceptanceMode::from_args(),
            theme: WidgetTheme::dark(),
            window: None,
            native_windows: HashMap::new(),
            gpu_recovery: gpu_recovery::State::default(),
            #[cfg(feature = "gpu-fault-injection")]
            target_probe: target_reconfiguration::Probe::default(),
            gpu_lifecycle: gpu_lifecycle::Smoke::Disabled,
            native_capture: native_capture::Fixture::from_args(),
            native_text_bridges: vesta_gpui_winit::WindowTextInputBridges::default(),
            native_window_order: Vec::new(),
            event_proxy: None,
            accessibility: None,
            semantics: AccessibilityTree::default(),
            gpu: None,
            text_context,
            text_state,
            search_state,
            area_state,
            area_layout: AreaWrapLayout::default(),
            number_state,
            vector_states,
            color_hex_state,
            password_state,
            table_note_state,
            readonly_label_state,
            text_rasterizer: TextRasterizer::default(),
            text_bridge: TextInputBridge::default(),
            input_ownership: InputOwnership::default(),
            pointer_click: PointerClick::default(),
            clipboard: HostClipboard::default(),
            hit_test: HitTestTree::default(),
            text_view: None,
            search_view: None,
            area_view: None,
            area_scroll_y: 0.0,
            area_follow_caret: true,
            number_view: None,
            vector_views: [None; 3],
            color_hex_view: None,
            password_view: None,
            table_note_view: None,
            readonly_label_view: None,
            text_session: None,
            caret_reveal: None,
            overlay_owner: None,
            action_window: STANDALONE_WINDOW,
            toast_focus: Default::default(),
            cursor: Point::default(),
            gallery_scroll: ScrollAreaState::new(0.0),
            camera: Camera::default(),
            scene: if std::env::args().any(|arg| arg == "--transparent-fixture") {
                transparent_fixture::scene()
            } else {
                ViewportScene::acceptance_scene()
            },
            camera_drag: false,
            camera_pan: false,
            transform_drag: None,
            last_cursor: Point::default(),
            hovered: None,
            tooltip: WindowTooltip::new(TOOLTIP_DELAY),
            next_busy_frame: Instant::now() + Duration::from_millis(80),
            busy_visible: false,
            caret_blink: CaretBlink::default(),
            cursor_icon: CursorIcon::Default,
            last_title_click: None,
            exit_requested: false,
            ui_actions: ActionRoute::default(),
            keymap: demo_keymap(),
            slider_rect: None,
            slider_drag: None,
            number_scrub: None,
            vector_scrub: None,
            drawer_resize: None,
            table_resize: None,
            ramp_drag: None,
            ramp_track_rect: None,
            table_rect: None,
            virtual_table_scroll_rect: None,
            workspace_rect: None,
            dock_frame: DockFrame::default(),
            workspace_drag: None,
            dock_focus: None,
            floating_move: None,
            gallery_scroll_rect: None,
            scroll_geometry: ScrollGeometry::default(),
            page_scroll_rects: HashMap::new(),
            tab_scroll_rects: HashMap::new(),
            tab_reveal: HashMap::new(),
            nested_scroll_rect: None,
            horizontal_scroll_rect: None,
            virtual_scroll_rect: None,
            tree_scroll_rect: None,
            tile_columns: 1,
            scrollbar_drag: None,
            focus_rects: HashMap::new(),
            workspace_resize: None,
            demo: DemoState::default(),
            smoke: std::env::args().any(|argument| argument == "--smoke"),
            document_smoke: window_documents::Smoke::from_args(),
            rendered_frames: 0,
            frame_meter: FrameMeter::default(),
            frame_profile: FrameProfiler::from_args(),
            gpu_profile: gpu_profile::GpuProfiler::from_args(),
            lifecycle_profile: lifecycle_profile::LifecycleProfiler::from_args(),
            fatal_error: None,
        }
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn request_all_redraw(&self) {
        self.request_redraw();
        for host in self.native_windows.values() {
            host.window.request_redraw();
        }
    }

    /// Closes the palette and modal dialogs when their owning window disappears, so an overlay
    /// can never stay open in a window that no longer exists (ADR-0254).
    fn close_owned_overlays(&mut self) {
        if self.overlay_owner.is_none() {
            return;
        }
        while let Some(id) = self.demo.dialog_stack.top() {
            self.close_demo_dialog(id, false);
        }
        self.close_command_palette();
        self.overlay_owner = None;
    }

    fn native_window_title(&self, id: FloatingWindowId) -> Option<String> {
        self.demo
            .workspace
            .floating_window(id)
            .map(|window| window.title.clone())
    }

    fn redraw_native_window(&mut self, window_id: WindowId) {
        if self
            .native_windows
            .get(&window_id)
            .is_none_or(|host| !host.surface.available())
        {
            return;
        }
        let mut frame_trace = self.frame_profile.begin();
        let input_watermark = self.lifecycle_profile.begin_frame();
        if let Some(window) = self
            .native_windows
            .get(&window_id)
            .map(|host| host.ui_window)
        {
            self.reconcile_window_menu(window);
            self.prepare_suggestions(window);
        }
        let password_snapshot = self.password_display_snapshot();
        let table_note_snapshot = self.table_note_state.snapshot();
        let tooltip_focus_root = self
            .native_windows
            .get(&window_id)
            .and_then(|host| self.focus_root_for_window(host.ui_window));
        let Some(mut host) = self.native_windows.remove(&window_id) else {
            return;
        };
        let Some(title) = self.native_window_title(host.logical_id) else {
            return;
        };
        let size = host.window.inner_size();
        if size.width == 0 || size.height == 0 {
            self.native_windows.insert(window_id, host);
            return;
        }
        let scale = host.window.scale_factor() as f32;
        let logical = size.to_logical::<f32>(host.window.scale_factor());
        let viewport = Viewport::new(logical.width, logical.height, scale);
        let focused_element = host.input.focus().map(|focus| focus.element);
        let focused_field = TextFieldKind::ALL
            .into_iter()
            .find(|field| Some(field.element(host.scopes.gallery)) == focused_element);
        let hovered = host.hit_test.hit(host.cursor);
        let dock_drag = self.dock_drag_visual(ContainerRef::Floating(host.logical_id));
        let collection_focus = collection_projection_focus(
            &self.demo,
            &host.actions,
            &host.input,
            host.ui_window,
            host.scopes.gallery,
            tooltip_focus_root,
        );
        host.area_layout
            .prepare(&mut self.area_state, &mut self.text_context);
        let gallery = build_gallery(&UiBuildState {
            documents: &host.documents,
            selected_feedback: native_capture::selected_feedback_enabled(self),
            ui_window: host.ui_window,
            menu_open: host.pane_menu.is_some(),
            scopes: host.scopes,
            workspace_container: ContainerRef::Floating(host.logical_id),
            mode: AcceptanceMode::Gallery2d,
            maximized: host.window.is_maximized(),
            theme: self.theme,
            scroll_offset: Point::default(),
            viewport_size: Size::new(logical.width, (logical.height - CHROME_HEIGHT).max(0.0)),
            snapshot: self.text_state.snapshot(),
            focused: focused_field == Some(TextFieldKind::Main),
            search_snapshot: self.search_state.snapshot(),
            search_query: self.search_state.text(),
            search_focused: focused_field == Some(TextFieldKind::Search),
            area_snapshot: self
                .area_state
                .snapshot()
                .with_viewport_scroll(Point::new(0.0, host.area_scroll_y), host.area_follow_caret),
            area_focused: focused_field == Some(TextFieldKind::Area),
            number_snapshot: self.number_state.snapshot(),
            number_focused: focused_field == Some(TextFieldKind::Number),
            vector_snapshots: std::array::from_fn(|axis| self.vector_states[axis].snapshot()),
            vector_focused: std::array::from_fn(|axis| {
                focused_field == Some(TextFieldKind::Vector(axis as u8))
            }),
            color_hex_snapshot: self.color_hex_state.snapshot(),
            password_mask: self.password_state.masked_value(),
            password_snapshot,
            table_note_snapshot,
            color_hex_focused: focused_field == Some(TextFieldKind::ColorHex),
            table_note_focused: focused_field == Some(TextFieldKind::TableNote),
            readonly_label: &self.readonly_label_state,
            readonly_label_focused: focused_field == Some(TextFieldKind::Label),
            virtual_focused: match collection_focus {
                Some(CollectionFocus::List { index, .. }) => Some(index),
                _ => None,
            },
            virtual_table_focused: match collection_focus {
                Some(CollectionFocus::Table(index)) => Some(index),
                _ => None,
            },
            virtual_table_viewport_height: window_collections::table_viewport_height(
                &host.scroll_geometry,
                host.scopes.gallery,
            ),
            tree_focused: match collection_focus {
                Some(CollectionFocus::Tree { index, .. }) => Some(index),
                _ => None,
            },
            demo: &self.demo,
            hovered,
            focused_element,
            dock_drag: dock_drag.clone(),
            popover_anchor: host.popover_anchor,
            combo_anchor: host.combo_anchor,
            gizmo_mode: self.scene.gizmo.mode,
            frame_stats: self.frame_meter.display,
            overlay_owner: self.overlay_owner,
        });
        host.tile_columns = gallery.tile_columns;
        let workspace_regions = gallery.workspace_regions.clone();
        let ramp_track = gallery.ramp_track;
        let slider = gallery.slider;
        let (mut root, mut action_pairs) = native_window::build_native_floating_root(
            host.chrome_scope,
            gallery.element,
            gallery.actions,
            self.theme,
            native_window::NativeChrome {
                scope: host.title_scope,
                title: &display_text(&title, self.demo.locale),
                state: vesta_gpui_widgets::TitleBarState {
                    maximized: host.window.is_maximized(),
                    hovered: host.hit_test.hit(host.cursor),
                },
            },
        );
        if let Some(menu) = host.pane_menu.clone() {
            if menu.context.is_live(&self.demo) {
                root = root.child(build_window_menu_popup(
                    host.scopes.menu,
                    self.theme,
                    &self.demo,
                    &menu,
                    hovered,
                    Size::new(logical.width, logical.height),
                    &mut action_pairs,
                    &mut self.text_context,
                ));
            } else {
                host.close_pane_menu();
            }
        }
        if let Some(drag) = &dock_drag {
            root = root.child(dock_drag_overlay(
                host.scopes.root,
                drag,
                self.theme,
                Size::new(logical.width, logical.height),
            ));
        }
        if self.owns_overlay(host.ui_window)
            && let Some(overlays) = build_owned_overlays(
                host.scopes,
                &self.demo,
                self.search_state.text(),
                &self.search_state.snapshot(),
                self.text_session_field(host.ui_window) == Some(TextFieldKind::Search),
                Size::new(logical.width, logical.height),
                self.theme,
                &mut action_pairs,
            )
        {
            root = root.child(overlays);
        }
        localize_tree(&mut root, self.demo.locale);
        frame_trace.mark(FrameStage::Build);
        let Ok(actions) = ActionRoute::build(&root, action_pairs.iter().copied()) else {
            self.native_windows.insert(window_id, host);
            return;
        };
        frame_trace.mark(FrameStage::Route);
        let window_active = host.window.has_focus();
        let base_children = root.children.len();
        let Ok(layout) = self.layout_floating_tooltips(
            &mut host,
            &mut root,
            &actions,
            viewport,
            window_active,
            tooltip_focus_root,
        ) else {
            self.native_windows.insert(window_id, host);
            return;
        };
        let Ok(actions) =
            window_tooltips::route_after_tooltip(&root, base_children, actions, action_pairs)
        else {
            self.native_windows.insert(window_id, host);
            return;
        };
        frame_trace.mark(FrameStage::Layout);
        let wrap_changed = host.area_layout.sync(
            &mut self.area_state,
            &mut self.text_context,
            host.scopes.gallery,
            &layout,
        );
        if wrap_changed {
            host.window.request_redraw();
        }
        match host.documents.sync_layout(
            host.scopes.markdown,
            &layout,
            &mut self.text_context,
            self.theme,
        ) {
            Ok(true) => {
                host.window.request_redraw();
                self.native_windows.insert(window_id, host);
                return;
            }
            Ok(false) => {}
            Err(error) => {
                self.fatal_error = Some(error.to_string());
                self.native_windows.insert(window_id, host);
                return;
            }
        }
        if host.documents.sync_input(
            host.ui_window,
            host.scopes.markdown,
            &layout,
            &mut host.input,
        ) {
            host.window.request_redraw();
            self.native_windows.insert(window_id, host);
            return;
        }
        let workspace_rect = layout
            .node(host.scopes.workspace.element(1))
            .map(|node| node.rect);
        host.dock_frame = DockFrame::from_layout(&workspace_regions, &layout, workspace_rect);
        self.sync_palette_layout(host.ui_window, host.scopes.root, &layout);
        self.sync_suggestion_layout(host.ui_window, host.scopes.gallery, &layout);
        host.ramp_track_rect = layout.node(ramp_track).map(|node| node.rect);
        host.slider_rect = layout.node(slider).map(|node| node.rect);
        host.table_rect = layout
            .node(host.scopes.gallery.element(TABLE_ROOT))
            .map(|node| node.rect);
        host.combo_anchor = layout
            .node(host.scopes.gallery.element(COMBO_TRIGGER))
            .map(|node| node.rect);
        host.popover_anchor = layout
            .node(host.scopes.gallery.element(POPOVER_TRIGGER))
            .map(|node| node.rect);
        let page_items = workspace_regions
            .panes
            .iter()
            .filter_map(|region| {
                self.demo
                    .workspace
                    .pane(region.pane)
                    .and_then(Pane::active_item)
                    .map(|item| item.id)
            })
            .collect::<Vec<_>>();
        host.page_scroll_item = page_items.first().copied();
        let previous_table_height =
            window_collections::table_viewport_height(&host.scroll_geometry, host.scopes.gallery);
        host.scroll_geometry =
            ScrollGeometry::from_layout(host.scopes.gallery, &layout, page_items.iter().copied())
                .with_tabs(&layout, &workspace_regions);
        let table_viewport_changed = previous_table_height
            != window_collections::table_viewport_height(
                &host.scroll_geometry,
                host.scopes.gallery,
            );
        let mut scroll_changed = (ScrollStates {
            tree: &mut self.demo.tree_scroll,
            virtual_list: &mut self.demo.virtual_scroll,
            virtual_table: &mut self.demo.virtual_table_scroll,
            nested: &mut self.demo.nested_scroll,
            horizontal: &mut self.demo.horizontal_scroll,
            gallery: None,
            pages: &mut self.demo.page_scroll,
            tabs: &mut self.demo.tab_scroll,
        })
        .sync_layout(host.scopes.gallery, &host.scroll_geometry, &layout)
            || table_viewport_changed;
        if scroll_changed {
            host.window.request_redraw();
        }
        // Tab strips and the TextArea viewport belong to the window that shows them, so their
        // scroll extents are synced from this window's own layout.
        host.tab_scroll_rects.clear();
        let pane_width_changed =
            self.demo
                .sync_pane_widths(workspace_regions.panes.iter().filter_map(|region| {
                    layout
                        .node(region.root)
                        .map(|root| (region.pane, root.rect.width))
                }));
        for region in &workspace_regions.panes {
            if let Some(list) = layout.node(region.tab_list) {
                host.tab_scroll_rects
                    .insert(region.pane, list.scroll_viewport());
                scroll_changed |= self
                    .demo
                    .tab_scroll
                    .entry(region.pane)
                    .or_default()
                    .sync_layout(list);
            }
        }
        let tab_changed = reveal_active_tabs(
            &workspace_regions,
            &layout,
            &self.demo.workspace,
            &mut self.demo.tab_scroll,
            &mut host.tab_reveal,
        );
        if tab_changed || pane_width_changed {
            host.window.request_redraw();
        }
        host.focus_rects = actions
            .focus_order()
            .iter()
            .copied()
            .filter_map(|id| layout.node(id).map(|node| (id, node.rect)))
            .collect();
        let focus_root = self
            .owns_overlay(host.ui_window)
            .then(|| {
                self.demo
                    .dialog_stack
                    .top()
                    .map(|id| dialog_element(host.scopes.root, id, 0))
                    .or_else(|| {
                        self.demo
                            .palette_session
                            .is_open()
                            .then(|| host.scopes.root.element(PALETTE_ROOT))
                    })
            })
            .flatten();
        let popup_scope = FocusScope::new(host.ui_window, &actions, None);
        let combo_closed = self
            .demo
            .combo
            .reconcile(host.ui_window, &popup_scope)
            .is_some();
        let popover_closed = self
            .demo
            .popover
            .reconcile(host.ui_window, &popup_scope)
            .is_some();
        if combo_closed || popover_closed {
            self.native_windows.insert(window_id, host);
            self.request_all_redraw();
            return;
        }
        let scope = FocusScope::new(host.ui_window, &actions, focus_root);
        let focus_native = DockFocusTransfer::request(
            &mut self.dock_focus,
            &self.demo.workspace,
            ContainerRef::Floating(host.logical_id),
            host.ui_window,
            &actions,
            focus_root,
            &mut host.input,
        );
        if let Some(restored) = scope.resolve_request(&mut host.input) {
            let owner = host.ui_window;
            if focus_native && restored.is_some() {
                host.window.focus_window();
            }
            host.actions = actions;
            self.native_windows.insert(window_id, host);
            self.set_window_focus(owner, restored.map(|target| target.element));
            if let Some(target) = restored {
                self.reveal_window_focus(owner, target.element);
            }
            self.request_all_redraw();
            return;
        }
        let reconciled = scope.reconcile(&mut host.input);
        if reconciled.lost_focus || reconciled.lost_capture {
            let owner = host.ui_window;
            host.actions = actions;
            host.pointer_click.cancel();
            self.native_windows.insert(window_id, host);
            if reconciled.lost_focus {
                self.cancel_window_editors_for(owner, PopupCloseReason::OwnerRemoved);
            } else {
                self.cancel_window_workspace_drag(owner);
                self.cancel_window_value_edits(owner);
            }
            self.request_all_redraw();
            return;
        }
        host.actions = actions;
        frame_trace.mark(FrameStage::Reconcile);
        if wrap_changed {
            self.native_windows.insert(window_id, host);
            return;
        }
        let paint = PaintList::build_with_focus_and_interaction(
            &root,
            &layout,
            host.input.visible_focus().map(|focus| focus.element),
            self.theme.palette.focus,
            self.window_paint_interaction(
                host.cursor,
                host.scrollbar_drag.map(|(element, _)| element),
            ),
        );
        if complete_caret_reveal(
            &mut self.caret_reveal,
            self.text_session,
            CaretRevealFrame {
                window: host.ui_window,
                gallery: host.scopes.gallery,
                actions: &host.actions,
                focus: host.input.focus(),
                focus_root,
                geometry: &host.scroll_geometry,
                paint: &paint,
                layout_stable: !(wrap_changed || scroll_changed || tab_changed),
            },
            &mut ScrollStates {
                tree: &mut self.demo.tree_scroll,
                virtual_list: &mut self.demo.virtual_scroll,
                virtual_table: &mut self.demo.virtual_table_scroll,
                nested: &mut self.demo.nested_scroll,
                horizontal: &mut self.demo.horizontal_scroll,
                gallery: None,
                pages: &mut self.demo.page_scroll,
                tabs: &mut self.demo.tab_scroll,
            },
        ) {
            host.window.request_redraw();
            self.native_windows.insert(window_id, host);
            return;
        }
        frame_trace.mark(FrameStage::Paint);
        host.hit_test = HitTestTree::build(&root, &layout);
        frame_trace.mark(FrameStage::Hit);
        host.caret_blink.update(
            focused_field
                .filter(|_| window_active)
                .map(|field| field.element(host.scopes.gallery)),
            Instant::now(),
        );
        if self.capture_geometry_changed(
            host.ui_window,
            host.scopes.gallery,
            &root,
            &layout,
            &host.actions,
        ) {
            if let Ok(renderer) = host.surface.capture_renderer() {
                renderer.cancel_capture();
            }
            self.ui_query.cancel_capture_binding(host.ui_window);
        }
        let Some(gpu) = self.gpu.as_mut() else {
            self.native_windows.insert(window_id, host);
            return;
        };
        let profile_context = FrameContext {
            window: host.ui_window,
            gallery: true,
            text_focused: focused_field.is_some(),
            active: window_active,
        };
        let output = match gpu.render(
            Some(&mut host.surface),
            RenderFrame {
                viewport,
                paint: &paint,
                context: &mut self.text_context,
                rasterizer: &mut self.text_rasterizer,
                images: Some(&DemoImages::for_frame(host.smoke_frames)),
                options: RenderOptions {
                    clear: Some(theme_clear(self.theme, false)),
                    selection: self.theme.palette.selection,
                    caret: self.theme.palette.focus,
                    caret_visible: host.caret_blink.visible(),
                },
                scene: None,
                finish: self.smoke && !render_backend::retirement_probe_enabled(),
                cpu_profile: frame_trace.renderer_profile(),
                gpu_profile: &mut self.gpu_profile,
                profile_context,
            },
        ) {
            Ok(Some(output)) => output,
            Ok(None) => {
                self.native_windows.insert(window_id, host);
                if self.display_blocked(window_id) {
                    self.cancel_gpu_interactions(&std::collections::HashSet::from([window_id]));
                }
                return;
            }
            Err(error) => {
                self.native_windows.insert(window_id, host);
                self.gpu_fault(window_id, error);
                return;
            }
        };
        frame_trace.mark(FrameStage::Renderer);
        self.lifecycle_profile
            .presented(host.ui_window, input_watermark);
        self.gpu_recovery.presented(window_id);
        if self.smoke {
            host.smoke_frames += 1;
        }
        host.text_views =
            match text_views_for(host.ui_window, host.scopes.gallery, &output.text_inputs) {
                Ok(views) => views,
                Err(error) => {
                    self.fatal_error = Some(format!("text_input_views: {error:?}"));
                    self.native_windows.insert(window_id, host);
                    return;
                }
            };
        if let Some(view) = host.text_views.get(TextFieldKind::Area) {
            host.area_scroll_y = view.scroll_y;
        }
        frame_trace.mark(FrameStage::TextViews);
        // A modal dialog owned by this window isolates the semantic tree the same way the main
        // window does, so assistive technology cannot reach the background (ADR-0254).
        let isolated = if self.overlay_owner == Some(host.ui_window) {
            self.demo
                .dialog_stack
                .top()
                .map(|id| dialog_element(host.scopes.root, id, 0))
        } else {
            None
        };
        let mut update = host.semantics.build(
            accessibility_root(&root, isolated),
            &layout,
            host.input.focus().map(|focus| focus.element),
            &format!("{} — Vesta", display_text(&title, self.demo.locale)),
        );
        enrich_window_text(
            &mut host.semantics,
            host.scopes.gallery,
            &host.text_views,
            TextFieldStates {
                main: &mut self.text_state,
                search: &mut self.search_state,
                area: &mut self.area_state,
                number: &mut self.number_state,
                vector: &mut self.vector_states,
                color: &mut self.color_hex_state,
                password: &mut self.password_state,
                table_note: &mut self.table_note_state,
                readonly_label: &mut self.readonly_label_state,
            },
            &mut self.text_context,
            &mut update,
        );
        if let Err(error) = host
            .documents
            .enrich_accessibility(&mut host.semantics, &mut update)
        {
            eprintln!("document accessibility: {error}");
        }
        host.accessibility.update_if_active(|| {
            self.frame_profile.observe_native(|| {
                let owner = host.input.focus().map(|target| target.element);
                NativeObservation::Projection(NativeProjection::from_update(
                    host.ui_window,
                    host.window.has_focus(),
                    host.window.scale_factor(),
                    owner,
                    owner.and_then(|element| host.semantics.node_for_element(element)),
                    &update,
                ))
            });
            update
        });
        frame_trace.mark(FrameStage::Accessibility);
        if let Some(field) = focused_field
            && let Some(bridge) = self.native_text_bridges.get_mut(host.ui_window)
        {
            bridge.sync_ime(
                &host.window,
                field != TextFieldKind::Label,
                host.text_views.get(field),
            );
        }
        frame_trace.mark(FrameStage::Ime);
        host.busy_visible = gallery
            .busy_roots
            .iter()
            .any(|id| window_idle::visible_geometry(&layout, *id));
        self.frame_profile.submitted(frame_trace, profile_context);
        if let Some(snapshot) = self.capture_query_snapshot(
            host.ui_window,
            host.scopes.gallery,
            &root,
            &layout,
            &host.actions,
            host.input.focus(),
            focus_root,
        ) {
            self.ui_query.publish_root(
                snapshot,
                root,
                &host.actions,
                &layout,
                self.ui_input_context(host.ui_window, &host.window),
            );
        }
        self.native_windows.insert(window_id, host);
    }

    fn native_window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.recovery_input(window_id, &event) {
            return;
        }
        if let Some(window) = self
            .native_windows
            .get(&window_id)
            .map(|host| host.ui_window)
        {
            self.observe_text_pointer_event(window, &event);
        }
        if let Some(host) = self.native_windows.get_mut(&window_id) {
            if vesta_gpui_winit::observe_focus_visibility(&event, &mut host.input) {
                host.window.request_redraw();
            }
            if self.ui_query.input_origin == vesta_gpui_winit::UiInputOrigin::Native
                && let WindowEvent::MouseInput { state, button, .. } = &event
            {
                self.frame_profile.observe_native(|| {
                    let scale = host.window.scale_factor();
                    NativeObservation::Pointer {
                        window: host.ui_window,
                        state: *state,
                        button: *button,
                        logical: host.cursor,
                        derived_physical: (
                            f64::from(host.cursor.x) * scale,
                            f64::from(host.cursor.y) * scale,
                        ),
                        hit: host.hit_test.hit(host.cursor),
                    }
                });
            }
        }
        if let WindowEvent::ModifiersChanged(modifiers) = &event
            && let Some(bridge) = self
                .native_windows
                .get(&window_id)
                .and_then(|host| self.native_text_bridges.get_mut(host.ui_window))
        {
            bridge.update_modifiers(modifiers);
        }
        // A window that owns the caret consumes its own key and IME events, exactly like the
        // main window; without this the bridge would never see them (ADR-0252).
        let host_ui_window = self
            .native_windows
            .get(&window_id)
            .map(|host| host.ui_window);
        if let Some(ui_window) = host_ui_window {
            self.action_window = ui_window;
        }
        if let Some(window) = host_ui_window
            && (self.handle_palette_scroll_event(window, &event)
                || self.handle_suggestion_scroll_event(window, &event))
        {
            self.request_all_redraw();
            return;
        }
        if let Some(ui_window) = host_ui_window
            && self.handle_window_document_event(ui_window, &event)
        {
            self.request_all_redraw();
            return;
        }
        if let Some(ui_window) = host_ui_window
            && self.handle_window_keyboard_event(ui_window, &event)
        {
            self.request_all_redraw();
            if self.exit_requested {
                _event_loop.exit();
            }
            return;
        }
        if let Some(ui_window) = host_ui_window
            && matches!(
                event,
                WindowEvent::KeyboardInput { .. } | WindowEvent::Ime(_) | WindowEvent::Text(_)
            )
            && self.text_session_field(ui_window).is_some()
        {
            self.handle_window_text_event(ui_window, &event);
            if let Some(host) = self.native_windows.get(&window_id) {
                host.window.request_redraw();
            }
            return;
        }
        let menu_hover = matches!(&event, WindowEvent::CursorMoved { .. });
        let mut begin_native_menu = None;
        let mut action = None;
        let mut restore = false;
        let mut redraw = false;
        let mut placement = None;
        let mut drag_update = None;
        let mut split_resize_update = None;
        let mut finish_split_resize = None;
        let mut begin_drag = None;
        let mut finish_drag = false;
        let mut ramp_redraw = false;
        let mut finish_ramp = None;
        let mut cancel_values_window = None;
        let mut dismiss_popover = false;
        let mut dismiss_combo = false;
        let mut close_palette = false;
        // Precomputed before any host borrow: the drawer geometry only reads shared state.
        let drawer_state = self.drawer_state();
        let mut begin_text = None;
        let mut text_drag_update = None;
        let mut text_exit = None;
        let mut number_draft_dirty = false;
        let mut vector_draft_axis = None;
        let drag_active = self.workspace_drag.is_some();
        let source_container = self
            .native_windows
            .get(&window_id)
            .map(|host| ContainerRef::Floating(host.logical_id));
        match event {
            WindowEvent::CloseRequested => restore = true,
            WindowEvent::Focused(false) => {
                if let Some(window) = host_ui_window {
                    let focused = self.window_focus(window);
                    self.toast_focus.observe_window_activation(
                        window,
                        false,
                        focused.map(|element| InputTarget { window, element }),
                    );
                }
                if host_ui_window.is_some_and(|window| {
                    self.is_password_control(window, self.window_focus(window))
                }) {
                    self.demo.password_revealed = false;
                }
                if let Some(host) = self.native_windows.get_mut(&window_id) {
                    host.tooltip.set_active(false, Instant::now());
                    cancel_values_window = Some(host.ui_window);
                    text_exit = Some(TextSessionExit::WindowBlur(host.ui_window));
                }
                redraw = true;
            }
            WindowEvent::Focused(true) => {
                if let Some(host) = self.native_windows.get_mut(&window_id) {
                    self.toast_focus
                        .observe_window_activation(host.ui_window, true, None);
                    host.tooltip.set_active(true, Instant::now());
                }
                self.native_window_order.retain(|id| *id != window_id);
                self.native_window_order.push(window_id);
                redraw = true;
            }
            WindowEvent::Resized(size) => {
                finish_split_resize = host_ui_window.map(|window| (window, false));
                if let Some(window) = host_ui_window {
                    self.cancel_table_resize_for(window);
                    self.cancel_window_scrollbar_drag(window);
                }
                if let Some(host) = self.native_windows.get_mut(&window_id) {
                    host.surface.resize(size.width, size.height);
                }
                placement = self.native_placement_for_event(window_id, &event);
                redraw = true;
            }
            WindowEvent::Moved(_) => {
                placement = self.native_placement_for_event(window_id, &event);
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(window) = host_ui_window {
                    self.cancel_table_resize_for(window);
                    self.cancel_window_scrollbar_drag(window);
                }
                if let Some(host) = self.native_windows.get_mut(&window_id) {
                    let size = host.window.inner_size();
                    host.surface.resize(size.width, size.height);
                }
                redraw = true;
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(host) = self.native_windows.get_mut(&window_id) {
                    let logical = position.to_logical::<f32>(host.window.scale_factor());
                    host.cursor = Point::new(logical.x, logical.y);
                    host.tooltip.hover(
                        host.hit_test.hit(host.cursor),
                        &host.actions,
                        Instant::now(),
                    );
                    let size = host
                        .window
                        .inner_size()
                        .to_logical::<f32>(host.window.scale_factor());
                    let resize = (!host.window.is_maximized() && cfg!(target_os = "windows"))
                        .then(|| {
                            chrome_resize_direction(host.cursor, Size::new(size.width, size.height))
                        })
                        .flatten();
                    host.window
                        .set_cursor(resize.map(CursorIcon::from).unwrap_or(CursorIcon::Default));
                    if let (Some(track), Some(drag)) =
                        (host.ramp_track_rect, host.ramp_drag.as_mut())
                    {
                        ramp_redraw = update_ramp_drag_at(
                            drag,
                            host.ui_window,
                            track,
                            host.cursor.x,
                            &mut self.demo.ramp,
                        ) || ramp_redraw;
                    }
                    if host
                        .slider_drag
                        .as_ref()
                        .is_some_and(|edit| edit.accepts(host.ui_window))
                        && let Some(rect) = host.slider_rect
                    {
                        let value = slider_value_at(rect, host.cursor).clamp(0.0, 1.0);
                        if value != self.demo.slider {
                            self.demo.slider = value;
                            ramp_redraw = true;
                        }
                    }
                    if let Some(mut scrub) = host.number_scrub {
                        let next = number_scrub_value(
                            &mut scrub,
                            host.ui_window,
                            host.cursor.x,
                            self.demo.number,
                        );
                        host.number_scrub = Some(scrub);
                        if let Some(value) = next {
                            self.demo.number = value;
                            number_draft_dirty = true;
                            ramp_redraw = true;
                        }
                    }
                    if let Some(mut scrub) = host.vector_scrub {
                        let current = self.demo.vector[usize::from(scrub.axis)];
                        let next =
                            vector_scrub_value(&mut scrub, host.ui_window, host.cursor.x, current);
                        host.vector_scrub = Some(scrub);
                        if let Some(value) = next {
                            self.demo.vector[usize::from(scrub.axis)] = Some(value);
                            vector_draft_axis = Some(scrub.axis);
                            ramp_redraw = true;
                        }
                    }
                    if let Some(mut drag) = host.drawer_resize
                        && let Some(delta) = drag.offset(host.ui_window, host.cursor.x)
                    {
                        host.drawer_resize = Some(drag);
                        let width = sidebar_drawer_width_at(*drag.initial(), delta, drawer_state);
                        if (width - self.demo.drawer_width).abs() > f32::EPSILON {
                            self.demo.drawer_width = width;
                            ramp_redraw = true;
                        }
                    }
                    if let Some(mut drag) = host.table_resize
                        && let Some(rect) = host.table_rect
                        && let Some(delta) = drag.session.offset(host.ui_window, host.cursor.x)
                    {
                        host.table_resize = Some(drag);
                        let table_width = rect.width.max(1.0);
                        if let Some(spec) = table_resize_width_bounds(
                            &self.demo.table_column_fractions,
                            table_width,
                            drag.column,
                        ) && let Some(initial_widths) =
                            table_column_widths(drag.session.initial(), table_width)
                        {
                            let start_width = initial_widths[drag.column];
                            let spec = TableColumnResize {
                                handle: WidgetIdentity::new(
                                    table_resize_element(host.scopes.gallery, drag.column),
                                    format!("gallery.table.column.{}.resize", drag.column),
                                ),
                                min_width: spec.0,
                                max_width: spec.1,
                                keyboard_step: 8.0,
                            };
                            let width = table_column_width_at(start_width, delta, &spec);
                            if let Some(next) = table_column_fractions_after(
                                &self.demo.table_column_fractions,
                                table_width,
                                drag.column,
                                width,
                            ) {
                                self.demo.table_column_fractions = next;
                                ramp_redraw = true;
                            }
                        }
                    }
                    if let Some((element, drag)) = host.scrollbar_drag {
                        let mut states = ScrollStates {
                            tree: &mut self.demo.tree_scroll,
                            virtual_list: &mut self.demo.virtual_scroll,
                            virtual_table: &mut self.demo.virtual_table_scroll,
                            nested: &mut self.demo.nested_scroll,
                            horizontal: &mut self.demo.horizontal_scroll,
                            gallery: None,
                            pages: &mut self.demo.page_scroll,
                            tabs: &mut self.demo.tab_scroll,
                        };
                        if update_scrollbar_drag_state(
                            host.scopes.gallery,
                            &host.scroll_geometry,
                            &mut states,
                            element,
                            drag,
                            host.cursor,
                        ) {
                            ramp_redraw = true;
                        }
                    }
                    text_drag_update = Some((host.ui_window, host.cursor));
                    split_resize_update = Some((host.ui_window, host.cursor));
                    drag_update = Some((ContainerRef::Floating(host.logical_id), host.cursor));
                }
                redraw = true;
            }
            WindowEvent::CursorLeft { .. } => {
                if let Some(host) = self.native_windows.get_mut(&window_id) {
                    host.pointer_click.cancel();
                    host.cursor = Point::new(-1.0, -1.0);
                    host.tooltip.leave(Instant::now());
                }
                redraw = true;
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                if let Some(host) = self.native_windows.get_mut(&window_id) {
                    host.pointer_click.cancel();
                    let tab = host
                        .hit_test
                        .hit(host.cursor)
                        .and_then(|hit| host.actions.action_for_hit(hit))
                        .and_then(|action| match action {
                            DemoAction::Workspace(WorkspaceViewAction::Tab(item)) => Some(*item),
                            _ => None,
                        });
                    begin_native_menu = Some((host.ui_window, tab, host.cursor));
                }
                redraw = true;
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(host) = self.native_windows.get_mut(&window_id) {
                    let hit = host.hit_test.hit(host.cursor);
                    let scrollbar = {
                        let mut states = ScrollStates {
                            tree: &mut self.demo.tree_scroll,
                            virtual_list: &mut self.demo.virtual_scroll,
                            virtual_table: &mut self.demo.virtual_table_scroll,
                            nested: &mut self.demo.nested_scroll,
                            horizontal: &mut self.demo.horizontal_scroll,
                            gallery: None,
                            pages: &mut self.demo.page_scroll,
                            tabs: &mut self.demo.tab_scroll,
                        };
                        begin_scrollbar_drag(
                            host.scopes.gallery,
                            &host.scroll_geometry,
                            &host.actions,
                            &mut states,
                            hit,
                            host.cursor,
                        )
                    };
                    // A painted thumb may overlap the frameless resize strip.
                    if scrollbar.is_none()
                        && self.ui_query.input_origin == vesta_gpui_winit::UiInputOrigin::Native
                        && native_window::try_window_chrome_press(
                            &host.window,
                            host.cursor,
                            &host.hit_test,
                            &host.actions,
                            &mut host.pointer_click,
                            &mut host.last_title_click,
                        )
                    {
                        return;
                    }
                    if host.pane_menu.is_some()
                        && !hit.is_some_and(|hit| {
                            host.actions
                                .is_descendant_of(hit, host.scopes.menu.element(1))
                        })
                    {
                        host.close_pane_menu();
                    }
                    // Same non-modal rule as the main window (ADR-0140): a press outside the
                    // overlay closes it while keeping its own action, and a press inside it is
                    // left to the routed action below.
                    dismiss_popover = self.demo.popover.opener().is_some_and(|opener| {
                        popup_dismiss_for_hit(
                            opener,
                            host.scopes.popover.element(POPOVER_ROOT),
                            host.ui_window,
                            hit,
                            &host.actions,
                        )
                    });
                    dismiss_combo = self.demo.combo.opener().is_some_and(|opener| {
                        popup_dismiss_for_hit(
                            opener,
                            host.scopes.gallery.element(COMBO_POPUP),
                            host.ui_window,
                            hit,
                            &host.actions,
                        )
                    });
                    let routed = hit
                        .and_then(|hit| host.actions.target_for_hit(hit))
                        .map(|(element, action)| (element, *action));
                    let ramp_point = hit.and_then(|hit| ramp_point_action(&host.actions, hit));
                    let text_field = hit.and_then(|hit| {
                        TextFieldKind::ALL
                            .into_iter()
                            .find(|field| field.element(host.scopes.gallery) == hit)
                    });
                    // Scrollbars first, like the main window: the thumb owns the press when the
                    // pointer is on it. A palette owned by this window closes on its backdrop
                    // before anything else (ADR-0254).
                    if self.demo.palette_session.is_open()
                        && self.overlay_owner == Some(host.ui_window)
                        && hit == Some(host.scopes.root.element(PALETTE_ROOT))
                    {
                        close_palette = true;
                    } else if let Some((element, drag)) = scrollbar {
                        host.input.set_focus(Some(InputTarget {
                            window: host.ui_window,
                            element,
                        }));
                        host.input.capture_pointer(
                            InputTarget {
                                window: host.ui_window,
                                element,
                            },
                            PointerButton::Primary,
                        );
                        host.scrollbar_drag = Some((element, drag));
                        text_exit = Some(TextSessionExit::FocusChange);
                    } else if let Some(field) = text_field {
                        begin_text = Some((host.ui_window, field, host.cursor));
                        match field {
                            TextFieldKind::Number => {
                                host.number_scrub = NumberScrub::new(
                                    InputTarget {
                                        window: host.ui_window,
                                        element: host.scopes.gallery.element(NUMBER_FIELD),
                                    },
                                    host.cursor.x,
                                    self.demo.number,
                                    4.0,
                                )
                                .ok();
                            }
                            TextFieldKind::Vector(axis) => {
                                host.vector_scrub =
                                    self.demo.vector.get(usize::from(axis)).and_then(|value| {
                                        Some(VectorScrub {
                                            axis,
                                            session: HorizontalEditSession::new(
                                                InputTarget {
                                                    window: host.ui_window,
                                                    element: host.scopes.gallery.element(
                                                        VECTOR_VALUE_BASE + u32::from(axis),
                                                    ),
                                                },
                                                host.cursor.x,
                                                *value,
                                                4.0,
                                            )
                                            .ok()?,
                                        })
                                    });
                            }
                            _ => {}
                        }
                    } else if hit == Some(host.scopes.gallery.element(DRAWER_RESIZE))
                        && self.demo.drawer_open
                    {
                        let element = host.scopes.gallery.element(DRAWER_RESIZE);
                        host.input.set_focus(Some(InputTarget {
                            window: host.ui_window,
                            element,
                        }));
                        host.input.capture_pointer(
                            InputTarget {
                                window: host.ui_window,
                                element,
                            },
                            PointerButton::Primary,
                        );
                        host.drawer_resize = DrawerResizeDrag::new(
                            InputTarget {
                                window: host.ui_window,
                                element,
                            },
                            host.cursor.x,
                            self.demo.drawer_width,
                            0.0,
                        )
                        .ok();
                        text_exit = Some(TextSessionExit::FocusChange);
                    } else if let Some(column) = (0..2).find(|column| {
                        hit == Some(table_resize_element(host.scopes.gallery, *column))
                    }) {
                        if let Some(rect) = host.table_rect
                            && table_resize_width_bounds(
                                &self.demo.table_column_fractions,
                                rect.width.max(1.0),
                                column,
                            )
                            .is_some()
                        {
                            let element = table_resize_element(host.scopes.gallery, column);
                            host.input.set_focus(Some(InputTarget {
                                window: host.ui_window,
                                element,
                            }));
                            host.input.capture_pointer(
                                InputTarget {
                                    window: host.ui_window,
                                    element,
                                },
                                PointerButton::Primary,
                            );
                            host.table_resize = HorizontalEditSession::new(
                                InputTarget {
                                    window: host.ui_window,
                                    element,
                                },
                                host.cursor.x,
                                self.demo.table_column_fractions,
                                0.0,
                            )
                            .ok()
                            .map(|session| TableResizeDrag { column, session });
                            text_exit = Some(TextSessionExit::FocusChange);
                        }
                    } else if let Some((element, id)) = ramp_point
                        && let Some(track) = host.ramp_track_rect
                    {
                        let drag = press_ramp_point(
                            &mut self.demo.ramp,
                            InputTarget {
                                window: host.ui_window,
                                element,
                            },
                            track,
                            id,
                            host.cursor.x,
                            SelectionModifiers {
                                additive: self
                                    .native_text_bridges
                                    .get(host.ui_window)
                                    .is_some_and(|bridge| bridge.modifiers().control_key()),
                                range: self
                                    .native_text_bridges
                                    .get(host.ui_window)
                                    .is_some_and(|bridge| bridge.modifiers().shift_key()),
                            },
                        );
                        if drag.is_some() {
                            host.input.capture_pointer(
                                InputTarget {
                                    window: host.ui_window,
                                    element,
                                },
                                PointerButton::Primary,
                            );
                        }
                        host.pointer_click.cancel();
                        host.input.set_focus(Some(InputTarget {
                            window: host.ui_window,
                            element,
                        }));
                        host.ramp_drag = drag;
                        text_exit = Some(TextSessionExit::FocusChange);
                        ramp_redraw = true;
                    } else if let Some((element, DemoAction::SetSlider)) = routed {
                        // The track starts its drag on press, exactly like the main window.
                        if let Some(rect) = host.slider_rect {
                            host.slider_drag = Some(ValueEditSession::new(
                                InputTarget {
                                    window: host.ui_window,
                                    element,
                                },
                                self.demo.slider,
                            ));
                            host.input.set_focus(Some(InputTarget {
                                window: host.ui_window,
                                element,
                            }));
                            host.input.capture_pointer(
                                InputTarget {
                                    window: host.ui_window,
                                    element,
                                },
                                PointerButton::Primary,
                            );
                            self.demo.slider = slider_value_at(rect, host.cursor).clamp(0.0, 1.0);
                            ramp_redraw = true;
                        } else {
                            host.pointer_click.press(
                                host.ui_window,
                                PointerButton::Primary,
                                &host.actions,
                                hit,
                            );
                        }
                    } else if let Some((element, routed_action)) = routed {
                        host.input.set_focus(Some(InputTarget {
                            window: host.ui_window,
                            element,
                        }));
                        if let DemoAction::Workspace(WorkspaceViewAction::Tab(item)) = routed_action
                        {
                            host.pointer_click.cancel();
                            begin_drag = Some((
                                item,
                                ContainerRef::Floating(host.logical_id),
                                host.cursor,
                                routed_action,
                            ));
                        } else if routed_action.starts_pointer_drag() {
                            host.pointer_click.cancel();
                            action = Some(routed_action);
                        } else {
                            host.pointer_click.press(
                                host.ui_window,
                                PointerButton::Primary,
                                &host.actions,
                                hit,
                            );
                        }
                    } else {
                        host.pointer_click.press(
                            host.ui_window,
                            PointerButton::Primary,
                            &host.actions,
                            hit,
                        );
                    }
                }
                redraw = true;
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(host) = self.native_windows.get_mut(&window_id) {
                    finish_split_resize = Some((host.ui_window, true));
                    let hit = host.hit_test.hit(host.cursor);
                    action = host
                        .pointer_click
                        .release(host.ui_window, PointerButton::Primary, &host.actions, hit)
                        .copied();
                    // Without this the window keeps the capture after the button is up, so a
                    // text selection or ramp drag would keep following the pointer.
                    host.input
                        .release_pointer(host.ui_window, PointerButton::Primary);
                    if let Some(mut edit) = host.slider_drag.take() {
                        edit.finish(host.ui_window, &self.demo.slider, true);
                    }
                    if let Some(mut scrub) = host.number_scrub.take() {
                        scrub.finish(host.ui_window, &self.demo.number, true);
                    }
                    if let Some(mut scrub) = host.vector_scrub.take() {
                        let current = self.demo.vector[usize::from(scrub.axis)];
                        scrub.finish(host.ui_window, &current, true);
                    }
                    if let Some(mut drag) = host.drawer_resize.take() {
                        drag.finish(host.ui_window, &self.demo.drawer_width, true);
                    }
                    if let Some(mut drag) = host.table_resize.take() {
                        drag.session.finish(
                            host.ui_window,
                            &self.demo.table_column_fractions,
                            true,
                        );
                    }
                    finish_scrollbar_drag(&mut host.scrollbar_drag, MouseButton::Left);
                    if let Some(mut drag) = host.ramp_drag.take()
                        && let Some(PointerEditOutcome::Commit { before, .. }) = drag
                            .session
                            .finish(host.ui_window, &self.demo.ramp.snapshot(), true)
                    {
                        finish_ramp = Some(before);
                    }
                }
                finish_drag = drag_active;
                redraw = true;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // Same inner-to-outer consumption as the main window, through this window's
                // own scroll geometry (ADR-0253 decision 7).
                let mut movement = match delta {
                    MouseScrollDelta::LineDelta(x, y) => Point::new(-x * 34.0, -y * 34.0),
                    MouseScrollDelta::PixelDelta(position) => {
                        let scale = self
                            .native_windows
                            .get(&window_id)
                            .map_or(1.0, |host| host.window.scale_factor());
                        let point = logical_point(position, scale);
                        Point::new(-point.x, -point.y)
                    }
                };
                if host_ui_window.is_some_and(|window| self.window_modifiers(window).shift_key())
                    && movement.x.abs() <= f32::EPSILON
                {
                    movement.x = movement.y;
                    movement.y = 0.0;
                }
                // The TextArea viewport and the Pane tab strip belong to this window, so they
                // take the wheel before the shared scroll path (ADR-0253 decision 7).
                if movement.x.is_finite()
                    && movement.y.is_finite()
                    && (movement.x.abs() > f32::EPSILON || movement.y.abs() > f32::EPSILON)
                    && let Some(window) = host_ui_window
                {
                    retire_caret_reveal(&mut self.caret_reveal, window);
                }
                if let Some(host) = self.native_windows.get(&window_id)
                    && let Some(view) = host.text_views.get(TextFieldKind::Area)
                    && view.bounds.contains(host.cursor)
                    && host.hit_test.hit(host.cursor)
                        == Some(host.scopes.gallery.element(TEXT_AREA))
                    && movement.y.abs() > f32::EPSILON
                {
                    restore_area_view_layout(&mut self.area_state, &mut self.text_context, view);
                    let limit = (self.area_state.snapshot().layout_height()
                        - (view.bounds.height - 4.0).max(0.0))
                    .max(0.0);
                    let next = (host.area_scroll_y + movement.y).clamp(0.0, limit);
                    let consumed = next - host.area_scroll_y;
                    if consumed.abs() > f32::EPSILON {
                        if let Some(host) = self.native_windows.get_mut(&window_id) {
                            host.area_scroll_y = next;
                            host.area_follow_caret = false;
                        }
                        movement.y -= consumed;
                    }
                }
                let tab_strip = self.native_windows.get(&window_id).and_then(|host| {
                    host.scroll_geometry
                        .tabs
                        .values()
                        .find(|(_, rect)| rect.contains(host.cursor))
                        .map(|&(pane, _)| pane)
                });
                if let Some(pane) = tab_strip {
                    let horizontal = if movement.x.abs() > f32::EPSILON {
                        movement.x
                    } else {
                        movement.y
                    };
                    self.demo
                        .tab_scroll
                        .entry(pane)
                        .or_default()
                        .scroll_by_point(Point::new(horizontal, 0.0));
                    if let Some(host) = self.native_windows.get(&window_id) {
                        host.window.request_redraw();
                    }
                }
                if tab_strip.is_none()
                    && let Some(host) = self.native_windows.get(&window_id)
                {
                    let gallery = host.scopes.gallery;
                    let hit = host.hit_test.hit(host.cursor);
                    let geometry = host.scroll_geometry.clone();
                    let mut states = ScrollStates {
                        tree: &mut self.demo.tree_scroll,
                        virtual_list: &mut self.demo.virtual_scroll,
                        virtual_table: &mut self.demo.virtual_table_scroll,
                        nested: &mut self.demo.nested_scroll,
                        horizontal: &mut self.demo.horizontal_scroll,
                        gallery: None,
                        pages: &mut self.demo.page_scroll,
                        tabs: &mut self.demo.tab_scroll,
                    };
                    wheel_scroll_path(
                        host.ui_window,
                        &host.actions,
                        &mut states,
                        hit,
                        &geometry,
                        gallery,
                        movement,
                    );
                }
                redraw = true;
            }
            WindowEvent::KeyboardInput {
                event: key_event,
                is_synthetic: false,
                ..
            } if key_event.state == ElementState::Pressed => {
                // Shared routing has handled controls, text barriers and shortcuts.
                // An unclaimed Escape retains the floating host's Dock-back convention.
                if key_event.logical_key == Key::Named(NamedKey::Escape) {
                    restore = true;
                }
            }
            WindowEvent::RedrawRequested => self.redraw_native_window(window_id),
            _ => {}
        }
        if let Some((window, tab, anchor)) = begin_native_menu
            && let Some(context) = self.pane_context_at(window, tab)
        {
            self.open_window_menu(
                window,
                if tab.is_some() { MenuId(6) } else { MenuId(1) },
                anchor,
                context,
            );
        }
        if menu_hover && let Some(host) = self.native_windows.get(&window_id) {
            self.hover_window_menu_item(host.ui_window, host.hit_test.hit(host.cursor));
        }
        if let Some((item, container, origin, activate)) = begin_drag {
            self.workspace_drag = Some(WorkspaceTabDrag {
                item,
                origin_container: container,
                origin,
                started: false,
                pointer_container: Some(container),
                cursor: origin,
                target: None,
            });
            self.apply_demo_action(activate);
            self.request_all_redraw();
        }
        if let Some((container, cursor)) = drag_update
            && self.update_native_dock_pointer(container, cursor)
        {
            self.request_all_redraw();
        }
        if let Some((window, pointer)) = split_resize_update
            && self.update_workspace_resize(window, pointer)
        {
            self.request_all_redraw();
        }
        if let Some((window, commit)) = finish_split_resize
            && self.finish_workspace_resize_for(window, commit)
        {
            action = None;
            self.request_all_redraw();
        }
        if finish_drag && source_container.is_some_and(|source| self.finish_workspace_drag(source))
        {
            action = None;
        }
        if let Some(snapshot) = finish_ramp {
            self.demo.ramp.commit_snapshot(snapshot);
            ramp_redraw = true;
        }
        if dismiss_popover {
            self.close_popover_for(self.action_window, PopupCloseReason::FocusTransfer);
        }
        if dismiss_combo {
            self.close_combo_for(self.action_window, PopupCloseReason::FocusTransfer);
        }
        if close_palette {
            // The modal backdrop consumes this press; use the main window's cancellation
            // path so focus returns instead of preserving a non-modal outside target.
            self.close_command_palette();
            self.request_all_redraw();
        }
        if let Some((window, field, pointer)) = begin_text {
            self.begin_text_selection(window, field, Some(pointer));
            self.request_all_redraw();
        }
        if let Some((window, pointer)) = text_drag_update
            && self.update_window_text_selection(window, pointer)
        {
            self.request_all_redraw();
        }
        if let Some(window) = cancel_values_window {
            if matches!(text_exit, Some(TextSessionExit::WindowBlur(_))) {
                self.cancel_window_editors_for(window, PopupCloseReason::FocusTransfer);
            } else {
                self.cancel_window_value_edits(window);
            }
        }
        self.complete_window_text_event(text_exit, number_draft_dirty, vector_draft_axis);
        if ramp_redraw {
            self.request_all_redraw();
        }
        if let Some((logical_id, placement)) = placement
            && placement.x.is_finite()
            && placement.y.is_finite()
            && placement.width.is_finite()
            && placement.height.is_finite()
            && placement.width >= 1.0
            && placement.height >= 1.0
            && let Some(window) = self.demo.workspace.floating_window_mut(logical_id)
        {
            window.placement = placement;
        }
        if restore {
            self.restore_native_window(window_id);
            return;
        }
        if let Some(action) = action {
            self.apply_native_action(window_id, action);
            self.request_all_redraw();
        } else if redraw && let Some(host) = self.native_windows.get(&window_id) {
            host.window.request_redraw();
        }
    }

    fn native_accessibility_action(
        &mut self,
        window_id: WindowId,
        request: accesskit::ActionRequest,
    ) {
        if let Some(window) = self
            .native_windows
            .get(&window_id)
            .map(|host| host.ui_window)
        {
            self.window_accessibility_action(window, request);
        }
    }

    fn workspace_cursor_icon(&self) -> Option<CursorIcon> {
        let axis = self.workspace_resize.map(|drag| drag.axis()).or_else(|| {
            if self.workspace_drag.is_some() || self.floating_move.is_some() {
                return None;
            }
            let hit = self.hit_test.hit(self.cursor)?;
            match self.ui_actions.action_for_hit(hit)? {
                DemoAction::Workspace(WorkspaceViewAction::BeginResize { axis, .. }) => Some(*axis),
                _ => None,
            }
        })?;
        Some(match axis {
            SplitAxis::Row => CursorIcon::EwResize,
            SplitAxis::Column => CursorIcon::NsResize,
        })
    }

    fn table_resize_cursor_icon(&self) -> Option<CursorIcon> {
        if self.table_resize.is_some()
            || self.hit_test.hit(self.cursor).is_some_and(|hit| {
                (0..2).any(|column| hit == table_resize_element(self.scopes.gallery, column))
            })
        {
            Some(CursorIcon::EwResize)
        } else {
            None
        }
    }

    fn drawer_resize_cursor_icon(&self) -> Option<CursorIcon> {
        if self.drawer_resize.is_some()
            || self.hit_test.hit(self.cursor) == Some(self.scopes.gallery.element(DRAWER_RESIZE))
        {
            Some(CursorIcon::EwResize)
        } else {
            None
        }
    }

    fn update_resize_cursor(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        let size = window.inner_size().to_logical::<f32>(window.scale_factor());
        let direction = (!window.is_maximized())
            .then(|| chrome_resize_direction(self.cursor, Size::new(size.width, size.height)))
            .flatten();
        let icon = direction
            .map(CursorIcon::from)
            .or_else(|| self.drawer_resize_cursor_icon())
            .or_else(|| self.table_resize_cursor_icon())
            .or_else(|| self.workspace_cursor_icon())
            .unwrap_or(CursorIcon::Default);
        if self.cursor_icon != icon {
            window.set_cursor(icon);
            self.cursor_icon = icon;
        }
    }

    fn focused_text_field(&self) -> Option<TextFieldKind> {
        self.text_session_field(STANDALONE_WINDOW)
    }

    fn drawer_state(&self) -> SidebarDrawerState<'static> {
        window_controls::drawer_state(&self.demo)
    }

    fn begin_drawer_resize(&mut self) -> bool {
        if !self.demo.drawer_open {
            return false;
        }
        let element = self.scopes.gallery.element(DRAWER_RESIZE);
        self.set_text_focus(None);
        self.set_focus_target(Some(element));
        self.input_ownership.capture_pointer(
            InputTarget {
                window: STANDALONE_WINDOW,
                element,
            },
            PointerButton::Primary,
        );
        self.drawer_resize = DrawerResizeDrag::new(
            InputTarget {
                window: STANDALONE_WINDOW,
                element,
            },
            self.cursor.x,
            self.demo.drawer_width,
            0.0,
        )
        .ok();
        self.drawer_resize.is_some()
    }

    fn update_drawer_resize(&mut self) -> bool {
        let Some(mut drag) = self.drawer_resize else {
            return false;
        };
        let Some(delta) = drag.offset(STANDALONE_WINDOW, self.cursor.x) else {
            return false;
        };
        self.drawer_resize = Some(drag);
        let width = sidebar_drawer_width_at(*drag.initial(), delta, self.drawer_state());
        if (width - self.demo.drawer_width).abs() <= f32::EPSILON {
            return false;
        }
        self.demo.drawer_width = width;
        true
    }

    fn finish_drawer_resize(&mut self) -> bool {
        self.drawer_resize.take().is_some_and(|mut drag| {
            drag.finish(STANDALONE_WINDOW, &self.demo.drawer_width, true)
                .is_some()
        })
    }

    fn cancel_drawer_resize(&mut self) -> bool {
        let Some(mut drag) = self.drawer_resize.take() else {
            return false;
        };
        if let Some(PointerEditOutcome::Cancel { restore }) =
            drag.finish(STANDALONE_WINDOW, &self.demo.drawer_width, false)
        {
            self.demo.drawer_width = restore;
        }
        self.input_ownership.cancel_capture();
        true
    }

    /// The focused element of one window.
    fn window_focus(&self, window: UiWindowId) -> Option<ElementId> {
        if window == STANDALONE_WINDOW {
            return self.input_ownership.focus().map(|focus| focus.element);
        }
        self.native_windows
            .values()
            .find(|host| host.ui_window == window)
            .and_then(|host| host.input.focus())
            .map(|focus| focus.element)
    }

    /// Reveal remains active while focus moves among the field's own controls.
    fn is_password_control(&self, window: UiWindowId, element: Option<ElementId>) -> bool {
        self.gallery_scopes(window).is_some_and(|gallery| {
            [PASSWORD_ROOT, PASSWORD_ROOT + 3, PASSWORD_ROOT + 5]
                .into_iter()
                .any(|id| element == Some(gallery.element(id)))
        })
    }

    /// Whether this window should project the palette or a modal dialog it opened.
    fn owns_overlay(&self, window: UiWindowId) -> bool {
        self.overlay_owner == Some(window)
    }

    /// The text field a window's element id names, using that window's own scopes.
    fn text_field_at(
        &self,
        window: UiWindowId,
        element: Option<ElementId>,
    ) -> Option<TextFieldKind> {
        let element = element?;
        let scopes = self.gallery_scopes(window)?;
        TextFieldKind::ALL
            .into_iter()
            .find(|field| field.element(scopes) == element)
    }

    /// The caret owner for one window, if that window holds the text session.
    fn text_session_field(&self, window: UiWindowId) -> Option<TextFieldKind> {
        self.text_session
            .filter(|session| session.window == window)
            .map(|session| session.field)
    }

    fn set_text_focus(&mut self, field: Option<TextFieldKind>) {
        self.set_focus_target(field.map(|field| field.element(self.scopes.gallery)));
    }

    fn open_menu_for(&mut self, menu: MenuId, anchor: Point) {
        if let Some(context) = self.pane_context_at(STANDALONE_WINDOW, self.demo.tab_menu_item) {
            self.open_window_menu(STANDALONE_WINDOW, menu, anchor, context);
        }
    }

    fn hover_menu_item(&mut self, hovered: Option<ElementId>) {
        self.hover_window_menu_item(STANDALONE_WINDOW, hovered);
    }

    fn open_menu(&mut self, anchor: Point) {
        self.open_menu_for(MenuId(1), anchor);
    }

    fn open_context_menu(&mut self, anchor: Point) {
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

    fn close_menu(&mut self, restore_focus: bool) {
        self.close_window_menu(
            STANDALONE_WINDOW,
            if restore_focus {
                PopupCloseReason::Explicit
            } else {
                PopupCloseReason::FocusTransfer
            },
        );
    }

    #[cfg(test)]
    fn traverse_focus(&mut self, backwards: bool) {
        self.traverse_window_focus(STANDALONE_WINDOW, backwards);
    }

    fn accessibility_action(&mut self, request: accesskit::ActionRequest) {
        self.window_accessibility_action(STANDALONE_WINDOW, request);
    }

    /// Press on a text field of the main window; native floating windows call
    /// [`App::begin_text_selection`] with their own window and pointer.
    fn start_text_selection(&mut self, field: TextFieldKind) {
        self.begin_text_selection(STANDALONE_WINDOW, field, Some(self.cursor));
    }

    fn try_scrollbar_press(&mut self) -> bool {
        if self.mode != AcceptanceMode::Gallery2d
            || self.demo.palette_session.is_open()
            || !self.demo.dialog_stack.is_empty()
        {
            return false;
        }
        let hit = self.hit_test.hit(self.cursor);
        let gallery = self.scopes.gallery;
        let pointer = self.cursor;
        let geometry = self.scroll_geometry.clone();
        let mut states = ScrollStates {
            tree: &mut self.demo.tree_scroll,
            virtual_list: &mut self.demo.virtual_scroll,
            virtual_table: &mut self.demo.virtual_table_scroll,
            nested: &mut self.demo.nested_scroll,
            horizontal: &mut self.demo.horizontal_scroll,
            gallery: Some(&mut self.gallery_scroll),
            pages: &mut self.demo.page_scroll,
            tabs: &mut self.demo.tab_scroll,
        };
        let Some((element, drag)) = begin_scrollbar_drag(
            gallery,
            &geometry,
            &self.ui_actions,
            &mut states,
            hit,
            pointer,
        ) else {
            return false;
        };
        self.set_text_focus(None);
        self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
        self.input_ownership.capture_pointer(
            InputTarget {
                window: STANDALONE_WINDOW,
                element,
            },
            PointerButton::Primary,
        );
        self.scrollbar_drag = Some((element, drag));
        true
    }

    fn update_scrollbar_drag(&mut self) {
        let Some((element, drag)) = self.scrollbar_drag else {
            return;
        };
        let gallery = self.scopes.gallery;
        let pointer = self.cursor;
        let geometry = self.scroll_geometry.clone();
        let mut states = ScrollStates {
            tree: &mut self.demo.tree_scroll,
            virtual_list: &mut self.demo.virtual_scroll,
            virtual_table: &mut self.demo.virtual_table_scroll,
            nested: &mut self.demo.nested_scroll,
            horizontal: &mut self.demo.horizontal_scroll,
            gallery: Some(&mut self.gallery_scroll),
            pages: &mut self.demo.page_scroll,
            tabs: &mut self.demo.tab_scroll,
        };
        if update_scrollbar_drag_state(gallery, &geometry, &mut states, element, drag, pointer) {
            self.request_redraw();
        }
    }

    fn pointer_press(&mut self) {
        self.pointer_click.cancel();
        if self.demo.palette_session.is_open()
            && self.hit_test.hit(self.cursor) == Some(self.scopes.root.element(PALETTE_ROOT))
        {
            self.close_command_palette();
            self.request_redraw();
            return;
        }
        if self.try_scrollbar_press() {
            self.request_redraw();
            return;
        }
        let hit = self.hit_test.hit(self.cursor);
        if self.demo.status_overflow_open
            && !hit.is_some_and(|element| {
                self.ui_actions
                    .is_descendant_of(element, self.scopes.status.element(30))
                    || self
                        .ui_actions
                        .is_descendant_of(element, self.scopes.status.element(40))
            })
        {
            self.demo.status_overflow_open = false;
        }
        self.dismiss_popover_for_hit(hit);
        self.dismiss_combo_for_hit(hit);
        match hit {
            Some(hit)
                if self.mode == AcceptanceMode::Gallery2d
                    && hit == self.scopes.gallery.element(DRAWER_RESIZE) =>
            {
                self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
                self.begin_drawer_resize();
            }
            Some(hit)
                if self.mode == AcceptanceMode::Gallery2d
                    && (0..2)
                        .any(|column| hit == table_resize_element(self.scopes.gallery, column)) =>
            {
                self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
                let column = (0..2)
                    .find(|column| hit == table_resize_element(self.scopes.gallery, *column))
                    .expect("matched table resize column");
                self.begin_table_resize(column);
            }
            Some(hit)
                if self.mode == AcceptanceMode::Gallery2d
                    && TextFieldKind::ALL
                        .into_iter()
                        .any(|field| field.element(self.scopes.gallery) == hit) =>
            {
                let field = TextFieldKind::ALL
                    .into_iter()
                    .find(|field| field.element(self.scopes.gallery) == hit)
                    .expect("matched a stable text field");
                self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
                self.start_text_selection(field);
                match field {
                    TextFieldKind::Number => self.begin_number_scrub(),
                    TextFieldKind::Vector(axis) => self.begin_vector_scrub(axis),
                    _ => {}
                }
            }
            Some(hit)
                if self.mode == AcceptanceMode::Gallery2d && self.ramp_point_at(hit).is_some() =>
            {
                self.begin_ramp_drag(hit);
            }
            Some(hit) if self.ui_actions.action_for_hit(hit).is_some() => {
                let routed = self
                    .ui_actions
                    .target_for_hit(hit)
                    .map(|(target, action)| (target, *action));
                if let Some((target, action)) = routed {
                    // A press inside the open popup must keep the menu alive until the matching
                    // release resolves the click. Closing it here rebuilds the tree without the
                    // item, so `PointerClick::release` no longer finds the pressed target and
                    // silently drops actions such as TearOffTab. Presses elsewhere still dismiss
                    // the menu, and menu-bar entries keep their own exemptions below.
                    let inside_open_menu = self
                        .ui_actions
                        .is_descendant_of(hit, self.scopes.menu.element(1));
                    if self.demo.menu.is_open()
                        && !inside_open_menu
                        && !matches!(
                            action,
                            DemoAction::ToggleMenu
                                | DemoAction::ToggleApplicationMenu
                                | DemoAction::OpenMenu(_)
                                | DemoAction::OpenSubmenu(_)
                                | DemoAction::MenuCommand(_)
                        )
                    {
                        self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
                    }
                    if self.ui_actions.focus_order().contains(&target) {
                        self.set_focus_target(Some(target));
                    } else if !matches!(action, DemoAction::FocusSearch | DemoAction::ClearSearch) {
                        self.set_text_focus(None);
                    }
                    if action.starts_pointer_drag() {
                        if let DemoAction::Workspace(WorkspaceViewAction::Tab(item)) = action {
                            self.workspace_drag = Some(WorkspaceTabDrag {
                                item,
                                origin_container: ContainerRef::Main,
                                origin: self.cursor,
                                started: false,
                                pointer_container: Some(ContainerRef::Main),
                                cursor: self.cursor,
                                target: None,
                            });
                        }
                        self.apply_demo_action(action);
                    } else {
                        self.pointer_click.press(
                            STANDALONE_WINDOW,
                            PointerButton::Primary,
                            &self.ui_actions,
                            Some(hit),
                        );
                    }
                }
            }
            Some(_) => {
                self.set_text_focus(None);
                self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
                self.close_combo_for(self.action_window, PopupCloseReason::FocusTransfer);
            }
            None if self.mode == AcceptanceMode::Viewport3d => {
                self.set_text_focus(None);
                if let Some(window) = &self.window {
                    let size = window.inner_size().to_logical::<f32>(window.scale_factor());
                    if let Some(drag) = TranslationDrag::begin(
                        &self.scene,
                        self.camera,
                        size.width,
                        size.height,
                        self.cursor.x,
                        self.cursor.y,
                    ) {
                        self.transform_drag = Some(drag);
                        self.apply_transform_action(drag, drag.begin_action());
                    } else {
                        let ndc_x = self.cursor.x / size.width.max(1.0) * 2.0 - 1.0;
                        let ndc_y = self.cursor.y / size.height.max(1.0) * 2.0 - 1.0;
                        let selected = pick(
                            &self.scene,
                            self.camera,
                            size.width / size.height.max(1.0),
                            ndc_x,
                            ndc_y,
                        );
                        self.scene.select(selected);
                        self.camera_drag = true;
                        self.last_cursor = self.cursor;
                    }
                }
            }
            None => {
                self.set_text_focus(None);
                self.close_window_menu(self.action_window, PopupCloseReason::FocusTransfer);
                self.close_combo_for(self.action_window, PopupCloseReason::FocusTransfer);
            }
        }
        self.request_redraw();
    }

    fn apply_demo_action(&mut self, action: DemoAction) {
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
            DemoAction::ViewportFrame => apply_action(&mut self.camera, ViewportAction::FrameAll),
            DemoAction::ViewportProjection => {
                apply_action(&mut self.camera, ViewportAction::ToggleProjection)
            }
        }
    }

    fn apply_transform_action(&mut self, drag: TranslationDrag, action: TransformAction) {
        match action {
            TransformAction::Begin { part, .. } => self.scene.gizmo.active = Some(part),
            TransformAction::Update { delta } => {
                if let Some(renderable) = self
                    .scene
                    .renderables
                    .iter_mut()
                    .find(|renderable| renderable.id == drag.target())
                {
                    renderable.translation = drag.original().plus(delta);
                }
            }
            TransformAction::Commit => self.scene.gizmo.active = None,
            TransformAction::Cancel => {
                if let Some(renderable) = self
                    .scene
                    .renderables
                    .iter_mut()
                    .find(|renderable| renderable.id == drag.target())
                {
                    renderable.translation = drag.original();
                }
                self.scene.gizmo.active = None;
            }
        }
    }

    fn cancel_transform_drag(&mut self) {
        if let Some(drag) = self.transform_drag.take() {
            self.apply_transform_action(drag, TransformAction::Cancel);
        }
    }

    fn paint_interaction(&self) -> PaintInteraction {
        self.window_paint_interaction(self.cursor, self.scrollbar_drag.map(|(element, _)| element))
    }

    fn window_paint_interaction(
        &self,
        cursor: Point,
        active_scrollbar: Option<ElementId>,
    ) -> PaintInteraction {
        let moving_dock = self.workspace_drag.is_some_and(|drag| drag.started)
            || self.floating_move.is_some()
            || self.workspace_resize.is_some();
        PaintInteraction {
            pointer: (!moving_dock).then_some(cursor),
            active_scrollbar,
        }
    }

    fn cancel_floating_move(&mut self) {
        if let Some(drag) = self.floating_move.take()
            && let Some(window) = self.demo.workspace.floating_window_mut(drag.window)
        {
            window.placement = drag.placement;
        }
    }

    fn update_floating_move(&mut self) {
        let (Some(drag), Some(workspace)) = (self.floating_move, self.workspace_rect) else {
            return;
        };
        if let Some(window) = self.demo.workspace.floating_window_mut(drag.window) {
            window.placement.x = (drag.placement.x + self.cursor.x - drag.origin.x)
                .clamp(0.0, (workspace.width - drag.placement.width).max(0.0));
            window.placement.y = (drag.placement.y + self.cursor.y - drag.origin.y)
                .clamp(0.0, (workspace.height - drag.placement.height).max(0.0));
            self.request_redraw();
        }
    }

    fn wheel(&mut self, delta: MouseScrollDelta) {
        let mut movement = match delta {
            MouseScrollDelta::LineDelta(x, y) => Point::new(-x * 34.0, -y * 34.0),
            MouseScrollDelta::PixelDelta(position) => {
                let scale = self
                    .window
                    .as_ref()
                    .map_or(1.0, |window| window.scale_factor());
                let point = logical_point(position, scale);
                Point::new(-point.x, -point.y)
            }
        };
        if self.text_bridge.modifiers().shift_key() && movement.x.abs() <= f32::EPSILON {
            movement.x = movement.y;
            movement.y = 0.0;
        }
        if self.mode == AcceptanceMode::Gallery2d {
            if movement.x.is_finite()
                && movement.y.is_finite()
                && (movement.x.abs() > f32::EPSILON || movement.y.abs() > f32::EPSILON)
            {
                retire_caret_reveal(&mut self.caret_reveal, STANDALONE_WINDOW);
            }
            if let Some(view) = self.area_view
                && view.bounds.contains(self.cursor)
                && self.hit_test.hit(self.cursor) == Some(self.scopes.gallery.element(TEXT_AREA))
                && movement.y.abs() > f32::EPSILON
            {
                restore_area_view_layout(&mut self.area_state, &mut self.text_context, view);
                let limit = (self.area_state.snapshot().layout_height()
                    - (view.bounds.height - 4.0).max(0.0))
                .max(0.0);
                let next = (self.area_scroll_y + movement.y).clamp(0.0, limit);
                let consumed = next - self.area_scroll_y;
                if consumed.abs() > f32::EPSILON {
                    self.area_scroll_y = next;
                    self.area_follow_caret = false;
                    movement.y -= consumed;
                }
            }
            if let Some(&(pane, _)) = self
                .scroll_geometry
                .tabs
                .values()
                .find(|(_, rect)| rect.contains(self.cursor))
            {
                let horizontal = if movement.x.abs() > f32::EPSILON {
                    movement.x
                } else {
                    movement.y
                };
                self.demo
                    .tab_scroll
                    .entry(pane)
                    .or_default()
                    .scroll_by_point(Point::new(horizontal, 0.0));
                self.request_redraw();
                return;
            }
            let gallery = self.scopes.gallery;
            let hit = self.hit_test.hit(self.cursor);
            let geometry = self.scroll_geometry.clone();
            let mut states = ScrollStates {
                tree: &mut self.demo.tree_scroll,
                virtual_list: &mut self.demo.virtual_scroll,
                virtual_table: &mut self.demo.virtual_table_scroll,
                nested: &mut self.demo.nested_scroll,
                horizontal: &mut self.demo.horizontal_scroll,
                gallery: Some(&mut self.gallery_scroll),
                pages: &mut self.demo.page_scroll,
                tabs: &mut self.demo.tab_scroll,
            };
            wheel_scroll_path(
                STANDALONE_WINDOW,
                &self.ui_actions,
                &mut states,
                hit,
                &geometry,
                gallery,
                movement,
            );
        } else if self.hit_test.hit(self.cursor).is_none() {
            apply_action(
                &mut self.camera,
                ViewportAction::Dolly {
                    delta: -movement.y * 0.02,
                },
            );
        }
        self.request_redraw();
    }

    fn redraw(&mut self) {
        if self.gpu.as_ref().is_none_or(|gpu| !gpu.available()) {
            return;
        }
        // Initial floating-window activation can blur the main window. Start
        // the scripted popup sequence only after those asynchronous factories
        // have finished; real windows continue drawing during creation.
        if self.smoke
            && self.rendered_frames == 0
            && (std::env::args().any(|a| a == "--smoke-floating") || gpu_lifecycle::enabled())
            && self
                .demo
                .workspace
                .floating_windows()
                .iter()
                .any(|logical| {
                    !self
                        .native_windows
                        .values()
                        .any(|host| host.logical_id == logical.id && host.surface.available())
                })
        {
            return;
        }
        let mut frame_trace = self.frame_profile.begin();
        let input_watermark = self.lifecycle_profile.begin_frame();
        self.reconcile_workspace_gestures();
        self.reconcile_window_menu(STANDALONE_WINDOW);
        self.prepare_suggestions(STANDALONE_WINDOW);
        let focus_root = self.focus_root_for_window(STANDALONE_WINDOW);
        let focused_field = self.focused_text_field();
        let virtual_focused = self.focused_virtual_index();
        let virtual_table_focused = self.focused_virtual_table_index();
        let tree_focused = self.focused_tree_index();
        let dock_drag = self.dock_drag_visual(ContainerRef::Main);
        let paint_interaction = self.paint_interaction();
        let isolated_accessibility_root = self
            .demo
            .dialog_stack
            .top()
            .map(|id| dialog_element(self.scopes.root, id, 0));
        let tooltip_focus_root = self.focus_root_for_window(STANDALONE_WINDOW);
        let password_snapshot = self.password_display_snapshot();
        let table_note_snapshot = self.table_note_state.snapshot();
        let Some(window) = self.window.clone() else {
            return;
        };
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        let scale = window.scale_factor() as f32;
        let logical = size.to_logical::<f32>(window.scale_factor());
        let viewport = Viewport::new(logical.width, logical.height, scale);
        self.area_layout
            .prepare(&mut self.area_state, &mut self.text_context);
        let mut built = build_root(
            UiBuildState {
                documents: &self.documents,
                selected_feedback: native_capture::selected_feedback_enabled(self),
                ui_window: STANDALONE_WINDOW,
                menu_open: self.demo.menu.is_open(),
                scopes: self.scopes,
                workspace_container: ContainerRef::Main,
                mode: self.mode,
                maximized: window.is_maximized(),
                theme: self.theme,
                scroll_offset: self.gallery_scroll.offset(),
                viewport_size: Size::new(logical.width, logical.height),
                snapshot: self.text_state.snapshot(),
                focused: focused_field == Some(TextFieldKind::Main),
                search_snapshot: self.search_state.snapshot(),
                search_query: self.search_state.text(),
                search_focused: focused_field == Some(TextFieldKind::Search),
                area_snapshot: self.area_state.snapshot().with_viewport_scroll(
                    Point::new(0.0, self.area_scroll_y),
                    self.area_follow_caret,
                ),
                area_focused: focused_field == Some(TextFieldKind::Area),
                number_snapshot: self.number_state.snapshot(),
                number_focused: focused_field == Some(TextFieldKind::Number),
                vector_snapshots: std::array::from_fn(|axis| self.vector_states[axis].snapshot()),
                vector_focused: std::array::from_fn(|axis| {
                    focused_field == Some(TextFieldKind::Vector(axis as u8))
                }),
                color_hex_snapshot: self.color_hex_state.snapshot(),
                password_mask: self.password_state.masked_value(),
                password_snapshot,
                table_note_snapshot,
                color_hex_focused: focused_field == Some(TextFieldKind::ColorHex),
                table_note_focused: focused_field == Some(TextFieldKind::TableNote),
                readonly_label: &self.readonly_label_state,
                readonly_label_focused: focused_field == Some(TextFieldKind::Label),
                virtual_focused,
                virtual_table_focused,
                virtual_table_viewport_height: window_collections::table_viewport_height(
                    &self.scroll_geometry,
                    self.scopes.gallery,
                ),
                tree_focused,
                demo: &self.demo,
                hovered: self.hovered,
                focused_element: self.input_ownership.focus().map(|focus| focus.element),
                dock_drag,
                popover_anchor: self
                    .focus_rects
                    .get(&self.scopes.gallery.element(POPOVER_TRIGGER))
                    .copied(),
                combo_anchor: self
                    .focus_rects
                    .get(&self.scopes.gallery.element(COMBO_TRIGGER))
                    .copied(),
                gizmo_mode: self.scene.gizmo.mode,
                frame_stats: self.frame_meter.display,
                overlay_owner: self.overlay_owner,
            },
            &mut self.text_context,
        );
        self.tile_columns = built.tile_columns;
        if self.smoke && self.mode == AcceptanceMode::Gallery2d {
            demo_images::append_smoke_image(&mut built.root, self.scopes.gallery);
        }
        frame_trace.mark(FrameStage::Build);
        let Ok(actions) = ActionRoute::build(&built.root, built.actions.iter().copied()) else {
            eprintln!("failed to route UI actions: duplicate or unknown element identity");
            return;
        };
        frame_trace.mark(FrameStage::Route);
        let window_active = window.has_focus();
        let base_children = built.root.children.len();
        let Ok(layout) = self.layout_main_tooltips(
            &mut built.root,
            &actions,
            viewport,
            window_active,
            tooltip_focus_root,
        ) else {
            return;
        };
        let Ok(actions) = window_tooltips::route_after_tooltip(
            &built.root,
            base_children,
            actions,
            built.actions,
        ) else {
            eprintln!("failed to route final tooltip tree: invalid UI identity");
            return;
        };
        frame_trace.mark(FrameStage::Layout);
        match self.documents.sync_layout(
            self.scopes.markdown,
            &layout,
            &mut self.text_context,
            self.theme,
        ) {
            Ok(true) => {
                self.request_redraw();
                return;
            }
            Ok(false) => {}
            Err(error) => {
                self.fatal_error = Some(error.to_string());
                return;
            }
        }
        if self.documents.sync_input(
            STANDALONE_WINDOW,
            self.scopes.markdown,
            &layout,
            &mut self.input_ownership,
        ) {
            self.request_redraw();
            return;
        }
        let popup_scope = FocusScope::new(STANDALONE_WINDOW, &actions, None);
        let combo_closed = self
            .demo
            .combo
            .reconcile(STANDALONE_WINDOW, &popup_scope)
            .is_some();
        let popover_closed = self
            .demo
            .popover
            .reconcile(STANDALONE_WINDOW, &popup_scope)
            .is_some();
        if combo_closed || popover_closed {
            self.request_redraw();
            return;
        }
        let scope = FocusScope::new(STANDALONE_WINDOW, &actions, focus_root);
        let focus_native = DockFocusTransfer::request(
            &mut self.dock_focus,
            &self.demo.workspace,
            ContainerRef::Main,
            STANDALONE_WINDOW,
            &actions,
            focus_root,
            &mut self.input_ownership,
        );
        if let Some(restored) = scope.resolve_request(&mut self.input_ownership) {
            if focus_native && restored.is_some() {
                window.focus_window();
            }
            if let Some(target) = restored {
                self.reveal_current_layout_focus(
                    STANDALONE_WINDOW,
                    self.scopes.gallery,
                    &layout,
                    &actions,
                    &built.workspace_regions,
                    target.element,
                );
            }
            self.ui_actions = actions;
            self.set_focus_target(restored.map(|target| target.element));
            self.request_redraw();
            return;
        }
        let reconciled = scope.reconcile(&mut self.input_ownership);
        if reconciled.lost_focus || reconciled.lost_capture {
            self.ui_actions = actions;
            self.pointer_click.cancel();
            if reconciled.lost_focus {
                self.cancel_window_editors_for(STANDALONE_WINDOW, PopupCloseReason::OwnerRemoved);
            } else {
                self.cancel_window_workspace_drag(STANDALONE_WINDOW);
                self.cancel_window_value_edits(STANDALONE_WINDOW);
            }
            self.request_redraw();
            return;
        }
        self.ui_actions = actions;
        if self.smoke
            && !window_documents::smoke_enabled()
            && self.mode == AcceptanceMode::Gallery2d
            && self.rendered_frames == 1
            && self
                .ui_actions
                .action_at(self.scopes.popover.element(POPOVER_BUTTON))
                != Some(&DemoAction::PopoverStep)
        {
            self.fatal_error = Some("smoke popover child action was not projected".into());
        }
        if self.smoke
            && !window_documents::smoke_enabled()
            && self.mode == AcceptanceMode::Gallery2d
            && self.rendered_frames == 2
            && self
                .ui_actions
                .action_at(self.scopes.gallery.element(COMBO_OPTION_BASE))
                != Some(&DemoAction::SelectCombo(0))
        {
            self.fatal_error = Some("smoke combo options were not projected".into());
        }
        let scroll_id = self.scopes.gallery.element(3);
        self.gallery_scroll_rect = layout
            .node(scroll_id)
            .and_then(|node| node.rect.intersection(viewport.logical_rect()));
        self.page_scroll_rects.clear();
        let mut page_changed = false;
        let active_pages = self
            .demo
            .workspace
            .panes()
            .into_iter()
            .map(|pane| pane.id)
            .chain(
                self.demo
                    .workspace
                    .floating_windows()
                    .iter()
                    .flat_map(|window| window.root.panes()),
            )
            .filter_map(|pane| {
                self.demo
                    .workspace
                    .pane(pane)?
                    .active_item()
                    .map(|item| item.id)
            })
            .collect::<Vec<_>>();
        // One capture feeds both the legacy per-field rects and the shared geometry the
        // scrollbar and wheel paths use, so the two windows agree on viewports.
        let previous_table_height =
            window_collections::table_viewport_height(&self.scroll_geometry, self.scopes.gallery);
        self.scroll_geometry =
            ScrollGeometry::from_layout(self.scopes.gallery, &layout, active_pages.iter().copied())
                .with_tabs(&layout, &built.workspace_regions);
        let table_viewport_changed = previous_table_height
            != window_collections::table_viewport_height(
                &self.scroll_geometry,
                self.scopes.gallery,
            );
        page_changed |= (ScrollStates {
            tree: &mut self.demo.tree_scroll,
            virtual_list: &mut self.demo.virtual_scroll,
            virtual_table: &mut self.demo.virtual_table_scroll,
            nested: &mut self.demo.nested_scroll,
            horizontal: &mut self.demo.horizontal_scroll,
            gallery: Some(&mut self.gallery_scroll),
            pages: &mut self.demo.page_scroll,
            tabs: &mut self.demo.tab_scroll,
        })
        .sync_layout(self.scopes.gallery, &self.scroll_geometry, &layout);
        for item in active_pages {
            if let Some(node) = layout.node(page_scroll_id(self.scopes.gallery, item)) {
                let rect = node
                    .rect
                    .intersection(node.inherited_clip)
                    .unwrap_or(node.rect);
                self.page_scroll_rects.insert(item, rect);
                page_changed |= self
                    .demo
                    .page_scroll
                    .entry(item)
                    .or_default()
                    .sync_layout(node);
            }
        }
        let nested_id = self.scopes.gallery.element(NESTED_SCROLL);
        self.nested_scroll_rect = layout.node(nested_id).and_then(|node| {
            node.rect
                .intersection(node.inherited_clip)
                .or(Some(node.rect))
        });
        let horizontal_id = self.scopes.gallery.element(HORIZONTAL_SCROLL);
        self.horizontal_scroll_rect = layout.node(horizontal_id).and_then(|node| {
            node.rect
                .intersection(node.inherited_clip)
                .or(Some(node.rect))
        });
        let virtual_id = self.scopes.gallery.element(VIRTUAL_SCROLL);
        self.virtual_scroll_rect = layout.node(virtual_id).and_then(|node| {
            node.rect
                .intersection(node.inherited_clip)
                .or(Some(node.rect))
        });
        let virtual_table_id = self.scopes.gallery.element(TABLE_VIRTUAL_ROOT + 2);
        self.virtual_table_scroll_rect = layout.node(virtual_table_id).and_then(|node| {
            node.rect
                .intersection(node.inherited_clip)
                .or(Some(node.rect))
        });
        let tree_id = self.scopes.gallery.element(TREE_SCROLL);
        self.tree_scroll_rect = layout.node(tree_id).and_then(|node| {
            node.rect
                .intersection(node.inherited_clip)
                .or(Some(node.rect))
        });
        let nested_changed = layout
            .node(nested_id)
            .is_some_and(|node| self.demo.nested_scroll.sync_layout(node));
        let horizontal_changed = layout
            .node(horizontal_id)
            .is_some_and(|node| self.demo.horizontal_scroll.sync_layout(node));
        let virtual_changed = layout
            .node(virtual_id)
            .is_some_and(|node| self.demo.virtual_scroll.sync_layout(node));
        let virtual_table_changed = layout
            .node(virtual_table_id)
            .is_some_and(|node| self.demo.virtual_table_scroll.sync_layout(node));
        let tree_changed = layout
            .node(tree_id)
            .is_some_and(|node| self.demo.tree_scroll.sync_layout(node));
        let gallery_changed = layout
            .node(scroll_id)
            .is_some_and(|node| self.gallery_scroll.sync_layout(node));
        self.tab_scroll_rects.clear();
        let mut tab_changed = false;
        let pane_width_changed =
            self.demo
                .sync_pane_widths(built.workspace_regions.panes.iter().filter_map(|region| {
                    layout
                        .node(region.root)
                        .map(|root| (region.pane, root.rect.width))
                }));
        for region in &built.workspace_regions.panes {
            if let Some(list) = layout.node(region.tab_list) {
                self.tab_scroll_rects
                    .insert(region.pane, list.scroll_viewport());
                let scroll = self.demo.tab_scroll.entry(region.pane).or_default();
                tab_changed |= scroll.sync_layout(list);
            }
        }
        tab_changed |= reveal_active_tabs(
            &built.workspace_regions,
            &layout,
            &self.demo.workspace,
            &mut self.demo.tab_scroll,
            &mut self.tab_reveal,
        );
        let layout_changed = nested_changed
            || page_changed
            || horizontal_changed
            || virtual_changed
            || virtual_table_changed
            || table_viewport_changed
            || tree_changed
            || gallery_changed
            || tab_changed
            || pane_width_changed;
        if layout_changed {
            window.request_redraw();
        }
        let wrap_changed = self.sync_area_wrap_layout(self.scopes.gallery, &layout);
        if wrap_changed {
            window.request_redraw();
        }
        let old_combo_anchor = self
            .focus_rects
            .get(&self.scopes.gallery.element(COMBO_TRIGGER))
            .copied();
        let old_popover_anchor = self
            .focus_rects
            .get(&self.scopes.gallery.element(POPOVER_TRIGGER))
            .copied();
        self.focus_rects = self
            .ui_actions
            .focus_order()
            .iter()
            .copied()
            .filter(|id| self.ui_actions.is_descendant_of(*id, scroll_id))
            .filter_map(|id| layout.node(id).map(|node| (id, node.rect)))
            .collect();
        if self.demo.popover.is_open()
            && self
                .focus_rects
                .get(&self.scopes.gallery.element(POPOVER_TRIGGER))
                .copied()
                != old_popover_anchor
        {
            window.request_redraw();
        }
        if self
            .demo
            .combo
            .opener()
            .is_some_and(|opener| opener.window == STANDALONE_WINDOW)
            && self
                .focus_rects
                .get(&self.scopes.gallery.element(COMBO_TRIGGER))
                .copied()
                != old_combo_anchor
        {
            window.request_redraw();
        }
        self.slider_rect = built
            .slider
            .and_then(|id| layout.node(id).map(|node| node.rect));
        self.sync_palette_layout(STANDALONE_WINDOW, self.scopes.root, &layout);
        self.sync_suggestion_layout(STANDALONE_WINDOW, self.scopes.gallery, &layout);
        self.ramp_track_rect = built
            .ramp_track
            .and_then(|id| layout.node(id).map(|node| node.rect));
        self.table_rect = layout
            .node(self.scopes.gallery.element(TABLE_ROOT))
            .map(|node| node.rect);
        self.workspace_rect = layout
            .node(self.scopes.workspace.element(1))
            .map(|node| node.rect);
        self.dock_frame =
            DockFrame::from_layout(&built.workspace_regions, &layout, self.workspace_rect);
        let constrained_splits = self
            .dock_frame
            .splits
            .iter()
            .filter_map(|frame| {
                let current = dock_split_ratio(&self.demo.workspace, frame.container, frame.split)?;
                let pointer = Point::new(
                    frame.rect.x + frame.rect.width * current,
                    frame.rect.y + frame.rect.height * current,
                );
                let constrained =
                    split_ratio_at(frame.rect, frame.axis, pointer, MIN_DOCK_PANE_EXTENT);
                ((constrained - current).abs() > 0.001).then_some((
                    frame.container,
                    frame.split,
                    constrained,
                ))
            })
            .collect::<Vec<_>>();
        if !constrained_splits.is_empty() {
            for (container, split, ratio) in constrained_splits {
                let _ = self.demo.workspace.set_split_ratio(container, split, ratio);
            }
            window.request_redraw();
        }
        let paint_focus = self
            .input_ownership
            .visible_focus()
            .map(|focus| focus.element);
        frame_trace.mark(FrameStage::Reconcile);
        if wrap_changed {
            return;
        }
        let paint = PaintList::build_with_focus_and_interaction(
            &built.root,
            &layout,
            paint_focus,
            self.theme.palette.focus,
            paint_interaction,
        );
        if complete_caret_reveal(
            &mut self.caret_reveal,
            self.text_session,
            CaretRevealFrame {
                window: STANDALONE_WINDOW,
                gallery: self.scopes.gallery,
                actions: &self.ui_actions,
                focus: self.input_ownership.focus(),
                focus_root,
                geometry: &self.scroll_geometry,
                paint: &paint,
                layout_stable: !(layout_changed || wrap_changed),
            },
            &mut ScrollStates {
                tree: &mut self.demo.tree_scroll,
                virtual_list: &mut self.demo.virtual_scroll,
                virtual_table: &mut self.demo.virtual_table_scroll,
                nested: &mut self.demo.nested_scroll,
                horizontal: &mut self.demo.horizontal_scroll,
                gallery: Some(&mut self.gallery_scroll),
                pages: &mut self.demo.page_scroll,
                tabs: &mut self.demo.tab_scroll,
            },
        ) {
            window.request_redraw();
            return;
        }
        frame_trace.mark(FrameStage::Paint);
        self.hit_test = HitTestTree::build(&built.root, &layout);
        let hovered = self.hit_test.hit(self.cursor);
        if self.hovered != hovered {
            self.hovered = hovered;
            self.tooltip
                .hover(hovered, &self.ui_actions, Instant::now());
            window.request_redraw();
        }
        frame_trace.mark(FrameStage::Hit);
        self.caret_blink.update(
            focused_field
                .filter(|_| window_active)
                .map(|field| field.element(self.scopes.gallery)),
            Instant::now(),
        );
        if self.capture_geometry_changed(
            STANDALONE_WINDOW,
            self.scopes.gallery,
            &built.root,
            &layout,
            &self.ui_actions,
        ) {
            if let Some(gpu) = &mut self.gpu
                && let Ok(renderer) = gpu.capture_renderer()
            {
                renderer.cancel_capture();
            }
            self.ui_query.cancel_capture_binding(STANDALONE_WINDOW);
        }
        let Some(gpu) = self.gpu.as_mut() else {
            return;
        };
        let profile_context = FrameContext {
            window: STANDALONE_WINDOW,
            gallery: self.mode == AcceptanceMode::Gallery2d,
            text_focused: focused_field.is_some(),
            active: window_active,
        };
        let output = match gpu.render(
            None,
            RenderFrame {
                viewport,
                paint: &paint,
                context: &mut self.text_context,
                rasterizer: &mut self.text_rasterizer,
                images: Some(&DemoImages::for_frame(self.rendered_frames)),
                options: RenderOptions {
                    clear: (self.mode == AcceptanceMode::Gallery2d)
                        .then(|| theme_clear(self.theme, false)),
                    selection: self.theme.palette.selection,
                    caret: self.theme.palette.focus,
                    caret_visible: self.caret_blink.visible(),
                },
                scene: (self.mode == AcceptanceMode::Viewport3d).then_some((
                    self.camera,
                    &self.scene,
                    theme_clear(self.theme, true),
                )),
                finish: self.smoke
                    && !render_backend::retirement_probe_enabled()
                    && self.rendered_frames + 1
                        >= render_backend::smoke_frame_limit(
                            self.mode == AcceptanceMode::Gallery2d,
                        ),
                cpu_profile: frame_trace.renderer_profile(),
                gpu_profile: &mut self.gpu_profile,
                profile_context,
            },
        ) {
            Ok(Some(output)) => output,
            Ok(None) => {
                if self.display_blocked(window.id()) {
                    self.cancel_gpu_interactions(&std::collections::HashSet::from([window.id()]));
                }
                return;
            }
            Err(error) => {
                self.gpu_fault(window.id(), error);
                return;
            }
        };
        frame_trace.mark(FrameStage::Renderer);
        self.lifecycle_profile
            .presented(STANDALONE_WINDOW, input_watermark);
        if let Err(error) =
            demo_images::verify_frame(self.rendered_frames, &paint, viewport, &output)
        {
            self.fatal_error = Some(error);
            return;
        }
        if self.smoke
            && self.mode == AcceptanceMode::Gallery2d
            && self.rendered_frames == 2
            && !demo_images::faults_enabled()
            && output.bitmap_reuses == 0
        {
            self.fatal_error = Some("settled UI frame did not reuse any bitmap texture".into());
        }
        if self.smoke && self.mode == AcceptanceMode::Gallery2d {
            if self.rendered_frames == 0 && output.glyph_uploads == 0 && output.glyph_reuses == 0 {
                self.fatal_error =
                    Some("first UI frame has neither uploaded nor reused atlas glyphs".into());
            }
            if self.rendered_frames == 2 && output.glyph_reuses == 0 {
                self.fatal_error = Some("settled UI frame did not reuse atlas glyphs".into());
            }
        }
        let text_views =
            match text_views_for(STANDALONE_WINDOW, self.scopes.gallery, &output.text_inputs) {
                Ok(views) => views,
                Err(error) => {
                    self.fatal_error = Some(format!("text_input_views: {error:?}"));
                    return;
                }
            };
        self.text_view = text_views.get(TextFieldKind::Main);
        self.search_view = text_views.get(TextFieldKind::Search);
        self.area_view = text_views.get(TextFieldKind::Area);
        if let Some(view) = self.area_view {
            self.area_scroll_y = view.scroll_y;
        }
        self.number_view = text_views.get(TextFieldKind::Number);
        self.vector_views =
            std::array::from_fn(|axis| text_views.get(TextFieldKind::Vector(axis as u8)));
        self.color_hex_view = text_views.get(TextFieldKind::ColorHex);
        self.password_view = text_views.get(TextFieldKind::Password);
        self.table_note_view = text_views.get(TextFieldKind::TableNote);
        self.readonly_label_view = text_views.get(TextFieldKind::Label);
        frame_trace.mark(FrameStage::TextViews);
        if let Some(adapter) = &mut self.accessibility {
            let accessibility_root = accessibility_root(&built.root, isolated_accessibility_root);
            let mut update = self.semantics.build(
                accessibility_root,
                &layout,
                self.input_ownership.focus().map(|focus| focus.element),
                self.demo.locale.window_title(),
            );
            enrich_window_text(
                &mut self.semantics,
                self.scopes.gallery,
                &text_views,
                TextFieldStates {
                    main: &mut self.text_state,
                    search: &mut self.search_state,
                    area: &mut self.area_state,
                    number: &mut self.number_state,
                    vector: &mut self.vector_states,
                    color: &mut self.color_hex_state,
                    password: &mut self.password_state,
                    table_note: &mut self.table_note_state,
                    readonly_label: &mut self.readonly_label_state,
                },
                &mut self.text_context,
                &mut update,
            );
            if let Err(error) = self
                .documents
                .enrich_accessibility(&mut self.semantics, &mut update)
            {
                eprintln!("document accessibility: {error}");
            }
            adapter.update_if_active(|| {
                self.frame_profile.observe_native(|| {
                    let owner = self.input_ownership.focus().map(|target| target.element);
                    NativeObservation::Projection(NativeProjection::from_update(
                        STANDALONE_WINDOW,
                        window.has_focus(),
                        window.scale_factor(),
                        owner,
                        owner.and_then(|element| self.semantics.node_for_element(element)),
                        &update,
                    ))
                });
                update
            });
        }
        frame_trace.mark(FrameStage::Accessibility);
        if focused_field.is_some() {
            self.text_bridge.sync_ime(
                &window,
                focused_field != Some(TextFieldKind::Label),
                match focused_field {
                    Some(TextFieldKind::Main) => self.text_view,
                    Some(TextFieldKind::Search) => self.search_view,
                    Some(TextFieldKind::Area) => self.area_view,
                    Some(TextFieldKind::Number) => self.number_view,
                    Some(TextFieldKind::Vector(axis)) => self.vector_views[usize::from(axis)],
                    Some(TextFieldKind::ColorHex) => self.color_hex_view,
                    Some(TextFieldKind::Password) => self.password_view,
                    Some(TextFieldKind::TableNote) => self.table_note_view,
                    Some(TextFieldKind::Label) => self.readonly_label_view,
                    None => None,
                },
            );
        }
        frame_trace.mark(FrameStage::Ime);
        self.frame_profile.submitted(frame_trace, profile_context);
        self.frame_meter.submitted(Instant::now());
        self.busy_visible = built.busy_roots.is_some_and(|ids| {
            ids.iter()
                .any(|id| window_idle::visible_geometry(&layout, *id))
        });
        self.gpu_recovery.presented(window.id());
        self.rendered_frames += 1;
        if let Some(snapshot) = self.capture_query_snapshot(
            STANDALONE_WINDOW,
            self.scopes.gallery,
            &built.root,
            &layout,
            &self.ui_actions,
            self.input_ownership.focus(),
            focus_root,
        ) {
            self.ui_query.publish_root(
                snapshot,
                built.root,
                &self.ui_actions,
                &layout,
                self.ui_input_context(STANDALONE_WINDOW, &window),
            );
        }
    }
}

fn chrome_resize_direction(cursor: Point, size: Size) -> Option<ResizeDirection> {
    if cursor.x < 0.0 || cursor.y < 0.0 || cursor.x > size.width || cursor.y > size.height {
        return None;
    }
    let west = cursor.x <= RESIZE_EDGE;
    let east = cursor.x >= size.width - RESIZE_EDGE;
    let north = cursor.y <= RESIZE_EDGE;
    let south = cursor.y >= size.height - RESIZE_EDGE;
    match (west, east, north, south) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (_, true, true, _) => Some(ResizeDirection::NorthEast),
        (true, _, _, true) => Some(ResizeDirection::SouthWest),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        (true, _, _, _) => Some(ResizeDirection::West),
        (_, true, _, _) => Some(ResizeDirection::East),
        (_, _, true, _) => Some(ResizeDirection::North),
        (_, _, _, true) => Some(ResizeDirection::South),
        _ => None,
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    application_run::run()
}

fn theme_clear(theme: WidgetTheme, viewport: bool) -> [f32; 4] {
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

struct BuiltUi {
    root: Element,
    actions: Vec<(ElementId, DemoAction)>,
    busy_roots: Option<[ElementId; 2]>,
    slider: Option<ElementId>,
    ramp_track: Option<ElementId>,
    workspace_regions: WorkspaceViewRegions,
    tile_columns: usize,
}

struct GalleryUi {
    element: Element,
    actions: Vec<(ElementId, DemoAction)>,
    busy_roots: [ElementId; 2],
    slider: ElementId,
    ramp_track: ElementId,
    workspace_regions: WorkspaceViewRegions,
    tile_columns: usize,
}

struct ViewportUi {
    element: Element,
    actions: Vec<(ElementId, DemoAction)>,
}

struct UiBuildState<'a> {
    documents: &'a GalleryDocuments,
    selected_feedback: bool,
    ui_window: UiWindowId,
    menu_open: bool,
    scopes: UiScopes,
    workspace_container: ContainerRef,
    mode: AcceptanceMode,
    maximized: bool,
    theme: WidgetTheme,
    scroll_offset: Point,
    viewport_size: Size,
    snapshot: vesta_gpui::TextInputSnapshot,
    focused: bool,
    search_snapshot: vesta_gpui::TextInputSnapshot,
    search_query: &'a str,
    search_focused: bool,
    area_snapshot: vesta_gpui::TextInputSnapshot,
    area_focused: bool,
    number_snapshot: vesta_gpui::TextInputSnapshot,
    number_focused: bool,
    vector_snapshots: [vesta_gpui::TextInputSnapshot; 3],
    vector_focused: [bool; 3],
    color_hex_snapshot: vesta_gpui::TextInputSnapshot,
    password_mask: String,
    password_snapshot: vesta_gpui::TextInputSnapshot,
    table_note_snapshot: vesta_gpui::TextInputSnapshot,
    color_hex_focused: bool,
    table_note_focused: bool,
    readonly_label: &'a TextInputState,
    readonly_label_focused: bool,
    virtual_focused: Option<usize>,
    virtual_table_focused: Option<usize>,
    virtual_table_viewport_height: f32,
    tree_focused: Option<usize>,
    demo: &'a DemoState,
    hovered: Option<ElementId>,
    focused_element: Option<ElementId>,
    dock_drag: Option<DockDragVisual>,
    popover_anchor: Option<Rect>,
    combo_anchor: Option<Rect>,
    gizmo_mode: TransformMode,
    frame_stats: Option<(f32, f32)>,
    /// The window that owns the palette or modal dialog, if any (ADR-0254).
    overlay_owner: Option<UiWindowId>,
}

fn build_root(input: UiBuildState<'_>, text_context: &mut TextInputContext) -> BuiltUi {
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
fn build_owned_overlays(
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

fn build_status(scope: ElementScope, input: &UiBuildState<'_>) -> Widget<DemoAction> {
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

fn build_combo_popup(
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

fn build_popover(
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

fn widget_element(
    widget: Widget<DemoAction>,
    actions: &mut Vec<(ElementId, DemoAction)>,
) -> Element {
    actions.extend(widget.actions);
    widget.element
}

fn demo_workspace() -> WorkspaceModel {
    let left = PaneId(1);
    let right = PaneId(2);
    let root = WorkspaceNode::split(
        SplitId(1),
        SplitAxis::Row,
        0.5,
        WorkspaceNode::leaf(left),
        WorkspaceNode::leaf(right),
    );
    let mut model = WorkspaceModel::with_tree(root, [Pane::new(left), Pane::new(right)])
        .expect("the static standalone workspace is valid");
    let controls = model
        .add_item(left, "Controls", PanelTypeId::new("gallery.controls"))
        .expect("the static left pane exists");
    model
        .add_item(left, "Inputs", PanelTypeId::new("gallery.inputs"))
        .expect("the static left pane exists");
    let collections = model
        .add_item(
            right,
            "Collections",
            PanelTypeId::new("gallery.collections"),
        )
        .expect("the static right pane exists");
    model
        .add_item(right, "Layout", PanelTypeId::new("gallery.layout"))
        .expect("the static right pane exists");
    ensure_documents_tab(&mut model);
    model
        .activate(collections)
        .expect("static gallery tab exists");
    model.activate(controls).expect("static gallery tab exists");
    model
}

fn ensure_documents_tab(model: &mut WorkspaceModel) -> Option<ItemId> {
    if let Some(item) = model
        .root()
        .panes()
        .into_iter()
        .chain(
            model
                .floating_windows()
                .iter()
                .flat_map(|window| window.root.panes()),
        )
        .filter_map(|pane| model.pane(pane))
        .flat_map(|pane| pane.items.iter())
        .find(|item| item.panel_type.as_str() == "gallery.documents")
    {
        return Some(item.id);
    }
    let pane = model.panes().first()?.id;
    model
        .add_item(pane, "Documents", PanelTypeId::new("gallery.documents"))
        .ok()
}

fn page_scroll_id(scope: ElementScope, item: ItemId) -> ElementId {
    scope.element(
        GALLERY_PAGE_SCROLL_BASE + u32::try_from(item.0).expect("bounded workspace item ID"),
    )
}

fn build_viewport_overlays(
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
        ("Frame", DemoAction::ViewportFrame, false),
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

fn section(ids: &mut Ids, heading: &str, controls: Vec<Element>, theme: WidgetTheme) -> Element {
    section_with_axis(ids, heading, controls, theme, Axis::Column)
}

fn section_with_axis(
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

fn control(ids: &mut Ids, label_text: &str, background: Color, theme: WidgetTheme) -> Element {
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

fn label(ids: &mut Ids, value: &str, color: Color, theme: WidgetTheme) -> Element {
    text(
        ids.id(),
        ids.key("label"),
        value,
        theme.typography.font_size,
        color,
    )
}

fn text(id: ElementId, key: ElementKey, value: &str, size: f32, color: Color) -> Element {
    Element::text(id, key, value).styled(
        Style::default()
            .size(Dimension::Auto, Dimension::Px((size * 1.5).max(18.0)))
            .text(size, color)
            .text_align(TextHAlign::Start, TextVAlign::Center)
            .pointer_events(false),
    )
}

struct Ids {
    scope: ElementScope,
    next: u32,
}

impl Ids {
    const fn new(scope: ElementScope, first: u32) -> Self {
        Self { scope, next: first }
    }

    fn id(&mut self) -> ElementId {
        let id = self.scope.element(self.next);
        self.next += 1;
        id
    }

    fn identity(&mut self, prefix: &str) -> WidgetIdentity {
        let local = self.next;
        self.next += 1;
        let identity = self.scope.identity(local, format!("{prefix}.{local}"));
        WidgetIdentity {
            element: identity.id,
            key: identity.key,
        }
    }

    fn key(&self, prefix: &str) -> ElementKey {
        let local = self.next.saturating_sub(1);
        self.scope.identity(local, format!("{prefix}.{local}")).key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn semantic_id(element: &Element, role: SemanticRole) -> Option<ElementId> {
        if element
            .semantic
            .as_ref()
            .is_some_and(|semantic| semantic.role == role)
        {
            return Some(element.id);
        }
        element
            .children
            .iter()
            .find_map(|child| semantic_id(child, role))
    }

    /// Mirrors a window build: derives the shared scroll geometry the wheel and scrollbar
    /// paths read (ADR-0253 decision 7) for the given active pages.
    fn capture_scroll_geometry(app: &mut App, layout: &LayoutTree, pages: &[ItemId]) {
        app.scroll_geometry =
            ScrollGeometry::from_layout(app.scopes.gallery, layout, pages.iter().copied());
        for item in pages {
            if let Some(node) = layout.node(page_scroll_id(app.scopes.gallery, *item)) {
                app.page_scroll_rects.insert(*item, node.rect);
            }
        }
    }

    pub(super) fn install_action_route(app: &mut App) -> Element {
        install_action_route_with_viewport(app, Size::new(1280.0, 800.0))
    }

    pub(super) fn activate_gallery_page(app: &mut App, panel_type: &str) -> ItemId {
        let item = app
            .demo
            .workspace
            .panes()
            .into_iter()
            .flat_map(|pane| pane.items.iter())
            .find(|item| item.panel_type.as_str() == panel_type)
            .map(|item| item.id)
            .expect("gallery page exists");
        app.demo
            .workspace
            .activate(item)
            .expect("gallery item is live");
        item
    }

    pub(super) fn install_action_route_with_viewport(
        app: &mut App,
        viewport_size: Size,
    ) -> Element {
        install_action_route_with_regions(app, viewport_size).0
    }

    fn install_action_route_with_regions(
        app: &mut App,
        viewport_size: Size,
    ) -> (Element, WorkspaceViewRegions) {
        install_action_route_for_container(app, viewport_size, ContainerRef::Main)
    }

    fn install_action_route_for_container(
        app: &mut App,
        viewport_size: Size,
        container: ContainerRef,
    ) -> (Element, WorkspaceViewRegions) {
        app.area_layout
            .prepare(&mut app.area_state, &mut app.text_context);
        let password_snapshot = app.password_display_snapshot();
        let table_note_snapshot = app.table_note_state.snapshot();
        let built = build_root(
            UiBuildState {
                documents: &app.documents,
                selected_feedback: native_capture::selected_feedback_enabled(app),
                ui_window: STANDALONE_WINDOW,
                menu_open: app.demo.menu.is_open(),
                scopes: app.scopes,
                workspace_container: container,
                mode: app.mode,
                maximized: false,
                theme: app.theme,
                scroll_offset: app.gallery_scroll.offset(),
                viewport_size,
                snapshot: app.text_state.snapshot(),
                focused: app.focused_text_field() == Some(TextFieldKind::Main),
                search_snapshot: app.search_state.snapshot(),
                search_query: app.search_state.text(),
                search_focused: app.focused_text_field() == Some(TextFieldKind::Search),
                area_snapshot: app.area_state.snapshot().with_viewport_scroll(
                    Point::new(0.0, app.area_scroll_y),
                    app.area_follow_caret,
                ),
                area_focused: app.focused_text_field() == Some(TextFieldKind::Area),
                number_snapshot: app.number_state.snapshot(),
                number_focused: app.focused_text_field() == Some(TextFieldKind::Number),
                vector_snapshots: std::array::from_fn(|axis| app.vector_states[axis].snapshot()),
                vector_focused: std::array::from_fn(|axis| {
                    app.focused_text_field() == Some(TextFieldKind::Vector(axis as u8))
                }),
                color_hex_snapshot: app.color_hex_state.snapshot(),
                password_mask: app.password_state.masked_value(),
                password_snapshot,
                table_note_snapshot,
                color_hex_focused: app.focused_text_field() == Some(TextFieldKind::ColorHex),
                table_note_focused: app.focused_text_field() == Some(TextFieldKind::TableNote),
                readonly_label: &app.readonly_label_state,
                readonly_label_focused: app.focused_text_field() == Some(TextFieldKind::Label),
                virtual_focused: app.focused_virtual_index(),
                virtual_table_focused: app.focused_virtual_table_index(),
                virtual_table_viewport_height: window_collections::table_viewport_height(
                    &app.scroll_geometry,
                    app.scopes.gallery,
                ),
                tree_focused: app.focused_tree_index(),
                demo: &app.demo,
                hovered: app.hovered,
                focused_element: app.input_ownership.focus().map(|focus| focus.element),
                dock_drag: app.dock_drag_visual(container),
                popover_anchor: None,
                combo_anchor: None,
                gizmo_mode: app.scene.gizmo.mode,
                frame_stats: app.frame_meter.display,
                overlay_owner: app.overlay_owner,
            },
            &mut app.text_context,
        );
        app.tile_columns = built.tile_columns;
        app.ui_actions = ActionRoute::build(&built.root, built.actions).expect("valid UI actions");
        let scope = FocusScope::new(
            STANDALONE_WINDOW,
            &app.ui_actions,
            app.focus_root_for_window(STANDALONE_WINDOW),
        );
        if let Some(restored) = scope.resolve_request(&mut app.input_ownership) {
            app.set_focus_target(restored.map(|target| target.element));
        }
        (built.root, built.workspace_regions)
    }

    #[test]
    fn tab_context_tear_off_targets_the_clicked_item_and_rejects_a_stale_target() {
        let mut app = App::new();
        let pane = PaneId(2);
        let selected = app.demo.workspace.pane(pane).expect("right pane").items[0].id;
        let clicked = app.demo.workspace.pane(pane).expect("right pane").items[1].id;
        let root = install_action_route(&mut app);
        let layout = LayoutEngine
            .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
            .expect("gallery layout");
        app.hit_test = HitTestTree::build(&root, &layout);
        let tab = app
            .ui_actions
            .focus_order()
            .iter()
            .copied()
            .find(|element| {
                app.ui_actions.action_at(*element)
                    == Some(&DemoAction::Workspace(WorkspaceViewAction::Tab(clicked)))
            })
            .expect("inactive tab action");
        let rect = layout.node(tab).expect("inactive tab layout").rect;
        app.open_context_menu(Point::new(
            rect.x + rect.width * 0.5,
            rect.y + rect.height * 0.5,
        ));
        assert_eq!(app.demo.menu.current(), Some(MenuId(6)));
        assert_eq!(
            app.demo.workspace.pane(pane).and_then(|pane| pane.active),
            Some(selected)
        );
        let command = demo_menu(MenuId(6), &app.demo).items[0].action;
        assert_eq!(command, DemoAction::TearOffTab(clicked));
        app.apply_demo_action(command);
        assert_eq!(app.demo.workspace.floating_windows().len(), 1);
        assert!(
            app.demo.workspace.floating_windows()[0]
                .root
                .panes()
                .into_iter()
                .any(|pane| app
                    .demo
                    .workspace
                    .pane(pane)
                    .is_some_and(|pane| pane.items.iter().any(|item| item.id == clicked)))
        );
        assert_eq!(
            app.demo.workspace.pane(pane).and_then(|pane| pane.active),
            Some(selected)
        );
        app.demo
            .workspace
            .close_item(clicked)
            .expect("close torn tab");
        app.apply_demo_action(command);
        assert!(app.demo.workspace.item(clicked).is_none());
        assert!(app.demo.workspace.floating_windows().is_empty());
    }

    mod collection_navigation;
    mod collections;
    mod dock_lifecycle;
    #[path = "documents.rs"]
    mod documents;
    mod layout;
    mod scroll_input;

    fn collect_test_elements<'a>(element: &'a Element, out: &mut Vec<&'a Element>) {
        out.push(element);
        for child in &element.children {
            collect_test_elements(child, out);
        }
    }

    fn collection_window_fixture(
        row: &Element,
        action: DemoAction,
    ) -> (UiScopes, ElementId, ActionRoute<DemoAction>) {
        let scopes = UiScopes::new();
        let focus = scopes.gallery.element(1_000_001);
        let mut row = row.clone();
        row.id = focus;
        let root = Element::container(
            scopes.root.element(1),
            ElementKey::new("native.collection.fixture"),
        )
        .child(row);
        let route = ActionRoute::build(&root, [(focus, action)]).expect("native collection route");
        (scopes, focus, route)
    }

    /// ADR-0242 decision 9: a mouse click must keep the open popup alive until the release
    /// resolves it. Closing the menu on press rebuilds the tree without the item, so
    /// `PointerClick::release` no longer finds the pressed target and the action (for example
    /// TearOffTab) is dropped while dragging the same tab still works.
    #[test]
    fn pressing_a_menu_entry_keeps_the_popup_until_the_click_resolves() {
        let mut app = App::new();
        let pane = PaneId(2);
        let clicked = app.demo.workspace.pane(pane).expect("right pane").items[1].id;

        let root = install_action_route(&mut app);
        let layout = LayoutEngine
            .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
            .expect("gallery layout");
        app.hit_test = HitTestTree::build(&root, &layout);
        let tab = app
            .ui_actions
            .focus_order()
            .iter()
            .copied()
            .find(|element| {
                app.ui_actions.action_at(*element)
                    == Some(&DemoAction::Workspace(WorkspaceViewAction::Tab(clicked)))
            })
            .expect("inactive tab action");
        let rect = layout.node(tab).expect("inactive tab layout").rect;
        app.open_context_menu(Point::new(
            rect.x + rect.width * 0.5,
            rect.y + rect.height * 0.5,
        ));

        // The host paints the tab menu before the pointer is released.
        let root = install_action_route(&mut app);
        let layout = LayoutEngine
            .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
            .expect("menu layout");
        app.hit_test = HitTestTree::build(&root, &layout);
        let mut elements = Vec::new();
        collect_test_elements(&root, &mut elements);
        let item = elements
            .iter()
            .copied()
            .find(|element| {
                app.ui_actions.action_at(element.id) == Some(&DemoAction::TearOffTab(clicked))
            })
            .expect("tear off menu entry");
        let item_rect = layout.node(item.id).expect("menu entry layout").rect;
        let item_center = Point::new(
            item_rect.x + item_rect.width * 0.5,
            item_rect.y + item_rect.height * 0.5,
        );
        app.cursor = item_center;
        app.pointer_press();
        assert!(
            app.demo.menu.is_open(),
            "pressing a menu entry must not close the popup before the release"
        );

        // The press requests a frame; the release then resolves the same routed target.
        let root = install_action_route(&mut app);
        let layout = LayoutEngine
            .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
            .expect("rebuilt menu layout");
        app.hit_test = HitTestTree::build(&root, &layout);
        let released = app
            .pointer_click
            .release(
                STANDALONE_WINDOW,
                PointerButton::Primary,
                &app.ui_actions,
                app.hit_test.hit(app.cursor),
            )
            .copied();
        assert_eq!(released, Some(DemoAction::TearOffTab(clicked)));

        // A press outside the popup still dismisses the menu and keeps its own action.
        let mut elements = Vec::new();
        collect_test_elements(&root, &mut elements);
        let stepper = elements
            .iter()
            .copied()
            .find(|element| {
                matches!(
                    app.ui_actions.action_at(element.id),
                    Some(DemoAction::StepNumber(_))
                )
            })
            .expect("gallery stepper button");
        let stepper_rect = layout.node(stepper.id).expect("stepper layout").rect;
        app.cursor = Point::new(
            stepper_rect.x + stepper_rect.width * 0.5,
            stepper_rect.y + stepper_rect.height * 0.5,
        );
        app.pointer_press();
        assert!(
            !app.demo.menu.is_open(),
            "a press outside the popup still dismisses the menu"
        );
    }

    #[test]
    fn gallery_password_masks_secret_and_blocks_copy_cut_commands() {
        let mut app = App::new();
        activate_gallery_page(&mut app, "gallery.inputs");
        app.password_state
            .reset(&mut app.text_context, "private-value")
            .expect("password");
        let root = app.scopes.gallery.element(PASSWORD_ROOT);
        let display = app.scopes.gallery.element(PASSWORD_ROOT + 1);
        let hidden = install_action_route(&mut app);
        assert!(matches!(
            &hidden.find(display).expect("masked display").kind,
            vesta_gpui::ElementKind::TextInput { .. }
        ));
        app.apply_demo_action(DemoAction::FocusPassword);
        assert_eq!(
            app.text_session_field(STANDALONE_WINDOW),
            Some(TextFieldKind::Password)
        );
        assert_eq!(app.text_session_field(UiWindowId(77)), None);
        assert!(!app.handle_window_text_event(
            UiWindowId(77),
            &WindowEvent::Ime(winit::event::Ime::Commit("foreign".to_owned()))
        ));
        assert_eq!(app.password_state.text(), "private-value");
        assert!(app.handle_window_text_event(
            STANDALONE_WINDOW,
            &WindowEvent::Ime(winit::event::Ime::Enabled)
        ));
        app.password_state.act(
            &mut app.text_context,
            vesta_gpui::TextInputAction::TextEnd { extend: false },
        );
        assert!(app.handle_window_text_event(
            STANDALONE_WINDOW,
            &WindowEvent::Ime(winit::event::Ime::Commit("z".to_owned()))
        ));
        assert_eq!(app.password_state.text(), "private-valuez");
        app.password_state.act(
            &mut app.text_context,
            vesta_gpui::TextInputAction::SelectAll,
        );
        app.password_state
            .insert(
                &mut app.text_context,
                "Clipboard 中文 😀",
                vesta_gpui::TextEditKind::Paste,
            )
            .expect("replace selection");
        assert_eq!(app.password_state.text(), "Clipboard 中文 😀");
        app.password_state
            .act(&mut app.text_context, vesta_gpui::TextInputAction::Undo);
        assert_eq!(app.password_state.text(), "private-valuez");
        app.apply_demo_action(DemoAction::TogglePasswordReveal);
        let revealed = install_action_route(&mut app);
        app.set_focus_target(Some(app.scopes.gallery.element(PASSWORD_ROOT + 3)));
        assert!(
            app.demo.password_revealed,
            "reveal button belongs to the field"
        );
        app.apply_demo_action(DemoAction::TogglePasswordReveal);
        assert!(
            !app.demo.password_revealed,
            "Hide must not reveal again during focus transfer"
        );
        app.apply_demo_action(DemoAction::TogglePasswordReveal);
        assert!(matches!(
            &revealed.find(display).expect("revealed display").kind,
            vesta_gpui::ElementKind::TextInput { .. }
        ));
        for _ in 0..2 {
            app.traverse_focus(false);
            assert!(app.demo.password_revealed, "field controls retain reveal");
        }
        app.traverse_focus(false);
        assert!(!app.demo.password_revealed, "Tab out hides the secret");
        app.set_focus_target(Some(root));
        app.apply_demo_action(DemoAction::TogglePasswordReveal);
        let layout = LayoutEngine
            .compute(&revealed, Viewport::new(1280.0, 2400.0, 1.0))
            .expect("password semantics layout");
        let mut update = app.semantics.build(
            &revealed,
            &layout,
            app.input_ownership.focus().map(|focus| focus.element),
            WINDOW_TITLE,
        );
        let password_node = app
            .semantics
            .node_for_element(root)
            .expect("password semantic node");
        for (element, state) in [
            (app.scopes.gallery.element(TEXT_FIELD), &mut app.text_state),
            (
                app.scopes.gallery.element(SEARCH_FIELD),
                &mut app.search_state,
            ),
            (app.scopes.gallery.element(TEXT_AREA), &mut app.area_state),
            (root, &mut app.password_state),
        ] {
            app.semantics.enrich_text_input(
                &mut update,
                element,
                state,
                &mut app.text_context,
                Point::new(0.0, 0.0),
            );
        }
        assert_eq!(update.focus, password_node);
        app.set_focus_target(None);
        assert!(!app.demo.password_revealed);
        app.apply_demo_action(DemoAction::ClearPassword);
        assert!(app.password_state.is_empty());
        app.password_state
            .act(&mut app.text_context, vesta_gpui::TextInputAction::Undo);
        assert_eq!(app.password_state.text(), "private-valuez");
    }

    /// ADR-0252: the caret belongs to one window at a time. Keys and IME events reach the
    /// shared bridge only through that window; the other window's identical field stays a
    /// read-only mirror of the same shared text.
    #[test]
    fn text_session_routes_keys_and_ime_to_the_owning_window_only() {
        use winit::event::Ime;

        let mut app = App::new();
        activate_gallery_page(&mut app, "gallery.inputs");
        install_action_route(&mut app);
        let floating = UiWindowId(0x2000_0001);
        app.native_text_bridges
            .register(floating)
            .expect("registered simulated window");

        app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::Main, None);
        assert_eq!(
            app.text_session_field(STANDALONE_WINDOW),
            Some(TextFieldKind::Main)
        );
        assert_eq!(app.text_session_field(floating), None);

        let theme_before = app.theme;
        assert!(!app.handle_window_key(STANDALONE_WINDOW, &Key::Character("t".into())));
        assert_eq!(
            app.theme, theme_before,
            "text-owned keys cannot run a global action"
        );
        app.text_state
            .preedit(&mut app.text_context, "候选", None)
            .expect("composition");
        for key in [
            Key::Named(NamedKey::Escape),
            Key::Named(NamedKey::Tab),
            Key::Character("t".into()),
        ] {
            assert!(
                !app.handle_window_key(STANDALONE_WINDOW, &key),
                "IME owns its key"
            );
            assert!(app.text_state.is_composing());
            assert!(!app.demo.palette_session.is_open());
            assert_eq!(app.theme, theme_before);
        }
        app.text_state.cancel_compose(&mut app.text_context);

        let before = app.text_state.text().to_owned();
        assert!(
            !app.handle_window_text_event(
                floating,
                &WindowEvent::Ime(Ime::Commit("他".to_owned()))
            ),
            "a window without the caret must not consume text events"
        );
        assert_eq!(app.text_state.text(), before);

        assert!(app.handle_window_text_event(STANDALONE_WINDOW, &WindowEvent::Ime(Ime::Enabled)));
        assert!(app.handle_window_text_event(
            STANDALONE_WINDOW,
            &WindowEvent::Ime(Ime::Commit("他".to_owned()))
        ));
        assert!(
            app.text_state.text().contains('他'),
            "the owning window commits into the shared field: {}",
            app.text_state.text()
        );
        assert!(app.handle_window_text_event(STANDALONE_WINDOW, &WindowEvent::Ime(Ime::Disabled)));

        app.clear_text_session();
        assert_eq!(app.text_session_field(STANDALONE_WINDOW), None);
        assert!(
            !app.handle_window_text_event(
                STANDALONE_WINDOW,
                &WindowEvent::Ime(Ime::Commit("文".to_owned()))
            ),
            "without a caret session no window consumes text events"
        );

        app.text_session = Some(TextSession {
            window: floating,
            field: TextFieldKind::Main,
        });
        assert!(app.handle_window_text_event(floating, &WindowEvent::Ime(Ime::Enabled)));
        assert!(app.handle_window_text_event(
            floating,
            &WindowEvent::Ime(Ime::Preedit("候选".into(), Some((0, 6))))
        ));
        let before = app.text_state.text().to_owned();
        app.finish_window_text_session(STANDALONE_WINDOW);
        assert_eq!(app.text_session_field(floating), Some(TextFieldKind::Main));
        assert!(
            app.text_state.is_composing(),
            "foreign blur must preserve the active composition"
        );
        assert_eq!(app.text_state.text(), before);
        assert!(
            app.handle_window_text_event(floating, &WindowEvent::Ime(Ime::Commit("界".into())))
        );
        assert!(
            app.text_state.text().contains('界'),
            "foreign blur must not reset the owning bridge"
        );
        app.finish_window_text_session(floating);
        assert!(app.text_session.is_none());

        // Native blur restores a scrub's value before completing the text session.
        // Its last drag draft must not be committed over that restored value.
        app.demo.number = 128;
        app.number_state
            .reset(&mut app.text_context, "400")
            .expect("drag draft");
        app.text_session = Some(TextSession {
            window: floating,
            field: TextFieldKind::Number,
        });
        app.complete_window_text_event(Some(TextSessionExit::WindowBlur(floating)), true, None);
        assert_eq!(app.demo.number, 128);
        assert_eq!(app.number_state.text(), "128");
        assert!(app.text_session.is_none());
        for value in [Some(1.0), None] {
            app.demo.vector[1] = value;
            app.vector_states[1]
                .reset(&mut app.text_context, "9")
                .expect("vector drag draft");
            app.text_session = Some(TextSession {
                window: floating,
                field: TextFieldKind::Vector(1),
            });
            app.complete_window_text_event(
                Some(TextSessionExit::WindowBlur(floating)),
                false,
                Some(1),
            );
            assert_eq!(app.demo.vector[1], value);
            assert_eq!(
                app.vector_states[1].text(),
                if value.is_some() { "1" } else { "—" }
            );
        }
        app.text_session = Some(TextSession {
            window: floating,
            field: TextFieldKind::Number,
        });
        app.number_state
            .replace_accessibility(&mut app.text_context, "-42")
            .expect("ordinary draft");
        app.complete_window_text_event(Some(TextSessionExit::FocusChange), false, None);
        assert_eq!(
            app.demo.number, -42,
            "explicit focus transfer still commits the old field"
        );
        assert!(app.text_session.is_none());

        // Read-only labels share the window session without admitting stale IME
        // edits. Selection remains in the caller-owned source when focus exits.
        app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::Label, None);
        assert_eq!(
            app.text_session_field(STANDALONE_WINDOW),
            Some(TextFieldKind::Label)
        );
        app.readonly_label_state.act(
            &mut app.text_context,
            vesta_gpui::TextInputAction::SelectAll,
        );
        assert_eq!(
            app.readonly_label_state.selected_text(),
            Some(SELECTABLE_LABEL_SOURCE)
        );
        for ime in [
            Ime::Enabled,
            Ime::Preedit("候选".into(), None),
            Ime::Commit("提交".into()),
            Ime::Disabled,
        ] {
            assert!(app.handle_window_text_event(STANDALONE_WINDOW, &WindowEvent::Ime(ime)));
            assert_eq!(app.readonly_label_state.text(), SELECTABLE_LABEL_SOURCE);
            assert!(!app.readonly_label_state.is_composing());
        }
        app.finish_window_text_session(STANDALONE_WINDOW);
        assert_eq!(
            app.readonly_label_state.selected_text(),
            Some(SELECTABLE_LABEL_SOURCE)
        );
    }

    #[test]
    fn changing_text_focus_cancels_the_old_ime_preedit() {
        let mut app = App::new();
        activate_gallery_page(&mut app, "gallery.inputs");
        install_action_route(&mut app);
        app.set_text_focus(Some(TextFieldKind::Main));
        app.text_bridge
            .handle_ime::<std::convert::Infallible>(
                &winit::event::Ime::Enabled,
                &mut app.text_state,
                &mut app.text_context,
                false,
            )
            .expect("old field IME starts");
        let original_text = app.text_state.text().to_owned();
        app.text_state
            .preedit(&mut app.text_context, "中文", None)
            .expect("preedit succeeds");
        assert!(app.text_state.is_composing());

        app.traverse_focus(false);
        assert_eq!(app.focused_text_field(), Some(TextFieldKind::Search));
        assert!(!app.text_state.is_composing());
        assert_eq!(app.text_state.text(), original_text);

        let search_before = app.search_state.text().to_owned();
        app.text_bridge
            .handle_ime::<std::convert::Infallible>(
                &winit::event::Ime::Commit("旧字段提交".to_owned()),
                &mut app.search_state,
                &mut app.text_context,
                false,
            )
            .expect("late commit is ignored");
        assert_eq!(app.search_state.text(), search_before);

        app.search_state
            .preedit(&mut app.text_context, "搜索", None)
            .expect("search preedit succeeds");
        app.set_text_focus(None);
        assert!(!app.search_state.is_composing());
        assert!(!app.search_state.text().contains("搜索"));
    }

    #[test]
    fn leaving_and_returning_to_text_field_starts_a_new_undo_group() {
        let mut app = App::new();
        app.text_state
            .reset(&mut app.text_context, "")
            .expect("empty");
        app.set_text_focus(Some(TextFieldKind::Main));
        app.text_state
            .insert(
                &mut app.text_context,
                "first",
                vesta_gpui::TextEditKind::Typing,
            )
            .expect("first edit");
        app.set_text_focus(None);
        app.set_text_focus(Some(TextFieldKind::Main));
        app.text_state
            .insert(
                &mut app.text_context,
                "second",
                vesta_gpui::TextEditKind::Typing,
            )
            .expect("second edit");
        app.text_state
            .act(&mut app.text_context, vesta_gpui::TextInputAction::Undo);
        assert_eq!(app.text_state.text(), "first");
    }

    #[test]
    fn menu_dismissal_restores_only_a_live_opener_focus() {
        let mut app = App::new();
        activate_gallery_page(&mut app, "gallery.inputs");
        install_action_route(&mut app);
        app.set_text_focus(Some(TextFieldKind::Main));
        app.open_menu(Point::new(100.0, 100.0));
        assert_eq!(app.focused_text_field(), None);
        let first_item = menu_item_element(app.scopes.menu, MenuItemId(10));
        assert_eq!(
            app.input_ownership.focus().map(|target| target.element),
            Some(first_item)
        );
        install_action_route(&mut app);
        assert!(app.ui_actions.focus_order().contains(&first_item));
        assert!(!app.handle_window_menu_key(UiWindowId(99), &Key::Named(NamedKey::Escape)));
        assert!(app.demo.menu.is_open());
        let theme = app.theme;
        assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Character("t".into())));
        assert_eq!(
            app.theme, theme,
            "an open menu owns otherwise global shortcuts"
        );
        assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::End)));
        assert_eq!(app.demo.menu.highlighted(), Some(MenuItemId(11)));
        assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Home)));
        assert_eq!(app.demo.menu.highlighted(), Some(MenuItemId(10)));
        let export = menu_item_element(app.scopes.menu, MenuItemId(13));
        app.hover_menu_item(Some(export));
        assert_eq!(app.demo.menu.highlighted(), Some(MenuItemId(13)));
        assert_eq!(
            app.input_ownership.focus().map(|target| target.element),
            Some(export)
        );
        assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowRight)));
        assert_eq!(app.demo.menu.current(), Some(MenuId(2)));
        assert_eq!(
            app.demo.menu.highlighted(),
            None,
            "disabled export entries cannot focus"
        );
        let before = app.demo.workspace.clone();
        app.demo.menu.highlight(Some(MenuItemId(20)));
        assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Enter)));
        assert_eq!(app.demo.menu.highlighted(), None);
        assert!(app.demo.menu.is_open());
        assert_eq!(
            app.demo.workspace, before,
            "invalid highlight cannot activate reset"
        );
        assert!(!app.prepare_window_menu_action(
            STANDALONE_WINDOW,
            DemoAction::MenuCommand(MenuItemId(20))
        ));
        assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowLeft)));
        assert_eq!(app.demo.menu.current(), Some(MenuId(1)));
        app.apply_demo_action(DemoAction::MenuCommand(MenuItemId(1)));
        assert_eq!(app.focused_text_field(), Some(TextFieldKind::Main));

        app.open_menu(Point::new(100.0, 100.0));
        app.close_menu(false);
        assert_eq!(app.focused_text_field(), None);

        app.open_menu(Point::new(100.0, 100.0));
        app.demo.menu.open_for(
            MenuId(1),
            Some(InputTarget {
                window: STANDALONE_WINDOW,
                element: ElementId::new(u64::MAX),
            }),
        );
        app.close_menu(true);
        assert_eq!(app.input_ownership.focus(), None);

        let opener = app.scopes.header.element(APP_MENU);
        app.set_focus_target(Some(opener));
        app.apply_demo_action(DemoAction::ToggleApplicationMenu);
        assert_eq!(app.demo.menu_origin, MenuOrigin::ApplicationBar);
        assert_eq!(app.demo.menu.current(), Some(MenuId(0)));
        let expanded = install_action_route(&mut app);
        assert!(
            expanded
                .find(app.scopes.header.element(HEADER_FILE_MENU))
                .is_some()
        );
        assert!(expanded.find(app.scopes.header.element(25)).is_none());
        for expected in [MenuId(1), MenuId(3), MenuId(4), MenuId(0)] {
            assert!(
                app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowRight))
            );
            assert_eq!(app.demo.menu.current(), Some(expected));
        }
        assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowLeft)));
        assert_eq!(app.demo.menu.current(), Some(MenuId(4)));
        app.hover_menu_item(Some(app.scopes.header.element(HEADER_FILE_MENU)));
        assert_eq!(app.demo.menu.current(), Some(MenuId(1)));
        app.hover_menu_item(Some(export));
        assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowRight)));
        assert_eq!(app.demo.menu.current(), Some(MenuId(2)));
        assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowLeft)));
        assert_eq!(app.demo.menu.current(), Some(MenuId(1)));
        assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Escape)));
        assert!(!app.demo.menu.is_open());
        assert_eq!(app.demo.menu_origin, MenuOrigin::Context);
        assert_eq!(
            app.input_ownership.focus().map(|focus| focus.element),
            Some(opener)
        );
        let collapsed = install_action_route(&mut app);
        assert!(collapsed.find(app.scopes.header.element(25)).is_some());
        assert!(
            collapsed
                .find(app.scopes.header.element(HEADER_FILE_MENU))
                .is_none()
        );
        app.apply_demo_action(DemoAction::ToggleApplicationMenu);
        app.apply_demo_action(DemoAction::ToggleTheme);
        assert!(
            !app.demo.menu.is_open(),
            "application commands dismiss the menu bar"
        );
        app.set_focus_target(Some(opener));
        app.apply_demo_action(DemoAction::ToggleApplicationMenu);
        install_action_route(&mut app);
        assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Tab)));
        install_action_route(&mut app);
        assert_eq!(
            app.window_focus(STANDALONE_WINDOW),
            Some(app.scopes.header.element(MODE_2D)),
            "menu Tab traverses the collapsed tree, not a disappearing File header"
        );
        let previous = FocusScope::new(STANDALONE_WINDOW, &app.ui_actions, None)
            .order()
            .last()
            .copied();
        app.set_focus_target(Some(opener));
        app.apply_demo_action(DemoAction::ToggleApplicationMenu);
        install_action_route(&mut app);
        app.text_bridge
            .update_modifiers(&winit::event::Modifiers::from(
                winit::keyboard::ModifiersState::SHIFT,
            ));
        assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Tab)));
        install_action_route(&mut app);
        assert_eq!(app.input_ownership.focus(), previous);
        app.text_bridge
            .update_modifiers(&winit::event::Modifiers::from(
                winit::keyboard::ModifiersState::empty(),
            ));
        app.open_menu(Point::new(100.0, 100.0));
        assert_eq!(app.demo.menu_origin, MenuOrigin::Context);
        app.hover_menu_item(Some(app.scopes.header.element(HEADER_WINDOW_MENU)));
        assert_eq!(
            app.demo.menu.current(),
            Some(MenuId(1)),
            "context menus do not use header navigation"
        );
        assert!(app.handle_window_menu_key(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowLeft)));
        assert!(!app.demo.menu.is_open());
    }

    #[test]
    fn frameless_resize_resolves_edges_and_corners_without_swallowing_the_center() {
        let size = Size::new(900.0, 600.0);
        assert_eq!(
            chrome_resize_direction(Point::new(2.0, 2.0), size),
            Some(ResizeDirection::NorthWest)
        );
        assert_eq!(
            chrome_resize_direction(Point::new(898.0, 300.0), size),
            Some(ResizeDirection::East)
        );
        assert_eq!(
            chrome_resize_direction(Point::new(450.0, 599.0), size),
            Some(ResizeDirection::South)
        );
        assert_eq!(chrome_resize_direction(Point::new(450.0, 30.0), size), None);
        scroll_input::scrollbar_thumb_precedes_window_chrome();
    }

    #[test]
    fn accessibility_requests_use_the_same_action_and_text_edit_paths() {
        fn enrich_text_fields(app: &mut App, update: &mut accesskit::TreeUpdate) {
            for (element, state) in [
                (app.scopes.gallery.element(TEXT_FIELD), &mut app.text_state),
                (
                    app.scopes.gallery.element(SEARCH_FIELD),
                    &mut app.search_state,
                ),
                (app.scopes.gallery.element(TEXT_AREA), &mut app.area_state),
                (
                    app.scopes.gallery.element(NUMBER_FIELD),
                    &mut app.number_state,
                ),
            ] {
                app.semantics.enrich_text_input(
                    update,
                    element,
                    state,
                    &mut app.text_context,
                    Point::new(0.0, 0.0),
                );
            }
        }
        #[derive(Default)]
        struct AccessibilityChanges {
            focus: Vec<Option<accesskit::NodeId>>,
        }
        impl accesskit_consumer::TreeChangeHandler for AccessibilityChanges {
            fn node_added(&mut self, _: &accesskit_consumer::Node<'_>) {}
            fn node_updated(
                &mut self,
                _: &accesskit_consumer::Node<'_>,
                _: &accesskit_consumer::Node<'_>,
            ) {
            }
            fn focus_moved(
                &mut self,
                _: Option<&accesskit_consumer::Node<'_>>,
                new_node: Option<&accesskit_consumer::Node<'_>>,
            ) {
                self.focus.push(new_node.map(|node| node.id()));
            }
            fn node_removed(&mut self, _: &accesskit_consumer::Node<'_>) {}
        }
        let mut app = App::new();
        activate_gallery_page(&mut app, "gallery.inputs");
        let root = install_action_route_with_viewport(&mut app, Size::new(1280.0, 2400.0));
        let layout = LayoutEngine
            .compute(&root, Viewport::new(1280.0, 2400.0, 1.0))
            .expect("gallery layout");
        let mut update = app.semantics.build(&root, &layout, None, WINDOW_TITLE);
        enrich_text_fields(&mut app, &mut update);
        let mut consumer = accesskit_consumer::Tree::new(update.clone(), true);
        let mut changes = AccessibilityChanges::default();

        let text = app
            .semantics
            .node_for_element(app.scopes.gallery.element(TEXT_FIELD))
            .expect("text input semantic node");
        app.accessibility_action(accesskit::ActionRequest {
            action: AccessibilityAction::SetValue,
            target: text,
            data: Some(AccessibilityActionData::Value("无障碍输入".into())),
        });
        assert_eq!(app.text_state.text(), "无障碍输入");
        assert_eq!(app.focused_text_field(), Some(TextFieldKind::Main));
        app.text_state
            .act(&mut app.text_context, vesta_gpui::TextInputAction::Undo);
        assert_eq!(app.text_state.text(), "中文 IME / Selection / Undo");
        app.text_state
            .act(&mut app.text_context, vesta_gpui::TextInputAction::Redo);
        assert_eq!(app.text_state.text(), "无障碍输入");
        app.window_accessibility_action(
            UiWindowId(999),
            accesskit::ActionRequest {
                action: AccessibilityAction::SetValue,
                target: text,
                data: Some(AccessibilityActionData::Value("foreign edit".into())),
            },
        );
        assert_eq!(app.text_state.text(), "无障碍输入");

        let password = app
            .semantics
            .node_for_element(app.scopes.gallery.element(PASSWORD_ROOT))
            .expect("password semantic node");
        app.accessibility_action(accesskit::ActionRequest {
            action: AccessibilityAction::SetValue,
            target: password,
            data: Some(AccessibilityActionData::Value("sample-secret".into())),
        });
        assert_eq!(app.password_state.text(), "sample-secret");
        assert_eq!(app.focused_text_field(), Some(TextFieldKind::Password));
        let before = app.password_state.clone();
        app.accessibility_action(accesskit::ActionRequest {
            action: AccessibilityAction::SetValue,
            target: password,
            data: Some(AccessibilityActionData::Value("x".repeat(257).into())),
        });
        assert_eq!(
            app.password_state, before,
            "rejected platform edits preserve selection and history"
        );

        let area = app
            .semantics
            .node_for_element(app.scopes.gallery.element(TEXT_AREA))
            .expect("multiline input semantic node");
        assert_eq!(
            update
                .nodes
                .iter()
                .find(|(id, _)| *id == area)
                .expect("multiline node in update")
                .1
                .role(),
            accesskit::Role::MultilineTextInput
        );
        app.accessibility_action(accesskit::ActionRequest {
            action: AccessibilityAction::SetValue,
            target: area,
            data: Some(AccessibilityActionData::Value("地形\n道路".into())),
        });
        assert_eq!(app.area_state.text(), "地形\n道路");
        assert_eq!(app.focused_text_field(), Some(TextFieldKind::Area));
        let mut focused_update = app.semantics.build(
            &root,
            &layout,
            app.input_ownership.focus().map(|focus| focus.element),
            WINDOW_TITLE,
        );
        enrich_text_fields(&mut app, &mut focused_update);
        assert_eq!(focused_update.focus, area);
        let selection = focused_update
            .nodes
            .iter()
            .find(|(id, _)| *id == area)
            .expect("area projection")
            .1
            .text_selection()
            .cloned()
            .expect("area selection");
        consumer.update_and_process_changes(focused_update, &mut changes);
        assert_eq!(consumer.state().focus_id(), Some(area));
        assert_eq!(changes.focus.pop(), Some(Some(area)));
        let search = app
            .semantics
            .node_for_element(app.scopes.gallery.element(SEARCH_FIELD))
            .expect("search semantic node");
        app.set_text_focus(Some(TextFieldKind::Search));
        let mut focused_update = app.semantics.build(
            &root,
            &layout,
            app.input_ownership.focus().map(|focus| focus.element),
            WINDOW_TITLE,
        );
        enrich_text_fields(&mut app, &mut focused_update);
        assert_eq!(focused_update.focus, search);
        consumer.update_and_process_changes(focused_update, &mut changes);
        assert_eq!(consumer.state().focus_id(), Some(search));
        assert_eq!(changes.focus.pop(), Some(Some(search)));
        app.accessibility_action(accesskit::ActionRequest {
            action: AccessibilityAction::SetTextSelection,
            target: area,
            data: Some(AccessibilityActionData::SetTextSelection(selection)),
        });
        assert_eq!(
            app.focused_text_field(),
            Some(TextFieldKind::Area),
            "an unchanged valid range still acquires the source field's session"
        );
        app.set_text_focus(Some(TextFieldKind::Search));
        let invalid = accesskit::TextPosition {
            node: accesskit::NodeId(u64::MAX),
            character_index: 0,
        };
        app.accessibility_action(accesskit::ActionRequest {
            action: AccessibilityAction::SetTextSelection,
            target: area,
            data: Some(AccessibilityActionData::SetTextSelection(
                accesskit::TextSelection {
                    anchor: invalid,
                    focus: invalid,
                },
            )),
        });
        assert_eq!(
            app.focused_text_field(),
            Some(TextFieldKind::Search),
            "invalid run identities do not take focus"
        );

        let number = app
            .semantics
            .node_for_element(app.scopes.gallery.element(NUMBER_FIELD))
            .expect("number semantic node");
        let number_run = update
            .nodes
            .iter()
            .find(|(id, _)| *id == number)
            .expect("number projection")
            .1
            .children()[0];
        app.accessibility_action(accesskit::ActionRequest {
            action: AccessibilityAction::SetTextSelection,
            target: number,
            data: Some(AccessibilityActionData::SetTextSelection(
                accesskit::TextSelection {
                    anchor: accesskit::TextPosition {
                        node: number_run,
                        character_index: 0,
                    },
                    focus: accesskit::TextPosition {
                        node: number_run,
                        character_index: 2,
                    },
                },
            )),
        });
        assert_eq!(app.focused_text_field(), Some(TextFieldKind::Number));
        assert_eq!(
            app.number_state.selected_text(),
            Some("12"),
            "entering the scalar session must not erase the platform range"
        );

        activate_gallery_page(&mut app, "gallery.controls");
        let slider_root = install_action_route_with_viewport(&mut app, Size::new(1280.0, 2400.0));
        let slider_layout = LayoutEngine
            .compute(&slider_root, Viewport::new(1280.0, 2400.0, 1.0))
            .expect("controls layout");
        let slider_update = app
            .semantics
            .build(&slider_root, &slider_layout, None, WINDOW_TITLE);
        let slider_element =
            semantic_id(&slider_root, SemanticRole::Slider).expect("slider element");
        let slider_node = app
            .semantics
            .node_for_element(slider_element)
            .expect("slider semantic node");
        let slider_semantic = &slider_update
            .nodes
            .iter()
            .find(|(id, _)| *id == slider_node)
            .expect("slider node in update")
            .1;
        assert!(slider_semantic.supports_action(AccessibilityAction::SetValue));
        assert_eq!(slider_semantic.min_numeric_value(), Some(0.0));
        assert_eq!(slider_semantic.max_numeric_value(), Some(1.0));
        app.accessibility_action(accesskit::ActionRequest {
            action: AccessibilityAction::SetValue,
            target: slider_node,
            data: Some(AccessibilityActionData::NumericValue(0.75)),
        });
        assert_eq!(app.demo.slider, 0.75);

        let mode = app
            .semantics
            .node_for_element(app.scopes.header.element(MODE_3D))
            .expect("3D mode semantic node");
        app.accessibility_action(accesskit::ActionRequest {
            action: AccessibilityAction::Click,
            target: mode,
            data: None,
        });
        assert_eq!(app.mode, AcceptanceMode::Viewport3d);

        // An action queued by the platform before a rebuild must not edit a
        // field that the current interactive tree has removed or disabled.
        app.ui_actions = ActionRoute::default();
        app.accessibility_action(accesskit::ActionRequest {
            action: AccessibilityAction::SetValue,
            target: text,
            data: Some(AccessibilityActionData::Value("stale edit".into())),
        });
        assert_eq!(app.text_state.text(), "无障碍输入");
    }

    #[test]
    fn tab_drag_targets_reorder_split_and_float_through_workspace_actions() {
        fn assert_rect_near(actual: Rect, expected: Rect) {
            assert!((actual.x - expected.x).abs() < 0.01);
            assert!((actual.y - expected.y).abs() < 0.01);
            assert!((actual.width - expected.width).abs() < 0.01);
            assert!((actual.height - expected.height).abs() < 0.01);
        }

        let model = demo_workspace();
        let pane = model.panes().first().expect("demo pane").id;
        let frame = DockFrame {
            panes: vec![DockPaneFrame {
                pane,
                container: ContainerRef::Main,
                root: Rect::new(0.0, 0.0, 400.0, 300.0),
                tab_strip: Rect::new(0.0, 0.0, 400.0, 32.0),
                body: Rect::new(0.0, 32.0, 400.0, 268.0),
            }],
            tabs: vec![
                DockTabFrame {
                    pane,
                    rect: Rect::new(0.0, 0.0, 100.0, 32.0),
                },
                DockTabFrame {
                    pane,
                    rect: Rect::new(100.0, 0.0, 100.0, 32.0),
                },
            ],
            splits: vec![
                DockSplitFrame {
                    container: ContainerRef::Main,
                    split: SplitId(1),
                    axis: SplitAxis::Row,
                    rect: Rect::new(0.0, 0.0, 400.0, 300.0),
                },
                DockSplitFrame {
                    container: ContainerRef::Main,
                    split: SplitId(2),
                    axis: SplitAxis::Column,
                    rect: Rect::new(0.0, 0.0, 200.0, 300.0),
                },
            ],
            workspace: Some(Rect::new(0.0, 0.0, 400.0, 300.0)),
        };
        assert_eq!(
            frame.split_rect(ContainerRef::Main, SplitId(2), SplitAxis::Column),
            Some(Rect::new(0.0, 0.0, 200.0, 300.0)),
        );
        let tab_insert = frame
            .target(Point::new(240.0, 18.0), &model)
            .expect("tab insertion target");
        assert_eq!(tab_insert.action, DockAction::Merge { pane, slot: 2 });
        assert_eq!(tab_insert.preview, frame.panes[0].tab_strip.inset(1.0));
        assert!(tab_insert.insertion.is_some());
        for region in dock_tab_split_regions(frame.tabs[1].rect) {
            let point = Point::new(
                region.rect.x + region.rect.width * 0.5,
                region.rect.y + region.rect.height * 0.5,
            );
            let target = frame.target(point, &model).expect("tab split target");
            assert_eq!(
                target.action,
                DockAction::Split {
                    pane,
                    side: region.side,
                },
            );
            assert_eq!(
                target.preview,
                dock_edge_preview(frame.panes[0].root, region.side, 0.42)
            );
        }
        assert_eq!(
            frame
                .target(Point::new(40.0, 150.0), &model)
                .map(|target| target.action),
            Some(DockAction::Split {
                pane,
                side: Side::Left
            }),
        );
        for (side, point) in [
            (Side::Top, Point::new(250.0, 55.0)),
            (Side::Right, Point::new(360.0, 150.0)),
            (Side::Bottom, Point::new(250.0, 275.0)),
        ] {
            let target = frame.target(point, &model).expect("pane edge target");
            assert_eq!(target.action, DockAction::Split { pane, side });
            assert_eq!(
                target.preview,
                dock_edge_preview(frame.panes[0].root, side, 0.42),
            );
        }
        let center = frame
            .target(Point::new(200.0, 150.0), &model)
            .expect("center target");
        assert_eq!(
            center.action,
            DockAction::Merge {
                pane,
                slot: model.pane(pane).expect("live destination").items.len()
            }
        );
        assert_eq!(center.preview, frame.panes[0].root.inset(5.0));
        let window_edge = frame
            .target(Point::new(8.0, 150.0), &model)
            .expect("window edge target");
        assert_eq!(
            window_edge.action,
            DockAction::WindowSplit {
                container: ContainerRef::Main,
                side: Side::Left,
            },
        );
        assert_eq!(
            window_edge.preview,
            dock_edge_preview(frame.workspace.expect("workspace"), Side::Left, 0.25),
        );
        for (side, point) in [
            (Side::Top, Point::new(250.0, 5.0)),
            (Side::Right, Point::new(395.0, 150.0)),
            (Side::Bottom, Point::new(250.0, 295.0)),
        ] {
            let target = frame.target(point, &model).expect("window edge target");
            assert_eq!(
                target.action,
                DockAction::WindowSplit {
                    container: ContainerRef::Main,
                    side,
                },
            );
            assert_eq!(
                target.preview,
                dock_edge_preview(frame.workspace.expect("workspace"), side, 0.25),
            );
        }
        let floating = frame
            .target(Point::new(450.0, 450.0), &model)
            .expect("floating target");
        let DockAction::Float { placement } = floating.action else {
            panic!("dragging outside the workspace should float the tab");
        };
        assert!(placement.x >= 0.0 && placement.y >= 0.0);
        assert!(placement.x + placement.width <= 400.0);
        assert!(placement.y + placement.height <= 300.0);
        assert_eq!(floating.preview.x, placement.x);
        assert_eq!(floating.preview.y, placement.y);

        let guide_target = frame
            .target(Point::new(140.0, 8.0), &model)
            .expect("directional target");
        let mut app = App::new();
        let item = app.demo.workspace.panes()[0].items[0].id;
        app.cursor = Point::new(140.0, 8.0);
        app.workspace_drag = Some(WorkspaceTabDrag {
            item,
            origin_container: ContainerRef::Main,
            origin: Point::new(120.0, 16.0),
            started: true,
            pointer_container: Some(ContainerRef::Main),
            cursor: Point::new(140.0, 8.0),
            target: Some(guide_target),
        });
        assert_eq!(app.paint_interaction().pointer, None);
        let root = install_action_route(&mut app);
        let overlay = root
            .children
            .iter()
            .find(|child| child.id == app.scopes.root.element(100))
            .expect("dock overlay belongs to the application root");
        assert!(!overlay.style.pointer_events);
        let layout = LayoutEngine
            .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
            .expect("dock overlay layout");
        let paint = PaintList::build(&root, &layout);
        assert!(
            paint
                .commands()
                .iter()
                .any(|command| command.element_id() == Some(app.scopes.root.element(101)))
        );
        let page_id = page_scroll_id(app.scopes.gallery, item);
        let page = layout.node(page_id).expect("active dock page scrollbox");
        app.cursor = Point::new(page.rect.x + page.rect.width * 0.5, page.rect.y + 20.0);
        let painted_scrollbox_commands = |interaction| {
            PaintList::build_with_focus_and_interaction(
                &root,
                &layout,
                None,
                app.theme.palette.focus,
                interaction,
            )
            .commands()
            .iter()
            .filter(|command| {
                matches!(command, vesta_gpui::PaintCommand::FillRoundedRect { element, .. } if *element == page_id)
            })
            .count()
        };
        let dock_drag_count = painted_scrollbox_commands(app.paint_interaction());
        let hover_count = painted_scrollbox_commands(PaintInteraction {
            pointer: Some(app.cursor),
            active_scrollbar: None,
        });
        assert!(
            hover_count > dock_drag_count,
            "captured dock pointer must not reveal the adjacent page scrollbar"
        );
        let foreign_cursor = Point::new(-100.0, -100.0);
        assert_eq!(
            painted_scrollbox_commands(app.window_paint_interaction(foreign_cursor, None)),
            dock_drag_count,
            "another window's cursor must not reveal this scrollbar"
        );
        assert!(
            painted_scrollbox_commands(app.window_paint_interaction(foreign_cursor, Some(page_id)))
                > dock_drag_count,
            "the owning window's captured scrollbar remains visible outside its viewport"
        );
        assert_rect_near(
            layout
                .node(app.scopes.root.element(101))
                .expect("visible preview")
                .rect,
            guide_target.preview,
        );
        for index in 0..4 {
            assert!(
                !paint.commands().iter().any(|command| {
                    command.element_id() == Some(app.scopes.root.element(103 + index))
                }),
                "directional marker should not cover the tab during a drag"
            );
        }
        app.cursor = Point::new(240.0, 18.0);
        app.workspace_drag.as_mut().expect("active drag").target = Some(tab_insert);
        let root = install_action_route(&mut app);
        let layout = LayoutEngine
            .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
            .expect("tab insertion overlay layout");
        assert_rect_near(
            layout
                .node(app.scopes.root.element(101))
                .expect("tab strip preview")
                .rect,
            tab_insert.preview,
        );
        assert_rect_near(
            layout
                .node(app.scopes.root.element(107))
                .expect("tab insertion marker")
                .rect,
            tab_insert.insertion.expect("insertion rect"),
        );
        app.workspace_drag = None;
        assert_eq!(app.paint_interaction().pointer, Some(app.cursor));
    }

    #[test]
    fn workspace_drag_crosses_native_containers_and_commits_the_stable_target() {
        let mut app = App::new();
        let item = app
            .demo
            .workspace
            .pane(PaneId(1))
            .expect("source pane")
            .items[0]
            .id;
        let floating_item = app
            .demo
            .workspace
            .pane(PaneId(2))
            .expect("target source pane")
            .items[0]
            .id;
        app.demo
            .workspace
            .dock_item(
                floating_item,
                DockAction::Float {
                    placement: WindowPlacement::new(40.0, 40.0, 360.0, 240.0),
                },
            )
            .expect("target floating window");
        let floating = app.demo.workspace.floating_windows()[0].id;
        let target_pane = app.demo.workspace.floating_windows()[0].root.panes()[0];
        let main_geometry = DockWindowGeometry {
            container: ContainerRef::Main,
            outer: Rect::new(-800.0, 80.0, 1600.0, 1000.0),
            client: Rect::new(-800.0, 80.0, 1600.0, 1000.0),
            scale: 1.25,
        };
        let floating_geometry = DockWindowGeometry {
            container: ContainerRef::Floating(floating),
            outer: Rect::new(-620.0, 130.0, 500.0, 540.0),
            client: Rect::new(-600.0, 170.0, 480.0, 480.0),
            scale: 2.0,
        };
        let frame_for = |pane, container, width, height| DockFrame {
            panes: vec![DockPaneFrame {
                pane,
                container,
                root: Rect::new(0.0, 0.0, width, height),
                tab_strip: Rect::new(0.0, 0.0, width, 32.0),
                body: Rect::new(0.0, 32.0, width, height - 32.0),
            }],
            workspace: Some(Rect::new(0.0, 0.0, width, height)),
            ..DockFrame::default()
        };
        let main_frame = frame_for(PaneId(1), ContainerRef::Main, 1280.0, 800.0);
        let floating_frame = frame_for(target_pane, ContainerRef::Floating(floating), 240.0, 240.0);
        let windows = [
            (floating_geometry, &floating_frame),
            (main_geometry, &main_frame),
        ];
        let (container, cursor, target) = route_dock_pointer(
            main_geometry,
            Point::new(352.0, 256.0),
            &windows,
            &app.demo.workspace,
        )
        .expect("captured main event resolves floating window on a negative desktop");
        assert_eq!(container, ContainerRef::Floating(floating));
        assert_eq!(cursor, Point::new(120.0, 115.0));
        let target = target.expect("floating Dock body");
        assert_eq!(
            target.action,
            DockAction::Merge {
                pane: target_pane,
                slot: 1
            }
        );
        assert!(
            route_dock_pointer(
                main_geometry,
                Point::new(352.0, 56.0),
                &windows,
                &app.demo.workspace,
            )
            .is_none(),
            "floating title bar must occlude main Dock targets"
        );
        let (_, _, edge) = route_dock_pointer(
            main_geometry,
            Point::new(163.2, 232.0),
            &windows,
            &app.demo.workspace,
        )
        .expect("floating edge route");
        assert_eq!(
            edge.expect("floating edge").action,
            DockAction::WindowSplit {
                container: ContainerRef::Floating(floating),
                side: Side::Left,
            }
        );
        app.workspace_drag = Some(WorkspaceTabDrag {
            item,
            origin_container: ContainerRef::Main,
            origin: Point::new(120.0, 16.0),
            started: false,
            pointer_container: Some(ContainerRef::Main),
            cursor: Point::new(120.0, 16.0),
            target: None,
        });

        assert!(app.update_workspace_drag(
            ContainerRef::Floating(floating),
            Point::new(16.0, 18.0),
            Some(target),
        ));
        assert!(app.dock_drag_visual(ContainerRef::Main).is_none());
        assert_eq!(
            app.dock_drag_visual(ContainerRef::Floating(floating))
                .expect("target window preview")
                .target
                .expect("resolved dock target")
                .action,
            target.action,
        );

        // This geometry fixture has no OS display; use explicit admission.
        app.finish_workspace_drag_with_admission(ContainerRef::Main, |_| true);
        assert!(
            app.demo
                .workspace
                .pane(target_pane)
                .expect("target pane survives")
                .items
                .iter()
                .any(|candidate| candidate.id == item)
        );
        assert_eq!(
            app.demo.workspace.container_of_pane(target_pane),
            Some(ContainerRef::Floating(floating))
        );

        let return_pane = PaneId(1);
        let (return_container, return_cursor, return_target) = route_dock_pointer(
            floating_geometry,
            Point::new(275.0, 192.5),
            &windows,
            &app.demo.workspace,
        )
        .expect("captured floating event resolves main client");
        assert_eq!(return_container, ContainerRef::Main);
        assert_eq!(return_cursor, Point::new(600.0, 380.0));
        let return_target = return_target.expect("main Dock body");
        assert_eq!(
            return_target.action,
            DockAction::Merge {
                pane: return_pane,
                slot: app
                    .demo
                    .workspace
                    .pane(return_pane)
                    .expect("live destination")
                    .items
                    .len()
            }
        );
        app.workspace_drag = Some(WorkspaceTabDrag {
            item,
            origin_container: ContainerRef::Floating(floating),
            origin: Point::new(80.0, 16.0),
            started: false,
            pointer_container: Some(ContainerRef::Floating(floating)),
            cursor: Point::new(80.0, 16.0),
            target: None,
        });
        assert!(app.update_workspace_drag(
            ContainerRef::Main,
            Point::new(240.0, 120.0),
            Some(return_target),
        ));
        assert!(
            app.dock_drag_visual(ContainerRef::Floating(floating))
                .is_none()
        );
        assert!(app.dock_drag_visual(ContainerRef::Main).is_some());
        app.finish_workspace_drag_with_admission(ContainerRef::Floating(floating), |_| true);
        assert!(
            app.demo
                .workspace
                .pane(return_pane)
                .expect("main target pane survives")
                .items
                .iter()
                .any(|candidate| candidate.id == item)
        );
        assert_eq!(
            app.demo.workspace.container_of_pane(return_pane),
            Some(ContainerRef::Main)
        );
    }

    #[test]
    fn nested_menu_stays_inside_a_small_window_and_flips_left() {
        let mut app = App::new();
        let mut demo = DemoState {
            menu_anchor: Point::new(880.0, 570.0),
            ..DemoState::default()
        };
        demo.menu.open(MenuId(1));
        demo.menu.enter_submenu(MenuId(2));
        let mut actions = Vec::new();
        let popup = build_menu_popup(
            app.scopes.menu,
            app.theme,
            &demo,
            None,
            Size::new(900.0, 600.0),
            &mut actions,
            &mut app.text_context,
        );
        let menus = popup
            .children
            .iter()
            .filter(|child| child.key.as_str().contains("/menu.popup"))
            .collect::<Vec<_>>();
        assert_eq!(menus.len(), 2);
        assert_eq!(
            menus[0].semantic.as_ref().map(|semantic| semantic.role),
            Some(SemanticRole::Menu)
        );
        let root_x = menus[0].style.left.expect("root popup x");
        let child_x = menus[1].style.left.expect("submenu x");
        assert!(child_x < root_x, "submenu should flip to the left edge");
        for menu in menus {
            let x = menu.style.left.expect("popup x");
            let y = menu.style.top.expect("popup y");
            let Dimension::Px(width) = menu.style.width else {
                panic!("measured popup width")
            };
            assert!(x >= 8.0 && x + width <= 892.0);
            assert!(y >= CHROME_HEIGHT + 6.0 && y + 12.0 <= 592.0 - CHROME_STATUS_HEIGHT);
        }
        let route = ActionRoute::build(&popup, actions).expect("menu actions");
        assert!(
            route
                .focus_order()
                .contains(&menu_item_element(app.scopes.menu, MenuItemId(13)))
        );
        assert!(
            !route
                .focus_order()
                .contains(&menu_item_element(app.scopes.menu, MenuItemId(20)))
        );
        demo.menu.open(MenuId(3));
        for theme in [WidgetTheme::dark(), WidgetTheme::light()] {
            let mut actions = Vec::new();
            let popup = build_menu_popup(
                app.scopes.menu,
                theme,
                &demo,
                None,
                Size::new(900.0, 600.0),
                &mut actions,
                &mut app.text_context,
            );
            let entry = popup
                .find(menu_item_element(app.scopes.menu, MenuItemId(32)))
                .expect("theme entry");
            assert_eq!(
                entry.semantic.as_ref(),
                Some(
                    &Semantic::new(SemanticRole::MenuItemCheckBox, "Dark theme")
                        .checked(theme.mode == ThemeMode::Dark)
                )
            );
            let route = ActionRoute::build(&popup, actions).expect("theme menu route");
            assert_eq!(
                route.action_at(entry.id),
                Some(&DemoAction::MenuCommand(MenuItemId(32)))
            );
        }
    }

    #[test]
    fn translation_preview_cancels_or_commits_through_typed_actions() {
        let mut app = App::new();
        let selected = &app.scene.renderables[0];
        let original = selected.translation;
        let start = vesta_viewport::project_to_pixel(app.camera, 1280.0, 800.0, original)
            .expect("selected center projects");
        let end = vesta_viewport::project_to_pixel(
            app.camera,
            1280.0,
            800.0,
            original.plus(vesta_viewport::Vec3::new(1.3, 0.0, 0.0)),
        )
        .expect("axis projects");
        let cursor = [start[0] * 0.4 + end[0] * 0.6, start[1] * 0.4 + end[1] * 0.6];
        let drag =
            TranslationDrag::begin(&app.scene, app.camera, 1280.0, 800.0, cursor[0], cursor[1])
                .expect("translation handle");
        let advanced = [
            cursor[0] + (end[0] - start[0]) * 0.25,
            cursor[1] + (end[1] - start[1]) * 0.25,
        ];
        app.transform_drag = Some(drag);
        app.apply_transform_action(drag, drag.begin_action());
        app.apply_transform_action(drag, drag.update(advanced[0], advanced[1]));
        assert!(app.scene.renderables[0].translation.x > original.x);
        app.cancel_transform_drag();
        assert_eq!(app.scene.renderables[0].translation, original);
        assert_eq!(app.scene.gizmo.active, None);

        app.transform_drag = Some(drag);
        app.apply_transform_action(drag, drag.begin_action());
        app.apply_transform_action(drag, drag.update(advanced[0], advanced[1]));
        app.transform_drag = None;
        app.apply_transform_action(drag, TransformAction::Commit);
        assert!(app.scene.renderables[0].translation.x > original.x);
        assert_eq!(app.scene.gizmo.active, None);
    }
}
