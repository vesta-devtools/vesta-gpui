//! Host-owned three-component value projection.

use crate::{
    ControlState, NumberFieldIds, NumericComponent, Widget, WidgetIdentity, WidgetTheme,
    editable_number_field, number_field,
};
use vesta_gpui::{
    Align, Axis, Dimension, Element, Semantic, SemanticRole, Style, TextInputSnapshot,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VectorComponentValue {
    Value(f32),
    Mixed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VectorFieldIds {
    pub root: WidgetIdentity,
    pub axis_labels: [WidgetIdentity; 3],
    pub components: [NumberFieldIds; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VectorFieldSpec<'a> {
    pub label: &'a str,
    pub values: [VectorComponentValue; 3],
    pub unit: &'a str,
    pub precision: usize,
    pub state: ControlState,
}

#[derive(Clone)]
pub struct VectorComponentEdit {
    pub snapshot: TextInputSnapshot,
    pub focused: bool,
}

#[derive(Clone)]
pub struct VectorFieldEdit {
    pub components: [VectorComponentEdit; 3],
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

/// Display X/Y/Z components with independent typed step actions.
#[must_use]
pub fn vector_field<A>(
    ids: VectorFieldIds,
    spec: VectorFieldSpec<'_>,
    actions: [[A; 2]; 3],
    theme: WidgetTheme,
) -> Widget<A> {
    vector_field_impl(ids, spec, None, actions, theme)
}

/// Project three independent host-owned text drafts while keeping component actions typed.
#[must_use]
pub fn editable_vector_field<A>(
    ids: VectorFieldIds,
    spec: VectorFieldSpec<'_>,
    edit: VectorFieldEdit,
    actions: [[A; 2]; 3],
    theme: WidgetTheme,
) -> Widget<A> {
    vector_field_impl(ids, spec, Some(edit), actions, theme)
}

fn vector_field_impl<A>(
    ids: VectorFieldIds,
    spec: VectorFieldSpec<'_>,
    edit: Option<VectorFieldEdit>,
    actions: [[A; 2]; 3],
    theme: WidgetTheme,
) -> Widget<A> {
    let mut field = Widget::from_element(
        Element::container(ids.root.element, ids.root.key)
            .semantic(Semantic::new(SemanticRole::Group, spec.label))
            .styled(
                Style::default()
                    .size(Dimension::Auto, Dimension::Px(theme.metrics.control_height))
                    .max_width(Dimension::Percent(1.0))
                    .axis(Axis::Row)
                    .align(Align::Center)
                    .gap(4.0),
            ),
    );
    // The caller viewport clips the one-line group; component clips stay local.
    field.element.style.clip = vesta_gpui::ClipMode::Inherit;
    for (index, (((axis, label), component_ids), (value, [decrement, increment]))) in
        ["X", "Y", "Z"]
            .into_iter()
            .zip(ids.axis_labels)
            .zip(ids.components)
            .zip(spec.values.into_iter().zip(actions))
            .enumerate()
    {
        field = field.child(Widget::from_element(
            Element::text(label.element, label.key, format!("{axis} {}", spec.unit))
                .semantic(Semantic::new(
                    SemanticRole::Label,
                    format!("{axis} {}", spec.unit),
                ))
                .styled(
                    Style::default()
                        .text(
                            theme.typography.small_font_size,
                            NumericComponent::AXES[index]
                                .color(theme, spec.state == ControlState::Disabled),
                        )
                        .pointer_events(false),
                ),
        ));
        let text = match value {
            VectorComponentValue::Value(number) if number.is_finite() => {
                format!(
                    "{number:.precision$}{}",
                    spec.unit,
                    precision = spec.precision.min(6)
                )
            }
            VectorComponentValue::Value(_) | VectorComponentValue::Mixed => "—".to_owned(),
        };
        let mut component = if let Some(edit) = &edit {
            let input = &edit.components[index];
            let mut component = editable_number_field(
                component_ids,
                input.snapshot.clone(),
                input.focused,
                match value {
                    VectorComponentValue::Value(number) => f64::from(number),
                    VectorComponentValue::Mixed => 0.0,
                },
                edit.min,
                edit.max,
                edit.step,
                decrement,
                increment,
                spec.state,
                theme,
            );
            if let Some(input_element) = component.element.children.get_mut(1) {
                input_element.semantic = Some(match value {
                    VectorComponentValue::Value(number) if number.is_finite() => {
                        Semantic::new(SemanticRole::SpinButton, format!("{} {axis}", spec.label))
                            .value(text.clone())
                            .description(format!("Unit: {}", spec.unit))
                            .numeric(f64::from(number), edit.min, edit.max, Some(edit.step))
                    }
                    VectorComponentValue::Value(_) | VectorComponentValue::Mixed => {
                        Semantic::new(SemanticRole::TextInput, format!("{} {axis}", spec.label))
                            .value("Mixed")
                            .description("Type a number to replace the mixed value")
                    }
                });
            }
            component
        } else {
            number_field(
                component_ids,
                &text,
                decrement,
                increment,
                spec.state,
                theme,
            )
        };
        component.element.semantic = Some(Semantic::new(SemanticRole::Group, axis).value(text));
        component.element.style.width = Dimension::Px(100.0);
        component.element.style.flex_shrink = 0.0;
        field = field.child(component);
    }
    field
}
