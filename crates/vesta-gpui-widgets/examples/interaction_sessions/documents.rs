//! Bounded pages are caller policy; selection and logical table rows stay shared.
use super::{debug_error, require};
use std::{
    ops::Range,
    time::{Duration, Instant},
};
use vesta_gpui::{
    ActionRoute, Dimension, Element, ElementIdAllocator, ElementKey, ElementScope, FocusScope,
    InputOwnership, KeyChord, KeyModifiers, KeyStroke, LayoutTree, Point, PointerButton,
    RetainedLayoutEngine, SemanticRole, Style, TextInputContext, UiWindowId, Viewport,
};
use vesta_gpui_widgets::{
    MarkdownAnchorError, MarkdownInteraction, MarkdownKeyIntent, MarkdownPointerResult,
    MarkdownSurface, MarkdownView, ScrollAreaState, ScrollInputPath, WidgetTheme,
};

const PAGE_ROWS: usize = 4;

struct Projection {
    scope: ElementScope,
    interaction: MarkdownInteraction,
    input: InputOwnership,
    rows: Range<usize>,
    width: f32,
    layout: RetainedLayoutEngine,
}

impl Projection {
    fn new(window: UiWindowId) -> Result<Self, String> {
        let scope = ElementIdAllocator.allocate_scope().map_err(debug_error)?;
        let interaction = MarkdownInteraction::new(window, scope);
        let mut input = InputOwnership::default();
        input.set_focus(Some(interaction.target()));
        Ok(Self {
            scope,
            interaction,
            input,
            rows: 0..PAGE_ROWS,
            width: 480.0,
            layout: Default::default(),
        })
    }

    fn surface<'a>(
        &self,
        document: &'a MarkdownView,
        layout: &'a LayoutTree,
    ) -> MarkdownSurface<'a> {
        MarkdownSurface {
            window: self.interaction.target().window,
            document,
            layout,
            range: self.rows.clone(),
        }
    }

    fn frame(
        &mut self,
        document: &mut MarkdownView,
        context: &mut TextInputContext,
    ) -> Result<LayoutTree, String> {
        self.frame_with_scroll(document, context, None)
            .map(|(_, layout)| layout)
    }

    fn frame_with_scroll(
        &mut self,
        document: &mut MarkdownView,
        context: &mut TextInputContext,
        scroll: Option<(ElementScope, Point)>,
    ) -> Result<(Element, LayoutTree), String> {
        let theme = WidgetTheme::light();
        for _ in 0..5 {
            let mut root = self.interaction.view(document, self.rows.clone(), theme);
            if let Some((scope, offset)) = scroll {
                root.style.flex_shrink = 0.0;
                root = Element::scroll_box(
                    scope.element(0),
                    ElementKey::new("document.scroll"),
                    offset,
                )
                .styled(
                    Style::default()
                        .axis(vesta_gpui::Axis::Column)
                        .size(Dimension::Px(self.width), Dimension::Px(96.0)),
                )
                .child(root)
                .child(
                    Element::container(scope.element(1), ElementKey::new("following.content"))
                        .styled(
                            Style::default()
                                .size(Dimension::Px(self.width), Dimension::Px(120.0))
                                .flex(0.0, 0.0),
                        ),
                );
            }
            let layout = self
                .layout
                .compute_with_text_context(&root, Viewport::new(self.width, 500.0, 1.0), context)
                .map_err(debug_error)?;
            if !document
                .sync_layout(self.scope, &layout, context, theme)
                .map_err(debug_error)?
            {
                let surface = self.surface(document, &layout);
                self.interaction.sync(&surface, &mut self.input);
                return Ok((root, layout));
            }
        }
        Err("bounded document page failed to settle".into())
    }

    fn key(
        &mut self,
        document: &MarkdownView,
        layout: &LayoutTree,
        key: KeyStroke,
        modifiers: KeyModifiers,
    ) -> Result<(), String> {
        let surface = self.surface(document, layout);
        let intent = self
            .interaction
            .key(
                &surface,
                KeyChord::new(key, modifiers),
                64 * 1024,
                &mut self.input,
            )
            .map_err(debug_error)?;
        require(
            intent == Some(MarkdownKeyIntent::RevealSelection),
            "navigation not admitted",
        )
    }

    fn page_for_focus(&mut self, document: &MarkdownView) -> Result<usize, String> {
        let selection = self.interaction.selection().ok_or("no logical selection")?;
        let row = document
            .selection_focus_row(self.interaction.target().window, selection)
            .map_err(debug_error)?;
        let start = row / PAGE_ROWS * PAGE_ROWS;
        self.rows = start..(start + PAGE_ROWS).min(document.len());
        Ok(row)
    }
}

pub(super) fn qualify(context: &mut TextInputContext) -> Result<(), String> {
    let window = UiWindowId(37);
    let paragraphs = (0..61)
        .map(|index| format!("Section {index}: 中文 English مرحبا 👨‍👩‍👧‍👦"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let source = format!(
        "{paragraphs}\n\n| 项目 | 数值 | 说明 |\n| :--- | ---: | :---: |\n\
         | Seed | 42 | **固定** |\n\
         | Origin | `0, 0` | [帮助](https://example.invalid/table) |\n\
         | Empty | | مرحبا 👨‍👩‍👧‍👦 |\n"
    );
    let expected = format!(
        "{paragraphs}\n\n项目\t数值\t说明\nSeed\t42\t固定\nOrigin\t0, 0\t帮助\nEmpty\t\tمرحبا 👨‍👩‍👧‍👦"
    );
    let mut document = MarkdownView::parse(&source).map_err(debug_error)?;
    let mut projection = Projection::new(window)?;
    let first = projection.frame(&mut document, context)?;
    let plain = KeyModifiers::default();
    let command = KeyModifiers {
        control: true,
        ..plain
    };
    projection.key(
        &document,
        &first,
        KeyStroke::End,
        KeyModifiers {
            shift: true,
            ..command
        },
    )?;
    let selected = projection
        .interaction
        .selection()
        .ok_or("missing end selection")?;
    require(
        projection
            .interaction
            .selection_focus_rect(&projection.surface(&document, &first))
            .map_err(debug_error)?
            .is_none(),
        "off-page focus reused stale geometry",
    )?;
    require(
        document
            .copy_selection(window, selected, 64 * 1024)
            .map_err(debug_error)?
            == expected,
        "unshaped pages changed whole-document copy order",
    )?;
    let last_row = projection.page_for_focus(&document)?;
    require(
        last_row + 1 == document.len(),
        "final cell was confused with a display row",
    )?;
    let last = projection.frame(&mut document, context)?;
    let accessible = projection
        .interaction
        .accessibility(&projection.surface(&document, &last))
        .map_err(debug_error)?;
    let original_paragraphs = accessible
        .paragraphs
        .iter()
        .map(|text| text.element)
        .collect::<Vec<_>>();
    require(
        accessible.paragraphs.len() == 3
            && projection.interaction.selection() == Some(selected)
            && projection
                .interaction
                .selection_focus_rect(&projection.surface(&document, &last))
                .map_err(debug_error)?
                .is_some(),
        "bounded table page split cells or lost its accepted selection",
    )?;
    let table =
        projection
            .interaction
            .view(&document, projection.rows.clone(), WidgetTheme::light());
    require(
        table.children.len() == 1
            && table.children[0]
                .semantic
                .as_ref()
                .is_some_and(|s| s.role == SemanticRole::Table)
            && table.children[0].children.len() == 1
            && table.children[0].children[0].children.len() == 3,
        "one logical page row did not project one complete table row",
    )?;
    projection.width = 180.0;
    let narrow = projection.frame(&mut document, context)?;
    require(
        projection
            .interaction
            .accessibility(&projection.surface(&document, &narrow))
            .map_err(debug_error)?
            .paragraphs
            .iter()
            .map(|text| text.element)
            .collect::<Vec<_>>()
            == original_paragraphs
            && projection.interaction.selection() == Some(selected),
        "narrow reflow changed stable selection identity",
    )?;
    projection.key(&document, &narrow, KeyStroke::End, command)?;
    // Walk backward through the final, empty middle and first cells, then cross
    // the page boundary. All three cells first resolve to the same logical row.
    for _ in 0..3 {
        projection.key(&document, &narrow, KeyStroke::Home, plain)?;
        projection.key(&document, &narrow, KeyStroke::ArrowLeft, plain)?;
        let row = document
            .selection_focus_row(
                window,
                projection
                    .interaction
                    .selection()
                    .ok_or("backward selection missing")?,
            )
            .map_err(debug_error)?;
        if row != last_row {
            require(
                row + 1 == last_row,
                "backward navigation skipped a logical row",
            )?;
            break;
        }
    }
    require(
        projection.page_for_focus(&document)? + 1 == last_row,
        "failed to cross bounded page",
    )?;
    let previous = projection.frame(&mut document, context)?;
    require(
        projection
            .interaction
            .selection_focus_rect(&projection.surface(&document, &previous))
            .map_err(debug_error)?
            .is_some(),
        "new previous page cannot reveal its focus",
    )?;
    projection.key(&document, &previous, KeyStroke::Home, command)?;
    require(
        projection.page_for_focus(&document)? == 0,
        "document Home failed to return to first page",
    )?;
    let first_again = projection.frame(&mut document, context)?;
    require(
        projection
            .interaction
            .selection_focus_rect(&projection.surface(&document, &first_again))
            .map_err(debug_error)?
            .is_some(),
        "first page lost current reveal geometry",
    )?;
    require(
        document.selection_focus_row(UiWindowId(38), selected)
            == Err(MarkdownAnchorError::WrongWindow)
            && MarkdownView::parse("Other document")
                .map_err(debug_error)?
                .selection_focus_row(window, selected)
                == Err(MarkdownAnchorError::WrongDocument),
        "foreign selection was allowed to choose a page",
    )?;
    document
        .replace(&format!("New revision\n\n{source}"))
        .map_err(debug_error)?;
    require(
        document.selection_focus_row(window, selected) == Err(MarkdownAnchorError::StaleRevision),
        "old revision was allowed to choose a current page",
    )?;
    projection.frame(&mut document, context)?;
    require(
        projection.interaction.selection().is_none() && projection.input.capture().is_none(),
        "source replacement retained selection or capture",
    )?;
    println!(
        "Bounded mixed/table pages retain whole rows, stable selection, copy order and current reveal across End/Home, reverse cell traversal and narrow reflow; stale/foreign pages reject."
    );
    qualify_drag_scroll(context)?;
    Ok(())
}

fn qualify_drag_scroll(context: &mut TextInputContext) -> Result<(), String> {
    let source = (0..16)
        .map(|row| format!("Row {row}: 中文 English مرحبا 👨‍👩‍👧‍👦"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut document = MarkdownView::parse(&source).map_err(debug_error)?;
    let mut projection = Projection::new(UiWindowId(43))?;
    projection.rows = 0..document.len();
    projection.width = 240.0;
    let scroll_scope = ElementIdAllocator.allocate_scope().map_err(debug_error)?;
    let mut scroll = ScrollAreaState::default();
    let (_, layout) = projection.frame_with_scroll(
        &mut document,
        context,
        Some((scroll_scope, scroll.offset())),
    )?;
    scroll.sync_layout(
        layout
            .node(scroll_scope.element(0))
            .ok_or("missing document scroll")?,
    );
    let first = document.block_id_at(0).ok_or("missing first block")?;
    let first_paragraph = document
        .paragraph_id(projection.scope, first)
        .map_err(debug_error)?;
    let first_node = layout
        .node(first_paragraph)
        .ok_or("missing first paragraph")?;
    let clip = first_node
        .rect
        .intersection(first_node.inherited_clip)
        .ok_or("missing first paragraph clip")?;
    let origin = Point::new(clip.x + 8.0, clip.y + clip.height * 0.5);
    let target = projection.interaction.target();
    let surface = MarkdownSurface {
        window: target.window,
        document: &document,
        layout: &layout,
        range: projection.rows.clone(),
    };
    require(
        projection
            .interaction
            .press(&surface, origin, false, &mut projection.input)
            .map_err(debug_error)?
            == MarkdownPointerResult::Selection,
        "document drag was not admitted",
    )?;
    let anchor = projection
        .interaction
        .accessibility(&surface)
        .map_err(debug_error)?
        .selection
        .ok_or("missing drag selection")?
        .anchor;
    let mut now = Instant::now();
    let mut down_offset = 0.0;
    for direction in [1.0, -1.0] {
        let (_, layout) = projection.frame_with_scroll(
            &mut document,
            context,
            Some((scroll_scope, scroll.offset())),
        )?;
        let bounds = layout
            .node(scroll_scope.element(0))
            .ok_or("missing scroll viewport")?
            .rect;
        let held = Point::new(
            origin.x,
            if direction > 0.0 {
                bounds.bottom() + 40.0
            } else {
                bounds.y - 40.0
            },
        );
        let surface = MarkdownSurface {
            window: target.window,
            document: &document,
            layout: &layout,
            range: projection.rows.clone(),
        };
        projection
            .interaction
            .pointer_move(&surface, held, &mut projection.input)
            .map_err(debug_error)?;
        require(
            projection
                .interaction
                .drag_scroll_step(&surface, now, &mut projection.input)
                .map_err(debug_error)?
                .is_none(),
            "new direction scrolled before its deadline",
        )?;
        let deadline = projection
            .interaction
            .drag_scroll_deadline()
            .ok_or("missing edge deadline")?;
        let foreign = MarkdownSurface {
            window: UiWindowId(44),
            document: &document,
            layout: &layout,
            range: projection.rows.clone(),
        };
        require(
            projection
                .interaction
                .drag_scroll_step(
                    &foreign,
                    now + Duration::from_secs(1),
                    &mut projection.input,
                )
                .map_err(debug_error)?
                .is_none()
                && projection.interaction.drag_scroll_deadline() == Some(deadline),
            "foreign window advanced or cleared document scrolling",
        )?;
        for _ in 0..1000 {
            let (root, layout) = projection.frame_with_scroll(
                &mut document,
                context,
                Some((scroll_scope, scroll.offset())),
            )?;
            let surface = MarkdownSurface {
                window: target.window,
                document: &document,
                layout: &layout,
                range: projection.rows.clone(),
            };
            now += Duration::from_millis(20);
            if let Some(step) = projection
                .interaction
                .drag_scroll_step(&surface, now, &mut projection.input)
                .map_err(debug_error)?
            {
                require(
                    step.x == 0.0
                        && step.y.is_finite()
                        && step.y.abs() <= 30.0
                        && step.y.signum() == direction,
                    "edge scrolling escaped its finite directional bound",
                )?;
                let route = ActionRoute::<()>::build(&root, Vec::new()).map_err(debug_error)?;
                let path =
                    ScrollInputPath::new(&FocusScope::new(target.window, &route, None), target)
                        .ok_or("missing admitted document scroll path")?;
                let remaining = path.scroll(step, |id, movement| {
                    if id == scroll_scope.element(0) {
                        scroll.scroll_by_point(movement)
                    } else {
                        movement
                    }
                });
                if remaining == step {
                    projection.interaction.stop_drag_scroll();
                }
            }
            if projection.interaction.drag_scroll_deadline().is_none() {
                break;
            }
        }
        require(
            projection.interaction.drag_scroll_deadline().is_none()
                && projection.interaction.is_dragging(),
            "document boundary retained idle work or replaced its stable anchor",
        )?;
        let (_, layout) = projection.frame_with_scroll(
            &mut document,
            context,
            Some((scroll_scope, scroll.offset())),
        )?;
        let selected = projection
            .interaction
            .selection()
            .ok_or("missing final selection")?;
        let surface = MarkdownSurface {
            window: target.window,
            document: &document,
            layout: &layout,
            range: projection.rows.clone(),
        };
        require(
            projection
                .interaction
                .accessibility(&surface)
                .map_err(debug_error)?
                .selection
                .is_some_and(|selection| selection.anchor == anchor),
            "public selection projection replaced its stable anchor",
        )?;
        let row = document
            .selection_focus_row(target.window, selected)
            .map_err(debug_error)?;
        if direction > 0.0 {
            down_offset = scroll.offset_y();
            require(
                row + 1 == document.len() && down_offset > 96.0 && down_offset < scroll.max_y(),
                "stationary selection failed to reach the last block or scrolled following content",
            )?;
            let node = layout
                .node(target.element)
                .ok_or("missing current document")?;
            let clip = node
                .rect
                .intersection(node.inherited_clip)
                .ok_or("missing current clip")?;
            require(
                node.rect.bottom() <= clip.bottom() + 0.05,
                "document end did not become visible",
            )?;
        } else {
            require(
                row == 0 && scroll.offset_y() <= 0.01 && down_offset > scroll.offset_y(),
                "reverse scrolling did not return to the first block",
            )?;
            let surface = MarkdownSurface {
                window: target.window,
                document: &document,
                layout: &layout,
                range: projection.rows.clone(),
            };
            require(
                projection
                    .interaction
                    .release(
                        &surface,
                        held,
                        PointerButton::Primary,
                        &mut projection.input,
                    )
                    .map_err(debug_error)?
                    == MarkdownPointerResult::Selection,
                "document drag release was not consumed",
            )?;
        }
    }
    require(
        projection.input.capture().is_none()
            && !projection.interaction.is_dragging()
            && projection.interaction.drag_scroll_deadline().is_none(),
        "release retained capture or wakeups",
    )?;
    let before = projection.interaction.selection();
    let (_, layout) = projection.frame_with_scroll(
        &mut document,
        context,
        Some((scroll_scope, scroll.offset())),
    )?;
    let surface = MarkdownSurface {
        window: target.window,
        document: &document,
        layout: &layout,
        range: projection.rows.clone(),
    };
    projection
        .interaction
        .press(&surface, origin, false, &mut projection.input)
        .map_err(debug_error)?;
    projection
        .interaction
        .pointer_move(&surface, Point::new(origin.x, 140.0), &mut projection.input)
        .map_err(debug_error)?;
    projection
        .interaction
        .drag_scroll_step(&surface, now, &mut projection.input)
        .map_err(debug_error)?;
    require(
        projection.interaction.drag_scroll_deadline().is_some(),
        "cancel fixture never armed its deadline",
    )?;
    require(
        projection
            .interaction
            .key(
                &surface,
                KeyChord::new(KeyStroke::Escape, KeyModifiers::default()),
                64 * 1024,
                &mut projection.input,
            )
            .map_err(debug_error)?
            == Some(MarkdownKeyIntent::Handled)
            && projection.interaction.selection() == before
            && projection.input.capture().is_none()
            && projection.interaction.drag_scroll_deadline().is_none(),
        "Escape did not restore the accepted range and retire its deadline",
    )?;
    println!(
        "Public Markdown retained-scroll consumer: stationary/reverse boundaries, stable anchor, foreign-source rejection, release and Escape cleanup passed. Caller-driven input/time only; physical edge-held acceptance remains separate."
    );
    Ok(())
}
