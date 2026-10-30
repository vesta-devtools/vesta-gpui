//! A static editor outline built entirely from the public UI API.
use vesta_gpui::{
    Align, Axis, Color, Dimension, Element, ElementId, ElementKey, Insets, Justify, Style, Viewport,
};

/// Fixed identities belong to this standalone example only.
mod demo_ids {
    use vesta_gpui::ElementId;

    pub const ROOT: ElementId = ElementId::new(1);
    pub const TOP: ElementId = ElementId::new(2);
    pub const BODY: ElementId = ElementId::new(3);
    pub const LEFT: ElementId = ElementId::new(4);
    pub const CENTER: ElementId = ElementId::new(5);
    pub const RIGHT: ElementId = ElementId::new(6);
    pub const STATUS: ElementId = ElementId::new(7);
}

const BG_ROOT: Color = Color::rgb8(19, 22, 28);
const BG_TOP: Color = Color::rgb8(26, 30, 38);
const BG_PANEL: Color = Color::rgb8(23, 27, 34);
const BG_SURFACE: Color = Color::rgb8(15, 18, 23);
const BG_CARD: Color = Color::rgb8(29, 35, 44);
const BORDER: Color = Color::rgb8(48, 55, 67);
const BORDER_SUBTLE: Color = Color::rgb8(38, 44, 54);
const ACCENT: Color = Color::rgb8(89, 159, 255);
const TEXT: Color = Color::rgb8(224, 229, 237);
const TEXT_MUTED: Color = Color::rgb8(135, 145, 161);

fn main() {
    let outline = build_editor_outline(Viewport::new(1280.0, 720.0, 1.0));
    println!(
        "Editor outline: {} top-level regions",
        outline.children.len()
    );
}

/// Builds a static Top/Left/Center/Right/Bottom editor outline.
#[must_use]
fn build_editor_outline(viewport: Viewport) -> Element {
    Element::container(demo_ids::ROOT, key("studio.root"))
        .styled(
            Style::default()
                .size(
                    Dimension::Px(viewport.width.max(1.0)),
                    Dimension::Px(viewport.height.max(1.0)),
                )
                .axis(Axis::Column)
                .background(BG_ROOT)
                .clipped(),
        )
        .child(top_bar())
        .child(body())
        .child(status_bar())
}

fn top_bar() -> Element {
    panel(demo_ids::TOP, "studio.top")
        .styled(
            Style::default()
                .size(Dimension::Auto, Dimension::Px(48.0))
                .axis(Axis::Row)
                .align(Align::Center)
                .gap(22.0)
                .padding(Insets::symmetric(16.0, 0.0))
                .background(BG_TOP)
                .border(1.0, BORDER),
        )
        .child(label(100, "studio.top.brand", "VESTA", 74.0, 17.0, ACCENT))
        .child(label(
            101,
            "studio.top.menu",
            "FILE   EDIT   VIEW   WINDOW   HELP",
            330.0,
            12.0,
            TEXT_MUTED,
        ))
        .child(
            Element::container(ElementId::new(102), key("studio.top.spacer"))
                .styled(Style::default().flex(1.0, 1.0)),
        )
        .child(
            Element::container(ElementId::new(103), key("studio.top.mode"))
                .styled(
                    Style::default()
                        .size(Dimension::Px(120.0), Dimension::Px(26.0))
                        .align(Align::Center)
                        .justify(Justify::Center)
                        .background(Color::rgb8(32, 40, 51))
                        .border(1.0, Color::rgb8(62, 75, 92))
                        .radius(6.0),
                )
                .child(label(
                    104,
                    "studio.top.mode.label",
                    "ITERATION A",
                    92.0,
                    11.0,
                    TEXT,
                )),
        )
}

fn body() -> Element {
    panel(demo_ids::BODY, "studio.body")
        .styled(Style::default().axis(Axis::Row).flex(1.0, 1.0).clipped())
        .child(left_panel())
        .child(center_panel())
        .child(right_panel())
}

fn left_panel() -> Element {
    panel(demo_ids::LEFT, "studio.left")
        .styled(
            Style::default()
                .size(Dimension::Px(238.0), Dimension::Auto)
                .background(BG_PANEL)
                .border(1.0, BORDER)
                .clipped(),
        )
        .child(section_header(110, "studio.left.header", "PROJECT OUTLINE"))
        .child(
            panel(ElementId::new(111), "studio.left.content")
                .styled(
                    Style::default()
                        .flex(1.0, 1.0)
                        .gap(5.0)
                        .padding(Insets::all(10.0)),
                )
                .children([
                    list_row(112, "studio.left.row.workspace", "Workspace", true),
                    list_row(113, "studio.left.row.sources", "Sources", false),
                    list_row(114, "studio.left.row.outputs", "Outputs", false),
                    list_row(115, "studio.left.row.packages", "Packages", false),
                ]),
        )
}

fn center_panel() -> Element {
    panel(demo_ids::CENTER, "studio.center")
        .styled(
            Style::default()
                .flex(1.0, 1.0)
                .background(BG_ROOT)
                .clipped(),
        )
        .child(section_header(120, "studio.center.header", "WORKSPACE"))
        .child(
            panel(ElementId::new(121), "studio.center.surface.pad")
                .styled(
                    Style::default()
                        .flex(1.0, 1.0)
                        .padding(Insets::all(16.0))
                        .background(BG_SURFACE)
                        .clipped(),
                )
                .child(
                    panel(ElementId::new(122), "studio.center.surface")
                        .styled(
                            Style::default()
                                .flex(1.0, 1.0)
                                .align(Align::Center)
                                .justify(Justify::Center)
                                .background(Color::rgb8(18, 22, 28))
                                .border(1.0, BORDER_SUBTLE)
                                .radius(8.0)
                                .clipped(),
                        )
                        .child(
                            panel(ElementId::new(123), "studio.center.card")
                                .styled(
                                    Style::default()
                                        .size(Dimension::Px(320.0), Dimension::Px(146.0))
                                        .align(Align::Center)
                                        .justify(Justify::Center)
                                        .gap(10.0)
                                        .background(BG_CARD)
                                        .border(1.0, Color::rgb8(56, 66, 80))
                                        .radius(10.0),
                                )
                                .child(label(
                                    124,
                                    "studio.center.card.title",
                                    "UI-0 / UI-1",
                                    180.0,
                                    21.0,
                                    TEXT,
                                ))
                                .child(label(
                                    125,
                                    "studio.center.card.subtitle",
                                    "Declarative layout and batched paint",
                                    254.0,
                                    12.0,
                                    TEXT_MUTED,
                                ))
                                .child(
                                    panel(ElementId::new(126), "studio.center.card.rule").styled(
                                        Style::default()
                                            .size(Dimension::Px(82.0), Dimension::Px(3.0))
                                            .background(ACCENT)
                                            .radius(2.0),
                                    ),
                                ),
                        ),
                ),
        )
}

fn right_panel() -> Element {
    panel(demo_ids::RIGHT, "studio.right")
        .styled(
            Style::default()
                .size(Dimension::Px(292.0), Dimension::Auto)
                .background(BG_PANEL)
                .border(1.0, BORDER)
                .clipped(),
        )
        .child(section_header(130, "studio.right.header", "DETAILS"))
        .child(
            panel(ElementId::new(131), "studio.right.content")
                .styled(
                    Style::default()
                        .flex(1.0, 1.0)
                        .gap(8.0)
                        .padding(Insets::all(12.0)),
                )
                .children([
                    property_row(132, "studio.right.name", "Name", "Workspace"),
                    property_row(135, "studio.right.kind", "Kind", "Static outline"),
                    property_row(138, "studio.right.scale", "Scale", "Logical pixels"),
                ]),
        )
}

fn status_bar() -> Element {
    panel(demo_ids::STATUS, "studio.status")
        .styled(
            Style::default()
                .size(Dimension::Auto, Dimension::Px(28.0))
                .axis(Axis::Row)
                .align(Align::Center)
                .padding(Insets::symmetric(12.0, 0.0))
                .background(Color::rgb8(21, 25, 31))
                .border(1.0, BORDER),
        )
        .child(label(
            150,
            "studio.status.ready",
            "READY",
            54.0,
            10.0,
            Color::rgb8(98, 201, 148),
        ))
        .child(label(
            151,
            "studio.status.message",
            "  Reactive redraw  |  deterministic paint order",
            330.0,
            11.0,
            TEXT_MUTED,
        ))
        .child(
            Element::container(ElementId::new(152), key("studio.status.spacer"))
                .styled(Style::default().flex(1.0, 1.0)),
        )
        .child(label(
            153,
            "studio.status.backend",
            "BEVY / WGPU",
            96.0,
            10.0,
            TEXT_MUTED,
        ))
}

fn panel(id: ElementId, stable_key: &'static str) -> Element {
    Element::container(id, key(stable_key))
}

fn section_header(id: u64, stable_key: &'static str, text: &'static str) -> Element {
    panel(ElementId::new(id), stable_key)
        .styled(
            Style::default()
                .size(Dimension::Auto, Dimension::Px(40.0))
                .axis(Axis::Row)
                .align(Align::Center)
                .padding(Insets::symmetric(12.0, 0.0))
                .background(Color::rgb8(26, 31, 39))
                .border(1.0, BORDER_SUBTLE),
        )
        .child(label(
            id + 1_000,
            match stable_key {
                "studio.left.header" => "studio.left.header.label",
                "studio.center.header" => "studio.center.header.label",
                "studio.right.header" => "studio.right.header.label",
                _ => "studio.section.header.label",
            },
            text,
            180.0,
            11.0,
            TEXT_MUTED,
        ))
}

fn list_row(id: u64, stable_key: &'static str, text: &'static str, selected: bool) -> Element {
    let background = if selected {
        Color::rgb8(38, 55, 76)
    } else {
        Color::rgb8(25, 29, 36)
    };
    panel(ElementId::new(id), stable_key)
        .styled(
            Style::default()
                .size(Dimension::Auto, Dimension::Px(30.0))
                .axis(Axis::Row)
                .align(Align::Center)
                .padding(Insets::symmetric(9.0, 0.0))
                .background(background)
                .radius(5.0),
        )
        .child(label(
            id + 1_000,
            match stable_key {
                "studio.left.row.workspace" => "studio.left.row.workspace.label",
                "studio.left.row.sources" => "studio.left.row.sources.label",
                "studio.left.row.outputs" => "studio.left.row.outputs.label",
                "studio.left.row.packages" => "studio.left.row.packages.label",
                _ => "studio.left.row.label",
            },
            text,
            180.0,
            12.0,
            if selected { TEXT } else { TEXT_MUTED },
        ))
}

fn property_row(
    id: u64,
    stable_key: &'static str,
    name: &'static str,
    value: &'static str,
) -> Element {
    panel(ElementId::new(id), stable_key)
        .styled(
            Style::default()
                .size(Dimension::Auto, Dimension::Px(48.0))
                .axis(Axis::Column)
                .gap(3.0)
                .padding(Insets::symmetric(8.0, 6.0))
                .background(Color::rgb8(26, 31, 39))
                .border(1.0, BORDER_SUBTLE)
                .radius(5.0),
        )
        .child(label(
            id + 1,
            match stable_key {
                "studio.right.name" => "studio.right.name.label",
                "studio.right.kind" => "studio.right.kind.label",
                "studio.right.scale" => "studio.right.scale.label",
                _ => "studio.right.property.label",
            },
            name,
            90.0,
            10.0,
            TEXT_MUTED,
        ))
        .child(label(
            id + 2,
            match stable_key {
                "studio.right.name" => "studio.right.name.value",
                "studio.right.kind" => "studio.right.kind.value",
                "studio.right.scale" => "studio.right.scale.value",
                _ => "studio.right.property.value",
            },
            value,
            180.0,
            12.0,
            TEXT,
        ))
}

fn label(
    id: u64,
    stable_key: &'static str,
    text: &'static str,
    width: f32,
    font_size: f32,
    color: Color,
) -> Element {
    Element::text(ElementId::new(id), key(stable_key), text).styled(
        Style::default()
            .size(Dimension::Px(width), Dimension::Px(font_size * 1.35))
            .text(font_size, color),
    )
}

fn key(value: &'static str) -> ElementKey {
    ElementKey::new(value)
}
