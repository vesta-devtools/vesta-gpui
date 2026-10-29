//! Resource-independent image placement. Hosts retain pixels and loading policy.

use crate::{Rect, Size};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageFit {
    #[default]
    Stretch,
    Contain,
    Cover,
}

/// Physical minimum/center/maximum on either axis; RTL does not mirror images.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageAlign {
    Min,
    #[default]
    Center,
    Max,
}

impl ImageAlign {
    const fn fraction(self) -> f64 {
        match self {
            Self::Min => 0.0,
            Self::Center => 0.5,
            Self::Max => 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImagePresentation {
    pub fit: ImageFit,
    pub horizontal: ImageAlign,
    pub vertical: ImageAlign,
}

impl ImagePresentation {
    #[must_use]
    pub const fn new(fit: ImageFit) -> Self {
        Self {
            fit,
            horizontal: ImageAlign::Center,
            vertical: ImageAlign::Center,
        }
    }

    #[must_use]
    pub const fn aligned(mut self, horizontal: ImageAlign, vertical: ImageAlign) -> Self {
        self.horizontal = horizontal;
        self.vertical = vertical;
        self
    }

    /// `source` describes the original aspect, independently of downsampled texels.
    /// Invalid, empty or unrepresentable placement produces no drawable geometry.
    #[must_use]
    pub fn place(self, bounds: Rect, source: Size) -> Option<ImagePlacement> {
        if !valid_rect(bounds)
            || !source.width.is_finite()
            || !source.height.is_finite()
            || source.width <= 0.0
            || source.height <= 0.0
        {
            return None;
        }
        let [bw, bh, sw, sh] =
            [bounds.width, bounds.height, source.width, source.height].map(f64::from);
        let mut result = ImagePlacement {
            bounds,
            uv: Rect::new(0.0, 0.0, 1.0, 1.0),
        };
        match self.fit {
            ImageFit::Stretch => {}
            ImageFit::Contain => {
                let scale = (bw / sw).min(bh / sh);
                let width = (sw * scale).min(bw);
                let height = (sh * scale).min(bh);
                result.bounds = Rect::new(
                    (f64::from(bounds.x) + (bw - width) * self.horizontal.fraction()) as f32,
                    (f64::from(bounds.y) + (bh - height) * self.vertical.fraction()) as f32,
                    width as f32,
                    height as f32,
                );
            }
            ImageFit::Cover => {
                let source_ratio = sw / sh;
                let target_ratio = bw / bh;
                let width = (target_ratio / source_ratio).min(1.0);
                let height = (source_ratio / target_ratio).min(1.0);
                result.uv = Rect::new(
                    ((1.0 - width) * self.horizontal.fraction()) as f32,
                    ((1.0 - height) * self.vertical.fraction()) as f32,
                    width as f32,
                    height as f32,
                );
            }
        }
        (valid_rect(result.bounds) && valid_rect(result.uv)).then_some(result)
    }
}

/// Drawable logical rectangle and its normalized texture region.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImagePlacement {
    pub bounds: Rect,
    pub uv: Rect,
}

impl ImagePlacement {
    /// Crop geometry and UV together, preserving the original mapping.
    #[must_use]
    pub fn clipped(self, clip: Rect) -> Option<Self> {
        if !valid_rect(self.bounds) || !valid_rect(self.uv) || !valid_rect(clip) {
            return None;
        }
        let visible = self.bounds.intersection(clip)?;
        let x = f64::from(visible.x) - f64::from(self.bounds.x);
        let y = f64::from(visible.y) - f64::from(self.bounds.y);
        let sx = f64::from(self.uv.width) / f64::from(self.bounds.width);
        let sy = f64::from(self.uv.height) / f64::from(self.bounds.height);
        let result = Self {
            bounds: visible,
            uv: Rect::new(
                (f64::from(self.uv.x) + x * sx) as f32,
                (f64::from(self.uv.y) + y * sy) as f32,
                (f64::from(visible.width) * sx) as f32,
                (f64::from(visible.height) * sy) as f32,
            ),
        };
        valid_rect(result.uv).then_some(result)
    }

    #[must_use]
    pub fn texture_uv(self) -> [[f32; 2]; 4] {
        [
            [self.uv.x, self.uv.y],
            [self.uv.right(), self.uv.y],
            [self.uv.right(), self.uv.bottom()],
            [self.uv.x, self.uv.bottom()],
        ]
    }
}

fn valid_rect(rect: Rect) -> bool {
    [
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        rect.right(),
        rect.bottom(),
    ]
    .into_iter()
    .all(f32::is_finite)
        && rect.width > 0.0
        && rect.height > 0.0
        && rect.right() > rect.x
        && rect.bottom() > rect.y
}

#[cfg(test)]
mod tests;
