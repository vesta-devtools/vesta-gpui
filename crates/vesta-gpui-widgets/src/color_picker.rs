//! Controlled sRGB color draft projection; commits remain a host action.

use crate::{
    ControlState, NumberFieldIds, RadioIds, Widget, WidgetIdentity, WidgetTheme, button,
    number_field, radio, text_field,
};
use vesta_gpui::{
    Align, Axis, Color, Dimension, Element, Semantic, SemanticRole, Style, TextInputSnapshot,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorPickerMode {
    #[default]
    Rgb,
    Hsv,
}

impl ColorPickerMode {
    const fn channel_labels(self) -> [&'static str; 4] {
        match self {
            Self::Rgb => ["R", "G", "B", "A"],
            Self::Hsv => ["H", "S", "V", "A"],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColorPickerIds {
    pub root: WidgetIdentity,
    pub swatch: WidgetIdentity,
    pub swatch_label: WidgetIdentity,
    pub panel: WidgetIdentity,
    pub hex: WidgetIdentity,
    pub modes: WidgetIdentity,
    pub mode_choices: [RadioIds; 2],
    pub channel_rows: [WidgetIdentity; 4],
    pub channel_labels: [WidgetIdentity; 4],
    pub channels: [NumberFieldIds; 4],
    pub eyedropper: WidgetIdentity,
    pub eyedropper_label: WidgetIdentity,
    pub buttons: WidgetIdentity,
    pub confirm: WidgetIdentity,
    pub confirm_label: WidgetIdentity,
    pub cancel: WidgetIdentity,
    pub cancel_label: WidgetIdentity,
}

pub struct ColorPickerActions<A> {
    pub toggle: A,
    pub select_mode: [A; 2],
    pub decrease: [A; 4],
    pub increase: [A; 4],
    pub eyedropper: A,
    pub confirm: A,
    pub cancel: A,
}

pub struct ColorPickerView {
    pub draft: [u8; 4],
    pub mode: ColorPickerMode,
    pub channel_values: [String; 4],
    pub hex_snapshot: TextInputSnapshot,
    pub hex_focused: bool,
    pub expanded: bool,
}

#[must_use]
pub fn color_picker<A>(
    ids: ColorPickerIds,
    view: ColorPickerView,
    actions: ColorPickerActions<A>,
    theme: WidgetTheme,
) -> Widget<A> {
    let ColorPickerView {
        draft,
        mode,
        channel_values,
        hex_snapshot,
        hex_focused,
        expanded,
    } = view;
    let canonical_hex = format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        draft[0], draft[1], draft[2], draft[3]
    );
    let ColorPickerIds {
        root,
        swatch,
        swatch_label,
        panel,
        hex,
        modes,
        mode_choices,
        channel_rows,
        channel_labels,
        channels,
        eyedropper,
        eyedropper_label,
        buttons,
        confirm,
        confirm_label,
        cancel,
        cancel_label,
    } = ids;
    let ColorPickerActions {
        toggle,
        select_mode,
        decrease,
        increase,
        eyedropper: eyedropper_action,
        confirm: confirm_action,
        cancel: cancel_action,
    } = actions;
    let swatch = Widget::from_element(
        Element::container(swatch.element, swatch.key)
            .semantic(
                Semantic::new(SemanticRole::Button, "Edit color")
                    .description(&canonical_hex)
                    .expanded(expanded),
            )
            .styled(
                Style::default()
                    .size(
                        Dimension::Percent(1.0),
                        Dimension::Px(theme.metrics.control_height),
                    )
                    .min_width(Dimension::Px(0.0))
                    .axis(Axis::Row)
                    .align(Align::Center)
                    .background(Color::srgba(
                        f32::from(draft[0]) / 255.0,
                        f32::from(draft[1]) / 255.0,
                        f32::from(draft[2]) / 255.0,
                        f32::from(draft[3]) / 255.0,
                    ))
                    .border(1.0, theme.palette.border)
                    .focusable(true),
            )
            .child(
                Element::text(swatch_label.element, swatch_label.key, "Edit color").styled(
                    Style::default()
                        .text(theme.typography.font_size, theme.palette.text)
                        .background(theme.palette.panel)
                        .pointer_events(false),
                ),
            ),
    )
    .on_click(toggle);
    let mut root = Widget::from_element(
        Element::container(root.element, root.key).styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Auto)
                .max_width(Dimension::Px(220.0))
                .min_width(Dimension::Px(0.0))
                .axis(Axis::Column)
                .gap(5.0),
        ),
    )
    .child(swatch);
    if !expanded {
        return root;
    }
    let mut panel = Widget::from_element(
        Element::container(panel.element, panel.key)
            .semantic(Semantic::new(SemanticRole::Group, "RGBA color"))
            .styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Auto)
                    .min_width(Dimension::Px(0.0))
                    .axis(Axis::Column)
                    .gap(4.0)
                    .background(theme.palette.panel),
            ),
    );
    let mut hex_field = text_field(hex, hex_snapshot, hex_focused, theme);
    hex_field.semantic = Some(
        Semantic::new(SemanticRole::TextInput, "Hex RGBA")
            .description("Enter #RRGGBB or #RRGGBBAA"),
    );
    panel = panel.child(Widget::from_element(hex_field));
    let [rgb_ids, hsv_ids] = mode_choices;
    let [rgb_action, hsv_action] = select_mode;
    panel = panel.child(
        Widget::from_element(
            Element::container(modes.element, modes.key)
                .semantic(Semantic::new(SemanticRole::Group, "Color model"))
                .styled(
                    Style::default()
                        .size(Dimension::Percent(1.0), Dimension::Auto)
                        .min_width(Dimension::Px(0.0))
                        .axis(Axis::Row)
                        .gap(12.0),
                ),
        )
        .child(radio(
            rgb_ids,
            "RGB",
            mode == ColorPickerMode::Rgb,
            rgb_action,
            ControlState::Idle,
            theme,
        ))
        .child(radio(
            hsv_ids,
            "HSV",
            mode == ColorPickerMode::Hsv,
            hsv_action,
            ControlState::Idle,
            theme,
        )),
    );
    let labels = mode.channel_labels();
    for (index, (((((row, label), number_ids), value), decrease), increase)) in channel_rows
        .into_iter()
        .zip(channel_labels)
        .zip(channels)
        .zip(channel_values)
        .zip(decrease)
        .zip(increase)
        .enumerate()
    {
        let mut field = number_field(
            number_ids,
            value,
            decrease,
            increase,
            ControlState::Idle,
            theme,
        );
        field.element.style.width = Dimension::Percent(1.0);
        field.element.style.min_width = Dimension::Px(0.0);
        panel = panel.child(
            Widget::from_element(
                Element::container(row.element, row.key)
                    .styled(
                        Style::default()
                            .size(Dimension::Percent(1.0), Dimension::Auto)
                            .min_width(Dimension::Px(0.0))
                            .axis(Axis::Column)
                            .gap(4.0),
                    )
                    .child(
                        Element::text(label.element, label.key, labels[index]).styled(
                            Style::default()
                                .text(
                                    theme.typography.font_size,
                                    if mode == ColorPickerMode::Rgb {
                                        crate::NumericComponent::RGBA[index].color(theme, false)
                                    } else {
                                        theme.palette.text_muted
                                    },
                                )
                                .pointer_events(false),
                        ),
                    ),
            )
            .child(field),
        );
    }
    panel = panel.child(button(
        eyedropper,
        eyedropper_label,
        "Pick color",
        eyedropper_action,
        ControlState::Idle,
        theme,
    ));
    let controls = Widget::from_element(
        Element::container(buttons.element, buttons.key)
            .styled(Style::default().axis(Axis::Column).gap(4.0)),
    )
    .child(button(
        cancel,
        cancel_label,
        "Cancel",
        cancel_action,
        ControlState::Idle,
        theme,
    ))
    .child(button(
        confirm,
        confirm_label,
        "Apply",
        confirm_action,
        ControlState::Idle,
        theme,
    ));
    panel = panel.child(controls);
    root = root.child(panel);
    root
}
