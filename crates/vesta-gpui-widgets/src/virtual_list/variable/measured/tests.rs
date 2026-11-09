use super::*;
use vesta_gpui::{FontRole, TextInputContext};

fn source(count: usize) -> (LazyListMetrics, TextInputContext) {
    let context = TextInputContext::default();
    let metrics =
        LazyListMetrics::with_estimate(count, 20.0, 200.0, context.measurement_revision())
            .expect("estimated rows");
    (metrics, context)
}

#[test]
fn measurement_spends_only_its_budget_and_preserves_a_host_remapped_anchor() {
    let (mut rows, _) = source(100_000);
    let anchor = rows
        .metrics()
        .range(100.0, 4005.0, 2)
        .expect("view")
        .anchor()
        .expect("row 200");
    let requested = rows
        .metrics()
        .range(100.0, 4005.0, 2)
        .expect("view")
        .visible_rows();
    let mut calls = Vec::new();
    let result = rows
        .measure(requested.clone(), 3, |index| {
            calls.push(index);
            Some(30.0)
        })
        .expect("measured budget");
    assert_eq!(calls, requested.clone().take(3).collect::<Vec<_>>());
    assert_eq!(result.attempted, 3);
    assert_eq!(result.measured, 3);
    assert_eq!(result.pending, requested.len() - 3);
    assert!(result.geometry_changed);
    let offset = rows
        .metrics()
        .offset_for_anchor(anchor, 200, 100.0)
        .expect("same record");
    assert_eq!(
        rows.metrics()
            .range(100.0, offset, 0)
            .expect("restored")
            .anchor(),
        Some(anchor)
    );
    let rest = rows
        .measure(requested.clone(), usize::MAX, |_| Some(30.0))
        .expect("remaining rows");
    assert_eq!(rest.measured, requested.len() - 3);
    assert_eq!(rest.pending, 0);
    let complete = rows
        .measure(requested, usize::MAX, |_| {
            panic!("cached rows must not remeasure")
        })
        .expect("steady view");
    assert_eq!(
        complete,
        ListMeasurement {
            attempted: 0,
            measured: 0,
            geometry_changed: false,
            pending: 0
        }
    );
    assert_eq!(rows.is_measured(99_999), Some(false));
    assert_eq!(rows.is_measured(100_000), None);
}

#[test]
fn context_content_and_splices_invalidate_only_their_owned_measurements() {
    let (mut rows, mut context) = source(5);
    rows.measure(0..5, 5, |index| Some(20.0 + index as f32))
        .expect("all rows");
    let saved = rows.metrics().clone();
    assert!(
        !rows
            .set_context(200.0, context.measurement_revision())
            .expect("no-op context")
    );
    assert_eq!(rows.is_measured(0), Some(true));
    assert!(rows.invalidate(1..2).expect("changed text"));
    assert_eq!(rows.is_measured(0), Some(true));
    assert_eq!(rows.is_measured(1), Some(false));
    assert_eq!(
        rows.metrics(),
        &saved,
        "old height remains a provisional estimate"
    );
    assert!(rows.splice(0..0, &[15.0, 16.0]).expect("prepend"));
    assert_eq!(rows.is_measured(0), Some(false));
    assert_eq!(rows.is_measured(2), Some(true));
    assert_eq!(rows.is_measured(3), Some(false));
    assert_eq!(rows.is_measured(6), Some(true));
    assert!(
        rows.splice(2..3, &[20.0])
            .expect("same-height replacement invalidates content")
    );
    assert_eq!(rows.is_measured(2), Some(false));
    let old_geometry = rows.metrics().clone();
    assert!(
        rows.set_context(180.0, context.measurement_revision())
            .expect("new width")
    );
    assert_eq!(rows.metrics(), &old_geometry);
    assert!(rows.measured.iter().all(|value| !*value));
    rows.measure(0..7, 7, |_| Some(24.0))
        .expect("new width measurements");
    context.set_font_stack(FontRole::Ui, "Host font stack");
    assert!(
        rows.set_context(180.0, context.measurement_revision())
            .expect("font revision")
    );
    assert!(rows.measured.iter().all(|value| !*value));
    let foreign = TextInputContext::default();
    assert!(
        rows.set_context(180.0, foreign.measurement_revision())
            .expect("replacement context")
    );
}

#[test]
fn invalid_measurements_and_contexts_leave_geometry_and_validity_unchanged() {
    let (mut rows, context) = source(5);
    rows.measure(0..1, 1, |_| Some(24.0))
        .expect("one known row");
    let saved = rows.clone();
    for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            rows.set_context(invalid, context.measurement_revision()),
            Err(VirtualListError::InvalidGeometry)
        );
        assert_eq!(rows.width, saved.width);
        assert_eq!(rows.text_revision, saved.text_revision);
        assert_eq!(
            rows.measure(1..3, 2, |index| Some(if index == 1 {
                32.0
            } else {
                invalid
            })),
            Err(VirtualListError::InvalidGeometry)
        );
        assert_eq!(rows.metrics(), saved.metrics());
        assert_eq!(rows.measured, saved.measured);
    }
    assert_eq!(
        rows.measure(1..3, 2, |_| Some(f32::MAX)),
        Err(VirtualListError::ContentTooLarge)
    );
    assert_eq!(rows.metrics(), saved.metrics());
    assert_eq!(rows.measured, saved.measured);
    assert_eq!(
        rows.splice(1..2, &[2.0, f32::NAN]),
        Err(VirtualListError::InvalidGeometry)
    );
    assert_eq!(rows.metrics(), saved.metrics());
    assert_eq!(rows.measured, saved.measured);
    assert_eq!(
        rows.invalidate(4..6),
        Err(VirtualListError::ItemIndexOutOfBounds)
    );
    assert_eq!(rows.measured, saved.measured);
    assert_eq!(
        rows.measure(0..6, 1, |_| panic!("invalid ranges invoke no callback")),
        Err(VirtualListError::ItemIndexOutOfBounds)
    );
    let (mut large, _) = source(1000);
    assert_eq!(
        large.measure(0..321, 1, |_| panic!("unbounded ranges invoke no callback")),
        Err(VirtualListError::ViewportTooDense)
    );
}

#[test]
fn zero_budget_deferred_empty_and_removed_rows_have_explicit_outcomes() {
    let (mut rows, _) = source(4);
    let empty_budget = rows
        .measure(0..4, 0, |_| panic!("zero attempts"))
        .expect("zero budget");
    assert_eq!(empty_budget.pending, 4);
    assert_eq!(empty_budget.attempted, 0);
    let deferred = rows
        .measure(0..4, 2, |index| (index != 0).then_some(25.0))
        .expect("one deferred");
    assert_eq!(
        deferred,
        ListMeasurement {
            attempted: 2,
            measured: 1,
            geometry_changed: true,
            pending: 3
        }
    );
    let retry = rows
        .measure(0..4, 4, |_| Some(25.0))
        .expect("retry the remainder");
    assert_eq!(retry.measured, 3);
    assert_eq!(retry.pending, 0);
    rows.splice(0..4, &[]).expect("remove all");
    assert!(rows.metrics().is_empty());
    assert_eq!(rows.is_measured(0), None);
    assert_eq!(
        rows.measure(0..0, 4, |_| panic!("empty source"))
            .expect("empty")
            .pending,
        0
    );
    for (count, estimate) in [(usize::MAX, 20.0), (1, f32::MAX)] {
        let context = TextInputContext::default();
        assert!(matches!(
            LazyListMetrics::with_estimate(count, estimate, 200.0, context.measurement_revision()),
            Err(VirtualListError::ContentTooLarge)
        ));
    }
}
