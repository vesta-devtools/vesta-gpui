use super::*;

// Host fonts exercise real Latin/CJK/Arabic metrics rather than the portable
// icon fixture. This protects content height and scrollability after narrow reflow.
#[test]
fn constrained_mixed_baseline_keeps_every_wrapped_line_inside_its_layout() {
    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.layout");
    let root = install_action_route(&mut app);
    let row = root
        .find(app.scopes.gallery.element(25_200_000))
        .expect("mixed baseline");
    for width in [104.0, 120.0, 320.0, 500.0] {
        let container = Element::container(ElementId::new(0), ElementKey::new("test.viewport"))
            .styled(Style::default().size(Dimension::Px(width), Dimension::Auto))
            .child(row.clone());
        let layout = LayoutEngine
            .compute_with_text_context(
                &container,
                Viewport::new(width, 800.0, 1.25),
                &mut app.text_context,
            )
            .expect("mixed row");
        let row_frame = layout.node(row.id).expect("row frame").rect;
        for child in &row.children {
            let vesta_gpui::ElementKind::Text(text) = &child.kind else {
                continue;
            };
            let frame = layout.node(child.id).expect("text frame").rect;
            let shaped = app
                .text_context
                .shape_static_text_wrapped(
                    text,
                    child.style.visual.font_size,
                    child.style.visual.font_role,
                    Some(frame.width),
                )
                .expect("full wrapped source");
            if width == 500.0 {
                let natural = app
                    .text_context
                    .shape_static_text(
                        text,
                        child.style.visual.font_size,
                        child.style.visual.font_role,
                    )
                    .expect("natural width");
                assert!(
                    frame.width >= natural.layout_width(),
                    "a wide row must not truncate a child's natural width"
                );
                assert!(
                    frame.height <= natural.layout_height() + 1.0,
                    "a wide row must not manufacture a soft line"
                );
            }
            assert!(frame.width <= width + 1.0);
            assert!(
                frame.height + 1.0 >= shaped.layout_height(),
                "{} allocated {} required {} at width {}",
                text,
                frame.height,
                shaped.layout_height(),
                frame.width
            );
            assert!(
                frame.bottom() <= row_frame.bottom() + 1.0,
                "complete text stays in the row's measured scroll extent"
            );
        }
    }

    // A complete page also has percentage sizing, section shells and a reserved
    // scrollbar. Its scroll endpoint must expose the final leaf after reflow.
    let item = activate_gallery_page(&mut app, "gallery.layout");
    let mut retained = vesta_gpui::RetainedLayoutEngine::default();
    for ratio in [0.5, 0.90625, 0.09375, 0.90625] {
        app.demo
            .workspace
            .set_split_ratio(ContainerRef::Main, SplitId(1), ratio)
            .expect("live split");
        let root = install_action_route(&mut app);
        let layout = LayoutEngine
            .compute_with_text_context(
                &root,
                Viewport::new(1280.0, 800.0, 1.25),
                &mut app.text_context,
            )
            .expect("complete mixed page");
        let retained_layout = retained
            .compute_with_text_context(
                &root,
                Viewport::new(1280.0, 800.0, 1.25),
                &mut app.text_context,
            )
            .expect("retained complete mixed page");
        assert_eq!(retained_layout.nodes(), layout.nodes());
        let scroll = layout
            .node(page_scroll_id(app.scopes.gallery, item))
            .expect("page scroll");
        let (_, content_height) = scroll.scroll_content.expect("measured page extent");
        let row = layout
            .node(app.scopes.gallery.element(25_200_000))
            .expect("page baseline row");
        for slot in 1..=3 {
            let frame = layout
                .node(app.scopes.gallery.element(25_200_000 + slot))
                .expect("page text leaf")
                .rect;
            assert!(
                frame.bottom() <= row.rect.bottom() + 1.0,
                "page leaf {slot} {frame:?} exceeds row {:?} at ratio {ratio}",
                row.rect
            );
            assert!(
                frame.bottom() <= scroll.rect.y + content_height + 1.0,
                "leaf {} at {} cannot exceed page endpoint {} at ratio {}",
                slot,
                frame.bottom(),
                scroll.rect.y + content_height,
                ratio,
            );
        }
    }
}
