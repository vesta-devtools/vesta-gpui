//! Taffy's public algorithms with shaped baselines; topology and cache stay in TaffyTree.

use super::*;
use std::cell::Cell;
use taffy::{
    CacheTree, CompactLength, Display, Layout, LayoutFlexboxContainer, LayoutGridContainer,
    LayoutInput, LayoutOutput, LayoutPartialTree, RoundTree, RunMode, SizingMode,
    TraversePartialTree, TraverseTree, compute_cached_layout, compute_flexbox_layout,
    compute_grid_layout, compute_hidden_layout, compute_leaf_layout, compute_root_layout,
    round_layout,
};

#[derive(Clone, Copy, Default)]
struct Geometry {
    raw: Layout,
    rounded: Layout,
    rounding_origin: Point,
}

pub(super) struct LayoutStorage {
    tree: TaffyTree<TextMeasure>,
    geometry: HashMap<NodeId, Geometry>,
}

impl Default for LayoutStorage {
    fn default() -> Self {
        Self {
            tree: TaffyTree::new(),
            geometry: HashMap::new(),
        }
    }
}

impl LayoutStorage {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn new_leaf(&mut self, style: TaffyStyle) -> taffy::TaffyResult<NodeId> {
        self.tree.new_leaf(style)
    }
    pub(super) fn new_leaf_with_context(
        &mut self,
        style: TaffyStyle,
        context: TextMeasure,
    ) -> taffy::TaffyResult<NodeId> {
        self.tree.new_leaf_with_context(style, context)
    }
    pub(super) fn new_with_children(
        &mut self,
        style: TaffyStyle,
        children: &[NodeId],
    ) -> taffy::TaffyResult<NodeId> {
        self.tree.new_with_children(style, children)
    }
    pub(super) fn style(&self, node: NodeId) -> taffy::TaffyResult<&TaffyStyle> {
        self.tree.style(node)
    }
    pub(super) fn set_style(&mut self, node: NodeId, style: TaffyStyle) -> taffy::TaffyResult<()> {
        self.tree.set_style(node, style)
    }
    pub(super) fn children_match(&self, node: NodeId, children: &[NodeId]) -> bool {
        self.tree.child_ids(node).eq(children.iter().copied())
    }
    pub(super) fn set_children(
        &mut self,
        node: NodeId,
        children: &[NodeId],
    ) -> taffy::TaffyResult<()> {
        self.tree.set_children(node, children)
    }
    pub(super) fn get_node_context(&self, node: NodeId) -> Option<&TextMeasure> {
        self.tree.get_node_context(node)
    }
    pub(super) fn set_node_context(
        &mut self,
        node: NodeId,
        context: Option<TextMeasure>,
    ) -> taffy::TaffyResult<()> {
        self.tree.set_node_context(node, context)
    }

    pub(super) fn remove(&mut self, node: NodeId) -> taffy::TaffyResult<NodeId> {
        let removed = self.tree.remove(node)?;
        self.geometry.remove(&node);
        Ok(removed)
    }

    pub(super) fn layout(&self, node: NodeId) -> taffy::TaffyResult<&Layout> {
        self.geometry
            .get(&node)
            .map(|geometry| &geometry.rounded)
            .ok_or(taffy::TaffyError::InvalidInputNode(node))
    }

    pub(super) fn compute(
        &mut self,
        root: NodeId,
        available: TaffySize<AvailableSpace>,
        measure: &mut impl FnMut(&TextMeasure, Option<f32>, bool, Option<f32>) -> TextLayoutMetrics,
    ) -> Result<(), LayoutError> {
        self.tree.style(root).map_err(error)?;
        let mut view = Solver {
            storage: self,
            measure,
            invalid: Cell::new(None),
            invalid_text: Cell::new(None),
            error_style: TaffyStyle::DEFAULT,
        };
        compute_root_layout(&mut view, root, available);
        round_layout(&mut view, root);
        if let Some(element) = view.invalid_text.get() {
            return Err(LayoutError::InvalidTextMetrics(element));
        }
        if let Some(node) = view.invalid.get() {
            return Err(error(taffy::TaffyError::InvalidInputNode(node)));
        }
        Ok(())
    }
}

fn error(error: impl fmt::Display) -> LayoutError {
    LayoutError::LayoutEngine(error.to_string())
}

struct Solver<'a, F> {
    storage: &'a mut LayoutStorage,
    measure: &'a mut F,
    invalid: Cell<Option<NodeId>>,
    invalid_text: Cell<Option<ElementId>>,
    error_style: TaffyStyle,
}

impl<F> Solver<'_, F> {
    fn style(&self, node: NodeId) -> &TaffyStyle {
        match self.storage.tree.style(node) {
            Ok(style) => style,
            Err(_) => {
                self.invalid.set(Some(node));
                &self.error_style
            }
        }
    }
}

impl<F> TraversePartialTree for Solver<'_, F> {
    type ChildIter<'a>
        = <TaffyTree<TextMeasure> as TraversePartialTree>::ChildIter<'a>
    where
        Self: 'a;
    fn child_ids(&self, node: NodeId) -> Self::ChildIter<'_> {
        self.storage.tree.child_ids(node)
    }
    fn child_count(&self, node: NodeId) -> usize {
        self.storage.tree.child_count(node)
    }
    fn get_child_id(&self, node: NodeId, index: usize) -> NodeId {
        self.storage.tree.get_child_id(node, index)
    }
}

impl<F> TraverseTree for Solver<'_, F> {}

impl<F> CacheTree for Solver<'_, F> {
    fn cache_get(&self, node: NodeId, input: &LayoutInput) -> Option<LayoutOutput> {
        self.storage.tree.cache_get(node, input)
    }
    fn cache_store(&mut self, node: NodeId, input: &LayoutInput, output: LayoutOutput) {
        self.storage.tree.cache_store(node, input, output);
    }
    fn cache_clear(&mut self, node: NodeId) {
        self.storage.tree.cache_clear(node);
    }
}

impl<F> LayoutPartialTree for Solver<'_, F>
where
    F: FnMut(&TextMeasure, Option<f32>, bool, Option<f32>) -> TextLayoutMetrics,
{
    type CoreContainerStyle<'a>
        = &'a TaffyStyle
    where
        Self: 'a;
    type CustomIdent = <TaffyStyle as taffy::CoreStyle>::CustomIdent;

    fn get_core_container_style(&self, node: NodeId) -> Self::CoreContainerStyle<'_> {
        self.style(node)
    }
    fn set_unrounded_layout(&mut self, node: NodeId, layout: &Layout) {
        self.storage.geometry.entry(node).or_default().raw = *layout;
    }

    fn compute_child_layout(&mut self, node: NodeId, input: LayoutInput) -> LayoutOutput {
        if input.run_mode == RunMode::PerformHiddenLayout {
            return compute_hidden_layout(self, node);
        }
        compute_cached_layout(self, node, input, |tree, node, input| {
            match (tree.style(node).display, tree.child_count(node) > 0) {
                (Display::None, _) => compute_hidden_layout(tree, node),
                (Display::Flex, true) => compute_flexbox_layout(tree, node, input),
                (Display::Grid, true) => compute_grid_layout(tree, node, input),
                (_, false) => {
                    let style = tree.style(node).clone();
                    let context = tree.storage.tree.get_node_context(node);
                    // An auto-sized flex row first measures children before its
                    // percentage caps can resolve. Once its width is known,
                    // cross-size measurement must use the same wrapped width as
                    // final arrangement, rather than an earlier natural target.
                    let mut input = input;
                    if context.is_some_and(|context| context.wrap)
                        && input.sizing_mode == SizingMode::ContentSize
                        && style.max_size.width.tag() == CompactLength::PERCENT_TAG
                        && let (Some(width), Some(parent)) =
                            (input.known_dimensions.width, input.parent_size.width)
                    {
                        let minimum = match style.min_size.width.tag() {
                            CompactLength::LENGTH_TAG => Some(style.min_size.width.value()),
                            CompactLength::PERCENT_TAG => {
                                Some(parent * style.min_size.width.value())
                            }
                            _ => None,
                        };
                        let width = width.min(parent * style.max_size.width.value());
                        input.known_dimensions.width =
                            Some(minimum.map_or(width, |min| width.max(min)));
                    }
                    let mut output = compute_leaf_layout(
                        input,
                        &style,
                        |_, _| 0.0,
                        |known, available| {
                            let metrics = context.map_or(TextLayoutMetrics::default(), |context| {
                                let width = known.width.or(match available.width {
                                    AvailableSpace::Definite(width) => Some(width),
                                    _ => None,
                                });
                                let minimum = known.width.is_none()
                                    && matches!(available.width, AvailableSpace::MinContent);
                                let metrics = (tree.measure)(context, width, minimum, None);
                                if metrics.is_valid() {
                                    metrics
                                } else {
                                    tree.invalid_text.set(Some(context.element));
                                    TextLayoutMetrics::default()
                                }
                            });
                            TaffySize {
                                width: known.width.unwrap_or(metrics.size.width),
                                height: known.height.unwrap_or(metrics.size.height),
                            }
                        },
                    );
                    if let Some(context) =
                        context.filter(|context| context.intrinsic.first_baseline.is_some())
                    {
                        // The final leaf frame is the same frame static_text_view paints into.
                        let metrics = (tree.measure)(
                            context,
                            Some(output.size.width),
                            false,
                            Some(output.size.height),
                        );
                        if metrics.is_valid() {
                            output.first_baselines.y = metrics.first_baseline;
                        } else {
                            tree.invalid_text.set(Some(context.element));
                        }
                    }
                    output
                }
            }
        })
    }
}

impl<F> LayoutFlexboxContainer for Solver<'_, F>
where
    F: FnMut(&TextMeasure, Option<f32>, bool, Option<f32>) -> TextLayoutMetrics,
{
    type FlexboxContainerStyle<'a>
        = &'a TaffyStyle
    where
        Self: 'a;
    type FlexboxItemStyle<'a>
        = &'a TaffyStyle
    where
        Self: 'a;
    fn get_flexbox_container_style(&self, node: NodeId) -> Self::FlexboxContainerStyle<'_> {
        self.style(node)
    }
    fn get_flexbox_child_style(&self, node: NodeId) -> Self::FlexboxItemStyle<'_> {
        self.style(node)
    }
}

impl<F> LayoutGridContainer for Solver<'_, F>
where
    F: FnMut(&TextMeasure, Option<f32>, bool, Option<f32>) -> TextLayoutMetrics,
{
    type GridContainerStyle<'a>
        = &'a TaffyStyle
    where
        Self: 'a;
    type GridItemStyle<'a>
        = &'a TaffyStyle
    where
        Self: 'a;
    fn get_grid_container_style(&self, node: NodeId) -> Self::GridContainerStyle<'_> {
        self.style(node)
    }
    fn get_grid_child_style(&self, node: NodeId) -> Self::GridItemStyle<'_> {
        self.style(node)
    }
}

impl<F> RoundTree for Solver<'_, F> {
    fn get_unrounded_layout(&self, node: NodeId) -> Layout {
        match self.storage.geometry.get(&node) {
            Some(geometry) => geometry.raw,
            None => {
                self.invalid.set(Some(node));
                Layout::default()
            }
        }
    }
    fn set_final_layout(&mut self, node: NodeId, layout: &Layout) {
        let parent = self
            .storage
            .tree
            .parent(node)
            .and_then(|parent| self.storage.geometry.get(&parent))
            .map_or(Point::default(), |geometry| geometry.rounding_origin);
        let geometry = self.storage.geometry.entry(node).or_default();
        let origin = Point::new(
            parent.x + geometry.raw.location.x,
            parent.y + geometry.raw.location.y,
        );
        let mut rounded = *layout;
        // Sizes already use cumulative raw edges. Local positions must agree.
        rounded.location.x = origin.x.round() - parent.x.round();
        rounded.location.y = origin.y.round() - parent.y.round();
        geometry.rounding_origin = origin;
        geometry.rounded = rounded;
    }
}
