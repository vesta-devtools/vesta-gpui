use vesta_gpui::{
    ContentConstraint, Dimension, Element, ElementId, ElementKey, ElementKind, GridTrack,
    LayoutEngine, LayoutError, Point, Rect, RetainedLayoutEngine, Style, TextInputContext,
    Viewport, render,
};

#[test]
fn invalid_rectangles_cannot_reenter_visible_geometry() {
    let viewport = Viewport::new(100.0, 100.0, 1.25);
    let clip = viewport.logical_rect();
    for invalid in [
        Rect::new(f32::NAN, 0.0, 20.0, 20.0),
        Rect::new(0.0, 0.0, f32::NAN, 20.0),
        Rect::new(f32::NEG_INFINITY, 0.0, f32::INFINITY, 20.0),
        Rect::new(f32::MAX, 0.0, f32::MAX, 20.0),
        Rect::new(f32::MAX, 0.0, 20.0, 20.0),
    ] {
        assert!(
            invalid.intersection(clip).is_none() && clip.intersection(invalid).is_none(),
            "invalid rectangle must not become a visible region: {invalid:?}"
        );
        assert!(render::scissor(invalid, viewport).is_none());
        assert!(!invalid.contains(Point::new(10.0, 10.0)));
        assert!(!invalid.contains(Point::new(invalid.x, invalid.y)));
        assert!(!invalid.contains_rect(Rect::new(10.0, 10.0, 1.0, 1.0)));
    }
    let negative_origin = Rect::new(-10.0, -10.0, 20.0, 20.0);
    assert_eq!(
        negative_origin.intersection(clip),
        Some(Rect::new(0.0, 0.0, 10.0, 10.0))
    );
    assert!(negative_origin.contains(Point::new(10.0, 10.0)));
    assert!(Rect::new(1.0, 1.0, 0.0, 20.0).intersection(clip).is_none());
}

fn fixture() -> Element {
    Element::container(ElementId::new(1), ElementKey::new("root"))
        .styled(Style::default().size(Dimension::Px(100.0), Dimension::Px(100.0)))
        .child(
            Element::container(ElementId::new(2), ElementKey::new("child"))
                .styled(Style::default().size(Dimension::Px(20.0), Dimension::Px(20.0))),
        )
}

#[test]
fn nonfinite_layout_inputs_reject_and_allow_retained_recovery() {
    type Case = (&'static str, fn(&mut Element));
    let cases: &[Case] = &[
        ("width NaN", |e| e.style.width = Dimension::Px(f32::NAN)),
        ("percentage infinity", |e| {
            e.style.height = Dimension::Percent(f32::INFINITY)
        }),
        ("basis NaN", |e| {
            e.style.flex_basis = Some(Dimension::Px(f32::NAN))
        }),
        ("offset infinity", |e| e.style.left = Some(f32::INFINITY)),
        ("gap NaN", |e| e.style.gap = f32::NAN),
        ("padding infinity", |e| e.style.padding.left = f32::INFINITY),
        ("margin NaN", |e| e.style.margin.left = f32::NAN),
        ("grow infinity", |e| e.style.flex_grow = f32::INFINITY),
        ("shrink NaN", |e| e.style.flex_shrink = f32::NAN),
        ("grid fraction infinity", |e| {
            e.style.grid_columns = Some(vec![GridTrack::Fr(f32::INFINITY)])
        }),
        ("grid row NaN", |e| {
            e.style.grid_rows = vec![GridTrack::Px(f32::NAN)]
        }),
        ("scroll offset NaN", |e| {
            e.kind = ElementKind::ScrollBox(Point::new(f32::NAN, 0.0))
        }),
    ];
    let viewport = Viewport::new(100.0, 100.0, 1.0);
    let root = fixture();
    let mut context = TextInputContext::default();
    let mut retained = RetainedLayoutEngine::default();
    let expected = LayoutEngine
        .compute(&root, viewport)
        .expect("valid reference");
    retained
        .compute_with_text_context(&root, viewport, &mut context)
        .expect("warm retained");
    let mut accepted = Vec::new();
    for &(name, change) in cases {
        let mut invalid = root.clone();
        change(&mut invalid.children[0]);
        if !matches!(LayoutEngine.compute(&invalid, viewport), Err(LayoutError::NonFiniteStyle(id)) if id == ElementId::new(2))
        {
            accepted.push((name, "stateless"));
        }
        if !matches!(retained.compute_with_text_context(&invalid, viewport, &mut context), Err(LayoutError::NonFiniteStyle(id)) if id == ElementId::new(2))
        {
            accepted.push((name, "retained"));
        }
        if !matches!(LayoutEngine.measure_with_text_context(&invalid, ContentConstraint::default(), &mut context), Err(LayoutError::NonFiniteStyle(id)) if id == ElementId::new(2))
        {
            accepted.push((name, "measurement"));
        }
        assert_eq!(
            retained
                .compute_with_text_context(&root, viewport, &mut context)
                .expect("recovery")
                .nodes(),
            expected.nodes()
        );
    }
    assert!(
        accepted.is_empty(),
        "nonfinite layout input was accepted: {accepted:?}"
    );

    let mut valid = fixture();
    valid.children[0].style.width = Dimension::Px(-5.0);
    valid.children[0].style.height = Dimension::Px(0.0);
    valid.children[0].style.margin.left = -8.0;
    valid.children[0].style.padding.left = -7.0;
    let reference = LayoutEngine
        .compute(&valid, viewport)
        .expect("finite normalization");
    assert!(reference.nodes().iter().all(|node| node.rect.is_valid()));
    let child = reference.node(ElementId::new(2)).expect("child");
    assert_eq!(child.rect.width, 0.0);
    assert!(child.rect.x < 0.0, "finite negative margin is retained");
    assert_eq!(
        retained
            .compute_with_text_context(&valid, viewport, &mut context)
            .expect("finite retained")
            .nodes(),
        reference.nodes()
    );
    let desired = LayoutEngine
        .measure_with_text_context(&valid, ContentConstraint::default(), &mut context)
        .expect("finite measurement");
    assert!(desired.preferred.is_valid() && desired.min_content.is_valid());
}

#[test]
fn finite_input_overflow_never_publishes_invalid_layout() {
    let mut root = fixture();
    root.children[0].style = Style::default()
        .absolute(f32::MAX, 0.0)
        .size(Dimension::Px(20.0), Dimension::Px(20.0));
    root.children[0].children.push(
        Element::container(ElementId::new(3), ElementKey::new("grandchild")).styled(
            Style::default()
                .absolute(f32::MAX, 0.0)
                .size(Dimension::Px(10.0), Dimension::Px(10.0)),
        ),
    );
    let viewport = Viewport::new(100.0, 100.0, 1.0);
    let overflow = LayoutEngine.compute(&root, viewport);
    assert!(
        matches!(&overflow, Err(LayoutError::InvalidGeometry(id)) if *id == ElementId::new(3)),
        "finite style overflow must not publish a usable layout: {overflow:?}"
    );
    let mut context = TextInputContext::default();
    let mut retained = RetainedLayoutEngine::default();
    assert!(
        matches!(retained.compute_with_text_context(&root, viewport, &mut context), Err(LayoutError::InvalidGeometry(id)) if id == ElementId::new(3))
    );
    let valid = fixture();
    assert_eq!(
        retained
            .compute_with_text_context(&valid, viewport, &mut context)
            .expect("recovery")
            .nodes(),
        LayoutEngine
            .compute(&valid, viewport)
            .expect("reference")
            .nodes()
    );
}
