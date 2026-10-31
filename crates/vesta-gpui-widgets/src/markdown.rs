use crate::document_text::{label, wrapped};
use crate::{WidgetIdentity, WidgetTheme, rich_text_paragraph};
use std::collections::HashMap;
use vesta_gpui::{
    Axis, Dimension, Element, ElementScope, FontRole, Insets, LayoutTree, Style, StyledTextError,
    TextInputContext, TextInputSnapshot, TextMeasurementRevision, text_input_wrap_width,
};

#[cfg(test)]
#[path = "markdown/tests.rs"]
mod anchor_tests;
mod anchors;
mod geometry;
mod interaction;
mod links;
mod parse;
mod table;
pub use anchors::{
    MarkdownAnchor, MarkdownAnchorError, MarkdownBlockId, MarkdownDocumentId, MarkdownLink,
    MarkdownLinkIntent, MarkdownRevision, MarkdownSelection,
};
pub use geometry::MarkdownProjectionError;
pub use interaction::{
    MarkdownInteraction, MarkdownKeyIntent, MarkdownPointerResult, MarkdownSurface,
};

struct Block {
    id: MarkdownBlockId,
    content: parse::Content,
    shaped: Option<(f32, f32, TextMeasurementRevision, TextInputSnapshot)>,
    link_ids: Vec<u32>,
}

impl Block {
    fn size(&self, theme: WidgetTheme) -> f32 {
        let base = theme.typography.font_size + 1.0;
        base + match self.content.heading {
            1 => 12.0,
            2 => 8.0,
            3 => 5.0,
            4.. => 2.0,
            _ => 0.0,
        }
    }
}

#[derive(Default)]
pub struct MarkdownView {
    blocks: Vec<Block>,
    source: String,
    identity: Option<MarkdownDocumentId>,
    revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownError {
    ParagraphTooLong { block: usize },
    IdentityExhausted,
    RevisionExhausted,
    Layout(StyledTextError),
}

impl std::fmt::Display for MarkdownError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParagraphTooLong { block } => write!(
                formatter,
                "Markdown block {block} exceeds {} bytes",
                vesta_gpui::TEXT_INPUT_MAX_BYTES
            ),
            Self::Layout(error) => write!(formatter, "Markdown text layout failed: {error:?}"),
            Self::IdentityExhausted => write!(formatter, "Markdown identity space exhausted"),
            Self::RevisionExhausted => write!(formatter, "Markdown revision space exhausted"),
        }
    }
}

impl std::error::Error for MarkdownError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Layout(error) => Some(error),
            Self::ParagraphTooLong { .. } | Self::IdentityExhausted | Self::RevisionExhausted => {
                None
            }
        }
    }
}

impl MarkdownView {
    pub fn parse(text: &str) -> Result<Self, MarkdownError> {
        let mut document = Self::default();
        document.replace(text)?;
        Ok(document)
    }

    /// Atomically accept a new source revision. Ambiguous duplicate blocks do
    /// not inherit a preceding block's identity, selection or shaped cache.
    pub fn replace(&mut self, text: &str) -> Result<bool, MarkdownError> {
        if self.identity.is_some() && self.source == text {
            return Ok(false);
        }
        let contents = parse::parse(text)?;
        let identity = match self.identity {
            Some(identity) => identity,
            None => MarkdownDocumentId::allocate()?,
        };
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(MarkdownError::RevisionExhausted)?;
        let same_blocks = contents
            .iter()
            .eq(self.blocks.iter().map(|block| &block.content));
        let mut previous = HashMap::new();
        for block in &self.blocks {
            previous
                .entry(&block.content)
                .and_modify(|found| *found = None)
                .or_insert(Some(block));
        }
        let mut counts = HashMap::new();
        for content in &contents {
            *counts.entry(content).or_insert(0_usize) += 1;
        }
        let blocks = contents
            .iter()
            .enumerate()
            .map(|(index, content)| {
                let prior = if same_blocks {
                    self.blocks.get(index)
                } else if counts.get(content) == Some(&1) {
                    previous.get(content).copied().flatten()
                } else {
                    None
                };
                Ok((
                    prior.map_or_else(MarkdownBlockId::allocate, |block| Ok(block.id))?,
                    prior.and_then(|block| block.shaped.clone()),
                    match prior {
                        Some(block) => block.link_ids.clone(),
                        None => content
                            .links
                            .iter()
                            .map(|_| anchors::allocate_slots(1))
                            .collect::<Result<Vec<_>, MarkdownError>>()?,
                    },
                ))
            })
            .collect::<Result<Vec<_>, MarkdownError>>()?;
        self.blocks = contents
            .into_iter()
            .zip(blocks)
            .map(|(content, (id, shaped, link_ids))| Block {
                id,
                content,
                shaped,
                link_ids,
            })
            .collect();
        self.source = text.to_owned();
        self.identity = Some(identity);
        self.revision = revision;
        Ok(true)
    }

    pub fn len(&self) -> usize {
        self.row_starts().count()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Release derived glyph/layout snapshots without changing the document,
    /// block/link identities, accepted source revision or logical selections.
    /// A virtualized host can retain source history while bounding shaped text.
    pub fn release_layout(&mut self) -> usize {
        self.blocks
            .iter_mut()
            .filter_map(|block| block.shaped.take())
            .count()
    }

    pub fn sync_layout(
        &mut self,
        scope: ElementScope,
        layout: &LayoutTree,
        context: &mut TextInputContext,
        theme: WidgetTheme,
    ) -> Result<bool, MarkdownError> {
        let mut changed = false;
        let revision = context.measurement_revision();
        for block in &mut self.blocks {
            let Some(node) = layout.node(scope.element(block.id.0 + 2)) else {
                continue;
            };
            let width = text_input_wrap_width(node.rect);
            let size = block.size(theme);
            if block
                .shaped
                .as_ref()
                .is_some_and(|(current, current_size, current_revision, _)| {
                    *current == width && *current_size == size && current_revision == &revision
                })
            {
                continue;
            }
            let snapshot = context
                .shape_styled_text_with_options(
                    &block.content.text,
                    size,
                    if block.content.code {
                        FontRole::Mono
                    } else {
                        FontRole::Ui
                    },
                    &block.content.spans,
                    width,
                    vesta_gpui::StyledTextOptions {
                        alignment: block
                            .content
                            .cell
                            .map_or(vesta_gpui::StyledTextAlignment::Start, |cell| {
                                cell.alignment
                            }),
                        break_long_words: block.content.cell.is_some(),
                    },
                )
                .map_err(MarkdownError::Layout)?;
            block.shaped = Some((width, size, revision.clone(), snapshot));
            changed = true;
        }
        Ok(changed)
    }

    pub fn view(
        &self,
        scope: ElementScope,
        range: std::ops::Range<usize>,
        theme: WidgetTheme,
    ) -> Element {
        let mut root =
            Element::container(scope.element(0), scope.identity(0, "markdown.blocks").key).styled(
                Style::default()
                    .axis(Axis::Column)
                    .size(Dimension::Percent(1.0), Dimension::Auto)
                    .gap(10.0),
            );
        let mut elements = Vec::new();
        for block in self.blocks_in(range.clone()) {
            let id = block.id.0;
            let content = &block.content;
            let document = self.identity.map_or(0, MarkdownDocumentId::key);
            let mut row = Element::container(
                scope.element(id),
                scope
                    .identity(id, format!("markdown.{document}.block.{id}"))
                    .key,
            )
            .styled(
                Style::default()
                    .axis(Axis::Row)
                    .size(Dimension::Percent(1.0), Dimension::Auto)
                    .padding(Insets {
                        left: content.indent as f32 * 16.0,
                        ..Insets::all(0.0)
                    }),
            );
            if content.code {
                row.style.visual.background = Some(theme.palette.surface);
                row.style.visual.border_color = Some(theme.palette.border);
                row.style.visual.border_width = 1.0;
                row.style.visual.corner_radius = theme.metrics.radius;
            }
            if content.quote {
                row.style.visual.background = Some(theme.palette.panel);
                row.style.visual.border_color = None;
                row.style.visual.border_width = 0.0;
                row.style.padding.left += 12.0;
                row.style.padding.right = 8.0;
                row.style.padding.top = 4.0;
                row.style.padding.bottom = 4.0;
                row = row.child(
                    Element::container(
                        scope.element(id + 3),
                        scope
                            .identity(id + 3, format!("markdown.{document}.quote.{id}"))
                            .key,
                    )
                    .styled(
                        Style::default()
                            .absolute(content.indent as f32 * 16.0, 4.0)
                            .inset_bottom(4.0)
                            .size(Dimension::Px(3.0), Dimension::Auto)
                            .background(theme.palette.border)
                            .pointer_events(false),
                    ),
                );
            }
            if !content.marker.is_empty() {
                // Rich-text paragraphs have an 8px content inset; keep list
                // markers inside the same prose edge as ordinary paragraphs.
                row.style.padding.left += 8.0;
                let mut next = id + 1;
                let mut marker = label(
                    scope,
                    &mut next,
                    &content.marker,
                    theme.typography.font_size + 1.0,
                    theme.palette.text_muted,
                );
                marker.style.width = if content.marker == "•" {
                    Dimension::Auto
                } else {
                    Dimension::Px(36.0)
                };
                marker.style.flex_shrink = 0.0;
                marker.style.text_v_align = vesta_gpui::TextVAlign::Start;
                row = row.child(marker);
            }
            let mut body = if let Some((_, _, _, snapshot)) = &block.shaped {
                rich_text_paragraph(
                    WidgetIdentity::new(
                        scope.element(id + 2),
                        scope
                            .identity(id + 2, format!("markdown.{document}.text.{id}"))
                            .key
                            .as_str(),
                    ),
                    &content.text,
                    snapshot.clone(),
                    theme,
                )
            } else {
                let mut next = id + 2;
                wrapped(
                    scope,
                    &mut next,
                    &content.text,
                    block.size(theme),
                    theme.palette.text,
                )
            };
            body.style.width = Dimension::Auto;
            body.key = scope
                .identity(id + 2, format!("markdown.{document}.text.{id}"))
                .key;
            body.style.min_width = Dimension::Px(0.0);
            body.style.flex_grow = 1.0;
            body.style.flex_basis = Some(Dimension::Px(0.0));
            if !content.tooltip.is_empty() {
                body.tooltip = Some(content.tooltip.clone());
                row.tooltip = Some(content.tooltip.clone());
            }
            elements.push(row.child(body));
        }
        root.children = table::wrap(self, scope, range, elements, theme);
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_context_preserves_fences_references_and_safe_unicode_spans() {
        let source = format!(
            "# 标题\n\n中文 **粗体** 与 `code` [标签][ref]\n\n- 项目\n\n```rust\n{}## not a heading\n```\n\n<script>unsafe()</script>\n\n![替代文本](https://invalid.example/image.png)\n\n[ref]: ../secret.md",
            "line\n".repeat(210)
        );
        let document = MarkdownView::parse(&source).expect("document");
        assert_eq!(document.blocks[0].content.heading, 1);
        assert_eq!(document.blocks[1].content.text, "中文 粗体 与 code 标签");
        assert!(document.blocks[1].content.tooltip.contains("../secret.md"));
        assert!(
            document
                .blocks
                .iter()
                .any(|block| block.content.code && block.content.text.contains("## not a heading"))
        );
        assert!(
            document
                .blocks
                .iter()
                .any(|block| block.content.text.contains("<script>"))
        );
        assert!(
            document
                .blocks
                .iter()
                .any(|block| block.content.text.contains("替代文本"))
        );
        for block in &document.blocks {
            let block = &block.content;
            for span in &block.spans {
                assert!(
                    block.text.is_char_boundary(span.range.start)
                        && block.text.is_char_boundary(span.range.end)
                );
            }
        }
        assert!(document.blocks[1].content.spans.iter().any(
            |span| span.bold && &document.blocks[1].content.text[span.range.clone()] == "粗体"
        ));
        assert!(matches!(
            MarkdownView::parse(&"x".repeat(vesta_gpui::TEXT_INPUT_MAX_BYTES + 1)),
            Err(MarkdownError::ParagraphTooLong { block: 0 })
        ));
        let scope = vesta_gpui::ElementIdAllocator
            .allocate_scope()
            .expect("document scope");
        let other = vesta_gpui::ElementIdAllocator
            .allocate_scope()
            .expect("other scope");
        let root = vesta_gpui::ElementIdAllocator
            .allocate_scope()
            .expect("parent scope");
        let theme = WidgetTheme::light();
        let first = document.view(scope, 1..4, theme);
        assert_eq!(first.children.len(), 3);
        let paragraph = document
            .paragraph_id(scope, document.block_id_at(1).expect("paragraph identity"))
            .expect("paragraph element");
        assert_eq!(
            first.find(paragraph).expect("windowed paragraph").key,
            document
                .view(scope, 0..2, theme)
                .find(paragraph)
                .expect("same paragraph")
                .key
        );
        let combined = Element::container(root.element(0), root.identity(0, "documents").key)
            .styled(Style::default().axis(Axis::Column))
            .child(first)
            .child(document.view(other, 1..4, theme));
        vesta_gpui::LayoutEngine
            .compute(&combined, vesta_gpui::Viewport::new(400.0, 500.0, 1.0))
            .expect("two views have independent identities");
        let mut document = document;
        let mut context = TextInputContext::default();
        let snapshot = |document: &MarkdownView| {
            let view = document.view(scope, 1..2, theme);
            let vesta_gpui::ElementKind::TextInput { snapshot, .. } =
                &view.find(paragraph).expect("shaped paragraph").kind
            else {
                panic!("paragraph must publish shaped text");
            };
            snapshot.clone()
        };
        let project = |document: &MarkdownView, theme| {
            vesta_gpui::LayoutEngine
                .compute(
                    &document.view(scope, 1..2, theme),
                    vesta_gpui::Viewport::new(400.0, 500.0, 1.0),
                )
                .expect("document layout")
        };
        let layout = project(&document, theme);
        assert!(
            document
                .sync_layout(scope, &layout, &mut context, theme)
                .expect("shape")
        );
        let initial_snapshot = snapshot(&document);
        let layout = project(&document, theme);
        assert!(
            !document
                .sync_layout(scope, &layout, &mut context, theme)
                .expect("unchanged")
        );
        assert_eq!(snapshot(&document), initial_snapshot);
        let font = include_bytes!("../../vesta-gpui/tests/fixtures/MaterialIcons-Regular.ttf");
        assert!(context.register_font(std::sync::Arc::new(font.to_vec())));
        assert!(
            document
                .sync_layout(scope, &layout, &mut context, theme)
                .expect("successful registration invalidates equal-width snapshot")
        );
        let registered_snapshot = snapshot(&document);
        assert_ne!(registered_snapshot, initial_snapshot);
        assert!(
            !document
                .sync_layout(scope, &layout, &mut context, theme)
                .expect("registered font snapshot converges")
        );
        assert_eq!(snapshot(&document), registered_snapshot);
        context.set_font_stack(FontRole::Ui, "Material Icons");
        assert!(
            document
                .sync_layout(scope, &layout, &mut context, theme)
                .expect("role stack invalidates equal-width snapshot")
        );
        assert!(
            !document
                .sync_layout(scope, &layout, &mut context, theme)
                .expect("role stack snapshot converges")
        );
        context.set_font_stack(FontRole::Ui, "Material Icons");
        assert!(!context.register_font(std::sync::Arc::new(Vec::new())));
        assert!(
            !document
                .sync_layout(scope, &layout, &mut context, theme)
                .expect("no-op stack and rejected registration preserve cache")
        );
        let original_snapshot = snapshot(&document);
        let mut other_context = TextInputContext::default();
        assert!(other_context.register_font(std::sync::Arc::new(font.to_vec())));
        other_context.set_font_stack(FontRole::Ui, "Material Icons");
        assert!(
            document
                .sync_layout(scope, &layout, &mut other_context, theme)
                .expect("different context with equal font revision invalidates snapshot")
        );
        let replacement_snapshot = snapshot(&document);
        assert_ne!(replacement_snapshot, original_snapshot);
        assert!(
            !document
                .sync_layout(scope, &layout, &mut other_context, theme)
                .expect("replacement context snapshot converges")
        );
        assert_eq!(snapshot(&document), replacement_snapshot);
        assert!(
            document
                .sync_layout(scope, &layout, &mut context, theme)
                .expect("returning to original context invalidates replacement snapshot")
        );
        let mut resized_theme = theme;
        resized_theme.typography.font_size += 2.0;
        assert!(
            document
                .sync_layout(scope, &layout, &mut context, resized_theme)
                .expect("new typography")
        );
        let rendered = document.view(scope, 1..2, resized_theme);
        let paragraph = rendered.find(paragraph).expect("read-only paragraph");
        assert!(!paragraph.style.focusable && !paragraph.style.pointer_events);
        assert!(matches!(
            paragraph.kind,
            vesta_gpui::ElementKind::TextInput { focused: false, .. }
        ));
        let viewport = vesta_gpui::Viewport::new(400.0, 500.0, 1.0);
        let layout = vesta_gpui::LayoutEngine
            .compute(&rendered, viewport)
            .expect("read-only Markdown layout");
        let bounds = layout.node(paragraph.id).expect("paragraph bounds").rect;
        let cursor = vesta_gpui::Point::new(
            bounds.x + bounds.width * 0.5,
            bounds.y + bounds.height * 0.5,
        );
        let hit = vesta_gpui::HitTestTree::build(&rendered, &layout)
            .hit(cursor)
            .expect("read-only block pointer target");
        assert_ne!(hit, paragraph.id);
        assert!(rendered.find(hit).expect("hit block").tooltip.is_some());
        let route =
            vesta_gpui::ActionRoute::<u8>::build(&rendered, []).expect("read-only tooltip route");
        assert!(route.focus_order().is_empty());
        assert!(route.action_for_hit(hit).is_none());
        let popup = vesta_gpui::ElementIdAllocator
            .allocate_scope()
            .expect("tooltip scope");
        let delay = std::time::Duration::from_millis(500);
        let now = std::time::Instant::now();
        let mut tooltip = crate::TooltipController::new(delay);
        for timestamp in [now, now + delay] {
            let mut projection = rendered.clone();
            tooltip
                .layout(
                    &mut projection,
                    crate::TooltipFrame {
                        scope: popup,
                        viewport,
                        route: &route,
                        focus: None,
                        focus_root: None,
                        cursor,
                        active: true,
                        theme: resized_theme,
                        now: timestamp,
                    },
                    &mut context,
                )
                .expect("Markdown destination tooltip");
            assert_eq!(projection.find(popup.element(1)).is_some(), timestamp > now);
        }
        let original = match &paragraph.kind {
            vesta_gpui::ElementKind::TextInput { snapshot, .. } => snapshot.clone(),
            _ => panic!("shaped paragraph"),
        };
        document
            .replace(&format!("new block\n\n{source}"))
            .expect("insert before retained text");
        let view = document.view(scope, 0..document.len(), resized_theme);
        let vesta_gpui::ElementKind::TextInput {
            snapshot: retained, ..
        } = &view
            .find(paragraph.id)
            .expect("retained block identity")
            .kind
        else {
            panic!("retained snapshot");
        };
        assert_eq!(
            retained, &original,
            "unchanged unique block keeps its derived snapshot until width/font changes"
        );
    }
}
