use super::*;

pub(super) fn qualify() {
    let source = "before\n\n| Name | Value |\n| :--- | ---: |\n| **杉木😀** | [123](../safe) |\n| `a\\|b` | |\n\nafter";
    let document = MarkdownView::parse(source).expect("table document");
    assert_eq!(
        document.len(),
        5,
        "prose and complete table rows are page units"
    );
    let window = UiWindowId(41);
    let selection = document
        .select_all(window)
        .expect("selection")
        .expect("text");
    assert_eq!(
        document
            .copy_selection(window, selection, 4096)
            .expect("copy"),
        "before\n\nName\tValue\n杉木😀\t123\na|b\t\n\nafter"
    );
    let scope = ElementIdAllocator.allocate_scope().expect("table scope");
    let view = document.view(scope, 1..2, WidgetTheme::dark());
    assert_eq!(roles(&view, vesta_gpui::SemanticRole::Table), 1);
    assert_eq!(roles(&view, vesta_gpui::SemanticRole::Row), 1);
    assert_eq!(
        roles(&view, vesta_gpui::SemanticRole::ColumnHeader),
        2,
        "paging must retain a complete header row"
    );
    let unique = document.block_id_at(2).expect("body row identity");
    let intent = document.links_in(window, 2..3).next().expect("cell link");
    assert_eq!(
        document.resolve_link(window, intent).expect("link").label,
        "123"
    );
    for theme in [WidgetTheme::dark(), WidgetTheme::light()] {
        for direction in [
            vesta_gpui::FlowDirection::LeftToRight,
            vesta_gpui::FlowDirection::RightToLeft,
        ] {
            for width in [180.0, 480.0] {
                geometry(theme, direction, width);
            }
        }
    }
    let mut document = document;
    document
        .replace(&format!("inserted\n\n{source}"))
        .expect("insert prose");
    assert_eq!(
        document.block_id_at(3),
        Some(unique),
        "unique cells retain stable identity across unrelated insertion"
    );
    assert!(matches!(
        document.resolve_link(window, intent),
        Err(MarkdownAnchorError::StaleRevision)
    ));
    let ragged = MarkdownView::parse(
        "| A | B | C |\n| --- | --- | --- |\n| short |\n| a | b | c | ignored |\n",
    )
    .expect("GFM column normalization");
    let all = ragged.select_all(window).expect("all").expect("rows");
    assert_eq!(
        ragged
            .copy_selection(window, all, 4096)
            .expect("normalized cells"),
        "A\tB\tC\nshort\t\t\na\tb\tc"
    );
}

fn roles(root: &vesta_gpui::Element, role: vesta_gpui::SemanticRole) -> usize {
    usize::from(root.semantic.as_ref().is_some_and(|s| s.role == role))
        + root
            .children
            .iter()
            .map(|child| roles(child, role))
            .sum::<usize>()
}

fn geometry(theme: WidgetTheme, direction: vesta_gpui::FlowDirection, width: f32) {
    use vesta_gpui::*;
    let source = "before\n\n| Left | Center | Right |\n| :--- | :---: | ---: |\n| [\u{e88a}](../safe) | **\u{e88a}** | `\u{e88a}` |\n| 杉木😀 | | مرحبا |\n\nafter";
    let mut document = MarkdownView::parse(source).expect("styled cells");
    let scope = ElementIdAllocator.allocate_scope().expect("scope");
    let window = UiWindowId(41);
    let mut input = MarkdownInteraction::new(window, scope);
    let mut context = TextInputContext::default();
    assert!(context.register_font(std::sync::Arc::new(
        include_bytes!("../../../../vesta-gpui/tests/fixtures/MaterialIcons-Regular.ttf").to_vec()
    )));
    for role in [FontRole::Ui, FontRole::Mono] {
        context.set_font_stack(role, "Material Icons");
    }
    let viewport = Viewport::new(width, 1000.0, 1.25);
    let build = |document: &MarkdownView, input: &MarkdownInteraction| {
        let mut tree = input.view(document, 0..document.len(), theme);
        tree.style.flow_direction = Some(direction);
        tree
    };
    let mut layout = LayoutEngine
        .compute(&build(&document, &input), viewport)
        .expect("table layout");
    for _ in 0..6 {
        if !document
            .sync_layout(scope, &layout, &mut context, theme)
            .expect("shape cells")
        {
            break;
        }
        layout = LayoutEngine
            .compute(&build(&document, &input), viewport)
            .expect("shaped cells");
    }
    assert!(
        !document
            .sync_layout(scope, &layout, &mut context, theme)
            .expect("converged")
    );
    let tree = build(&document, &input);
    assert_eq!(roles(&tree, SemanticRole::Row), 3);
    assert_eq!(roles(&tree, SemanticRole::Cell), 6);
    let cells = document.blocks_in(2..3).collect::<Vec<_>>();
    let mut starts = Vec::new();
    for block in &cells {
        let (snapshot, viewport) = document
            .block_viewport(scope, &layout, block.id)
            .expect("cell snapshot");
        let rectangles = snapshot
            .range_rectangles(0..block.content.text.len())
            .expect("glyph");
        assert!(!rectangles.is_empty() && rectangles[0].width > 0.0);
        starts.push(rectangles[0].x);
        assert!(
            layout
                .node(scope.element(block.id.0))
                .expect("cell bounds")
                .rect
                .contains_rect(viewport.clip)
        );
    }
    assert!(
        starts[0] < starts[1] && starts[1] < starts[2],
        "alignment must affect actual glyphs in both flows"
    );
    let link = document.links_in(window, 2..3).next().expect("cell link");
    let rectangle = document
        .link_rectangles(scope, window, &layout, link)
        .expect("link bounds")[0];
    let point = Point::new(
        rectangle.x + rectangle.width * 0.5,
        rectangle.y + rectangle.height * 0.5,
    );
    assert_eq!(
        document
            .link_at_point(scope, window, &layout, 2..3, point)
            .expect("hit"),
        Some(link)
    );
    let surface = MarkdownSurface {
        window,
        document: &document,
        layout: &layout,
        range: 0..document.len(),
    };
    let mut ownership = InputOwnership::default();
    assert_eq!(
        input
            .press(&surface, point, false, &mut ownership)
            .expect("press"),
        MarkdownPointerResult::Selection
    );
    input.cancel(&mut ownership);
    input.sync(&surface, &mut ownership);
    let position = ReadOnlyTextPosition {
        element: scope.element(cells[1].id.0 + 2),
        byte: 0,
    };
    input
        .accept_accessible_selection(
            &surface,
            ReadOnlyDocumentSelection {
                root: input.target().element,
                revision: document.revision().0,
                anchor: position,
                focus: position,
            },
            &mut ownership,
        )
        .expect("focus center cell");
    let empty = document
        .blocks_in(3..4)
        .nth(1)
        .expect("empty center cell")
        .id;
    for (key, expected) in [
        (KeyStroke::ArrowDown, empty),
        (KeyStroke::ArrowUp, cells[1].id),
    ] {
        assert_eq!(
            input
                .key(
                    &surface,
                    KeyChord {
                        key,
                        modifiers: KeyModifiers::default()
                    },
                    4096,
                    &mut ownership
                )
                .expect("vertical cell navigation"),
            Some(MarkdownKeyIntent::RevealSelection)
        );
        assert_eq!(
            input.selection().expect("cell selection").focus.block(),
            expected,
            "vertical navigation retains the visual column, including an empty cell"
        );
        let projection = input
            .accessibility(&surface)
            .expect("current cell selection");
        let tree = build(&document, &input);
        let mut semantics = AccessibilityTree::default();
        let mut update = semantics.build(
            &tree,
            &layout,
            Some(input.target().element),
            "Selected cell",
        );
        semantics
            .enrich_read_only_document(&mut update, projection)
            .expect("native selection in cell, including empty cell");
    }
    let projection = input.accessibility(&surface).expect("same cell sources");
    let mut native = String::new();
    for (index, text) in projection.paragraphs.iter().enumerate() {
        native.push_str(text.source);
        if index + 1 < projection.paragraphs.len() {
            native.push_str(
                text.separator_after
                    .unwrap_or(projection.paragraph_separator),
            );
        }
    }
    let all = document.select_all(window).expect("all").expect("range");
    assert_eq!(
        native,
        document.copy_selection(window, all, 4096).expect("copy")
    );
    let selected = document
        .selected_view(scope, window, 0..document.len(), all, theme)
        .expect("selected cells");
    assert!(selected.find(scope.element(cells[1].id.0 + 2)).is_some());
    let mut semantics = AccessibilityTree::default();
    let mut update = semantics.build(&tree, &layout, Some(input.target().element), "Table");
    semantics
        .enrich_read_only_document(&mut update, projection)
        .expect("native table paragraphs, including empty cells");
}
