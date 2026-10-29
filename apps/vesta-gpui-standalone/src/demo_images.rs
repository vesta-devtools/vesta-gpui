//! Gallery pixels and an explicit smoke-only optional-resource failure sequence.
use std::sync::{Arc, OnceLock};
use vesta_gpui::render::{ImageData, ImageResolution, ImageResolver, RenderOutput};
use vesta_gpui::{
    Dimension, Element, ElementKey, ElementScope, PaintCommand, PaintList, Style, Viewport,
};

pub(super) fn append_smoke_image(root: &mut Element, scope: ElementScope) {
    // Reserved fixture ID: a visible resource is required to prove real uploads.
    let image = if faults_enabled() {
        Element::fitted_image(
            scope.element(27_000_000),
            ElementKey::new("smoke.image"),
            "demo://checkerboard",
            vesta_gpui::ImagePresentation::new(vesta_gpui::ImageFit::Cover),
        )
    } else {
        Element::image(
            scope.element(27_000_000),
            ElementKey::new("smoke.image"),
            "demo://checkerboard",
        )
    };
    root.children.push(
        image.styled(
            Style::default()
                .absolute(4.0, 4.0)
                .size(Dimension::Px(16.0), Dimension::Px(16.0))
                .pointer_events(false),
        ),
    );
}

pub(super) fn faults_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::args().any(|arg| arg == "--smoke-image-failures"))
}

pub(super) struct DemoImages {
    frame: u64,
}

impl DemoImages {
    pub(super) fn for_frame(frame: u64) -> Self {
        Self { frame }
    }
}

fn checkerboard() -> Arc<ImageData> {
    static IMAGE: OnceLock<Arc<ImageData>> = OnceLock::new();
    Arc::clone(IMAGE.get_or_init(|| {
        let mut rgba = Vec::with_capacity(64 * 64 * 4);
        for y in 0..64 {
            for x in 0..64 {
                rgba.extend_from_slice(if (x / 8 + y / 8) % 2 == 0 {
                    &[80, 145, 235, 255]
                } else {
                    &[31, 38, 48, 255]
                });
            }
        }
        Arc::new(ImageData {
            width: 64,
            height: 64,
            rgba,
        })
    }))
}

impl ImageResolver for DemoImages {
    fn resolve(&self, source: &str) -> Option<ImageData> {
        match self.resolve_state(source) {
            ImageResolution::Ready(image) => Some(image.as_ref().clone()),
            _ => None,
        }
    }

    fn resolve_state(&self, source: &str) -> ImageResolution {
        if source == "demo://image-wide" || source == "demo://image-tall" {
            return ImageResolution::Ready(fitting_pattern(source == "demo://image-tall"));
        }
        if source != "demo://checkerboard" {
            return ImageResolution::Missing;
        }
        if faults_enabled() {
            match self.frame {
                0 => return ImageResolution::Loading,
                1 => return ImageResolution::Missing,
                2 => {
                    return ImageResolution::Failed {
                        code: "fixture_decode_failed".into(),
                        message: "deliberate smoke fixture".into(),
                    };
                }
                3 => {
                    return ImageResolution::Ready(Arc::new(ImageData {
                        width: 2,
                        height: 2,
                        rgba: vec![0; 3],
                    }));
                }
                4 => {
                    return ImageResolution::Ready(Arc::new(ImageData {
                        width: 4096,
                        height: 2049,
                        rgba: vec![0; 4096 * 2049 * 4],
                    }));
                }
                _ => {}
            }
        }
        ImageResolution::Ready(checkerboard())
    }
}

fn fitting_pattern(tall: bool) -> Arc<ImageData> {
    static WIDE: OnceLock<Arc<ImageData>> = OnceLock::new();
    static TALL: OnceLock<Arc<ImageData>> = OnceLock::new();
    Arc::clone((if tall { &TALL } else { &WIDE }).get_or_init(|| {
        let (width, height) = if tall { (64_u32, 128_u32) } else { (128, 64) };
        let colors = [
            [220_u8, 40, 50],
            [40, 200, 80],
            [40, 100, 220],
            [230, 180, 40],
        ];
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for y in 0..height {
            for x in 0..width {
                let color = colors[(x * 4 / width) as usize];
                let shade = 4 - y * 4 / height;
                let alpha = if x >= width * 3 / 8
                    && x < width * 5 / 8
                    && y >= height * 3 / 8
                    && y < height * 5 / 8
                {
                    0
                } else if x < width / 4 && y < height / 4 {
                    128
                } else {
                    255
                };
                rgba.extend_from_slice(&[
                    (u32::from(color[0]) * shade / 4) as u8,
                    (u32::from(color[1]) * shade / 4) as u8,
                    (u32::from(color[2]) * shade / 4) as u8,
                    alpha,
                ]);
            }
        }
        Arc::new(ImageData {
            width,
            height,
            rgba,
        })
    }))
}

pub(super) fn verify_frame(
    frame: u64,
    paint: &PaintList,
    viewport: Viewport,
    output: &RenderOutput,
) -> Result<(), String> {
    if !faults_enabled() {
        return Ok(());
    }
    let expected = match frame {
        0 => Some("ui_image_loading"),
        1 => Some("ui_image_missing"),
        2 => Some("ui_image_failed"),
        3 => Some("ui_image_invalid_data"),
        4 => Some("ui_image_budget_exceeded"),
        _ => None,
    };
    let visible_images = paint
        .commands()
        .iter()
        .filter_map(|command| match command {
            PaintCommand::Image {
                element,
                bounds,
                clip,
                source,
            }
            | PaintCommand::FittedImage {
                element,
                bounds,
                clip,
                source,
                ..
            } if source == "demo://checkerboard"
                && bounds
                    .intersection(*clip)
                    .and_then(|r| r.intersection(viewport.logical_rect()))
                    .is_some() =>
            {
                Some(*element)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let diagnostics = &output.image_diagnostics;
    let valid = if let Some(code) = expected {
        !visible_images.is_empty()
            && diagnostics.iter().map(|d| d.element).collect::<Vec<_>>() == visible_images
            && diagnostics
                .iter()
                .all(|d| d.source == "demo://checkerboard" && d.issue.code() == code)
            && output.bitmap_uploads == 0
            && output.bitmap_reuses == 0
    } else {
        diagnostics.is_empty()
            && if frame == 5 {
                output.bitmap_uploads > 0
            } else {
                output.bitmap_reuses > 0
            }
    };
    if valid {
        eprintln!(
            "UI-R01 frame={frame} state={} presented=true",
            expected.unwrap_or("ready")
        );
        Ok(())
    } else {
        Err(format!(
            "ui_image_smoke_failed: frame={frame}, expected={expected:?}, output={output:?}"
        ))
    }
}
