use super::*;
use crate::{
    AccessibilityTree, Color, HitTestTree, PaintCommand, PaintList, ScrollAttachment,
    ScrollbarGutter, Semantic, SemanticRole, Style, TextInputContext,
};

#[test]
fn scroll_attachment_keeps_one_geometry_and_extent_through_fractional_scroll_and_reuse() {
    let id = ElementId::new;
    let mut retained = RetainedLayoutEngine::default();
    let mut text = TextInputContext::default();
    for flow in [FlowDirection::LeftToRight, FlowDirection::RightToLeft] {
        for scale in [1.0, 1.5, 2.0] {
            let viewport = Viewport::new(200.0, 100.0, scale);
            for attachment in [
                ScrollAttachment::Content,
                ScrollAttachment::ViewportX,
                ScrollAttachment::ViewportY,
                ScrollAttachment::Viewport,
            ] {
                for offset in [
                    Point::default(),
                    Point::new(30.25, 20.5),
                    Point::new(60.75, 40.25),
                ] {
                    let mut style = Style::default()
                        .size(Dimension::Px(200.0), Dimension::Px(100.0))
                        .flow_direction(flow);
                    style.scrollbar.gutter = ScrollbarGutter::Overlay;
                    let root = Element::scroll_box(id(1), ElementKey::new("scroll"), offset)
                        .styled(style)
                        .child(
                            Element::container(id(2), ElementKey::new("content"))
                                .styled(
                                    Style::default()
                                        .absolute(0.0, 0.0)
                                        .size(Dimension::Px(500.0), Dimension::Px(400.0))
                                        .scroll_attachment(attachment),
                                )
                                .child(
                                    Element::container(id(3), ElementKey::new("child"))
                                        .semantic(Semantic::new(
                                            SemanticRole::Button,
                                            "Pinned child",
                                        ))
                                        .styled(
                                            Style::default()
                                                .absolute(80.25, 50.5)
                                                .size(Dimension::Px(30.5), Dimension::Px(20.25))
                                                // A non-ScrollBox parent ignores this grandchild value.
                                                .scroll_attachment(ScrollAttachment::Viewport)
                                                .background(Color::rgb8(100, 80, 60))
                                                .focusable(true),
                                        ),
                                ),
                        );
                    let full = LayoutEngine
                        .compute(&root, viewport)
                        .expect("valid attached layout fixture");
                    let reused = retained
                        .compute_with_text_context(&root, viewport, &mut text)
                        .expect("valid attached layout fixture");
                    assert_eq!(full.nodes(), reused.nodes());
                    assert_eq!(
                        full.node(id(1))
                            .expect("valid attached layout fixture")
                            .scroll_content,
                        Some((500.0, 400.0))
                    );
                    let x_offset = match attachment {
                        ScrollAttachment::ViewportX | ScrollAttachment::Viewport => 0.0,
                        _ => flow.horizontal(offset.x),
                    };
                    let y_offset = match attachment {
                        ScrollAttachment::ViewportY | ScrollAttachment::Viewport => 0.0,
                        _ => offset.y,
                    };
                    let node = full.node(id(3)).expect("valid attached layout fixture");
                    assert_eq!(
                        node.rect,
                        Rect::new(80.0 - x_offset, 51.0 - y_offset, 31.0, 20.0)
                    );
                    assert_eq!(
                        HitTestTree::build(&root, &full)
                            .hit(Point::new(node.rect.x + 5.0, node.rect.y + 5.0)),
                        Some(id(3))
                    );
                    assert!(PaintList::build(&root, &full).commands().iter().any(|command|
                        matches!(command, PaintCommand::FillRoundedRect {element, rect, ..} if *element == id(3) && *rect == node.rect)));
                    let mut a11y = AccessibilityTree::default();
                    let update = a11y.build(&root, &full, Some(id(3)), "Attached scroll");
                    assert_eq!(
                        update.focus,
                        a11y.node_for_element(id(3))
                            .expect("valid attached layout fixture")
                    );
                    assert!(update.nodes.iter().any(|(_, semantic)| {
                        semantic.label() == Some("Pinned child")
                            && semantic.bounds().is_some_and(|bounds| {
                                bounds.x0 == f64::from(node.rect.x)
                                    && bounds.y0 == f64::from(node.rect.y)
                            })
                    }));
                }
            }
        }
    }
}
