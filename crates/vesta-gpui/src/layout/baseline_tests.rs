use super::*;
use crate::{Style, TextHAlign, TextInputContext, TextVAlign, render::static_text_view};
use std::sync::Arc;

fn painted_baseline(
    root: &Element,
    tree: &LayoutTree,
    id: ElementId,
    context: &mut TextInputContext,
) -> f32 {
    let element = root.find(id).expect("text");
    let crate::ElementKind::Text(text) = &element.kind else {
        panic!("text expected")
    };
    let rect = tree.node(id).expect("layout").rect;
    let (snapshot, view) = static_text_view(
        context,
        rect,
        rect,
        text,
        element.style.visual.font_size,
        element.style.visual.font_role,
        element.style.text_wrap,
        TextHAlign::Start,
        element.style.text_v_align,
    )
    .expect("shaped view");
    view.origin.y + snapshot.first_baseline().expect("actual Parley baseline")
}

#[test]
fn shaped_first_baselines_match_paint_across_fixed_wrapped_nested_and_retained_layout() {
    let mut context = TextInputContext::default();
    assert!(context.register_font(Arc::new(
        include_bytes!("../../tests/fixtures/MaterialIcons-Regular.ttf").to_vec()
    )));
    context.set_font_stack(FontRole::Ui, "Material Icons");
    context.set_font_stack(FontRole::Icon, "Material Icons");
    let text = |id, size, role, content: &str| {
        Element::text(
            ElementId::new(id),
            ElementKey::new(format!("text.{id}")),
            content,
        )
        .styled(
            Style::default()
                .text(size, crate::Color::rgb8(230, 230, 230))
                .font_role(role)
                .text_align(TextHAlign::Start, TextVAlign::Start),
        )
    };
    let mut root = Element::container(ElementId::new(1), ElementKey::new("baseline.root"))
        .styled(
            Style::default()
                .axis(Axis::Row)
                .align(Align::Baseline)
                .gap(8.0),
        )
        .child(text(2, 14.0, FontRole::Ui, "Small"))
        .child({
            let mut text = text(3, 29.0, FontRole::Icon, "home");
            text.style.height = Dimension::Px(64.0);
            text.style.text_v_align = TextVAlign::End;
            text
        })
        .child(
            Element::container(ElementId::new(4), ElementKey::new("nested"))
                .styled(
                    Style::default()
                        .padding(Insets::all(4.0))
                        .align(Align::Start),
                )
                .child(text(5, 21.0, FontRole::Ui, "Nested"))
                .child(text(7, 9.0, FontRole::Ui, "second line")),
        )
        .child({
            let mut text = text(6, 18.0, FontRole::Ui, "Wrapped content\n中文");
            text.style.width = Dimension::Px(65.0);
            text.style.text_wrap = true;
            text
        });
    let viewport = Viewport::new(500.0, 180.0, 1.0);
    let mut retained = RetainedLayoutEngine::default();
    for align in [TextVAlign::End, TextVAlign::Center, TextVAlign::Baseline] {
        root.children[1].style.text_v_align = align;
        let reference = LayoutEngine
            .compute_with_text_context(&root, viewport, &mut context)
            .expect("stateless");
        let actual = retained
            .compute_with_text_context(&root, viewport, &mut context)
            .expect("retained");
        assert_eq!(actual.nodes(), reference.nodes());
        let first = painted_baseline(&root, &actual, ElementId::new(2), &mut context);
        for id in [3, 5, 6] {
            let baseline = painted_baseline(&root, &actual, ElementId::new(id), &mut context);
            assert!(
                (baseline - first).abs() <= 1.0,
                "rounded first baselines differ: {first} vs {baseline}, id {id}"
            );
        }
        let steady = retained
            .compute_with_text_context(&root, viewport, &mut context)
            .expect("steady");
        assert_eq!(actual.nodes(), steady.nodes());
        let measured = LayoutEngine
            .measure_with_text_context(&root, ContentConstraint::default(), &mut context)
            .expect("measure");
        let rect = actual.node(root.id).expect("root").rect;
        assert_eq!(measured.preferred, Size::new(rect.width, rect.height));
    }
    root.style.flow_direction = Some(FlowDirection::RightToLeft);
    let rtl = retained
        .compute_with_text_context(&root, viewport, &mut context)
        .expect("RTL");
    let first = painted_baseline(&root, &rtl, ElementId::new(2), &mut context);
    assert!((painted_baseline(&root, &rtl, ElementId::new(3), &mut context) - first).abs() <= 1.0);
    root.style = root
        .style
        .grid(vec![GridTrack::Px(110.0); 4], vec![GridTrack::Auto]);
    let grid = retained
        .compute_with_text_context(&root, viewport, &mut context)
        .expect("grid baseline");
    let first = painted_baseline(&root, &grid, ElementId::new(2), &mut context);
    for id in [3, 5, 6] {
        assert!(
            (painted_baseline(&root, &grid, ElementId::new(id), &mut context) - first).abs() <= 1.0
        );
    }
    assert_eq!(
        grid.nodes(),
        LayoutEngine
            .compute_with_text_context(&root, viewport, &mut context)
            .expect("grid reference")
            .nodes()
    );
    // Reflow uses logical pixels at every host scale. A wrapped mixed-role
    // child is bounded by the narrow line; retained measurement, painting and
    // hit geometry must keep the same owner after the viewport changes.
    let mut wrapped = root.clone();
    wrapped.style = Style::default()
        .axis(Axis::Row)
        .flex_wrap(FlexWrap::Wrap)
        .align(Align::Baseline)
        .max_width(Dimension::Percent(1.0));
    for child in &mut wrapped.children {
        child.style.max_width = Dimension::Percent(1.0);
        child.style.text_wrap = true;
    }
    for scale in [1.0, 1.25, 1.5, 2.0] {
        for width in [120.0, 320.0, 500.0, 120.0] {
            let viewport = Viewport::new(width, 800.0, scale);
            let actual = retained
                .compute_with_text_context(&wrapped, viewport, &mut context)
                .expect("scaled reflow");
            let reference = LayoutEngine
                .compute_with_text_context(&wrapped, viewport, &mut context)
                .expect("scaled reference");
            assert_eq!(actual.nodes(), reference.nodes());
            let bounds = actual.node(wrapped.id).expect("current root").rect;
            assert!(bounds.width <= width);
            for child in &wrapped.children {
                let frame = actual.node(child.id).expect("same child identity").rect;
                assert!(frame.x >= bounds.x && frame.right() <= bounds.right() + 0.01);
                if let crate::ElementKind::Text(text) = &child.kind {
                    let shaped = context
                        .shape_static_text_wrapped(
                            text,
                            child.style.visual.font_size,
                            child.style.visual.font_role,
                            Some(frame.width),
                        )
                        .expect("current wrapped text");
                    assert!(
                        frame.height + 1.0 >= shaped.layout_height(),
                        "wrapped text must fit its measured height: {} actual {} required {} at width {}",
                        child.key.as_str(),
                        frame.height,
                        shaped.layout_height(),
                        frame.width
                    );
                }
            }
            let paint = crate::PaintList::build(&wrapped, &actual);
            assert_eq!(paint, crate::PaintList::build(&wrapped, &reference));
        }
    }
    root.style.align_content = Align::Baseline;
    assert!(
        matches!(retained.compute_with_text_context(&root, viewport, &mut context), Err(LayoutError::InvalidBaselineAlignment(id)) if id == root.id)
    );
    root.style.align_content = Align::Start;
    assert!(matches!(
        LayoutEngine.compute_with_text_metrics(&root, viewport, |request| {
            let mut metrics = shaped_metrics(&mut context, request);
            if request.height.is_some() {
                metrics.first_baseline = Some(f32::NAN);
            }
            metrics
        }),
        Err(LayoutError::InvalidTextMetrics(_))
    ));
    retained
        .compute_with_text_context(&root, viewport, &mut context)
        .expect("recover");
}
