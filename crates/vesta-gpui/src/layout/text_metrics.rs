use super::*;
use crate::{TextHAlign, TextInputContext, TextVAlign, render::static_text_view};

/// Shared measurement input. A known height requests a baseline relative to the
/// final leaf frame, including the same vertical text alignment as painting.
#[derive(Clone, Copy, Debug)]
pub struct TextLayoutRequest<'a> {
    pub text: &'a str,
    pub font_size: f32,
    pub role: FontRole,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub wrap: bool,
    pub vertical_align: TextVAlign,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextLayoutMetrics {
    pub size: Size,
    /// Real first baseline from the leaf frame's top; `None` uses Taffy's
    /// border-edge fallback and does not represent a text baseline.
    pub first_baseline: Option<f32>,
}

impl TextLayoutMetrics {
    pub(super) fn is_valid(self) -> bool {
        self.size.width.is_finite()
            && self.size.width >= 0.0
            && self.size.height.is_finite()
            && self.size.height >= 0.0
            && self.first_baseline.is_none_or(f32::is_finite)
    }

    pub(super) fn checked(self, element: ElementId) -> Result<Self, LayoutError> {
        if self.is_valid() {
            Ok(self)
        } else {
            Err(LayoutError::InvalidTextMetrics(element))
        }
    }
}

impl From<Size> for TextLayoutMetrics {
    fn from(size: Size) -> Self {
        Self {
            size,
            first_baseline: None,
        }
    }
}

pub(super) fn shaped_metrics(
    context: &mut TextInputContext,
    request: TextLayoutRequest<'_>,
) -> TextLayoutMetrics {
    let snapshot = match context.shape_static_text_wrapped(
        request.text,
        request.font_size,
        request.role,
        request.width.filter(|_| request.wrap),
    ) {
        Ok(snapshot) => snapshot,
        Err(_) => return text_intrinsic_size(request.text, request.font_size, request.role).into(),
    };
    // Auto-sized wrapped text needs enough whole logical pixels for its natural
    // width; otherwise final edge rounding can introduce a needless extra line.
    let width = if request.wrap && request.width.is_none() {
        snapshot.layout_width().ceil()
    } else {
        snapshot.layout_width()
    };
    let size = Size::new(width, snapshot.layout_height());
    let first_baseline = snapshot.first_baseline();
    if let Some(height) = request.height {
        let bounds = Rect::new(0.0, 0.0, request.width.unwrap_or(size.width), height);
        let first_baseline = static_text_view(
            context,
            bounds,
            bounds,
            request.text,
            request.font_size,
            request.role,
            request.wrap,
            TextHAlign::Start,
            request.vertical_align,
        )
        .and_then(|(snapshot, viewport)| {
            snapshot
                .first_baseline()
                .map(|baseline| viewport.origin.y + baseline)
        });
        TextLayoutMetrics {
            size,
            first_baseline,
        }
    } else {
        TextLayoutMetrics {
            size,
            first_baseline,
        }
    }
}

impl TextMeasure {
    pub(super) fn request(&self, width: Option<f32>, height: Option<f32>) -> TextLayoutRequest<'_> {
        // Taffy rounds final edges independently. A fractional measurement can
        // paint one logical pixel narrower and gain an otherwise unmeasured line.
        let width = if self.wrap {
            width.map(|width| width.floor().max(f32::EPSILON))
        } else {
            width
        };
        TextLayoutRequest {
            text: &self.text,
            font_size: self.font_size,
            role: self.role,
            width,
            height,
            wrap: self.wrap,
            vertical_align: self.vertical_align,
        }
    }
}
