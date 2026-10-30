use super::*;
use std::ops::Range;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct StyledTextSpan {
    pub range: Range<usize>,
    pub bold: bool,
    pub italic: bool,
    pub monospace: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyledTextError {
    TooLong,
    InvalidRange,
    InteractiveSnapshot,
}

/// Alignment is part of the immutable read-only glyph geometry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StyledTextAlignment {
    #[default]
    Start,
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StyledTextOptions {
    pub alignment: StyledTextAlignment,
    /// Allow emergency breaks at shaped cluster boundaries in constrained cells.
    pub break_long_words: bool,
}

impl TextInputContext {
    pub fn shape_styled_text(
        &mut self,
        text: &str,
        font_size: f32,
        role: FontRole,
        spans: &[StyledTextSpan],
        width: f32,
    ) -> Result<TextInputSnapshot, StyledTextError> {
        self.shape_styled_text_with_options(
            text,
            font_size,
            role,
            spans,
            width,
            StyledTextOptions::default(),
        )
    }

    pub fn shape_styled_text_with_options(
        &mut self,
        text: &str,
        font_size: f32,
        role: FontRole,
        spans: &[StyledTextSpan],
        width: f32,
        options: StyledTextOptions,
    ) -> Result<TextInputSnapshot, StyledTextError> {
        let _trace = crate::trace::detail_span("gpui.text", "text.shape_styled");
        if text.len() > TEXT_INPUT_MAX_BYTES {
            return Err(StyledTextError::TooLong);
        }
        if spans.iter().any(|span| {
            span.range.start > span.range.end
                || !text.is_char_boundary(span.range.start)
                || !text.is_char_boundary(span.range.end)
        }) {
            return Err(StyledTextError::InvalidRange);
        }
        let stack = self
            .font_stacks
            .get(&role)
            .cloned()
            .unwrap_or_else(|| "sans-serif".to_owned());
        let mono = self
            .font_stacks
            .get(&FontRole::Mono)
            .cloned()
            .unwrap_or_else(|| "monospace".to_owned());
        let mut builder = self.layout.ranged_builder(&mut self.fonts, text, 1.0, true);
        builder.push_default(StyleProperty::FontStack(FontStack::Source(Cow::Owned(
            stack,
        ))));
        builder.push_default(StyleProperty::FontSize(font_size));
        builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(
            font_size * 1.5,
        )));
        if options.break_long_words {
            builder.push_default(StyleProperty::OverflowWrap(parley::OverflowWrap::Anywhere));
        }
        for span in spans {
            if span.bold {
                builder.push(
                    StyleProperty::FontWeight(parley::fontique::FontWeight::BOLD),
                    span.range.clone(),
                );
            }
            if span.italic {
                builder.push(
                    StyleProperty::FontStyle(parley::fontique::FontStyle::Italic),
                    span.range.clone(),
                );
            }
            if span.monospace {
                builder.push(
                    StyleProperty::FontStack(FontStack::Source(Cow::Borrowed(&mono))),
                    span.range.clone(),
                );
            }
        }
        let mut layout = builder.build(text);
        let width = if width.is_finite() && width > 0.0 {
            width
        } else {
            1.0
        };
        layout.break_all_lines(Some(width));
        layout.align(
            Some(width),
            match options.alignment {
                StyledTextAlignment::Start => parley::Alignment::Start,
                StyledTextAlignment::Left => parley::Alignment::Left,
                StyledTextAlignment::Center => parley::Alignment::Center,
                StyledTextAlignment::Right => parley::Alignment::Right,
            },
            parley::AlignmentOptions::default(),
        );
        Ok(TextInputSnapshot(
            Arc::new(TextInputVisual {
                layout,
                selection: Vec::new(),
                caret: None,
                compose: Vec::new(),
            }),
            TextVAlign::Start,
            None,
            None,
            None,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styled_ranges_and_read_only_selection_share_layout_without_editor_state() {
        let mut context = TextInputContext::default();
        let font = include_bytes!("../../tests/fixtures/MaterialIcons-Regular.ttf");
        assert!(context.register_font(Arc::new(font.to_vec())));
        context.set_font_stack(FontRole::Ui, "Material Icons");
        let mut positions = Vec::new();
        for alignment in [
            StyledTextAlignment::Left,
            StyledTextAlignment::Center,
            StyledTextAlignment::Right,
        ] {
            let snapshot = context
                .shape_styled_text_with_options(
                    "\u{e88a}",
                    14.0,
                    FontRole::Ui,
                    &[],
                    200.0,
                    StyledTextOptions {
                        alignment,
                        ..Default::default()
                    },
                )
                .expect("aligned glyph");
            let rectangles = snapshot
                .range_rectangles(0.."\u{e88a}".len())
                .expect("glyph geometry");
            assert!(
                rectangles[0].width > 0.0,
                "the registered fixture must produce real geometry"
            );
            positions.push(rectangles[0].x);
            let cursor = snapshot.layout_cursor(0).expect("same cursor");
            assert!((cursor.bounds().x - rectangles[0].x).abs() < 1.0);
            assert!(
                snapshot
                    .clone()
                    .with_read_only_selection(0.."\u{e88a}".len())
                    .is_ok()
            );
        }
        assert!(positions[0] < positions[1] && positions[1] < positions[2]);
        let source = "中bold😀";
        for range in [1..3, 0..100, Range { start: 6, end: 2 }] {
            assert_eq!(
                context.shape_styled_text(
                    source,
                    14.0,
                    FontRole::Ui,
                    &[StyledTextSpan {
                        range,
                        bold: true,
                        ..Default::default()
                    }],
                    200.0
                ),
                Err(StyledTextError::InvalidRange)
            );
        }
        let snapshot = context
            .shape_styled_text(
                source,
                14.0,
                FontRole::Ui,
                &[StyledTextSpan {
                    range: 3..7,
                    bold: true,
                    ..Default::default()
                }],
                200.0,
            )
            .expect("valid UTF-8 spans");
        assert!(
            snapshot.0.selection.is_empty()
                && snapshot.0.caret.is_none()
                && snapshot.0.compose.is_empty()
        );
        for range in [1..3, 0..100, Range { start: 6, end: 2 }] {
            assert_eq!(
                snapshot.range_rectangles(range),
                Err(StyledTextError::InvalidRange)
            );
        }
        assert_eq!(snapshot.byte_index_at(Point::new(f32::NAN, 0.0)), None);
        let selected = snapshot
            .clone()
            .with_read_only_selection(3..7)
            .expect("read-only range");
        assert!(
            Arc::ptr_eq(&snapshot.0, &selected.0),
            "selection shares the immutable glyph layout"
        );
        let cursor = snapshot.layout_cursor(3).expect("cluster edge");
        assert!(cursor.belongs_to(&selected));
        assert!(matches!(
            snapshot.layout_cursor(1),
            Err(StyledTextError::InvalidRange)
        ));
        assert!(
            snapshot
                .layout_cursor_at(Point::new(f32::NAN, 0.0))
                .is_none()
        );
        let reshaped = context
            .shape_styled_text(source, 14.0, FontRole::Ui, &[], 80.0)
            .expect("new layout");
        assert!(
            !cursor.belongs_to(&reshaped),
            "old visual affinity cannot survive reshaping"
        );
        let bounds = Rect::new(0.0, 0.0, 220.0, 80.0);
        let view = snapshot.viewport(bounds, bounds, Point::default());
        assert!(
            !selected
                .decorations(view, false, false, 1.0)
                .selection
                .is_empty()
        );
        assert!(
            snapshot
                .decorations(view, false, false, 1.0)
                .selection
                .is_empty()
        );
        let colors = crate::TextInputColors {
            text: crate::Color::srgba(0.0, 0.0, 0.0, 0.0),
            selection: crate::Color::rgb8(255, 0, 0),
            caret: crate::Color::rgb8(0, 0, 0),
        };
        let mut rasterizer = crate::TextRasterizer::default();
        let base = rasterizer
            .render_shared(&snapshot, view, 1.0, false, false, colors)
            .expect("base bitmap");
        let marked = rasterizer
            .render_shared(&selected, view, 1.0, false, false, colors)
            .expect("selected bitmap");
        assert!(
            base.rgba != marked.rgba,
            "bitmap cache must include the external selection range"
        );
        let again = rasterizer
            .render_shared(&selected, view, 1.0, false, false, colors)
            .expect("stable bitmap");
        assert!(Arc::ptr_eq(&marked, &again));
        let mut editor = TextInputState::new(14.0, "Material Icons");
        editor.reset(&mut context, "author").expect("editor source");
        assert_eq!(
            editor.snapshot().with_read_only_selection(0..3),
            Err(StyledTextError::InteractiveSnapshot)
        );
        assert_eq!(editor.text(), "author");
    }
}
