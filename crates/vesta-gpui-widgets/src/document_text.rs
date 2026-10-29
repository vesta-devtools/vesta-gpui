use crate::{TextContent, WidgetIdentity, WidgetTheme, text_label};
use vesta_gpui::{Color, Dimension, Element, ElementScope, Style, TextHAlign, TextVAlign};

pub(crate) fn label(
    scope: ElementScope,
    next: &mut u32,
    value: &str,
    size: f32,
    color: Color,
) -> Element {
    let id = *next;
    *next += 1;
    text_label(
        WidgetIdentity {
            element: scope.element(id),
            key: scope.identity(id, format!("text.{id}")).key,
        },
        TextContent::Literal(value),
        WidgetTheme::dark(),
    )
    .styled(
        Style::default()
            .size(Dimension::Percent(1.0), Dimension::Px(size * 1.5))
            .text(size, color)
            .text_align(TextHAlign::Start, TextVAlign::Start)
            .pointer_events(false),
    )
}

pub(crate) fn wrapped(
    scope: ElementScope,
    next: &mut u32,
    value: &str,
    size: f32,
    color: Color,
) -> Element {
    let mut element = label(scope, next, value, size, color);
    element.style.height = Dimension::Auto;
    element.style.text_wrap = true;
    element
}
