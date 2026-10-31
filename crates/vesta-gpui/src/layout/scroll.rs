//! Overflow negotiation uses Taffy's own scrollbar insets and layout caches.

use super::*;
use crate::{ElementKind, ScrollbarGutter};
use taffy::{Overflow, Point as TaffyPoint};

pub(super) fn scroll_extent_clip(element: &Element) -> crate::ClipMode {
    if element.style.clip == crate::ClipMode::Inherit
        && matches!(element.kind, ElementKind::ScrollBox(_))
    {
        crate::ClipMode::Clip
    } else {
        element.style.clip
    }
}

pub(super) fn scroll_content_size(
    element: &Element,
    taffy: &LayoutStorage,
    nodes: &HashMap<ElementId, NodeId>,
) -> Option<(f32, f32)> {
    fn visible_size(
        element: &Element,
        taffy: &LayoutStorage,
        nodes: &HashMap<ElementId, NodeId>,
    ) -> Option<(f32, f32)> {
        let clip = scroll_extent_clip(element);
        if clip == crate::ClipMode::NoClip {
            return None;
        }
        let layout = taffy.layout(*nodes.get(&element.id)?).ok()?;
        let mut size = (layout.size.width, layout.size.height);
        if clip == crate::ClipMode::Inherit {
            for child in &element.children {
                if let Some((width, height)) = visible_size(child, taffy, nodes)
                    && let Some(node) = nodes.get(&child.id)
                    && let Ok(child_layout) = taffy.layout(*node)
                {
                    size.0 = size.0.max(child_layout.location.x.max(0.0) + width);
                    size.1 = size.1.max(child_layout.location.y.max(0.0) + height);
                }
            }
        }
        Some(size)
    }

    let mut bounds: Option<(f32, f32)> = None;
    for child in &element.children {
        if let Some((width, height)) = visible_size(child, taffy, nodes)
            && let Some(node) = nodes.get(&child.id)
            && let Ok(layout) = taffy.layout(*node)
        {
            let width = layout.location.x.max(0.0) + width;
            let height = layout.location.y.max(0.0) + height;
            bounds = Some(bounds.map_or((width, height), |(w, h)| (w.max(width), h.max(height))));
        }
    }
    bounds.map(|(w, h)| {
        if element.style.scrollbar.gutter == ScrollbarGutter::Reserve {
            (
                w + element.style.padding.right.max(0.0),
                h + element.style.padding.bottom.max(0.0),
            )
        } else {
            (w, h)
        }
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compute_scroll_layout(
    root: &Element,
    taffy: &mut LayoutStorage,
    nodes: &HashMap<ElementId, NodeId>,
    root_node: NodeId,
    viewport: Viewport,
    reset: bool,
    measure: &mut impl FnMut(TextLayoutRequest<'_>) -> TextLayoutMetrics,
) -> Result<(), LayoutError> {
    fn collect<'a>(element: &'a Element, output: &mut Vec<&'a Element>) {
        if matches!(element.kind, ElementKind::ScrollBox(_))
            && element.style.scrollbar.gutter == ScrollbarGutter::Reserve
        {
            output.push(element);
        }
        for child in &element.children {
            collect(child, output);
        }
    }
    let mut scrolls = Vec::new();
    collect(root, &mut scrolls);
    if reset {
        for scroll in &scrolls {
            let node = nodes[&scroll.id];
            let mut style = taffy.style(node).map_err(layout_error)?.clone();
            style.overflow = TaffyPoint {
                x: Overflow::Hidden,
                y: Overflow::Hidden,
            };
            if taffy.style(node).map_err(layout_error)? != &style {
                taffy.set_style(node, style).map_err(layout_error)?;
            }
        }
    }
    // Each iteration adds at least one of 2 * scroll_count axes. Hover, paint
    // and scroll offsets never participate in this fixed point.
    loop {
        compute_taffy_layout(taffy, root_node, viewport, measure)?;
        let mut changed = false;
        for scroll in &scrolls {
            let node = nodes[&scroll.id];
            let layout = *taffy.layout(node).map_err(layout_error)?;
            let Some((w, h)) = scroll_content_size(scroll, taffy, nodes) else {
                continue;
            };
            let mut style = taffy.style(node).map_err(layout_error)?.clone();
            let before = style.overflow;
            if w > (layout.size.width - layout.scrollbar_size.width).max(0.0) + 0.01 {
                style.overflow.x = Overflow::Scroll;
            }
            if h > (layout.size.height - layout.scrollbar_size.height).max(0.0) + 0.01 {
                style.overflow.y = Overflow::Scroll;
            }
            if style.overflow != before {
                taffy.set_style(node, style).map_err(layout_error)?;
                changed = true;
            }
        }
        if !changed {
            return Ok(());
        }
    }
}

fn layout_error(error: impl fmt::Display) -> LayoutError {
    LayoutError::LayoutEngine(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HitTestTree, PaintList, ScrollbarStyle, ScrollbarVisibility, Style};

    #[test]
    fn reserved_overflow_reflows_clips_and_hits_independently_of_thumb_visibility() {
        for flow in [FlowDirection::LeftToRight, FlowDirection::RightToLeft] {
            let mut root = Element::scroll_box(
                ElementId::new(1),
                ElementKey::new("scroll"),
                Point::default(),
            )
            .styled(
                Style::default()
                    .size(Dimension::Px(100.0), Dimension::Px(60.0))
                    .flow_direction(flow)
                    .scrollbar(ScrollbarStyle {
                        gutter: ScrollbarGutter::Reserve,
                        visibility: ScrollbarVisibility::OnHover,
                        ..ScrollbarStyle::default()
                    }),
            )
            .child(
                Element::container(ElementId::new(2), ElementKey::new("content"))
                    .styled(Style::default().size(Dimension::Px(95.0), Dimension::Px(100.0))),
            );
            let viewport = Viewport::new(100.0, 60.0, 1.0);
            let layout = LayoutEngine
                .compute(&root, viewport)
                .expect("reserved layout");
            let node = layout.node(root.id).expect("scroll");
            let visible = node.scroll_viewport();
            assert!(
                visible.width < node.rect.width && visible.height < node.rect.height,
                "vertical overflow must also negotiate the horizontal overflow it induces"
            );
            let (Some(vertical), _, Some(horizontal), _) =
                node.scrollbar_geometry(Point::default())
            else {
                panic!("two tracks");
            };
            assert!(
                vertical.intersection(visible).is_none()
                    && horizontal.intersection(visible).is_none()
            );
            assert!(
                vertical.intersection(horizontal).is_none(),
                "tracks must leave the corner free"
            );
            let pointer = Point::new(
                vertical.x + vertical.width * 0.5,
                vertical.y + vertical.height * 0.5,
            );
            assert_eq!(
                HitTestTree::build(&root, &layout).hit(pointer),
                Some(root.id),
                "content cannot intercept the gutter"
            );
            assert_eq!(
                layout
                    .node(root.children[0].id)
                    .expect("content")
                    .inherited_clip,
                visible
            );
            let paint = PaintList::build(&root, &layout);
            assert!(paint.commands().iter().any(|command| matches!(command, crate::PaintCommand::PushClip { rect, .. } if *rect == visible)));
            for visibility in [ScrollbarVisibility::Always, ScrollbarVisibility::OnHover] {
                root.style.scrollbar.visibility = visibility;
                let updated = LayoutEngine
                    .compute(&root, viewport)
                    .expect("visibility only");
                assert_eq!(updated.nodes(), layout.nodes());
            }
            root.children[0].style.width = Dimension::Percent(1.0);
            let responsive = LayoutEngine
                .compute(&root, viewport)
                .expect("responsive content");
            let node = responsive.node(root.id).expect("scroll");
            assert!(
                node.scrollbar_geometry(Point::default()).2.is_none(),
                "responsive width uses the reduced viewport"
            );
            root.children[0].style.height = Dimension::Px(40.0);
            let restored = LayoutEngine
                .compute(&root, viewport)
                .expect("content shrank");
            let node = restored.node(root.id).expect("scroll");
            assert_eq!(
                node.scroll_viewport(),
                node.rect,
                "no overflow releases both gutters"
            );
            root.style.width = Dimension::Px(6.0);
            root.style.height = Dimension::Px(6.0);
            root.children[0].style.width = Dimension::Px(100.0);
            root.children[0].style.height = Dimension::Px(100.0);
            let tiny = LayoutEngine
                .compute(&root, viewport)
                .expect("tiny viewport");
            let node = tiny.node(root.id).expect("scroll");
            for track in [
                node.scrollbar_geometry(Point::new(1000.0, 1000.0)).0,
                node.scrollbar_geometry(Point::new(1000.0, 1000.0)).2,
            ]
            .into_iter()
            .flatten()
            {
                assert_eq!(track.intersection(node.rect), Some(track));
            }

            let mut inherited = Element::scroll_box(
                ElementId::new(10),
                ElementKey::new("inherited.scroll"),
                Point::default(),
            )
            .styled(
                Style::default()
                    .size(Dimension::Px(100.0), Dimension::Px(60.0))
                    .flow_direction(flow)
                    .scrollbar(ScrollbarStyle {
                        gutter: ScrollbarGutter::Reserve,
                        ..ScrollbarStyle::default()
                    }),
            )
            .child(
                Element::container(ElementId::new(11), ElementKey::new("inherited.wrapper"))
                    .styled(Style::default().size(Dimension::Percent(1.0), Dimension::Px(40.0)))
                    .child(
                        Element::container(ElementId::new(12), ElementKey::new("inherited.wide"))
                            .styled(
                                Style::default().size(Dimension::Px(220.0), Dimension::Px(40.0)),
                            ),
                    ),
            );
            inherited.children[0].style.clip = crate::ClipMode::Inherit;
            let visible_overflow = LayoutEngine
                .compute(&inherited, viewport)
                .expect("inherited overflow");
            let mut retained = crate::RetainedLayoutEngine::default();
            let mut context = crate::TextInputContext::default();
            assert_eq!(
                retained
                    .compute_with_text_context(&inherited, viewport, &mut context)
                    .expect("retained inherited overflow")
                    .nodes(),
                visible_overflow.nodes()
            );
            let scroll = visible_overflow
                .node(inherited.id)
                .expect("inherited scroll");
            assert!(scroll.scroll_content.expect("content").0 >= 220.0);
            assert!(scroll.scrollbar_geometry(Point::default()).2.is_some());
            for boundary in [crate::ClipMode::Clip, crate::ClipMode::NoClip] {
                inherited.children[0].style.clip = boundary;
                let bounded = LayoutEngine
                    .compute(&inherited, viewport)
                    .expect("bounded overflow");
                assert_eq!(
                    retained
                        .compute_with_text_context(&inherited, viewport, &mut context)
                        .expect("retained boundary change")
                        .nodes(),
                    bounded.nodes()
                );
                assert!(
                    bounded
                        .node(inherited.id)
                        .expect("bounded scroll")
                        .scrollbar_geometry(Point::default())
                        .2
                        .is_none()
                );
            }
            inherited.children[0].style.clip = crate::ClipMode::Inherit;
            inherited.children[0].kind = ElementKind::ScrollBox(Point::default());
            let nested = LayoutEngine
                .compute(&inherited, viewport)
                .expect("nested isolation");
            assert_eq!(
                retained
                    .compute_with_text_context(&inherited, viewport, &mut context)
                    .expect("retained nested isolation")
                    .nodes(),
                nested.nodes()
            );
            assert!(
                nested
                    .node(inherited.id)
                    .expect("outer scroll")
                    .scrollbar_geometry(Point::default())
                    .2
                    .is_none()
            );
            assert!(
                nested
                    .node(inherited.children[0].id)
                    .expect("inner scroll")
                    .scrollbar_geometry(Point::default())
                    .2
                    .is_some()
            );
        }
    }
}
