//! Taffy node reuse over the current declarative tree, with no second layout algorithm.

use super::*;
use crate::{TextInputContext, TextMeasurementRevision};

pub struct RetainedLayoutEngine {
    taffy: LayoutStorage,
    nodes: HashMap<ElementId, NodeId>,
    scroll_clips: HashMap<ElementId, crate::ClipMode>,
    text_source: Option<TextMeasurementRevision>,
    viewport: Option<Viewport>,
}

impl Default for RetainedLayoutEngine {
    fn default() -> Self {
        Self {
            taffy: LayoutStorage::new(),
            nodes: HashMap::new(),
            scroll_clips: HashMap::new(),
            text_source: None,
            viewport: None,
        }
    }
}

impl RetainedLayoutEngine {
    /// Releases the retained tree when its window projection is no longer live.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Keeps only the current tree. Each window owns its own engine; replacing the
    /// text context or its fonts invalidates all affected text measurements.
    pub fn compute_with_text_context(
        &mut self,
        root: &Element,
        viewport: Viewport,
        context: &mut TextInputContext,
    ) -> Result<LayoutTree, LayoutError> {
        let _trace = crate::trace::span("gpui.layout", "layout.retained");
        validate_layout(root, viewport)?;
        let source = context.measurement_revision();
        let text_changed = self.text_source.as_ref().is_none_or(|old| old != &source);
        let result = self.compute_validated(root, viewport, text_changed, &mut |request| {
            shaped_metrics(context, request)
        });
        if result.is_ok() {
            self.text_source = Some(source);
        }
        result
    }

    fn compute_validated(
        &mut self,
        root: &Element,
        viewport: Viewport,
        text_changed: bool,
        measure: &mut impl FnMut(TextLayoutRequest<'_>) -> TextLayoutMetrics,
    ) -> Result<LayoutTree, LayoutError> {
        let result = self.update_tree(root, viewport, text_changed, measure);
        if result.is_err() {
            *self = Self::default();
        }
        result
    }

    fn update_tree(
        &mut self,
        root: &Element,
        viewport: Viewport,
        text_changed: bool,
        measure: &mut impl FnMut(TextLayoutRequest<'_>) -> TextLayoutMetrics,
    ) -> Result<LayoutTree, LayoutError> {
        let mut live = HashSet::new();
        let mut changed = self.viewport != Some(viewport);
        let root_node = self.sync_node(
            root,
            false,
            Axis::Column,
            false,
            text_changed,
            &mut live,
            &mut changed,
            measure,
        )?;
        let mut removed = self
            .nodes
            .keys()
            .copied()
            .filter(|id| !live.contains(id))
            .collect::<Vec<_>>();
        removed.sort();
        for id in removed {
            changed = true;
            self.scroll_clips.remove(&id);
            if let Some(node) = self.nodes.remove(&id) {
                // Taffy's remove only deletes topology, so release text context explicitly.
                self.taffy
                    .set_node_context(node, None)
                    .map_err(layout_error)?;
                self.taffy.remove(node).map_err(layout_error)?;
            }
        }
        if changed {
            compute_scroll_layout(
                root,
                &mut self.taffy,
                &self.nodes,
                root_node,
                viewport,
                true,
                measure,
            )?;
        }
        self.viewport = Some(viewport);
        layout_output(root, viewport, &self.taffy, &self.nodes)
    }

    #[allow(clippy::too_many_arguments)]
    fn sync_node(
        &mut self,
        element: &Element,
        in_scrollbox: bool,
        parent_axis: Axis,
        parent_is_grid: bool,
        text_changed: bool,
        live: &mut HashSet<ElementId>,
        changed: &mut bool,
        measure: &mut impl FnMut(TextLayoutRequest<'_>) -> TextLayoutMetrics,
    ) -> Result<NodeId, LayoutError> {
        live.insert(element.id);
        let clip = super::scroll::scroll_extent_clip(element);
        if self.scroll_clips.insert(element.id, clip) != Some(clip) {
            *changed = true;
        }
        let is_scrollbox = matches!(element.kind, crate::ElementKind::ScrollBox(_));
        let mut children = Vec::with_capacity(element.children.len());
        for child in &element.children {
            children.push(self.sync_node(
                child,
                is_scrollbox,
                element.style.axis,
                element.style.grid_columns.is_some(),
                text_changed,
                live,
                changed,
                measure,
            )?);
        }
        let mut style = node_style(element, in_scrollbox, parent_axis, parent_is_grid)?;
        let node = if let Some(node) = self.nodes.get(&element.id).copied() {
            let old = self.taffy.style(node).map_err(layout_error)?;
            if is_scrollbox
                && style.scrollbar_width == crate::SCROLLBAR_GUTTER
                && old.scrollbar_width == crate::SCROLLBAR_GUTTER
            {
                style.overflow = old.overflow;
            }
            if self.taffy.style(node).map_err(layout_error)? != &style {
                *changed = true;
                self.taffy.set_style(node, style).map_err(layout_error)?;
            }
            if !self.taffy.children_match(node, &children) {
                *changed = true;
                self.taffy
                    .set_children(node, &children)
                    .map_err(layout_error)?;
            }
            node
        } else {
            *changed = true;
            let node = self.taffy.new_leaf(style).map_err(layout_error)?;
            // This subtree may already belong to an older retained parent.
            // set_children detaches it there before attaching it to the new node.
            self.taffy
                .set_children(node, &children)
                .map_err(layout_error)?;
            self.nodes.insert(element.id, node);
            node
        };
        let text = if children.is_empty() {
            match &element.kind {
                crate::ElementKind::Text(text) => Some(text),
                _ => None,
            }
        } else {
            None
        };
        if let Some(text) = text {
            let old = self.taffy.get_node_context(node);
            let unchanged = !text_changed
                && old.is_some_and(|old| {
                    old.text == *text
                        && old.font_size == element.style.visual.font_size
                        && old.role == element.style.visual.font_role
                        && old.wrap == element.style.text_wrap
                        && old.vertical_align == element.style.text_v_align
                });
            if !unchanged {
                *changed = true;
                let context = TextMeasure {
                    element: element.id,
                    intrinsic: measure(TextLayoutRequest {
                        text,
                        font_size: element.style.visual.font_size,
                        role: element.style.visual.font_role,
                        width: None,
                        height: None,
                        wrap: element.style.text_wrap,
                        vertical_align: element.style.text_v_align,
                    })
                    .checked(element.id)?,
                    text: text.clone(),
                    font_size: element.style.visual.font_size,
                    role: element.style.visual.font_role,
                    wrap: element.style.text_wrap,
                    vertical_align: element.style.text_v_align,
                };
                self.taffy
                    .set_node_context(node, Some(context))
                    .map_err(layout_error)?;
            }
        } else if self.taffy.get_node_context(node).is_some() {
            *changed = true;
            self.taffy
                .set_node_context(node, None)
                .map_err(layout_error)?;
        }
        Ok(node)
    }
}

fn layout_error(error: impl fmt::Display) -> LayoutError {
    LayoutError::LayoutEngine(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, ElementKind, Style};

    #[test]
    fn retained_layout_reparents_shared_content_before_removing_old_popup_ancestors() {
        let mut engine = RetainedLayoutEngine::default();
        let mut context = TextInputContext::default();
        let viewport = Viewport::new(820.0, 580.0, 1.0);
        for popup in [Some(100), Some(101), Some(100), None, Some(102), None] {
            let mut root = Element::container(ElementId::new(1), ElementKey::new("root"))
                .styled(Style::default().size(Dimension::Percent(1.0), Dimension::Percent(1.0)));
            if let Some(popup) = popup {
                root = root.child(
                    Element::container(ElementId::new(popup), ElementKey::new("popup"))
                        .styled(Style::default().size(Dimension::Px(220.0), Dimension::Auto))
                        .child(
                            Element::container(ElementId::new(2), ElementKey::new("content"))
                                .child(Element::text(
                                    ElementId::new(3),
                                    ElementKey::new("label"),
                                    format!("Menu {popup}"),
                                )),
                        ),
                );
            }
            let actual = engine
                .compute_with_text_context(&root, viewport, &mut context)
                .expect("replace and dismiss popup");
            let reference = LayoutEngine
                .compute_with_text_context(&root, viewport, &mut context)
                .expect("stateless reference");
            assert_eq!(actual.nodes(), reference.nodes());
            assert_eq!(engine.nodes.len(), if popup.is_some() { 4 } else { 1 });
        }
    }

    #[test]
    fn retained_layout_reuses_measurement_and_matches_stateless_across_tree_and_font_changes() {
        let id = ElementId::new;
        let mut root = Element::container(id(1), ElementKey::new("root"))
            .styled(Style::default().size(Dimension::Percent(1.0), Dimension::Percent(1.0)))
            .child(
                Element::scroll_box(id(2), ElementKey::new("scroll"), Point::default())
                    .styled(
                        Style::default()
                            .size(Dimension::Percent(1.0), Dimension::Px(20.0))
                            .scrollbar(crate::ScrollbarStyle {
                                gutter: crate::ScrollbarGutter::Reserve,
                                ..crate::ScrollbarStyle::default()
                            }),
                    )
                    .child(
                        Element::text(
                            id(3),
                            ElementKey::new("text"),
                            "Wrapped content wraps over a narrow viewport",
                        )
                        .styled(Style::default().text_wrap(true)),
                    ),
            )
            .child(
                Element::container(id(4), ElementKey::new("other"))
                    .styled(Style::default().size(Dimension::Percent(1.0), Dimension::Px(90.0))),
            );
        let viewport = Viewport::new(240.0, 200.0, 1.0);
        let mut engine = RetainedLayoutEngine::default();
        let mut context = TextInputContext::default();
        let compare = |engine: &mut RetainedLayoutEngine,
                       root: &Element,
                       viewport,
                       context: &mut TextInputContext| {
            let actual = engine
                .compute_with_text_context(root, viewport, context)
                .expect("retained");
            let reference = LayoutEngine
                .compute_with_text_context(root, viewport, context)
                .expect("reference");
            assert_eq!(actual.nodes(), reference.nodes());
            actual
        };
        compare(&mut engine, &root, viewport, &mut context);
        let original_node = engine.nodes[&id(3)];
        let mut calls = 0;
        let unchanged = engine
            .compute_validated(&root, viewport, false, &mut |_| {
                calls += 1;
                TextLayoutMetrics::default()
            })
            .expect("unchanged");
        assert_eq!(
            calls, 0,
            "a steady frame does not call the measurement source"
        );
        root.style.flow_direction = Some(FlowDirection::RightToLeft);
        let projected = engine
            .compute_validated(&root, viewport, false, &mut |_| {
                panic!("a flow-only projection does not remeasure content")
            })
            .expect("RTL projection");
        assert!(
            projected
                .nodes()
                .iter()
                .all(|node| node.flow_direction == FlowDirection::RightToLeft)
        );
        compare(&mut engine, &root, viewport, &mut context);
        root.children[0].children[0].style.visual.text_color = Color::rgb8(20, 30, 40);
        root.children[0].kind = ElementKind::ScrollBox(Point::new(0.0, 9.0));
        let scrolled = compare(&mut engine, &root, viewport, &mut context);
        assert_eq!(
            scrolled.node(id(3)).expect("scrolled").rect.y,
            unchanged.node(id(3)).expect("initial").rect.y - 9.0
        );
        compare(
            &mut engine,
            &root,
            Viewport::new(100.0, 200.0, 1.5),
            &mut context,
        );
        let text = root.children[0].children.remove(0);
        root.children[1].children.push(text);
        compare(&mut engine, &root, viewport, &mut context);
        assert_eq!(
            engine.nodes[&id(3)],
            original_node,
            "cross-parent movement preserves the node"
        );
        root.children.swap(0, 1);
        compare(&mut engine, &root, viewport, &mut context);
        root.children[0].children[0].kind = ElementKind::Text("Updated text".into());
        compare(&mut engine, &root, viewport, &mut context);
        let previous_revision = engine.text_source.clone().expect("source");
        context.set_font_stack(FontRole::Ui, "Host replacement family");
        compare(&mut engine, &root, viewport, &mut context);
        assert_ne!(
            engine.text_source.as_ref().expect("source"),
            &previous_revision
        );
        let mut new_context = TextInputContext::default();
        compare(&mut engine, &root, viewport, &mut new_context);
        assert_ne!(
            engine.text_source.as_ref().expect("source"),
            &context.measurement_revision()
        );
        root.children[0].children.clear();
        compare(&mut engine, &root, viewport, &mut new_context);
        assert!(!engine.nodes.contains_key(&id(3)));
        let preserved = engine.nodes.len();
        root.children.push(root.children[0].clone());
        assert!(matches!(
            engine.compute_with_text_context(&root, viewport, &mut new_context),
            Err(LayoutError::DuplicateElementId(_))
        ));
        assert_eq!(
            engine.nodes.len(),
            preserved,
            "invalid input does not corrupt the retained tree"
        );
        root.children.pop();
        compare(&mut engine, &root, viewport, &mut new_context);
        engine.clear();
        assert!(engine.nodes.is_empty());
        assert!(engine.text_source.is_none());
        compare(&mut engine, &root, viewport, &mut new_context);
    }
}
