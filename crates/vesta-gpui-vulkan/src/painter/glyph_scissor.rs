//! A whole run can share the viewport scissor when its original glyph quads fit
//! inside its existing physical scissor. Overflow keeps the original run intact.
use super::*;

pub(super) fn contains(clip: Scissor, rect: Rect, viewport: Viewport) -> bool {
    let scale = viewport.scale_factor;
    rect.x * scale >= clip.x as f32
        && rect.y * scale >= clip.y as f32
        && rect.right() * scale <= clip.x as f32 + clip.width as f32
        && rect.bottom() * scale <= clip.y as f32 + clip.height as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contained_glyphs_keep_the_original_scissor_coverage_at_fractional_dpi() {
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            let viewport = Viewport::new(101.0 / scale, 71.0 / scale, scale);
            let clip = render::scissor(Rect::new(9.1, 6.2, 23.3, 19.8), viewport).expect("clip");
            for (rect, expected) in [
                (Rect::new(10.0, 7.0, 10.0, 8.0), true),
                (Rect::new(8.0, 7.0, 10.0, 8.0), false),
                (Rect::new(10.0, 4.0, 10.0, 8.0), false),
                (Rect::new(10.0, 7.0, 30.0, 8.0), false),
                (Rect::new(10.0, 7.0, 10.0, 25.0), false),
            ] {
                assert_eq!(contains(clip, rect, viewport), expected);
                if !expected {
                    continue;
                }
                for py in 0..71 {
                    for px in 0..101 {
                        for sample in [0.125, 0.5, 0.875] {
                            let x = (px as f32 + sample) / scale;
                            let y = (py as f32 + sample) / scale;
                            let in_rect =
                                x >= rect.x && x < rect.right() && y >= rect.y && y < rect.bottom();
                            let in_clip = px >= clip.x
                                && px < clip.x + clip.width
                                && py >= clip.y
                                && py < clip.y + clip.height;
                            assert_eq!(in_rect, in_rect && in_clip);
                        }
                    }
                }
            }
        }
    }
}
