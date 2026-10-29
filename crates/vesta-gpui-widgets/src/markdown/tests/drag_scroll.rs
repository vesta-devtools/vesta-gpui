use super::*;
use std::time::{Duration, Instant};
use vesta_gpui::{
    ActionRoute, Dimension, Element, ElementKey, FocusScope, InputOwnership, KeyChord,
    KeyModifiers, LayoutEngine, Point, PointerButton, Style, TextInputContext, Viewport,
};

pub(super) fn verify_owned_stationary_drag() {
    let window = UiWindowId(41);
    let scope = ElementIdAllocator.allocate_scope().expect("drag scope");
    let parent = ElementIdAllocator.allocate_scope().expect("scroll scope");
    let source = (0..12)
        .map(|i| format!("row {i} alpha 中文👨‍👩‍👧‍👦 omega"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut document = MarkdownView::parse(&source).expect("long document");
    let theme = WidgetTheme::dark();
    let mut context = TextInputContext::default();
    context.register_font(std::sync::Arc::new(
        include_bytes!("../../../../vesta-gpui/tests/fixtures/MaterialIcons-Regular.ttf").to_vec(),
    ));
    context.set_font_stack(vesta_gpui::FontRole::Ui, "Material Icons");
    let project = |document: &mut MarkdownView, context: &mut TextInputContext, offset| {
        let mut result = None;
        for _ in 0..4 {
            let mut child = document.view(scope, 0..document.len(), theme);
            child.style.flex_shrink = 0.0;
            let root = Element::scroll_box(
                parent.element(0),
                ElementKey::new("scroll"),
                Point::new(0.0, offset),
            )
            .styled(
                Style::default()
                    .axis(vesta_gpui::Axis::Column)
                    .size(Dimension::Px(240.0), Dimension::Px(60.0)),
            )
            .child(child);
            let root = root.child(
                Element::container(parent.element(1), ElementKey::new("following.sibling")).styled(
                    Style::default()
                        .size(Dimension::Percent(1.0), Dimension::Px(120.0))
                        .flex(0.0, 0.0),
                ),
            );
            let layout = LayoutEngine
                .compute(&root, Viewport::new(240.0, 60.0, 1.0))
                .expect("clipped layout");
            if !document
                .sync_layout(scope, &layout, context, theme)
                .expect("shape")
            {
                result = Some((root, layout));
                break;
            }
        }
        result.expect("converged document")
    };
    let (_, mut layout) = project(&mut document, &mut context, 0.0);
    let document_node = layout.node(scope.element(0)).expect("document bounds");
    let document_clip = document_node
        .rect
        .intersection(document_node.inherited_clip)
        .expect("document clip");
    let document_limit = document_node.rect.bottom() - document_clip.bottom();
    let first = document.block_id_at(0).expect("first");
    let (_, viewport) = document
        .block_viewport(scope, &layout, first)
        .expect("first viewport");
    assert!(viewport.clip.width > 0.0 && viewport.clip.height > 0.0);
    let point = Point::new(
        viewport.clip.x + viewport.clip.width * 0.5,
        viewport.clip.y + viewport.clip.height * 0.5,
    );
    let held = Point::new(point.x, 140.0);
    let mut state = MarkdownInteraction::new(window, scope);
    let mut input = InputOwnership::default();
    let mut now = Instant::now();
    let surface = MarkdownSurface {
        window,
        document: &document,
        layout: &layout,
        range: 0..document.len(),
    };
    state
        .press(&surface, point, false, &mut input)
        .expect("press");
    let anchor = state.selection().expect("start").anchor;
    state
        .pointer_move(&surface, held, &mut input)
        .expect("held edge");
    assert_eq!(state.drag_scroll_step(&surface, now, &mut input), Ok(None));
    let deadline = state.drag_scroll_deadline().expect("edge wakeup");
    assert!(deadline > now);
    let foreign = MarkdownSurface {
        window: UiWindowId(42),
        document: &document,
        layout: &layout,
        range: 0..document.len(),
    };
    assert_eq!(
        state.drag_scroll_step(&foreign, now + Duration::from_secs(1), &mut input),
        Ok(None)
    );
    assert_eq!(
        state.drag_scroll_deadline(),
        Some(deadline),
        "a foreign tick cannot advance the owner"
    );
    assert_eq!(
        state.drag_scroll_step(&surface, now, &mut input),
        Ok(None),
        "no step before its deadline"
    );
    now += Duration::from_secs(1);
    let step = state
        .drag_scroll_step(&surface, now, &mut input)
        .expect("late step")
        .expect("bounded catchup");
    assert_eq!(step.x, 0.0);
    assert!(
        step.y > 0.0 && step.y <= 30.0,
        "a delayed event loop cannot jump across text"
    );
    let mut scroll = crate::ScrollAreaState::default();
    scroll.sync_layout(layout.node(parent.element(0)).expect("scroll bounds"));
    scroll.scroll_by_point(step);
    for _ in 0..250 {
        let (root, next_layout) = project(&mut document, &mut context, scroll.offset_y());
        layout = next_layout;
        let surface = MarkdownSurface {
            window,
            document: &document,
            layout: &layout,
            range: 0..document.len(),
        };
        now += Duration::from_millis(20);
        if let Some(step) = state
            .drag_scroll_step(&surface, now, &mut input)
            .expect("live tick")
        {
            let route = ActionRoute::<()>::build(&root, Vec::new()).expect("route");
            let path =
                crate::ScrollInputPath::new(&FocusScope::new(window, &route, None), state.target())
                    .expect("scroll path");
            let remaining = path.scroll(step, |id, remaining| {
                if id == parent.element(0) {
                    scroll.scroll_by_point(remaining)
                } else {
                    remaining
                }
            });
            if remaining == step {
                state.stop_drag_scroll();
                break;
            }
        }
        if state.drag_scroll_deadline().is_none() {
            break;
        }
    }
    assert!(
        scroll.offset_y() > 60.0,
        "stationary dragging reveals more than one viewport"
    );
    assert!(
        scroll.offset_y() < scroll.max_y(),
        "reaching the document edge must not scroll its following sibling"
    );
    assert!(
        scroll.offset_y() <= document_limit + 0.01,
        "the final step overshot the document by {}px",
        scroll.offset_y() - document_limit
    );
    assert_eq!(state.selection().expect("range").anchor, anchor);
    assert_ne!(
        state.selection().expect("advanced focus").focus.block(),
        first
    );
    assert!(
        state.drag_scroll_deadline().is_none(),
        "the boundary stops the idle timer"
    );
    let surface = MarkdownSurface {
        window,
        document: &document,
        layout: &layout,
        range: 0..document.len(),
    };
    assert_eq!(
        state.drag_scroll_step(&surface, now + Duration::from_secs(2), &mut input),
        Ok(None)
    );
    state
        .pointer_move(&surface, Point::new(held.x, -50.0), &mut input)
        .expect("reverse edge");
    assert_eq!(state.drag_scroll_step(&surface, now, &mut input), Ok(None));
    assert!(
        state.drag_scroll_deadline().is_some(),
        "a real move rearms the gesture"
    );
    let (_, near_start) = project(&mut document, &mut context, 3.0);
    let surface = MarkdownSurface {
        window,
        document: &document,
        layout: &near_start,
        range: 0..document.len(),
    };
    let document_node = near_start
        .node(scope.element(0))
        .expect("near-start bounds");
    let document_clip = document_node
        .rect
        .intersection(document_node.inherited_clip)
        .expect("near-start clip");
    let remaining = document_clip.y - document_node.rect.y;
    assert!(remaining > 0.0 && remaining < 12.0);
    let reverse = state
        .drag_scroll_step(&surface, now + Duration::from_millis(20), &mut input)
        .expect("reverse tick")
        .expect("remaining text above");
    assert!(
        reverse.y < 0.0 && reverse.y >= -remaining - 0.01,
        "the final reverse step cannot enter preceding content"
    );
    input.cancel_capture();
    state
        .drag_scroll_step(&surface, now + Duration::from_secs(1), &mut input)
        .expect("lost capture");
    assert!(!state.is_dragging());
    assert!(state.drag_scroll_deadline().is_none());
    assert!(input.capture().is_none());
    assert_eq!(
        state.selection(),
        None,
        "capture loss keeps existing pre-press rollback"
    );
    assert_eq!(
        state.release(&surface, held, PointerButton::Primary, &mut input),
        Ok(MarkdownPointerResult::Ignored)
    );
    state
        .key(
            &surface,
            KeyChord::character(
                'a',
                KeyModifiers {
                    control: true,
                    ..Default::default()
                },
            ),
            64 * 1024,
            &mut input,
        )
        .expect("accepted selection before the failed gesture");
    let accepted = state.selection().expect("accepted full range");
    state
        .press(&surface, point, false, &mut input)
        .expect("fault press");
    state
        .pointer_move(&surface, held, &mut input)
        .expect("fault edge");
    state
        .drag_scroll_step(&surface, now, &mut input)
        .expect("armed owner");
    let deadline = state.drag_scroll_deadline().expect("fault deadline");
    let mut child = document.view(scope, 0..document.len(), theme);
    child.style.flex_shrink = 0.0;
    let unshaped = Element::scroll_box(
        parent.element(0),
        ElementKey::new("scroll"),
        Point::default(),
    )
    .styled(Style::default().size(Dimension::Px(180.0), Dimension::Px(60.0)))
    .child(child);
    let stale = LayoutEngine
        .compute(&unshaped, Viewport::new(180.0, 60.0, 1.0))
        .expect("new width before text layout converges");
    let stale_surface = MarkdownSurface {
        window,
        document: &document,
        layout: &stale,
        range: 0..document.len(),
    };
    assert!(matches!(
        state.drag_scroll_step(
            &stale_surface,
            deadline + Duration::from_millis(1),
            &mut input
        ),
        Err(MarkdownProjectionError::StaleWidth(_))
    ));
    assert!(
        state.drag_scroll_deadline().is_none(),
        "projection failure must not retain an expired Host wakeup"
    );
    assert!(!state.is_dragging() && input.capture().is_none());
    assert_eq!(
        state.selection(),
        Some(accepted),
        "failed projection restores the accepted range"
    );
    assert_eq!(
        state.drag_scroll_step(
            &stale_surface,
            deadline + Duration::from_secs(1),
            &mut input
        ),
        Ok(None),
        "the retired gesture cannot repeatedly fail at the stale width"
    );
}
