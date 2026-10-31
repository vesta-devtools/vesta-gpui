use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
};

use taffy::{AvailableSpace, NodeId, Size as TaffySize, Style as TaffyStyle, TaffyTree};

#[cfg(test)]
use crate::{Align, Dimension, FlexWrap, GridCell, GridTrack, Insets};
use crate::{
    Axis, Element, ElementId, ElementKey, FlowDirection, FontRole, Point, PositionMode, Rect, Size,
    Viewport, element::DuplicateIdentity,
};

mod style;
use style::node_style;
mod retained;
mod scroll;
pub use retained::RetainedLayoutEngine;
use scroll::{compute_scroll_layout, scroll_content_size};
mod solver;
use solver::LayoutStorage;
mod text_metrics;
use text_metrics::shaped_metrics;
pub use text_metrics::{TextLayoutMetrics, TextLayoutRequest};
mod measurement;
pub use measurement::{ContentConstraint, DesiredSize};
#[cfg(test)]
mod baseline_tests;
#[cfg(test)]
mod cumulative_rounding_tests;
#[cfg(test)]
mod flow_tests;

#[derive(Clone, Debug, PartialEq)]
struct TextMeasure {
    element: ElementId,
    intrinsic: TextLayoutMetrics,
    text: String,
    font_size: f32,
    role: FontRole,
    wrap: bool,
    vertical_align: crate::TextVAlign,
}

/// A derived layout record in stable depth-first element order.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutNode {
    pub id: ElementId,
    pub flow_direction: FlowDirection,
    pub rect: Rect,
    pub inherited_clip: Rect,
    /// ScrollBox 内容（未平移）包围盒尺寸；非 ScrollBox 或无可测内容时为 None。
    pub scroll_content: Option<(f32, f32)>,
    /// Content viewport after reserved gutters; the outer frame remains `rect`.
    pub scroll_viewport: Option<Rect>,
}

impl LayoutNode {
    #[must_use]
    pub fn scroll_viewport(&self) -> Rect {
        self.scroll_viewport.unwrap_or(self.rect)
    }

    /// Paint and input share tracks in the gutter and use the real visible extent.
    #[must_use]
    pub fn scrollbar_geometry(
        &self,
        offset: Point,
    ) -> (Option<Rect>, Option<Rect>, Option<Rect>, Option<Rect>) {
        crate::paint::scrollbar_geometry_for_node(self, offset)
    }
}

/// Immutable output from one layout pass.
#[derive(Clone, Debug)]
pub struct LayoutTree {
    viewport: Viewport,
    nodes: Vec<LayoutNode>,
    by_id: HashMap<ElementId, usize>,
}

impl LayoutTree {
    #[must_use]
    pub const fn viewport(&self) -> Viewport {
        self.viewport
    }

    #[must_use]
    pub fn nodes(&self) -> &[LayoutNode] {
        &self.nodes
    }

    #[must_use]
    pub fn node(&self, id: ElementId) -> Option<&LayoutNode> {
        self.by_id.get(&id).map(|index| &self.nodes[*index])
    }
}

/// Errors produced before a usable layout can be published.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayoutError {
    InvalidViewport,
    InvalidConstraint,
    InvalidAspectRatio(ElementId),
    InvalidBaselineAlignment(ElementId),
    InvalidTextMetrics(ElementId),
    NonFiniteStyle(ElementId),
    InvalidGeometry(ElementId),
    DuplicateElementId(ElementId),
    DuplicateElementKey(ElementKey),
    LayoutEngine(String),
    MissingLayout(ElementId),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidViewport => {
                formatter.write_str("viewport must have finite positive dimensions")
            }
            Self::InvalidConstraint => {
                formatter.write_str("content constraints must be finite positive dimensions")
            }
            Self::InvalidAspectRatio(id) => {
                write!(
                    formatter,
                    "element {} requires a finite positive aspect ratio",
                    id.get()
                )
            }
            Self::InvalidBaselineAlignment(id) => write!(
                formatter,
                "element {} cannot use baseline for line alignment",
                id.get()
            ),
            Self::InvalidTextMetrics(id) => write!(
                formatter,
                "element {} requires finite text dimensions and baseline",
                id.get()
            ),
            Self::NonFiniteStyle(id) => write!(
                formatter,
                "element {} requires finite layout and scroll inputs",
                id.get()
            ),
            Self::InvalidGeometry(id) => write!(
                formatter,
                "element {} produced invalid or overflowing layout geometry",
                id.get()
            ),
            Self::DuplicateElementId(id) => {
                write!(formatter, "duplicate element id {}", id.get())
            }
            Self::DuplicateElementKey(key) => {
                write!(formatter, "duplicate element key {}", key.as_str())
            }
            Self::LayoutEngine(message) => write!(formatter, "layout engine error: {message}"),
            Self::MissingLayout(id) => write!(formatter, "missing layout for element {}", id.get()),
        }
    }
}

impl Error for LayoutError {}

/// Stateless adapter between Vesta styles and Taffy's mature Flexbox implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct LayoutEngine;

impl LayoutEngine {
    /// Computes one deterministic layout tree for the requested viewport.
    pub fn compute(self, root: &Element, viewport: Viewport) -> Result<LayoutTree, LayoutError> {
        self.compute_with_text_measure(root, viewport, text_intrinsic_size)
    }

    /// Hosts with a font source can supply shaped text metrics for exact layout.
    pub fn compute_with_text_measure(
        self,
        root: &Element,
        viewport: Viewport,
        mut measure_text: impl FnMut(&str, f32, FontRole) -> Size,
    ) -> Result<LayoutTree, LayoutError> {
        self.compute_with_text_layout(root, viewport, |text, size, role, _width| {
            measure_text(text, size, role)
        })
    }

    /// Measure wrapped labels again when Taffy knows their available width.
    pub fn compute_with_text_layout(
        self,
        root: &Element,
        viewport: Viewport,
        mut measure_text: impl FnMut(&str, f32, FontRole, Option<f32>) -> Size,
    ) -> Result<LayoutTree, LayoutError> {
        self.compute_with_text_metrics(root, viewport, |request| {
            measure_text(request.text, request.font_size, request.role, request.width).into()
        })
    }

    /// Arranges using the same shaped text metrics and baselines as rendering.
    pub fn compute_with_text_context(
        self,
        root: &Element,
        viewport: Viewport,
        context: &mut crate::TextInputContext,
    ) -> Result<LayoutTree, LayoutError> {
        self.compute_with_text_metrics(root, viewport, |request| shaped_metrics(context, request))
    }

    pub fn compute_with_text_metrics(
        self,
        root: &Element,
        viewport: Viewport,
        mut measure_text: impl FnMut(TextLayoutRequest<'_>) -> TextLayoutMetrics,
    ) -> Result<LayoutTree, LayoutError> {
        let _trace = crate::trace::span("gpui.layout", "layout.compute");
        validate_layout(root, viewport)?;
        let mut taffy = LayoutStorage::new();
        let mut taffy_nodes = HashMap::new();
        let root_node = build_taffy_tree(
            root,
            &mut taffy,
            &mut taffy_nodes,
            false,
            Axis::Column,
            false,
            &mut measure_text,
        )?;
        compute_scroll_layout(
            root,
            &mut taffy,
            &taffy_nodes,
            root_node,
            viewport,
            false,
            &mut measure_text,
        )?;
        layout_output(root, viewport, &taffy, &taffy_nodes)
    }
}

fn validate_layout(root: &Element, viewport: Viewport) -> Result<(), LayoutError> {
    if !viewport.is_valid() {
        return Err(LayoutError::InvalidViewport);
    }
    validate_identity(root)
}

fn validate_identity(root: &Element) -> Result<(), LayoutError> {
    let mut ids = HashSet::new();
    let mut keys = HashSet::new();
    root.validate_unique_identity(&mut ids, &mut keys)
        .map_err(|duplicate| match duplicate {
            DuplicateIdentity::Id(id) => LayoutError::DuplicateElementId(id),
            DuplicateIdentity::Key(key) => LayoutError::DuplicateElementKey(key),
        })
}

fn compute_taffy_layout(
    taffy: &mut LayoutStorage,
    root_node: NodeId,
    viewport: Viewport,
    measure_text: &mut impl FnMut(TextLayoutRequest<'_>) -> TextLayoutMetrics,
) -> Result<(), LayoutError> {
    run_taffy_layout(
        taffy,
        root_node,
        TaffySize {
            width: AvailableSpace::Definite(viewport.width),
            height: AvailableSpace::Definite(viewport.height),
        },
        &mut |measure, width, _minimum, height| {
            if height.is_some() || (measure.wrap && width.is_some()) {
                measure_text(measure.request(width, height))
            } else {
                measure.intrinsic
            }
        },
    )
}

fn run_taffy_layout(
    taffy: &mut LayoutStorage,
    root_node: NodeId,
    available: TaffySize<AvailableSpace>,
    measure_text: &mut impl FnMut(&TextMeasure, Option<f32>, bool, Option<f32>) -> TextLayoutMetrics,
) -> Result<(), LayoutError> {
    taffy.compute(root_node, available, measure_text)
}

fn layout_output(
    root: &Element,
    viewport: Viewport,
    taffy: &LayoutStorage,
    taffy_nodes: &HashMap<ElementId, NodeId>,
) -> Result<LayoutTree, LayoutError> {
    let mut nodes = Vec::with_capacity(taffy_nodes.len());
    collect_layout(
        root,
        taffy,
        taffy_nodes,
        Point::default(),
        viewport.logical_rect(),
        FlowDirection::LeftToRight,
        &mut nodes,
    )?;
    let by_id = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id, index))
        .collect();
    Ok(LayoutTree {
        viewport,
        nodes,
        by_id,
    })
}

fn build_taffy_tree(
    element: &Element,
    taffy: &mut LayoutStorage,
    nodes: &mut HashMap<ElementId, NodeId>,
    in_scrollbox: bool,
    parent_axis: Axis,
    parent_is_grid: bool,
    measure_text: &mut impl FnMut(TextLayoutRequest<'_>) -> TextLayoutMetrics,
) -> Result<NodeId, LayoutError> {
    let is_scrollbox = matches!(element.kind, crate::ElementKind::ScrollBox(_));
    let child_nodes = element
        .children
        .iter()
        .map(|child| {
            build_taffy_tree(
                child,
                taffy,
                nodes,
                is_scrollbox,
                element.style.axis,
                element.style.grid_columns.is_some(),
                measure_text,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let style = node_style(element, in_scrollbox, parent_axis, parent_is_grid)?;
    let node = if child_nodes.is_empty() {
        if let crate::ElementKind::Text(text) = &element.kind {
            taffy.new_leaf_with_context(
                style,
                TextMeasure {
                    element: element.id,
                    intrinsic: measure_text(TextLayoutRequest {
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
                },
            )
        } else {
            taffy.new_leaf(style)
        }
    } else {
        taffy.new_with_children(style, &child_nodes)
    }
    .map_err(|error| LayoutError::LayoutEngine(error.to_string()))?;
    nodes.insert(element.id, node);
    Ok(node)
}

fn collect_layout(
    element: &Element,
    taffy: &LayoutStorage,
    taffy_nodes: &HashMap<ElementId, NodeId>,
    parent_origin: Point,
    inherited_clip: Rect,
    inherited_flow: FlowDirection,
    output: &mut Vec<LayoutNode>,
) -> Result<(), LayoutError> {
    let node_id = *taffy_nodes
        .get(&element.id)
        .ok_or(LayoutError::MissingLayout(element.id))?;
    let layout = taffy
        .layout(node_id)
        .map_err(|error| LayoutError::LayoutEngine(error.to_string()))?;
    let origin = Point::new(
        parent_origin.x + layout.location.x,
        parent_origin.y + layout.location.y,
    );
    let rect = Rect::new(origin.x, origin.y, layout.size.width, layout.size.height);
    if !rect.is_valid() || !inherited_clip.is_valid() {
        return Err(LayoutError::InvalidGeometry(element.id));
    }
    let flow_direction = element.style.flow_direction.unwrap_or(inherited_flow);
    // ScrollBox 内容包围盒使用未平移的自然坐标；Inherit 后代的可见溢出沿同一路径
    // 传播，Clip／嵌套 ScrollBox／NoClip 浮层保留各自边界（ADR-0409）。
    let scroll_content = match element.kind {
        crate::ElementKind::ScrollBox(_) => scroll_content_size(element, taffy, taffy_nodes),
        _ => None,
    };
    let scroll_viewport = matches!(element.kind, crate::ElementKind::ScrollBox(_)).then(|| {
        let width = layout.scrollbar_size.width.min(rect.width);
        let height = layout.scrollbar_size.height.min(rect.height);
        Rect::new(
            rect.x
                + if flow_direction == FlowDirection::RightToLeft {
                    width
                } else {
                    0.0
                },
            rect.y,
            (rect.width - width).max(0.0),
            (rect.height - height).max(0.0),
        )
    });
    if scroll_content.is_some_and(|(width, height)| !Size::new(width, height).is_valid())
        || scroll_viewport.is_some_and(|rect| !rect.is_valid())
    {
        return Err(LayoutError::InvalidGeometry(element.id));
    }
    output.push(LayoutNode {
        id: element.id,
        flow_direction,
        rect,
        inherited_clip,
        scroll_content,
        scroll_viewport,
    });

    let child_clip = if element.style.clip == crate::ClipMode::NoClip {
        // NoClip 浮层：不裁剪自身、也不受父级裁剪（悬停菜单等可超出容器）。
        rect
    } else if matches!(element.kind, crate::ElementKind::ScrollBox(_))
        || element.style.clip == crate::ClipMode::Clip
    {
        inherited_clip
            .intersection(scroll_viewport.unwrap_or(rect))
            .unwrap_or_else(|| Rect::new(rect.x, rect.y, 0.0, 0.0))
    } else {
        inherited_clip
    };
    // ScrollBox 内容整体平移：内容上移 offset（露出 offset 之下的部分）。
    // paint 与 hit test 直接使用偏移后的 rect，无需感知滚动。
    let content_origin = match element.kind {
        crate::ElementKind::ScrollBox(offset) => Point::new(
            origin.x - flow_direction.horizontal(offset.x),
            origin.y - offset.y,
        ),
        _ => origin,
    };
    for child in &element.children {
        let mut child_origin = content_origin;
        if flow_direction == FlowDirection::RightToLeft
            && child.style.position != PositionMode::Absolute
        {
            let child_node = taffy_nodes
                .get(&child.id)
                .ok_or(LayoutError::MissingLayout(child.id))?;
            let child_layout = taffy
                .layout(*child_node)
                .map_err(|error| LayoutError::LayoutEngine(error.to_string()))?;
            let relative = child
                .style
                .left
                .or_else(|| child.style.right.map(|right| -right))
                .unwrap_or(0.0);
            child_origin.x += rect.width - element.style.padding.right.max(0.0)
                + element.style.padding.left.max(0.0)
                - 2.0 * child_layout.location.x
                - child_layout.size.width
                + child.style.margin.left
                - child.style.margin.right
                + 2.0 * relative;
        }
        collect_layout(
            child,
            taffy,
            taffy_nodes,
            child_origin,
            child_clip,
            flow_direction,
            output,
        )?;
    }
    Ok(())
}

/// 文本固有尺寸（CJK 全角、其余按 0.62 倍字宽估算）。供宿主按内容动态
/// 计算布局宽度（如 Tab 菜单列的"最宽项"自适应宽度）。
pub fn text_intrinsic_size(text: &str, font_size: f32, font_role: FontRole) -> Size {
    let line_height = (font_size * 1.28).max(1.0);
    let width = text
        .lines()
        .map(|line| {
            line.chars()
                .map(|character| {
                    let factor = match font_role {
                        FontRole::Ui if (character as u32) > 0x2E80 => 1.0,
                        FontRole::Ui => 0.62,
                        // A monospace cell is intentionally independent of the glyph.
                        FontRole::Mono
                        | FontRole::MonoBold
                        | FontRole::MonoItalic
                        | FontRole::MonoBoldItalic => 0.62,
                        FontRole::MonoCjk => 1.0,
                        FontRole::Icon => 1.0,
                    };
                    factor * font_size
                })
                .sum::<f32>()
        })
        .fold(0.0_f32, f32::max);
    let height = text.lines().count().max(1) as f32 * line_height;
    Size::new(width, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ElementKey, Style};

    #[test]
    fn grid_tracks_and_spans_use_the_same_layout_tree_as_flex() {
        cumulative_rounding_tests::qualify();
        let child = |id, key, column, row| {
            Element::container(ElementId::new(id), ElementKey::new(key))
                .styled(Style::default().grid_cell(column, row))
        };
        let root = Element::container(ElementId::new(1), ElementKey::new("grid"))
            .styled(
                Style::default()
                    .size(Dimension::Px(200.0), Dimension::Px(80.0))
                    .grid(
                        vec![GridTrack::Px(50.0), GridTrack::Fr(1.0), GridTrack::Px(30.0)],
                        vec![GridTrack::Px(20.0), GridTrack::Px(20.0)],
                    )
                    .gap(5.0),
            )
            .child(child(
                2,
                "first",
                GridCell::new(Some(1), 1),
                GridCell::new(Some(1), 1),
            ))
            .child(child(
                3,
                "middle",
                GridCell::new(Some(2), 1),
                GridCell::new(Some(1), 1),
            ))
            .child(child(
                4,
                "last",
                GridCell::new(Some(3), 1),
                GridCell::new(Some(1), 1),
            ))
            .child(child(
                5,
                "spanning",
                GridCell::new(Some(1), 2),
                GridCell::new(Some(2), 1),
            ));
        let layout = LayoutEngine
            .compute(&root, Viewport::new(200.0, 80.0, 1.0))
            .expect("grid layout");
        assert_eq!(
            layout.node(ElementId::new(2)).expect("first cell").rect,
            Rect::new(0.0, 0.0, 50.0, 20.0)
        );
        assert_eq!(
            layout.node(ElementId::new(3)).expect("middle cell").rect,
            Rect::new(55.0, 0.0, 110.0, 20.0)
        );
        assert_eq!(
            layout.node(ElementId::new(4)).expect("last cell").rect,
            Rect::new(170.0, 0.0, 30.0, 20.0)
        );
        assert_eq!(
            layout.node(ElementId::new(5)).expect("spanning cell").rect,
            Rect::new(0.0, 25.0, 165.0, 20.0)
        );
    }

    #[test]
    fn scrollbox_below_a_flex_middle_layer_stays_compressed() {
        // Spreadsheet 场景：面板根（flex 1）位于 pane 内，ScrollBox 在根内部。
        // 中间层（flex 1 Column）与 ScrollBox 都应被压缩，内容溢出由滚动呈现。
        let root = Element::container(ElementId::new(1), ElementKey::new("root"))
            .styled(
                Style::default()
                    .size(Dimension::Px(100.0), Dimension::Px(80.0))
                    .axis(Axis::Column),
            )
            .child(
                Element::container(ElementId::new(2), ElementKey::new("panel"))
                    .styled(Style::default().flex(1.0, 1.0).axis(Axis::Column))
                    .child(
                        Element::scroll_box(
                            ElementId::new(3),
                            ElementKey::new("sb"),
                            Point::new(0.0, 0.0),
                        )
                        .styled(Style::default().flex(1.0, 1.0).axis(Axis::Column))
                        .child(
                            Element::container(ElementId::new(4), ElementKey::new("c")).styled(
                                Style::default().size(Dimension::Px(100.0), Dimension::Px(200.0)),
                            ),
                        ),
                    ),
            );
        let layout = LayoutEngine
            .compute(&root, Viewport::new(100.0, 100.0, 1.0))
            .expect("valid layout");
        let panel = layout.node(ElementId::new(2)).expect("panel");
        assert!(
            panel.rect.height < 200.0,
            "面板根不应被内容撑开: {}",
            panel.rect.height
        );
        let scrollbox = layout.node(ElementId::new(3)).expect("scrollbox");
        let (_, content_h) = scrollbox.scroll_content.expect("scroll content");
        assert!(
            content_h > scrollbox.rect.height,
            "ScrollBox 应溢出: content={content_h} viewport={}",
            scrollbox.rect.height
        );
    }

    #[test]
    fn fixed_width_scrollbox_in_a_row_keeps_its_interactive_viewport() {
        let root = Element::container(ElementId::new(1), ElementKey::new("root"))
            .styled(
                Style::default()
                    .size(Dimension::Px(300.0), Dimension::Px(100.0))
                    .axis(Axis::Row),
            )
            .child(
                Element::scroll_box(
                    ElementId::new(2),
                    ElementKey::new("scroll"),
                    Point::default(),
                )
                .styled(Style::default().size(Dimension::Px(220.0), Dimension::Px(70.0)))
                .child(
                    Element::container(ElementId::new(3), ElementKey::new("content"))
                        .styled(Style::default().size(Dimension::Px(220.0), Dimension::Px(190.0))),
                ),
            );
        let layout = LayoutEngine
            .compute(&root, Viewport::new(300.0, 100.0, 1.0))
            .expect("fixed scroll layout");
        let scroll = layout.node(ElementId::new(2)).expect("scroll node");
        assert_eq!(scroll.rect.width, 220.0);
        assert_eq!(scroll.rect.height, 70.0);
        assert!(
            scroll
                .scroll_content
                .is_some_and(|content| content.1 > 70.0)
        );
    }

    #[test]
    fn rejects_duplicate_stable_identity() {
        let duplicate_id = ElementId::new(2);
        let root = Element::container(ElementId::new(1), ElementKey::new("root"))
            .child(Element::container(duplicate_id, ElementKey::new("first")))
            .child(Element::container(duplicate_id, ElementKey::new("second")));

        assert!(matches!(
            LayoutEngine.compute(&root, Viewport::new(100.0, 100.0, 1.0)),
            Err(LayoutError::DuplicateElementId(id)) if id == duplicate_id
        ));
    }

    #[test]
    fn child_slots_and_wrapped_lines_keep_hit_geometry_and_identity_on_reflow() {
        use crate::HitTestTree;
        let child = |id, height| {
            Element::container(ElementId::new(id), ElementKey::new(format!("child.{id}")))
                .styled(Style::default().size(Dimension::Px(70.0), Dimension::Px(height)))
        };
        let mut root = Element::container(ElementId::new(1), ElementKey::new("wrap"))
            .styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Px(100.0))
                    .axis(Axis::Row)
                    .flex_wrap(FlexWrap::Wrap)
                    .align_content(Align::Start)
                    .align(Align::Start)
                    .gap(6.0),
            )
            .child(child(2, 30.0))
            .child(child(3, 10.0))
            .child(child(4, 20.0));
        root.children[1].style = root.children[1]
            .style
            .clone()
            .align_self(Align::End)
            .margin(Insets {
                left: 4.0,
                right: 3.0,
                bottom: 2.0,
                ..Default::default()
            });
        let wide = LayoutEngine
            .compute(&root, Viewport::new(260.0, 120.0, 1.0))
            .expect("wide");
        let first = wide.node(ElementId::new(2)).expect("first").rect;
        let second = wide.node(ElementId::new(3)).expect("second").rect;
        let third = wide.node(ElementId::new(4)).expect("third").rect;
        assert!(
            second.x >= first.right() + 10.0,
            "child margin remains outside its box"
        );
        assert!(
            (second.bottom() + 2.0 - first.bottom()).abs() < 0.01,
            "only this child aligns to the line end"
        );
        assert_eq!(third.y, first.y);
        let narrow = LayoutEngine
            .compute(&root, Viewport::new(150.0, 120.0, 1.5))
            .expect("narrow");
        let moved = narrow
            .node(ElementId::new(4))
            .expect("same stable third")
            .rect;
        assert!(moved.y >= first.bottom() + 6.0, "the row starts a new line");
        let hits = HitTestTree::build(&root, &narrow);
        assert_eq!(
            hits.hit(Point::new(moved.x + 2.0, moved.y + 2.0)),
            Some(ElementId::new(4))
        );
        assert_eq!(
            wide.nodes().iter().map(|n| n.id).collect::<Vec<_>>(),
            narrow.nodes().iter().map(|n| n.id).collect::<Vec<_>>()
        );
        root.style.flex_wrap = FlexWrap::WrapReverse;
        let reverse = LayoutEngine
            .compute(&root, Viewport::new(150.0, 120.0, 1.0))
            .expect("reverse");
        assert!(
            reverse.node(ElementId::new(4)).expect("last").rect.y
                < reverse.node(ElementId::new(2)).expect("first").rect.y
        );
        root.style.flex_wrap = FlexWrap::NoWrap;
        root.children[1].style.margin.left = -4.0;
        let unwrapped = LayoutEngine
            .compute(&root, Viewport::new(260.0, 120.0, 1.0))
            .expect("negative margin");
        let first = unwrapped.node(ElementId::new(2)).expect("first").rect;
        let second = unwrapped.node(ElementId::new(3)).expect("second").rect;
        assert!((second.x - first.right() - 2.0).abs() < 0.01);
    }
}
