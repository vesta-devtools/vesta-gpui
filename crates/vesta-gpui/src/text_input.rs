//! Editable text and immutable Parley geometry. Fonts and I/O belong to the host.
use std::{borrow::Cow, collections::HashMap, fmt, sync::Arc};

use parley::{
    FontContext, FontStack, Layout, LayoutContext, LineHeight, PlainEditor, StyleProperty,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{FontRole, Point, Rect, TextVAlign};

mod styled;
pub use styled::{StyledTextAlignment, StyledTextError, StyledTextOptions, StyledTextSpan};
mod accessibility;
mod marks;
mod navigation;
mod read_only;
mod selection;
use marks::EditSnapshot;
pub use marks::TextInputMark;
pub use navigation::TextLayoutCursor;
mod static_text;
pub use static_text::{EllipsisError, EllipsizedText};
use static_text::{StaticTextBaseKey, StaticTextCache, StaticTextWrappedKey};

pub(crate) const CARET_VISUAL_GAP: f32 = 1.0;

pub const TEXT_INPUT_MAX_BYTES: usize = 16 * 1024;
const HISTORY_STEPS: usize = 32;
const HISTORY_TEXT_BYTES: usize = 256 * 1024;

pub struct TextInputContext {
    measurement_source: Arc<()>,
    fonts: FontContext,
    layout: LayoutContext<()>,
    font_revision: u64,
    font_stacks: HashMap<FontRole, String>,
    static_text_bases: StaticTextCache<StaticTextBaseKey>,
    static_text_wrapped: StaticTextCache<StaticTextWrappedKey>,
    static_text_ellipsized: StaticTextCache<StaticTextWrappedKey, EllipsizedText>,
}

/// Opaque in-memory validity token for measurements from a text context.
/// It is not a persistent identity or a serialized document revision.
#[derive(Clone, Debug)]
pub struct TextMeasurementRevision {
    source: Arc<()>,
    font_revision: u64,
}
impl PartialEq for TextMeasurementRevision {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source, &other.source) && self.font_revision == other.font_revision
    }
}
impl Eq for TextMeasurementRevision {}

impl Default for TextInputContext {
    fn default() -> Self {
        Self {
            measurement_source: Arc::new(()),
            fonts: FontContext {
                collection: parley::fontique::Collection::new(
                    parley::fontique::CollectionOptions {
                        shared: false,
                        system_fonts: false,
                    },
                ),
                source_cache: Default::default(),
            },
            layout: LayoutContext::new(),
            font_revision: 0,
            font_stacks: HashMap::new(),
            static_text_bases: StaticTextCache::default(),
            static_text_wrapped: StaticTextCache::default(),
            static_text_ellipsized: StaticTextCache::default(),
        }
    }
}

impl TextInputContext {
    /// Changes after successful font registration, a changed role stack, or
    /// replacement by another context. No-op/failed font updates preserve it.
    pub fn measurement_revision(&self) -> TextMeasurementRevision {
        TextMeasurementRevision {
            source: Arc::clone(&self.measurement_source),
            font_revision: self.font_revision,
        }
    }

    fn invalidate_fonts(&mut self) {
        self.font_revision = self.font_revision.wrapping_add(1);
        self.static_text_bases.clear();
        self.static_text_wrapped.clear();
        self.static_text_ellipsized.clear();
    }

    /// Register host-provided bytes; this API never searches the filesystem.
    pub fn register_font(&mut self, bytes: Arc<Vec<u8>>) -> bool {
        let families = self
            .fonts
            .collection
            .register_fonts(parley::fontique::Blob::new(bytes), None);
        if families.is_empty() {
            return false;
        }
        self.fonts.collection.append_generic_families(
            parley::fontique::GenericFamily::SansSerif,
            families.iter().map(|(id, _)| *id),
        );
        self.invalidate_fonts();
        true
    }

    /// Choose a host-registered family for a semantic static-text role.
    pub fn set_font_stack(&mut self, role: FontRole, stack: impl Into<String>) {
        let stack = stack.into();
        if stack.is_empty() || self.font_stacks.get(&role) == Some(&stack) {
            return;
        }
        self.font_stacks.insert(role, stack);
        self.invalidate_fonts();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextInputError {
    TooLong,
    Composing,
    ReadOnly,
    InvalidMark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextEditKind {
    Typing,
    Paste,
    Ime,
    Backspace,
    Delete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextInputAction {
    Left {
        extend: bool,
        word: bool,
    },
    Right {
        extend: bool,
        word: bool,
    },
    /// 多行编辑器：上下移动一行（单行字段不绑定这两个动作）。
    Up {
        extend: bool,
    },
    Down {
        extend: bool,
    },
    Start {
        extend: bool,
    },
    End {
        extend: bool,
    },
    /// 缓冲区首尾：多行编辑器里对应 Ctrl+Home / Ctrl+End。
    TextStart {
        extend: bool,
    },
    TextEnd {
        extend: bool,
    },
    SelectAll,
    Backspace {
        word: bool,
    },
    Delete {
        word: bool,
    },
    Undo,
    Redo,
}

/// A platform range resolved against one tree's text-run identities.
/// Transient byte offsets are applied after the host acquires the text session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextInputSelection {
    anchor: usize,
    focus: usize,
}

#[derive(Clone)]
pub struct TextInputSnapshot(
    pub(crate) Arc<TextInputVisual>,
    TextVAlign,
    Option<(Point, bool)>,
    Option<Arc<SnapshotSelection>>,
    Option<(f32, f32)>,
);

#[derive(Debug, PartialEq)]
pub(crate) struct SnapshotSelection {
    range: std::ops::Range<usize>,
    rectangles: Vec<Rect>,
}

#[derive(Clone)]
pub(crate) struct TextInputVisual {
    pub layout: Layout<()>,
    pub selection: Vec<Rect>,
    pub caret: Option<Rect>,
    pub compose: Vec<Rect>,
}

impl Default for TextInputSnapshot {
    fn default() -> Self {
        Self(
            Arc::new(TextInputVisual {
                layout: Layout::default(),
                selection: Vec::new(),
                caret: None,
                compose: Vec::new(),
            }),
            TextVAlign::Center,
            None,
            None,
            None,
        )
    }
}

impl fmt::Debug for TextInputSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextInputSnapshot")
            .field("caret", &self.0.caret)
            .field("selection", &self.selection_rectangles())
            .field("vertical_alignment", &self.1)
            .field("scroll_policy", &self.2)
            .field("horizontal_insets", &self.4)
            .finish()
    }
}
impl PartialEq for TextInputSnapshot {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
            && self.1 == other.1
            && self.2 == other.2
            && self.3 == other.3
            && self.4 == other.4
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextInputViewport {
    pub bounds: Rect,
    pub clip: Rect,
    pub scroll_x: f32,
    /// 纵向滚动偏移：多行编辑器内容高于值框时可由宿主滚轮控制或跟随光标。
    pub scroll_y: f32,
    pub origin: Point,
    pub caret: Option<Rect>,
}

/// Logical decoration geometry derived from the same snapshot as shaped glyphs.
pub struct TextDecorationRects {
    pub selection: Vec<Rect>,
    pub compose: Vec<Rect>,
    pub caret: Option<Rect>,
}

impl TextInputViewport {
    pub fn to_layout(self, point: Point) -> Point {
        Point::new(point.x - self.origin.x, point.y - self.origin.y)
    }
}

/// 文本布局的水平内边距：值框两侧各留这么多，主机给出的折行宽度也用它。
fn text_input_padding(bounds: Rect) -> f32 {
    8.0_f32.min(bounds.width * 0.25)
}

/// 文本布局可用宽度。多行编辑器必须用这一个函数把折行宽度设进 `TextInputState`
/// （`new_multiline` / `set_wrap_width`），命中测试、光标与绘制才会共用同一份布局。
pub fn text_input_wrap_width(bounds: Rect) -> f32 {
    (bounds.width - text_input_padding(bounds) * 2.0).max(1.0)
}

fn rect(rect: parley::BoundingBox) -> Rect {
    Rect::new(
        rect.x0 as f32,
        rect.y0 as f32,
        rect.width() as f32,
        rect.height() as f32,
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextContentWidths {
    pub minimum: f32,
    pub maximum: f32,
}

impl TextInputSnapshot {
    /// Intrinsic widths at Parley's break opportunities, using the shaped fonts.
    pub fn content_widths(&self) -> TextContentWidths {
        let widths = self.0.layout.calculate_content_widths();
        TextContentWidths {
            minimum: widths.min,
            maximum: widths.max,
        }
    }

    pub fn decorations(
        &self,
        view: TextInputViewport,
        focused: bool,
        caret_visible: bool,
        scale: f32,
    ) -> TextDecorationRects {
        let placed = |rect: Rect| {
            Rect::new(
                view.origin.x + rect.x,
                view.origin.y + rect.y,
                rect.width,
                rect.height,
            )
        };
        let selection = self
            .selection_rectangles()
            .iter()
            .copied()
            .map(placed)
            .collect();
        let compose = if focused {
            let thickness = if scale.is_finite() && scale > 0.0 {
                scale.max(1.0) / scale
            } else {
                1.0
            };
            self.0
                .compose
                .iter()
                .copied()
                .map(|rect| {
                    let placed = placed(rect);
                    Rect::new(placed.x, placed.bottom() - 1.0, placed.width, thickness)
                })
                .collect()
        } else {
            Vec::new()
        };
        let caret = (focused && caret_visible)
            .then(|| {
                self.0.caret.map(|rect| {
                    let mut placed = placed(rect);
                    placed.x += CARET_VISUAL_GAP;
                    placed
                })
            })
            .flatten();
        TextDecorationRects {
            selection,
            compose,
            caret,
        }
    }

    /// Shaped text width for renderer alignment, including the active font fallback.
    pub fn layout_width(&self) -> f32 {
        self.0.layout.full_width()
    }

    /// 文本布局的实际高度：多行值框按它决定字段高度（按折行宽度排完后的真实行高）。
    pub fn layout_height(&self) -> f32 {
        self.0.layout.height()
    }

    /// Parley's first line baseline, also valid for wrapped and hard-line text.
    pub fn first_baseline(&self) -> Option<f32> {
        let baseline = self.0.layout.lines().next()?.metrics().baseline;
        baseline.is_finite().then_some(baseline)
    }

    pub(crate) fn single_line_baseline(&self) -> Option<f32> {
        let mut lines = self.0.layout.lines();
        let baseline = lines.next()?.metrics().baseline;
        (lines.next().is_none() && baseline.is_finite()).then_some(baseline)
    }

    /// Set presentation alignment without copying text or reshaping the shared layout.
    #[must_use]
    pub fn with_vertical_alignment(mut self, alignment: TextVAlign) -> Self {
        self.1 = alignment;
        self
    }

    /// Project host-owned view offset without changing editable text or its shaped layout.
    /// When `follow_caret` is false, the vertical offset may temporarily hide the caret.
    #[must_use]
    pub fn with_viewport_scroll(mut self, scroll: Point, follow_caret: bool) -> Self {
        self.2 = Some((scroll, follow_caret));
        self
    }

    /// Override single-line text presentation insets without reshaping its layout.
    /// Defaults retain the original automatic padding. Non-finite/negative
    /// values become zero; oversized values leave at least one logical pixel.
    #[must_use]
    pub fn with_horizontal_insets(mut self, left: f32, right: f32) -> Self {
        let finite = |value: f32| {
            if value.is_finite() {
                value.max(0.0)
            } else {
                0.0
            }
        };
        self.4 = Some((finite(left), finite(right)));
        self
    }

    pub fn viewport(&self, bounds: Rect, clip: Rect, old_scroll: Point) -> TextInputViewport {
        let (old_scroll, follow_caret) = self.2.unwrap_or((old_scroll, true));
        let (padding, width) = self.4.map_or_else(
            || (text_input_padding(bounds), text_input_wrap_width(bounds)),
            |(left, right)| {
                let available = (bounds.width - 1.0).max(0.0);
                let left = left.min(available);
                let right = right.min((available - left).max(0.0));
                (left, (bounds.width - left - right).max(1.0))
            },
        );
        // Layout width can exclude trailing whitespace; the editing extent must
        // include the selection focus so End and Shift+End remain visible.
        let extent = self
            .0
            .layout
            .full_width()
            .max(self.0.caret.map_or(0.0, |r| r.right() + CARET_VISUAL_GAP));
        let limit = (extent + 2.0 - width).max(0.0);
        let mut scroll_x = old_scroll.x.clamp(0.0, limit);
        if let Some(caret) = self.0.caret {
            if caret.x < scroll_x {
                scroll_x = caret.x;
            }
            if caret.right() + CARET_VISUAL_GAP > scroll_x + width {
                scroll_x = caret.right() + CARET_VISUAL_GAP - width;
            }
        }
        scroll_x = scroll_x.clamp(0.0, limit);
        // 纵向：内容装得下时使用显式对齐（默认居中）；装不下时顶对齐，并让光标始终留在值框内
        // （多行编辑器内容高于值框时，滚动跟着光标走，值框自身不滚动）。
        let content_height = self.0.layout.height();
        let inner_height = (bounds.height - 4.0).max(0.0);
        let limit_y = (content_height - inner_height).max(0.0);
        let mut scroll_y = old_scroll.y.clamp(0.0, limit_y);
        if let Some(caret) = self.0.caret.filter(|_| follow_caret) {
            if caret.y < scroll_y {
                scroll_y = caret.y;
            }
            if caret.bottom() > scroll_y + inner_height {
                scroll_y = caret.bottom() - inner_height;
            }
        }
        scroll_y = scroll_y.clamp(0.0, limit_y);
        let origin = Point::new(
            bounds.x + padding - scroll_x,
            if limit_y > 0.0 {
                bounds.y + 2.0 - scroll_y
            } else {
                match self.1 {
                    TextVAlign::Start => bounds.y + 2.0,
                    TextVAlign::Center | TextVAlign::Baseline => {
                        bounds.y + (bounds.height - content_height).max(0.0) * 0.5
                    }
                    TextVAlign::End => bounds.y + 2.0 + (inner_height - content_height).max(0.0),
                }
            },
        );
        let inner = Rect::new(bounds.x + padding, bounds.y + 2.0, width, inner_height);
        let clip = inner.intersection(clip).unwrap_or_default();
        let caret = self
            .0
            .caret
            .map(|r| Rect::new(r.x + origin.x, r.y + origin.y, r.width, r.height));
        TextInputViewport {
            bounds,
            clip,
            scroll_x,
            scroll_y,
            origin,
            caret,
        }
    }
}

/// The only editable buffer; Parley's layout is a derived view of this buffer.
#[derive(Clone)]
pub struct TextInputState {
    editor: PlainEditor<()>,
    marks: Vec<TextInputMark>,
    password: bool,
    read_only: bool,
    max_bytes: usize,
    legacy_accessibility: Option<parley::layout::LayoutAccessibility>,
    undo: Vec<EditSnapshot>,
    redo: Vec<EditSnapshot>,
    compose_before: Option<PlainEditor<()>>,
    group: Option<TextEditKind>,
    visual_source: Option<(
        parley::editing::Generation,
        Option<f32>,
        TextMeasurementRevision,
    )>,
    /// `Some` = 多行编辑器：按这个宽度折行，硬换行是普通字符；`None` = 单行字段。
    wrap_width: Option<f32>,
    visual: TextInputSnapshot,
}

impl fmt::Debug for TextInputState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextInputState")
            .field(
                "text",
                &if self.password {
                    "<redacted>"
                } else {
                    self.text()
                },
            )
            .field("composing", &self.is_composing())
            .finish()
    }
}
impl PartialEq for TextInputState {
    fn eq(&self, other: &Self) -> bool {
        self.password == other.password
            && self.read_only == other.read_only
            && self.max_bytes == other.max_bytes
            && self.editor.raw_text() == other.editor.raw_text()
            && self.editor.raw_selection().anchor() == other.editor.raw_selection().anchor()
            && self.editor.raw_selection().focus() == other.editor.raw_selection().focus()
            && self.editor.raw_compose() == other.editor.raw_compose()
            && self.marks == other.marks
    }
}
impl Default for TextInputState {
    fn default() -> Self {
        Self::new(13.0, "sans-serif")
    }
}

impl TextInputState {
    pub fn new(font_size: f32, font_stack: &str) -> Self {
        let font_size = if font_size.is_finite() {
            font_size.clamp(8.0, 48.0)
        } else {
            13.0
        };
        let mut editor = PlainEditor::new(font_size);
        editor.set_quantize(false);
        editor
            .edit_styles()
            .insert(StyleProperty::FontStack(FontStack::Source(Cow::Owned(
                font_stack.to_owned(),
            ))));
        editor
            .edit_styles()
            .insert(StyleProperty::LineHeight(LineHeight::Absolute(
                font_size * 1.45,
            )));
        Self {
            editor,
            marks: Vec::new(),
            password: false,
            read_only: false,
            max_bytes: TEXT_INPUT_MAX_BYTES,
            legacy_accessibility: None,
            undo: Vec::new(),
            redo: Vec::new(),
            compose_before: None,
            group: None,
            visual_source: None,
            wrap_width: None,
            visual: TextInputSnapshot::default(),
        }
    }
    /// A single-line secret buffer with masked snapshots and an atomic edit limit.
    pub fn new_password(font_size: f32, font_stack: &str, max_bytes: usize) -> Self {
        Self {
            password: true,
            max_bytes: max_bytes.clamp(1, TEXT_INPUT_MAX_BYTES),
            ..Self::new(font_size, font_stack)
        }
    }

    #[must_use]
    pub fn is_password(&self) -> bool {
        self.password
    }

    /// Masked metadata for password widgets and platform values, one mark per grapheme.
    pub fn masked_value(&self) -> String {
        "•".repeat(self.text().graphemes(true).count())
    }

    pub fn text(&self) -> &str {
        self.compose_before
            .as_ref()
            .unwrap_or(&self.editor)
            .raw_text()
    }

    /// Committed UTF-8 byte positions. Composition has a separate working buffer;
    /// passwords and active composition deliberately expose no selection positions.
    pub fn inspection_selection(&self) -> Option<(usize, usize)> {
        (!self.password && !self.is_composing()).then(|| {
            let selection = self.editor.raw_selection();
            (selection.anchor().index(), selection.focus().index())
        })
    }
    pub fn is_empty(&self) -> bool {
        self.text().is_empty()
    }
    /// 多行编辑器：允许硬换行，并按 `wrap_width` 折行。折行宽度必须来自
    /// `text_input_wrap_width`（与绘制、命中测试同一份换算），否则光标与排版会错位。
    pub fn new_multiline(font_size: f32, font_stack: &str, wrap_width: f32) -> Self {
        let mut state = Self::new(font_size, font_stack);
        state.wrap_width = Some(wrap_width.max(1.0));
        state
    }
    pub fn is_multiline(&self) -> bool {
        self.wrap_width.is_some()
    }

    /// 值框宽度变化时同步折行宽度（单行字段不受影响）；布局立即按新宽度重排。
    /// 返回折行宽度是否真的变了：宿主据此决定要不要按新排版重建一帧。
    pub fn set_wrap_width(&mut self, context: &mut TextInputContext, wrap_width: f32) -> bool {
        let Some(current) = self.wrap_width else {
            return false;
        };
        let wrap_width = wrap_width.max(1.0);
        if current == wrap_width {
            return false;
        }
        crate::trace::instant("gpui.text", "text.wrap_width.changed");
        self.wrap_width = Some(wrap_width);
        self.refresh(context);
        true
    }
    pub fn is_composing(&self) -> bool {
        self.compose_before.is_some()
    }
    pub fn selected_text(&self) -> Option<&str> {
        self.editor.selected_text()
    }
    pub fn snapshot(&self) -> TextInputSnapshot {
        self.visual.clone()
    }
    pub fn end_group(&mut self) {
        self.group = None;
    }

    pub fn refresh(&mut self, context: &mut TextInputContext) {
        let _trace = crate::trace::span("gpui.text", "text.input.refresh");
        let fonts = context.measurement_revision();
        if self
            .visual_source
            .as_ref()
            .is_some_and(|(generation, width, source)| {
                *generation == self.editor.generation()
                    && *width == self.wrap_width
                    && source == &fonts
            })
            && self.editor.try_layout().is_some()
        {
            // A frame refresh is not an edit. Retain the complete immutable
            // evidence used for hit positions and guarded input dispatch.
            return;
        }
        if self
            .visual_source
            .as_ref()
            .is_none_or(|(_, _, source)| source != &fonts)
        {
            self.editor.set_scale(1.0);
        }
        // 单行字段保持 `None`（不折行，与既有行为一致）；多行编辑器按值框宽度折行。
        if self
            .visual_source
            .as_ref()
            .is_none_or(|(_, width, _)| *width != self.wrap_width)
        {
            self.editor.set_width(self.wrap_width);
        }
        self.editor
            .refresh_layout(&mut context.fonts, &mut context.layout);
        self.visual_source = Some((self.editor.generation(), self.wrap_width, fonts));
        let layout = self.editor.try_layout().cloned().unwrap_or_default();
        let selection = self
            .editor
            .selection_geometry()
            .into_iter()
            .map(|(r, _)| rect(r))
            .collect();
        let compose = self
            .editor
            .raw_compose()
            .as_ref()
            .map(|range| {
                let selection = parley::editing::Selection::new(
                    parley::editing::Cursor::from_byte_index(
                        &layout,
                        range.start,
                        parley::Affinity::Downstream,
                    ),
                    parley::editing::Cursor::from_byte_index(
                        &layout,
                        range.end,
                        parley::Affinity::Upstream,
                    ),
                );
                selection
                    .geometry(&layout)
                    .into_iter()
                    .map(|(r, _)| rect(r))
                    .collect()
            })
            .unwrap_or_default();
        self.visual = TextInputSnapshot(
            Arc::new(TextInputVisual {
                layout,
                selection,
                caret: self.editor.cursor_geometry(1.0).map(rect),
                compose,
            }),
            TextVAlign::Center,
            None,
            None,
            None,
        );
        if self.password {
            let masked = self.masked_editor(context);
            self.visual = masked.snapshot();
            if let Some(range) = self.editor.raw_compose() {
                let source = self.editor.raw_text();
                let index = |byte| {
                    source
                        .grapheme_indices(true)
                        .take_while(|(i, _)| *i < byte)
                        .count()
                        * "•".len()
                };
                let layout = &self.visual.0.layout;
                let selection = parley::editing::Selection::new(
                    parley::editing::Cursor::from_byte_index(
                        layout,
                        index(range.start),
                        parley::Affinity::Downstream,
                    ),
                    parley::editing::Cursor::from_byte_index(
                        layout,
                        index(range.end),
                        parley::Affinity::Upstream,
                    ),
                );
                let compose = selection
                    .geometry(layout)
                    .into_iter()
                    .map(|(r, _)| rect(r))
                    .collect();
                Arc::make_mut(&mut self.visual.0).compose = compose;
            }
        }
    }

    fn projection_editor(&self) -> Self {
        let mut projection = Self::new(13.0, "sans-serif");
        projection.editor = self.editor.clone();
        projection.wrap_width = self.wrap_width;
        projection
    }

    fn masked_editor(&self, context: &mut TextInputContext) -> Self {
        let source = self.editor.raw_text();
        let index = |byte| {
            source
                .grapheme_indices(true)
                .take_while(|(i, _)| *i < byte)
                .count()
                * "•".len()
        };
        let anchor = index(self.editor.raw_selection().anchor().index());
        let focus = index(self.editor.raw_selection().focus().index());
        let mut projection = self.projection_editor();
        projection
            .editor
            .set_text(&"•".repeat(source.graphemes(true).count()));
        projection
            .editor
            .driver(&mut context.fonts, &mut context.layout)
            .select_byte_range(anchor, focus);
        projection.refresh(context);
        projection
    }

    /// Explicitly reveal the buffer for a host-authorized display. Default snapshots stay masked.
    pub fn revealed_snapshot(&self, context: &mut TextInputContext) -> TextInputSnapshot {
        let mut projection = self.projection_editor();
        projection.refresh(context);
        projection.snapshot()
    }

    pub fn reset(
        &mut self,
        context: &mut TextInputContext,
        value: &str,
    ) -> Result<(), TextInputError> {
        let value = if self.read_only {
            // Read-only copy must retain source bytes rather than normalize
            // line endings or tabs like editable field input.
            Cow::Borrowed(value)
        } else {
            Cow::Owned(normalized_input(value, self.is_multiline())?)
        };
        if value.len() > self.max_bytes {
            return Err(TextInputError::TooLong);
        }
        self.editor.set_text(&value);
        self.marks.clear();
        self.compose_before = None;
        self.undo.clear();
        self.redo.clear();
        self.group = None;
        let mut driver = self.editor.driver(&mut context.fonts, &mut context.layout);
        if self.read_only {
            driver.select_byte_range(0, 0);
        } else {
            driver.move_to_text_end();
        }
        self.refresh(context);
        Ok(())
    }

    pub fn insert(
        &mut self,
        context: &mut TextInputContext,
        text: &str,
        kind: TextEditKind,
    ) -> Result<bool, TextInputError> {
        if self.read_only {
            return Err(TextInputError::ReadOnly);
        }
        if self.is_composing() {
            return Err(TextInputError::Composing);
        }
        let text = normalized_input(text, self.is_multiline())?;
        let before = self.edit_snapshot();
        self.expand_mark_selection(context);
        self.snap_navigation_selection(context, None);
        let selected = self.editor.raw_selection().text_range().len();
        if self.editor.raw_text().len() - selected + text.len() > self.max_bytes {
            // Range correction belongs to the edit; a rejected edit preserves
            // the exact original cursor and its visual affinity.
            self.editor = before.editor;
            return Err(TextInputError::TooLong);
        }
        let range = self.editor.raw_selection().text_range();
        self.editor
            .driver(&mut context.fonts, &mut context.layout)
            .insert_or_replace_selection(&text);
        let changed = self.record(before, kind, range, text.len());
        self.refresh(context);
        Ok(changed)
    }

    pub fn act(&mut self, context: &mut TextInputContext, action: TextInputAction) -> bool {
        if self.read_only
            && matches!(
                action,
                TextInputAction::Backspace { .. }
                    | TextInputAction::Delete { .. }
                    | TextInputAction::Undo
                    | TextInputAction::Redo
            )
        {
            return false;
        }
        if self.is_composing() {
            return false;
        }
        if self.password
            && !matches!(
                action,
                TextInputAction::Backspace { .. }
                    | TextInputAction::Delete { .. }
                    | TextInputAction::Undo
                    | TextInputAction::Redo
            )
        {
            let mut masked = self.masked_editor(context);
            masked.act(context, action);
            return self.apply_masked_selection(context, &masked);
        }
        self.refresh(context);
        if matches!(action, TextInputAction::Undo | TextInputAction::Redo) {
            let (source, destination) = if action == TextInputAction::Undo {
                (&mut self.undo, &mut self.redo)
            } else {
                (&mut self.redo, &mut self.undo)
            };
            let Some(previous) = source.pop() else {
                return false;
            };
            destination.push(EditSnapshot {
                editor: std::mem::replace(&mut self.editor, previous.editor),
                marks: std::mem::replace(&mut self.marks, previous.marks),
            });
            self.trim_history();
            self.editor.set_scale(1.0);
            self.group = None;
            self.refresh(context);
            return true;
        }
        let before = self.edit_snapshot();
        let range = self.editor.raw_selection().text_range();
        let previous_focus = self.editor.raw_selection().focus().index();
        let backwards = matches!(action, TextInputAction::Backspace { word: false });
        let deletion = if range.is_empty()
            && matches!(
                action,
                TextInputAction::Backspace { word: false }
                    | TextInputAction::Delete { word: false }
            ) {
            let index = range.start;
            if backwards {
                self.editor.raw_text()[..index]
                    .grapheme_indices(true)
                    .next_back()
                    .map(|(start, _)| start..index)
            } else {
                self.editor.raw_text()[index..]
                    .graphemes(true)
                    .next()
                    .map(|text| index..index + text.len())
            }
        } else {
            None
        };
        let text_len = self.editor.raw_text().len();
        let multiline = self.is_multiline();
        let mut driver = self.editor.driver(&mut context.fonts, &mut context.layout);
        match action {
            TextInputAction::Left { extend, word } => match (extend, word) {
                (false, false) => driver.move_left(),
                (false, true) => driver.move_word_left(),
                (true, false) => driver.select_left(),
                (true, true) => driver.select_word_left(),
            },
            TextInputAction::Right { extend, word } => match (extend, word) {
                (false, false) => driver.move_right(),
                (false, true) => driver.move_word_right(),
                (true, false) => driver.select_right(),
                (true, true) => driver.select_word_right(),
            },
            // Parley's text-start/end movement preserves x on a single line. Resolve
            // buffer endpoints by byte index; multiline Home/End remain line-local.
            TextInputAction::Up { extend: false } => driver.move_up(),
            TextInputAction::Up { extend: true } => driver.select_up(),
            TextInputAction::Down { extend: false } => driver.move_down(),
            TextInputAction::Down { extend: true } => driver.select_down(),
            TextInputAction::Start { extend: false } if multiline => driver.move_to_line_start(),
            TextInputAction::Start { extend: true } if multiline => driver.select_to_line_start(),
            TextInputAction::End { extend: false } if multiline => driver.move_to_line_end(),
            TextInputAction::End { extend: true } if multiline => driver.select_to_line_end(),
            TextInputAction::Start { extend: false }
            | TextInputAction::TextStart { extend: false } => driver.select_byte_range(0, 0),
            TextInputAction::Start { extend: true }
            | TextInputAction::TextStart { extend: true } => driver.extend_selection_to_byte(0),
            TextInputAction::End { extend: false } | TextInputAction::TextEnd { extend: false } => {
                driver.select_byte_range(text_len, text_len)
            }
            TextInputAction::End { extend: true } | TextInputAction::TextEnd { extend: true } => {
                driver.extend_selection_to_byte(text_len)
            }
            TextInputAction::SelectAll => driver.select_all(),
            TextInputAction::Backspace { word: true } => driver.backdelete_word(),
            TextInputAction::Delete { word: true } => driver.delete_word(),
            TextInputAction::Backspace { word: false }
            | TextInputAction::Delete { word: false } => {
                if let Some(range) = deletion {
                    driver.select_byte_range(range.start, range.end);
                }
                driver.delete_selection();
            }
            TextInputAction::Undo | TextInputAction::Redo => unreachable!(),
        }
        match action {
            TextInputAction::Backspace { .. } => {
                let range = self.deleted_mark_range(context, &before);
                self.record(before, TextEditKind::Backspace, range, 0);
            }
            TextInputAction::Delete { .. } => {
                let range = self.deleted_mark_range(context, &before);
                self.record(before, TextEditKind::Delete, range, 0);
            }
            _ => self.group = None,
        }
        self.snap_navigation_selection(context, Some(previous_focus));
        self.refresh(context);
        true
    }

    pub fn point(
        &mut self,
        context: &mut TextInputContext,
        point: Point,
        extend: bool,
        word: bool,
    ) -> bool {
        if self.password {
            if self.is_composing() {
                return false;
            }
            let mut masked = self.masked_editor(context);
            masked.point(context, point, extend, word);
            return self.apply_masked_selection(context, &masked);
        }
        self.point_revealed(context, point, extend, word)
    }

    fn apply_masked_selection(&mut self, context: &mut TextInputContext, masked: &Self) -> bool {
        let source = self.editor.raw_text();
        let index = |byte: usize| {
            source
                .grapheme_indices(true)
                .nth(byte / "•".len())
                .map_or(source.len(), |(i, _)| i)
        };
        let anchor = index(masked.editor.raw_selection().anchor().index());
        let focus = index(masked.editor.raw_selection().focus().index());
        let before = *self.editor.raw_selection();
        self.group = None;
        self.editor
            .driver(&mut context.fonts, &mut context.layout)
            .select_byte_range(anchor, focus);
        let changed = *self.editor.raw_selection() != before;
        self.refresh(context);
        changed
    }

    /// Pointer geometry for an explicitly revealed projection.
    pub fn point_revealed(
        &mut self,
        context: &mut TextInputContext,
        point: Point,
        extend: bool,
        word: bool,
    ) -> bool {
        if self.is_composing() {
            return false;
        }
        self.group = None;
        let before = *self.editor.raw_selection();
        let mut driver = self.editor.driver(&mut context.fonts, &mut context.layout);
        if word {
            driver.select_word_at_point(point.x, point.y);
        } else if extend {
            driver.extend_selection_to_point(point.x, point.y);
        } else {
            driver.move_to_point(point.x, point.y);
        }
        let after = self.editor.raw_selection();
        let changed = before.anchor() != after.anchor()
            || before.focus() != after.focus()
            || self
                .visual_source
                .as_ref()
                .is_none_or(|(_, _, source)| source != &context.measurement_revision());
        if changed {
            self.snap_navigation_selection(context, None);
            self.refresh(context);
        }
        changed
    }

    pub fn preedit(
        &mut self,
        context: &mut TextInputContext,
        text: &str,
        cursor: Option<(usize, usize)>,
    ) -> Result<(), TextInputError> {
        if self.read_only {
            return Err(TextInputError::ReadOnly);
        }
        if text.is_empty() {
            self.cancel_compose(context);
            return Ok(());
        }
        let text = normalized_input(text, self.is_multiline())?;
        let original = self.compose_before.as_ref().unwrap_or(&self.editor);
        if original.raw_text().len() - original.raw_selection().text_range().len() + text.len()
            > self.max_bytes
        {
            return Err(TextInputError::TooLong);
        }
        if self.compose_before.is_none() {
            self.expand_mark_selection(context);
            self.snap_navigation_selection(context, None);
            self.end_group();
            self.compose_before = Some(self.editor.clone());
        }
        let clamp = |index: usize| {
            let mut index = index.min(text.len());
            while !text.is_char_boundary(index) {
                index -= 1;
            }
            index
        };
        let cursor = cursor.map(|(a, b)| (clamp(a), clamp(b)));
        self.editor
            .driver(&mut context.fonts, &mut context.layout)
            .set_compose(&text, cursor);
        self.refresh(context);
        Ok(())
    }

    pub fn cancel_compose(&mut self, context: &mut TextInputContext) {
        if let Some(before) = self.compose_before.take() {
            self.editor = before;
            self.editor.set_scale(1.0);
            self.refresh(context);
        }
    }

    pub fn commit_ime(
        &mut self,
        context: &mut TextInputContext,
        text: &str,
    ) -> Result<bool, TextInputError> {
        if self.read_only {
            return Err(TextInputError::ReadOnly);
        }
        if text.is_empty() {
            self.cancel_compose(context);
            return Ok(false);
        }
        let text = normalized_input(text, self.is_multiline())?;
        let original = self.compose_before.as_ref().unwrap_or(&self.editor);
        if original.raw_text().len() - original.raw_selection().text_range().len() + text.len()
            > self.max_bytes
        {
            return Err(TextInputError::TooLong);
        }
        self.cancel_compose(context);
        self.end_group();
        self.insert(context, &text, TextEditKind::Ime)
    }

    fn record(
        &mut self,
        before: EditSnapshot,
        kind: TextEditKind,
        range: std::ops::Range<usize>,
        inserted: usize,
    ) -> bool {
        self.move_marks(range, inserted);
        if before.editor.raw_text() == self.editor.raw_text() && before.marks == self.marks {
            return false;
        }
        let coalesce = matches!(
            kind,
            TextEditKind::Typing | TextEditKind::Backspace | TextEditKind::Delete
        );
        if !coalesce || self.group != Some(kind) {
            self.undo.push(before);
        }
        self.redo.clear();
        self.trim_history();
        self.group = coalesce.then_some(kind);
        true
    }

    fn trim_history(&mut self) {
        while self.undo.len() + self.redo.len() > HISTORY_STEPS
            || self
                .undo
                .iter()
                .chain(&self.redo)
                .map(|entry| {
                    entry.editor.raw_text().len()
                        + entry.marks.iter().map(|mark| mark.id.len()).sum::<usize>()
                })
                .sum::<usize>()
                > HISTORY_TEXT_BYTES
        {
            if !self.undo.is_empty() {
                self.undo.remove(0);
            } else {
                self.redo.remove(0);
            }
        }
    }
}

/// 输入规整：单行字段把换行与制表符折成空格；多行编辑器保留硬换行（CRLF 归一为 LF，
/// 制表符仍按空格排版），两者共用同一份字节上限。
fn normalized_input(text: &str, multiline: bool) -> Result<String, TextInputError> {
    if text.len() > TEXT_INPUT_MAX_BYTES {
        return Err(TextInputError::TooLong);
    }
    if multiline {
        return Ok(text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\t', " "));
    }
    Ok(text.replace("\r\n", " ").replace(['\r', '\n', '\t'], " "))
}

#[cfg(test)]
#[path = "text_input_tests.rs"]
mod tests;
