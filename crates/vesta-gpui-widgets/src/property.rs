//! Product-neutral property rows around host-supplied value widgets.

use vesta_gpui::{
    Align, Axis, Dimension, Element, Insets, Semantic, SemanticRole, Style, TextHAlign, TextVAlign,
};

use crate::{Widget, WidgetAvailability, WidgetIdentity, WidgetTheme};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertyValidation {
    pub severity: ValidationSeverity,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertyRowState {
    pub label: String,
    pub availability: WidgetAvailability,
    pub validation: Option<PropertyValidation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertyRowIds {
    pub root: WidgetIdentity,
    pub label: WidgetIdentity,
    pub validation: WidgetIdentity,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PropertyRowPresentation {
    #[default]
    Standard,
    /// Studio's compact 24px row, right-aligned 142px label and unboxed gutter.
    Compact,
}

/// Compose a value control selected by the host from its authoritative schema.
/// Hidden/disabled conditions are already evaluated by the host.
#[must_use]
pub fn property_row_widget<A>(
    ids: PropertyRowIds,
    state: PropertyRowState,
    value: Widget<A>,
    theme: WidgetTheme,
) -> Option<Widget<A>> {
    property_row_with_presentation(ids, state, value, PropertyRowPresentation::Standard, theme)
}

#[must_use]
pub fn property_row_with_presentation<A>(
    ids: PropertyRowIds,
    state: PropertyRowState,
    mut value: Widget<A>,
    presentation: PropertyRowPresentation,
    theme: WidgetTheme,
) -> Option<Widget<A>> {
    if !state.availability.visible {
        return None;
    }
    if let Some(validation) = &state.validation
        && let Some(semantic) = &mut value.element.semantic
    {
        semantic.description = Some(match semantic.description.take() {
            Some(existing) if !existing.is_empty() => {
                format!("{existing}; {}", validation.message)
            }
            _ => validation.message.clone(),
        });
    }
    value.element.style = value
        .element
        .style
        .size(Dimension::Auto, Dimension::Percent(1.0))
        .flex(1.0, 1.0);
    let row = Element::container(ids.root.element, ids.root.key)
        .semantic(Semantic::new(SemanticRole::Group, &state.label))
        .styled(
            Style::default()
                .size(
                    Dimension::Percent(1.0),
                    Dimension::Px(theme.metrics.control_height),
                )
                .max_width(Dimension::Px(420.0))
                .axis(Axis::Row)
                .align(Align::Center)
                .gap(5.0)
                .background(theme.palette.field)
                .border(1.0, theme.palette.border)
                .radius(theme.metrics.radius),
        )
        .child(
            Element::text(ids.label.element, ids.label.key, &state.label)
                .semantic(Semantic::new(SemanticRole::Label, &state.label))
                .styled(
                    Style::default()
                        .size(Dimension::Px(126.0), Dimension::Percent(1.0))
                        .padding(Insets::symmetric(8.0, 0.0))
                        .text(theme.typography.font_size, theme.palette.text_muted)
                        .pointer_events(false),
                ),
        );
    let mut widget = Widget::from_element(row).child(value);
    if let Some(validation) = state.validation {
        let (symbol, color) = match validation.severity {
            ValidationSeverity::Info => ("i", theme.palette.accent),
            ValidationSeverity::Warning => ("!", theme.palette.warning),
            ValidationSeverity::Error => ("×", theme.palette.danger),
        };
        let hint = Element::text(ids.validation.element, ids.validation.key, symbol)
            .semantic(Semantic::new(SemanticRole::Label, &validation.message))
            .tooltip(validation.message)
            .styled(
                Style::default()
                    .size(Dimension::Px(20.0), Dimension::Percent(1.0))
                    .text(theme.typography.font_size, color)
                    .text_align(TextHAlign::Center, TextVAlign::Center),
            );
        widget = widget.child(Widget::from_element(hint));
    }
    if presentation == PropertyRowPresentation::Compact {
        widget.element.style = widget
            .element
            .style
            .clone()
            .size(Dimension::Percent(1.0), Dimension::Px(24.0))
            .max_width(Dimension::Auto)
            .gap(10.0)
            .background(vesta_gpui::Color::srgba(0.0, 0.0, 0.0, 0.0))
            .border(0.0, theme.palette.border)
            .radius(0.0);
        let label = &mut widget.element.children[0];
        label.style = label
            .style
            .clone()
            .size(Dimension::Px(142.0), Dimension::Percent(1.0))
            .flex(0.0, 0.0)
            .padding(Insets::all(0.0))
            .text(10.0, theme.palette.text)
            .text_align(TextHAlign::End, TextVAlign::Center);
    }
    widget.conditioned(state.availability)
}
