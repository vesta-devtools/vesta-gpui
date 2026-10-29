//! Bounded list virtualization over the shared Element and ScrollBox model.

mod variable;

pub use variable::{VariableListMetrics, VariableListRange, variable_list_with_items};

use vesta_gpui::{Dimension, Element, Point, Semantic, SemanticRole, Style};

use crate::{Widget, WidgetIdentity};

const MAX_CONTENT_HEIGHT: f64 = 16_777_216.0;
const MAX_VISIBLE_ROWS: usize = 256;
const MAX_OVERSCAN: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualListError {
    InvalidGeometry,
    ContentTooLarge,
    ViewportTooDense,
    ItemCountMismatch,
}

/// Visible indices and scroll geometry for one immutable list projection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VirtualListRange {
    pub count: usize,
    pub first: usize,
    pub end: usize,
    pub offset: f32,
    pub row_height: f32,
    pub viewport_height: f32,
    pub content_height: f32,
}

impl VirtualListRange {
    pub fn compute(
        count: usize,
        row_height: f32,
        viewport_height: f32,
        offset: f32,
        overscan: usize,
    ) -> Result<Self, VirtualListError> {
        if !row_height.is_finite()
            || row_height < 1.0
            || !viewport_height.is_finite()
            || viewport_height <= 0.0
            || !offset.is_finite()
        {
            return Err(VirtualListError::InvalidGeometry);
        }
        let content_height = count as f64 * f64::from(row_height);
        if content_height > MAX_CONTENT_HEIGHT {
            return Err(VirtualListError::ContentTooLarge);
        }
        let visible_rows = ((viewport_height / row_height).ceil() as usize).saturating_add(1);
        if visible_rows > MAX_VISIBLE_ROWS {
            return Err(VirtualListError::ViewportTooDense);
        }
        let content_height = content_height as f32;
        let offset = offset.clamp(0.0, (content_height - viewport_height).max(0.0));
        let padding = overscan.min(MAX_OVERSCAN);
        let first = ((offset / row_height).floor() as usize).saturating_sub(padding);
        let end = (((offset + viewport_height) / row_height).ceil() as usize)
            .saturating_add(padding)
            .min(count);
        Ok(Self {
            count,
            first,
            end,
            offset,
            row_height,
            viewport_height,
            content_height,
        })
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.end - self.first
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.first == self.end
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VirtualListIds {
    pub root: WidgetIdentity,
    pub content: WidgetIdentity,
}

pub(crate) fn validate_item_range(
    range: VirtualListRange,
    count: usize,
) -> Result<(), VirtualListError> {
    if range.count != count {
        return Err(VirtualListError::ItemCountMismatch);
    }
    if range.first > range.end || range.end > count {
        return Err(VirtualListError::InvalidGeometry);
    }
    Ok(())
}

/// Build visible rows from actual typed items rather than forcing callers to index data.
/// A retained offscreen item is built once; identities must come from the item, not its position.
pub fn virtual_list_with_items<T, A>(
    ids: VirtualListIds,
    width: Dimension,
    items: &[T],
    range: VirtualListRange,
    retained_index: Option<usize>,
    mut row: impl FnMut(&T) -> Widget<A>,
) -> Result<Widget<A>, VirtualListError> {
    validate_item_range(range, items.len())?;
    Ok(virtual_list(ids, width, range, retained_index, |index| {
        row(&items[index])
    }))
}

/// Render visible rows and optionally retain a focused offscreen row.
/// The host supplies stable row IDs and typed actions.
#[must_use]
pub fn virtual_list<A>(
    ids: VirtualListIds,
    width: Dimension,
    range: VirtualListRange,
    retained_index: Option<usize>,
    row: impl FnMut(usize) -> Widget<A>,
) -> Widget<A> {
    let retained = retained_index
        .filter(|index| *index < range.count && (*index < range.first || *index >= range.end));
    let rows = (range.first..range.end)
        .chain(retained)
        .map(|index| (index, index as f32 * range.row_height, range.row_height));
    project_list(
        ids,
        width,
        range.offset,
        range.viewport_height,
        range.content_height,
        rows,
        row,
    )
}

fn project_list<A>(
    ids: VirtualListIds,
    width: Dimension,
    offset: f32,
    viewport_height: f32,
    content_height: f32,
    rows: impl Iterator<Item = (usize, f32, f32)>,
    mut row: impl FnMut(usize) -> Widget<A>,
) -> Widget<A> {
    let mut content = Widget::from_element(
        Element::container(ids.content.element, ids.content.key)
            .styled(Style::default().size(Dimension::Percent(1.0), Dimension::Px(content_height))),
    );
    for (index, top, height) in rows {
        let mut item = row(index);
        item.element.style = item
            .element
            .style
            .absolute(0.0, top)
            .size(Dimension::Percent(1.0), Dimension::Px(height));
        content = content.child(item);
    }
    Widget::from_element(
        Element::scroll_box(ids.root.element, ids.root.key, Point::new(0.0, offset))
            .semantic(Semantic::new(SemanticRole::List, "List"))
            .styled(Style::default().size(width, Dimension::Px(viewport_height))),
    )
    .child(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vesta_gpui::{ActionRoute, ElementId, ElementKey, LayoutEngine, Viewport};

    #[test]
    fn large_list_projects_only_visible_stable_rows() {
        let range = VirtualListRange::compute(10_000, 28.0, 140.0, 140_000.0, 2)
            .expect("bounded list geometry");
        assert!(range.len() <= 10);
        let mut projected = Vec::new();
        let widget = virtual_list(
            VirtualListIds {
                root: WidgetIdentity::new(ElementId::new(1), "list"),
                content: WidgetIdentity::new(ElementId::new(2), "list.content"),
            },
            Dimension::Px(240.0),
            range,
            None,
            |index| {
                projected.push(index);
                Widget::from_element(
                    Element::container(
                        ElementId::new(10_000 + index as u64),
                        ElementKey::new(format!("row.{index}")),
                    )
                    .styled(Style::default().focusable(true)),
                )
                .on_click(index)
            },
        );
        assert_eq!(projected, (range.first..range.end).collect::<Vec<_>>());
        let route = ActionRoute::build(&widget.element, widget.actions).expect("visible row route");
        assert_eq!(route.focus_order().len(), range.len());
        assert_eq!(
            route.action_for_hit(ElementId::new(10_000 + range.first as u64)),
            Some(&range.first)
        );
        let layout = LayoutEngine
            .compute(&widget.element, Viewport::new(240.0, 140.0, 1.0))
            .expect("virtual list layout");
        assert_eq!(
            layout.node(ElementId::new(2)).expect("content").rect.height,
            280_000.0
        );
        let viewport = layout.node(ElementId::new(1)).expect("scroll viewport");
        assert!(viewport.scroll_viewport().width < viewport.rect.width);
        assert_eq!(
            viewport.scroll_content,
            Some((viewport.scroll_viewport().width, 280_000.0))
        );
        let mut items = (0..10_000).collect::<Vec<_>>();
        items.swap(0, 9_999);
        let mut built = Vec::new();
        let retained = virtual_list_with_items(
            VirtualListIds {
                root: WidgetIdentity::new(ElementId::new(1), "list"),
                content: WidgetIdentity::new(ElementId::new(2), "list.content"),
            },
            Dimension::Px(240.0),
            &items,
            range,
            Some(0),
            |id| {
                built.push(*id);
                Widget::from_element(
                    Element::container(
                        ElementId::new(10_000 + *id as u64),
                        ElementKey::new(format!("row.{id}")),
                    )
                    .styled(Style::default().focusable(true)),
                )
                .on_click(*id)
            },
        )
        .expect("typed list data");
        assert_eq!(&built[..range.len()], &items[range.first..range.end]);
        assert_eq!(built.last(), Some(&9_999));
        let retained_route =
            ActionRoute::build(&retained.element, retained.actions).expect("retained focus route");
        assert_eq!(retained_route.focus_order().len(), range.len() + 1);
        assert!(
            retained_route
                .focus_order()
                .contains(&ElementId::new(19_999))
        );
        let mismatch: Result<Widget<usize>, _> = virtual_list_with_items(
            VirtualListIds {
                root: WidgetIdentity::new(ElementId::new(1), "list"),
                content: WidgetIdentity::new(ElementId::new(2), "list.content"),
            },
            Dimension::Px(240.0),
            &items[..9],
            range,
            None,
            |_| panic!("mismatched data must be rejected before constructing rows"),
        );
        assert_eq!(mismatch.err(), Some(VirtualListError::ItemCountMismatch));

        let heights = items
            .iter()
            .map(|id| if id % 3 == 0 { 56.0 } else { 28.0 })
            .collect::<Vec<_>>();
        let metrics = VariableListMetrics::new(&heights).expect("mixed measured heights");
        let variable_range = metrics.range(140.0, 140_000.0, 2).expect("measured range");
        let visible = variable_range.visible_rows();
        let offset = variable_range.offset();
        let mut built = Vec::new();
        let variable = variable_list_with_items(
            VirtualListIds {
                root: WidgetIdentity::new(ElementId::new(1), "list"),
                content: WidgetIdentity::new(ElementId::new(2), "list.content"),
            },
            Dimension::Px(240.0),
            &items,
            variable_range,
            Some(0),
            |id| {
                built.push(*id);
                Widget::from_element(
                    Element::container(
                        ElementId::new(10_000 + *id as u64),
                        ElementKey::new(format!("row.{id}")),
                    )
                    .semantic(Semantic::new(
                        SemanticRole::ListItem,
                        format!("Record {id}"),
                    ))
                    .styled(Style::default().focusable(true)),
                )
                .on_click(*id)
            },
        )
        .expect("matching measured data");
        assert_eq!(&built[..visible.len()], &items[visible.clone()]);
        assert_eq!(built.last(), Some(&9_999));
        assert_eq!(built.len(), visible.len() + 1);
        assert!(built.len() <= MAX_VISIBLE_ROWS + 2 * MAX_OVERSCAN + 1);
        let layout = LayoutEngine
            .compute(&variable.element, Viewport::new(240.0, 140.0, 1.0))
            .expect("measured row layout");
        assert_eq!(
            layout.node(ElementId::new(2)).expect("content").rect.height,
            metrics.content_height()
        );
        for index in visible {
            let id = ElementId::new(10_000 + items[index] as u64);
            let node = layout.node(id).expect("materialized measured row");
            let (top, height) = metrics.row_bounds(index).expect("measured row geometry");
            assert_eq!(node.rect.y, top - offset);
            assert_eq!(node.rect.height, height);
        }
        let route =
            ActionRoute::build(&variable.element, variable.actions).expect("measured route");
        assert_eq!(route.action_at(ElementId::new(19_999)), Some(&9_999));
        assert!(route.focus_order().contains(&ElementId::new(19_999)));
        let mismatch: Result<Widget<usize>, _> = variable_list_with_items(
            VirtualListIds {
                root: WidgetIdentity::new(ElementId::new(1), "list"),
                content: WidgetIdentity::new(ElementId::new(2), "list.content"),
            },
            Dimension::Px(240.0),
            &items[..9],
            metrics.range(140.0, 0.0, 2).expect("valid range"),
            None,
            |_| panic!("mismatched variable items must reject before callbacks"),
        );
        assert_eq!(mismatch.err(), Some(VirtualListError::ItemCountMismatch));
    }

    #[test]
    fn range_clamps_edges_and_rejects_unbounded_geometry() {
        let top = VirtualListRange::compute(10_000, 28.0, 140.0, -100.0, 2).expect("top");
        assert_eq!(top.first, 0);
        let bottom = VirtualListRange::compute(10_000, 28.0, 140.0, f32::MAX, 2).expect("bottom");
        assert_eq!(bottom.end, 10_000);
        assert_eq!(
            VirtualListRange::compute(10_000, 0.0, 140.0, 0.0, 2),
            Err(VirtualListError::InvalidGeometry)
        );
        assert_eq!(
            VirtualListRange::compute(10_000_000, 28.0, 140.0, 0.0, 2),
            Err(VirtualListError::ContentTooLarge)
        );
        for viewport in [f32::MAX, f32::MAX / 2.0, 256.0] {
            assert_eq!(
                VirtualListRange::compute(1, 1.0, viewport, 0.0, 0),
                Err(VirtualListError::ViewportTooDense),
                "dense finite viewports must reject without integer overflow"
            );
        }
        for heights in [&[0.0][..], &[f32::NAN], &[f32::INFINITY], &[-1.0]] {
            assert_eq!(
                VariableListMetrics::new(heights),
                Err(VirtualListError::InvalidGeometry)
            );
        }
        assert_eq!(
            VariableListMetrics::new(&[f32::MAX]),
            Err(VirtualListError::ContentTooLarge)
        );
        let metrics = VariableListMetrics::new(&[10.0, 30.0, 20.0]).expect("mixed heights");
        assert_eq!(
            metrics
                .range(20.0, 10.0, 0)
                .expect("boundary range")
                .visible_rows(),
            1..2
        );
        assert_eq!(
            metrics
                .range(20.0, -1.0, 0)
                .expect("top range")
                .visible_rows(),
            0..2
        );
        assert_eq!(
            metrics
                .range(20.0, f32::MAX, 0)
                .expect("bottom range")
                .visible_rows(),
            2..3
        );
        assert_eq!(
            metrics.range(f32::NAN, 0.0, 0),
            Err(VirtualListError::InvalidGeometry)
        );
        let dense = VariableListMetrics::new(&[1.0; 1_000]).expect("finite dense rows");
        assert_eq!(
            dense.range(300.0, 0.0, 0),
            Err(VirtualListError::ViewportTooDense)
        );
        let bounded = dense
            .range(10.0, 400.0, usize::MAX)
            .expect("bounded overscan");
        assert_eq!(bounded.visible_rows(), 368..442);
        let empty = VariableListMetrics::new(&[]).expect("empty measured list");
        assert!(empty.range(20.0, 20.0, 2).expect("empty range").is_empty());
        let large = VariableListMetrics::new(&[16_777_215.0, 1.0]).expect("height limit");
        assert_eq!(
            metrics
                .range(f32::MIN_POSITIVE, 10.0, 0)
                .expect("positive subpixel view")
                .visible_rows(),
            1..2,
        );
        assert_eq!(
            large
                .range(0.01, f32::MAX, 0)
                .expect("fractional viewport")
                .visible_rows(),
            1..2
        );
    }
}
