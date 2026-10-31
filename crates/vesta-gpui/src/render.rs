//! CPU preparation shared by native UI render backends.
use std::sync::Arc;

mod stroke;
pub use stroke::polyline_vertices;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub trait ImageResolver {
    /// Compatibility entry point. `None` projects to `ImageResolution::Missing`.
    fn resolve(&self, source: &str) -> Option<ImageData>;

    /// Hosts own loading, retries and redraw scheduling; renderers perform no I/O.
    /// Override this to share immutable pixels rather than cloning them every frame.
    fn resolve_state(&self, source: &str) -> ImageResolution {
        self.resolve(source)
            .map(|data| ImageResolution::Ready(Arc::new(data)))
            .unwrap_or(ImageResolution::Missing)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageResolution {
    Loading,
    Ready(Arc<ImageData>),
    Missing,
    Failed { code: String, message: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageIssue {
    Loading,
    Missing,
    Failed { code: String, message: String },
    InvalidData,
    BudgetExceeded,
    DimensionExceeded,
}

impl ImageIssue {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Loading => "ui_image_loading",
            Self::Missing => "ui_image_missing",
            Self::Failed { .. } => "ui_image_failed",
            Self::InvalidData => "ui_image_invalid_data",
            Self::BudgetExceeded => "ui_image_budget_exceeded",
            Self::DimensionExceeded => "ui_image_dimension_exceeded",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageDiagnostic {
    pub element: ElementId,
    pub source: String,
    pub issue: ImageIssue,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderOutput {
    pub draw_calls: usize,
    pub unsupported_images: usize,
    /// Local image fallbacks in this completed frame, including pending loads.
    pub image_diagnostics: Vec<ImageDiagnostic>,
    pub bitmap_uploads: usize,
    pub bitmap_reuses: usize,
    pub glyph_uploads: usize,
    pub glyph_reuses: usize,
    pub text_inputs: Vec<(ElementId, TextInputViewport)>,
}

impl RenderOutput {
    pub fn record_bitmap(&mut self, reused: Option<bool>) {
        match reused {
            Some(true) => self.bitmap_reuses += 1,
            Some(false) => self.bitmap_uploads += 1,
            None => {}
        }
    }
}
use crate::{
    Color, ElementId, FontRole, Point, Rect, TextHAlign, TextInputContext, TextInputViewport,
    TextVAlign, Viewport,
};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ShapeVertex {
    pub position: [f32; 2],
    pub local: [f32; 2],
    pub size: [f32; 2],
    pub radius_border: [f32; 2],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct TextureVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scissor {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[allow(clippy::too_many_arguments)]
pub fn shape_vertices(
    rect: Rect,
    radius: f32,
    border: f32,
    rotation: f32,
    color: Color,
    viewport: Viewport,
) -> [ShapeVertex; 6] {
    // Intersecting corner arcs otherwise produce pointed ends on short controls.
    let radius = radius
        .max(0.0)
        .min(rect.width.min(rect.height).max(0.0) * 0.5);
    // Butt-joined flat fills must not both fade into the background at their seam.
    // Reuse the quad shader's full coverage; curves, strokes and rotations keep AA.
    let radius = if radius == 0.0 && border <= 0.0 && rotation == 0.0 {
        -1.0
    } else {
        radius
    };
    let center = Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5);
    let corners = [
        (Point::new(rect.x, rect.y), [0.0, 0.0]),
        (Point::new(rect.right(), rect.y), [rect.width, 0.0]),
        (
            Point::new(rect.right(), rect.bottom()),
            [rect.width, rect.height],
        ),
        (Point::new(rect.x, rect.bottom()), [0.0, rect.height]),
    ];
    let linear = linear_color(color);
    [0, 1, 2, 0, 2, 3].map(|index| {
        let (point, local) = corners[index];
        ShapeVertex {
            position: ndc(rotate(point, center, rotation), viewport),
            local,
            size: [rect.width, rect.height],
            radius_border: [radius, border.max(0.0)],
            color: linear,
        }
    })
}

/// Finite, strictly convex quads have one consistent winding in either direction.
pub fn valid_quad(points: &[Point; 4]) -> bool {
    if points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return false;
    }
    let mut sign = 0.0_f64;
    for i in 0..4 {
        let [a, b, c] = [points[i], points[(i + 1) % 4], points[(i + 2) % 4]];
        let cross = (f64::from(b.x) - f64::from(a.x)) * (f64::from(c.y) - f64::from(b.y))
            - (f64::from(b.y) - f64::from(a.y)) * (f64::from(c.x) - f64::from(b.x));
        if cross == 0.0 || (sign != 0.0 && cross.signum() != sign) {
            return false;
        }
        sign = cross.signum();
    }
    true
}
pub fn quad_vertices(points: &[Point; 4], color: Color, viewport: Viewport) -> Vec<ShapeVertex> {
    if !valid_quad(points) {
        return Vec::new();
    }
    let linear = linear_color(color);
    [0, 1, 2, 0, 2, 3]
        .map(|index| ShapeVertex {
            position: ndc(points[index], viewport),
            local: [0.0, 0.0],
            size: [0.0, 0.0],
            radius_border: [-1.0, 0.0],
            color: linear,
        })
        .to_vec()
}

#[allow(clippy::too_many_arguments)]
pub fn static_text_view(
    context: &mut TextInputContext,
    bounds: Rect,
    clip: Rect,
    text: &str,
    font_size: f32,
    font_role: FontRole,
    wrap: bool,
    h_align: TextHAlign,
    v_align: TextVAlign,
) -> Option<(crate::TextInputSnapshot, TextInputViewport)> {
    let snapshot = context
        .shape_static_text_wrapped(text, font_size, font_role, wrap.then_some(bounds.width))
        .ok()?;
    let intrinsic_width = snapshot.layout_width();
    let intrinsic_height = snapshot.layout_height();
    let x = match h_align {
        TextHAlign::Start => bounds.x,
        TextHAlign::Center => bounds.x + (bounds.width - intrinsic_width).max(0.0) * 0.5,
        TextHAlign::End => bounds.right() - intrinsic_width.min(bounds.width),
    };
    let y = match v_align {
        TextVAlign::Start => bounds.y,
        TextVAlign::Center => bounds.y + (bounds.height - intrinsic_height).max(0.0) * 0.5,
        TextVAlign::Baseline => {
            let centered = bounds.y + (bounds.height - intrinsic_height).max(0.0) * 0.5;
            snapshot
                .single_line_baseline()
                .map_or(centered, |baseline| {
                    context
                        .shape_static_text("Hg", font_size, font_role)
                        .ok()
                        .and_then(|reference| {
                            reference.single_line_baseline().map(|reference_baseline| {
                                bounds.y
                                    + (bounds.height - reference.layout_height()).max(0.0) * 0.5
                                    + reference_baseline
                                    - baseline
                            })
                        })
                        .unwrap_or(centered)
                })
        }
        TextVAlign::End => bounds.bottom() - intrinsic_height.min(bounds.height),
    };
    Some((
        snapshot,
        TextInputViewport {
            bounds,
            clip: bounds.intersection(clip).unwrap_or_default(),
            scroll_x: 0.0,
            scroll_y: 0.0,
            origin: Point::new(x, y),
            caret: None,
        },
    ))
}

pub fn texture_vertices(rect: Rect, viewport: Viewport) -> [TextureVertex; 6] {
    texture_vertices_uv(
        rect,
        viewport,
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
    )
}

pub fn texture_vertices_uv(
    rect: Rect,
    viewport: Viewport,
    uv: [[f32; 2]; 4],
) -> [TextureVertex; 6] {
    let corners = [
        (Point::new(rect.x, rect.y), uv[0]),
        (Point::new(rect.right(), rect.y), uv[1]),
        (Point::new(rect.right(), rect.bottom()), uv[2]),
        (Point::new(rect.x, rect.bottom()), uv[3]),
    ];
    [0, 1, 2, 0, 2, 3].map(|index| TextureVertex {
        position: ndc(corners[index].0, viewport),
        uv: corners[index].1,
    })
}

pub fn ndc(point: Point, viewport: Viewport) -> [f32; 2] {
    [
        point.x / viewport.width * 2.0 - 1.0,
        1.0 - point.y / viewport.height * 2.0,
    ]
}

pub fn rotate(point: Point, center: Point, radians: f32) -> Point {
    if radians == 0.0 {
        return point;
    }
    let (sin, cos) = radians.sin_cos();
    let dx = point.x - center.x;
    let dy = point.y - center.y;
    Point::new(
        center.x + dx * cos - dy * sin,
        center.y + dx * sin + dy * cos,
    )
}

pub fn scissor(rect: Rect, viewport: Viewport) -> Option<Scissor> {
    let visible = rect.intersection(viewport.logical_rect())?;
    let scale = viewport.scale_factor;
    // A physical window size round-tripped through a fractional logical DPI can be
    // slightly below the original integer (for example 31 -> 30.999998 at 175%).
    let physical_width = (viewport.width * scale).round() as u32;
    let physical_height = (viewport.height * scale).round() as u32;
    let x = (visible.x * scale).floor().max(0.0) as u32;
    let y = (visible.y * scale).floor().max(0.0) as u32;
    let right = (visible.right() * scale).ceil().min(physical_width as f32) as u32;
    let bottom = (visible.bottom() * scale)
        .ceil()
        .min(physical_height as f32) as u32;
    if right <= x || bottom <= y {
        return None;
    }
    Some(Scissor {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}

pub fn linear_color(color: Color) -> [f32; 4] {
    [
        srgb_to_linear(color.red),
        srgb_to_linear(color.green),
        srgb_to_linear(color.blue),
        color.alpha.clamp(0.0, 1.0),
    ]
}

pub fn srgb_to_linear(channel: f32) -> f32 {
    let channel = channel.clamp(0.0, 1.0);
    if channel <= 0.040_45 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

pub const SHAPE_SHADER: &str = include_str!("render/shape.wgsl");
pub const TEXTURE_SHADER: &str = include_str!("render/texture.wgsl");

#[cfg(test)]
mod tests {
    use super::*;

    fn coverage(vertices: &[ShapeVertex], point: Point, viewport: Viewport) -> usize {
        let p = ndc(point, viewport).map(f64::from);
        vertices
            .chunks_exact(3)
            .filter(|tri| {
                let v = [tri[0], tri[1], tri[2]].map(|v| v.position.map(f64::from));
                let mut signs = [0.0; 3];
                for i in 0..3 {
                    let a = v[i];
                    let b = v[(i + 1) % 3];
                    signs[i] = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
                }
                signs.iter().all(|v| *v > 1e-10) || signs.iter().all(|v| *v < -1e-10)
            })
            .count()
    }

    #[test]
    fn connected_strokes_cover_bends_and_keep_prepared_vertices_finite() {
        let viewport = Viewport::new(100.0, 100.0, 1.25);
        let color = Color::srgba(0.2, 0.3, 0.4, 0.5);
        let path = [
            Point::new(10.0, 10.0),
            Point::new(30.0, 10.0),
            Point::new(30.0, 30.0),
        ];
        let mesh = polyline_vertices(&path, 8.0, color, viewport);
        assert_eq!(
            coverage(&mesh, Point::new(31.0, 8.5), viewport),
            1,
            "ordinary stroke bend must cover its outer bevel"
        );
        assert_eq!(
            coverage(&mesh, Point::new(28.0, 12.0), viewport),
            1,
            "ordinary stroke bend must not double-blend its inner join"
        );
        for angle in [5.0_f64, 30.0, 60.0, 90.0, 120.0, 165.0] {
            let (sin, cos) = angle.to_radians().sin_cos();
            let points = [
                Point::new(10.0, 50.0),
                Point::new(50.0, 50.0),
                Point::new((50.0 + 40.0 * cos) as f32, (50.0 + 40.0 * sin) as f32),
            ];
            let mesh = polyline_vertices(&points, 8.0, color, viewport);
            let outer = Point::new(
                (50.0 + 4.0 * sin / 3.0) as f32,
                (50.0 - 4.0 * (1.0 + cos) / 3.0) as f32,
            );
            let inner = Point::new(
                (50.0 - 0.37 * 4.0 * sin / (1.0 + cos)) as f32,
                (50.0 + 0.37 * 4.0) as f32,
            );
            assert_eq!(
                coverage(&mesh, outer, viewport),
                1,
                "outer bevel at {angle} degrees"
            );
            assert_eq!(
                coverage(&mesh, inner, viewport),
                1,
                "inner overlap at {angle} degrees"
            );
        }
        for reversed in [false, true] {
            for mirrored in [false, true] {
                let mut points = path.to_vec();
                if mirrored {
                    for p in &mut points {
                        p.y = 40.0 - p.y;
                    }
                }
                if reversed {
                    points.reverse();
                }
                points.insert(1, points[1]);
                let mesh = polyline_vertices(&points, 8.0, color, viewport);
                let gap = Point::new(31.0, if mirrored { 31.5 } else { 8.5 });
                let inner = Point::new(28.0, if mirrored { 28.0 } else { 12.0 });
                assert_eq!(coverage(&mesh, gap, viewport), 1);
                assert_eq!(coverage(&mesh, inner, viewport), 1);
                assert!(
                    mesh.iter()
                        .all(|v| v.position.iter().all(|x| x.is_finite()))
                );
            }
        }
        let closed = [path[0], path[1], path[2], Point::new(10.0, 30.0), path[0]];
        assert_eq!(
            coverage(
                &polyline_vertices(&closed, 8.0, color, viewport),
                Point::new(8.5, 9.0),
                viewport
            ),
            1
        );
        let large = polyline_vertices(
            &[
                Point::new(-f32::MAX / 4.0, 50.0),
                Point::new(f32::MAX / 4.0, 50.0),
            ],
            8.0,
            color,
            viewport,
        );
        assert!(
            large.iter().any(|v| v.position[1].abs() > 0.01),
            "finite long strokes must retain width"
        );
        for points in [
            vec![
                Point::new(10.0, 10.0),
                Point::new(30.0, 10.0),
                Point::new(11.0, 10.001),
            ],
            vec![path[0], path[1], path[0]],
        ] {
            let mesh = polyline_vertices(&points, 8.0, color, viewport);
            assert!(!mesh.is_empty());
            assert!(mesh.len() <= points.len() * 9);
            assert!(
                mesh.iter()
                    .all(|v| v.position.iter().all(|x| x.is_finite() && x.abs() <= 1.1))
            );
        }
        for width in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(polyline_vertices(&path, width, color, viewport).is_empty());
        }
        assert!(
            polyline_vertices(&[path[0], Point::new(f32::NAN, 1.0)], 2.0, color, viewport)
                .is_empty()
        );
        assert!(polyline_vertices(&path, 2.0, color, Viewport::new(0.0, 100.0, 1.0)).is_empty());
        assert!(
            polyline_vertices(
                &path,
                2.0,
                color,
                Viewport::new(f32::MIN_POSITIVE, 100.0, 1.0)
            )
            .is_empty()
        );
    }

    #[test]
    fn convex_quad_projection_rejects_invalid_geometry_and_keeps_winding() {
        let points = [
            Point::new(0.0, 0.0),
            Point::new(20.0, 2.0),
            Point::new(16.0, 19.0),
            Point::new(2.0, 17.0),
        ];
        let viewport = Viewport::new(100.0, 100.0, 1.25);
        let color = Color::rgb8(100, 120, 140);
        for quad in [points, [points[3], points[2], points[1], points[0]]] {
            assert!(valid_quad(&quad));
            let vertices = quad_vertices(&quad, color, viewport);
            assert_eq!(vertices.len(), 6);
            for (v, index) in vertices.iter().zip([0, 1, 2, 0, 2, 3]) {
                assert_eq!(v.position, ndc(quad[index], viewport));
            }
        }
        for quad in [
            [points[0], points[2], points[1], points[3]],
            [points[0]; 4],
            [points[0], points[1], Point::new(f32::NAN, 1.0), points[3]],
            [points[0], points[1], Point::new(3.0, 3.0), points[3]],
        ] {
            assert!(!valid_quad(&quad));
            assert!(quad_vertices(&quad, color, viewport).is_empty());
        }
    }

    #[test]
    fn rounded_shape_vertices_bound_capsule_radii_for_fills_and_strokes() {
        for scale in [1.0, 1.25, 1.5] {
            let viewport = Viewport::new(200.0, 200.0, scale);
            for (width, height, requested, expected) in [
                (120.0, 28.0, 20.0, 14.0),
                (28.0, 120.0, 20.0, 14.0),
                (28.0, 28.0, 20.0, 14.0),
                (8.0, 28.0, 20.0, 4.0),
                (120.0, 3.0, 20.0, 1.5),
                (120.0, 28.0, 6.0, 6.0),
                (120.0, 28.0, -1.0, 0.0),
                (120.0, 0.0, 20.0, 0.0),
            ] {
                for border in [0.0, 1.5] {
                    for rotation in [0.0, 0.5] {
                        let vertices = shape_vertices(
                            Rect::new(10.2, 20.4, width, height),
                            requested,
                            border,
                            rotation,
                            Color::rgb8(30, 40, 50),
                            viewport,
                        );
                        let expected = if expected == 0.0 && border == 0.0 && rotation == 0.0 {
                            -1.0
                        } else {
                            expected
                        };
                        for vertex in vertices {
                            assert_eq!(
                                vertex.radius_border,
                                [expected, border],
                                "radius must fit {width} x {height} after layout, for fill and stroke"
                            );
                            assert_eq!(vertex.size, [width, height]);
                        }
                    }
                }
            }
            let rect = Rect::new(10.2, 20.4, 120.0, 28.0);
            let color = Color::srgba(0.2, 0.3, 0.4, 0.72);
            let fill = shape_vertices(rect, 0.0, 0.0, 0.0, color, viewport);
            let quad = quad_vertices(
                &[
                    Point::new(rect.x, rect.y),
                    Point::new(rect.right(), rect.y),
                    Point::new(rect.right(), rect.bottom()),
                    Point::new(rect.x, rect.bottom()),
                ],
                color,
                viewport,
            );
            for (fill, quad) in fill.iter().zip(quad) {
                assert_eq!(fill.position, quad.position);
                assert_eq!(fill.color, quad.color);
                assert_eq!(
                    fill.radius_border, quad.radius_border,
                    "abutting flat fills must use the same full coverage as existing quads"
                );
            }
        }
    }
}
