//! Gallery field identities and shared window text event routing.
use super::*;

/// A window's derived wrap constraint; buffer, composition and history stay shared.
#[derive(Default)]
pub(super) struct AreaWrapLayout {
    width: Option<f32>,
}

impl AreaWrapLayout {
    pub(super) fn prepare(&self, state: &mut TextInputState, context: &mut TextInputContext) {
        if let Some(width) = self.width {
            state.set_wrap_width(context, width);
        }
    }

    pub(super) fn sync(
        &mut self,
        state: &mut TextInputState,
        context: &mut TextInputContext,
        gallery: ElementScope,
        layout: &LayoutTree,
    ) -> bool {
        self.width = layout
            .node(gallery.element(TEXT_AREA))
            .map(|node| text_input_wrap_width(node.rect));
        self.width
            .is_some_and(|width| state.set_wrap_width(context, width))
    }
}

/// Input coordinates and wheel range refer to the last presented source viewport.
pub(super) fn restore_area_view_layout(
    state: &mut TextInputState,
    context: &mut TextInputContext,
    view: vesta_gpui::TextInputViewport,
) {
    state.set_wrap_width(context, text_input_wrap_width(view.bounds));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{
        activate_gallery_page, install_action_route, install_action_route_with_viewport,
    };
    use std::{sync::Arc, time::Duration};

    mod narrow_vector;
    mod numeric_steps;
    mod swatch;

    #[test]
    fn pointer_click_modes_reach_the_shared_editor_without_splitting_window_ownership() {
        let mut app = App::new();
        activate_gallery_page(&mut app, "gallery.inputs");
        install_action_route(&mut app);
        let font =
            include_bytes!("../../../crates/vesta-gpui/tests/fixtures/MaterialIcons-Regular.ttf");
        assert!(app.text_context.register_font(Arc::new(font.to_vec())));
        app.text_state = TextInputState::new(13.0, "Material Icons");
        let origin = Point::new(12.0, 112.0);
        app.text_view = Some(vesta_gpui::TextInputViewport {
            bounds: Rect::new(4.0, 108.0, 360.0, 28.0),
            clip: Rect::new(4.0, 108.0, 360.0, 28.0),
            scroll_x: 0.0,
            scroll_y: 0.0,
            origin,
            caret: None,
        });
        for (source, interior, expected) in [
            ("alpha beta", "alpha b".len(), "beta"),
            ("L مرحبا R", "L مر".len(), "مرحبا"),
        ] {
            app.clear_text_session();
            app.text_state
                .reset(&mut app.text_context, source)
                .expect("source");
            let cursor = app
                .text_state
                .snapshot()
                .layout_cursor(interior)
                .expect("source cursor");
            let bounds = cursor.bounds();
            let point = Point::new(
                origin.x + bounds.x,
                origin.y + bounds.y + bounds.height * 0.5,
            );
            let now = Instant::now();
            app.begin_text_selection_at(STANDALONE_WINDOW, TextFieldKind::Main, Some(point), now);
            app.input_ownership
                .release_pointer(STANDALONE_WINDOW, PointerButton::Primary);
            app.begin_text_selection_at(
                STANDALONE_WINDOW,
                TextFieldKind::Main,
                Some(point),
                now + Duration::from_millis(100),
            );
            assert_eq!(app.text_state.selected_text(), Some(expected), "{source}");
            assert!(!app.update_window_text_selection(STANDALONE_WINDOW, point));
            assert_eq!(
                app.text_state.selected_text(),
                Some(expected),
                "stationary move keeps word"
            );
            let target = app
                .input_ownership
                .capture()
                .expect("primary capture")
                .target;
            app.input_ownership
                .capture_pointer(target, PointerButton::Secondary);
            assert!(!app.update_window_text_selection(STANDALONE_WINDOW, point));
            assert_eq!(app.text_state.selected_text(), Some(expected));
            app.input_ownership.capture_pointer(
                InputTarget {
                    window: UiWindowId(999),
                    ..target
                },
                PointerButton::Primary,
            );
            assert!(!app.update_window_text_selection(STANDALONE_WINDOW, point));
            assert_eq!(app.text_state.selected_text(), Some(expected));
            app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::Main, None);
            assert!(
                app.input_ownership.capture().is_none(),
                "programmatic focus is not a press"
            );
        }
        app.handle_window_text_event(
            STANDALONE_WINDOW,
            &WindowEvent::Ime(winit::event::Ime::Enabled),
        );
        app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::Main, None);
        app.handle_window_text_event(
            STANDALONE_WINDOW,
            &WindowEvent::Ime(winit::event::Ime::Preedit("你".into(), None)),
        );
        assert!(
            app.text_state.is_composing(),
            "programmatic focus keeps the existing IME session"
        );

        // Opening an editor replaces capture even when the logical field is the
        // same. A pointer preview must be restored before draft exit can commit it.
        app.text_state.cancel_compose(&mut app.text_context);
        for next in [TextFieldKind::Main, TextFieldKind::Number] {
            app.set_text_focus(Some(TextFieldKind::Number));
            app.demo.number = 128;
            let target = InputTarget {
                window: STANDALONE_WINDOW,
                element: TextFieldKind::Number.element(app.scopes.gallery),
            };
            app.number_scrub =
                Some(NumberScrub::new(target, 0.0, 128, 0.0).expect("number preview"));
            app.input_ownership
                .capture_pointer(target, PointerButton::Primary);
            app.demo.number = 400;
            app.number_state
                .reset(&mut app.text_context, "400")
                .expect("preview draft");
            app.begin_text_selection(STANDALONE_WINDOW, next, None);
            assert_eq!(
                app.demo.number, 128,
                "text entry must restore the captured preview before draft exit"
            );
            assert_eq!(app.number_state.text(), "128");
            assert!(app.number_scrub.is_none());
            assert!(app.input_ownership.capture().is_none());
            assert!(!app.finish_number_scrub(), "obsolete release cannot commit");
            assert_eq!(app.text_session_field(STANDALONE_WINDOW), Some(next));
        }

        // This is the typed host transition, not an OS-window simulation. It must
        // distinguish a pointer preview from an ordinary draft in the same field.
        app.set_text_focus(Some(TextFieldKind::Number));
        let target = InputTarget {
            window: STANDALONE_WINDOW,
            element: TextFieldKind::Number.element(app.scopes.gallery),
        };
        app.number_scrub = Some(NumberScrub::new(target, 0.0, 128, 0.0).expect("source preview"));
        app.input_ownership
            .capture_pointer(target, PointerButton::Primary);
        app.demo.number = 400;
        app.number_state
            .reset(&mut app.text_context, "400")
            .expect("source draft");
        let floating = UiWindowId(0x2000_0001);
        app.transition_window_text_session(floating, Some(TextFieldKind::Number));
        assert_eq!(
            app.demo.number, 128,
            "foreign caret transfer cancels the source preview"
        );
        assert_eq!(app.number_state.text(), "128");
        assert!(app.number_scrub.is_none() && app.input_ownership.capture().is_none());
        app.number_state
            .reset(&mut app.text_context, "unfinished")
            .expect("ordinary draft");
        app.transition_window_text_session(STANDALONE_WINDOW, Some(TextFieldKind::Number));
        assert_eq!(
            app.number_state.text(),
            "unfinished",
            "an owner-only transfer preserves the ordinary draft"
        );
        assert_eq!(app.demo.number, 128);
        app.begin_text_selection(UiWindowId(999), TextFieldKind::Main, None);
        assert_eq!(
            app.number_state.text(),
            "unfinished",
            "an invalid destination cannot exit the current draft"
        );
        app.number_state
            .reset(&mut app.text_context, "42")
            .expect("ordinary valid draft");
        app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::Main, None);
        assert_eq!(
            app.demo.number, 42,
            "ordinary field exit still commits a valid draft"
        );
        typed_numeric_terminals_keep_components_independent();
        color_terminals_keep_the_owned_composite_snapshot();
        ramp_hex_keeps_the_owned_point_and_single_history();
        numeric_source_loss_preserves_the_accepted_model();
        native_capture::qualify_numeric_loss();
    }

    fn typed_numeric_terminals_keep_components_independent() {
        narrow_vector::qualify();
        numeric_steps::qualify();
        use vesta_gpui::{TextEditKind, TextInputAction};
        let mut app = App::new();
        activate_gallery_page(&mut app, "gallery.inputs");
        install_action_route(&mut app);
        for (draft, expected) in [
            (" 42 ", 42),
            ("2147483648", 42),
            ("bad", 42),
            ("10000", 999),
            ("-10000", -999),
            ("-999", -999),
        ] {
            app.set_text_focus(Some(TextFieldKind::Number));
            app.number_state
                .reset(&mut app.text_context, draft)
                .expect("number draft");
            app.set_text_focus(None);
            assert_eq!(app.demo.number, expected, "number terminal {draft}");
            assert_eq!(app.number_state.text(), expected.to_string());
            assert_eq!(app.demo.vector, [Some(1.0), None, Some(3.0)]);
        }
        for draft in ["", "NaN", "inf", "1e999"] {
            app.set_text_focus(Some(TextFieldKind::Vector(1)));
            assert_eq!(app.vector_states[1].text(), "");
            app.vector_states[1]
                .reset(&mut app.text_context, draft)
                .expect("mixed draft");
            app.set_text_focus(None);
            assert_eq!(
                app.demo.vector,
                [Some(1.0), None, Some(3.0)],
                "invalid mixed component {draft}"
            );
        }
        app.set_text_focus(Some(TextFieldKind::Vector(1)));
        app.vector_states[1]
            .insert(&mut app.text_context, "17", TextEditKind::Typing)
            .expect("mixed edit");
        assert!(app.vector_states[1].act(&mut app.text_context, TextInputAction::Undo));
        assert_eq!(app.vector_states[1].text(), "");
        assert!(app.vector_states[1].act(&mut app.text_context, TextInputAction::Redo));
        assert_eq!(app.vector_states[1].text(), "17");
        assert!(
            app.demo.vector[1].is_none(),
            "draft history cannot materialize the mixed model"
        );
        app.set_text_focus(None);
        assert_eq!(app.demo.vector, [Some(1.0), Some(17.0), Some(3.0)]);
        for (draft, expected) in [(" 2.5 ", 2.5), ("1e3", 999.0), ("NaN", 999.0), ("-", 999.0)] {
            app.set_text_focus(Some(TextFieldKind::Vector(1)));
            app.vector_states[1]
                .reset(&mut app.text_context, draft)
                .expect("finite axis draft");
            app.set_text_focus(None);
            assert_eq!(app.demo.vector, [Some(1.0), Some(expected), Some(3.0)]);
            assert_eq!(app.demo.number, -999);
        }
    }

    fn color_terminals_keep_the_owned_composite_snapshot() {
        let mut app = App::new();
        activate_gallery_page(&mut app, "gallery.inputs");
        install_action_route(&mut app);
        let original = app.demo.color_committed;
        app.apply_window_action(STANDALONE_WINDOW, DemoAction::ToggleColorPicker);
        install_action_route(&mut app);
        let hex = TextFieldKind::ColorHex.element(app.scopes.gallery);
        assert!(app.ui_actions.focus_order().contains(&hex));
        app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::ColorHex, None);
        app.color_hex_state
            .reset(&mut app.text_context, "#11223344")
            .expect("hex draft");
        assert!(app.commit_color_hex_draft());
        assert_eq!(app.demo.color_draft, [17, 34, 51, 68]);
        assert_eq!(app.demo.color_committed, original);
        app.color_hex_state
            .reset(&mut app.text_context, "#bad-hex")
            .expect("invalid hex");
        assert!(!app.commit_color_hex_draft());
        assert_eq!(app.demo.color_draft, [17, 34, 51, 68]);
        app.apply_window_action(UiWindowId(999), DemoAction::ApplyColor);
        assert!(app.demo.color_open);
        assert_eq!(app.demo.color_committed, original);
        app.apply_window_action(STANDALONE_WINDOW, DemoAction::CancelColor);
        assert_eq!(app.demo.color_committed, original);
        assert!(!app.demo.color_open && app.demo.color_session.is_none());
        app.apply_window_action(STANDALONE_WINDOW, DemoAction::ApplyColor);
        assert_eq!(
            app.demo.color_committed, original,
            "late apply cannot revive a cancelled session"
        );

        app.apply_window_action(STANDALONE_WINDOW, DemoAction::ToggleColorPicker);
        install_action_route(&mut app);
        app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::ColorHex, None);
        app.color_hex_state
            .reset(&mut app.text_context, "#10203040")
            .expect("accepted hex");
        assert!(app.commit_color_hex_draft());
        app.apply_window_action(STANDALONE_WINDOW, DemoAction::StepColor(1, 8));
        assert_eq!(app.demo.color_draft, [16, 40, 48, 64]);
        assert_eq!(app.demo.color_committed, original);
        app.apply_window_action(STANDALONE_WINDOW, DemoAction::ApplyColor);
        assert_eq!(app.demo.color_committed, [16, 40, 48, 64]);
        app.apply_window_action(STANDALONE_WINDOW, DemoAction::ApplyColor);
        assert_eq!(app.demo.color_committed, [16, 40, 48, 64]);

        app.apply_window_action(STANDALONE_WINDOW, DemoAction::ToggleColorPicker);
        install_action_route(&mut app);
        app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::ColorHex, None);
        app.color_hex_state
            .reset(&mut app.text_context, "#FFFFFFFF")
            .expect("unaccepted hex");
        let root = app.scopes.gallery.element(COLOR_PICKER_ROOT);
        app.retire_window_subtree_input(STANDALONE_WINDOW, root);
        assert!(!app.demo.color_open && app.demo.color_session.is_none());
        assert!(app.text_session.is_none());
        assert_eq!(app.demo.color_committed, [16, 40, 48, 64]);
        swatch::mode_terminals_preserve_the_owned_draft();
    }

    fn ramp_hex_keeps_the_owned_point_and_single_history() {
        for mode in [ColorPickerMode::Rgb, ColorPickerMode::Hsv] {
            let mut app = App::new();
            activate_gallery_page(&mut app, "gallery.inputs");
            let point = app.demo.ramp.primary;
            let before = app.demo.ramp.snapshot();
            let swatch = app.demo.color_committed;
            app.apply_window_action(STANDALONE_WINDOW, DemoAction::RampEditColor);
            app.apply_window_action(STANDALONE_WINDOW, DemoAction::SelectColorMode(mode));
            install_action_route(&mut app);
            let hex = TextFieldKind::ColorHex.element(app.scopes.gallery);
            assert!(
                app.ui_actions
                    .is_descendant_of(hex, app.scopes.gallery.element(RAMP_ROOT + 10))
            );
            assert!(
                !app.ui_actions
                    .is_descendant_of(hex, app.scopes.gallery.element(COLOR_PICKER_ROOT))
            );
            app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::ColorHex, None);
            app.color_hex_state.act(
                &mut app.text_context,
                vesta_gpui::TextInputAction::SelectAll,
            );
            assert!(app.handle_window_text_event(
                STANDALONE_WINDOW,
                &WindowEvent::Ime(winit::event::Ime::Enabled)
            ));
            assert!(app.handle_window_text_event(
                STANDALONE_WINDOW,
                &WindowEvent::Ime(winit::event::Ime::Commit("#11223344".into()))
            ));
            assert_eq!(
                app.demo.ramp.snapshot(),
                before,
                "draft is not a ramp commit"
            );
            assert_eq!(app.demo.color_committed, swatch);
            assert!(app.commit_color_hex_draft());
            assert_eq!(app.demo.color_draft, [17, 34, 51, 68]);
            assert_eq!(app.demo.ramp.snapshot(), before);
            // Shared handler after admission, not an unknown-window adapter
            // rejection. This source token does not create a native Host.
            app.action_window = UiWindowId(999);
            app.apply_demo_action(DemoAction::CancelColor);
            assert!(
                app.demo.color_open,
                "foreign CancelColor must preserve the owned Ramp editor"
            );
            assert_eq!(app.demo.color_target, ColorEditTarget::RampPoint(point));
            assert_eq!(app.demo.color_draft, [17, 34, 51, 68]);
            app.apply_demo_action(DemoAction::ApplyColor);
            assert!(app.demo.color_open, "foreign Apply preserves the editor");
            assert_eq!(app.demo.ramp.snapshot(), before);
            app.action_window = STANDALONE_WINDOW;
            let root = install_action_route_with_viewport(&mut app, Size::new(1280.0, 2400.0));
            let layout = LayoutEngine
                .compute(&root, Viewport::new(1280.0, 2400.0, 1.0))
                .expect("ramp pointer layout");
            app.hit_test = HitTestTree::build(&root, &layout);
            app.ramp_track_rect = Some(
                layout
                    .node(app.scopes.gallery.element(RAMP_ROOT + 1))
                    .expect("ramp track")
                    .rect,
            );
            let handle = app.scopes.gallery.element(RAMP_ROOT + 100 + point * 2);
            let rect = layout.node(handle).expect("owned ramp handle").rect;
            app.cursor = Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5);
            assert_eq!(app.hit_test.hit(app.cursor), Some(handle));
            app.pointer_press();
            assert!(app.ramp_drag.is_some() && app.demo.color_open);
            app.cursor.x += 30.0;
            assert!(app.update_ramp_drag());
            let preview = app.demo.ramp.snapshot();
            let capture = app.input_ownership.capture();
            let preview_hex = app.color_hex_state.text().to_owned();
            app.action_window = UiWindowId(999);
            for foreign in [
                DemoAction::ApplyColor,
                DemoAction::CancelColor,
                DemoAction::StepColor(0, 8),
                DemoAction::PickColor,
                DemoAction::SelectColorMode(if mode == ColorPickerMode::Rgb {
                    ColorPickerMode::Hsv
                } else {
                    ColorPickerMode::Rgb
                }),
            ] {
                app.apply_demo_action(foreign);
                assert!(
                    app.ramp_drag.is_some(),
                    "foreign ApplyColor must preserve the active ramp capture: {foreign:?}"
                );
                assert_eq!(app.input_ownership.capture(), capture);
                assert_eq!(app.demo.ramp.snapshot(), preview);
                assert_eq!(app.demo.color_mode, mode);
                assert_eq!(app.color_hex_state.text(), preview_hex);
                assert_eq!(app.demo.color_draft, [17, 34, 51, 68]);
            }
            app.action_window = STANDALONE_WINDOW;
            assert!(app.cancel_ramp_drag());
            assert_eq!(app.demo.ramp.snapshot(), before);
            app.apply_window_action(STANDALONE_WINDOW, DemoAction::RampSelect(3));
            install_action_route(&mut app);
            let commit_before = app.demo.ramp.snapshot();
            assert_eq!(app.demo.color_target, ColorEditTarget::RampPoint(point));
            app.apply_window_action(STANDALONE_WINDOW, DemoAction::ApplyColor);
            assert_eq!(
                app.demo.ramp.point(point).expect("edited point").rgba,
                [17, 34, 51, 68]
            );
            assert_eq!(app.demo.ramp.undo.len(), 1);
            assert!(app.demo.ramp.redo.is_empty());
            assert_eq!(app.demo.color_committed, swatch);
            app.apply_window_action(STANDALONE_WINDOW, DemoAction::ApplyColor);
            assert_eq!(
                app.demo.ramp.undo.len(),
                1,
                "late Apply cannot duplicate history"
            );
            app.apply_window_action(STANDALONE_WINDOW, DemoAction::RampUndo);
            assert_eq!(app.demo.ramp.snapshot(), commit_before);
            app.apply_window_action(STANDALONE_WINDOW, DemoAction::RampRedo);
            assert_eq!(
                app.demo.ramp.point(point).expect("redo point").rgba,
                [17, 34, 51, 68]
            );
            let accepted = app.demo.ramp.snapshot();
            let history = app.demo.ramp.undo.len();
            app.apply_window_action(STANDALONE_WINDOW, DemoAction::RampEditColor);
            install_action_route(&mut app);
            app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::ColorHex, None);
            app.color_hex_state
                .reset(&mut app.text_context, "#FFFFFFFF")
                .expect("cancel draft");
            app.apply_window_action(STANDALONE_WINDOW, DemoAction::CancelColor);
            assert_eq!(app.demo.ramp.snapshot(), accepted);
            assert_eq!(app.demo.ramp.undo.len(), history);
            assert_eq!(app.demo.color_committed, swatch);
            assert!(app.text_session.is_none() && app.demo.color_session.is_none());
            install_action_route(&mut app);
            assert_eq!(
                app.window_focus(STANDALONE_WINDOW),
                Some(app.scopes.gallery.element(RAMP_ROOT + 2))
            );
            app.apply_window_action(STANDALONE_WINDOW, DemoAction::ToggleColorPicker);
            install_action_route(&mut app);
            app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::ColorHex, None);
            assert_eq!(app.color_hex_state.text(), format_rgba_hex(swatch));
            assert_eq!(
                app.demo.ramp.snapshot(),
                accepted,
                "shared Hex identity does not leak old target data"
            );
            app.apply_window_action(STANDALONE_WINDOW, DemoAction::CancelColor);
            eprintln!(
                "ramp-hex-terminals mode={mode:?} point={point} undo={} swatch_unchanged=true",
                app.demo.ramp.undo.len()
            );
        }
    }

    fn numeric_source_loss_preserves_the_accepted_model() {
        let mut failures = Vec::new();
        for reason in [
            PopupCloseReason::OwnerRemoved,
            PopupCloseReason::WindowUnavailable,
        ] {
            for field in [
                TextFieldKind::Number,
                TextFieldKind::Vector(0),
                TextFieldKind::Vector(1),
            ] {
                let mut app = App::new();
                activate_gallery_page(&mut app, "gallery.inputs");
                install_action_route(&mut app);
                let number = app.demo.number;
                let vector = app.demo.vector;
                app.begin_text_selection(STANDALONE_WINDOW, field, None);
                let (state, context) = app.text_state_and_context_mut(field);
                state
                    .reset(
                        context,
                        if field == TextFieldKind::Number {
                            "42"
                        } else {
                            "17"
                        },
                    )
                    .expect("unaccepted numeric draft");
                state
                    .preedit(context, "未确认", None)
                    .expect("uncommitted numeric composition");
                app.cancel_window_editors_for(UiWindowId(999), reason);
                assert_eq!(app.text_session_field(STANDALONE_WINDOW), Some(field));
                assert!(
                    app.window_text_composing(STANDALONE_WINDOW),
                    "foreign cleanup cannot end a numeric owner"
                );
                assert_eq!((app.demo.number, app.demo.vector), (number, vector));
                if reason == PopupCloseReason::OwnerRemoved {
                    let root = field.element(app.scopes.gallery);
                    app.retire_window_subtree_input(STANDALONE_WINDOW, root);
                } else {
                    app.cancel_window_editors_for(STANDALONE_WINDOW, reason);
                }
                if (app.demo.number, app.demo.vector) != (number, vector) {
                    failures.push(format!(
                        "{reason:?}/{field:?}: number={}, vector={:?}",
                        app.demo.number, app.demo.vector
                    ));
                }
                assert!(app.text_session.is_none());
                assert!(app.input_ownership.focus().is_none());
                let before_late = (app.demo.number, app.demo.vector);
                assert!(!app.handle_window_text_event(
                    STANDALONE_WINDOW,
                    &WindowEvent::Ime(winit::event::Ime::Commit("99".into()))
                ));
                assert_eq!((app.demo.number, app.demo.vector), before_late);
            }
        }
        assert!(
            failures.is_empty(),
            "cancelled numeric source must preserve accepted Number and mixed axes: {failures:?}"
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TextFieldKind {
    Main,
    Search,
    Area,
    Number,
    Vector(u8),
    ColorHex,
    Password,
    TableNote,
    Label,
}

impl TextFieldKind {
    pub(super) const ALL: [Self; 11] = [
        Self::Main,
        Self::Search,
        Self::Area,
        Self::Number,
        Self::Vector(0),
        Self::Vector(1),
        Self::Vector(2),
        Self::ColorHex,
        Self::Password,
        Self::TableNote,
        Self::Label,
    ];

    pub(super) fn element(self, scope: ElementScope) -> ElementId {
        scope.element(match self {
            Self::Main => TEXT_FIELD,
            Self::Search => SEARCH_FIELD,
            Self::Area => TEXT_AREA,
            Self::Number => NUMBER_FIELD,
            Self::Vector(axis) => VECTOR_VALUE_BASE + u32::from(axis),
            Self::ColorHex => COLOR_HEX_FIELD,
            Self::Password => PASSWORD_ROOT,
            Self::TableNote => TABLE_NOTE_FIELD,
            Self::Label => SELECTABLE_LABEL_FIELD,
        })
    }

    pub(super) fn input_element(self, scope: ElementScope) -> ElementId {
        if self == Self::Password {
            scope.element(PASSWORD_ROOT + 1)
        } else {
            self.element(scope)
        }
    }
}

impl App {
    /// Commits and resets per-field drafts and ends the previous IME session when the caret
    /// moves between text fields. The shared text state is the authority for every window,
    /// so drafts depend on the field while IME teardown also follows window ownership.
    pub(super) fn transition_text_field(
        &mut self,
        previous: Option<TextSession>,
        next: Option<TextSession>,
    ) {
        let next_window = next.map(|session| session.window);
        let transition = TextSessionTransition::new(previous, next);
        let previous_field = transition.previous_field();
        let next = transition.next_field();
        if previous_field == Some(TextFieldKind::Number) && next != previous_field {
            self.commit_number_draft();
        }
        if next == Some(TextFieldKind::Number) && previous_field != next {
            self.reset_number_draft();
        }
        if let Some(TextFieldKind::Vector(axis)) = previous_field
            && next != previous_field
        {
            self.commit_vector_draft(axis);
        }
        if let Some(TextFieldKind::Vector(axis)) = next
            && previous_field != next
        {
            self.reset_vector_draft(axis);
            if self.demo.vector[usize::from(axis)].is_none() {
                let _ = self.vector_states[usize::from(axis)].reset(&mut self.text_context, "");
            }
        }
        if previous_field == Some(TextFieldKind::ColorHex) && next != previous_field {
            self.commit_color_hex_draft();
        }
        if next == Some(TextFieldKind::ColorHex) && previous_field != next {
            self.reset_color_hex_draft();
        }
        if previous_field == Some(TextFieldKind::TableNote) && next != previous_field {
            self.commit_table_note_draft();
        }
        if next == Some(TextFieldKind::TableNote) && previous_field != next {
            self.reset_table_note_draft();
        }
        if transition.fields_changed() || transition.ime_owner_to_end().is_some() {
            self.cancel_text_composition(previous_field);
            // Disable the OS IME context on the window that loses the caret, not only on the
            // main window: a stale candidate list must not follow the session elsewhere.
            if let Some(window) = transition.ime_owner_to_end() {
                self.end_window_ime_session(window);
            }
            if next == Some(TextFieldKind::Area)
                && let Some(window) = next_window
            {
                self.follow_area_caret(window);
            }
        }
    }

    pub(super) fn follow_area_caret(&mut self, window: UiWindowId) {
        if window == STANDALONE_WINDOW {
            self.area_follow_caret = true;
        } else if let Some(host) = self
            .native_windows
            .values_mut()
            .find(|host| host.ui_window == window)
        {
            host.area_follow_caret = true;
        }
    }

    /// Keep the shared editor's derived wrap width in step with the presenting
    /// window's actual field, including resize and native Dock split changes.
    pub(super) fn sync_area_wrap_layout(
        &mut self,
        gallery: ElementScope,
        layout: &LayoutTree,
    ) -> bool {
        self.area_layout.sync(
            &mut self.area_state,
            &mut self.text_context,
            gallery,
            layout,
        )
    }

    fn restore_area_input_layout(&mut self, window: UiWindowId, field: TextFieldKind) {
        if field == TextFieldKind::Area
            && let Some(view) = self.text_view_for(window, field)
        {
            restore_area_view_layout(&mut self.area_state, &mut self.text_context, view);
        }
    }

    /// Gives `window` the caret for `field`, mirroring the main window path: per-field draft
    /// transitions and IME teardown happen once on the shared state, then the window receives
    /// focus, pointer capture and its own IME session.
    pub(super) fn begin_text_selection(
        &mut self,
        window: UiWindowId,
        field: TextFieldKind,
        pointer: Option<Point>,
    ) {
        self.begin_text_selection_at(window, field, pointer, Instant::now());
    }

    fn begin_text_selection_at(
        &mut self,
        window: UiWindowId,
        field: TextFieldKind,
        pointer: Option<Point>,
        now: Instant,
    ) {
        let Some(gallery) = self.gallery_scopes(window) else {
            return;
        };
        self.cancel_inline_for_focus(window, Some(field.element(gallery)));
        if field != TextFieldKind::Password {
            self.demo.password_revealed = false;
        }
        self.prepare_text_selection_capture(window);
        self.transition_window_text_session(window, Some(field));
        if field == TextFieldKind::Area {
            self.follow_area_caret(window);
        }
        let target = InputTarget {
            window,
            element: field.element(gallery),
        };
        if window == STANDALONE_WINDOW {
            // Focus first: changing focus drops any existing pointer capture.
            self.input_ownership.set_focus(Some(target));
            if pointer.is_some() {
                self.input_ownership
                    .capture_pointer(target, PointerButton::Primary);
            } else {
                self.input_ownership.cancel_capture();
                self.text_bridge.clear_text_pointer();
            }
        } else if let Some(host) = self
            .native_windows
            .values_mut()
            .find(|host| host.ui_window == window)
        {
            host.input.set_focus(Some(target));
            if pointer.is_some() {
                host.input.capture_pointer(target, PointerButton::Primary);
            } else {
                host.input.cancel_capture();
                if let Some(bridge) = self.native_text_bridges.get_mut(window) {
                    bridge.clear_text_pointer();
                }
            }
        }
        let view = pointer.and_then(|_| self.text_view_for(window, field));
        if let (Some(pointer), Some(view)) = (pointer, view) {
            self.restore_area_input_layout(window, field);
            let composing = self.text_state_and_context_mut(field).0.is_composing();
            let Some(mode) = self
                .text_bridge_for(window)
                .and_then(|bridge| bridge.text_pointer_press(target, pointer, now, composing))
            else {
                self.sync_text_ime_for(window);
                return;
            };
            let extend = mode == vesta_gpui_winit::TextPointerAction::Extend;
            let word = mode == vesta_gpui_winit::TextPointerAction::Word;
            let revealed = field == TextFieldKind::Password && self.demo.password_revealed;
            let (state, context) = self.text_state_and_context_mut(field);
            if revealed {
                state.point_revealed(context, view.to_layout(pointer), extend, word);
            } else {
                state.point(context, view.to_layout(pointer), extend, word);
            }
        }
        self.sync_text_ime_for(window);
    }

    /// Extends the selection of the window that captured the pointer for a text field.
    pub(super) fn update_window_text_selection(
        &mut self,
        window: UiWindowId,
        pointer: Point,
    ) -> bool {
        let Some(gallery) = self.gallery_scopes(window) else {
            return false;
        };
        let capture = if window == STANDALONE_WINDOW {
            self.input_ownership.capture()
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .and_then(|host| host.input.capture())
        };
        let Some(capture) = capture else {
            return false;
        };
        if capture.target.window != window || capture.button != PointerButton::Primary {
            return false;
        }
        let field = TextFieldKind::ALL
            .into_iter()
            .find(|field| field.element(gallery) == capture.target.element);
        let (Some(field), Some(view)) = (
            field,
            field.and_then(|field| self.text_view_for(window, field)),
        ) else {
            return false;
        };
        if !self
            .text_bridge_for(window)
            .is_some_and(|bridge| bridge.text_pointer_drag(capture.target, pointer))
        {
            return false;
        }
        let revealed = field == TextFieldKind::Password && self.demo.password_revealed;
        self.restore_area_input_layout(window, field);
        let (state, context) = self.text_state_and_context_mut(field);
        if revealed {
            state.point_revealed(context, view.to_layout(pointer), true, false);
        } else {
            state.point(context, view.to_layout(pointer), true, false);
        }
        true
    }

    /// Drops the caret session and ends its IME session in the owning window.
    pub(super) fn clear_text_session(&mut self) {
        self.caret_reveal = None;
        let Some(previous) = self.text_session.take() else {
            return;
        };
        self.cancel_text_composition(Some(previous.field));
        self.end_window_ime_session(previous.window);
    }

    pub(super) fn text_bridge_for(&mut self, window: UiWindowId) -> Option<&mut TextInputBridge> {
        if window == STANDALONE_WINDOW {
            Some(&mut self.text_bridge)
        } else {
            self.native_text_bridges.get_mut(window)
        }
    }

    /// Runs before normal dispatch, even when another widget consumes the event.
    pub(super) fn observe_text_pointer_event(&mut self, window: UiWindowId, event: &WindowEvent) {
        self.restart_caret_on_input(window, event);
        let hit = if window == STANDALONE_WINDOW {
            self.hit_test.hit(self.cursor)
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .and_then(|host| host.hit_test.hit(host.cursor))
        };
        let text_hit = self.text_field_at(window, hit).is_some();
        if let Some(bridge) = self.text_bridge_for(window) {
            bridge.observe_text_pointer_event(event, text_hit);
        }
    }

    /// Field state plus the shared context, as disjoint borrows.
    pub(super) fn text_state_and_context_mut(
        &mut self,
        field: TextFieldKind,
    ) -> (&mut TextInputState, &mut TextInputContext) {
        let context = &mut self.text_context;
        let state = match field {
            TextFieldKind::Main => &mut self.text_state,
            TextFieldKind::Search => &mut self.search_state,
            TextFieldKind::Area => &mut self.area_state,
            TextFieldKind::Number => &mut self.number_state,
            TextFieldKind::Vector(axis) => &mut self.vector_states[usize::from(axis)],
            TextFieldKind::ColorHex => &mut self.color_hex_state,
            TextFieldKind::Password => &mut self.password_state,
            TextFieldKind::TableNote => &mut self.table_note_state,
            TextFieldKind::Label => &mut self.readonly_label_state,
        };
        (state, context)
    }

    /// The viewport one window laid out for a field; `None` when that window does not show it.
    pub(super) fn text_view_for(
        &self,
        window: UiWindowId,
        field: TextFieldKind,
    ) -> Option<vesta_gpui::TextInputViewport> {
        if window == STANDALONE_WINDOW {
            return match field {
                TextFieldKind::Main => self.text_view,
                TextFieldKind::Search => self.search_view,
                TextFieldKind::Area => self.area_view,
                TextFieldKind::Number => self.number_view,
                TextFieldKind::Vector(axis) => self.vector_views[usize::from(axis)],
                TextFieldKind::ColorHex => self.color_hex_view,
                TextFieldKind::Password => self.password_view,
                TextFieldKind::TableNote => self.table_note_view,
                TextFieldKind::Label => self.readonly_label_view,
            };
        }
        self.native_windows
            .values()
            .find(|host| host.ui_window == window)
            .and_then(|host| host.text_views.get(field))
    }

    /// Routes a key or IME event to the shared text bridge when `window` owns the caret.
    pub(super) fn handle_window_text_event(
        &mut self,
        window: UiWindowId,
        event: &WindowEvent,
    ) -> bool {
        let Some(field) = self.text_session_field(window) else {
            return false;
        };
        self.restore_area_input_layout(window, field);
        self.action_window = window;
        if let WindowEvent::KeyboardInput { event: key, .. } = event
            && key.state == ElementState::Pressed
            && (self.handle_palette_key_for(window, &key.logical_key)
                || self.handle_inline_edit_key_for(window, &key.logical_key)
                || self.handle_suggestion_key_for(window, &key.logical_key))
        {
            self.request_all_redraw();
            return true;
        }
        // Built from disjoint field paths so the bridge can be borrowed at the same time.
        let (state, context, clipboard) = (
            match field {
                TextFieldKind::Main => &mut self.text_state,
                TextFieldKind::Search => &mut self.search_state,
                TextFieldKind::Area => &mut self.area_state,
                TextFieldKind::Number => &mut self.number_state,
                TextFieldKind::Vector(axis) => &mut self.vector_states[usize::from(axis)],
                TextFieldKind::ColorHex => &mut self.color_hex_state,
                TextFieldKind::Password => &mut self.password_state,
                TextFieldKind::TableNote => &mut self.table_note_state,
                TextFieldKind::Label => &mut self.readonly_label_state,
            },
            &mut self.text_context,
            &mut self.clipboard,
        );
        let bridge = if window == STANDALONE_WINDOW {
            &mut self.text_bridge
        } else if let Some(bridge) = self.native_text_bridges.get_mut(window) {
            bridge
        } else {
            return false;
        };
        match bridge.handle_window_event(event, state, context, clipboard, false) {
            Ok(TextInputResponse::Submit) => {
                if field == TextFieldKind::Number {
                    self.commit_number_draft();
                } else if field == TextFieldKind::TableNote {
                    self.commit_table_note_draft();
                } else if field == TextFieldKind::ColorHex {
                    self.commit_color_hex_draft();
                } else if let TextFieldKind::Vector(axis) = field {
                    self.commit_vector_draft(axis);
                }
                self.set_window_focus(window, None);
                self.request_all_redraw();
            }
            Ok(TextInputResponse::Cancel) => {
                if field == TextFieldKind::Number {
                    self.reset_number_draft();
                } else if field == TextFieldKind::TableNote {
                    self.reset_table_note_draft();
                } else if field == TextFieldKind::ColorHex {
                    self.reset_color_hex_draft();
                } else if let TextFieldKind::Vector(axis) = field {
                    self.reset_vector_draft(axis);
                }
                self.set_window_focus(window, None);
                self.request_all_redraw();
            }
            Ok(TextInputResponse::TraverseFocus { backwards }) => {
                self.traverse_window_focus(window, backwards);
                self.request_all_redraw();
            }
            Ok(response) if response.is_handled() => {
                if matches!(response, TextInputResponse::Handled { changed: true })
                    && let Some(gallery) = self.gallery_scopes(window)
                {
                    if field == TextFieldKind::Area {
                        self.follow_area_caret(window);
                    }
                    self.caret_reveal = Some(InputTarget {
                        window,
                        element: field.element(gallery),
                    });
                }
                self.request_all_redraw();
            }
            Err(error) => eprintln!("navigating native window text: {error}"),
            _ => {}
        }
        true
    }

    pub(super) fn cancel_text_composition(&mut self, field: Option<TextFieldKind>) {
        if let Some(field) = field {
            let (state, context) = self.text_state_and_context_mut(field);
            state.cancel_compose(context);
            state.end_group();
        }
    }
}
