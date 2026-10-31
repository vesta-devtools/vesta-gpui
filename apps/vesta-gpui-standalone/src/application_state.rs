//! Initial host state; window and input ownership remain with App.
use super::*;

impl App {
    pub(super) fn new() -> Self {
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
            frame_reflow_probe: None,
            ui_query: ui_query::Store::new(
                std::env::args().any(|arg| ui_query::stdio_argument(&arg).is_some()),
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
            input_layout_ready: false,
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
                viewport_demo::scene()
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
}

pub(super) struct DemoState {
    pub(super) locale: UiLocale,
    pub(super) long_document: bool,
    pub(super) viewport3d_available: bool,
    pub(super) toggle: bool,
    pub(super) compact_toggle: bool,
    pub(super) busy: bool,
    pub(super) busy_phase: u8,
    pub(super) drawer_open: bool,
    pub(super) drawer_width: f32,
    pub(super) drawer_tab: u8,
    pub(super) checkbox: BinaryState,
    pub(super) radio: u8,
    pub(super) slider: f32,
    pub(super) slider_enabled: bool,
    pub(super) number: i32,
    pub(super) vector: [Option<f32>; 3],
    pub(super) key_capture: KeyChordCapture,
    pub(super) key_chord: Option<KeyChord>,
    pub(super) toast_queue: ToastQueue<DemoAction>,
    pub(super) next_toast_id: u32,
    pub(super) table_sort_column: u8,
    pub(super) table_note: String,
    pub(super) table_sort_descending: bool,
    pub(super) table_selected: u8,
    pub(super) table_selected_cell: Option<(u8, u8)>,
    pub(super) virtual_table_selected: usize,
    pub(super) virtual_table_scroll: ScrollAreaState,
    pub(super) table_column_fractions: [f32; 3],
    pub(super) tile_selected: usize,
    pub(super) ramp: RampDemo,
    pub(super) status_overflow_open: bool,
    pub(super) combo: PopoverState,
    pub(super) combo_selected: u8,
    pub(super) suggestions: vesta_gpui_widgets::SuggestionState<u16>,
    pub(super) suggestion_serial: u64,
    pub(super) suggestion_dismissed: Option<(InputTarget, String)>,
    pub(super) suggestion_scroll_node: Option<vesta_gpui::LayoutNode>,
    pub(super) suggestion_drag: Option<vesta_gpui_widgets::ScrollbarDrag>,
    pub(super) dialog_stack: ModalStack<u8>,
    pub(super) inline_label: String,
    pub(super) inline_session: vesta_gpui_widgets::TemporaryEditorState<TextInputState>,
    pub(super) palette_session: vesta_gpui_widgets::TemporaryEditorState<TextInputState>,
    pub(super) palette: vesta_gpui_widgets::CommandPaletteState<u8>,
    pub(super) palette_scroll_node: Option<vesta_gpui::LayoutNode>,
    pub(super) palette_drag: Option<ScrollbarDrag>,
    pub(super) breadcrumb_depth: u8,
    pub(super) breadcrumb_expanded: bool,
    pub(super) toolbar_overflow_open: bool,
    pub(super) color_committed: [u8; 4],
    pub(super) color_draft: [u8; 4],
    pub(super) color_mode: ColorPickerMode,
    pub(super) color_open: bool,
    pub(super) color_session: Option<ValueEditSession<[u8; 4]>>,
    pub(super) color_target: ColorEditTarget,
    pub(super) password_revealed: bool,
    pub(super) popover: PopoverState,
    pub(super) menu: MenuController,
    pub(super) menu_origin: MenuOrigin,
    pub(super) menu_context: Option<MenuContext>,
    pub(super) menu_anchor: Point,
    pub(super) tab_menu_item: Option<ItemId>,
    pub(super) selected_tab: u8,
    pub(super) selected_list: u8,
    pub(super) virtual_ids: Vec<usize>,
    pub(super) virtual_metrics: Option<vesta_gpui_widgets::VariableListMetrics>,
    pub(super) virtual_selection: ListSelectionState<usize>,
    pub(super) tree_entries: Vec<TreeViewEntry<usize>>,
    pub(super) tree_expansion: TreeExpansionState<usize>,
    pub(super) tree_selection: ListSelectionState<usize>,
    pub(super) tree_expanded: bool,
    pub(super) accordion_open: Option<u8>,
    pub(super) collapsed_gallery_sections: HashSet<GallerySectionId>,
    pub(super) nested_scroll: ScrollAreaState,
    pub(super) horizontal_scroll: ScrollAreaState,
    pub(super) horizontal_flow: FlowDirection,
    pub(super) virtual_scroll: ScrollAreaState,
    pub(super) tree_scroll: ScrollAreaState,
    pub(super) page_scroll: HashMap<ItemId, ScrollAreaState>,
    pub(super) tab_scroll: HashMap<PaneId, ScrollAreaState>,
    pub(super) pane_widths: HashMap<PaneId, f32>,
    pub(super) workspace: WorkspaceModel,
    pub(super) pane_history: HashMap<PaneId, PaneHistory>,
    pub(super) zoomed_panes: HashMap<ContainerRef, PaneId>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct PaneHistory {
    entries: Vec<ItemId>,
    pub(super) cursor: usize,
}

impl PaneHistory {
    pub(super) fn visit(&mut self, item: ItemId) {
        if self.entries.get(self.cursor) == Some(&item) {
            return;
        }
        self.entries.truncate(self.cursor.saturating_add(1));
        self.entries.push(item);
        self.cursor = self.entries.len() - 1;
    }

    pub(super) fn target(&self, direction: PaneNavigationDirection) -> Option<(usize, ItemId)> {
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
    pub(super) fn sync_pane_widths(
        &mut self,
        widths: impl IntoIterator<Item = (PaneId, f32)>,
    ) -> bool {
        let mut changed = false;
        for (pane, width) in widths {
            let previous = self.pane_widths.insert(pane, width);
            changed |= previous.is_none_or(|previous| (previous - width).abs() > 1.0);
        }
        changed
    }

    pub(super) fn sync_workspace_ui(&mut self) {
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
