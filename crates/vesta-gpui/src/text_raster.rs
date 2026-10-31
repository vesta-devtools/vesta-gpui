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
    /// Invalid geometry/scale, unrepresentable pixel coordinates or bitmap extent.
    InvalidViewport,
    /// Physical font em or one complete prepared run exceeds its resource budget.
    GlyphBudgetExceeded,
}

impl std::fmt::Display for TextRasterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidViewport => f.write_str("invalid or oversized text raster viewport"),
            Self::GlyphBudgetExceeded => f.write_str("text raster glyph budget exceeded"),
        }
    }
}
impl std::error::Error for TextRasterError {}

const GLYPH_CACHE_ENTRIES: usize = 4096;
const GLYPH_CACHE_BYTES: usize = 8 * 1024 * 1024;
const GLYPH_RUN_CACHE_ENTRIES: usize = 256;
const GLYPH_RUN_CACHE_GLYPHS: usize = 8192;
const GLYPH_RUN_CACHE_BYTES: usize = 8 * 1024 * 1024;
const BITMAP_CACHE_ENTRIES: usize = 64;
const BITMAP_CACHE_BYTES: usize = 16 * 1024 * 1024;
const MAX_BITMAP_PIXELS: f32 = 4_194_304.0;
const MAX_GLYPH_EM_PIXELS: f32 = 2048.0;

/// One checked bounds-relative physical coordinate system for both raster paths.
struct RasterViewport {
    origin: crate::Point,
    clip: Rect,
    width: f32,
    height: f32,
}

impl RasterViewport {
    fn new(view: TextInputViewport, scale: f32) -> Result<Self, TextRasterError> {
        if !scale.is_finite()
            || scale <= 0.0
            || !view.bounds.is_valid()
            || !view.clip.is_valid()
            || !view.origin.x.is_finite()
            || !view.origin.y.is_finite()
        {
            return Err(TextRasterError::InvalidViewport);
        }
        let physical = |value: f64| {
            let value = value * f64::from(scale);
            if value.is_finite() && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&value) {
                Ok(value as f32)
            } else {
                Err(TextRasterError::InvalidViewport)
            }
        };
        let width = physical(f64::from(view.bounds.width))?.ceil();
        let height = physical(f64::from(view.bounds.height))?.ceil();
        let origin = crate::Point::new(
            physical(f64::from(view.origin.x) - f64::from(view.bounds.x))?,
            physical(f64::from(view.origin.y) - f64::from(view.bounds.y))?,
        );
        let clip = if let Some(clip) = view.clip.intersection(view.bounds) {
            Rect::new(
                physical(f64::from(clip.x) - f64::from(view.bounds.x))?,
                physical(f64::from(clip.y) - f64::from(view.bounds.y))?,
                physical(f64::from(clip.width))?,
                physical(f64::from(clip.height))?,
            )
        } else {
            Rect::default()
        };
        if !clip.is_valid() {
            return Err(TextRasterError::InvalidViewport);
        }
        Ok(Self {
            origin,
            clip,
            width,
            height,
        })
    }
}

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
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.key.coords.len() * std::mem::size_of::<i16>())
            .saturating_add(self.image.data.len())
    }

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
    clip: [u32; 4],
    origin: [u32; 2],
    scale: u32,
}

impl GlyphRunKey {
    fn new(snapshot: &TextInputSnapshot, view: &RasterViewport, scale: f32) -> Self {
        Self {
            snapshot: Arc::as_ptr(&snapshot.0) as usize,
            clip: [
                view.clip.x.to_bits(),
                view.clip.y.to_bits(),
                view.clip.width.to_bits(),
                view.clip.height.to_bits(),
            ],
            origin: [view.origin.x.to_bits(), view.origin.y.to_bits()],
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
            total.saturating_add(glyph.retained_bytes())
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
/// Host-local CPU raster preparation with bounded caches. Drop releases its
/// cache ownership; returned images/snapshots can outlive it through their Arcs.
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
        let physical = RasterViewport::new(view, scale)?;
        if !(1.0..=8192.0).contains(&physical.width)
            || !(1.0..=8192.0).contains(&physical.height)
            || physical.width * physical.height > MAX_BITMAP_PIXELS
        {
            return Err(TextRasterError::InvalidViewport);
        }
        let key = BitmapKey::new(snapshot, view, scale, focused, caret_visible, colors);
        if let Some(bitmap) = self.bitmaps.find(&key) {
            return Ok(bitmap);
        }
        let glyphs = self.glyph_run(snapshot, scale, &physical)?;
        let bitmap = Arc::new(Self::compose_bitmap(
            snapshot,
            scale,
            physical,
            focused,
            caret_visible,
            colors,
            &glyphs,
        ));
        self.bitmaps.insert(key, snapshot.clone(), bitmap.clone());
        Ok(bitmap)
    }

    fn compose_bitmap(
        snapshot: &TextInputSnapshot,
        scale: f32,
        physical: RasterViewport,
        focused: bool,
        caret_visible: bool,
        colors: TextInputColors,
        glyphs: &[RasterGlyph],
    ) -> TextInputBitmap {
        let RasterViewport {
            origin,
            clip,
            width,
            height,
        } = physical;
        let mut image = TextInputBitmap {
            width: width as u32,
            height: height as u32,
            rgba: vec![0; width as usize * height as usize * 4],
        };
        let transform = |r: Rect| {
            Rect::new(
                origin.x + r.x * scale,
                origin.y + r.y * scale,
                r.width * scale,
                r.height * scale,
            )
        };
        for r in snapshot.selection_rectangles() {
            let mut color = colors.selection;
            if !focused {
                color.alpha *= 0.45;
            }
            fill(&mut image, transform(*r), clip, rgba(color));
        }
        let foreground = rgba(colors.text);
        for glyph in glyphs {
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
        }
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
        image
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

    /// Borrow a complete checked run without cloning keys/images. Invalid
    /// viewport or glyph-budget failure invokes no callbacks; empty clip is a no-op.
    pub fn visit_glyph_refs(
        &mut self,
        snapshot: &TextInputSnapshot,
        view: TextInputViewport,
        scale: f32,
        mut visit: impl FnMut(&RasterGlyph),
    ) -> Result<(), TextRasterError> {
        let physical = RasterViewport::new(view, scale)?;
        let glyphs = self.glyph_run(snapshot, scale, &physical)?;
        for glyph in glyphs.iter() {
            visit(glyph);
        }
        Ok(())
    }

    fn glyph_run(
        &mut self,
        snapshot: &TextInputSnapshot,
        scale: f32,
        physical: &RasterViewport,
    ) -> Result<Arc<Vec<RasterGlyph>>, TextRasterError> {
        if physical.clip.is_empty() {
            return Ok(Arc::new(Vec::new()));
        }
        let key = GlyphRunKey::new(snapshot, physical, scale);
        if let Some(glyphs) = self.glyph_runs.find(&key) {
            return Ok(glyphs);
        }
        if snapshot
            .0
            .layout
            .lines()
            .flat_map(|line| line.runs())
            .any(|run| {
                let em = run.font_size() * scale;
                !em.is_finite() || em <= 0.0 || em > MAX_GLYPH_EM_PIXELS
            })
        {
            return Err(TextRasterError::GlyphBudgetExceeded);
        }
        let mut glyphs = Vec::new();
        let mut glyph_bytes = 0usize;
        let RasterViewport { origin, clip, .. } = *physical;
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
                    let px = origin.x + (x + glyph.x) * scale;
                    let py = origin.y + (glyph_run.baseline() - glyph.y) * scale;
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
                    let left = (px.floor() as i32)
                        .checked_add(glyph_image.placement.left)
                        .ok_or(TextRasterError::InvalidViewport)?;
                    let top = (py.floor() as i32)
                        .checked_sub(glyph_image.placement.top)
                        .ok_or(TextRasterError::InvalidViewport)?;
                    let glyph = RasterGlyph {
                        key,
                        left,
                        top,
                        width: glyph_image.placement.width,
                        height: glyph_image.placement.height,
                        image: glyph_image,
                    };
                    glyph_bytes = glyph_bytes.saturating_add(glyph.retained_bytes());
                    if glyphs.len() == GLYPH_RUN_CACHE_GLYPHS || glyph_bytes > GLYPH_RUN_CACHE_BYTES
                    {
                        return Err(TextRasterError::GlyphBudgetExceeded);
                    }
                    glyphs.push(glyph);
                }
            }
        }
        Ok(self.glyph_runs.insert(key, snapshot.clone(), glyphs))
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
    fn raster_admission_rejects_invalid_geometry_before_work_and_recovers() {
        let snapshot = TextInputSnapshot::default();
        let bounds = Rect::new(0.0, 0.0, 100.0, 40.0);
        let view = TextInputViewport {
            bounds,
            clip: bounds,
            origin: crate::Point::default(),
            caret: None,
            scroll_x: 0.0,
            scroll_y: 0.0,
        };
        let colors = TextInputColors {
            text: Color::rgb8(255, 255, 255),
            selection: Color::rgb8(255, 255, 255),
            caret: Color::rgb8(255, 255, 255),
        };
        let mut rasterizer = TextRasterizer::default();
        let mut accepted = Vec::new();
        for (name, candidate) in [
            (
                "nan-origin",
                TextInputViewport {
                    origin: crate::Point::new(f32::NAN, 0.0),
                    ..view
                },
            ),
            (
                "infinite-bounds",
                TextInputViewport {
                    bounds: Rect::new(f32::INFINITY, 0.0, 100.0, 40.0),
                    ..view
                },
            ),
            (
                "negative-bounds",
                TextInputViewport {
                    bounds: Rect::new(0.0, 0.0, -1.0, 40.0),
                    ..view
                },
            ),
            (
                "nan-clip",
                TextInputViewport {
                    clip: Rect::new(0.0, f32::NAN, 100.0, 40.0),
                    ..view
                },
            ),
            (
                "negative-clip",
                TextInputViewport {
                    clip: Rect::new(0.0, 0.0, -1.0, 40.0),
                    ..view
                },
            ),
            (
                "unrepresentable-origin",
                TextInputViewport {
                    origin: crate::Point::new(f32::MAX, 0.0),
                    ..view
                },
            ),
        ] {
            let mut callbacks = 0;
            if rasterizer.visit_glyph_refs(&snapshot, candidate, 1.0, |_| callbacks += 1)
                != Err(TextRasterError::InvalidViewport)
            {
                accepted.push(format!("borrowed:{name}"));
            }
            if rasterizer.visit_glyphs(&snapshot, candidate, 1.0, |_| callbacks += 1)
                != Err(TextRasterError::InvalidViewport)
            {
                accepted.push(format!("owned:{name}"));
            }
            if rasterizer
                .render_shared(&snapshot, candidate, 1.0, false, false, colors)
                .is_ok()
            {
                accepted.push(format!("bitmap:{name}"));
            }
            assert_eq!(callbacks, 0);
        }
        assert!(
            accepted.is_empty(),
            "invalid raster viewports were accepted: {accepted:?}"
        );
        assert!(
            rasterizer.glyphs.images.is_empty()
                && rasterizer.glyph_runs.entries.is_empty()
                && rasterizer.bitmaps.entries.is_empty()
        );
        assert!(
            rasterizer
                .render_shared(&snapshot, view, 1.0, false, false, colors)
                .is_ok()
        );
        let mut context = crate::TextInputContext::default();
        assert!(context.register_font(Arc::new(
            include_bytes!("../tests/fixtures/MaterialIcons-Regular.ttf").to_vec()
        )));
        context.set_font_stack(crate::FontRole::Ui, "Material Icons");
        let ordinary = context
            .shape_styled_text("\u{e145}", 24.0, crate::FontRole::Ui, &[], 100.0)
            .expect("ordinary glyph");
        let mut callbacks = 0;
        rasterizer
            .visit_glyph_refs(&ordinary, view, 1.0, |_| callbacks += 1)
            .expect("real glyph");
        assert!(callbacks > 0);
        let mut rejected_callbacks = 0;
        for owned in [false, true] {
            let result = if owned {
                rasterizer.visit_glyphs(&ordinary, view, 100.0, |_| rejected_callbacks += 1)
            } else {
                rasterizer.visit_glyph_refs(&ordinary, view, 100.0, |_| rejected_callbacks += 1)
            };
            assert_eq!(result, Err(TextRasterError::GlyphBudgetExceeded));
        }
        let oversized = context
            .shape_styled_text(
                &"X".repeat(GLYPH_RUN_CACHE_GLYPHS + 1),
                8.0,
                crate::FontRole::Ui,
                &[],
                1_000_000.0,
            )
            .expect("bounded source, oversized glyph run");
        let wide = TextInputViewport {
            bounds: Rect::new(0.0, 0.0, 1_000_000.0, 40.0),
            clip: Rect::new(0.0, 0.0, 1_000_000.0, 40.0),
            ..view
        };
        assert_eq!(
            rasterizer.visit_glyph_refs(&oversized, wide, 1.0, |_| rejected_callbacks += 1),
            Err(TextRasterError::GlyphBudgetExceeded)
        );
        assert_eq!(
            rejected_callbacks, 0,
            "a rejected run cannot publish a prefix"
        );
        assert!(
            rasterizer.glyph_runs.glyph_count <= GLYPH_RUN_CACHE_GLYPHS
                && rasterizer.glyph_runs.bytes <= GLYPH_RUN_CACHE_BYTES
        );
        assert!(
            rasterizer
                .render_shared(&ordinary, view, 1.0, false, false, colors)
                .is_ok()
        );
        let weak = Arc::downgrade(&ordinary.0);
        drop(ordinary);
        assert!(
            weak.upgrade().is_some(),
            "cached snapshots protect pointer-key identity"
        );
        for index in 0..=GLYPH_RUN_CACHE_ENTRIES {
            let next = context
                .shape_styled_text(
                    &format!("\u{e145}{index}"),
                    13.0,
                    crate::FontRole::Ui,
                    &[],
                    100.0,
                )
                .expect("new snapshot");
            let first = rasterizer
                .render_shared(&next, view, 1.0, false, false, colors)
                .expect("new bitmap");
            let second = rasterizer
                .render_shared(&next, view, 1.0, false, false, colors)
                .expect("cached bitmap");
            assert!(Arc::ptr_eq(&first, &second));
        }
        assert!(
            weak.upgrade().is_none(),
            "eviction releases snapshots after their last run/bitmap entry"
        );
        assert!(
            rasterizer.bitmaps.entries.len() <= BITMAP_CACHE_ENTRIES
                && rasterizer.bitmaps.bytes <= BITMAP_CACHE_BYTES
        );
        assert!(
            rasterizer.glyphs.images.len() <= GLYPH_CACHE_ENTRIES
                && rasterizer.glyphs.bytes <= GLYPH_CACHE_BYTES
        );
        assert!(
            rasterizer.glyph_runs.entries.len() <= GLYPH_RUN_CACHE_ENTRIES
                && rasterizer.glyph_runs.bytes <= GLYPH_RUN_CACHE_BYTES
        );
    }

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
