use super::*;
use crate::{
    AccessibilityTree, HitTestTree, PaintCommand, PaintList, Semantic, SemanticRole, Style,
    TextHAlign,
};

#[test]
fn inherited_flow_keeps_identity_hit_semantics_physical_slots_and_scroll_projection_consistent() {
    let id = ElementId::new;
    let mut root = Element::container(id(1), ElementKey::new("root"))
        .styled(
            Style::default()
                .size(Dimension::Px(300.0), Dimension::Px(100.0))
                .axis(Axis::Row)
                .align(Align::Start)
                .flow_direction(FlowDirection::RightToLeft)
                .padding(Insets {
                    left: 12.0,
                    right: 18.0,
                    ..Insets::default()
                }),
        )
        .child(
            Element::text(id(2), ElementKey::new("label"), "مرحبا English 中文")
                .semantic(Semantic::new(SemanticRole::Button, "mixed label"))
                .styled(
                    Style::default()
                        .size(Dimension::Px(50.0), Dimension::Px(20.0))
                        .margin(Insets {
                            left: 7.0,
                            right: 9.0,
                            ..Insets::default()
                        }),
                ),
        )
        .child(
            Element::container(id(3), ElementKey::new("ltr"))
                .styled(
                    Style::default()
                        .size(Dimension::Px(100.0), Dimension::Px(20.0))
                        .axis(Axis::Row)
                        .flow_direction(FlowDirection::LeftToRight),
                )
                .child(
                    Element::text(id(4), ElementKey::new("left"), "Left")
                        .styled(Style::default().size(Dimension::Px(25.0), Dimension::Px(20.0))),
                )
                .child(
                    Element::text(id(5), ElementKey::new("right"), "Right")
                        .styled(Style::default().size(Dimension::Px(25.0), Dimension::Px(20.0))),
                ),
        )
        .child(
            Element::container(id(6), ElementKey::new("physical")).styled(
                Style::default()
                    .absolute(40.0, 60.0)
                    .size(Dimension::Px(10.0), Dimension::Px(10.0)),
            ),
        );
    let viewport = Viewport::new(300.0, 100.0, 1.0);
    let layout = LayoutEngine.compute(&root, viewport).expect("RTL layout");
    let label = layout.node(id(2)).expect("label");
    assert_eq!(label.rect.right(), 300.0 - 18.0 - 9.0);
    assert_eq!(label.flow_direction, FlowDirection::RightToLeft);
    assert!(layout.node(id(3)).expect("LTR box").rect.x < label.rect.x);
    assert!(
        layout.node(id(4)).expect("LTR left").rect.x
            < layout.node(id(5)).expect("LTR right").rect.x
    );
    assert_eq!(layout.node(id(6)).expect("absolute").rect.x, 40.0);
    let pointer = Point::new(label.rect.x + label.rect.width * 0.5, label.rect.y + 5.0);
    assert_eq!(HitTestTree::build(&root, &layout).hit(pointer), Some(id(2)));
    assert!(PaintList::build(&root, &layout).commands().iter().any(|command|
        matches!(command, PaintCommand::Text { element, h_align: TextHAlign::End, .. } if *element == id(2))));
    let mut semantics = AccessibilityTree::default();
    let update = semantics.build(&root, &layout, None, "RTL");
    assert!(update.nodes.iter().any(|(_, node)| {
        node.label() == Some("mixed label")
            && node
                .bounds()
                .is_some_and(|bounds| bounds.x0 == f64::from(label.rect.x))
    }));
    root.children[0].style.left = Some(10.0);
    let displaced = LayoutEngine
        .compute(&root, viewport)
        .expect("physical relative offset");
    assert_eq!(
        displaced.node(id(2)).expect("displaced").rect.x,
        label.rect.x + 10.0
    );
    let scroll = Element::scroll_box(id(10), ElementKey::new("scroll"), Point::default())
        .styled(
            Style::default()
                .size(Dimension::Px(200.0), Dimension::Px(60.0))
                .flow_direction(FlowDirection::RightToLeft),
        )
        .child(
            Element::container(id(11), ElementKey::new("wide"))
                .styled(
                    Style::default()
                        .size(Dimension::Px(500.0), Dimension::Px(40.0))
                        .axis(Axis::Row),
                )
                .child(
                    Element::text(id(12), ElementKey::new("start"), "Start")
                        .styled(Style::default().size(Dimension::Px(50.0), Dimension::Px(20.0))),
                ),
        );
    let at_start = LayoutEngine
        .compute(&scroll, viewport)
        .expect("logical start");
    assert_eq!(at_start.node(id(12)).expect("start").rect.right(), 200.0);
    let mut shifted = scroll;
    shifted.kind = crate::ElementKind::ScrollBox(Point::new(100.0, 0.0));
    let at_offset = LayoutEngine
        .compute(&shifted, viewport)
        .expect("logical offset");
    assert_eq!(
        at_offset.node(id(12)).expect("shifted").rect.x,
        at_start.node(id(12)).expect("initial").rect.x + 100.0
    );
    assert_eq!(
        at_offset.node(id(10)).expect("viewport").scroll_content,
        Some((500.0, 40.0))
    );
}
