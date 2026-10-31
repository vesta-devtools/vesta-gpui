/// An sRGB color with straight alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

/// 元素裁剪模式：默认 Clip，以自身矩形裁剪绘制和子级；Inherit 显式继承祖先裁剪；
/// NoClip 浮层不受父级裁剪，改以自身矩形为边界（用于悬停菜单等浮层）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClipMode {
    Inherit,
    #[default]
    Clip,
    NoClip,
}

impl Color {
    #[must_use]
    pub const fn srgba(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    #[must_use]
    pub const fn rgb8(red: u8, green: u8, blue: u8) -> Self {
        Self::srgba(
            red as f32 / 255.0,
            green as f32 / 255.0,
            blue as f32 / 255.0,
            1.0,
        )
    }
}

/// A declarative size constraint.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Dimension {
    #[default]
    Auto,
    Px(f32),
    /// A fraction where `1.0` represents one hundred percent.
    Percent(f32),
}

/// Main layout axis for a container.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Axis {
    Row,
    #[default]
    Column,
}

/// Whether a Flex container starts additional lines on its cross axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

/// Container flow and logical horizontal scrolling. Glyph order belongs to
/// shaping; images and absolute coordinates remain physical.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FlowDirection {
    #[default]
    LeftToRight,
    RightToLeft,
}

impl FlowDirection {
    #[must_use]
    pub const fn horizontal(self, value: f32) -> f32 {
        match self {
            Self::LeftToRight => value,
            Self::RightToLeft => -value,
        }
    }
}

/// A CSS Grid track in logical pixels, available space fractions, or content size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridTrack {
    Auto,
    Px(f32),
    Fr(f32),
}

/// One-based grid line and the number of tracks occupied by an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridCell {
    pub start: Option<u16>,
    pub span: u16,
}

impl Default for GridCell {
    fn default() -> Self {
        Self {
            start: None,
            span: 1,
        }
    }
}

impl GridCell {
    #[must_use]
    pub const fn new(start: Option<u16>, span: u16) -> Self {
        Self { start, span }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PositionMode {
    #[default]
    Relative,
    Absolute,
}

/// Cross-axis alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    /// First baseline for items; unavailable for line `align_content`.
    Baseline,
    #[default]
    Stretch,
}

/// Horizontal text alignment inside the element's paint bounds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextHAlign {
    #[default]
    Start,
    Center,
    End,
}

/// Vertical text alignment inside the element's paint bounds. Defaults to `Center`
/// because single-line UI text (labels, inputs, buttons) is expected to sit in the
/// vertical middle of its fixed-height element.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextVAlign {
    Start,
    #[default]
    Center,
    /// Align single-line static text to a centered reference line for its font role.
    Baseline,
    End,
}

/// Semantic font selection without exposing host-specific font handles to `vesta-gpui`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontRole {
    #[default]
    Ui,
    /// Host-provided fixed-width font for terminal and code-oriented surfaces.
    Mono,
    MonoBold,
    MonoItalic,
    MonoBoldItalic,
    MonoCjk,
    Icon,
}

/// Main-axis alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

/// Logical padding applied by the layout adapter.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Insets {
    #[must_use]
    pub const fn all(value: f32) -> Self {
        Self {
            left: value,
            right: value,
            top: value,
            bottom: value,
        }
    }

    #[must_use]
    pub const fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self {
            left: horizontal,
            right: horizontal,
            top: vertical,
            bottom: vertical,
        }
    }
}

/// Visual properties consumed by the paint-list builder.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualStyle {
    pub background: Option<Color>,
    pub border_color: Option<Color>,
    pub border_width: f32,
    pub corner_radius: f32,
    pub text_color: Color,
    pub font_size: f32,
    pub font_role: FontRole,
}

impl Default for VisualStyle {
    fn default() -> Self {
        Self {
            background: None,
            border_color: None,
            border_width: 0.0,
            corner_radius: 0.0,
            text_color: Color::rgb8(220, 225, 232),
            font_size: 13.0,
            font_role: FontRole::Ui,
        }
    }
}

/// Local-coordinate stroke data: butt endpoints and connected bevel joins.
/// Repeating the first point closes the join; self-crossing/short runs may overlap.
#[derive(Clone, Debug, PartialEq)]
pub struct Polyline {
    pub points: Vec<crate::geometry::Point>,
    pub width: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollbarVisibility {
    #[default]
    Always,
    OnHover,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollbarGutter {
    Overlay,
    /// Reserve space on overflowing axes, independently of thumb visibility.
    #[default]
    Reserve,
}

/// Who supplies focus feedback for this element; logical focus is unaffected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FocusIndicator {
    /// The painter adds a static outline when keyboard focus is visible.
    #[default]
    Outline,
    /// The widget supplies its own editing border/caret, possibly on an ancestor.
    Control,
}

/// ScrollBox appearance and space policy; hidden thumbs retain scrolling geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarStyle {
    pub visibility: ScrollbarVisibility,
    pub gutter: ScrollbarGutter,
    pub track: Color,
    pub thumb: Color,
    pub thumb_hovered: Color,
}

impl Default for ScrollbarStyle {
    fn default() -> Self {
        Self {
            visibility: ScrollbarVisibility::Always,
            gutter: ScrollbarGutter::Reserve,
            track: Color::srgba(1.0, 1.0, 1.0, 0.14),
            thumb: Color::srgba(1.0, 1.0, 1.0, 0.78),
            thumb_hovered: Color::srgba(1.0, 1.0, 1.0, 0.78),
        }
    }
}

/// Layout-aligned pointer shape; independent of painting and child clipping.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HitShape {
    #[default]
    Rectangle,
    Ellipse,
}

/// Layout, interaction, and visual inputs for an [`Element`](crate::Element).
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    pub position: PositionMode,
    pub left: Option<f32>,
    pub top: Option<f32>,
    pub right: Option<f32>,
    pub bottom: Option<f32>,
    pub width: Dimension,
    pub height: Dimension,
    /// Width divided by height; explicit dimensions and min/max remain authoritative.
    pub aspect_ratio: Option<f32>,
    pub min_width: Dimension,
    pub min_height: Dimension,
    pub max_width: Dimension,
    pub max_height: Dimension,
    pub flex_basis: Option<Dimension>,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub axis: Axis,
    /// `None` inherits the parent's flow; the root defaults to LTR.
    pub flow_direction: Option<FlowDirection>,
    pub flex_wrap: FlexWrap,
    /// `Some` selects Taffy Grid, including an empty implicit-track grid.
    pub grid_columns: Option<Vec<GridTrack>>,
    pub grid_rows: Vec<GridTrack>,
    pub grid_column: GridCell,
    pub grid_row: GridCell,
    pub align: Align,
    /// Override the parent's cross-axis alignment for this child only.
    pub align_self: Option<Align>,
    /// Cross-axis packing of multiple Flex lines.
    pub align_content: Align,
    pub justify: Justify,
    pub gap: f32,
    pub padding: Insets,
    /// External logical-pixel spacing; unlike padding, negative values are allowed.
    pub margin: Insets,
    /// 裁剪模式：默认裁到自身矩形；Inherit 显式继承祖先；NoClip 浮层不受父裁剪。
    pub clip: ClipMode,
    pub pointer_events: bool,
    pub hit_shape: HitShape,
    /// Whether this element participates in keyboard Tab navigation.
    pub focusable: bool,
    pub focus_indicator: FocusIndicator,
    /// Keep this subtree hittable as an inert boundary, without dispatching
    /// an action registered on a containing compound widget.
    pub action_boundary: bool,
    pub z_index: i32,
    pub text_h_align: TextHAlign,
    pub text_v_align: TextVAlign,
    /// Static text may soft-wrap within its measured content width.
    pub text_wrap: bool,
    /// Rounded-rectangle fill/border rotation in radians about the element center.
    /// Text, polyline/quad points, layout and hit bounds remain in their own coordinates.
    pub rotation: Option<f32>,
    /// Local-coordinate connected stroke; hit/semantic bounds still use the layout box.
    pub polyline: Option<Polyline>,
    pub filled_quad: Option<[crate::geometry::Point; 4]>,
    pub scrollbar: ScrollbarStyle,
    pub visual: VisualStyle,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            position: PositionMode::Relative,
            left: None,
            top: None,
            right: None,
            bottom: None,
            width: Dimension::Auto,
            height: Dimension::Auto,
            aspect_ratio: None,
            min_width: Dimension::Auto,
            min_height: Dimension::Auto,
            max_width: Dimension::Auto,
            max_height: Dimension::Auto,
            flex_basis: None,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            axis: Axis::Column,
            flow_direction: None,
            flex_wrap: FlexWrap::NoWrap,
            grid_columns: None,
            grid_rows: Vec::new(),
            grid_column: GridCell::default(),
            grid_row: GridCell::default(),
            align: Align::Stretch,
            align_self: None,
            align_content: Align::Stretch,
            justify: Justify::Start,
            gap: 0.0,
            padding: Insets::default(),
            margin: Insets::default(),
            clip: ClipMode::Clip,
            pointer_events: true,
            hit_shape: HitShape::Rectangle,
            focusable: false,
            focus_indicator: FocusIndicator::Outline,
            action_boundary: false,
            z_index: 0,
            text_h_align: TextHAlign::Start,
            text_v_align: TextVAlign::Center,
            text_wrap: false,
            rotation: None,
            polyline: None,
            filled_quad: None,
            scrollbar: ScrollbarStyle::default(),
            visual: VisualStyle::default(),
        }
    }
}

impl Style {
    #[must_use]
    pub fn aspect_ratio(mut self, ratio: f32) -> Self {
        self.aspect_ratio = Some(ratio);
        self
    }

    #[must_use]
    pub fn flow_direction(mut self, direction: FlowDirection) -> Self {
        self.flow_direction = Some(direction);
        self
    }

    #[must_use]
    pub fn scrollbar(mut self, scrollbar: ScrollbarStyle) -> Self {
        self.scrollbar = scrollbar;
        self
    }

    #[must_use]
    pub fn text_wrap(mut self, enabled: bool) -> Self {
        self.text_wrap = enabled;
        self
    }

    #[must_use]
    pub fn absolute(mut self, left: f32, top: f32) -> Self {
        self.position = PositionMode::Absolute;
        self.left = Some(left);
        self.top = Some(top);
        self
    }

    #[must_use]
    pub fn inset_right(mut self, right: f32) -> Self {
        self.right = Some(right);
        self
    }

    #[must_use]
    pub fn inset_bottom(mut self, bottom: f32) -> Self {
        self.bottom = Some(bottom);
        self
    }

    #[must_use]
    pub fn size(mut self, width: Dimension, height: Dimension) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    #[must_use]
    pub fn max_height(mut self, height: Dimension) -> Self {
        self.max_height = height;
        self
    }

    #[must_use]
    pub fn min_height(mut self, height: Dimension) -> Self {
        self.min_height = height;
        self
    }

    #[must_use]
    pub fn min_width(mut self, width: Dimension) -> Self {
        self.min_width = width;
        self
    }

    #[must_use]
    pub fn max_width(mut self, width: Dimension) -> Self {
        self.max_width = width;
        self
    }

    #[must_use]
    pub fn flex(mut self, grow: f32, shrink: f32) -> Self {
        self.flex_grow = grow;
        self.flex_shrink = shrink;
        self
    }

    #[must_use]
    pub fn flex_basis(mut self, basis: Dimension) -> Self {
        self.flex_basis = Some(basis);
        self
    }

    #[must_use]
    pub fn axis(mut self, axis: Axis) -> Self {
        self.axis = axis;
        self
    }

    #[must_use]
    pub fn grid(mut self, columns: Vec<GridTrack>, rows: Vec<GridTrack>) -> Self {
        self.grid_columns = Some(columns);
        self.grid_rows = rows;
        self
    }

    #[must_use]
    pub fn grid_cell(mut self, column: GridCell, row: GridCell) -> Self {
        self.grid_column = column;
        self.grid_row = row;
        self
    }

    #[must_use]
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    #[must_use]
    pub fn align_self(mut self, align: Align) -> Self {
        self.align_self = Some(align);
        self
    }

    #[must_use]
    pub fn align_content(mut self, align: Align) -> Self {
        self.align_content = align;
        self
    }

    #[must_use]
    pub fn flex_wrap(mut self, wrap: FlexWrap) -> Self {
        self.flex_wrap = wrap;
        self
    }

    #[must_use]
    pub fn margin(mut self, margin: Insets) -> Self {
        self.margin = margin;
        self
    }

    #[must_use]
    pub fn justify(mut self, justify: Justify) -> Self {
        self.justify = justify;
        self
    }

    #[must_use]
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: Insets) -> Self {
        self.padding = padding;
        self
    }

    #[must_use]
    pub fn clipped(mut self) -> Self {
        self.clip = ClipMode::Clip;
        self
    }

    /// 浮层元素：脱离父级裁剪，以自身矩形为边界。
    #[must_use]
    pub fn no_clip(mut self) -> Self {
        self.clip = ClipMode::NoClip;
        self
    }

    /// Rotate rounded-rectangle fill/border preparation; other payloads keep their coordinates.
    #[must_use]
    pub fn rotation(mut self, radians: f32) -> Self {
        self.rotation = Some(radians);
        self
    }

    #[must_use]
    pub fn pointer_events(mut self, enabled: bool) -> Self {
        self.pointer_events = enabled;
        self
    }
    /// Narrows this element's pointer surface only; child clipping and semantic
    /// bounds still use the layout rectangle.
    pub fn hit_shape(mut self, shape: HitShape) -> Self {
        self.hit_shape = shape;
        self
    }

    #[must_use]
    pub fn focusable(mut self, enabled: bool) -> Self {
        self.focusable = enabled;
        self
    }

    #[must_use]
    pub fn focus_indicator(mut self, indicator: FocusIndicator) -> Self {
        self.focus_indicator = indicator;
        self
    }

    #[must_use]
    pub fn action_boundary(mut self, enabled: bool) -> Self {
        self.action_boundary = enabled;
        self
    }

    /// Set a connected bevel stroke. Exact repeated endpoints close its join.
    #[must_use]
    pub fn polyline(mut self, points: Vec<crate::geometry::Point>, width: f32) -> Self {
        self.polyline = Some(Polyline { points, width });
        self
    }

    /// A convex local-coordinate quad, filled with the background color.
    pub fn filled_quad(mut self, points: [crate::geometry::Point; 4]) -> Self {
        self.filled_quad = Some(points);
        self
    }

    #[must_use]
    pub fn z_index(mut self, value: i32) -> Self {
        self.z_index = value;
        self
    }

    #[must_use]
    pub fn background(mut self, color: Color) -> Self {
        self.visual.background = Some(color);
        self
    }

    #[must_use]
    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.visual.border_width = width;
        self.visual.border_color = Some(color);
        self
    }

    #[must_use]
    pub fn radius(mut self, radius: f32) -> Self {
        self.visual.corner_radius = radius;
        self
    }

    #[must_use]
    pub fn text(mut self, font_size: f32, color: Color) -> Self {
        self.visual.font_size = font_size;
        self.visual.text_color = color;
        self
    }

    #[must_use]
    pub fn font_role(mut self, role: FontRole) -> Self {
        self.visual.font_role = role;
        self
    }

    #[must_use]
    pub fn text_align(mut self, h: TextHAlign, v: TextVAlign) -> Self {
        self.text_h_align = h;
        self.text_v_align = v;
        self
    }
}
