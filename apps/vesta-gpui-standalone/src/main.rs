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
mod presentation;
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
mod viewport_demo;
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
mod window_pointer;
mod window_presentation;
mod window_tooltips;

mod workspace_session;

use application_chrome::build_header;
use application_state::DemoState;
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
use presentation::*;
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
use window_keyboard::{demo_keymap, key_chord_caption};
use window_menus::*;
use window_platform::{HostClipboard, chrome_resize_direction};
use window_pointer::{
    NumberScrub, VectorScrub, begin_scrollbar_drag, finish_scrollbar_drag,
    update_scrollbar_drag_state, wheel_scroll_path,
};
use workspace_session::{demo_workspace, ensure_documents_tab};

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

type DrawerResizeDrag = HorizontalEditSession<f32>;

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

fn main() -> Result<(), Box<dyn Error>> {
    let recording = vesta_gpui_winit::UiTraceRecording::from_arguments(std::env::args_os())?;
    recording.finish_after_run(application_run::run())
}

#[cfg(test)]
mod tests;
