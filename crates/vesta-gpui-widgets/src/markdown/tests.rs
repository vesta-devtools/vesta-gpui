use super::*;
use vesta_gpui::{ElementIdAllocator, UiWindowId};

#[test]
fn revision_and_window_boundaries_prevent_stale_copy_and_link_aliasing() {
    let window = UiWindowId(1);
    let other_window = UiWindowId(2);
    let source = "# Brief\n\n中文 **粗体** 👨‍👩‍👧‍👦 [Vesta `Core`][ref]\n\n尾声\n\n[ref]: ../core.md\n";
    let mut document = MarkdownView::parse(source).expect("document");
    let document_id = document.document_id().expect("document identity");
    let revision = document.revision();
    let first = document.block_id_at(1).expect("first block");
    let last = document.block_id_at(2).expect("last block");
    let scope = ElementIdAllocator.allocate_scope().expect("scope");
    let paragraph = document.paragraph_id(scope, first).expect("element");
    let theme = WidgetTheme::dark();
    assert_eq!(
        document.view(scope, 1..2, theme).find(paragraph),
        document.view(scope, 0..3, theme).find(paragraph),
        "paging must not create a new paragraph identity"
    );
    let start = document.anchor(first, 0).expect("first anchor");
    let end = document.anchor(last, "尾声".len()).expect("last anchor");
    let selection = document.selection(window, start, end).expect("selection");
    let copied = "中文 粗体 👨‍👩‍👧‍👦 Vesta Core\n\n尾声";
    let all = document
        .select_all(window)
        .expect("select entire document")
        .expect("document range");
    assert_eq!(
        document
            .copy_selection(window, all, 4096)
            .expect("full rendered copy"),
        format!("Brief\n\n{copied}")
    );
    assert_eq!(
        MarkdownView::parse("")
            .expect("empty document")
            .select_all(window),
        Ok(None)
    );
    assert_eq!(
        document
            .copy_selection(window, selection, copied.len())
            .expect("mixed copy"),
        copied
    );
    let reversed = document
        .selection(window, end, start)
        .expect("reverse selection");
    assert_eq!(
        document
            .copy_selection(window, reversed, copied.len())
            .expect("reverse copy"),
        copied
    );
    assert_eq!(
        document.copy_selection(other_window, selection, copied.len()),
        Err(MarkdownAnchorError::WrongWindow)
    );
    assert_eq!(
        document.copy_selection(window, selection, copied.len() - 1),
        Err(MarkdownAnchorError::CopyLimit {
            limit: copied.len() - 1
        })
    );
    let empty = document
        .selection(window, start, start)
        .expect("empty selection");
    assert_eq!(
        document
            .copy_selection(window, empty, 0)
            .expect("empty copy"),
        ""
    );
    assert_eq!(
        document.anchor(first, 1),
        Err(MarkdownAnchorError::InvalidBoundary)
    );
    let emoji = "中文 粗体 ".len();
    assert!(document.anchor(first, emoji).is_ok());
    assert!(
        document.anchor(first, emoji + "👨".len()).is_err(),
        "ZWJ family must remain a complete grapheme"
    );
    assert!(document.anchor(first, usize::MAX).is_err());
    let intent = document
        .links_in(window, 1..2)
        .next()
        .expect("reference link");
    let original_link_target = document
        .link_target(scope, window, intent)
        .expect("link target");
    assert_eq!(
        document.resolve_link(window, intent).expect("link"),
        MarkdownLink {
            label: "Vesta Core",
            destination: "../core.md"
        }
    );
    assert_eq!(
        document.resolve_link(other_window, intent),
        Err(MarkdownAnchorError::WrongWindow)
    );
    assert!(document.links_in(window, 2..3).next().is_none());
    assert!(!document.replace(source).expect("same source"));
    assert_eq!(document.revision(), revision);
    assert!(document.resolve_link(window, intent).is_ok());
    assert!(matches!(
        document.replace(&"x".repeat(vesta_gpui::TEXT_INPUT_MAX_BYTES + 1)),
        Err(MarkdownError::ParagraphTooLong { .. })
    ));
    assert_eq!(
        document.revision(),
        revision,
        "rejected replacement is atomic"
    );
    assert_eq!(
        document
            .copy_selection(window, selection, copied.len())
            .expect("retained copy"),
        copied
    );
    assert!(
        document
            .replace(&format!("新增\n\n{source}"))
            .expect("insert")
    );
    assert_eq!(document.document_id(), Some(document_id));
    assert_eq!(
        document.block_id_at(2),
        Some(first),
        "unique unchanged block survives insertion"
    );
    assert_eq!(
        document
            .paragraph_id(scope, first)
            .expect("retained element"),
        paragraph
    );
    assert_eq!(
        document.copy_selection(window, selection, copied.len()),
        Err(MarkdownAnchorError::StaleRevision)
    );
    assert_eq!(
        document.resolve_link(window, intent),
        Err(MarkdownAnchorError::StaleRevision)
    );
    let current = document
        .links_in(window, 2..3)
        .next()
        .expect("current link");
    assert!(document.resolve_link(window, current).is_ok());
    assert_eq!(
        document
            .link_target(scope, window, current)
            .expect("retained link target"),
        original_link_target
    );
    assert!(document.replace("changed").expect("remove"));
    assert_eq!(
        document.paragraph_id(scope, first),
        Err(MarkdownAnchorError::UnknownBlock)
    );
    assert!(document.replace(source).expect("restore contents"));
    let restored_link = document
        .links_in(window, 1..2)
        .next()
        .expect("restored link");
    assert_ne!(
        document
            .link_target(scope, window, restored_link)
            .expect("restored target"),
        original_link_target
    );
    let duplicates =
        MarkdownView::parse("[same](next.md) [same](next.md)").expect("duplicate links");
    let duplicate_targets = duplicates
        .links_in(window, 0..1)
        .map(|intent| {
            duplicates
                .link_target(scope, window, intent)
                .expect("unique target")
        })
        .collect::<Vec<_>>();
    assert_eq!(duplicate_targets.len(), 2);
    assert_ne!(duplicate_targets[0], duplicate_targets[1]);
    assert_ne!(
        document.block_id_at(1),
        Some(first),
        "removed identity is never reused"
    );
    let replacement = MarkdownView::parse(source).expect("different document in same host scope");
    assert!(
        replacement
            .view(scope, 0..replacement.len(), theme)
            .find(paragraph)
            .is_none()
    );
    assert_eq!(
        replacement.resolve_link(window, intent),
        Err(MarkdownAnchorError::WrongDocument)
    );
    assert!(replacement.selection(window, start, end).is_err());
    let unavailable = MarkdownView::default();
    assert_eq!(
        unavailable.select_all(window),
        Err(MarkdownAnchorError::DocumentUnavailable)
    );
    assert_eq!(
        unavailable.resolve_link(window, intent),
        Err(MarkdownAnchorError::DocumentUnavailable)
    );

    let mut duplicates = MarkdownView::parse("same\n\nsame\n\nunique").expect("duplicates");
    let repeated = [duplicates.block_id_at(0), duplicates.block_id_at(1)];
    let unique = duplicates.block_id_at(2);
    assert!(
        duplicates
            .replace("insert\n\nsame\n\nsame\n\nunique")
            .expect("ambiguous insert")
    );
    for index in 1..3 {
        assert!(
            !repeated.contains(&duplicates.block_id_at(index)),
            "duplicate identity cannot be guessed from position"
        );
    }
    assert_eq!(duplicates.block_id_at(3), unique);
    let repeated = [duplicates.block_id_at(1), duplicates.block_id_at(2)];
    assert!(
        duplicates
            .replace("insert\n\nsame\n\nsame\n\nunique\n")
            .expect("source-only change")
    );
    assert_eq!(
        [duplicates.block_id_at(1), duplicates.block_id_at(2)],
        repeated,
        "identical complete block sequence is unambiguous"
    );
    let image = MarkdownView::parse("![alt](image.png) [a](x) [b](y)").expect("image and links");
    let links = image
        .links_in(window, 0..image.len())
        .map(|intent| image.resolve_link(window, intent).expect("link").label)
        .collect::<Vec<_>>();
    assert_eq!(links, ["a", "b"], "image source is not a navigation link");
}

#[test]
fn shaped_link_fragments_and_selection_share_clipping_and_document_identity() {
    tables::qualify();
    use vesta_gpui::{
        Dimension, ElementKind, FontRole, LayoutEngine, Point, Rect, Style, TextInputContext,
        Viewport,
    };
    let window = UiWindowId(1);
    let other = UiWindowId(2);
    // Quote decoration and a quoted list marker must not collide with the stable
    // text/link identities or change the authoritative selection geometry.
    let mut document = MarkdownView::parse("> before [long **中文** `code` مرحبا mixed link with enough words to wrap across several lines](../next.md) after\n\n> - second block").expect("document");
    let scope = ElementIdAllocator.allocate_scope().expect("scope");
    let parent = ElementIdAllocator.allocate_scope().expect("scroll scope");
    let first = document.block_id_at(0).expect("first block");
    let second = document.block_id_at(1).expect("second block");
    let start = document.anchor(first, 0).expect("start");
    let end = document.anchor(second, "second block".len()).expect("end");
    let selection = document.selection(window, start, end).expect("selection");
    let theme = WidgetTheme::light();
    let intent = document.links_in(window, 0..1).next().expect("link");
    assert!(matches!(
        document.selected_view(scope, window, 0..1, selection, theme),
        Err(MarkdownProjectionError::UnshapedBlock(_))
    ));
    let mut context = TextInputContext::default();
    let font = include_bytes!("../../../vesta-gpui/tests/fixtures/MaterialIcons-Regular.ttf");
    assert!(context.register_font(std::sync::Arc::new(font.to_vec())));
    context.set_font_stack(FontRole::Ui, "Material Icons");
    let project = |document: &MarkdownView, width: f32, context: &mut TextInputContext| {
        let root = Element::scroll_box(
            parent.element(0),
            parent.identity(0, "document.scroll").key,
            Point::default(),
        )
        .styled(Style::default().size(Dimension::Px(width), Dimension::Px(140.0)))
        .child(document.view(scope, 0..document.len(), theme));
        LayoutEngine
            .compute_with_text_layout(
                &root,
                Viewport::new(width, 140.0, 1.25),
                |text, size, role, wrap| {
                    let snapshot = context
                        .shape_static_text_wrapped(text, size, role, wrap)
                        .expect("fallback");
                    vesta_gpui::Size::new(snapshot.layout_width(), snapshot.layout_height())
                },
            )
            .expect("document layout")
    };
    let mut layout = project(&document, 240.0, &mut context);
    for _ in 0..4 {
        if !document
            .sync_layout(scope, &layout, &mut context, theme)
            .expect("shape")
        {
            break;
        }
        layout = project(&document, 240.0, &mut context);
    }
    assert!(
        !document
            .sync_layout(scope, &layout, &mut context, theme)
            .expect("converged")
    );
    let accepted = (
        document.document_id(),
        document.revision(),
        document.block_id_at(0),
    );
    let copied_before_release = document
        .copy_selection(window, selection, 4096)
        .expect("accepted selection");
    assert!(
        document.release_layout() > 0,
        "virtualized hosts can release actual glyph snapshots"
    );
    assert_eq!(document.release_layout(), 0);
    assert_eq!(
        (
            document.document_id(),
            document.revision(),
            document.block_id_at(0)
        ),
        accepted
    );
    assert_eq!(
        document
            .copy_selection(window, selection, 4096)
            .expect("source selection survives"),
        copied_before_release
    );
    assert!(
        document
            .sync_layout(scope, &layout, &mut context, theme)
            .expect("recreate derived layout")
    );
    assert_eq!(
        (
            document.document_id(),
            document.revision(),
            document.block_id_at(0)
        ),
        accepted
    );
    let rectangles = document
        .link_rectangles(scope, window, &layout, intent)
        .expect("link fragments");
    assert!(
        rectangles.len() > 1,
        "wrapped link must have individual line fragments"
    );
    for rectangle in &rectangles {
        assert!(Rect::new(0.0, 0.0, 240.0, 140.0).contains_rect(*rectangle));
        let point = Point::new(
            rectangle.x + rectangle.width * 0.5,
            rectangle.y + rectangle.height * 0.5,
        );
        assert_eq!(
            document
                .link_at_point(scope, window, &layout, 0..1, point)
                .expect("link hit"),
            Some(intent)
        );
        assert!(
            document
                .anchor_at_point(scope, &layout, first, point)
                .expect("anchor hit")
                .is_some()
        );
    }
    assert_eq!(
        document
            .anchor_at_point(scope, &layout, first, Point::new(10.0, -1.0))
            .expect("outside clip"),
        None
    );
    assert_eq!(
        document
            .link_at_point(scope, window, &layout, 0..1, Point::new(10.0, 141.0))
            .expect("outside viewport"),
        None
    );
    assert_eq!(
        document.link_rectangles(scope, other, &layout, intent),
        Err(MarkdownProjectionError::Anchor(
            MarkdownAnchorError::WrongWindow
        ))
    );
    let first_paragraph = document
        .paragraph_id(scope, first)
        .expect("first paragraph");
    let selected = document
        .selected_view(scope, window, 0..2, selection, theme)
        .expect("selected view");
    let ElementKind::TextInput { snapshot, focused } =
        &selected.find(first_paragraph).expect("paragraph").kind
    else {
        panic!("shaped selected paragraph");
    };
    assert!(!focused);
    let node = layout.node(first_paragraph).expect("paragraph layout");
    let view = snapshot.viewport(node.rect, node.inherited_clip, Point::default());
    assert!(
        !snapshot
            .decorations(view, false, false, 1.25)
            .selection
            .is_empty()
    );
    let plain = document.view(scope, 0..2, theme);
    let ElementKind::TextInput {
        snapshot: original, ..
    } = &plain.find(first_paragraph).expect("plain paragraph").kind
    else {
        panic!("plain paragraph");
    };
    assert!(
        original
            .decorations(view, false, false, 1.25)
            .selection
            .is_empty(),
        "selection projection never mutates the cached source snapshot"
    );
    assert_eq!(snapshot.layout_width(), original.layout_width());
    assert_eq!(snapshot.layout_height(), original.layout_height());
    let page = LayoutEngine
        .compute(
            &document.view(scope, 1..2, theme),
            Viewport::new(240.0, 140.0, 1.25),
        )
        .expect("next page");
    assert_eq!(
        document.link_rectangles(scope, window, &page, intent),
        Err(MarkdownProjectionError::MissingLayout(first)),
        "old page has no link geometry in the current projection"
    );
    assert_eq!(
        document
            .link_at_point(scope, window, &page, 1..2, Point::new(20.0, 20.0))
            .expect("next page hit"),
        None
    );
    assert_eq!(
        document
            .copy_selection(window, selection, 4096)
            .expect("cross-block copy"),
        "before long 中文 code مرحبا mixed link with enough words to wrap across several lines after\n\nsecond block"
    );
    let mut wider = project(&document, 400.0, &mut context);
    assert_eq!(
        document.link_rectangles(scope, window, &wider, intent),
        Err(MarkdownProjectionError::StaleWidth(first))
    );
    for _ in 0..4 {
        if !document
            .sync_layout(scope, &wider, &mut context, theme)
            .expect("reflow including scrollbar gutter")
        {
            break;
        }
        wider = project(&document, 400.0, &mut context);
    }
    assert!(
        !document
            .sync_layout(scope, &wider, &mut context, theme)
            .expect("wider view converges")
    );
    assert!(
        document
            .link_rectangles(scope, window, &wider, intent)
            .is_ok()
    );
    assert!(document.replace("changed").expect("new revision"));
    assert_eq!(
        document.link_rectangles(scope, window, &wider, intent),
        Err(MarkdownProjectionError::Anchor(
            MarkdownAnchorError::StaleRevision
        ))
    );
    assert!(matches!(
        document.selected_view(scope, window, 0..1, selection, theme),
        Err(MarkdownProjectionError::Anchor(
            MarkdownAnchorError::StaleRevision
        ))
    ));
}

#[test]
fn selection_gestures_reject_late_links_and_keep_window_focus_and_copy_boundaries() {
    use vesta_gpui::{
        FontRole, InputOwnership, InputTarget, KeyChord, KeyModifiers, KeyStroke, LayoutEngine,
        Point, PointerButton, TextInputContext, Viewport,
    };
    let window = UiWindowId(11);
    let other = UiWindowId(12);
    let scope = ElementIdAllocator.allocate_scope().expect("gesture scope");
    let mut document =
        MarkdownView::parse("[first link](../next.md) tail\n\n中文 👨‍👩‍👧‍👦 Z").expect("document");
    let mut state = MarkdownInteraction::new(window, scope);
    let theme = WidgetTheme::dark();
    let mut context = TextInputContext::default();
    let font = include_bytes!("../../../vesta-gpui/tests/fixtures/MaterialIcons-Regular.ttf");
    context.register_font(std::sync::Arc::new(font.to_vec()));
    context.set_font_stack(FontRole::Ui, "Material Icons");
    let project = |document: &mut MarkdownView,
                   range: std::ops::Range<usize>,
                   context: &mut TextInputContext| {
        let mut layout = LayoutEngine
            .compute(
                &document.view(scope, range.clone(), theme),
                Viewport::new(400.0, 800.0, 1.0),
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
                    &document.view(scope, range.clone(), theme),
                    Viewport::new(400.0, 800.0, 1.0),
                )
                .expect("shaped layout");
        }
        layout
    };
    let layout = project(&mut document, 0..2, &mut context);
    let surface = MarkdownSurface {
        window,
        document: &document,
        layout: &layout,
        range: 0..2,
    };
    let mut input = InputOwnership::default();
    let link = document.links_in(window, 0..1).next().expect("link");
    let rect = document
        .link_rectangles(scope, window, &layout, link)
        .expect("link geometry")[0];
    let point = Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5);
    assert_eq!(
        state.press(&surface, point, false, &mut input),
        Ok(MarkdownPointerResult::Selection)
    );
    let pressed = state.selection();
    assert_eq!(
        input.capture().map(|capture| capture.target),
        Some(state.target())
    );
    let wrong = MarkdownSurface {
        window: other,
        document: &document,
        layout: &layout,
        range: 0..2,
    };
    assert_eq!(
        state.release(&wrong, point, PointerButton::Primary, &mut input),
        Ok(MarkdownPointerResult::Ignored)
    );
    assert_eq!(
        state.release(&surface, point, PointerButton::Secondary, &mut input),
        Ok(MarkdownPointerResult::Ignored)
    );
    assert!(state.is_dragging());
    assert_eq!(
        state.release(&surface, point, PointerButton::Primary, &mut input),
        Ok(MarkdownPointerResult::Activate(link))
    );
    assert!(input.capture().is_none());
    state
        .press(&surface, point, false, &mut input)
        .expect("press link again");
    let second = document.block_id_at(1).expect("second block");
    let node = layout
        .node(document.paragraph_id(scope, second).expect("paragraph"))
        .expect("second layout");
    let end = Point::new(
        node.rect.x + node.rect.width - 1.0,
        node.rect.y + node.rect.height * 0.5,
    );
    state
        .pointer_move(&surface, end, &mut input)
        .expect("cross block drag");
    assert_ne!(state.selection(), pressed);
    assert_eq!(
        state.release(&surface, point, PointerButton::Primary, &mut input),
        Ok(MarkdownPointerResult::Selection),
        "dragging back to the pressed link must never activate it"
    );
    let projected = state.view(&document, 0..2, theme);
    let link_target = document
        .link_target(scope, window, link)
        .expect("projected target");
    let link_node = projected
        .find(link_target.element)
        .expect("independent semantic");
    assert_eq!(
        link_node.semantic.as_ref().expect("semantic").role,
        vesta_gpui::SemanticRole::Link
    );
    assert!(link_node.style.focusable && !link_node.style.pointer_events);
    let route =
        vesta_gpui::ActionRoute::<()>::build(&projected, Vec::new()).expect("unique identities");
    assert!(route.focus_order().contains(&link_target.element));
    let linked_layout = LayoutEngine
        .compute(&projected, Viewport::new(400.0, 800.0, 1.0))
        .expect("link overlay layout");
    let paragraph = document
        .paragraph_id(scope, document.block_id_at(0).expect("first"))
        .expect("paragraph");
    assert_eq!(
        linked_layout.node(paragraph).expect("new bounds").rect,
        layout.node(paragraph).expect("original bounds").rect
    );
    let surface = MarkdownSurface {
        window,
        document: &document,
        layout: &linked_layout,
        range: 0..2,
    };
    input.set_focus(Some(link_target));
    for key in [KeyStroke::Enter, KeyStroke::Space] {
        assert_eq!(
            state.key(
                &surface,
                KeyChord::new(key, KeyModifiers::default()),
                4096,
                &mut input
            ),
            Ok(Some(MarkdownKeyIntent::Activate(link)))
        );
    }
    assert_eq!(
        state.activate_target(
            &surface,
            InputTarget {
                window: other,
                element: link_target.element
            }
        ),
        Ok(None)
    );
    let off_page = MarkdownSurface {
        window,
        document: &document,
        layout: &linked_layout,
        range: 1..2,
    };
    assert_eq!(state.activate_target(&off_page, link_target), Ok(None));
    assert_eq!(
        state
            .resolve_activation(&surface, link)
            .expect("live intent")
            .expect("visible link")
            .destination,
        "../next.md"
    );
    input.set_focus(Some(state.target()));
    let command = KeyModifiers {
        control: true,
        ..KeyModifiers::default()
    };
    assert_eq!(
        state.key(
            &surface,
            KeyChord::character('a', command),
            4096,
            &mut input
        ),
        Ok(Some(MarkdownKeyIntent::Handled))
    );
    let all = state.selection();
    assert_eq!(
        state.key(
            &surface,
            KeyChord::character('c', command),
            4096,
            &mut input
        ),
        Ok(Some(MarkdownKeyIntent::Copy(
            "first link tail\n\n中文 👨‍👩‍👧‍👦 Z".into()
        )))
    );
    assert_eq!(
        state.key(&surface, KeyChord::character('x', command), 1, &mut input),
        Err(MarkdownProjectionError::Anchor(
            MarkdownAnchorError::CopyLimit { limit: 1 }
        ))
    );
    state
        .press(&surface, point, false, &mut input)
        .expect("page press");
    let page = project(&mut document, 1..2, &mut context);
    let next = MarkdownSurface {
        window,
        document: &document,
        layout: &page,
        range: 1..2,
    };
    assert!(state.sync(&next, &mut input));
    assert_eq!(
        state.selection(),
        all,
        "page change restores the accepted selection before press"
    );
    assert_eq!(
        state.release(&next, point, PointerButton::Primary, &mut input),
        Ok(MarkdownPointerResult::Ignored)
    );
    state
        .key(
            &next,
            KeyChord::new(KeyStroke::Home, KeyModifiers::default()),
            4096,
            &mut input,
        )
        .expect("block start");
    for _ in 0..3 {
        state
            .key(
                &next,
                KeyChord::new(KeyStroke::ArrowRight, KeyModifiers::default()),
                4096,
                &mut input,
            )
            .expect("before emoji");
    }
    let shift = KeyModifiers {
        shift: true,
        ..KeyModifiers::default()
    };
    state
        .key(
            &next,
            KeyChord::new(KeyStroke::ArrowRight, shift),
            4096,
            &mut input,
        )
        .expect("whole grapheme");
    assert_eq!(
        state.key(&next, KeyChord::character('c', command), 4096, &mut input),
        Ok(Some(MarkdownKeyIntent::Copy("👨‍👩‍👧‍👦".into())))
    );
    input.set_focus(None);
    assert_eq!(
        state.key(&next, KeyChord::character('a', command), 4096, &mut input),
        Ok(None)
    );
    assert_eq!(
        state.selection().expect("selection").focus.byte(),
        "中文 👨‍👩‍👧‍👦".len()
    );
    let root = state.view(&document, 1..2, theme);
    assert!(root.style.focusable);
    assert!(!document.view(scope, 1..2, theme).style.focusable);
    assert!(document.replace("replacement").expect("new revision"));
    let changed = project(&mut document, 0..1, &mut context);
    state.sync(
        &MarkdownSurface {
            window,
            document: &document,
            layout: &changed,
            range: 0..1,
        },
        &mut input,
    );
    assert!(state.selection().is_none());
    assert!(input.capture().is_none());
    drag_scroll::verify_owned_stationary_drag();
}

#[path = "tests/drag_scroll.rs"]
mod drag_scroll;
#[path = "tests/navigation.rs"]
mod navigation;
#[path = "tests/tables.rs"]
mod tables;
