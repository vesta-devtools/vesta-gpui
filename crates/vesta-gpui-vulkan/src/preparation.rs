//! Device-free production paint preparation.
use crate::{RenderOptions, RendererError, painter};
use vesta_gpui::render::{ImageResolver, RenderOutput};
use vesta_gpui::{PaintList, TextInputContext, TextRasterizer, Viewport};

/// Owned CPU batches, before atlas packing/upload. No Vulkan resources are held.
pub struct PreparedPaint {
    pub(crate) batches: Vec<painter::Prepared>,
    pub(crate) output: RenderOutput,
}
impl PreparedPaint {
    pub fn output(&self) -> &RenderOutput {
        &self.output
    }
    /// CPU batches, not the final number of GPU draw calls.
    pub fn batch_count(&self) -> usize {
        self.batches.len()
    }
    pub fn vertex_count(&self) -> usize {
        self.batches
            .iter()
            .map(|batch| match batch {
                painter::Prepared::Shape { vertices, .. } => vertices.len(),
                painter::Prepared::Bitmap { vertices, .. }
                | painter::Prepared::Glyph { vertices, .. } => vertices.len(),
            })
            .sum()
    }
    pub(crate) fn into_parts(self) -> (Vec<painter::Prepared>, RenderOutput) {
        (self.batches, self.output)
    }
}

/// Run the exact painter used by native and offscreen frames without a Device.
/// The caller supplies fonts/images and an explicit image-dimension capability.
#[allow(clippy::too_many_arguments)]
pub fn prepare_paint(
    viewport: Viewport,
    paint: &PaintList,
    context: &mut TextInputContext,
    rasterizer: &mut TextRasterizer,
    images: Option<&dyn ImageResolver>,
    max_image_dimension: u32,
    options: RenderOptions,
) -> Result<PreparedPaint, RendererError> {
    let (batches, output) = painter::prepare(
        viewport,
        paint,
        context,
        rasterizer,
        images,
        max_image_dimension,
        options,
    )?;
    Ok(PreparedPaint { batches, output })
}
