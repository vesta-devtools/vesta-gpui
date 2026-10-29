//! Product composition over public GPUI widgets; timeline graphics are content.
mod gallery;
mod timeline;
use crate::model::*;
use crate::study::Study;
use vesta_gpui::{
    Align, Axis, Color, Dimension as D, Element, ElementKey, Insets, Point, Semantic, SemanticRole,
    Style, TextHAlign, TextVAlign,
};
use vesta_gpui_widgets::{
    ButtonPresentation, ButtonTone, ControlState, SliderIds, Widget, button_with_presentation,
    list_item, slider,
};

pub const LEFT: f32 = 228.0;
pub const RIGHT: f32 = 270.0;
pub const TOP: f32 = 82.0;
pub const BOTTOM: f32 = 242.0;
pub const PANEL: Color = Color::rgb8(29, 33, 36);
pub const DARK: Color = Color::rgb8(23, 27, 30);

fn container(
    m: &Model,
    id: u32,
    width: f32,
    height: f32,
    axis: Axis,
    color: Option<Color>,
) -> Widget<Action> {
    let mut style = Style::default()
        .size(D::Px(width), D::Px(height))
        .axis(axis)
        .flex(0.0, 0.0);
    if let Some(color) = color {
        style = style.background(color);
    }
    Widget::from_element(
        Element::container(m.id(id), ElementKey::new(format!("motion.{id}"))).styled(style),
    )
}
fn text(m: &Model, id: u32, content: impl Into<String>, size: f32, color: Color) -> Widget<Action> {
    Widget::from_element(
        Element::text(m.id(id), ElementKey::new(format!("motion.{id}")), content).styled(
            Style::default()
                .size(D::Auto, D::Px(24.0))
                .text(size, color)
                .flex(0.0, 0.0)
                .pointer_events(false),
        ),
    )
}
fn at(mut widget: Widget<Action>, x: f32, y: f32) -> Widget<Action> {
    widget.element.style = widget.element.style.absolute(x, y);
    widget
}
fn line(m: &Model, id: u32, points: Vec<Point>, width: f32, color: Color) -> Widget<Action> {
    Widget::from_element(
        Element::container(m.id(id), ElementKey::new(format!("motion.{id}"))).styled(
            Style::default()
                .absolute(0.0, 0.0)
                .size(D::Percent(1.0), D::Percent(1.0))
                .polyline(points, width)
                .background(color)
                .pointer_events(false),
        ),
    )
}
fn rule(m: &Model, id: u32, width: f32) -> Widget<Action> {
    container(m, id, width, 1.0, Axis::Row, Some(BORDER))
}
fn button(
    m: &Model,
    id: u32,
    label: &str,
    action: Action,
    width: f32,
    selected: bool,
) -> Widget<Action> {
    let mut theme = m.theme;
    if selected {
        theme.palette.surface = theme.palette.menu_selected;
    }
    let state = if m.hovered == Some(m.id(id)) {
        ControlState::Hovered
    } else {
        ControlState::Idle
    };
    let mut widget = button_with_presentation(
        m.identity(id),
        m.identity(id + 1),
        label,
        action,
        state,
        ButtonPresentation {
            tone: ButtonTone::Normal,
            loading: false,
        },
        theme,
    );
    widget.element.style.width = D::Px(width);
    widget.element.style.flex_shrink = 0.0;
    if width <= 40.0 {
        widget.element.style.padding = Insets::all(0.0);
    }
    widget
}

pub fn build(m: &Model, width: f32, height: f32) -> Widget<Action> {
    let main_height = (height - TOP - BOTTOM).max(280.0);
    let center_width = (width - LEFT - RIGHT - 2.0).max(400.0);
    container(m, 1, width, height, Axis::Column, None)
        .child(header(m, width))
        .child(toolbar(m, width))
        .child(
            container(m, 90, width, main_height, Axis::Row, None)
                .child(layers(m, main_height))
                .child(container(
                    m,
                    91,
                    1.0,
                    main_height,
                    Axis::Column,
                    Some(BORDER),
                ))
                .child(composition(m, center_width, main_height))
                .child(container(
                    m,
                    92,
                    1.0,
                    main_height,
                    Axis::Column,
                    Some(BORDER),
                ))
                .child(inspector(m, main_height)),
        )
        .child(timeline::build(m, width))
        .child(status(m, width))
}

fn header(m: &Model, width: f32) -> Widget<Action> {
    let mut row = container(m, 2, width, 38.0, Axis::Row, Some(Color::rgb8(25, 29, 32)));
    row.element.style = row
        .element
        .style
        .align(Align::Center)
        .gap(22.0)
        .padding(Insets::symmetric(17.0, 0.0));
    row = row.child(text(m, 3, "◈", 22.0, MINT)).child(text(
        m,
        4,
        "Vesta Motion",
        15.0,
        m.theme.palette.text,
    ));
    row = row.child(text(m, 10, "REALTIME MOTION LAB", 10.0, MUTED));
    let mut spacer = text(m, 15, m.study.title(), 11.0, MUTED);
    spacer.element.style = spacer
        .element
        .style
        .flex(1.0, 1.0)
        .text_align(TextHAlign::End, TextVAlign::Center);
    row.child(spacer).child(text(m, 16, "PREVIEW", 10.0, MINT))
}
fn toolbar(m: &Model, width: f32) -> Widget<Action> {
    let mut bar = container(m, 20, width, 44.0, Axis::Row, Some(PANEL));
    bar.element.style = bar
        .element
        .style
        .align(Align::Center)
        .padding(Insets::symmetric(12.0, 0.0))
        .gap(7.0)
        .border(1.0, BORDER);
    let mut spacer = text(
        m,
        29,
        format!("Composition:  {}", m.study.title()),
        11.0,
        MUTED,
    );
    spacer.element.style = spacer
        .element
        .style
        .flex(1.0, 1.0)
        .text_align(TextHAlign::End, TextVAlign::Center);
    let mut export = button_with_presentation(
        m.identity(36),
        m.identity(37),
        "Export",
        Action::Start,
        ControlState::Disabled,
        ButtonPresentation::default(),
        m.theme,
    );
    export.element.style.width = D::Px(76.0);
    export.element.tooltip = Some("Offline export is outside this first preview slice".into());
    bar.child(button(
        m,
        21,
        "↖  Select",
        Action::Select(Layer::Ribbon),
        82.0,
        true,
    ))
    .child(button(m, 23, "↺  Reset", Action::Reset, 76.0, false))
    .child(button(m, 25, "Guides", Action::Guides, 64.0, m.guides))
    .child(button(m, 27, "Demo reel", Action::Tour, 82.0, m.tour))
    .child(spacer)
    .child(text(m, 30, "Fit", 11.0, m.theme.palette.text))
    .child(text(m, 31, "  1920 × 1080  ·  30 fps  ", 11.0, MUTED))
    .child(export)
}
fn layers(m: &Model, height: f32) -> Widget<Action> {
    let compact = height < 500.0;
    let row_height = if compact { 20.0 } else { 28.0 };
    let card_height = if compact {
        70.0
    } else if height < 620.0 {
        80.0
    } else {
        100.0
    };
    let columns = if height < 620.0 { 3 } else { 2 };
    let mut panel = container(m, 100, LEFT, height, Axis::Column, Some(PANEL));
    panel.element.style = panel.element.style.padding(Insets::all(10.0)).gap(4.0);
    panel = panel.child(text(m, 101, "Studies  /  06", 13.0, m.theme.palette.text));
    if !compact {
        panel = panel.child(text(
            m,
            102,
            "1–6  Switch study   ·   T  Demo reel",
            10.0,
            MUTED,
        ));
    }
    panel = panel
        .child(gallery::cards(m, card_height, columns))
        .child(rule(m, 103, LEFT - 20.0))
        .child(text(m, 104, "Layers", 12.0, MUTED));
    for layer in Layer::ALL {
        let base = layer.slot();
        let mut row = container(m, base + 8, LEFT - 20.0, row_height, Axis::Row, None);
        row.element.style = row.element.style.align(Align::Center).gap(4.0);
        let mut item = list_item(
            m.identity(base),
            m.identity(base + 1),
            m.layer_name(layer),
            m.selected == layer,
            Action::Select(layer),
            ControlState::Idle,
            m.theme,
        );
        item.element.style.width = D::Px(172.0);
        item.element.style.height = D::Px(row_height);
        let mut visible = button(
            m,
            base + 3,
            if m.visible(layer) { "◉" } else { "○" },
            Action::Visibility(layer),
            28.0,
            false,
        );
        visible.element.style.height = D::Px(row_height);
        panel = panel.child(row.child(visible).child(item));
    }
    let mut spacer = container(m, 190, LEFT - 20.0, 0.0, Axis::Column, None);
    spacer.element.style = spacer.element.style.flex(1.0, 1.0);
    panel = panel
        .child(spacer)
        .child(rule(m, 191, LEFT - 20.0))
        .child(text(m, 192, "Palette", 11.0, MUTED));
    let mut palette = container(m, 280, LEFT - 20.0, 28.0, Axis::Row, None);
    palette.element.style.gap = 5.0;
    for (i, label) in ["Gold", "Jade", "Iris"].into_iter().enumerate() {
        palette = palette.child(button(
            m,
            300 + i as u32 * 40,
            label,
            Action::Palette(i as u8),
            65.0,
            m.params().palette == i as u8,
        ));
    }
    panel.child(palette)
}

fn composition(m: &Model, width: f32, height: f32) -> Widget<Action> {
    let mut frame = container(m, SURFACE, width, height, Axis::Column, None);
    frame.element.semantic = Some(Semantic::new(
        SemanticRole::Group,
        format!("Animated {} composition", m.study.title()),
    ));
    frame = frame
        .child(at(text(m, 601, "Composition", 11.0, MUTED), 18.0, 8.0))
        .child(at(
            text(m, 602, m.study.title(), 11.0, m.theme.palette.text),
            122.0,
            8.0,
        ))
        .child(at(text(m, 603, "●  LIVE", 10.0, MINT), width - 70.0, 8.0));
    if m.guides {
        let mut guide = container(m, 604, width - 48.0, height - 80.0, Axis::Column, None);
        guide.element.style = guide
            .element
            .style
            .border(1.0, Color::rgb8(48, 55, 57))
            .pointer_events(false);
        frame = frame.child(at(guide, 24.0, 40.0));
    }
    if m.visible(Layer::Title) {
        frame = frame
            .child(at(
                text(m, 620, m.study.heading(), 19.0, Color::rgb8(229, 211, 178)),
                52.0,
                66.0,
            ))
            .child(at(
                text(
                    m,
                    621,
                    format!("S T U D Y   /   0 {}", m.study.index() + 1),
                    9.0,
                    MUTED,
                ),
                54.0,
                96.0,
            ));
    }
    frame
        .child(at(
            text(
                m,
                622,
                format!("{}   /   LIVE PREVIEW", m.study.category()),
                9.0,
                MUTED,
            ),
            30.0,
            height - 30.0,
        ))
        .child(at(
            text(m, 623, "Space  Play / Pause", 10.0, MUTED),
            width - 144.0,
            height - 30.0,
        ))
}
fn inspector(m: &Model, height: f32) -> Widget<Action> {
    let mut panel = container(m, 700, RIGHT, height, Axis::Column, Some(PANEL));
    panel.element.style = panel.element.style.padding(Insets::all(16.0)).gap(8.0);
    panel = panel
        .child(text(m, 701, "Inspector", 13.0, m.theme.palette.text))
        .child(text(
            m,
            702,
            format!("◈   {}", m.layer_name(m.selected)),
            14.0,
            m.theme.palette.text,
        ))
        .child(rule(m, 703, RIGHT - 32.0));
    if m.selected == Layer::Ribbon {
        panel = panel.child(text(m, 704, m.study.category(), 10.0, MINT));
        for param in [Parameter::Count, Parameter::Radius, Parameter::Twist] {
            panel = panel.child(parameter(m, param));
        }
        panel = panel
            .child(rule(m, 705, RIGHT - 32.0))
            .child(text(m, 706, "ANIMATION DRIVER", 10.0, MINT))
            .child(text(
                m,
                707,
                format!(
                    "Cycle phase        {:.1}°",
                    crate::preview::rotation(m.time).to_degrees()
                ),
                12.0,
                m.theme.palette.text,
            ));
    } else {
        let info = match m.selected {
            Layer::Trails => "Three orbital paths",
            Layer::Title => m.study.title(),
            Layer::Background => "Graphite stage",
            Layer::Ribbon => "",
        };
        panel = panel
            .child(text(m, 708, info, 12.0, m.theme.palette.text))
            .child(button(
                m,
                710,
                if m.visible(m.selected) {
                    "Hide layer"
                } else {
                    "Show layer"
                },
                Action::Visibility(m.selected),
                RIGHT - 32.0,
                false,
            ));
    }
    panel = panel
        .child(rule(m, 712, RIGHT - 32.0))
        .child(text(m, 713, "PLAYBACK", 10.0, MINT))
        .child(parameter(m, Parameter::Speed));
    let mut spacer = container(m, 800, RIGHT - 32.0, 5.0, Axis::Column, None);
    spacer.element.style = spacer.element.style.flex(1.0, 1.0);
    panel
        .child(spacer)
        .child(text(m, 801, "APPEARANCE", 10.0, MINT))
        .child(text(
            m,
            802,
            ["Gold / Coral", "Jade / Pearl", "Silver / Iris"][m.params().palette as usize],
            12.0,
            GOLD,
        ))
        .child(text(
            m,
            803,
            "Display preview  ·  Native Vulkan",
            10.0,
            MUTED,
        ))
}
fn parameter(m: &Model, param: Parameter) -> Widget<Action> {
    let base = param.slot();
    let mut group = container(m, base + 8, RIGHT - 32.0, 58.0, Axis::Column, None);
    group = group.child(text(
        m,
        base + 9,
        m.study.parameter_name(param),
        11.0,
        MUTED,
    ));
    let mut widget = slider(
        SliderIds {
            root: m.identity(base),
            track: m.identity(base + 1),
            fill: m.identity(base + 2),
            thumb: m.identity(base + 3),
            value_label: m.identity(base + 4),
        },
        m.value(param),
        Action::Parameter(param),
        ControlState::Idle,
        m.theme,
    );
    if let Some(label) = widget.element.children.get_mut(1) {
        label.kind = vesta_gpui::ElementKind::Text(m.formatted(param));
    }
    if let Some(semantic) = &mut widget.element.semantic {
        semantic.label = m.study.parameter_name(param).into();
    }
    group.child(widget)
}
fn status(m: &Model, width: f32) -> Widget<Action> {
    let mut bar = container(m, 900, width, 24.0, Axis::Row, Some(DARK));
    bar.element.style = bar
        .element
        .style
        .align(Align::Center)
        .padding(Insets::symmetric(12.0, 0.0))
        .gap(20.0);
    let mut spacer = text(m, 902, &m.status, 10.0, MUTED);
    spacer.element.style = spacer.element.style.flex(1.0, 1.0);
    bar.child(text(
        m,
        901,
        if m.playing {
            "●  Playing"
        } else {
            "Ⅱ  Paused"
        },
        10.0,
        MINT,
    ))
    .child(spacer)
    .child(text(m, 903, "Vesta GPUI  /  Vulkan", 10.0, MUTED))
    .child(text(
        m,
        904,
        "1920 × 1080   ·   12 s   ·   30 fps",
        10.0,
        MUTED,
    ))
}
