use super::*;
use std::cell::Cell;
use vesta_gpui::render::{ImageIssue, ImageResolution};
use vesta_gpui::{Dimension, Element, ElementId, ElementKey, LayoutEngine, Style};

struct Resolver {
    state: ImageResolution,
    calls: Cell<usize>,
}
impl ImageResolver for Resolver {
    fn resolve(&self, _: &str) -> Option<ImageData> {
        panic!("preparation must use the state contract");
    }
    fn resolve_state(&self, _: &str) -> ImageResolution {
        self.calls.set(self.calls.get() + 1);
        self.state.clone()
    }
}

fn fixture(presentation: Option<vesta_gpui::ImagePresentation>) -> PaintList {
    let image = match presentation {
        Some(presentation) => Element::fitted_image(
            ElementId::new(2),
            ElementKey::new("image"),
            "fixture://image",
            presentation,
        ),
        None => Element::image(
            ElementId::new(2),
            ElementKey::new("image"),
            "fixture://image",
        ),
    };
    let root = Element::container(ElementId::new(1), ElementKey::new("root"))
        .styled(Style::default().size(Dimension::Px(100.0), Dimension::Px(100.0)))
        .child(image.styled(Style::default().size(Dimension::Px(16.0), Dimension::Px(16.0))))
        .child(
            Element::container(ElementId::new(3), ElementKey::new("neighbor")).styled(
                Style::default()
                    .size(Dimension::Px(16.0), Dimension::Px(16.0))
                    .background(Color::rgb8(20, 80, 140)),
            ),
        );
    let layout = LayoutEngine
        .compute(&root, Viewport::new(100.0, 100.0, 1.0))
        .expect("layout");
    PaintList::build(&root, &layout)
}

fn prepare_fixture(
    paint: &PaintList,
    resolver: Option<&dyn ImageResolver>,
    viewport: Viewport,
    max_dimension: u32,
) -> (Vec<Prepared>, RenderOutput) {
    crate::prepare_paint(
        viewport,
        paint,
        &mut TextInputContext::default(),
        &mut TextRasterizer::default(),
        resolver,
        max_dimension,
        RenderOptions {
            clear: [0.0; 4],
            selection: Color::rgb8(60, 120, 220),
            caret: Color::rgb8(255, 255, 255),
            caret_visible: true,
        },
    )
    .expect("optional image state must not terminate the frame")
    .into_parts()
}

#[test]
fn image_failures_preserve_adjacent_paint_identity_and_recover_without_negative_cache() {
    use vesta_gpui::{ImageFit, ImagePresentation};
    for presentation in [
        None,
        Some(ImagePresentation::new(ImageFit::Stretch)),
        Some(ImagePresentation::new(ImageFit::Contain)),
        Some(ImagePresentation::new(ImageFit::Cover)),
    ] {
        verify_image_failures(presentation);
    }
}

fn verify_image_failures(presentation: Option<vesta_gpui::ImagePresentation>) {
    let paint = fixture(presentation);
    let viewport = Viewport::new(100.0, 100.0, 1.0);
    let failed = ImageResolution::Failed {
        code: "fixture_decode_failed".into(),
        message: "bad format".into(),
    };
    let invalid = ImageResolution::Ready(Arc::new(ImageData {
        width: 2,
        height: 2,
        rgba: vec![0; 3],
    }));
    for (state, expected) in [
        (ImageResolution::Loading, "ui_image_loading"),
        (ImageResolution::Missing, "ui_image_missing"),
        (failed.clone(), "ui_image_failed"),
        (invalid, "ui_image_invalid_data"),
        (
            ImageResolution::Ready(Arc::new(ImageData {
                width: 0,
                height: 2,
                rgba: vec![],
            })),
            "ui_image_invalid_data",
        ),
        (
            ImageResolution::Ready(Arc::new(ImageData {
                width: 5,
                height: 1,
                rgba: vec![0; 20],
            })),
            "ui_image_dimension_exceeded",
        ),
    ] {
        let resolver = Resolver {
            state,
            calls: Cell::new(0),
        };
        let (draws, output) = prepare_fixture(&paint, Some(&resolver), viewport, 4);
        assert_eq!(resolver.calls.get(), 1);
        assert_eq!(output.unsupported_images, 1);
        assert_eq!(output.image_diagnostics[0].element, ElementId::new(2));
        assert_eq!(output.image_diagnostics[0].source, "fixture://image");
        assert_eq!(output.image_diagnostics[0].issue.code(), expected);
        if let ImageResolution::Failed { code, message } = &resolver.state {
            assert_eq!(
                output.image_diagnostics[0].issue,
                ImageIssue::Failed {
                    code: code.clone(),
                    message: message.clone()
                }
            );
        }
        assert!(
            draws
                .iter()
                .all(|draw| matches!(draw, Prepared::Shape { .. }))
        );
        assert!(
            draws
                .iter()
                .map(|draw| match draw {
                    Prepared::Shape { vertices, .. } => vertices.len(),
                    _ => 0,
                })
                .sum::<usize>()
                >= 12,
            "placeholder must not discard adjacent geometry"
        );
    }
    let (_, missing) = prepare_fixture(&paint, None, viewport, 4);
    assert_eq!(missing.image_diagnostics[0].issue, ImageIssue::Missing);

    let pixels = Arc::new(ImageData {
        width: 2,
        height: 2,
        rgba: vec![255; 16],
    });
    let mut resolver = Resolver {
        state: failed,
        calls: Cell::new(0),
    };
    prepare_fixture(&paint, Some(&resolver), viewport, 4);
    resolver.state = ImageResolution::Ready(Arc::clone(&pixels));
    let (draws, recovered) = prepare_fixture(&paint, Some(&resolver), viewport, 4);
    assert!(recovered.image_diagnostics.is_empty());
    assert_eq!(recovered.unsupported_images, 0);
    assert!(
        draws.iter().any(
            |draw| matches!(draw, Prepared::Bitmap { image, .. } if Arc::ptr_eq(image, &pixels))
        )
    );
    assert_eq!(
        resolver.calls.get(),
        2,
        "failure must not be negatively cached"
    );

    let invisible = Viewport::new(0.5, 0.5, 1.0);
    // The clip is outside the viewport, so even a valid source must not resolve.
    let root = Element::image(
        ElementId::new(9),
        ElementKey::new("offscreen"),
        "fixture://offscreen",
    );
    let layout = LayoutEngine.compute(&root, viewport).expect("layout");
    let paint =
        PaintList::build_from_elements(&[&root], &layout, Rect::new(50.0, 50.0, 10.0, 10.0));
    let before = resolver.calls.get();
    let (draws, clipped) = prepare_fixture(&paint, Some(&resolver), invisible, 4);
    assert!(draws.is_empty());
    assert!(clipped.image_diagnostics.is_empty());
    assert_eq!(
        resolver.calls.get(),
        before,
        "clipped resources must not be resolved"
    );
}

#[test]
fn oversized_optional_image_is_rejected_before_gpu_upload() {
    let image = Arc::new(ImageData {
        width: 4096,
        height: 2049,
        rgba: vec![0; 4096 * 2049 * 4],
    });
    let resolver = Resolver {
        state: ImageResolution::Ready(image),
        calls: Cell::new(0),
    };
    for presentation in [
        None,
        Some(vesta_gpui::ImagePresentation::new(
            vesta_gpui::ImageFit::Cover,
        )),
    ] {
        let (draws, output) = prepare_fixture(
            &fixture(presentation),
            Some(&resolver),
            Viewport::new(100.0, 100.0, 1.0),
            8192,
        );
        assert_eq!(
            output.image_diagnostics[0].issue,
            ImageIssue::BudgetExceeded
        );
        assert!(
            draws
                .iter()
                .all(|draw| matches!(draw, Prepared::Shape { .. }))
        );
    }
}
