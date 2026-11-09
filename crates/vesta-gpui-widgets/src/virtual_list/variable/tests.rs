use super::*;

fn reference_edges(heights: &[f32]) -> Vec<f32> {
    let mut edges = vec![0.0];
    let mut total = 0.0_f64;
    for height in heights {
        total += f64::from(*height);
        edges.push(total as f32);
    }
    edges
}

#[test]
fn splices_match_independent_geometry_through_growth_replacement_and_removal() {
    let mut seed = 147_u64;
    let mut next = || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        seed >> 32
    };
    for count in [0, 1, 63, 64, 65, 127, 128, 129, 511, 512, 513] {
        let mut heights = vec![12.25; count];
        let mut metrics = VariableListMetrics::new(&heights).expect("source");
        for step in 0..192 {
            let start = if step % 3 == 0 {
                heights.len()
            } else {
                next() as usize % (heights.len() + 1)
            };
            let end = (start + next() as usize % 5).min(heights.len());
            let added = (0..next() % 8)
                .map(|_| 1.0 + (next() % 200) as f32 / 8.0)
                .collect::<Vec<_>>();
            let changed = heights[start..end] != added;
            assert_eq!(metrics.splice(start..end, &added), Ok(changed));
            heights.splice(start..end, added);
            if step % 4 == 0 && !heights.is_empty() {
                let index = next() as usize % heights.len();
                heights[index] = 9.125;
                metrics
                    .update_height(index, heights[index])
                    .expect("interleaved point update");
            }
            assert_eq!(
                metrics,
                VariableListMetrics::new(&heights).expect("full reconstruction")
            );
            let edges = reference_edges(&heights);
            for index in 0..heights.len() {
                assert_eq!(
                    metrics.row_bounds(index),
                    Some((edges[index], edges[index + 1] - edges[index]))
                );
            }
            for offset in [0.0, metrics.content_height() * 0.5, f32::MAX] {
                let range = metrics.range(37.5, offset, 2).expect("range after splice");
                if let Some(anchor) = range.anchor() {
                    assert_eq!(
                        metrics.offset_for_anchor(anchor, anchor.index(), 37.5),
                        Ok(range.offset())
                    );
                }
            }
        }
        metrics.splice(0..metrics.len(), &[]).expect("remove all");
        assert_eq!(metrics, VariableListMetrics::new(&[]).expect("empty"));
        // Cross several Fenwick powers of two by growing from the empty owner.
        for index in 0..1025 {
            metrics
                .splice(index..index, &[3.5])
                .expect("incremental append");
        }
        assert_eq!(
            metrics,
            VariableListMetrics::new(&[3.5; 1025]).expect("grown reference")
        );
    }
}

#[test]
fn splice_admission_is_atomic_and_uses_the_final_replacement_extent() {
    let mut metrics = VariableListMetrics::new(&[20.0; 65]).expect("rows");
    let saved = metrics.clone();
    for range in [Range { start: 2, end: 1 }, 0..66, usize::MAX..usize::MAX] {
        assert_eq!(
            metrics.splice(range, &[1.0]),
            Err(VirtualListError::ItemIndexOutOfBounds)
        );
        assert_eq!(metrics, saved);
    }
    for heights in [
        [2.0, f32::NAN],
        [2.0, 0.0],
        [2.0, f32::INFINITY],
        [2.0, -1.0],
    ] {
        assert_eq!(
            metrics.splice(64..65, &heights),
            Err(VirtualListError::InvalidGeometry)
        );
        assert_eq!(metrics, saved);
    }
    assert_eq!(metrics.splice(10..12, &[20.0, 20.0]), Ok(false));
    assert_eq!(metrics.splice(65..65, &[]), Ok(false));
    assert_eq!(metrics, saved);
    let mut full = VariableListMetrics::new(&[1.0, 16_777_215.0]).expect("extent limit");
    let saved = full.clone();
    assert_eq!(
        full.splice(2..2, &[1.0]),
        Err(VirtualListError::ContentTooLarge)
    );
    assert_eq!(full, saved);
    assert_eq!(full.splice(0..2, &[16_777_215.0, 1.0]), Ok(true));
    assert_eq!(
        full,
        VariableListMetrics::new(&[16_777_215.0, 1.0]).expect("same final extent")
    );
    assert_eq!(full.splice(0..2, &[1.0, 16_777_215.0]), Ok(true));
    assert_eq!(full, saved);
}

#[test]
fn incremental_geometry_matches_reference_after_random_updates_and_rejects_atomically() {
    let mut seed = 79_u64;
    let mut next = || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        seed >> 32
    };
    for count in [0, 1, 63, 64, 65, 128, 129, 1_031] {
        let mut heights = (0..count)
            .map(|_| 1.0 + (next() % 512) as f32 / 8.0)
            .collect::<Vec<_>>();
        let mut metrics = VariableListMetrics::new(&heights).expect("measured source");
        for _ in 0..128 {
            if count > 0 {
                let index = next() as usize % count;
                let height = 1.0 + (next() % 512) as f32 / 8.0;
                let changed = heights[index] != height;
                assert_eq!(metrics.update_height(index, height), Ok(changed));
                heights[index] = height;
            }
            let edges = reference_edges(&heights);
            assert_eq!(metrics.content_height(), *edges.last().expect("zero edge"));
            for index in 0..count {
                assert_eq!(
                    metrics.row_bounds(index),
                    Some((edges[index], edges[index + 1] - edges[index]))
                );
            }
            assert_eq!(metrics.row_bounds(count), None);
            for requested in [-1.0, (next() % 20_000) as f32 / 4.0, f32::MAX] {
                let range = metrics
                    .range(72.0, requested, 2)
                    .expect("bounded visible range");
                let offset = range.offset();
                let first = edges
                    .partition_point(|top| *top <= offset)
                    .saturating_sub(1)
                    .min(count);
                let end = edges
                    .partition_point(|top| f64::from(*top) < f64::from(offset) + 72.0)
                    .min(count)
                    .max(first.saturating_add(1).min(count));
                assert_eq!(
                    range.visible_rows(),
                    first.saturating_sub(2)..end.saturating_add(2).min(count)
                );
                assert_eq!(
                    range.anchor().map(ListScrollAnchor::index),
                    (count > 0).then_some(first)
                );
            }
        }
        let saved = metrics.clone();
        assert_eq!(
            metrics.update_height(count, 1.0),
            Err(VirtualListError::ItemIndexOutOfBounds)
        );
        assert_eq!(metrics, saved);
        if count > 0 {
            for height in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::MAX] {
                assert!(metrics.update_height(0, height).is_err());
                assert_eq!(metrics, saved, "failed update must preserve the index");
            }
            assert_eq!(metrics.update_height(0, heights[0]), Ok(false));
            assert_eq!(metrics, saved);
        }
    }
    let mut edge = VariableListMetrics::new(&[16_777_215.0, 1.0]).expect("content limit");
    let saved = edge.clone();
    assert_eq!(
        edge.update_height(1, 2.0),
        Err(VirtualListError::ContentTooLarge)
    );
    assert_eq!(edge, saved);
    edge.update_height(0, 16_777_214.0)
        .expect("shrink at limit");
    edge.update_height(1, 2.0).expect("reuse released extent");
    let range = edge
        .range(0.01, f32::MAX, 0)
        .expect("fractional last viewport");
    assert_eq!(range.visible_rows(), 1..2);
    assert_eq!(range.anchor().expect("last anchor").index(), 1);
}

#[test]
fn anchors_follow_host_remapping_and_stay_inside_shrunk_rows() {
    let mut metrics = VariableListMetrics::new(&[20.0; 12]).expect("rows");
    let anchor = metrics
        .range(40.0, 87.0, 3)
        .expect("scrolled view")
        .anchor()
        .expect("anchor");
    assert_eq!(anchor.index(), 4);
    assert_eq!(anchor.offset_in_row(), 7.0);
    metrics.update_height(0, 80.0).expect("earlier row grew");
    assert_eq!(metrics.offset_for_anchor(anchor, 4, 40.0), Ok(147.0));
    // The host resolves the same stable ID to index 6 after inserting/reordering.
    assert_eq!(metrics.offset_for_anchor(anchor, 6, 40.0), Ok(187.0));
    metrics.update_height(4, 3.0).expect("anchor row shrank");
    let offset = metrics
        .offset_for_anchor(anchor, 4, 40.0)
        .expect("restored anchor");
    assert!((140.0..143.0).contains(&offset));
    assert_eq!(
        metrics
            .range(40.0, offset, 0)
            .expect("current range")
            .anchor()
            .expect("same row")
            .index(),
        4
    );
    assert_eq!(
        metrics.offset_for_anchor(anchor, 12, 40.0),
        Err(VirtualListError::ItemIndexOutOfBounds)
    );
    assert_eq!(
        metrics.offset_for_anchor(anchor, 4, f32::NAN),
        Err(VirtualListError::InvalidGeometry)
    );
    assert_eq!(metrics.offset_for_anchor(anchor, 4, 1_000.0), Ok(0.0));
    let fixed = super::super::VirtualListRange::compute(12, 20.0, 40.0, 87.0, 3)
        .expect("fixed range")
        .anchor()
        .expect("fixed anchor");
    assert_eq!(fixed, anchor);
    assert_eq!(
        VariableListMetrics::new(&[])
            .expect("empty")
            .range(40.0, 0.0, 0)
            .expect("empty range")
            .anchor(),
        None
    );
}

#[test]
fn updated_projection_preserves_layout_hits_actions_focus_and_semantics() {
    use crate::WidgetIdentity;
    use vesta_gpui::{
        AccessibilityTree, ActionRoute, Element, ElementId, ElementKey, HitTestTree, LayoutEngine,
        Point, Semantic, SemanticRole, Style, Viewport,
    };

    let items = (0..1_000_usize).collect::<Vec<_>>();
    let mut metrics = VariableListMetrics::new(&[20.0; 1_000]).expect("source geometry");
    let anchor = metrics
        .range(100.0, 4_005.0, 2)
        .expect("old view")
        .anchor()
        .expect("row 200");
    metrics
        .update_height(8, 80.0)
        .expect("offscreen height changed");
    metrics
        .update_height(202, 40.0)
        .expect("visible height changed");
    let offset = metrics
        .offset_for_anchor(anchor, 200, 100.0)
        .expect("same top item");
    let range = metrics.range(100.0, offset, 2).expect("updated view");
    let shown = range.visible_rows();
    let mut built = Vec::new();
    let id = |index: usize| ElementId::new(100 + index as u64);
    let widget = variable_list_with_items(
        super::super::VirtualListIds {
            root: WidgetIdentity::new(ElementId::new(1), "list"),
            content: WidgetIdentity::new(ElementId::new(2), "content"),
        },
        Dimension::Px(240.0),
        &items,
        range,
        Some(0),
        |index| {
            built.push(*index);
            Widget::from_element(
                Element::container(id(*index), ElementKey::new(format!("row.{index}")))
                    .semantic(Semantic::new(
                        SemanticRole::ListItem,
                        format!("Record {index}"),
                    ))
                    .styled(Style::default().focusable(true)),
            )
            .on_click(*index)
        },
    )
    .expect("current projection");
    assert_eq!(&built[..shown.len()], &items[shown]);
    assert_eq!(built.last(), Some(&0));
    assert!(
        built.len() < 16,
        "only visible/overscan and retained rows are built"
    );
    let layout = LayoutEngine
        .compute(&widget.element, Viewport::new(240.0, 100.0, 1.5))
        .expect("real layout");
    let hits = HitTestTree::build(&widget.element, &layout);
    let route = ActionRoute::build(&widget.element, widget.actions).expect("current actions");
    assert_eq!(route.action_at(id(202)), Some(&202));
    assert!(route.focus_order().contains(&id(0)));
    assert_eq!(hits.hit(Point::new(10.0, 40.0)), Some(id(202)));
    let mut semantics = AccessibilityTree::default();
    let update = semantics.build(&widget.element, &layout, Some(id(0)), "Objects");
    assert_eq!(
        update.focus,
        semantics
            .node_for_element(id(0))
            .expect("retained semantic owner")
    );
    for index in [0, 200, 202] {
        assert!(semantics.node_for_element(id(index)).is_some());
    }
    assert!(semantics.node_for_element(id(900)).is_none());
    assert_eq!(layout.node(id(200)).expect("anchor row").rect.y, -5.0);
    assert_eq!(layout.node(id(202)).expect("changed row").rect.height, 40.0);
}
