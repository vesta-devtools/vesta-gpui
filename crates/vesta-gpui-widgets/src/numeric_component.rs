//! Component identity stays textual; color is a secondary presentation cue.
use crate::WidgetTheme;
use vesta_gpui::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumericComponent {
    X,
    Y,
    Z,
    W,
    U,
    V,
    R,
    G,
    B,
    A,
}

impl NumericComponent {
    pub const AXES: [Self; 4] = [Self::X, Self::Y, Self::Z, Self::W];
    pub const RGBA: [Self; 4] = [Self::R, Self::G, Self::B, Self::A];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::W => "W",
            Self::U => "U",
            Self::V => "V",
            Self::R => "R",
            Self::G => "G",
            Self::B => "B",
            Self::A => "A",
        }
    }

    #[must_use]
    pub fn color(self, theme: WidgetTheme, disabled: bool) -> Color {
        let mut color = match self {
            Self::X | Self::U | Self::R => theme.palette.danger,
            Self::Y | Self::V | Self::G => theme.palette.success,
            Self::Z | Self::B => match theme.mode {
                crate::ThemeMode::Dark => Color::rgb8(79, 132, 190),
                crate::ThemeMode::Light => Color::rgb8(55, 113, 179),
            },
            Self::W | Self::A => theme.palette.text_muted,
        };
        if disabled {
            color.alpha *= 0.45;
        }
        color
    }

    #[must_use]
    pub fn badge_caption(self, theme: WidgetTheme, disabled: bool) -> Color {
        let mut caption = crate::theme::caption_on_fill(self.color(theme, false));
        if disabled {
            caption.alpha *= 0.45;
        }
        caption
    }
}
