use vesta_gpui::{
    Dimension, Element, EllipsisError, Rect, Semantic, SemanticRole, Style, TextHAlign,
    TextInputContext, TextInputState, TextSource, TextVAlign, text_input_wrap_width,
};

use crate::{WidgetIdentity, WidgetTheme};

/// The source of display text. Hosts resolve localized copy before projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextContent<'a> {
    /// User data, identifiers, or other text that must be shown verbatim.
    Literal(&'a str),
    /// Product copy with a stable resource key and the current locale's value.
    Localized { key: &'a str, value: &'a str },
}

impl<'a> TextContent<'a> {
    #[must_use]
    pub const fn value(self) -> &'a str {
        match self {
            Self::Literal(value) | Self::Localized { value, .. } => value,
        }
    }

    #[must_use]
    pub const fn localization_key(self) -> Option<&'a str> {
        match self {
            Self::Literal(_) => None,
            Self::Localized { key, .. } => Some(key),
        }
    }
}

/// Projects a display label while keeping widget identity separate from copy identity.
#[must_use]
pub fn text_label(
    identity: WidgetIdentity,
    content: TextContent<'_>,
    theme: WidgetTheme,
) -> Element {
    let value = content.value();
    Element::text(identity.element, identity.key, value)
        .text_source(match content {
            TextContent::Literal(_) => TextSource::Literal,
            TextContent::Localized { key, .. } => TextSource::Localized {
                key: key.to_owned(),
            },
        })
        .semantic(Semantic::new(SemanticRole::Label, value))
        .styled(
            Style::default()
                .size(Dimension::Auto, Dimension::Auto)
                .text(theme.typography.font_size, theme.palette.text)
                .text_align(TextHAlign::Start, TextVAlign::Start)
                .text_wrap(true)
                .pointer_events(false),
        )
}

/// Single-line display at the caller's exact outer width. Full source/provenance
/// remain accessible; only the derived glyph snapshot is ellipsized. Width must
/// be finite and nonnegative, and includes the shared text viewport's insets.
pub fn ellipsized_label(
    identity: WidgetIdentity,
    content: TextContent<'_>,
    width: f32,
    theme: WidgetTheme,
    context: &mut TextInputContext,
) -> Result<Element, EllipsisError> {
    if !width.is_finite() || width < 0.0 {
        return Err(EllipsisError::InvalidWidth);
    }
    let inner_width = if width == 0.0 {
        0.0
    } else {
        text_input_wrap_width(Rect::new(0.0, 0.0, width, 0.0))
    };
    let shaped = context.shape_static_text_ellipsized(
        content.value(),
        theme.typography.font_size,
        vesta_gpui::FontRole::Ui,
        inner_width,
    )?;
    let snapshot = shaped.snapshot().with_vertical_alignment(TextVAlign::Start);
    let height = snapshot
        .layout_height()
        .max(theme.typography.font_size * 1.5)
        + 4.0;
    Ok(
        Element::text_input(identity.element, identity.key, snapshot, false)
            .text_source(match content {
                TextContent::Literal(_) => TextSource::Literal,
                TextContent::Localized { key, .. } => TextSource::Localized {
                    key: key.to_owned(),
                },
            })
            .semantic(Semantic::new(SemanticRole::Label, content.value()))
            .styled(
                Style::default()
                    .size(Dimension::Px(width), Dimension::Px(height.max(4.0)))
                    .min_width(Dimension::Px(0.0))
                    .text(theme.typography.font_size, theme.palette.text)
                    .pointer_events(false),
            ),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectableLabelError {
    WritableBuffer,
    SourceMismatch,
}

impl std::fmt::Display for SelectableLabelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "selectable label: {self:?}")
    }
}

impl std::error::Error for SelectableLabelError {}

/// Caller-owned read-only selection, using the editor's public geometry and the
/// host's injected clipboard bridge. This full-source view does not ellipsize.
pub fn selectable_label(
    identity: WidgetIdentity,
    content: TextContent<'_>,
    state: &TextInputState,
    focused: bool,
    theme: WidgetTheme,
) -> Result<Element, SelectableLabelError> {
    if !state.is_read_only() {
        return Err(SelectableLabelError::WritableBuffer);
    }
    if state.text() != content.value() {
        return Err(SelectableLabelError::SourceMismatch);
    }
    let snapshot = state.snapshot().with_vertical_alignment(TextVAlign::Start);
    let height = snapshot
        .layout_height()
        .max(theme.typography.font_size * 1.5)
        + 4.0;
    Ok(
        Element::text_input(identity.element, identity.key, snapshot, focused)
            .text_source(match content {
                TextContent::Literal(_) => TextSource::Literal,
                TextContent::Localized { key, .. } => TextSource::Localized { key: key.into() },
            })
            .semantic(
                Semantic::new(
                    if state.is_multiline() {
                        SemanticRole::MultilineTextInput
                    } else {
                        SemanticRole::TextInput
                    },
                    content.value(),
                )
                .value(content.value())
                .read_only(true),
            )
            .styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Px(height))
                    .min_width(Dimension::Px(0.0))
                    .text(theme.typography.font_size, theme.palette.text)
                    .focusable(true),
            ),
    )
}
