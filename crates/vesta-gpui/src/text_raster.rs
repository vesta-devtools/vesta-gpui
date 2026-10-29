//! Bounded CPU glyph rasterization of the exact Parley layout used for hit testing.
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use parley::{
    PositionedLayoutItem,
    swash::{
        FontRef,
        scale::{
            Render, ScaleContext, Source, StrikeWith,
            image::{Content, Image},
        },
        zeno::{Format, Vector},
    },
};

use crate::{Color, Rect, TextInputSnapshot, TextInputViewport, text_input::CARET_VISUAL_GAP};

#[derive(Clone, Copy)]
pub struct TextInputColors {
    pub text: Color,
    pub selection: Color,
    pub caret: Color,
}

#[derive(Clone)]
pub struct TextInputBitmap {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRasterError {
    InvalidViewport,
}

const GLYPH_CACHE_ENTRIES: usize = 4096;
const GLYPH_CACHE_BYTES: usize = 8 * 1024 * 1024;
const GLYPH_RUN_CACHE_ENTRIES: usize = 256;
const GLYPH_RUN_CACHE_GLYPHS: usize = 8192;
const GLYPH_RUN_CACHE_BYTES: usize = 8 * 1024 * 1024;
const BITMAP_CACHE_ENTRIES: usize = 64;
const BITMAP_CACHE_BYTES: usize = 16 * 1024 * 1024;
const MAX_BITMAP_PIXELS: f32 = 4_194_304.0;

#[derive(Clone, PartialEq, Eq, Hash)]
struct BitmapKey {
    snapshot: usize,
    read_only_selection: Option<[usize; 2]>,
    bounds: [u32; 4],
    clip: [u32; 4],
    origin: [u32; 2],
    scale: u32,
    focused: bool,
    caret_visible: bool,
    text: [u8; 4],
    selection: [u8; 4],
    caret: [u8; 4],
}

impl BitmapKey {
    fn new(
        snapshot: &TextInputSnapshot,
        view: TextInputViewport,
        scale: f32,
        focused: bool,
        caret_visible: bool,
        colors: TextInputColors,
    ) -> Self {
        let rect_bits = |rect: Rect| {
            [
                rect.x.to_bits(),
                rect.y.to_bits(),
                rect.width.to_bits(),
                rect.height.to_bits(),
            ]
        };
        Self {
            snapshot: Arc::as_ptr(&snapshot.0) as usize,
            read_only_selection: snapshot.selection_range(),
            bounds: rect_bits(view.bounds),
            clip: rect_bits(view.clip),
            origin: [view.origin.x.to_bits(), view.origin.y.to_bits()],
            scale: scale.to_bits(),
            focused,
            caret_visible,
            text: rgba(colors.text),
            selection: rgba(colors.selection),
            caret: rgba(colors.caret),
        }
    }
}

struct CachedBitmap {
    key: BitmapKey,
    _snapshot: TextInputSnapshot,
    bitmap: Arc<TextInputBitmap>,
}

#[derive(Default)]
struct BitmapCache {
    entries: VecDeque<CachedBitmap>,
    bytes: usize,
}

impl BitmapCache {
    fn find(&mut self, key: &BitmapKey) -> Option<Arc<TextInputBitmap>> {
        let index = self.entries.iter().position(|entry| &entry.key == key)?;
        let entry = self.entries.remove(index)?;
        let bitmap = entry.bitmap.clone();
        self.entries.push_back(entry);
        Some(bitmap)
    }

    fn insert(
        &mut self,
        key: BitmapKey,
        snapshot: TextInputSnapshot,
        bitmap: Arc<TextInputBitmap>,
    ) {
        let bytes = bitmap.rgba.len();
        if bytes > BITMAP_CACHE_BYTES {
            return;
        }
        while self.entries.len() >= BITMAP_CACHE_ENTRIES || self.bytes + bytes > BITMAP_CACHE_BYTES
        {
            if let Some(old) = self.entries.pop_front() {
                self.bytes -= old.bitmap.rgba.len();
            }
        }
        self.bytes += bytes;
        self.entries.push_back(CachedBitmap {
            key,
            _snapshot: snapshot,
            bitmap,
        });
    }
}

/// Content identity of one scaled and positioned font glyph.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    font_blob: u64,
    font_index: u32,
    font_size: u32,
    coords: Vec<i16>,
    glyph: u16,
    subpixel_x: u32,
    subpixel_y: u32,
}

/// A glyph positioned in physical pixels relative to a text bitmap's bounds.
#[derive(Clone)]
pub struct RasterGlyph {
    pub key: GlyphKey,
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    image: Arc<Image>,
}

impl RasterGlyph {
    /// Straight-alpha RGBA, preserving color emoji and applying text opacity.
    #[must_use]
    pub fn rgba(&self, color: Color) -> Vec<u8> {
        let foreground = rgba(color);
        (0..self.width as usize * self.height as usize)
            .flat_map(|at| glyph_pixel(self.image.content, &self.image.data, at, foreground))
            .collect()
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct GlyphRunKey {
    snapshot: usize,
    bounds_size: [u32; 2],
    clip: [u32; 4],
    origin: [u32; 2],
    scale: u32,
}

impl GlyphRunKey {
    fn new(snapshot: &TextInputSnapshot, view: TextInputViewport, scale: f32) -> Self {
        Self {
            snapshot: Arc::as_ptr(&snapshot.0) as usize,
            bounds_size: [view.bounds.width.to_bits(), view.bounds.height.to_bits()],
            clip: [
                (view.clip.x - view.bounds.x).to_bits(),
                (view.clip.y - view.bounds.y).to_bits(),
                view.clip.width.to_bits(),
                view.clip.height.to_bits(),
            ],
            origin: [
                (view.origin.x - view.bounds.x).to_bits(),
                (view.origin.y - view.bounds.y).to_bits(),
            ],
            scale: scale.to_bits(),
        }
    }
}

struct CachedGlyphRun {
    key: GlyphRunKey,
    _snapshot: TextInputSnapshot,
    glyphs: Arc<Vec<RasterGlyph>>,
    bytes: usize,
}

#[derive(Default)]
struct GlyphRunCache {
    entries: VecDeque<CachedGlyphRun>,
    glyph_count: usize,
    bytes: usize,
}

impl GlyphRunCache {
    fn find(&mut self, key: &GlyphRunKey) -> Option<Arc<Vec<RasterGlyph>>> {
        let index = self.entries.iter().position(|entry| &entry.key == key)?;
        let entry = self.entries.remove(index)?;
        let glyphs = entry.glyphs.clone();
        self.entries.push_back(entry);
        Some(glyphs)
    }

    fn insert(
        &mut self,
        key: GlyphRunKey,
        snapshot: TextInputSnapshot,
        glyphs: Vec<RasterGlyph>,
    ) -> Arc<Vec<RasterGlyph>> {
        let bytes = glyphs.iter().fold(0usize, |total, glyph| {
            total.saturating_add(
                std::mem::size_of::<RasterGlyph>()
                    + glyph.key.coords.len() * std::mem::size_of::<i16>()
                    + glyph.image.data.len(),
            )
        });
        let glyphs = Arc::new(glyphs);
        if glyphs.len() > GLYPH_RUN_CACHE_GLYPHS || bytes > GLYPH_RUN_CACHE_BYTES {
            return glyphs;
        }
        while self.entries.len() >= GLYPH_RUN_CACHE_ENTRIES
            || self.glyph_count + glyphs.len() > GLYPH_RUN_CACHE_GLYPHS
            || self.bytes + bytes > GLYPH_RUN_CACHE_BYTES
        {
            let Some(old) = self.entries.pop_front() else {
                break;
            };
            self.glyph_count -= old.glyphs.len();
            self.bytes -= old.bytes;
        }
        self.glyph_count += glyphs.len();
        self.bytes += bytes;
        self.entries.push_back(CachedGlyphRun {
            key,
            _snapshot: snapshot,
            glyphs: glyphs.clone(),
            bytes,
        });
        glyphs
    }
}

#[derive(Default)]
struct GlyphCache {
    images: HashMap<GlyphKey, Arc<Image>>,
    order: VecDeque<GlyphKey>,
    bytes: usize,
}

impl GlyphCache {
    fn get_or_render(
        &mut self,
        key: GlyphKey,
        render: impl FnOnce() -> Option<Image>,
    ) -> Option<Arc<Image>> {
        if let Some(image) = self.images.get(&key) {
            return Some(image.clone());
        }
        let image = Arc::new(render()?);
        if image.data.len() <= GLYPH_CACHE_BYTES {
            while self.order.len() >= GLYPH_CACHE_ENTRIES
                || self.bytes + image.data.len() > GLYPH_CACHE_BYTES
            {
                let Some(old) = self.order.pop_front() else {
                    break;
                };
                if let Some(image) = self.images.remove(&old) {
                    self.bytes -= image.data.len();
                }
            }
            self.bytes += image.data.len();
            self.order.push_back(key.clone());
            self.images.insert(key, image.clone());
        }
        Some(image)
    }
}

#[derive(Default)]
pub struct TextRasterizer {
    scale: ScaleContext,
    glyphs: GlyphCache,
    glyph_runs: GlyphRunCache,
    bitmaps: BitmapCache,
}

impl TextRasterizer {
    pub fn render(
        &mut self,
        snapshot: &TextInputSnapshot,
        view: TextInputViewport,
        scale: f32,
        focused: bool,
        caret_visible: bool,
        colors: TextInputColors,
    ) -> Result<TextInputBitmap, TextRasterError> {
        self.render_shared(snapshot, view, scale, focused, caret_visible, colors)
            .map(|bitmap| bitmap.as_ref().clone())
    }

    /// Share a bounded composed bitmap across unchanged paint frames.
    pub fn render_shared(
        &mut self,
        snapshot: &TextInputSnapshot,
        view: TextInputViewport,
        scale: f32,
        focused: bool,
        caret_visible: bool,
        colors: TextInputColors,
    ) -> Result<Arc<TextInputBitmap>, TextRasterError> {
        let key = BitmapKey::new(snapshot, view, scale, focused, caret_visible, colors);
        if let Some(bitmap) = self.bitmaps.find(&key) {
            return Ok(bitmap);
        }
        let bitmap = Arc::new(self.render_uncached(
            snapshot,
            view,
            scale,
            focused,
            caret_visible,
            colors,
        )?);
        self.bitmaps.insert(key, snapshot.clone(), bitmap.clone());
        Ok(bitmap)
    }

    fn render_uncached(
        &mut self,
        snapshot: &TextInputSnapshot,
        view: TextInputViewport,
        scale: f32,
        focused: bool,
        caret_visible: bool,
        colors: TextInputColors,
    ) -> Result<TextInputBitmap, TextRasterError> {
        if !scale.is_finite()
            || scale <= 0.0
            || !view.bounds.width.is_finite()
            || !view.bounds.height.is_finite()
        {
            return Err(TextRasterError::InvalidViewport);
        }
        let width = (view.bounds.width * scale).ceil();
        let height = (view.bounds.height * scale).ceil();
        if !(1.0..=8192.0).contains(&width)
            || !(1.0..=8192.0).contains(&height)
            || width * height > MAX_BITMAP_PIXELS
        {
            return Err(TextRasterError::InvalidViewport);
        }
        let mut image = TextInputBitmap {
            width: width as u32,
            height: height as u32,
            rgba: vec![0; width as usize * height as usize * 4],
        };
        let transform = |r: Rect| {
            Rect::new(
                (view.origin.x - view.bounds.x + r.x) * scale,
                (view.origin.y - view.bounds.y + r.y) * scale,
                r.width * scale,
                r.height * scale,
            )
        };
        let clip = Rect::new(
            (view.clip.x - view.bounds.x) * scale,
            (view.clip.y - view.bounds.y) * scale,
            view.clip.width * scale,
            view.clip.height * scale,
        );
        for r in snapshot.selection_rectangles() {
            let mut color = colors.selection;
            if !focused {
                color.alpha *= 0.45;
            }
            fill(&mut image, transform(*r), clip, rgba(color));
        }
        let foreground = rgba(colors.text);
        self.visit_glyphs(snapshot, view, scale, |glyph| {
            for y in 0..glyph.height {
                for x in 0..glyph.width {
                    let at = (y * glyph.width + x) as usize;
                    let color = glyph_pixel(glyph.image.content, &glyph.image.data, at, foreground);
                    blend(
                        &mut image,
                        glyph.left + x as i32,
                        glyph.top + y as i32,
                        clip,
                        color,
                    );
                }
            }
        })?;
        if focused {
            for r in &snapshot.0.compose {
                let mut underline = transform(*r);
                underline.y = underline.bottom() - scale;
                underline.height = scale.max(1.0);
                fill(&mut image, underline, clip, foreground);
            }
            if caret_visible && let Some(caret) = snapshot.0.caret {
                let mut caret = transform(caret);
                caret.x += CARET_VISUAL_GAP * scale;
                fill(&mut image, caret, clip, rgba(colors.caret));
            }
        }
        Ok(image)
    }

    /// Visit exactly the glyph images and positions used by bitmap rendering.
    pub fn visit_glyphs(
        &mut self,
        snapshot: &TextInputSnapshot,
        view: TextInputViewport,
        scale: f32,
        mut visit: impl FnMut(RasterGlyph),
    ) -> Result<(), TextRasterError> {
        self.visit_glyph_refs(snapshot, view, scale, |glyph| visit(glyph.clone()))
    }

    /// Borrow cached glyphs without cloning their keys and image handles.
    pub fn visit_glyph_refs(
        &mut self,
        snapshot: &TextInputSnapshot,
        view: TextInputViewport,
        scale: f32,
        mut visit: impl FnMut(&RasterGlyph),
    ) -> Result<(), TextRasterError> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err(TextRasterError::InvalidViewport);
        }
        let key = GlyphRunKey::new(snapshot, view, scale);
        if let Some(glyphs) = self.glyph_runs.find(&key) {
            for glyph in glyphs.iter() {
                visit(glyph);
            }
            return Ok(());
        }
        let mut glyphs = Vec::new();
        let clip = Rect::new(
            (view.clip.x - view.bounds.x) * scale,
            (view.clip.y - view.bounds.y) * scale,
            view.clip.width * scale,
            view.clip.height * scale,
        );
        for line in snapshot.0.layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                let font = run.font();
                let Some(font_ref) = FontRef::from_index(font.data.as_ref(), font.index as usize)
                else {
                    continue;
                };
                let mut scaler = self
                    .scale
                    .builder(font_ref)
                    .size(run.font_size() * scale)
                    .normalized_coords(run.normalized_coords().iter().copied())
                    .hint(true)
                    .build();
                let mut x = glyph_run.offset();
                for glyph in glyph_run.glyphs() {
                    let px = (view.origin.x - view.bounds.x + x + glyph.x) * scale;
                    let py =
                        (view.origin.y - view.bounds.y + glyph_run.baseline() - glyph.y) * scale;
                    x += glyph.advance;
                    // Skip clipped glyphs before raster work, keeping long single lines bounded.
                    if py + run.font_size() * scale * 3.0 < clip.y
                        || py - run.font_size() * scale * 3.0 > clip.bottom()
                        || px + run.font_size() * scale * 3.0 < clip.x
                        || px - run.font_size() * scale * 2.0 > clip.right()
                    {
                        continue;
                    }
                    let Ok(id) = u16::try_from(glyph.id) else {
                        continue;
                    };
                    let subpixel_x = px - px.floor();
                    let subpixel_y = py - py.floor();
                    let key = GlyphKey {
                        font_blob: font.data.id(),
                        font_index: font.index,
                        font_size: (run.font_size() * scale).to_bits(),
                        coords: run.normalized_coords().to_vec(),
                        glyph: id,
                        subpixel_x: subpixel_x.to_bits(),
                        subpixel_y: subpixel_y.to_bits(),
                    };
                    let Some(glyph_image) = self.glyphs.get_or_render(key.clone(), || {
                        Render::new(&[
                            Source::ColorOutline(0),
                            Source::ColorBitmap(StrikeWith::BestFit),
                            Source::Outline,
                        ])
                        .format(Format::Alpha)
                        .offset(Vector::new(subpixel_x, subpixel_y))
                        .render(&mut scaler, id)
                    }) else {
                        continue;
                    };
                    let left = px.floor() as i32 + glyph_image.placement.left;
                    let top = py.floor() as i32 - glyph_image.placement.top;
                    glyphs.push(RasterGlyph {
                        key,
                        left,
                        top,
                        width: glyph_image.placement.width,
                        height: glyph_image.placement.height,
                        image: glyph_image,
                    });
                }
            }
        }
        let glyphs = self.glyph_runs.insert(key, snapshot.clone(), glyphs);
        for glyph in glyphs.iter() {
            visit(glyph);
        }
        Ok(())
    }
}

fn glyph_pixel(content: Content, data: &[u8], at: usize, foreground: [u8; 4]) -> [u8; 4] {
    let opacity = |alpha: u8| ((alpha as u16 * foreground[3] as u16) / 255) as u8;
    match content {
        Content::Mask => [
            foreground[0],
            foreground[1],
            foreground[2],
            opacity(data[at]),
        ],
        Content::Color => {
            let mut color: [u8; 4] = data[at * 4..at * 4 + 4]
                .try_into()
                .expect("RGBA glyph pixel");
            color[3] = opacity(color[3]);
            color
        }
        Content::SubpixelMask => {
            let alpha = data[at * 4..at * 4 + 3].iter().copied().max().unwrap_or(0);
            [foreground[0], foreground[1], foreground[2], opacity(alpha)]
        }
    }
}

fn rgba(color: Color) -> [u8; 4] {
    [color.red, color.green, color.blue, color.alpha]
        .map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn fill(image: &mut TextInputBitmap, rect: Rect, clip: Rect, color: [u8; 4]) {
    let Some(rect) = rect.intersection(clip) else {
        return;
    };
    for y in rect.y.floor().max(0.0) as i32..rect.bottom().ceil().min(image.height as f32) as i32 {
        for x in rect.x.floor().max(0.0) as i32..rect.right().ceil().min(image.width as f32) as i32
        {
            blend(image, x, y, clip, color);
        }
    }
}

fn blend(image: &mut TextInputBitmap, x: i32, y: i32, clip: Rect, source: [u8; 4]) {
    if source[3] == 0
        || x < 0
        || y < 0
        || x >= image.width as i32
        || y >= image.height as i32
        || !clip.contains(crate::Point::new(x as f32 + 0.5, y as f32 + 0.5))
    {
        return;
    }
    let at = (y as usize * image.width as usize + x as usize) * 4;
    let destination = &mut image.rgba[at..at + 4];
    let sa = source[3] as f32 / 255.0;
    let da = destination[3] as f32 / 255.0;
    let alpha = sa + da * (1.0 - sa);
    for c in 0..3 {
        destination[c] = ((source[c] as f32 * sa + destination[c] as f32 * da * (1.0 - sa)) / alpha)
            .round() as u8;
    }
    destination[3] = (alpha * 255.0).round() as u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positioned_glyph_cache_follows_snapshot_origin_clip_and_scale() {
        let font = include_bytes!("../tests/fixtures/MaterialIcons-Regular.ttf");
        let mut context = crate::TextInputContext::default();
        assert!(context.register_font(Arc::new(font.to_vec())));
        context.set_font_stack(crate::FontRole::Icon, "Material Icons");
        let snapshot = context
            .shape_static_text("\u{e145}", 24.0, crate::FontRole::Icon)
            .expect("icon layout");
        let mut rasterizer = TextRasterizer::default();
        let bounds = Rect::new(0.0, 0.0, 100.0, 40.0);
        let view = TextInputViewport {
            bounds,
            clip: bounds,
            scroll_x: 0.0,
            scroll_y: 0.0,
            origin: crate::Point::new(0.0, 0.0),
            caret: None,
        };
        let mut positions = Vec::new();
        rasterizer
            .visit_glyphs(&snapshot, view, 1.0, |glyph| positions.push(glyph.left))
            .expect("first rasterization");
        assert!(!positions.is_empty());
        rasterizer
            .visit_glyphs(&snapshot, view, 1.0, |_| {})
            .expect("cached rasterization");
        assert_eq!(rasterizer.glyph_runs.entries.len(), 1);

        let translated = TextInputViewport {
            bounds: Rect::new(32.0, 18.0, bounds.width, bounds.height),
            clip: Rect::new(32.0, 18.0, bounds.width, bounds.height),
            origin: crate::Point::new(32.0, 18.0),
            ..view
        };
        let mut translated_positions = Vec::new();
        rasterizer
            .visit_glyphs(&snapshot, translated, 1.0, |glyph| {
                translated_positions.push(glyph.left)
            })
            .expect("translation-invariant rasterization");
        assert_eq!(translated_positions, positions);
        assert_eq!(rasterizer.glyph_runs.entries.len(), 1);

        let moved = TextInputViewport {
            origin: crate::Point::new(8.0, 0.0),
            ..view
        };
        let mut moved_positions = Vec::new();
        rasterizer
            .visit_glyphs(&snapshot, moved, 1.0, |glyph| {
                moved_positions.push(glyph.left)
            })
            .expect("moved rasterization");
        assert_eq!(moved_positions[0], positions[0] + 8);
        assert_eq!(rasterizer.glyph_runs.entries.len(), 2);

        let clipped = TextInputViewport {
            clip: Rect::new(0.0, 0.0, 50.0, 40.0),
            ..view
        };
        rasterizer
            .visit_glyphs(&snapshot, clipped, 1.0, |_| {})
            .expect("clipped rasterization");
        rasterizer
            .visit_glyphs(&snapshot, view, 1.25, |_| {})
            .expect("scaled rasterization");
        let replacement = context
            .shape_static_text("\u{e14c}", 24.0, crate::FontRole::Icon)
            .expect("replacement layout");
        rasterizer
            .visit_glyphs(&replacement, view, 1.0, |_| {})
            .expect("replacement rasterization");
        assert_eq!(rasterizer.glyph_runs.entries.len(), 5);
    }

    #[test]
    fn text_opacity_applies_to_color_emoji_and_subpixel_glyphs() {
        let foreground = [10, 20, 30, 128];
        assert_eq!(glyph_pixel(Content::Mask, &[255], 0, foreground)[3], 128);
        assert_eq!(
            glyph_pixel(Content::Color, &[100, 150, 200, 255], 0, foreground),
            [100, 150, 200, 128]
        );
        assert_eq!(
            glyph_pixel(Content::SubpixelMask, &[255, 100, 30, 0], 0, foreground),
            [10, 20, 30, 128]
        );
    }
}
