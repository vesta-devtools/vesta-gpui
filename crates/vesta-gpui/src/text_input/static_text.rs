//! Immutable static shaping, width reflow and grapheme ellipsis (ADR-0229).
use super::*;
use std::{collections::VecDeque, hash::Hash};

const STATIC_TEXT_CACHE_ENTRIES: usize = 512;
const STATIC_TEXT_CACHE_BYTES: usize = 128 * 1024;

pub(super) type StaticTextBaseKey = (String, u32, FontRole);
pub(super) type StaticTextWrappedKey = (String, u32, FontRole, u32);

pub(super) struct StaticTextCache<K, V = TextInputSnapshot> {
    entries: HashMap<K, (usize, V)>,
    order: VecDeque<K>,
    source_bytes: usize,
}

impl<K, V> Default for StaticTextCache<K, V> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            source_bytes: 0,
        }
    }
}

impl<K, V> StaticTextCache<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    fn get(&self, key: &K) -> Option<V> {
        self.entries.get(key).map(|(_, snapshot)| snapshot.clone())
    }

    fn insert(&mut self, key: K, source_bytes: usize, snapshot: V) {
        if source_bytes > STATIC_TEXT_CACHE_BYTES {
            return;
        }
        while self.order.len() >= STATIC_TEXT_CACHE_ENTRIES
            || self.source_bytes + source_bytes > STATIC_TEXT_CACHE_BYTES
        {
            let Some(old) = self.order.pop_front() else {
                break;
            };
            if let Some((old_bytes, _)) = self.entries.remove(&old) {
                self.source_bytes -= old_bytes;
            }
        }
        self.source_bytes += source_bytes;
        self.order.push_back(key.clone());
        self.entries.insert(key, (source_bytes, snapshot));
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.source_bytes = 0;
    }
}

impl TextInputContext {
    /// Shape static text with the same Parley fonts used by editable text.
    /// The generous width preserves hard lines without introducing soft wraps.
    pub fn shape_static_text(
        &mut self,
        text: &str,
        font_size: f32,
        role: FontRole,
    ) -> Result<TextInputSnapshot, TextInputError> {
        self.shape_static_text_wrapped(text, font_size, role, None)
    }

    /// Shape static text with an optional finite width for soft wrapping.
    pub fn shape_static_text_wrapped(
        &mut self,
        text: &str,
        font_size: f32,
        role: FontRole,
        wrap_width: Option<f32>,
    ) -> Result<TextInputSnapshot, TextInputError> {
        let wrap_width = wrap_width.filter(|width| width.is_finite() && *width > 0.0);
        let base_key = (text.to_owned(), font_size.to_bits(), role);
        if let Some(wrap_width) = wrap_width {
            let wrapped_key = (
                text.to_owned(),
                font_size.to_bits(),
                role,
                wrap_width.to_bits(),
            );
            if let Some(snapshot) = self.static_text_wrapped.get(&wrapped_key) {
                return Ok(snapshot);
            }
            let base = self.shape_static_text_wrapped(text, font_size, role, None)?;
            let mut layout = base.0.layout.clone();
            layout.break_all_lines(Some(wrap_width));
            layout.align(
                Some(wrap_width),
                parley::Alignment::Left,
                parley::AlignmentOptions {
                    align_when_overflowing: true,
                },
            );
            let snapshot = TextInputSnapshot(
                Arc::new(TextInputVisual {
                    layout,
                    selection: Vec::new(),
                    caret: None,
                    compose: Vec::new(),
                }),
                TextVAlign::Center,
                None,
                None,
                None,
            );
            self.static_text_wrapped
                .insert(wrapped_key, text.len(), snapshot.clone());
            return Ok(snapshot);
        }
        if let Some(snapshot) = self.static_text_bases.get(&base_key) {
            return Ok(snapshot);
        }
        let default_stack = match role {
            FontRole::Mono
            | FontRole::MonoBold
            | FontRole::MonoItalic
            | FontRole::MonoBoldItalic
            | FontRole::MonoCjk => "monospace",
            FontRole::Ui | FontRole::Icon => "sans-serif",
        };
        let stack = self
            .font_stacks
            .get(&role)
            .map_or(default_stack, String::as_str);
        let mut state = TextInputState::new_read_only(
            font_size,
            stack,
            Some(wrap_width.unwrap_or(1_000_000.0)),
        );
        if matches!(role, FontRole::MonoBold | FontRole::MonoBoldItalic) {
            state.editor.edit_styles().insert(StyleProperty::FontWeight(
                parley::fontique::FontWeight::BOLD,
            ));
        }
        if matches!(role, FontRole::MonoItalic | FontRole::MonoBoldItalic) {
            state.editor.edit_styles().insert(StyleProperty::FontStyle(
                parley::fontique::FontStyle::Italic,
            ));
        }
        state.reset(self, text)?;
        // Static paint places this intrinsic block in its own frame. Preserve
        // Parley's bidi order, but remove the editor's directional alignment
        // against the generous no-wrap width before publishing the cache entry.
        let mut snapshot = state.snapshot();
        Arc::make_mut(&mut snapshot.0).layout.align(
            None,
            parley::Alignment::Left,
            parley::AlignmentOptions {
                align_when_overflowing: true,
            },
        );
        self.static_text_bases
            .insert(base_key, text.len(), snapshot.clone());
        Ok(snapshot)
    }

    #[cfg(test)]
    pub(crate) fn static_text_cache_counts(&self) -> (usize, usize) {
        (
            self.static_text_bases.entries.len(),
            self.static_text_wrapped.entries.len(),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EllipsisError {
    InvalidWidth,
    InvalidFontSize,
    Shape(TextInputError),
}

impl fmt::Display for EllipsisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "static text ellipsis failed: {self:?}")
    }
}

impl std::error::Error for EllipsisError {}

/// A derived single-line display; the caller retains the complete source.
#[derive(Clone, Debug, PartialEq)]
pub struct EllipsizedText {
    text: Arc<str>,
    snapshot: TextInputSnapshot,
    truncated: bool,
}

impl EllipsizedText {
    pub fn visible_text(&self) -> &str {
        &self.text
    }

    pub fn snapshot(&self) -> TextInputSnapshot {
        self.snapshot.clone()
    }

    pub const fn is_truncated(&self) -> bool {
        self.truncated
    }
}

impl TextInputContext {
    /// Fit a logical grapheme prefix plus U+2026 to an exact single-line width.
    /// A hard-line suffix is omitted with the marker. An unfit marker displays empty.
    /// Contextual shaping may be nonmonotonic; the selected candidate is measured
    /// to fit, but this logarithmic search does not promise a globally longest prefix.
    pub fn shape_static_text_ellipsized(
        &mut self,
        text: &str,
        font_size: f32,
        role: FontRole,
        width: f32,
    ) -> Result<EllipsizedText, EllipsisError> {
        if !width.is_finite() || width < 0.0 {
            return Err(EllipsisError::InvalidWidth);
        }
        if !font_size.is_finite() || font_size <= 0.0 {
            return Err(EllipsisError::InvalidFontSize);
        }
        if text.len() > TEXT_INPUT_MAX_BYTES {
            return Err(EllipsisError::Shape(TextInputError::TooLong));
        }
        let key = (text.to_owned(), font_size.to_bits(), role, width.to_bits());
        if let Some(result) = self.static_text_ellipsized.get(&key) {
            return Ok(result);
        }
        let first = text
            .split(['\r', '\n', '\u{2028}', '\u{2029}'])
            .next()
            .unwrap_or_default();
        let base = self
            .shape_static_text(first, font_size, role)
            .map_err(EllipsisError::Shape)?;
        let mut result = if text.is_empty() {
            EllipsizedText {
                text: Arc::from(""),
                snapshot: TextInputSnapshot::default(),
                truncated: false,
            }
        } else if first.len() == text.len() && base.layout_width() <= width {
            EllipsizedText {
                text: Arc::from(first),
                snapshot: base,
                truncated: false,
            }
        } else {
            self.ellipsis_prefix(first, font_size, role, width)?
        };
        // PlainEditor snapshots include an insertion caret; immutable displays
        // must not scroll toward it or publish editing decorations.
        let visual = Arc::make_mut(&mut result.snapshot.0);
        visual.caret = None;
        visual.selection.clear();
        visual.compose.clear();
        self.static_text_ellipsized
            .insert(key, text.len() + result.text.len(), result.clone());
        Ok(result)
    }

    fn ellipsis_prefix(
        &mut self,
        first: &str,
        font_size: f32,
        role: FontRole,
        width: f32,
    ) -> Result<EllipsizedText, EllipsisError> {
        let marker = self
            .shape_static_text("…", font_size, role)
            .map_err(EllipsisError::Shape)?;
        let mut best = EllipsizedText {
            text: Arc::from("…"),
            snapshot: marker,
            truncated: true,
        };
        if width == 0.0 || best.snapshot.layout_width() > width {
            best.text = Arc::from("");
            // The editor's empty-layout placeholder has an editing extent. A
            // non-editable empty display needs neither that extent nor glyphs.
            best.snapshot = TextInputSnapshot::default();
            return Ok(best);
        }
        let mut ends = vec![0];
        ends.extend(
            first
                .grapheme_indices(true)
                .map(|(start, grapheme)| start + grapheme.len()),
        );
        while ends
            .last()
            .is_some_and(|end| *end + "…".len() > TEXT_INPUT_MAX_BYTES)
        {
            ends.pop();
        }
        let (mut low, mut high) = (0, ends.len());
        while low < high {
            let middle = low + (high - low) / 2;
            let candidate = format!("{}…", &first[..ends[middle]]);
            let snapshot = self
                .shape_static_text(&candidate, font_size, role)
                .map_err(EllipsisError::Shape)?;
            if snapshot.layout_width() <= width {
                best.text = Arc::from(candidate);
                best.snapshot = snapshot;
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        Ok(best)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ellipsis_fits_graphemes_preserves_sources_and_invalidates_font_results() {
        let mut context = TextInputContext::default();
        assert!(context.register_font(Arc::new(
            include_bytes!("../../tests/fixtures/MaterialIcons-Regular.ttf").to_vec(),
        )));
        let source = "Ae\u{301}👨‍👩‍👧‍👦中مرحبا tail";
        let full_width = context
            .shape_static_text(source, 14.0, FontRole::Ui)
            .expect("full shape")
            .layout_width();
        let wide = context
            .shape_static_text_ellipsized(source, 14.0, FontRole::Ui, full_width + 1.0)
            .expect("wide label");
        assert_eq!(wide.visible_text(), source);
        assert!(!wide.is_truncated());
        let empty = context
            .shape_static_text_ellipsized("", 14.0, FontRole::Ui, 0.0)
            .expect("empty static source");
        assert_eq!(empty.visible_text(), "");
        assert!(!empty.is_truncated());
        assert_eq!(empty.snapshot.layout_width(), 0.0);
        let boundaries = source
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .chain(std::iter::once(source.len()))
            .collect::<Vec<_>>();
        for fraction in [0.0, 0.15, 0.35, 0.6, 0.85] {
            let width = full_width * fraction;
            let result = context
                .shape_static_text_ellipsized(source, 14.0, FontRole::Ui, width)
                .expect("narrow label");
            assert!(result.is_truncated());
            assert!(
                result.snapshot.layout_width() <= width,
                "visible={:?} shaped={} available={width}",
                result.visible_text(),
                result.snapshot.layout_width()
            );
            let prefix = result.visible_text().strip_suffix('…').unwrap_or_default();
            assert!(source.starts_with(prefix) && boundaries.contains(&prefix.len()));
            let again = context
                .shape_static_text_ellipsized(source, 14.0, FontRole::Ui, width)
                .expect("cached label");
            assert_eq!(result, again);
        }
        let hard = context
            .shape_static_text_ellipsized("first\r\nhidden source", 14.0, FontRole::Ui, 1000.0)
            .expect("hard line");
        assert_eq!(hard.visible_text(), "first…");
        assert!(
            hard.snapshot.0.caret.is_none()
                && hard.snapshot.0.selection.is_empty()
                && hard.snapshot.0.compose.is_empty()
        );
        for width in [-1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(
                context.shape_static_text_ellipsized(source, 14.0, FontRole::Ui, width),
                Err(EllipsisError::InvalidWidth)
            );
        }
        for size in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(
                context.shape_static_text_ellipsized(source, size, FontRole::Ui, 50.0),
                Err(EllipsisError::InvalidFontSize)
            );
        }
        assert_eq!(
            context.shape_static_text_ellipsized(
                &"x".repeat(TEXT_INPUT_MAX_BYTES + 1),
                14.0,
                FontRole::Ui,
                50.0,
            ),
            Err(EllipsisError::Shape(TextInputError::TooLong))
        );
        let old = context
            .shape_static_text_ellipsized(source, 14.0, FontRole::Ui, full_width * 0.6)
            .expect("old font result");
        context.set_font_stack(FontRole::Ui, "Material Icons");
        assert!(context.static_text_ellipsized.entries.is_empty());
        let refreshed = context
            .shape_static_text_ellipsized(source, 14.0, FontRole::Ui, full_width * 0.6)
            .expect("new font result");
        assert_ne!(old.snapshot, refreshed.snapshot);
        assert_eq!(source, "Ae\u{301}👨‍👩‍👧‍👦中مرحبا tail");
        for index in 0..600 {
            context
                .shape_static_text_ellipsized(source, 14.0, FontRole::Ui, index as f32)
                .expect("width stream");
        }
        assert!(context.static_text_ellipsized.entries.len() <= STATIC_TEXT_CACHE_ENTRIES);
        assert!(context.static_text_ellipsized.source_bytes <= STATIC_TEXT_CACHE_BYTES);
    }
}
