use crate::{
    Color, Element, ElementId, ElementKind, FlowDirection, FontRole, LayoutTree, Point, Rect, Size,
    TextHAlign, TextVAlign,
    layout::text_intrinsic_size,
    style::{ScrollbarStyle, ScrollbarVisibility},
};

mod focus;

/// A backend-neutral, ordered drawing instruction.
#[derive(Clone, Debug, PartialEq)]
pub enum PaintCommand {
    FillQuad {
        element: ElementId,
        points: [Point; 4],
        color: Color,
    },
    PushClip {
        rect: Rect,
        /// true = 浮层（NoClip）：不与其父级裁剪求交，用于悬停菜单等可超出容器。
        no_clip: bool,
    },
    PopClip,
    FillRoundedRect {
        element: ElementId,
        rect: Rect,
        radius: f32,
        /// 绘制旋转（弧度，绕 rect 中心）。
        rotation: f32,
        color: Color,
    },
    StrokeRoundedRect {
        element: ElementId,
        rect: Rect,
        radius: f32,
        width: f32,
        /// 绘制旋转（弧度，绕 rect 中心）。
        rotation: f32,
        color: Color,
    },
    /// Window-coordinate connected bevel stroke; the Host still owns the source payload.
    FillPolyline {
        element: ElementId,
        points: Vec<Point>,
        width: f32,
        color: Color,
    },
    Text {
        element: ElementId,
        origin: Point,
        bounds: Rect,
        /// Effective ancestor clip at the point this text is painted.
        clip: Rect,
        text: String,
        font_size: f32,
        font_role: FontRole,
        color: Color,
        h_align: TextHAlign,
        v_align: TextVAlign,
        wrap: bool,
    },
    Image {
        element: ElementId,
        bounds: Rect,
        clip: Rect,
        source: String,
    },
    FittedImage {
        element: ElementId,
        bounds: Rect,
        clip: Rect,
        source: String,
        presentation: crate::ImagePresentation,
    },
    TextInput {
        element: ElementId,
        bounds: Rect,
        clip: Rect,
        snapshot: crate::TextInputSnapshot,
        focused: bool,
        color: Color,
    },
}

impl PaintCommand {
    #[must_use]
    pub const fn element_id(&self) -> Option<ElementId> {
        match self {
            Self::FillRoundedRect { element, .. }
            | Self::StrokeRoundedRect { element, .. }
            | Self::FillPolyline { element, .. }
            | Self::FillQuad { element, .. }
            | Self::Text { element, .. }
            | Self::Image { element, .. }
            | Self::FittedImage { element, .. }
            | Self::TextInput { element, .. } => Some(*element),
            Self::PushClip { .. } | Self::PopClip => None,
        }
    }
}

/// Immutable paint output for one element-tree revision.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaintList {
    commands: Vec<PaintCommand>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PaintInteraction {
    pub pointer: Option<Point>,
    pub active_scrollbar: Option<ElementId>,
}

impl PaintList {
    #[must_use]
    pub fn build(root: &Element, layout: &LayoutTree) -> Self {
        Self::build_from_elements_with_focus(
            &[root],
            layout,
            layout.viewport().logical_rect(),
            None,
            PaintInteraction::default(),
        )
    }

    /// Paint a keyboard focus ring at its element's own paint/clip position.
    /// The host supplies a semantic theme color; the UI core does not own a
    /// fixed palette or a window's focus state.
    #[must_use]
    pub fn build_with_focus(
        root: &Element,
        layout: &LayoutTree,
        focused: Option<ElementId>,
        color: Color,
    ) -> Self {
        Self::build_from_elements_with_focus(
            &[root],
            layout,
            layout.viewport().logical_rect(),
            focused.map(|id| (id, color)),
            PaintInteraction::default(),
        )
    }

    /// Paint hover-triggered scrollbars from the current pointer position and
    /// keep the captured scrollbar visible while its thumb is being dragged.
    #[must_use]
    pub fn build_with_focus_and_interaction(
        root: &Element,
        layout: &LayoutTree,
        focused: Option<ElementId>,
        color: Color,
        interaction: PaintInteraction,
    ) -> Self {
        Self::build_from_elements_with_focus(
            &[root],
            layout,
            layout.viewport().logical_rect(),
            focused.map(|id| (id, color)),
            interaction,
        )
    }

    /// Paint a tree while omitting complete overlay subtrees that another
    /// camera will draw later. Layout and hit testing still use the full tree.
    #[must_use]
    pub fn build_excluding(root: &Element, layout: &LayoutTree, excluded: &[ElementId]) -> Self {
        let _trace = crate::trace::span("gpui.paint", "paint.build_excluding");
        let mut commands = vec![PaintCommand::PushClip {
            rect: layout.viewport().logical_rect(),
            no_clip: false,
        }];
        append_element_filtered(
            root,
            layout,
            &mut commands,
            excluded,
            None,
            PaintInteraction::default(),
        );
        commands.push(PaintCommand::PopClip);
        Self { commands }
    }

    /// Paint a subset of an already-laid-out tree, clipped to `clip`.
    /// Used to re-draw overlays after a later 3D camera has covered the base UI pass.
    #[must_use]
    pub fn build_from_elements(elements: &[&Element], layout: &LayoutTree, clip: Rect) -> Self {
        Self::build_from_elements_with_focus(
            elements,
            layout,
            clip,
            None,
            PaintInteraction::default(),
        )
    }

    fn build_from_elements_with_focus(
        elements: &[&Element],
        layout: &LayoutTree,
        clip: Rect,
        focus: Option<(ElementId, Color)>,
        interaction: PaintInteraction,
    ) -> Self {
        let _trace = crate::trace::span("gpui.paint", "paint.build");
        let mut commands = Vec::new();
        commands.push(PaintCommand::PushClip {
            rect: clip,
            no_clip: false,
        });
        for element in elements {
            append_element_filtered(element, layout, &mut commands, &[], focus, interaction);
        }
        commands.push(PaintCommand::PopClip);
        Self { commands }
    }

    #[must_use]
    pub fn commands(&self) -> &[PaintCommand] {
        &self.commands
    }
}

fn append_element_filtered(
    element: &Element,
    layout: &LayoutTree,
    commands: &mut Vec<PaintCommand>,
    excluded: &[ElementId],
    focus: Option<(ElementId, Color)>,
    interaction: PaintInteraction,
) {
    if excluded.contains(&element.id) {
        return;
    }
    let Some(node) = layout.node(element.id) else {
        return;
    };
    let paint_clip = if element.style.clip == crate::ClipMode::NoClip {
        node.rect
            .intersection(layout.viewport().logical_rect())
            .unwrap_or_default()
    } else if matches!(element.kind, ElementKind::ScrollBox(_))
        || element.style.clip == crate::ClipMode::Clip
    {
        node.inherited_clip
            .intersection(node.rect)
            .unwrap_or_default()
    } else {
        node.inherited_clip
    };
    let visual = &element.style.visual;
    let rotation = element.style.rotation.unwrap_or(0.0);
    // 先压裁剪再画自身背景/边框：NoClip 浮层（子菜单等）超出父容器时，
    // 改以浮层自身矩形作为边界，避免被父级裁空。
    if element.style.clip == crate::ClipMode::NoClip {
        // NoClip 浮层：压入脱离父级的边界（renderer 不与其父级求交）。
        commands.push(PaintCommand::PushClip {
            rect: node.rect,
            no_clip: true,
        });
    } else if matches!(element.kind, ElementKind::ScrollBox(_))
        || element.style.clip == crate::ClipMode::Clip
    {
        commands.push(PaintCommand::PushClip {
            rect: node.rect,
            no_clip: false,
        });
    }
    if let Some(points) = &element.style.filled_quad {
        commands.push(PaintCommand::FillQuad {
            element: element.id,
            points: points.map(|p| Point::new(p.x + node.rect.x, p.y + node.rect.y)),
            color: visual
                .background
                .unwrap_or(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        });
    } else if let Some(polyline) = &element.style.polyline {
        let width = polyline.width;
        let points = polyline
            .points
            .iter()
            .map(|point| Point::new(point.x + node.rect.x, point.y + node.rect.y))
            .collect::<Vec<_>>();
        commands.push(PaintCommand::FillPolyline {
            element: element.id,
            points,
            width,
            color: visual
                .background
                .unwrap_or(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        });
    } else if let Some(color) = visual.background {
        commands.push(PaintCommand::FillRoundedRect {
            element: element.id,
            rect: node.rect,
            radius: visual.corner_radius,
            rotation,
            color,
        });
    }
    if let (Some(color), width) = (visual.border_color, visual.border_width)
        && width > 0.0
    {
        commands.push(PaintCommand::StrokeRoundedRect {
            element: element.id,
            rect: node.rect,
            radius: visual.corner_radius,
            width,
            rotation,
            color,
        });
    }
    if let ElementKind::Text(text) = &element.kind {
        let bounds = text_paint_bounds(node.rect, text, visual.font_size, visual.font_role);
        commands.push(PaintCommand::Text {
            element: element.id,
            origin: Point::new(node.rect.x, node.rect.y),
            bounds,
            clip: paint_clip,
            text: text.clone(),
            font_size: visual.font_size,
            font_role: visual.font_role,
            color: visual.text_color,
            h_align: match (node.flow_direction, element.style.text_h_align) {
                (FlowDirection::RightToLeft, TextHAlign::Start) => TextHAlign::End,
                (FlowDirection::RightToLeft, TextHAlign::End) => TextHAlign::Start,
                (_, align) => align,
            },
            v_align: element.style.text_v_align,
            wrap: element.style.text_wrap,
        });
    }
    if let ElementKind::Image(source) = &element.kind {
        commands.push(PaintCommand::Image {
            element: element.id,
            bounds: node.rect,
            clip: paint_clip,
            source: source.clone(),
        });
    }
    if let ElementKind::FittedImage {
        source,
        presentation,
    } = &element.kind
    {
        commands.push(PaintCommand::FittedImage {
            element: element.id,
            bounds: node.rect,
            clip: paint_clip,
            source: source.clone(),
            presentation: *presentation,
        });
    }
    if let ElementKind::TextInput { snapshot, focused } = &element.kind {
        commands.push(PaintCommand::TextInput {
            element: element.id,
            bounds: node.rect,
            clip: paint_clip,
            snapshot: snapshot.clone(),
            focused: *focused,
            color: visual.text_color,
        });
    }

    let reserved_scroll =
        matches!(element.kind, ElementKind::ScrollBox(_)) && node.scroll_viewport() != node.rect;
    if reserved_scroll {
        commands.push(PaintCommand::PushClip {
            rect: node.scroll_viewport(),
            no_clip: false,
        });
    }
    for child in element.children_in_paint_order() {
        append_element_filtered(child, layout, commands, excluded, focus, interaction);
    }
    if reserved_scroll {
        commands.push(PaintCommand::PopClip);
    }
    focus::append_indicator(element, node.rect, focus, commands);
    if element.style.clip == crate::ClipMode::NoClip
        || matches!(element.kind, ElementKind::ScrollBox(_))
        || element.style.clip == crate::ClipMode::Clip
    {
        commands.push(PaintCommand::PopClip);
    }
    // Scrollbars use the same negotiated viewport as child clipping and input.
    if let ElementKind::ScrollBox(offset) = &element.kind
        && node.scroll_content.is_some()
        && (element.style.scrollbar.visibility == ScrollbarVisibility::Always
            || interaction.active_scrollbar == Some(element.id)
            || interaction
                .pointer
                .is_some_and(|pointer| node.rect.contains(pointer)))
    {
        append_scrollbars(
            node,
            *offset,
            element.style.scrollbar,
            interaction,
            commands,
        );
    }
}

/// 滚动条几何常量：细条、贴边；颜色和显示策略由 Style 提供。
pub const SCROLLBAR_SIZE: f32 = 7.0;
pub const SCROLLBAR_INSET: f32 = 1.5;
pub const SCROLLBAR_GUTTER: f32 = SCROLLBAR_SIZE + SCROLLBAR_INSET * 2.0;

pub(crate) fn scrollbar_geometry_for_node(
    node: &crate::LayoutNode,
    offset: Point,
) -> (Option<Rect>, Option<Rect>, Option<Rect>, Option<Rect>) {
    let (w, h) = node.scroll_content.unwrap_or_default();
    scrollbar_geometry_in_bounds(
        node.rect,
        node.scroll_viewport(),
        Size::new(w, h),
        offset,
        node.flow_direction,
    )
}

/// 一个 ScrollBox 的滚动条几何（与绘制共用同一公式，供输入层命中/拖拽）。
/// 返回 `(垂直 track, 垂直 thumb, 水平 track, 水平 thumb)`；对应轴无溢出时为 None。
#[must_use]
pub fn scrollbar_geometry(
    viewport: Rect,
    content: Size,
    offset: Point,
) -> (Option<Rect>, Option<Rect>, Option<Rect>, Option<Rect>) {
    scrollbar_geometry_in_direction(viewport, content, offset, FlowDirection::LeftToRight)
}

/// RTL offsets start at the right edge; input and paint use this same geometry.
#[must_use]
pub fn scrollbar_geometry_in_direction(
    viewport: Rect,
    content: Size,
    offset: Point,
    flow: FlowDirection,
) -> (Option<Rect>, Option<Rect>, Option<Rect>, Option<Rect>) {
    scrollbar_geometry_in_bounds(viewport, viewport, content, offset, flow)
}

fn scrollbar_geometry_in_bounds(
    bounds: Rect,
    viewport: Rect,
    content: Size,
    mut offset: Point,
    flow: FlowDirection,
) -> (Option<Rect>, Option<Rect>, Option<Rect>, Option<Rect>) {
    if flow == FlowDirection::RightToLeft {
        offset.x = (content.width - viewport.width).max(0.0) - offset.x;
    }
    let vertical_track = (content.height > viewport.height && viewport.height > 0.0)
        .then(|| {
            Rect::new(
                match flow {
                    FlowDirection::LeftToRight => bounds.right() - SCROLLBAR_SIZE - SCROLLBAR_INSET,
                    FlowDirection::RightToLeft => bounds.x + SCROLLBAR_INSET,
                },
                viewport.y + SCROLLBAR_INSET,
                SCROLLBAR_SIZE,
                (viewport.height - SCROLLBAR_INSET * 2.0).max(0.0),
            )
        })
        .and_then(|track| track.intersection(bounds))
        .filter(|track| !track.is_empty());
    let vertical = if let Some(track) = vertical_track {
        let thumb_height = (track.height * viewport.height / content.height)
            .max(SCROLLBAR_SIZE * 2.0)
            .min(track.height.max(0.0));
        let max_scroll = (content.height - viewport.height).max(0.0);
        let thumb_y = if max_scroll > 0.0 {
            (offset.y.clamp(0.0, max_scroll) / max_scroll) * (track.height - thumb_height)
        } else {
            0.0
        };
        (
            Some(track),
            Some(Rect::new(
                track.x,
                track.y + thumb_y,
                track.width,
                thumb_height,
            )),
        )
    } else {
        (None, None)
    };
    let horizontal_track = (content.width > viewport.width && viewport.width > 0.0)
        .then(|| {
            Rect::new(
                viewport.x + SCROLLBAR_INSET,
                bounds.bottom() - SCROLLBAR_SIZE - SCROLLBAR_INSET,
                (viewport.width - SCROLLBAR_INSET * 2.0).max(0.0),
                SCROLLBAR_SIZE,
            )
        })
        .and_then(|track| track.intersection(bounds))
        .filter(|track| !track.is_empty());
    let horizontal = if let Some(track) = horizontal_track {
        let thumb_width = (track.width * viewport.width / content.width)
            .max(SCROLLBAR_SIZE * 2.0)
            .min(track.width.max(0.0));
        let max_scroll = (content.width - viewport.width).max(0.0);
        let thumb_x = if max_scroll > 0.0 {
            (offset.x.clamp(0.0, max_scroll) / max_scroll) * (track.width - thumb_width)
        } else {
            0.0
        };
        (
            Some(track),
            Some(Rect::new(
                track.x + thumb_x,
                track.y,
                thumb_width,
                track.height,
            )),
        )
    } else {
        (None, None)
    };
    (vertical.0, vertical.1, horizontal.0, horizontal.1)
}

fn append_scrollbars(
    node: &crate::LayoutNode,
    offset: Point,
    style: ScrollbarStyle,
    interaction: PaintInteraction,
    commands: &mut Vec<PaintCommand>,
) {
    let element = node.id;
    let (vertical_track, vertical_thumb, horizontal_track, horizontal_thumb) =
        node.scrollbar_geometry(offset);
    let track_radius = SCROLLBAR_SIZE * 0.5;
    if let Some(track) = vertical_track {
        commands.push(PaintCommand::FillRoundedRect {
            element,
            rect: track,
            radius: track_radius,
            rotation: 0.0,
            color: style.track,
        });
    }
    if let Some(thumb) = vertical_thumb {
        commands.push(PaintCommand::FillRoundedRect {
            element,
            rect: thumb,
            radius: track_radius,
            rotation: 0.0,
            color: if interaction.active_scrollbar != Some(element)
                && interaction
                    .pointer
                    .is_some_and(|pointer| thumb.contains(pointer))
            {
                style.thumb_hovered
            } else {
                style.thumb
            },
        });
    }
    if let Some(track) = horizontal_track {
        commands.push(PaintCommand::FillRoundedRect {
            element,
            rect: track,
            radius: track_radius,
            rotation: 0.0,
            color: style.track,
        });
    }
    if let Some(thumb) = horizontal_thumb {
        commands.push(PaintCommand::FillRoundedRect {
            element,
            rect: thumb,
            radius: track_radius,
            rotation: 0.0,
            color: if interaction.active_scrollbar != Some(element)
                && interaction
                    .pointer
                    .is_some_and(|pointer| thumb.contains(pointer))
            {
                style.thumb_hovered
            } else {
                style.thumb
            },
        });
    }
}

fn text_paint_bounds(layout_rect: Rect, text: &str, font_size: f32, font_role: FontRole) -> Rect {
    let intrinsic = text_intrinsic_size(text, font_size, font_role);
    Rect::new(
        layout_rect.x,
        layout_rect.y,
        if layout_rect.width > 0.0 {
            layout_rect.width
        } else {
            intrinsic.width
        },
        if layout_rect.height > 0.0 {
            layout_rect.height
        } else {
            intrinsic.height
        },
    )
}

/// A hit-test item derived from the same element and clip order as painting.
#[derive(Clone, Copy, Debug, PartialEq)]
struct HitTestEntry {
    element: ElementId,
    visible_rect: Rect,
    layout_rect: Rect,
    shape: crate::HitShape,
}

/// Immutable front-to-back hit-test index.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HitTestTree {
    entries: Vec<HitTestEntry>,
}

impl HitTestTree {
    #[must_use]
    pub fn build(root: &Element, layout: &LayoutTree) -> Self {
        let _trace = crate::trace::span("gpui.input", "hit_test.build");
        let mut entries = Vec::new();
        append_hit_entries(root, layout, layout.viewport().logical_rect(), &mut entries);
        Self { entries }
    }

    #[must_use]
    pub fn hit(&self, point: Point) -> Option<ElementId> {
        self.entries
            .iter()
            .rev()
            .find(|entry| {
                if !entry.visible_rect.contains(point) {
                    return false;
                }
                match entry.shape {
                    crate::HitShape::Rectangle => true,
                    crate::HitShape::Ellipse => {
                        let rect = entry.layout_rect;
                        let rx = f64::from(rect.width) * 0.5;
                        let ry = f64::from(rect.height) * 0.5;
                        if rx <= 0.0 || ry <= 0.0 {
                            return false;
                        }
                        let x = (f64::from(point.x) - f64::from(rect.x) - rx) / rx;
                        let y = (f64::from(point.y) - f64::from(rect.y) - ry) / ry;
                        x * x + y * y <= 1.0
                    }
                }
            })
            .map(|entry| entry.element)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn append_hit_entries(
    element: &Element,
    layout: &LayoutTree,
    inherited_clip: Rect,
    entries: &mut Vec<HitTestEntry>,
) {
    let Some(node) = layout.node(element.id) else {
        return;
    };
    let own_clip = if element.style.clip == crate::ClipMode::NoClip {
        // The renderer resets the clip to this overlay's rect, ignoring its
        // ancestors. It still clips the overlay's own descendants to that rect.
        node.rect
            .intersection(layout.viewport().logical_rect())
            .unwrap_or_default()
    } else {
        inherited_clip
    };
    if (element.style.pointer_events || element.style.action_boundary)
        && let Some(visible_rect) = node.rect.intersection(own_clip)
    {
        entries.push(HitTestEntry {
            element: element.id,
            visible_rect,
            layout_rect: node.rect,
            shape: element.style.hit_shape,
        });
    }
    let child_clip = if element.style.clip == crate::ClipMode::NoClip {
        Some(own_clip)
    } else if matches!(element.kind, ElementKind::ScrollBox(_))
        || element.style.clip == crate::ClipMode::Clip
    {
        node.scroll_viewport().intersection(inherited_clip)
    } else {
        Some(inherited_clip)
    };
    // A nested NoClip descendant can reset an empty ancestor clip, so visit
    // the whole subtree even when the current clip has no visible area.
    let child_clip = child_clip.unwrap_or_default();
    for child in element.children_in_paint_order() {
        append_hit_entries(child, layout, child_clip, entries);
    }
}

#[cfg(test)]
mod tests {
    use crate::{Axis, Dimension, ElementKey, LayoutEngine, SemanticRole, Style, Viewport};

    use super::*;

    #[test]
    fn elliptical_pointer_surface_keeps_original_center_when_clipped() {
        let root = Element::container(ElementId::new(1), ElementKey::new("clip"))
            .styled(
                Style::default()
                    .size(Dimension::Px(50.0), Dimension::Px(100.0))
                    .clipped(),
            )
            .child(
                Element::container(ElementId::new(2), ElementKey::new("ellipse")).styled(
                    Style::default()
                        .absolute(-50.0, 0.0)
                        .size(Dimension::Px(100.0), Dimension::Px(100.0))
                        .hit_shape(crate::HitShape::Ellipse),
                ),
            );
        let layout = LayoutEngine
            .compute(&root, Viewport::new(100.0, 100.0, 1.25))
            .expect("layout");
        let hits = HitTestTree::build(&root, &layout);
        assert_eq!(hits.hit(Point::new(5.0, 10.0)), Some(ElementId::new(2)));
        assert_eq!(hits.hit(Point::new(45.0, 5.0)), Some(ElementId::new(1)));
        assert_eq!(hits.hit(Point::new(55.0, 50.0)), None);
    }

    fn overlapping_tree() -> Element {
        Element::container(ElementId::new(1), ElementKey::new("root"))
            .styled(
                Style::default()
                    .size(Dimension::Px(100.0), Dimension::Px(100.0))
                    .clipped(),
            )
            .child(
                Element::container(ElementId::new(2), ElementKey::new("child")).styled(
                    Style::default()
                        .size(Dimension::Px(100.0), Dimension::Px(100.0))
                        .background(Color::rgb8(32, 32, 32)),
                ),
            )
    }

    #[test]
    fn inert_action_boundary_remains_a_hit_target() {
        let tree = Element::container(ElementId::new(1), ElementKey::new("parent"))
            .styled(Style::default().size(Dimension::Px(100.0), Dimension::Px(100.0)))
            .child(
                Element::container(ElementId::new(2), ElementKey::new("disabled")).styled(
                    Style::default()
                        .size(Dimension::Px(50.0), Dimension::Px(50.0))
                        .pointer_events(false)
                        .action_boundary(true),
                ),
            );
        let layout = LayoutEngine
            .compute(&tree, Viewport::new(100.0, 100.0, 1.0))
            .expect("valid layout");
        let hits = HitTestTree::build(&tree, &layout);
        assert_eq!(hits.hit(Point::new(10.0, 10.0)), Some(ElementId::new(2)));
    }

    #[test]
    fn focus_ring_is_painted_only_for_the_declared_tab_stop() {
        let focused = ElementId::new(2);
        let focus_color = Color::rgb8(15, 120, 230);
        let tree = Element::container(ElementId::new(1), ElementKey::new("root"))
            .styled(Style::default().size(Dimension::Px(100.0), Dimension::Px(100.0)))
            .child(
                Element::container(focused, ElementKey::new("button")).styled(
                    Style::default()
                        .size(Dimension::Px(40.0), Dimension::Px(20.0))
                        .focusable(true),
                ),
            );
        let layout = LayoutEngine
            .compute(&tree, Viewport::new(100.0, 100.0, 1.0))
            .expect("valid layout");
        let painted = PaintList::build_with_focus(&tree, &layout, Some(focused), focus_color);
        assert!(painted.commands().iter().any(|command| {
            matches!(
                command,
                PaintCommand::StrokeRoundedRect { element, color, .. }
                    if *element == focused && *color == focus_color
            )
        }));
        let not_focusable =
            PaintList::build_with_focus(&tree, &layout, Some(ElementId::new(1)), focus_color);
        assert!(!not_focusable.commands().iter().any(|command| {
            matches!(
                command,
                PaintCommand::StrokeRoundedRect { color, .. } if *color == focus_color
            )
        }));

        // A field owns one editing border; a read-only text target still needs
        // the generic keyboard indicator because it has no editing caret.
        for read_only in [false, true] {
            let editor = Element::text_input(
                focused,
                ElementKey::new("editor"),
                crate::TextInputState::default().snapshot(),
                true,
            )
            .semantic(crate::Semantic::new(SemanticRole::TextInput, "Text").read_only(read_only))
            .styled(
                Style::default()
                    .size(Dimension::Px(80.0), Dimension::Px(20.0))
                    .border(1.0, Color::rgb8(50, 60, 70))
                    .focus_indicator(if read_only {
                        crate::FocusIndicator::Outline
                    } else {
                        crate::FocusIndicator::Control
                    })
                    .focusable(true),
            );
            let layout = LayoutEngine
                .compute(&editor, Viewport::new(100.0, 100.0, 1.0))
                .expect("editor layout");
            let paint = PaintList::build_with_focus(&editor, &layout, Some(focused), focus_color);
            let outline = paint.commands().iter().any(|command| {
                matches!(command, PaintCommand::StrokeRoundedRect { color, .. } if *color == focus_color)
            });
            assert_eq!(outline, read_only);
            assert!(paint.commands().iter().any(|command| {
                matches!(command, PaintCommand::TextInput { focused: true, .. })
            }));
        }

        let mut own_border = tree.clone();
        own_border.children[0].style.visual.border_width = 1.0;
        own_border.children[0].style.visual.border_color = Some(focus_color);
        // Ordinary borders must not suppress the keyboard indicator just because
        // a custom theme happens to use the same color.
        let ordinary_paint =
            PaintList::build_with_focus(&own_border, &layout, Some(focused), focus_color);
        assert_eq!(
            ordinary_paint
                .commands()
                .iter()
                .filter(|command| {
                    matches!(command, PaintCommand::StrokeRoundedRect { element, color, .. }
                    if *element == focused && *color == focus_color)
                })
                .count(),
            2
        );
        own_border.children[0].style.focus_indicator = crate::FocusIndicator::Control;
        let own_paint =
            PaintList::build_with_focus(&own_border, &layout, Some(focused), focus_color);
        assert_eq!(
            own_paint
                .commands()
                .iter()
                .filter(|command| {
                    matches!(command, PaintCommand::StrokeRoundedRect { element, color, .. }
                if *element == focused && *color == focus_color)
                })
                .count(),
            1
        );

        for (width, height) in [(6.0, 80.0), (80.0, 6.0)] {
            let splitter = Element::container(focused, ElementKey::new("splitter"))
                .styled(
                    Style::default()
                        .size(Dimension::Px(width), Dimension::Px(height))
                        .focusable(true),
                )
                .semantic(crate::Semantic::new(SemanticRole::Splitter, "Resize panes"));
            let layout = LayoutEngine
                .compute(&splitter, Viewport::new(100.0, 100.0, 1.0))
                .expect("splitter layout");
            let painted =
                PaintList::build_with_focus(&splitter, &layout, Some(focused), focus_color);
            assert!(!painted.commands().iter().any(|command| {
                matches!(command, PaintCommand::StrokeRoundedRect { element, .. } if *element == focused)
            }));
            let highlight = painted.commands().iter().find_map(|command| match command {
                PaintCommand::FillRoundedRect {
                    element,
                    rect,
                    color,
                    ..
                } if *element == focused && *color == focus_color => Some(*rect),
                _ => None,
            });
            let highlight = highlight.expect("focused splitter has one line highlight");
            if width < height {
                assert!(highlight.width < width);
                assert_eq!(highlight.height, height);
            } else {
                assert_eq!(highlight.width, width);
                assert!(highlight.height < height);
            }
        }
    }

    #[test]
    fn paint_order_is_stable_for_the_same_state() {
        let root = overlapping_tree();
        let layout = LayoutEngine
            .compute(&root, Viewport::new(100.0, 100.0, 1.0))
            .expect("valid layout");
        assert_eq!(
            PaintList::build(&root, &layout),
            PaintList::build(&root, &layout)
        );
    }

    #[test]
    fn excluded_overlay_subtree_is_reserved_for_a_later_camera() {
        let overlay = Element::container(ElementId::new(3), ElementKey::new("modal"))
            .styled(
                Style::default()
                    .size(Dimension::Px(50.0), Dimension::Px(50.0))
                    .background(Color::rgb8(255, 255, 255)),
            )
            .child(
                Element::container(ElementId::new(4), ElementKey::new("modal-child")).styled(
                    Style::default()
                        .size(Dimension::Px(20.0), Dimension::Px(20.0))
                        .background(Color::rgb8(0, 0, 0)),
                ),
            );
        let root = overlapping_tree().child(overlay);
        let layout = LayoutEngine
            .compute(&root, Viewport::new(100.0, 100.0, 1.0))
            .expect("valid layout");
        let paint = PaintList::build_excluding(&root, &layout, &[ElementId::new(3)]);
        assert!(paint.commands().iter().all(|command| {
            command.element_id() != Some(ElementId::new(3))
                && command.element_id() != Some(ElementId::new(4))
        }));
        assert!(
            paint
                .commands()
                .iter()
                .any(|command| command.element_id() == Some(ElementId::new(2))),
            "non-overlay UI remains in the base pass"
        );
    }

    #[test]
    fn hit_test_uses_reverse_paint_order() {
        let root = overlapping_tree();
        let layout = LayoutEngine
            .compute(&root, Viewport::new(100.0, 100.0, 1.0))
            .expect("valid layout");
        let hits = HitTestTree::build(&root, &layout);
        assert_eq!(hits.hit(Point::new(10.0, 10.0)), Some(ElementId::new(2)));
    }

    #[test]
    fn no_clip_hit_bounds_match_paint_and_nested_overlay_can_escape() {
        let overflow_id = ElementId::new(3);
        let submenu_item_id = ElementId::new(5);
        let root = Element::container(ElementId::new(0), ElementKey::new("root"))
            .styled(
                Style::default()
                    .size(Dimension::Px(100.0), Dimension::Px(100.0))
                    .pointer_events(false),
            )
            .child(
                Element::container(ElementId::new(1), ElementKey::new("overlay"))
                    .styled(
                        Style::default()
                            .absolute(10.0, 10.0)
                            .size(Dimension::Px(40.0), Dimension::Px(40.0))
                            .no_clip(),
                    )
                    .child(
                        Element::container(overflow_id, ElementKey::new("overflow")).styled(
                            Style::default()
                                .absolute(45.0, 0.0)
                                .size(Dimension::Px(10.0), Dimension::Px(10.0))
                                .pointer_events(true),
                        ),
                    )
                    .child(
                        Element::container(ElementId::new(4), ElementKey::new("submenu"))
                            .styled(
                                Style::default()
                                    .absolute(35.0, 15.0)
                                    .size(Dimension::Px(40.0), Dimension::Px(20.0))
                                    .no_clip(),
                            )
                            .child(
                                Element::container(
                                    submenu_item_id,
                                    ElementKey::new("submenu-item"),
                                )
                                .styled(
                                    Style::default()
                                        .size(Dimension::Px(40.0), Dimension::Px(20.0))
                                        .pointer_events(true),
                                ),
                            ),
                    ),
            );
        let layout = LayoutEngine
            .compute(&root, Viewport::new(100.0, 100.0, 1.0))
            .expect("valid layout");
        let hits = HitTestTree::build(&root, &layout);
        assert_eq!(hits.hit(Point::new(60.0, 15.0)), None);
        assert_eq!(hits.hit(Point::new(60.0, 30.0)), Some(submenu_item_id));
    }

    #[test]
    fn default_parent_masks_nested_text_and_pointer_hits() {
        let parent_id = ElementId::new(2);
        let text_id = ElementId::new(3);
        let root = Element::container(ElementId::new(1), ElementKey::new("root"))
            .styled(Style::default().size(Dimension::Px(100.0), Dimension::Px(60.0)))
            .child(
                Element::container(parent_id, ElementKey::new("parent"))
                    .styled(
                        Style::default()
                            .absolute(10.0, 10.0)
                            .size(Dimension::Px(50.0), Dimension::Px(30.0)),
                    )
                    .child(
                        Element::text(text_id, ElementKey::new("text"), "clipped").styled(
                            Style::default()
                                .absolute(0.0, 20.0)
                                .size(Dimension::Px(80.0), Dimension::Px(20.0)),
                        ),
                    ),
            );
        let layout = LayoutEngine
            .compute(&root, Viewport::new(100.0, 100.0, 1.0))
            .expect("valid layout");
        let paint = PaintList::build(&root, &layout);

        assert!(paint.commands().iter().any(|command| {
            matches!(
                command,
                PaintCommand::Text { element, bounds, clip, .. }
                    if *element == text_id
                        && *bounds == Rect::new(10.0, 30.0, 80.0, 20.0)
                        && *clip == Rect::new(10.0, 30.0, 50.0, 10.0)
            )
        }));
        let hits = HitTestTree::build(&root, &layout);
        assert_eq!(hits.hit(Point::new(20.0, 35.0)), Some(text_id));
        assert_ne!(hits.hit(Point::new(65.0, 35.0)), Some(text_id));
        assert_ne!(hits.hit(Point::new(20.0, 45.0)), Some(text_id));
    }

    #[test]
    fn scroll_box_text_and_hit_testing_share_the_clip() {
        let text_id = ElementId::new(2);
        let root = Element::scroll_box(
            ElementId::new(1),
            ElementKey::new("scroll"),
            Point::default(),
        )
        .styled(
            Style::default()
                .size(Dimension::Px(100.0), Dimension::Px(40.0))
                .axis(Axis::Column),
        )
        .child(
            Element::text(text_id, ElementKey::new("label"), "scrolled")
                .styled(Style::default().size(Dimension::Px(80.0), Dimension::Px(20.0))),
        );
        let layout = LayoutEngine
            .compute(&root, Viewport::new(100.0, 100.0, 1.0))
            .expect("valid layout");
        let paint = PaintList::build(&root, &layout);
        assert!(paint.commands().iter().any(|command| {
            matches!(
                command,
                PaintCommand::Text {
                    element,
                    clip,
                    ..
                } if *element == text_id && *clip == Rect::new(0.0, 0.0, 80.0, 20.0)
            )
        }));
        let hits = HitTestTree::build(&root, &layout);
        assert_ne!(hits.hit(Point::new(5.0, 45.0)), Some(text_id));
    }
}
