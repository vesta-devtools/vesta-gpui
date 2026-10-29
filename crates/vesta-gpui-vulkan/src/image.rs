//! Optional image failures stay local; GPU allocation/submission failures do not.
use std::sync::Arc;
use vesta_gpui::ElementId;
use vesta_gpui::render::{
    ImageData, ImageDiagnostic, ImageIssue, ImageResolution, ImageResolver, RenderOutput,
};

pub(super) const MAX_TEXTURE_BYTES: usize = 32 * 1024 * 1024;

pub(super) fn resolve(
    element: ElementId,
    source: &str,
    resolver: Option<&dyn ImageResolver>,
    max_dimension: u32,
    output: &mut RenderOutput,
) -> Option<Arc<ImageData>> {
    let issue = match resolver.map(|r| r.resolve_state(source)) {
        Some(ImageResolution::Ready(image)) => {
            let bytes = (image.width as usize)
                .checked_mul(image.height as usize)
                .and_then(|pixels| pixels.checked_mul(4));
            if image.width == 0 || image.height == 0 || bytes != Some(image.rgba.len()) {
                ImageIssue::InvalidData
            } else if image.width > max_dimension || image.height > max_dimension {
                ImageIssue::DimensionExceeded
            } else if image.rgba.len() > MAX_TEXTURE_BYTES {
                ImageIssue::BudgetExceeded
            } else {
                return Some(image);
            }
        }
        Some(ImageResolution::Loading) => ImageIssue::Loading,
        Some(ImageResolution::Failed { code, message }) => ImageIssue::Failed { code, message },
        Some(ImageResolution::Missing) | None => ImageIssue::Missing,
    };
    output.unsupported_images += 1;
    output.image_diagnostics.push(ImageDiagnostic {
        element,
        source: source.to_owned(),
        issue,
    });
    None
}
