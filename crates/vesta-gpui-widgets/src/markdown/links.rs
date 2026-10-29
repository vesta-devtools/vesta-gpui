//! Independent link semantics projected onto the original shaped paragraph.
use vesta_gpui::{
    Dimension, Element, ElementScope, InputTarget, Point, Rect, Semantic, SemanticRole, Style,
    UiWindowId,
};

use super::{MarkdownAnchorError, MarkdownLinkIntent, MarkdownView};

impl MarkdownView {
    pub fn link_target(
        &self,
        scope: ElementScope,
        window: UiWindowId,
        intent: MarkdownLinkIntent,
    ) -> Result<InputTarget, MarkdownAnchorError> {
        self.resolve_link(window, intent)?;
        let block = &self.blocks[self.block_index(intent.block)?];
        let slot = block
            .content
            .links
            .iter()
            .zip(&block.link_ids)
            .find_map(|(link, id)| {
                (link.range.start == intent.start && link.range.end == intent.end).then_some(*id)
            })
            .ok_or(MarkdownAnchorError::UnknownLink)?;
        Ok(InputTarget {
            window,
            element: scope.element(slot),
        })
    }

    pub(super) fn intent_for_target(
        &self,
        scope: ElementScope,
        window: UiWindowId,
        range: std::ops::Range<usize>,
        target: InputTarget,
    ) -> Option<MarkdownLinkIntent> {
        if target.window != window {
            return None;
        }
        self.links_in(window, range)
            .find(|&intent| self.link_target(scope, window, intent).ok() == Some(target))
    }

    pub(super) fn attach_link_semantics(
        &self,
        root: &mut Element,
        scope: ElementScope,
        range: std::ops::Range<usize>,
    ) {
        for block in self.blocks_in(range) {
            let Some((width, _, _, snapshot)) = &block.shaped else {
                continue;
            };
            let Some(body) = super::table::find_mut(root, scope.element(block.id.0 + 2)) else {
                continue;
            };
            // Same viewport conversion as painting; absolute children never
            // participate in the paragraph's intrinsic width or line breaking.
            let bounds = Rect::new(0.0, 0.0, width + 16.0, snapshot.layout_height() + 4.0);
            let origin = snapshot.viewport(bounds, bounds, Point::default()).origin;
            for (link, &id) in block.content.links.iter().zip(&block.link_ids) {
                let Ok(rectangles) = snapshot.range_rectangles(link.range.clone()) else {
                    continue;
                };
                let Some(bounds) = rectangles.into_iter().reduce(|a, b| {
                    let x = a.x.min(b.x);
                    let y = a.y.min(b.y);
                    Rect::new(
                        x,
                        y,
                        a.right().max(b.right()) - x,
                        a.bottom().max(b.bottom()) - y,
                    )
                }) else {
                    continue;
                };
                body.children.push(
                    Element::container(
                        scope.element(id),
                        scope.identity(id, format!("markdown.link.{id}")).key,
                    )
                    .semantic(
                        Semantic::new(SemanticRole::Link, &block.content.text[link.range.clone()])
                            .description(&link.destination)
                            .activatable(true),
                    )
                    .styled(
                        Style::default()
                            .absolute(bounds.x + origin.x, bounds.y + origin.y)
                            .size(Dimension::Px(bounds.width), Dimension::Px(bounds.height))
                            .pointer_events(false)
                            .focusable(true),
                    ),
                );
            }
        }
    }
}
