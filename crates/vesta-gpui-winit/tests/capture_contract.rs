//! Public capture bounds and redaction across actual Core layout/snapshot data.
use vesta_gpui::{
    ActionRoute, Dimension, Element, ElementId, ElementKey, LayoutEngine, Rect, Semantic,
    SemanticRole, Style, UiQueryLimits, UiSnapshot, UiSnapshotFrame, UiSnapshotId, UiWindowId,
    Viewport,
};
use vesta_gpui_winit::{UiCaptureGeometry, UiCaptureRequest};

#[test]
fn conservative_crop_edges_preserve_fractional_pixels_and_reject_invalid_extents() {
    let rect = Rect::new(3200.0, 0.0, 1.6, 1.0);
    let viewport = Viewport::new(3204.0, 20.0, 1.25);
    let geometry = UiCaptureGeometry::new(rect, viewport).expect("small crop in valid target");
    assert_eq!(geometry.origin, [4000, 0]);
    assert_eq!(
        geometry.origin[0] + geometry.extent[0],
        (f64::from(rect.right()) * f64::from(viewport.scale_factor)).ceil() as u32,
        "a capture crop must not lose its conservative right pixel"
    );
    assert_eq!(
        geometry.logical,
        rect.intersection(viewport.logical_rect())
            .expect("clipped request")
    );
    for invalid in [
        Rect::new(f32::NAN, 0.0, 1.0, 1.0),
        Rect::new(f32::MAX, 0.0, f32::MAX, 1.0),
        Rect::new(f32::MAX, 0.0, 1.0, 1.0),
        Rect::new(0.0, 0.0, -1.0, 1.0),
    ] {
        assert_eq!(
            UiCaptureGeometry::new(invalid, viewport).err(),
            Some("invalid_capture_region")
        );
    }
    for invalid in [
        Viewport::new(f32::MAX, 20.0, 4.0),
        Viewport::new(4_294_967_296.0, 20.0, 1.0),
        Viewport::new(20.0, 20.0, f32::NAN),
    ] {
        assert_eq!(
            UiCaptureGeometry::new(Rect::new(0.0, 0.0, 1.0, 1.0), invalid).err(),
            Some("invalid_capture_region")
        );
    }
    let whole = Viewport::new(1280.8, 16.0, 1.25);
    let last =
        UiCaptureGeometry::new(whole.logical_rect(), whole).expect("native target convention");
    assert_eq!(last.extent[0], 1601, "do not invent a larger native target");
    assert_eq!(
        UiCaptureGeometry::new(
            Rect::new(0.0, 0.0, 4096.0, 4096.0),
            Viewport::new(4096.0, 4096.0, 1.0)
        )
        .err(),
        Some("invalid_capture_region")
    );
}

#[test]
fn capture_rejects_sensitive_pixels_outside_the_requested_logical_crop() {
    let root = Element::container(ElementId::new(1), ElementKey::new("root"))
        .styled(Style::default().size(Dimension::Px(32.0), Dimension::Px(16.0)))
        .child(
            Element::container(ElementId::new(2), ElementKey::new("secret"))
                .styled(
                    Style::default()
                        .absolute(10.0, 0.0)
                        .size(Dimension::Px(2.0), Dimension::Px(16.0)),
                )
                .semantic(
                    Semantic::new(SemanticRole::PasswordInput, "credential").value("never export"),
                ),
        );
    let viewport = Viewport::new(32.0, 16.0, 1.25);
    let capture = |viewport: Viewport, limits: UiQueryLimits| {
        let layout = LayoutEngine.compute(&root, viewport).expect("layout");
        let route = ActionRoute::<()>::build(&root, []).expect("route");
        UiSnapshot::capture(
            UiSnapshotFrame {
                id: UiSnapshotId {
                    instance: 42,
                    window: UiWindowId(1),
                    revision: 1,
                },
                root: &root,
                layout: &layout,
                route: &route,
                focus: None,
                modal_root: None,
            },
            limits,
            |_| None,
            |_| false,
        )
        .expect("realized snapshot")
    };
    let snapshot = capture(viewport, UiQueryLimits::default());
    assert!(snapshot.nodes().iter().any(|node| node.value.redacted));
    let mut request = UiCaptureRequest {
        instance: 42,
        window: UiWindowId(1),
        revision: 1,
        element: None,
        rect: Some(Rect::new(0.0, 0.0, 9.8, 16.0)),
        viewport: None,
    };
    assert_eq!(
        request.geometry(&snapshot, viewport).err(),
        Some("sensitive_region"),
        "a rounded capture pixel overlapping a redacted control must be rejected"
    );
    request.rect = Some(Rect::new(0.0, 0.0, 9.5, 16.0));
    let safe = request
        .geometry(&snapshot, viewport)
        .expect("crop before sensitive pixel");
    assert_eq!(safe.extent, [12, 20]);
    assert_eq!(safe.logical.width, 9.5);
    // Geometry is intentionally reusable for pending-capture revalidation;
    // source authorization belongs to Host, not this arithmetic method.
    request.revision = 2;
    assert!(request.geometry(&snapshot, viewport).is_ok());
    let incomplete = capture(
        viewport,
        UiQueryLimits {
            nodes: 1,
            ..UiQueryLimits::default()
        },
    );
    assert_eq!(
        request.geometry(&incomplete, viewport).err(),
        Some("capture_incomplete_source")
    );
    let integral = Viewport::new(32.0, 16.0, 1.0);
    let touching = capture(integral, UiQueryLimits::default());
    request.rect = Some(Rect::new(0.0, 0.0, 10.0, 16.0));
    assert_eq!(
        request
            .geometry(&touching, integral)
            .expect("half-open boundary")
            .extent,
        [10, 16]
    );
}
