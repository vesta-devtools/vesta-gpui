//! Explicit display lighting data, independent of editor defaults and GPU APIs.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceLighting {
    /// Direction xyz and intensity w.
    pub key: [f32; 4],
    pub fill: [f32; 4],
    /// Hemisphere up direction xyz and intensity w.
    pub sky: [f32; 4],
    /// Additive color xyz and ambient intensity w.
    pub bias: [f32; 4],
}

impl Default for SurfaceLighting {
    fn default() -> Self {
        // Preserve the generic viewport's original single directional light.
        Self {
            key: [0.45, 0.8, 0.3, 0.72],
            fill: [0.0, 1.0, 0.0, 0.0],
            sky: [0.0, 1.0, 0.0, 0.0],
            bias: [0.0, 0.0, 0.0, 0.28],
        }
    }
}

impl SurfaceLighting {
    #[must_use]
    pub fn uniform(self) -> [[f32; 4]; 4] {
        [self.key, self.fill, self.sky, self.bias]
    }

    #[must_use]
    pub fn is_valid(self) -> bool {
        self.uniform().iter().flatten().all(|v| v.is_finite())
            && [self.key, self.fill, self.sky]
                .into_iter()
                .all(|direction| {
                    let squared = direction[..3].iter().map(|v| v * v).sum::<f32>();
                    squared.is_finite() && squared > f32::EPSILON
                })
    }
}
