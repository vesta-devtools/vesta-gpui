//! Shared document views exercised by the standalone host in each window.

use super::*;
use vesta_gpui_widgets::{
    DiffLine, DiffViewMode, MarkdownError, MarkdownInteraction, MarkdownSurface, MarkdownView,
    diff_view, ellipsized_label, parse_unified_diff,
};
mod paging;
pub(crate) use paging::DocumentAction;

const ELLIPSIS_ID: u32 = 2; // Markdown block identities start at 10,000.
const LABEL_SOURCE: &str = "世界创作 / Vesta 地形 مرحبا 👨‍👩‍👧‍👦 / terrain/very_long_revision_with_完整原文_and_stable_identity.vda";

#[derive(Debug)]
pub(crate) enum GalleryDocumentError {
    Markdown(MarkdownError),
    Ellipsis(vesta_gpui::EllipsisError),
}

impl std::fmt::Display for GalleryDocumentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Markdown(error) => write!(formatter, "{error}"),
            Self::Ellipsis(error) => write!(formatter, "{error}"),
        }
    }
}

const MARKDOWN: &str = concat!(
    "# Vesta 文档预览\n\n中文、English 与 **粗体**、*斜体*、`inline code` 共同排版。缩放窗口会按实际宽度重新折行。\n\n## World brief\n\n- 山地、河流与森林\n- 可复用的图片、按钮和编辑字段\n- [Vesta framework](https://example.invalid/vesta)\n\n> 每个窗口独立排版，内容使用公共 Markdown View。\n\n```rust\nlet seed = 42;\nlet name = \"地形\";\n```\n",
    "\n## Structured data\n\n",
    "| 项目 | 数值 | 说明 |\n| :--- | ---: | :---: |\n",
    "| Seed | 42 | **固定** |\n",
    "| Origin | `0, 0` | [帮助](https://example.invalid/table) |\n",
    "| Empty | | — |\n",
);
const PATCH: &str = "diff --git a/world.rs b/world.rs\n--- a/world.rs\n+++ b/world.rs\n@@ -1,4 +1,5 @@\n fn world() {\n-    let seed = 1;\n+    let seed = 42;\n+    let name = \"地形 / Terrain\";\n     generate(seed);\n }\n";

pub(crate) struct GalleryDocuments {
    pub(crate) markdown: MarkdownView,
    diff: Vec<DiffLine>,
    ellipsized: Option<Element>,
    interaction: Option<MarkdownInteraction>,
    layout: Option<LayoutTree>,
    paging: paging::Paging,
}

impl Default for GalleryDocuments {
    fn default() -> Self {
        Self {
            markdown: MarkdownView::parse(MARKDOWN).expect("bounded gallery Markdown"),
            diff: parse_unified_diff(PATCH),
            ellipsized: None,
            interaction: None,
            layout: None,
            paging: Default::default(),
        }
    }
}

impl GalleryDocuments {
    pub(crate) fn for_sample(long: bool) -> Result<Self, MarkdownError> {
        let mut documents = Self::default();
        documents
            .paging
            .load_sample(long, &mut documents.markdown)?;
        Ok(documents)
    }

    pub(crate) fn is_paged(&self) -> bool {
        self.paging.is_paged()
    }

    pub(crate) fn replace_sample(
        &mut self,
        long: bool,
        input: &mut InputOwnership,
    ) -> Result<(), MarkdownError> {
        if self.paging.load_sample(long, &mut self.markdown)? {
            self.cancel_input(input);
            self.layout = None;
        }
        Ok(())
    }

    pub(crate) fn defer_focus(&mut self, input: &mut InputOwnership) {
        self.layout = None;
        if let Some(interaction) = &self.interaction {
            input.request_focus(interaction.target());
        }
    }
    pub(crate) fn range(&self) -> std::ops::Range<usize> {
        self.paging.range(self.markdown.len())
    }

    pub(crate) fn apply_action(
        &mut self,
        action: DocumentAction,
        input: &mut InputOwnership,
    ) -> bool {
        if !self.paging.apply(action, self.markdown.len()) {
            return false;
        }
        self.cancel_input(input);
        if action == DocumentAction::NextSample
            && let Some(interaction) = &self.interaction
            && let Some(selection) = interaction.selection()
            && let Ok(row) = self
                .markdown
                .selection_focus_row(interaction.target().window, selection)
        {
            self.paging.follow(row, self.markdown.len());
        }
        self.defer_focus(input);
        true
    }

    pub(crate) fn follow_selection(
        &mut self,
        input: &mut InputOwnership,
    ) -> Result<bool, vesta_gpui_widgets::MarkdownAnchorError> {
        let Some(interaction) = &self.interaction else {
            return Ok(false);
        };
        let Some(selection) = interaction.selection() else {
            return Ok(false);
        };
        let target = interaction.target();
        let row = self
            .markdown
            .selection_focus_row(target.window, selection)?;
        if !self.paging.follow(row, self.markdown.len()) {
            return Ok(false);
        }
        self.layout = None;
        input.request_focus(target);
        Ok(true)
    }
    pub(crate) fn enrich_accessibility(
        &self,
        semantics: &mut AccessibilityTree,
        update: &mut accesskit::TreeUpdate,
    ) -> Result<(), vesta_gpui_widgets::MarkdownProjectionError> {
        let Some(interaction) = &self.interaction else {
            return Ok(());
        };
        if semantics
            .node_for_element(interaction.target().element)
            .is_none()
        {
            return Ok(());
        }
        let Some(layout) = &self.layout else {
            return Ok(());
        };
        let projection = interaction.accessibility(&MarkdownSurface {
            window: interaction.target().window,
            document: &self.markdown,
            layout,
            range: self.range(),
        })?;
        semantics
            .enrich_read_only_document(update, projection)
            .map_err(vesta_gpui_widgets::MarkdownProjectionError::Accessibility)
    }

    pub(crate) fn sync_input(
        &mut self,
        window: UiWindowId,
        scope: ElementScope,
        layout: &LayoutTree,
        input: &mut InputOwnership,
    ) -> bool {
        let range = self.range();
        let initialized = self.interaction.is_none();
        let interaction = self
            .interaction
            .get_or_insert_with(|| MarkdownInteraction::new(window, scope));
        let changed = interaction.sync(
            &MarkdownSurface {
                window,
                document: &self.markdown,
                layout,
                range,
            },
            input,
        );
        if layout.node(interaction.target().element).is_some() {
            if self.layout.as_ref().is_none_or(|old| {
                old.viewport() != layout.viewport() || old.nodes() != layout.nodes()
            }) {
                self.layout = Some(layout.clone());
            }
        } else {
            self.layout = None;
        }
        changed || initialized
    }

    pub(crate) fn input_parts(
        &mut self,
        window: UiWindowId,
    ) -> Option<(&mut MarkdownInteraction, MarkdownSurface<'_>)> {
        let range = self.range();
        Some((
            self.interaction.as_mut()?,
            MarkdownSurface {
                window,
                document: &self.markdown,
                layout: self.layout.as_ref()?,
                range,
            },
        ))
    }

    pub(crate) fn cancel_input(&mut self, input: &mut InputOwnership) -> bool {
        let Some(interaction) = self.interaction.as_mut() else {
            return false;
        };
        let changed = interaction.cancel(input);
        self.cancel_pending_focus(input) || changed
    }

    pub(crate) fn cancel_pending_focus(&self, input: &mut InputOwnership) -> bool {
        self.interaction.as_ref().is_some_and(|interaction| {
            input.pending_focus_target(interaction.target().window) == Some(interaction.target())
                && input.cancel_focus_request()
        })
    }

    pub(crate) fn sync_layout(
        &mut self,
        scope: ElementScope,
        layout: &LayoutTree,
        context: &mut TextInputContext,
        theme: WidgetTheme,
    ) -> Result<bool, GalleryDocumentError> {
        let mut changed = self
            .markdown
            .sync_layout(scope, layout, context, theme)
            .map_err(GalleryDocumentError::Markdown)?;
        if let Some(node) = layout.node(scope.element(ELLIPSIS_ID)) {
            let mut label = ellipsized_label(
                WidgetIdentity::new(scope.element(ELLIPSIS_ID), "gallery.documents.ellipsis"),
                TextContent::Literal(LABEL_SOURCE),
                node.rect.width,
                theme,
                context,
            )
            .map_err(GalleryDocumentError::Ellipsis)?;
            label.style.width = Dimension::Percent(1.0);
            if self.ellipsized.as_ref() != Some(&label) {
                self.ellipsized = Some(label);
                changed = true;
            }
        }
        Ok(changed)
    }

    pub(crate) fn register_page(
        &self,
        pages: &mut HashMap<&'static str, Vec<Element>>,
        scopes: UiScopes,
        theme: WidgetTheme,
        actions: &mut Vec<(ElementId, DemoAction)>,
    ) {
        pages.insert("gallery.documents", self.sections(scopes, theme, actions));
    }

    pub(crate) fn sections(
        &self,
        scopes: UiScopes,
        theme: WidgetTheme,
        actions: &mut Vec<(ElementId, DemoAction)>,
    ) -> Vec<Element> {
        vec![
            Element::container(
                scopes.markdown.element(1),
                ElementKey::new("gallery.documents.markdown"),
            )
            .styled(
                Style::default()
                    .axis(Axis::Column)
                    .size(Dimension::Percent(1.0), Dimension::Auto)
                    .padding(Insets::all(12.0))
                    .gap(8.0),
            )
            .child(
                self.paging
                    .controls(scopes.markdown, self.markdown.len(), theme, actions),
            )
            .child(self.ellipsized.clone().unwrap_or_else(|| {
                let mut label = text_label(
                    WidgetIdentity::new(
                        scopes.markdown.element(ELLIPSIS_ID),
                        "gallery.documents.ellipsis",
                    ),
                    TextContent::Literal(LABEL_SOURCE),
                    theme,
                );
                label.style.width = Dimension::Percent(1.0);
                label.style.min_width = Dimension::Px(0.0);
                label.style.height = Dimension::Px(theme.typography.font_size * 1.5 + 4.0);
                label.style.text_wrap = false;
                label
            }))
            .child(self.interaction.as_ref().map_or_else(
                || self.markdown.view(scopes.markdown, self.range(), theme),
                |interaction| interaction.view(&self.markdown, self.range(), theme),
            )),
            Element::container(
                scopes.diff.element(1),
                ElementKey::new("gallery.documents.diff"),
            )
            .styled(
                Style::default()
                    .axis(Axis::Column)
                    .size(Dimension::Percent(1.0), Dimension::Auto)
                    .padding(Insets::all(12.0))
                    .gap(8.0),
            )
            .child(
                Element::text(
                    scopes.diff.element(2),
                    ElementKey::new("gallery.documents.diff.title"),
                    "Diff View · world.rs",
                )
                .styled(
                    Style::default()
                        .size(Dimension::Percent(1.0), Dimension::Px(28.0))
                        .text(theme.typography.font_size + 2.0, theme.palette.text)
                        .pointer_events(false),
                ),
            )
            .child(diff_view(
                scopes.diff,
                &self.diff,
                0..self.diff.len(),
                DiffViewMode::Unified,
                theme,
            )),
        ]
    }
}
