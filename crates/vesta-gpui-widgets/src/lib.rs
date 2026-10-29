//! Backend-neutral widget projections built from [`vesta_gpui::Element`] values.
//!
//! Widgets own interaction semantics and typed actions, but never mutate an application's
//! document or retained business state. Hosts rebuild these projections from their state and
//! dispatch the returned action after hit testing.

#![forbid(unsafe_code)]

mod slider;
pub use slider::{
    SliderIds, SliderKey, resize_slider, slider, slider_value_at, slider_value_for_key,
};
mod breadcrumb;
mod button_presentation;
mod caret_blink;
mod choice_navigation;
mod chrome;
mod collection_focus;
mod collection_navigation;
mod color_edit;
mod color_picker;
mod command_palette;
mod command_palette_state;
mod compact;
mod controls;
mod diff;
mod document_text;
mod drawer;
mod editable_combo;
mod inline_edit;
mod key_chord;
mod key_dispatch;
mod markdown;
mod menu_bar;
mod menu_popup;
mod modal;
mod notification_focus;
mod numeric_component;
mod password;
mod popover;
mod popup_lifecycle;
mod progress;
mod property;
mod property_set;
mod radio_group;
mod ramp;
mod rich_text;
mod scroll_input;
mod search_field;
mod selection;
mod state;
mod suggestion_state;
mod suggestion_view;
mod tab_switcher;
mod table_view;
mod temporary_editor;
mod text_editor;
mod text_label;
mod theme;
mod tile_grid;
mod title_bar;
mod toast;
mod toolbar;
mod tooltip;
mod tooltip_controller;
mod tree_controller;
mod tree_view;
mod value_edit;
mod vector_field;
mod virtual_list;

pub use breadcrumb::{BreadcrumbIds, BreadcrumbSegment, breadcrumb, breadcrumb_visible_indices};
pub use button_presentation::{ButtonPresentation, ButtonTone, button_with_presentation};
pub use caret_blink::CaretBlink;
pub use choice_navigation::{ChoiceBoundary, ChoiceKeyAction, choice_key_action};
pub use chrome::{
    CHROME_STATUS_CONTROL_WIDTH, CHROME_STATUS_HEIGHT, CHROME_TAB_HEIGHT, CHROME_TITLE_HEIGHT,
    CHROME_WINDOW_CONTROL_WIDTH, ChromeButtonRole, ChromeButtonStyle, StatusItem,
    StatusOverflowEntry, StatusPartition, chrome_button, partition_status_items, status_bar,
    status_label, status_progress, status_separator,
};
pub use collection_focus::{collection_focus_after_change, reveal_collection_row};
pub use collection_navigation::linear_collection_index;
pub use color_edit::{
    color_channel_step, format_rgba_hex, hsva_to_rgba, parse_rgba_hex, rgba_to_hsva,
};
pub use color_picker::{
    ColorPickerActions, ColorPickerIds, ColorPickerMode, ColorPickerView, color_picker,
};
pub use command_palette::{
    CommandPaletteEntry, CommandPaletteGeometry, CommandPaletteIds, CommandPaletteScrollIds,
    command_palette, command_palette_geometry, command_palette_scrolled,
};
pub use command_palette_state::{
    COMMAND_PALETTE_ROW_HEIGHT, CommandPaletteState, command_palette_matches,
};
pub use compact::{
    BusyIndicatorIds, CompactButtonKind, CompactSign, IconButtonContent, IconButtonPresentation,
    IconButtonSpec, StatusTone, busy_indicator, compact_sign, icon_button,
    icon_button_with_presentation, status_badge, toggle_button,
};
pub use controls::{
    ComboBoxIds, MenuItemIds, MenuItemView, NumberFieldIds, SPLITTER_HIT_SIZE, SplitterIds,
    TreeItemIds, action_for, combo_box, combo_option, combo_option_with_content,
    editable_number_field, list_item, list_item_from, list_item_with_content, menu_item,
    number_field, property_row, splitter, tree_item, tree_item_with_content,
};
pub use diff::{DiffLine, DiffLineKind, DiffViewMode, diff_view, parse_unified_diff};
pub use drawer::{
    SidebarDrawerIds, SidebarDrawerResizeKey, SidebarDrawerState, sidebar_drawer,
    sidebar_drawer_width_at, sidebar_drawer_width_for_key,
};
pub use editable_combo::{EditableComboIds, EditableSuggestion, editable_combo};
pub use inline_edit::{
    InlineEditIds, InlineEditKeyAction, inline_edit_key_action, inline_editable_label,
};
pub use key_chord::{KeyCaptureAction, KeyChordCapture, KeyChordSelectorView, key_chord_selector};
pub use key_dispatch::{KeyRouteContext, KeyRouteLayer, KeyRouteOutcome, route_key};
pub use markdown::{
    MarkdownAnchor, MarkdownAnchorError, MarkdownBlockId, MarkdownDocumentId, MarkdownError,
    MarkdownInteraction, MarkdownKeyIntent, MarkdownLink, MarkdownLinkIntent,
    MarkdownPointerResult, MarkdownProjectionError, MarkdownRevision, MarkdownSelection,
    MarkdownSurface, MarkdownView,
};
pub use menu_bar::{MenuBarEntry, MenuBarView, menu_bar, menu_bar_anchor, menu_bar_next};
pub use menu_popup::{
    MenuPickerColumns, MenuPickerContent, MenuPickerPopupSpec, MenuPopupSpec, menu_picker_popup,
    menu_picker_popup_with_content, menu_picker_popup_with_header, menu_popup, menu_submenu_anchor,
};
pub use modal::{
    ModalDialogSpec, ModalDismissPolicy, ModalIds, ModalStack, ModalStackError, modal_dialog,
    modal_dialog_with_labels,
};
pub use notification_focus::NotificationFocusState;
pub use numeric_component::NumericComponent;
pub use password::{PasswordFieldActions, PasswordFieldIds, PasswordFieldView, password_field};
pub use popover::{
    PopoverState, PopupKeyAction, popover, popover_trigger, popup_dismiss_for_hit, popup_key_action,
};
pub use popup_lifecycle::{PopupCloseReason, PopupDismissal};
pub use progress::{ProgressState, ProgressStateError, progress_bar_state};
pub use property::{
    PropertyRowIds, PropertyRowPresentation, PropertyRowState, PropertyValidation,
    ValidationSeverity, property_row_widget, property_row_with_presentation,
};
pub use property_set::{
    PropertySetIds, PropertySetPresentation, PropertySetSection, PropertySetSpec, property_set,
    property_set_with_presentation,
};
pub use radio_group::{RadioGroupError, RadioGroupOption, radio_group, radio_group_key_action};
pub use ramp::{
    RAMP_BAND_SEGMENTS, RAMP_POINT_HEIGHT, RAMP_POINT_TOP, RAMP_POINT_WIDTH, RAMP_TRACK_BORDER,
    RAMP_TRACK_HEIGHT, RampError, RampIds, RampInterpolation, RampPoint, RampSpec, ramp_color_at,
    ramp_editor, ramp_group_translation, ramp_handle_x, ramp_point_offset,
    ramp_position_for_pointer, ramp_position_from_offset, ramp_travel,
};
pub use rich_text::{RichTextRun, RichTextTone, rich_text, rich_text_paragraph};
pub use scroll_input::ScrollInputPath;
pub use search_field::{SearchFieldIds, search_field};
pub use selection::{ListSelectionState, SelectionModifiers};
pub use state::{
    MenuController, MenuKeyAction, PopupDirection, ScrollAreaState, ScrollAxis, ScrollRevealStep,
    ScrollRevealViewport, ScrollbarDrag, TabBarState, place_popup, scroll_reveal_path,
    scroll_wheel_path,
};
pub use suggestion_state::{
    SUGGESTION_ROW_HEIGHT, SuggestionError, SuggestionOption, SuggestionPhase, SuggestionRequest,
    SuggestionRequestId, SuggestionState,
};
pub use suggestion_view::{SuggestionViewIds, editable_combo_controlled};
pub use tab_switcher::{TabSwitcherEntry, TabSwitcherStyle, tab_switcher};
pub use table_view::{
    TableCell, TableCellContent, TableColumn, TableColumnContent, TableColumnResize,
    TableResizeKey, TableRow, TableRowContent, TableViewError, TableWindowIds,
    table_column_width_at, table_column_width_for_key, table_view, table_view_windowed,
    table_view_windowed_scrolled, table_view_windowed_scrolled_with_content,
    table_view_windowed_with_content, table_view_with_content,
};
pub use temporary_editor::{TemporaryEditorExchange, TemporaryEditorExit, TemporaryEditorState};
pub use text_editor::{text_area, text_field};
pub use text_label::{
    SelectableLabelError, TextContent, ellipsized_label, selectable_label, text_label,
};
pub use theme::{ThemeMetrics, ThemeMode, ThemePalette, ThemeTypography, WidgetTheme};
pub use tile_grid::{
    TileGridContent, TileGridDirection, TileGridError, TileGridGeometry, TileGridIds, TileGridItem,
    TileGridRange, tile_grid, tile_grid_next_index, tile_grid_next_index_in_direction,
    tile_grid_range, tile_grid_windowed_with_content, tile_grid_windowed_with_retained_content,
    tile_grid_with_content,
};
pub use title_bar::{TITLE_BAR_HEIGHT, TitleBarActions, TitleBarLabels, TitleBarState, title_bar};
pub use toast::{ToastId, ToastIds, ToastNotice, ToastQueue, ToastQueueError, toast_widget};
pub use toolbar::{ToolbarIds, ToolbarItem, toolbar, toolbar_visible_count};
pub use tooltip::{TooltipState, tooltip_popup};
pub use tooltip_controller::{TooltipController, TooltipFrame};
pub use tree_controller::{TreeViewController, TreeViewUpdate, tree_focus_after_change};
pub use tree_view::{
    TreeExpansionState, TreeKeyAction, TreeRowContentIds, TreeViewEntry, TreeViewError,
    TreeViewRowActions, TreeViewRowIds, TreeViewRowState, VisibleTree, tree_view_row,
    tree_view_row_from, tree_view_row_with_content, tree_view_windowed,
};
pub use value_edit::{
    DraftResolution, HorizontalEditSession, PointerEditError, PointerEditOutcome, ValueEditSession,
    resolve_value_draft,
};
pub use vector_field::{
    VectorComponentEdit, VectorComponentValue, VectorFieldEdit, VectorFieldIds, VectorFieldSpec,
    editable_vector_field, vector_field,
};
pub use virtual_list::{
    VariableListMetrics, VariableListRange, VirtualListError, VirtualListIds, VirtualListRange,
    variable_list_with_items, virtual_list, virtual_list_with_items,
};

use vesta_gpui::{
    Align, Axis, Color, Dimension, Element, ElementId, ElementKey, Insets, Justify, Point, Rect,
    Semantic, SemanticRole, Style, TextHAlign, TextVAlign,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WidgetIdentity {
    pub element: ElementId,
    pub key: ElementKey,
}

impl WidgetIdentity {
    #[must_use]
    pub fn new(element: ElementId, key: impl Into<String>) -> Self {
        Self {
            element,
            key: ElementKey::new(key),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlState {
    #[default]
    Idle,
    Hovered,
    Pressed,
    Disabled,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Widget<A> {
    pub element: Element,
    pub actions: Vec<(ElementId, A)>,
}

/// Arbitrary content with an explicit accessible label. Callers can build the widget
/// from typed objects or closures; the collection shell retains interaction semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct WidgetContent<A> {
    pub label: String,
    pub widget: Widget<A>,
}

impl<A> WidgetContent<A> {
    #[must_use]
    pub fn new(label: impl Into<String>, widget: Widget<A>) -> Self {
        Self {
            label: label.into(),
            widget,
        }
    }
}

/// A host-computed condition result. The UI never parses `disable_when` or
/// `hide_when`; VDA/Studio resolve those against author values and pass this
/// domain-neutral state to the composed widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidgetAvailability {
    pub visible: bool,
    pub enabled: bool,
}

impl Default for WidgetAvailability {
    fn default() -> Self {
        Self {
            visible: true,
            enabled: true,
        }
    }
}

impl<A> Widget<A> {
    #[must_use]
    pub fn from_element(element: Element) -> Self {
        Self {
            element,
            actions: Vec::new(),
        }
    }

    /// Attach a typed click action to this widget's root. Nested actions take
    /// precedence when the host resolves a hit through `ActionRoute`.
    #[must_use]
    pub fn on_click(mut self, action: A) -> Self {
        self.actions.push((self.element.id, action));
        self
    }

    /// Compose a child widget without losing its action identities.
    #[must_use]
    pub fn child(mut self, child: Self) -> Self {
        self.element.children.push(child.element);
        self.actions.extend(child.actions);
        self
    }

    /// Lift a reusable child action into a host or compound widget action type.
    #[must_use]
    pub fn map_action<B>(self, mut map: impl FnMut(A) -> B) -> Widget<B> {
        Widget {
            element: self.element,
            actions: self
                .actions
                .into_iter()
                .map(|(id, action)| (id, map(action)))
                .collect(),
        }
    }

    /// Apply an already-evaluated visibility/enablement condition to this
    /// complete subtree. `None` means the subtree is omitted from layout,
    /// paint, hit testing, and focus order. Disabled keeps layout/paint but
    /// prevents this subtree and its ancestor actions from activating.
    /// The control's visual disabled state remains a theme projection.
    #[must_use]
    pub fn conditioned(mut self, availability: WidgetAvailability) -> Option<Self> {
        if !availability.visible {
            return None;
        }
        if !availability.enabled {
            self.element.style.pointer_events = false;
            self.element.style.action_boundary = true;
            self.element.style.focusable = false;
            self.actions.clear();
        }
        Some(self)
    }

    #[must_use]
    pub fn action_at(&self, element: ElementId) -> Option<&A> {
        self.actions
            .iter()
            .find_map(|(candidate, action)| (*candidate == element).then_some(action))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollapsibleIds {
    pub root: WidgetIdentity,
    pub header: WidgetIdentity,
    pub disclosure: WidgetIdentity,
    pub label: WidgetIdentity,
}

pub(crate) const ICON_LINE_SIZE: f32 = 14.0;
pub(crate) const ICON_LINE_STROKE: f32 = 1.2;

pub(crate) fn disclosure_chevron_points(expanded: bool) -> [Point; 3] {
    if expanded {
        [
            Point::new(2.5, 5.5),
            Point::new(7.0, 10.5),
            Point::new(11.5, 5.5),
        ]
    } else {
        [
            Point::new(5.5, 2.5),
            Point::new(10.5, 7.0),
            Point::new(5.5, 11.5),
        ]
    }
}

fn disclosure_chevron_element(identity: WidgetIdentity, expanded: bool, color: Color) -> Element {
    Element::container(identity.element, identity.key).styled(
        Style::default()
            .size(Dimension::Px(ICON_LINE_SIZE), Dimension::Px(ICON_LINE_SIZE))
            .polyline(
                disclosure_chevron_points(expanded).to_vec(),
                ICON_LINE_STROKE,
            )
            .background(color)
            .pointer_events(false),
    )
}

/// Project an accordion panel from host-owned expansion state. Collapsed
/// content is absent from layout, hit testing, focus traversal, and actions.
#[must_use]
pub fn collapsible<A>(
    ids: CollapsibleIds,
    title: impl Into<String>,
    expanded: bool,
    content: Widget<A>,
    toggle_action: A,
    theme: WidgetTheme,
) -> Widget<A> {
    let title = title.into();
    let header = Widget::from_element(
        Element::container(ids.header.element, ids.header.key)
            .semantic(Semantic::new(SemanticRole::Button, &title).expanded(expanded))
            .styled(
                Style::default()
                    .size(
                        Dimension::Percent(1.0),
                        Dimension::Px(theme.metrics.control_height),
                    )
                    .axis(Axis::Row)
                    .align(Align::Center)
                    .gap(6.0)
                    .padding(Insets::symmetric(10.0, 0.0))
                    .background(theme.palette.surface)
                    .focusable(true),
            )
            .child(disclosure_chevron_element(
                ids.disclosure,
                expanded,
                theme.palette.text,
            ))
            .child(
                Element::text(ids.label.element, ids.label.key, title).styled(
                    Style::default()
                        .text(theme.typography.font_size, theme.palette.text)
                        .pointer_events(false),
                ),
            ),
    )
    .on_click(toggle_action);
    let mut panel = Widget::from_element(
        Element::container(ids.root.element, ids.root.key).styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Auto)
                .axis(Axis::Column)
                .gap(4.0)
                .border(1.0, theme.palette.border),
        ),
    )
    .child(header);
    if expanded {
        panel = panel.child(content);
    }
    panel
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpandableAreaIds {
    pub root: WidgetIdentity,
    pub header: WidgetIdentity,
    pub disclosure: WidgetIdentity,
    pub label: WidgetIdentity,
    pub body: WidgetIdentity,
}

/// A section surface projected from host-owned expansion state. The body is
/// removed entirely while collapsed, including its actions and focus targets.
#[must_use]
pub fn expandable_area<A>(
    ids: ExpandableAreaIds,
    title: impl Into<String>,
    expanded: bool,
    content: Widget<A>,
    toggle_action: A,
    theme: WidgetTheme,
) -> Widget<A> {
    let title = title.into();
    let header = Widget::from_element(
        Element::container(ids.header.element, ids.header.key)
            .semantic(Semantic::new(SemanticRole::Button, &title).expanded(expanded))
            .styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Px(34.0))
                    .axis(Axis::Row)
                    .align(Align::Center)
                    .gap(6.0)
                    .padding(Insets::symmetric(12.0, 0.0))
                    .background(theme.palette.surface)
                    .focusable(true),
            )
            .child(disclosure_chevron_element(
                ids.disclosure,
                expanded,
                theme.palette.text,
            ))
            .child(
                Element::text(ids.label.element, ids.label.key, title).styled(
                    Style::default()
                        .text(theme.typography.font_size, theme.palette.text)
                        .pointer_events(false),
                ),
            ),
    )
    .on_click(toggle_action);
    let mut area = Widget::from_element(
        Element::container(ids.root.element, ids.root.key).styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Auto)
                .axis(Axis::Column)
                .background(theme.palette.panel)
                .border(1.0, theme.palette.border)
                .radius(theme.metrics.radius),
        ),
    )
    .child(header);
    if expanded {
        area = area.child(
            Widget::from_element(
                Element::container(ids.body.element, ids.body.key).styled(
                    Style::default()
                        .size(Dimension::Percent(1.0), Dimension::Auto)
                        .axis(Axis::Column),
                ),
            )
            .child(content),
        );
    }
    area
}

#[must_use]
pub fn button<A>(
    root: WidgetIdentity,
    label: WidgetIdentity,
    text: impl Into<String>,
    action: A,
    state: ControlState,
    theme: WidgetTheme,
) -> Widget<A> {
    let text = text.into();
    let enabled = state != ControlState::Disabled;
    let background = theme.palette.control_background(state);
    let text_color = if enabled {
        theme.palette.text
    } else {
        theme.palette.text_disabled
    };
    let root_id = root.element;
    let element = Element::container(root.element, root.key)
        .semantic(Semantic::new(SemanticRole::Button, &text))
        .styled(
            Style::default()
                .size(Dimension::Auto, Dimension::Px(theme.metrics.control_height))
                .padding(Insets::symmetric(12.0, 0.0))
                .align(Align::Center)
                .justify(Justify::Center)
                .background(background)
                .border(1.0, theme.palette.border)
                .radius(theme.metrics.radius)
                .pointer_events(enabled)
                .focusable(enabled)
                .action_boundary(!enabled),
        )
        .child(
            Element::text(label.element, label.key, text).styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                    .text(theme.typography.font_size, text_color)
                    .text_align(TextHAlign::Center, TextVAlign::Center)
                    .pointer_events(false),
            ),
        );
    Widget {
        element,
        actions: enabled.then_some((root_id, action)).into_iter().collect(),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BinaryState {
    #[default]
    Off,
    On,
    Mixed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToggleIds {
    pub root: WidgetIdentity,
    pub track: WidgetIdentity,
    pub knob: WidgetIdentity,
    pub label: WidgetIdentity,
}

#[must_use]
pub fn toggle<A>(
    ids: ToggleIds,
    text: impl Into<String>,
    checked: bool,
    action: A,
    state: ControlState,
    theme: WidgetTheme,
) -> Widget<A> {
    let text = text.into();
    let ToggleIds {
        root,
        track,
        knob,
        label,
    } = ids;
    let enabled = state != ControlState::Disabled;
    let root_id = root.element;
    let track_color = if checked {
        theme.palette.accent
    } else {
        theme.palette.surface_pressed
    };
    let knob_x = if checked { 20.0 } else { 3.0 };
    let element = Element::container(root.element, root.key)
        .semantic(Semantic::new(SemanticRole::Switch, &text).checked(checked))
        .styled(
            Style::default()
                .size(Dimension::Auto, Dimension::Px(theme.metrics.control_height))
                .axis(vesta_gpui::Axis::Row)
                .align(Align::Center)
                .gap(8.0)
                .pointer_events(enabled)
                .focusable(enabled)
                .action_boundary(!enabled),
        )
        .child(
            Element::container(track.element, track.key)
                .styled(
                    Style::default()
                        .size(Dimension::Px(40.0), Dimension::Px(22.0))
                        .background(track_color)
                        .border(1.0, theme.palette.border)
                        .radius(11.0)
                        .pointer_events(false),
                )
                .child(
                    Element::container(knob.element, knob.key).styled(
                        Style::default()
                            .absolute(knob_x, 3.0)
                            .size(Dimension::Px(16.0), Dimension::Px(16.0))
                            .background(if enabled {
                                theme::control_mark_on_fill(track_color)
                            } else {
                                theme.palette.text_disabled
                            })
                            .radius(8.0)
                            .pointer_events(false),
                    ),
                ),
        )
        .child(
            Element::text(label.element, label.key, text).styled(
                Style::default()
                    .size(Dimension::Auto, Dimension::Px(theme.metrics.control_height))
                    .text(
                        theme.typography.font_size,
                        if enabled {
                            theme.palette.text
                        } else {
                            theme.palette.text_disabled
                        },
                    )
                    .pointer_events(false),
            ),
        );
    Widget {
        element,
        actions: enabled.then_some((root_id, action)).into_iter().collect(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckboxIds {
    pub root: WidgetIdentity,
    pub box_identity: WidgetIdentity,
    pub mark: WidgetIdentity,
    pub label: WidgetIdentity,
}

#[must_use]
pub fn checkbox<A>(
    ids: CheckboxIds,
    text: impl Into<String>,
    value: BinaryState,
    action: A,
    state: ControlState,
    theme: WidgetTheme,
) -> Widget<A> {
    let text = text.into();
    let CheckboxIds {
        root,
        box_identity,
        mark,
        label,
    } = ids;
    let enabled = state != ControlState::Disabled;
    let root_id = root.element;
    let checked = value != BinaryState::Off;
    let mark_size = match value {
        BinaryState::Off => 0.0,
        BinaryState::On => 10.0,
        BinaryState::Mixed => 10.0,
    };
    let mark_height = if value == BinaryState::Mixed {
        3.0
    } else {
        mark_size
    };
    let element = Element::container(root.element, root.key)
        .semantic(match value {
            BinaryState::Off => Semantic::new(SemanticRole::CheckBox, &text).checked(false),
            BinaryState::On => Semantic::new(SemanticRole::CheckBox, &text).checked(true),
            BinaryState::Mixed => Semantic::new(SemanticRole::CheckBox, &text).mixed(),
        })
        .styled(
            Style::default()
                .size(Dimension::Auto, Dimension::Px(theme.metrics.control_height))
                .axis(vesta_gpui::Axis::Row)
                .align(Align::Center)
                .gap(8.0)
                .pointer_events(enabled)
                .focusable(enabled)
                .action_boundary(!enabled),
        )
        .child(
            Element::container(box_identity.element, box_identity.key)
                .styled(
                    Style::default()
                        .size(Dimension::Px(18.0), Dimension::Px(18.0))
                        .align(Align::Center)
                        .justify(Justify::Center)
                        .background(if !enabled {
                            theme.palette.surface_disabled
                        } else if checked {
                            theme.palette.accent
                        } else {
                            theme.palette.field
                        })
                        .border(1.0, theme.palette.border)
                        .radius(4.0)
                        .pointer_events(false),
                )
                .child(
                    Element::container(mark.element, mark.key).styled(
                        Style::default()
                            .size(Dimension::Px(mark_size), Dimension::Px(mark_height))
                            .background(if enabled {
                                theme::control_mark_on_fill(theme.palette.accent)
                            } else {
                                theme.palette.text_disabled
                            })
                            .polyline(
                                match value {
                                    BinaryState::On => vec![
                                        Point::new(0.5, 5.0),
                                        Point::new(3.5, 8.0),
                                        Point::new(9.5, 1.5),
                                    ],
                                    BinaryState::Mixed => {
                                        vec![Point::new(0.5, 1.5), Point::new(9.5, 1.5)]
                                    }
                                    BinaryState::Off => Vec::new(),
                                },
                                1.6,
                            )
                            .pointer_events(false),
                    ),
                ),
        )
        .child(
            Element::text(label.element, label.key, text).styled(
                Style::default()
                    .size(Dimension::Auto, Dimension::Px(theme.metrics.control_height))
                    .text(
                        theme.typography.font_size,
                        if enabled {
                            theme.palette.text
                        } else {
                            theme.palette.text_disabled
                        },
                    )
                    .pointer_events(false),
            ),
        );
    Widget {
        element,
        actions: enabled.then_some((root_id, action)).into_iter().collect(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RadioIds {
    pub root: WidgetIdentity,
    pub ring: WidgetIdentity,
    pub dot: WidgetIdentity,
    pub label: WidgetIdentity,
}

#[must_use]
pub fn radio<A>(
    ids: RadioIds,
    text: impl Into<String>,
    selected: bool,
    action: A,
    state: ControlState,
    theme: WidgetTheme,
) -> Widget<A> {
    let text = text.into();
    let RadioIds {
        root,
        ring,
        dot,
        label,
    } = ids;
    let enabled = state != ControlState::Disabled;
    let root_id = root.element;
    let element = Element::container(root.element, root.key)
        .semantic(Semantic::new(SemanticRole::RadioButton, &text).checked(selected))
        .styled(
            Style::default()
                .size(Dimension::Auto, Dimension::Px(theme.metrics.control_height))
                .axis(vesta_gpui::Axis::Row)
                .align(Align::Center)
                .gap(8.0)
                .pointer_events(enabled)
                .focusable(enabled)
                .action_boundary(!enabled),
        )
        .child(
            Element::container(ring.element, ring.key)
                .styled(
                    Style::default()
                        .size(Dimension::Px(18.0), Dimension::Px(18.0))
                        .align(Align::Center)
                        .justify(Justify::Center)
                        .background(if enabled {
                            theme.palette.field
                        } else {
                            theme.palette.surface_disabled
                        })
                        .border(
                            if selected { 2.0 } else { 1.0 },
                            if !enabled {
                                theme.palette.text_disabled
                            } else if selected {
                                theme.palette.accent
                            } else {
                                theme.palette.border
                            },
                        )
                        .radius(9.0)
                        .pointer_events(false),
                )
                .child(
                    Element::container(dot.element, dot.key).styled(
                        Style::default()
                            .size(
                                Dimension::Px(if selected { 8.0 } else { 0.0 }),
                                Dimension::Px(if selected { 8.0 } else { 0.0 }),
                            )
                            .background(if enabled {
                                theme.palette.accent
                            } else {
                                theme.palette.text_disabled
                            })
                            .radius(4.0)
                            .pointer_events(false),
                    ),
                ),
        )
        .child(
            Element::text(label.element, label.key, text).styled(
                Style::default()
                    .size(Dimension::Auto, Dimension::Px(theme.metrics.control_height))
                    .text(
                        theme.typography.font_size,
                        if enabled {
                            theme.palette.text
                        } else {
                            theme.palette.text_disabled
                        },
                    )
                    .pointer_events(false),
            ),
        );
    Widget {
        element,
        actions: enabled.then_some((root_id, action)).into_iter().collect(),
    }
}

#[must_use]
pub fn progress_bar(
    root: WidgetIdentity,
    fill: WidgetIdentity,
    label: WidgetIdentity,
    value: f32,
    theme: WidgetTheme,
) -> Element {
    let value = value.clamp(0.0, 1.0);
    Element::container(root.element, root.key)
        .semantic(
            Semantic::new(SemanticRole::ProgressIndicator, "Progress")
                .value(format!("{:.0}%", value * 100.0)),
        )
        .styled(
            Style::default()
                .size(
                    Dimension::Px(220.0),
                    Dimension::Px(theme.metrics.control_height),
                )
                .background(theme.palette.field)
                .border(1.0, theme.palette.border)
                .radius(theme.metrics.radius)
                .pointer_events(false),
        )
        .child(
            Element::container(fill.element, fill.key).styled(
                Style::default()
                    .absolute(0.0, 0.0)
                    .size(Dimension::Percent(value), Dimension::Percent(1.0))
                    .background(theme.palette.progress_fill)
                    .radius(theme.metrics.radius)
                    .pointer_events(false),
            ),
        )
        .child(
            Element::text(label.element, label.key, format!("{:.0}%", value * 100.0)).styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                    .text(theme.typography.small_font_size, theme.palette.text)
                    .text_align(TextHAlign::Center, TextVAlign::Center)
                    .pointer_events(false),
            ),
        )
}

#[must_use]
pub fn tab<A>(
    root: WidgetIdentity,
    label: WidgetIdentity,
    text: impl Into<String>,
    selected: bool,
    action: A,
    state: ControlState,
    theme: WidgetTheme,
) -> Widget<A> {
    let text = text.into();
    let enabled = state != ControlState::Disabled;
    let root_id = root.element;
    let background = if selected {
        theme.palette.field
    } else {
        theme.palette.panel
    };
    let foreground = if enabled {
        theme.palette.text
    } else {
        theme.palette.text_disabled
    };
    let element = Element::container(root.element, root.key)
        .semantic(Semantic::new(SemanticRole::Tab, &text).selected(selected))
        .styled(
            Style::default()
                .size(Dimension::Auto, Dimension::Px(CHROME_TAB_HEIGHT))
                .padding(Insets::symmetric(14.0, 0.0))
                .align(Align::Center)
                .justify(Justify::Center)
                .background(background)
                .border(0.0, theme.palette.border)
                .radius(0.0)
                .pointer_events(enabled)
                .focusable(enabled)
                .action_boundary(!enabled),
        )
        .child(
            Element::text(label.element, label.key, text).styled(
                Style::default()
                    .size(Dimension::Auto, Dimension::Percent(1.0))
                    .text(theme.typography.font_size, foreground)
                    .text_align(TextHAlign::Center, TextVAlign::Center)
                    .pointer_events(false),
            ),
        );
    Widget {
        element,
        actions: enabled.then_some((root_id, action)).into_iter().collect(),
    }
}

#[cfg(test)]
mod tests;
