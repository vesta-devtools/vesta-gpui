//! Bounded immutable glyph pages; GPU textures are replaced only after misses.
use std::{collections::HashMap, sync::Arc};
use vesta_gpui::{Color, GlyphKey, RasterGlyph};
use vulkano::descriptor_set::DescriptorSet;

pub(super) const SIDE: u32 = 1024;
const MAX_PAGES: usize = 4;
const MAX_ENTRIES: usize = 4096;

#[derive(Clone, Copy)]
pub(super) struct Placement {
    pub page: usize,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub reused: bool,
}
#[derive(PartialEq, Eq, Hash)]
struct AtlasKey {
    glyph: GlyphKey,
    color: [u32; 4],
}
impl AtlasKey {
    fn new(glyph: &RasterGlyph, color: Color) -> Self {
        Self {
            glyph: glyph.key.clone(),
            color: [
                color.red.to_bits(),
                color.green.to_bits(),
                color.blue.to_bits(),
                color.alpha.to_bits(),
            ],
        }
    }
}
pub(super) struct Page {
    pub rgba: Vec<u8>,
    pub set: Option<Arc<DescriptorSet>>,
    pub dirty: bool,
    next_x: u32,
    next_y: u32,
    row_height: u32,
}
impl Page {
    fn new() -> Self {
        Self {
            rgba: vec![0; (SIDE * SIDE * 4) as usize],
            set: None,
            dirty: true,
            next_x: 0,
            next_y: 0,
            row_height: 0,
        }
    }
    fn reserve(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        if width > SIDE || height > SIDE {
            return None;
        }
        if self.next_x + width > SIDE {
            self.next_x = 0;
            self.next_y += self.row_height;
            self.row_height = 0;
        }
        if self.next_y + height > SIDE {
            return None;
        }
        let at = (self.next_x, self.next_y);
        self.next_x += width;
        self.row_height = self.row_height.max(height);
        Some(at)
    }
}
#[derive(Default)]
pub(super) struct GlyphAtlas {
    pub pages: Vec<Page>,
    entries: HashMap<AtlasKey, Placement>,
}
impl GlyphAtlas {
    pub fn clear(&mut self) {
        self.pages.clear();
        self.entries.clear();
    }
    pub fn begin_frame(&mut self) {
        // Recycle only between frames: placements already emitted in this frame
        // never change. Old GPU descriptors remain owned by submitted commands.
        if self.entries.len() >= MAX_ENTRIES {
            self.clear();
        }
    }
    pub fn get_or_insert(&mut self, glyph: &RasterGlyph, color: Color) -> Option<Placement> {
        if glyph.width == 0
            || glyph.height == 0
            || glyph.width > SIDE - 2
            || glyph.height > SIDE - 2
        {
            return None;
        }
        let key = AtlasKey::new(glyph, color);
        if let Some(placement) = self.entries.get(&key) {
            return Some(Placement {
                reused: true,
                ..*placement
            });
        }
        if self.entries.len() >= MAX_ENTRIES {
            return None;
        }
        let width = glyph.width + 2;
        let height = glyph.height + 2;
        let mut reserved = self
            .pages
            .last_mut()
            .and_then(|page| page.reserve(width, height));
        if reserved.is_none() && self.pages.len() < MAX_PAGES {
            self.pages.push(Page::new());
            reserved = self
                .pages
                .last_mut()
                .and_then(|page| page.reserve(width, height));
        }
        let (x, y) = reserved?;
        let page_index = self.pages.len() - 1;
        let page = &mut self.pages[page_index];
        let rgba = glyph_pixels(glyph, color);
        for (row, pixels) in rgba.chunks_exact(width as usize * 4).enumerate() {
            let offset = (((y as usize + row) * SIDE as usize) + x as usize) * 4;
            page.rgba[offset..offset + pixels.len()].copy_from_slice(pixels);
        }
        page.dirty = true;
        let placement = Placement {
            page: page_index,
            x,
            y,
            width,
            height,
            reused: false,
        };
        self.entries.insert(key, placement);
        Some(placement)
    }
}

/// The same straight-alpha pixels and transparent gutter used by individual textures.
pub(super) fn glyph_pixels(glyph: &RasterGlyph, color: Color) -> Vec<u8> {
    let width = glyph.width + 2;
    let mut rgba = vec![0; width as usize * (glyph.height + 2) as usize * 4];
    for (row, pixels) in glyph
        .rgba(color)
        .chunks_exact(glyph.width as usize * 4)
        .enumerate()
    {
        let offset = ((row + 1) * width as usize + 1) * 4;
        rgba[offset..offset + pixels.len()].copy_from_slice(pixels);
    }
    rgba
}

#[cfg(test)]
mod tests {
    #[test]
    fn page_placements_keep_gutters_and_pixels_without_rewriting_existing_entries() {
        use vesta_gpui::{
            Color, FontRole, Point, Rect, TextInputContext, TextInputViewport, TextRasterizer,
        };
        const WHITE: Color = Color::srgba(1.0, 1.0, 1.0, 1.0);
        const BLACK: Color = Color::srgba(0.0, 0.0, 0.0, 1.0);
        let mut context = TextInputContext::default();
        assert!(context.register_font(std::sync::Arc::new(
            include_bytes!("../../vesta-gpui/tests/fixtures/MaterialIcons-Regular.ttf").to_vec()
        )));
        context.set_font_stack(FontRole::Icon, "Material Icons");
        let snapshot = context
            .shape_static_text("\u{e5d2}", 13.0, FontRole::Icon)
            .expect("text");
        let bounds = Rect::new(0.0, 0.0, 64.0, 32.0);
        let view = TextInputViewport {
            bounds,
            clip: bounds,
            scroll_x: 0.0,
            scroll_y: 0.0,
            origin: Point::new(0.0, 0.0),
            caret: None,
        };
        let mut rasterizer = TextRasterizer::default();
        let mut glyphs = Vec::new();
        rasterizer
            .visit_glyph_refs(&snapshot, view, 1.0, |glyph| glyphs.push(glyph.clone()))
            .expect("raster");
        let glyph = &glyphs[0];
        let mut atlas = super::GlyphAtlas::default();
        let first_pixels = super::glyph_pixels(glyph, WHITE);
        let first = atlas.get_or_insert(glyph, WHITE).expect("first glyph");
        let second = atlas.get_or_insert(glyph, BLACK).expect("other color");
        assert_eq!((first.x, second.x), (0, glyph.width + 2));
        atlas.pages[0].dirty = false;
        let reused = atlas.get_or_insert(glyph, WHITE).expect("same glyph");
        assert!(reused.reused);
        assert!(
            !atlas.pages[0].dirty,
            "cache hits must not rewrite texture pixels"
        );
        assert_eq!(
            (reused.page, reused.x, reused.y),
            (first.page, first.x, first.y)
        );
        for (row, pixels) in first_pixels
            .chunks_exact(first.width as usize * 4)
            .enumerate()
        {
            let offset = row * super::SIDE as usize * 4;
            assert_eq!(&atlas.pages[0].rgba[offset..offset + pixels.len()], pixels);
        }
        let mut oversized = glyph.clone();
        oversized.width = super::SIDE;
        assert!(atlas.get_or_insert(&oversized, WHITE).is_none());
        // The same font glyph with another color must keep separate placements.
        assert_ne!(first.x, second.x);
        let translucent = atlas
            .get_or_insert(glyph, Color::srgba(1.0, 1.0, 1.0, 0.5))
            .expect("opacity");
        assert_ne!(first.x, translucent.x);
        rasterizer
            .visit_glyph_refs(&snapshot, view, 2.0, |scaled| {
                let at = atlas.get_or_insert(scaled, WHITE).expect("different DPI");
                assert!(!at.reused);
            })
            .expect("scaled raster");
        for index in atlas.entries.len()..super::MAX_ENTRIES {
            atlas
                .get_or_insert(
                    glyph,
                    Color::srgba(index as f32 / super::MAX_ENTRIES as f32, 0.25, 0.75, 1.0),
                )
                .expect("bounded entry");
        }
        assert!(
            atlas
                .get_or_insert(glyph, Color::srgba(0.5, 0.5, 0.5, 1.0))
                .is_none()
        );
        atlas.begin_frame();
        assert!(atlas.pages.is_empty());
        assert!(atlas.get_or_insert(glyph, WHITE).is_some());
    }
}
