//! Compact actions and read-only status surfaces shared by desktop hosts.

use crate::{
    ControlState, ICON_LINE_SIZE, ICON_LINE_STROKE, Widget, WidgetIdentity, WidgetTheme, button,
};
use vesta_gpui::{
    Align, Axis, Color, Dimension, Element, Insets, Point, Semantic, SemanticRole, Style,
    TextHAlign, TextVAlign,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompactSign {
    Plus,
    Minus,
}

/// Decorative line sign; its parent owns interaction and accessible meaning.
#[must_use]
pub fn compact_sign(identity: WidgetIdentity, sign: CompactSign, color: Color) -> Element {
    let points = match sign {
        CompactSign::Minus => vec![Point::new(2.5, 7.0), Point::new(11.5, 7.0)],
        CompactSign::Plus => vec![
            Point::new(2.5, 7.0),
            Point::new(11.5, 7.0),
            Point::new(7.0, 7.0),
            Point::new(7.0, 2.5),
            Point::new(7.0, 11.5),
        ],
    };
    Element::container(identity.element, identity.key).styled(
        Style::default()
            .size(Dimension::Px(ICON_LINE_SIZE), Dimension::Px(ICON_LINE_SIZE))
            .polyline(points, ICON_LINE_STROKE)
            .background(color)
            .pointer_events(false),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompactButtonKind {
    Icon,
    Toolbar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IconButtonSpec<'a> {
    pub icon: &'a str,
    pub name: &'a str,
    pub shortcut: Option<&'a str>,
    pub kind: CompactButtonKind,
    pub state: ControlState,
}

/// The icon remains decorative; the Button shell owns meaning and interaction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IconButtonContent<'a> {
    /// Use the glyph supplied by the existing IconButtonSpec.
    #[default]
    Glyph,
    /// A host-resolved source token, using the existing centered Contain path.
    Image(&'a str),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IconButtonPresentation<'a> {
    pub content: IconButtonContent<'a>,
    /// None is an ordinary action; Some projects the caller's current state.
    pub selected: Option<bool>,
}

/// A compact action whose glyph is visual; the full name is its accessible label.
#[must_use]
pub fn icon_button<A>(
    root: WidgetIdentity,
    glyph: WidgetIdentity,
    spec: IconButtonSpec<'_>,
    action: A,
    theme: WidgetTheme,
) -> Widget<A> {
    let mut widget = button(root, glyph, spec.icon, action, spec.state, theme);
    widget.element.semantic = Some(Semantic::new(SemanticRole::Button, spec.name));
    widget.element.tooltip = Some(match spec.shortcut {
        Some(shortcut) => format!("{} ({shortcut})", spec.name),
        None => spec.name.to_owned(),
    });
    let extent = match spec.kind {
        CompactButtonKind::Icon => theme.metrics.control_height,
        CompactButtonKind::Toolbar => theme.metrics.control_height + 6.0,
    };
    widget.element.style.width = Dimension::Px(extent);
    widget.element.style.padding = Insets::all(0.0);
    if let Some(label) = widget.element.children.first_mut() {
        let sign = match spec.icon {
            "+" => Some(CompactSign::Plus),
            "−" => Some(CompactSign::Minus),
            _ => None,
        };
        if let Some(sign) = sign {
            *label = compact_sign(
                WidgetIdentity {
                    element: label.id,
                    key: label.key.clone(),
                },
                sign,
                if spec.state == ControlState::Disabled {
                    theme.palette.text_disabled
                } else {
                    theme.palette.text
                },
            );
        } else {
            label.style.visual.font_size = theme.typography.font_size + 3.0;
        }
    }
    widget
}

/// Extend the existing shell without giving decorative content another owner.
#[must_use]
pub fn icon_button_with_presentation<A>(
    root: WidgetIdentity,
    glyph: WidgetIdentity,
    spec: IconButtonSpec<'_>,
    action: A,
    presentation: IconButtonPresentation<'_>,
    theme: WidgetTheme,
) -> Widget<A> {
    let mut widget = icon_button(root, glyph, spec, action, theme);
    if let IconButtonContent::Image(source) = presentation.content
        && let Some(icon) = widget.element.children.first_mut()
    {
        *icon = Element::fitted_image(
            icon.id,
            icon.key.clone(),
            source,
            vesta_gpui::ImagePresentation::new(vesta_gpui::ImageFit::Contain),
        )
        .styled(
            Style::default()
                .size(Dimension::Px(ICON_LINE_SIZE), Dimension::Px(ICON_LINE_SIZE))
                .flex(0.0, 0.0)
                .pointer_events(false),
        );
    }
    if let Some(selected) = presentation.selected {
        widget.element.semantic = widget.element.semantic.map(|s| s.checked(selected));
        if selected && spec.state != ControlState::Disabled {
            widget.element.style.visual.background = Some(theme.palette.menu_selected);
            widget.element.style.visual.border_color = Some(theme.palette.accent);
        }
    }
    widget
}

/// A button with host-owned pressed state and a typed toggle action.
#[must_use]
pub fn toggle_button<A>(
    root: WidgetIdentity,
    label: WidgetIdentity,
    caption: &str,
    pressed: bool,
    state: ControlState,
    action: A,
    theme: WidgetTheme,
) -> Widget<A> {
    let mut widget = button(root, label, caption, action, state, theme);
    widget.element.semantic = Some(Semantic::new(SemanticRole::Button, caption).checked(pressed));
    if pressed && state != ControlState::Disabled {
        widget.element.style.visual.background = Some(theme.palette.menu_selected);
        widget.element.style.visual.border_color = Some(theme.palette.accent);
    }
    widget
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusTone {
    Information,
    Success,
    Warning,
    Error,
}

impl StatusTone {
    fn name(self) -> &'static str {
        match self {
            Self::Information => "Information",
            Self::Success => "Success",
            Self::Warning => "Warning",
            Self::Error => "Error",
        }
    }
}

/// A read-only status badge; its severity is announced as text as well as color.
#[must_use]
pub fn status_badge<A>(
    root: WidgetIdentity,
    label: WidgetIdentity,
    text: &str,
    tone: StatusTone,
    theme: WidgetTheme,
) -> Widget<A> {
    let color = match tone {
        StatusTone::Information => theme.palette.accent,
        StatusTone::Success => theme.palette.success,
        StatusTone::Warning => theme.palette.warning,
        StatusTone::Error => theme.palette.danger,
    };
    Widget::from_element(
        Element::container(root.element, root.key)
            .semantic(Semantic::new(
                SemanticRole::Status,
                format!("{}: {text}", tone.name()),
            ))
            .styled(
                Style::default()
                    .size(
                        Dimension::Auto,
                        Dimension::Px(theme.metrics.compact_control_height),
                    )
                    .axis(Axis::Row)
                    .align(Align::Center)
                    .padding(Insets::symmetric(8.0, 0.0))
                    .background(theme.palette.field)
                    .border(1.0, color)
                    .radius(theme.metrics.radius)
                    .pointer_events(false),
            )
            .child(
                Element::text(label.element, label.key, text).styled(
                    Style::default()
                        .text(theme.typography.small_font_size, color)
                        .text_align(TextHAlign::Start, TextVAlign::Center)
                        .pointer_events(false),
                ),
            ),
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BusyIndicatorIds {
    pub root: WidgetIdentity,
    pub spinner: WidgetIdentity,
    pub label: WidgetIdentity,
    pub cancel: WidgetIdentity,
    pub cancel_label: WidgetIdentity,
}

/// Project host-owned running state. The host advances `phase` and performs cancellation.
#[must_use]
pub fn busy_indicator<A>(
    ids: BusyIndicatorIds,
    caption: &str,
    active: bool,
    phase: u8,
    cancel_enabled: bool,
    cancel_action: A,
    theme: WidgetTheme,
) -> Widget<A> {
    let glyph = if active {
        ["◐", "◓", "◑", "◒"][usize::from(phase % 4)]
    } else {
        "✓"
    };
    let state = if active { "Running" } else { "Stopped" };
    let surface = Widget::from_element(
        Element::container(ids.root.element, ids.root.key)
            .semantic(
                Semantic::new(SemanticRole::Status, format!("{caption}: {state}")).busy(active),
            )
            .styled(
                Style::default()
                    .size(Dimension::Auto, Dimension::Px(theme.metrics.control_height))
                    .axis(Axis::Row)
                    .align(Align::Center)
                    .padding(Insets::symmetric(8.0, 0.0))
                    .gap(7.0)
                    .background(theme.palette.field),
            )
            .child(
                Element::text(ids.spinner.element, ids.spinner.key, glyph).styled(
                    Style::default()
                        .text(theme.typography.font_size + 3.0, theme.palette.accent)
                        .pointer_events(false),
                ),
            )
            .child(
                Element::text(
                    ids.label.element,
                    ids.label.key,
                    format!("{caption}: {state}"),
                )
                .styled(
                    Style::default()
                        .text(theme.typography.font_size, theme.palette.text)
                        .pointer_events(false),
                ),
            ),
    );
    surface.child(button(
        ids.cancel,
        ids.cancel_label,
        "Cancel",
        cancel_action,
        if active && cancel_enabled {
            ControlState::Idle
        } else {
            ControlState::Disabled
        },
        theme,
    ))
}
