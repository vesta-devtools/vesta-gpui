//! Shared semantic palette, metrics and typography.
use crate::ControlState;
use vesta_gpui::{Color, Element, ElementKind, ScrollbarStyle, ScrollbarVisibility};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
}

/// Semantic colors. Applications can replace any token without changing widget code.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemePalette {
    /// Main document and work surface.
    pub canvas: Color,
    /// Secondary panes and read-only content surfaces.
    pub panel: Color,

    /// Window title background.
    pub chrome_title: Color,
    /// Window status background.
    pub chrome_status: Color,
    /// Rules between chrome regions.
    pub chrome_divider: Color,

    /// Popup menu background.
    pub menu_surface: Color,
    /// Highlighted menu items and selected toggle controls.
    pub menu_selected: Color,

    pub scrollbar_track: Color,
    pub scrollbar_thumb: Color,
    pub scrollbar_thumb_hovered: Color,

    /// Shared background for ordinary controls, section/table headers, and code blocks.
    pub surface: Color,
    /// Temporary pointer hover; separate from a persistent selection.
    pub surface_hovered: Color,
    pub surface_pressed: Color,
    pub surface_disabled: Color,
    /// Editable content and active chrome surfaces.
    pub field: Color,
    /// Ordinary control and content borders.
    pub border: Color,

    /// Primary action and icon emphasis.
    pub accent: Color,
    pub accent_hovered: Color,

    pub text: Color,
    pub text_muted: Color,
    pub text_disabled: Color,

    /// Keyboard focus outline.
    pub focus: Color,
    /// Persistent collection and text selection.
    pub selection: Color,

    pub danger: Color,
    pub warning: Color,
    pub success: Color,
    pub overlay: Color,
    /// Progress captions must remain readable over both fill and track.
    pub progress_fill: Color,
    /// Tooltip chrome contrasts with the workspace independently of other overlays.
    pub tooltip_surface: Color,
    pub tooltip_text: Color,
    pub tooltip_border: Color,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeMetrics {
    pub radius: f32,
    pub control_height: f32,
    pub compact_control_height: f32,
    pub spacing: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeTypography {
    pub font_size: f32,
    pub small_font_size: f32,
    pub heading_font_size: f32,
}

/// Public theme configuration shared by widgets, standalone hosts, and viewport overlays.
///
/// The separation mirrors mature desktop UI systems: semantic palette, density/shape metrics,
/// and typography remain independently replaceable instead of being hard-coded per widget.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WidgetTheme {
    pub mode: ThemeMode,
    pub palette: ThemePalette,
    pub metrics: ThemeMetrics,
    pub typography: ThemeTypography,
}

impl Default for WidgetTheme {
    fn default() -> Self {
        Self::dark()
    }
}

impl WidgetTheme {
    #[must_use]
    pub const fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            palette: ThemePalette {
                canvas: Color::rgb8(34, 36, 39),
                panel: Color::rgb8(43, 45, 49),
                chrome_title: Color::rgb8(59, 65, 77),
                chrome_status: Color::rgb8(59, 65, 77),
                chrome_divider: Color::rgb8(70, 75, 87),
                menu_surface: Color::rgb8(48, 51, 58),
                menu_selected: Color::rgb8(64, 80, 102),
                scrollbar_track: Color::srgba(0.35, 0.38, 0.43, 0.16),
                scrollbar_thumb: Color::rgb8(109, 115, 126),
                scrollbar_thumb_hovered: Color::rgb8(82, 88, 99),
                surface: Color::rgb8(55, 58, 62),
                surface_hovered: Color::rgb8(65, 68, 73),
                surface_pressed: Color::rgb8(47, 50, 54),
                surface_disabled: Color::rgb8(48, 50, 53),
                field: Color::rgb8(38, 40, 44),
                border: Color::rgb8(72, 75, 80),
                accent: Color::rgb8(79, 132, 190),
                accent_hovered: Color::rgb8(96, 149, 205),
                text: Color::rgb8(222, 224, 227),
                text_muted: Color::rgb8(166, 170, 175),
                text_disabled: Color::rgb8(113, 117, 122),
                focus: Color::rgb8(102, 158, 213),
                selection: Color::srgba(0.28, 0.48, 0.70, 0.42),
                danger: Color::rgb8(197, 93, 87),
                warning: Color::rgb8(204, 162, 83),
                success: Color::rgb8(108, 166, 111),
                overlay: Color::srgba(0.13, 0.14, 0.15, 0.96),
                progress_fill: Color::rgb8(51, 84, 124),
                tooltip_surface: Color::rgb8(224, 226, 229),
                tooltip_text: Color::rgb8(30, 32, 35),
                tooltip_border: Color::rgb8(75, 80, 88),
            },
            metrics: ThemeMetrics {
                radius: 3.0,
                control_height: 28.0,
                compact_control_height: 23.0,
                spacing: 7.0,
            },
            typography: ThemeTypography {
                font_size: 13.0,
                small_font_size: 11.0,
                heading_font_size: 18.0,
            },
        }
    }

    #[must_use]
    pub const fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            palette: ThemePalette {
                canvas: Color::rgb8(255, 255, 255),
                panel: Color::rgb8(249, 252, 253),
                chrome_title: Color::rgb8(224, 233, 244),
                chrome_status: Color::rgb8(224, 233, 244),
                chrome_divider: Color::rgb8(183, 199, 221),
                menu_surface: Color::rgb8(255, 255, 255),
                menu_selected: Color::rgb8(217, 230, 246),
                scrollbar_track: Color::srgba(0.45, 0.48, 0.50, 0.05),
                scrollbar_thumb: Color::rgb8(201, 207, 211),
                scrollbar_thumb_hovered: Color::rgb8(152, 159, 166),
                surface: Color::rgb8(226, 232, 240),
                surface_hovered: Color::rgb8(215, 225, 235),
                surface_pressed: Color::rgb8(204, 219, 238),
                surface_disabled: Color::rgb8(246, 247, 248),
                field: Color::rgb8(255, 255, 255),
                border: Color::rgb8(188, 201, 218),
                accent: Color::rgb8(26, 28, 31),
                accent_hovered: Color::rgb8(52, 55, 59),
                text: Color::rgb8(26, 28, 31),
                text_muted: Color::rgb8(82, 92, 104),
                text_disabled: Color::rgb8(164, 170, 175),
                focus: Color::rgb8(108, 120, 130),
                selection: Color::rgb8(197, 216, 239),
                danger: Color::rgb8(176, 69, 66),
                warning: Color::rgb8(145, 100, 28),
                success: Color::rgb8(59, 127, 76),
                overlay: Color::srgba(1.0, 1.0, 1.0, 0.98),
                progress_fill: Color::rgb8(190, 206, 224),
                tooltip_surface: Color::rgb8(45, 48, 53),
                tooltip_text: Color::rgb8(255, 255, 255),
                tooltip_border: Color::rgb8(190, 195, 203),
            },
            metrics: ThemeMetrics {
                radius: 3.0,
                control_height: 28.0,
                compact_control_height: 23.0,
                spacing: 7.0,
            },
            typography: ThemeTypography {
                font_size: 13.0,
                small_font_size: 11.0,
                heading_font_size: 18.0,
            },
        }
    }

    #[must_use]
    pub const fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Light => Self::light(),
        }
    }

    /// Themed scrollbars appear on area hover or while their thumb is dragged.
    #[must_use]
    pub fn scrollbar_style(self) -> ScrollbarStyle {
        ScrollbarStyle {
            visibility: ScrollbarVisibility::OnHover,
            track: self.palette.scrollbar_track,
            thumb: self.palette.scrollbar_thumb,
            thumb_hovered: self.palette.scrollbar_thumb_hovered,
            ..ScrollbarStyle::default()
        }
    }

    /// Apply this theme to a composed subtree, including raw and virtual ScrollBoxes.
    /// Explicit gutter geometry is preserved; hosts opt into the theme's hover policy.
    /// Apply local appearance overrides after this pass when a region needs its own policy.
    pub fn apply_scrollbars(self, root: &mut Element) {
        if matches!(root.kind, ElementKind::ScrollBox(_)) {
            root.style.scrollbar = ScrollbarStyle {
                gutter: root.style.scrollbar.gutter,
                ..self.scrollbar_style()
            };
        }
        for child in &mut root.children {
            self.apply_scrollbars(child);
        }
    }

    #[must_use]
    pub const fn with_accent(mut self, accent: Color, hovered: Color) -> Self {
        self.palette.accent = accent;
        self.palette.accent_hovered = hovered;
        self.palette.focus = accent;
        self
    }

    #[must_use]
    pub fn with_density(mut self, factor: f32) -> Self {
        let factor = if factor.is_finite() {
            factor.clamp(0.75, 1.5)
        } else {
            1.0
        };
        self.metrics.control_height *= factor;
        self.metrics.compact_control_height *= factor;
        self.metrics.spacing *= factor;
        self
    }

    /// Switch palette mode while preserving configured metrics and typography.
    #[must_use]
    pub const fn toggled(self) -> Self {
        let mut next = match self.mode {
            ThemeMode::Dark => Self::light(),
            ThemeMode::Light => Self::dark(),
        };
        next.metrics = self.metrics;
        next.typography = self.typography;
        next
    }
}

fn relative_luminance(color: Color) -> f32 {
    let linear = |channel: f32| {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.red) + 0.7152 * linear(color.green) + 0.0722 * linear(color.blue)
}

/// Choose the stronger black/white contrast for an opaque sRGB control fill.
pub(crate) fn caption_on_fill(fill: Color) -> Color {
    let luminance = relative_luminance(fill);
    if (luminance + 0.05) / 0.05 >= 1.05 / (luminance + 0.05) {
        Color::rgb8(0, 0, 0)
    } else {
        Color::rgb8(255, 255, 255)
    }
}

pub(crate) fn control_mark_on_fill(fill: Color) -> Color {
    let preferred = Color::rgb8(248, 250, 253);
    let fill_luminance = relative_luminance(fill);
    let mark_luminance = relative_luminance(preferred);
    let contrast =
        (fill_luminance.max(mark_luminance) + 0.05) / (fill_luminance.min(mark_luminance) + 0.05);
    if contrast >= 3.0 {
        preferred
    } else {
        caption_on_fill(fill)
    }
}

impl ThemePalette {
    pub(crate) const fn selectable_background(self, selected: bool, state: ControlState) -> Color {
        match (selected, state) {
            (false, state) => self.control_background(state),
            (true, ControlState::Disabled) => Color {
                alpha: self.selection.alpha * 0.5,
                ..self.selection
            },
            (true, _) => self.selection,
        }
    }

    #[must_use]
    pub const fn control_background(self, state: ControlState) -> Color {
        match state {
            ControlState::Idle => self.surface,
            ControlState::Hovered => self.surface_hovered,
            ControlState::Pressed => self.surface_pressed,
            ControlState::Disabled => self.surface_disabled,
        }
    }
}

impl ThemeMetrics {
    #[must_use]
    pub fn scaled(mut self, factor: f32) -> Self {
        let factor = if factor.is_finite() {
            factor.clamp(0.75, 1.5)
        } else {
            1.0
        };
        self.control_height *= factor;
        self.compact_control_height *= factor;
        self.spacing *= factor;
        self
    }
}
