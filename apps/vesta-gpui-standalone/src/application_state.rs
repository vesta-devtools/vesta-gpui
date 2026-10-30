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
}
