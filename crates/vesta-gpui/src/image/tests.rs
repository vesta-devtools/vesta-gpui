use super::*;

#[test]
fn image_presentation_changes_preserve_retained_geometry_child_routes_and_semantics() {
    use crate::{
        AccessibilityTree, ActionRoute, Dimension, Element, ElementId, ElementKey, ElementKind,
        HitTestTree, LayoutEngine, PaintCommand, PaintList, Point, RetainedLayoutEngine, Semantic,
        SemanticRole, Style, TextInputContext, Viewport,
    };
    let image_id = ElementId::new(2);
    let child_id = ElementId::new(3);
    let mut root = Element::container(ElementId::new(1), ElementKey::new("root"))
        .styled(Style::default().size(Dimension::Px(60.0), Dimension::Px(45.0)))
        .child(
            Element::image(image_id, ElementKey::new("image"), "fixture://original")
                .styled(
                    Style::default()
                        .absolute(-10.0, -5.0)
                        .size(Dimension::Px(120.0), Dimension::Px(80.0)),
                )
                .semantic(Semantic::new(SemanticRole::Label, "source image"))
                .child(
                    Element::container(child_id, ElementKey::new("child"))
                        .styled(
                            Style::default()
                                .absolute(20.0, 20.0)
                                .size(Dimension::Px(12.0), Dimension::Px(10.0)),
                        )
                        .semantic(Semantic::new(SemanticRole::Button, "child action")),
                ),
        );
    let viewport = Viewport::new(200.0, 150.0, 1.25);
    let mut context = TextInputContext::default();
    let mut retained = RetainedLayoutEngine::default();
    let mut accessibility = AccessibilityTree::default();
    let original = retained
        .compute_with_text_context(&root, viewport, &mut context)
        .expect("legacy");
    let original_semantics = accessibility.build(&root, &original, None, "image");
    assert!(
        PaintList::build(&root, &original)
            .commands()
            .iter()
            .any(|c| matches!(c, PaintCommand::Image { .. }))
    );
    for (fit, source) in [
        (ImageFit::Stretch, "fixture://wide"),
        (ImageFit::Contain, "fixture://tall"),
        (ImageFit::Cover, "fixture://replacement"),
    ] {
        let presentation = ImagePresentation::new(fit).aligned(ImageAlign::Max, ImageAlign::Min);
        root.children[0].kind = ElementKind::FittedImage {
            source: source.into(),
            presentation,
        };
        let layout = retained
            .compute_with_text_context(&root, viewport, &mut context)
            .expect("retained");
        let stateless = LayoutEngine
            .compute_with_text_context(&root, viewport, &mut context)
            .expect("stateless");
        assert_eq!(layout.nodes(), original.nodes());
        assert_eq!(layout.nodes(), stateless.nodes());
        let paint = PaintList::build(&root, &layout);
        assert!(
            paint
                .commands()
                .iter()
                .any(|c| matches!(c, PaintCommand::FittedImage {
            element, bounds, clip, source: actual, presentation: actual_presentation,
        } if *element == image_id && *bounds == Rect::new(-10.0, -5.0, 120.0, 80.0)
            && *clip == Rect::new(0.0, 0.0, 60.0, 45.0)
            && actual == source && *actual_presentation == presentation))
        );
        let hits = HitTestTree::build(&root, &layout);
        assert_eq!(hits.hit(Point::new(15.0, 20.0)), Some(child_id));
        assert_eq!(hits.hit(Point::new(5.0, 5.0)), Some(image_id));
        assert_eq!(hits.hit(Point::new(65.0, 20.0)), None);
        let route =
            ActionRoute::build(&root, [(image_id, "image"), (child_id, "child")]).expect("route");
        assert_eq!(route.action_at(child_id), Some(&"child"));
        assert_eq!(
            accessibility.build(&root, &layout, None, "image"),
            original_semantics
        );
    }
}

#[test]
fn image_mapping_preserves_aspect_anchors_and_clipping_without_nonfinite_output() {
    let frame = Rect::new(20.0, 30.0, 120.0, 80.0);
    for (source, contain, cover) in [
        (
            Size::new(200.0, 100.0),
            Rect::new(20.0, 40.0, 120.0, 60.0),
            Rect::new(0.125, 0.0, 0.75, 1.0),
        ),
        (
            Size::new(100.0, 200.0),
            Rect::new(60.0, 30.0, 40.0, 80.0),
            Rect::new(0.0, 1.0 / 3.0, 1.0, 1.0 / 3.0),
        ),
    ] {
        for fit in [ImageFit::Stretch, ImageFit::Contain, ImageFit::Cover] {
            let placement = ImagePresentation::new(fit)
                .place(frame, source)
                .expect("place");
            assert_eq!(
                placement.bounds,
                if fit == ImageFit::Contain {
                    contain
                } else {
                    frame
                }
            );
            assert_eq!(
                placement.uv,
                if fit == ImageFit::Cover {
                    cover
                } else {
                    Rect::new(0.0, 0.0, 1.0, 1.0)
                }
            );
            for x in [ImageAlign::Min, ImageAlign::Center, ImageAlign::Max] {
                for y in [ImageAlign::Min, ImageAlign::Center, ImageAlign::Max] {
                    let anchored = ImagePresentation::new(fit)
                        .aligned(x, y)
                        .place(frame, source)
                        .expect("anchor");
                    assert!(
                        frame.contains(crate::Point::new(anchored.bounds.x, anchored.bounds.y))
                    );
                    assert!(anchored.bounds.right() <= frame.right());
                    assert!(anchored.bounds.bottom() <= frame.bottom());
                    assert!(anchored.uv.x >= 0.0 && anchored.uv.right() <= 1.0);
                    assert!(anchored.uv.y >= 0.0 && anchored.uv.bottom() <= 1.0);
                    if fit == ImageFit::Contain {
                        assert_eq!(
                            anchored.bounds.x,
                            frame.x + (frame.width - anchored.bounds.width) * x.fraction() as f32
                        );
                        assert_eq!(
                            anchored.bounds.y,
                            frame.y + (frame.height - anchored.bounds.height) * y.fraction() as f32
                        );
                    } else if fit == ImageFit::Cover {
                        assert_eq!(
                            anchored.uv.x,
                            (1.0 - f64::from(anchored.uv.width)) as f32 * x.fraction() as f32
                        );
                    }
                }
            }
            let clip = Rect::new(65.0, 50.0, 30.0, 20.0);
            let cropped = placement.clipped(clip).expect("overlapping clip");
            assert_eq!(cropped.bounds, clip);
            // Mapping a logical point through original and cropped quads agrees.
            let original_u = placement.uv.x
                + (clip.x - placement.bounds.x) / placement.bounds.width * placement.uv.width;
            let original_v = placement.uv.y
                + (clip.y - placement.bounds.y) / placement.bounds.height * placement.uv.height;
            assert!((cropped.uv.x - original_u).abs() < 1e-6);
            assert!((cropped.uv.y - original_v).abs() < 1e-6);
            assert!(
                placement
                    .clipped(Rect::new(400.0, 400.0, 10.0, 10.0))
                    .is_none()
            );
        }
    }
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(
            ImagePresentation::default()
                .place(frame, Size::new(bad, 20.0))
                .is_none()
        );
        assert!(
            ImagePresentation::default()
                .place(Rect::new(0.0, 0.0, 10.0, bad), Size::new(20.0, 20.0))
                .is_none()
        );
    }
    for fit in [ImageFit::Contain, ImageFit::Cover] {
        // Intermediate aspect/scale arithmetic must not overflow at finite extremes.
        if let Some(result) =
            ImagePresentation::new(fit).place(frame, Size::new(f32::MAX, f32::MIN_POSITIVE))
        {
            assert!(valid_rect(result.bounds) && valid_rect(result.uv));
        }
    }
    assert!(
        ImagePresentation::default()
            .place(Rect::new(1e30, 0.0, 1.0, 1.0), Size::new(1.0, 1.0))
            .is_none()
    );
    assert!(
        ImagePresentation::new(ImageFit::Cover)
            .place(frame, Size::new(1e20, 1.0))
            .is_none(),
        "a centered UV region whose endpoints collapse in f32 is unrepresentable"
    );
}
