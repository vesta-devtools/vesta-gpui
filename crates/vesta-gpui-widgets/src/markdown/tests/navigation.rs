use super::*;
#[path = "navigation_pages.rs"]
mod pages;

#[test]
fn visual_line_navigation_keeps_columns_affinity_graphemes_and_live_projection() {
    pages::qualify();
    use vesta_gpui::{
        FontRole, InputOwnership, KeyChord, KeyModifiers, KeyStroke, LayoutEngine,
        TextInputContext, Viewport,
    };
    let window = UiWindowId(31);
    let scope = ElementIdAllocator
        .allocate_scope()
        .expect("navigation scope");
    let theme = WidgetTheme::dark();
    let code = "abcde fghij klmno pqrst\nab\nabcde fghij klmno pqrst\n中文👨‍👩‍👧‍👦Z\n";
    let mut document =
        MarkdownView::parse(&format!("```\n{code}```\n\n- next indented block")).expect("document");
    let mut state = MarkdownInteraction::new(window, scope);
    let mut context = TextInputContext::default();
    let font = include_bytes!("../../../../vesta-gpui/tests/fixtures/MaterialIcons-Regular.ttf");
    context.register_font(std::sync::Arc::new(font.to_vec()));
    for role in [FontRole::Ui, FontRole::Mono] {
        context.set_font_stack(role, "Material Icons");
    }
    let project = |document: &mut MarkdownView, context: &mut TextInputContext, width| {
        let mut layout = LayoutEngine
            .compute(
                &document.view(scope, 0..2, theme),
                Viewport::new(width, 50.0, 1.0),
            )
            .expect("layout");
        for _ in 0..4 {
            if !document
                .sync_layout(scope, &layout, context, theme)
                .expect("shape")
            {
                break;
            }
            layout = LayoutEngine
                .compute(
                    &document.view(scope, 0..2, theme),
                    Viewport::new(width, 50.0, 1.0),
                )
                .expect("shaped layout");
        }
        layout
    };
    let layout = project(&mut document, &mut context, 500.0);
    let surface = MarkdownSurface {
        window,
        document: &document,
        layout: &layout,
        range: 0..2,
    };
    let mut input = InputOwnership::default();
    input.set_focus(Some(state.target()));
    let plain = KeyModifiers::default();
    let shift = KeyModifiers {
        shift: true,
        ..plain
    };
    let command = KeyModifiers {
        control: true,
        ..plain
    };
    assert_eq!(
        state.key(
            &surface,
            KeyChord::new(KeyStroke::Home, command),
            4096,
            &mut input
        ),
        Ok(Some(MarkdownKeyIntent::RevealSelection))
    );
    for _ in 0..8 {
        assert_eq!(
            state.key(
                &surface,
                KeyChord::new(KeyStroke::ArrowRight, plain),
                4096,
                &mut input
            ),
            Ok(Some(MarkdownKeyIntent::RevealSelection))
        );
    }
    assert_eq!(state.selection().expect("focus").focus.byte(), 8);
    let first_position = state.selection();
    assert_eq!(
        state.key(
            &surface,
            KeyChord::new(KeyStroke::PageDown, shift),
            4096,
            &mut input
        ),
        Ok(Some(MarkdownKeyIntent::RevealSelection))
    );
    let paged = state.selection().expect("viewport page");
    assert_eq!(paged.anchor.byte(), 8);
    assert_ne!(
        Some(paged),
        first_position,
        "page movement advances by visual height"
    );
    assert_eq!(
        state.key(
            &surface,
            KeyChord::new(KeyStroke::PageUp, plain),
            4096,
            &mut input
        ),
        Ok(Some(MarkdownKeyIntent::RevealSelection))
    );
    assert_eq!(
        state.selection(),
        first_position,
        "page return retains the long-line column"
    );

    // The focus block is valid, but another candidate has stale projected width.
    let mut stale_page = document.view(scope, 0..2, theme);
    stale_page.children[1].style.width = Dimension::Px(200.0);
    let stale_page = LayoutEngine
        .compute(&stale_page, Viewport::new(500.0, 50.0, 1.0))
        .expect("stale candidate layout");
    assert_eq!(
        state.key(
            &MarkdownSurface {
                layout: &stale_page,
                range: surface.range.clone(),
                ..surface
            },
            KeyChord::new(KeyStroke::PageDown, shift),
            4096,
            &mut input
        ),
        Err(MarkdownProjectionError::StaleWidth(
            document.block_id_at(1).expect("candidate")
        ))
    );
    assert_eq!(
        state.selection(),
        first_position,
        "stale candidate cannot partly publish a page move"
    );
    assert_eq!(
        state.key(
            &surface,
            KeyChord::new(KeyStroke::ArrowDown, plain),
            4096,
            &mut input
        ),
        Ok(Some(MarkdownKeyIntent::RevealSelection))
    );
    assert_eq!(state.selection().expect("short line").focus.byte(), 26);
    state
        .key(
            &surface,
            KeyChord::new(KeyStroke::ArrowDown, plain),
            4096,
            &mut input,
        )
        .expect("third line");
    assert_eq!(
        state.selection().expect("preferred column").focus.byte(),
        35
    );
    let focus = state
        .selection_focus_rect(&surface)
        .expect("focus geometry")
        .expect("focus");
    assert!(
        focus.bottom() > 50.0,
        "offscreen focus remains available for host reveal"
    );
    for _ in 0..2 {
        state
            .key(
                &surface,
                KeyChord::new(KeyStroke::ArrowUp, plain),
                4096,
                &mut input,
            )
            .expect("return column");
    }
    assert_eq!(state.selection().expect("first line").focus.byte(), 8);
    state
        .key(
            &surface,
            KeyChord::new(KeyStroke::ArrowDown, shift),
            4096,
            &mut input,
        )
        .expect("extend line");
    assert_eq!(state.selection().expect("stable anchor").anchor.byte(), 8);
    assert_eq!(
        state.key(
            &surface,
            KeyChord::character('c', command),
            4096,
            &mut input
        ),
        Ok(Some(MarkdownKeyIntent::Copy("hij klmno pqrst\nab".into())))
    );
    let accepted = state.selection();
    let stale = LayoutEngine
        .compute(
            &document.view(scope, 0..2, theme),
            Viewport::new(600.0, 50.0, 1.0),
        )
        .expect("unshaped new width");
    assert!(matches!(
        state.key(
            &MarkdownSurface {
                window,
                document: &document,
                layout: &stale,
                range: 0..2
            },
            KeyChord::new(KeyStroke::ArrowUp, plain),
            4096,
            &mut input
        ),
        Err(MarkdownProjectionError::StaleWidth(_))
    ));
    assert_eq!(
        state.selection(),
        accepted,
        "stale geometry cannot partially publish navigation"
    );
    let layout = project(&mut document, &mut context, 600.0);
    let surface = MarkdownSurface {
        window,
        document: &document,
        layout: &layout,
        range: 0..2,
    };
    let short_focus = state
        .selection_focus_rect(&surface)
        .expect("reshaped short line")
        .expect("focus");
    let first_block = document.block_id_at(0).expect("first");
    let (snapshot, viewport) = document
        .block_viewport(scope, &layout, first_block)
        .expect("fresh viewport");
    let first_line = snapshot.layout_cursor(0).expect("first line").bounds();
    let expected_at_x = document
        .anchor_at_point(
            scope,
            &layout,
            first_block,
            vesta_gpui::Point::new(
                short_focus.x,
                viewport.origin.y + first_line.y + first_line.height * 0.5,
            ),
        )
        .expect("point at new column")
        .expect("visible first line");
    state
        .key(
            &surface,
            KeyChord::new(KeyStroke::ArrowUp, plain),
            4096,
            &mut input,
        )
        .expect("reshape remaps logical focus");
    assert_eq!(
        state.selection().expect("new column").focus,
        expected_at_x,
        "reshaping resets to the short line's actual visual column"
    );
    assert_ne!(
        expected_at_x.byte(),
        8,
        "the fixture distinguishes stale preferred column eight"
    );
    let before = state.selection();
    let wrong = MarkdownSurface {
        window: UiWindowId(32),
        document: &document,
        layout: &layout,
        range: 0..2,
    };
    assert_eq!(
        state.key(
            &wrong,
            KeyChord::new(KeyStroke::ArrowUp, plain),
            4096,
            &mut input
        ),
        Ok(None)
    );
    assert_eq!(state.selection(), before);
    let off_page = MarkdownSurface {
        range: 1..2,
        document: &document,
        layout: &layout,
        window,
    };
    for key in [KeyStroke::ArrowDown, KeyStroke::PageDown] {
        assert_eq!(
            state.key(&off_page, KeyChord::new(key, plain), 4096, &mut input),
            Ok(None)
        );
    }
    assert_eq!(
        state.selection(),
        before,
        "off-page accepted focus is not relocated"
    );

    // Start at a real soft-wrap edge with upstream affinity, then move up.
    let narrow = project(&mut document, &mut context, 120.0);
    let surface = MarkdownSurface {
        window,
        document: &document,
        layout: &narrow,
        range: 0..2,
    };
    let first = document.block_id_at(0).expect("code block");
    let (snapshot, viewport) = document
        .block_viewport(scope, &narrow, first)
        .expect("viewport");
    let cursor = snapshot.layout_cursor(0).expect("cursor");
    assert!(cursor.line_count() > 5, "source contains actual soft wraps");
    let end_of_line = cursor
        .on_line(0, snapshot.layout_width() + 100.0)
        .expect("first soft line end");
    assert_eq!(end_of_line.line_index(), Some(0));
    assert!(end_of_line.byte_index() < code.find('\n').expect("hard break"));
    let point = vesta_gpui::Point::new(
        viewport.clip.right() - 0.01,
        viewport.origin.y + end_of_line.bounds().y + end_of_line.bounds().height * 0.5,
    );
    assert_eq!(
        state.press(&surface, point, false, &mut input),
        Ok(MarkdownPointerResult::Selection)
    );
    state
        .release(
            &surface,
            point,
            vesta_gpui::PointerButton::Primary,
            &mut input,
        )
        .expect("release");
    state
        .key(
            &surface,
            KeyChord::new(KeyStroke::ArrowDown, plain),
            4096,
            &mut input,
        )
        .expect("next visual line");
    let focus = state
        .selection_focus_rect(&surface)
        .expect("moved bounds")
        .expect("focus");
    let actual = snapshot
        .layout_cursor_at(viewport.to_layout(vesta_gpui::Point::new(
            focus.x,
            focus.y + focus.height * 0.5,
        )))
        .expect("actual cursor");
    assert_eq!(
        actual.line_index(),
        Some(1),
        "upstream wrap affinity must not reinterpret focus on the next line"
    );
    // Every moved position is still a valid complete-grapheme document anchor.
    for step in 0..32 {
        state
            .key(
                &surface,
                KeyChord::new(
                    if step % 2 == 0 {
                        KeyStroke::PageDown
                    } else {
                        KeyStroke::ArrowDown
                    },
                    shift,
                ),
                4096,
                &mut input,
            )
            .expect("across Unicode and blocks");
        let focus = state.selection().expect("selection").focus;
        document
            .anchor(focus.block(), focus.byte())
            .expect("complete grapheme");
    }
    assert_eq!(
        state.selection().expect("last page edge").focus.block(),
        document.block_id_at(1).expect("second")
    );
    let last = state.selection().expect("selection").focus;
    state
        .key(
            &surface,
            KeyChord::new(KeyStroke::ArrowDown, plain),
            4096,
            &mut input,
        )
        .expect("clamp");
    assert_eq!(state.selection().expect("clamped").focus, last);
    for (key, modifiers) in [
        (KeyStroke::ArrowLeft, plain),
        (KeyStroke::ArrowRight, shift),
        (KeyStroke::Home, plain),
        (KeyStroke::End, shift),
        (KeyStroke::Home, command),
        (
            KeyStroke::End,
            KeyModifiers {
                shift: true,
                ..command
            },
        ),
    ] {
        let before = state.selection().expect("logical focus");
        assert_eq!(
            state.key(&surface, KeyChord::new(key, modifiers), 4096, &mut input),
            Ok(Some(MarkdownKeyIntent::RevealSelection))
        );
        let selection = state.selection().expect("logical navigation");
        if modifiers.shift {
            assert_eq!(selection.anchor, before.anchor);
        } else {
            assert_eq!(selection.anchor, selection.focus);
        }
        document
            .anchor(selection.focus.block(), selection.focus.byte())
            .expect("complete grapheme");
        assert!(
            state
                .selection_focus_rect(&surface)
                .expect("unclipped focus geometry")
                .is_some()
        );
        assert_eq!(input.focus(), Some(state.target()));
    }
    input.set_focus(None);
    state.sync(&surface, &mut input);
    let accepted = state.selection();
    assert_eq!(
        state.key(
            &surface,
            KeyChord::new(KeyStroke::ArrowUp, plain),
            4096,
            &mut input
        ),
        Ok(None)
    );
    assert_eq!(
        state.selection(),
        accepted,
        "focus loss cannot move accepted selection"
    );
    input.set_focus(Some(state.target()));
    document.replace("replacement").expect("revision");
    let next = project(&mut document, &mut context, 120.0);
    state.sync(
        &MarkdownSurface {
            window,
            document: &document,
            layout: &next,
            range: 0..1,
        },
        &mut input,
    );
    assert!(state.selection().is_none());
}
