#![forbid(unsafe_code)]

mod application_chrome;
mod application_run;
mod application_state;
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
mod window_presentation;
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
mod tests;
