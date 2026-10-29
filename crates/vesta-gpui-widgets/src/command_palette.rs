//! Searchable command surface with host-owned query and typed actions.

use crate::{COMMAND_PALETTE_ROW_HEIGHT, Widget, WidgetIdentity, WidgetTheme};
use vesta_gpui::{
    Axis, Color, Dimension, Element, Point, Rect, Semantic, SemanticRole, Size, Style,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandPaletteIds {
    pub backdrop: WidgetIdentity,
    pub panel: WidgetIdentity,
    pub title: WidgetIdentity,
    pub list: WidgetIdentity,
    pub empty: WidgetIdentity,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommandPaletteEntry<A> {
    pub root: WidgetIdentity,
    pub label: String,
    pub shortcut: String,
    pub highlighted: bool,
    pub action: Option<A>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandPaletteScrollIds {
    pub base: CommandPaletteIds,
    pub content: WidgetIdentity,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CommandPaletteGeometry {
    pub panel: Rect,
    pub list_height: f32,
}

#[must_use]
pub fn command_palette_geometry(
    viewport: Size,
    count: usize,
    theme: WidgetTheme,
) -> CommandPaletteGeometry {
    let finite = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
    let width = (finite(viewport.width) - 32.0).clamp(0.0, 500.0);
    let header =
        26.0 + theme.typography.heading_font_size * 1.5 + theme.metrics.control_height + 16.0;
    let height = (header + count.max(1) as f32 * COMMAND_PALETTE_ROW_HEIGHT)
        .min((finite(viewport.height) - 32.0).max(0.0));
    CommandPaletteGeometry {
        panel: Rect::new(
            (finite(viewport.width) - width) / 2.0,
            50.0_f32.min((finite(viewport.height) - height) / 2.0),
            width,
            height,
        ),
        list_height: (height - header).max(0.0),
    }
}

pub fn command_palette<A>(
    ids: CommandPaletteIds,
    viewport: Size,
    editor: Element,
    entries: Vec<CommandPaletteEntry<A>>,
    theme: WidgetTheme,
) -> Widget<A> {
    let geometry = command_palette_geometry(viewport, entries.len(), theme);
    let offset = entries
        .iter()
        .position(|entry| entry.highlighted)
        .map_or(0.0, |i| {
            ((i + 1) as f32 * COMMAND_PALETTE_ROW_HEIGHT - geometry.list_height).max(0.0)
        });
    project(ids, None, viewport, editor, entries, offset, theme)
}

/// Explicit host-owned scroll with bounded result projection. Stable row IDs come from the
/// entries, never from their visible indices. The editor is the sole keyboard focus stop.
pub fn command_palette_scrolled<A>(
    ids: CommandPaletteScrollIds,
    viewport: Size,
    editor: Element,
    entries: Vec<CommandPaletteEntry<A>>,
    offset: f32,
    theme: WidgetTheme,
) -> Widget<A> {
    project(
        ids.base,
        Some(ids.content),
        viewport,
        editor,
        entries,
        offset,
        theme,
    )
}

fn project<A>(
    ids: CommandPaletteIds,
    content: Option<WidgetIdentity>,
    viewport: Size,
    mut editor: Element,
    entries: Vec<CommandPaletteEntry<A>>,
    offset: f32,
    theme: WidgetTheme,
) -> Widget<A> {
    let geometry = command_palette_geometry(viewport, entries.len(), theme);
    let virtualized = content.is_some();
    let content_height = entries.len().max(1) as f32 * COMMAND_PALETTE_ROW_HEIGHT;
    let offset = if offset.is_finite() {
        offset.clamp(0.0, (content_height - geometry.list_height).max(0.0))
    } else {
        0.0
    };
    let mut list = Widget::from_element(
        Element::scroll_box(ids.list.element, ids.list.key, Point::new(0.0, offset))
            .semantic(Semantic::new(SemanticRole::ListBox, "Commands"))
            .styled(
                Style::default()
                    .axis(Axis::Column)
                    .size(Dimension::Percent(1.0), Dimension::Px(geometry.list_height))
                    .flex(0.0, 0.0)
                    .scrollbar(theme.scrollbar_style()),
            ),
    );
    let mut body = content.map(|id| {
        Widget::from_element(
            Element::container(id.element, id.key).styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Px(content_height))
                    .flex(0.0, 0.0),
            ),
        )
    });
    if entries.is_empty() {
        let empty = Widget::from_element(
            Element::text(ids.empty.element, ids.empty.key, "No matching commands")
                .semantic(Semantic::new(SemanticRole::Label, "No matching commands"))
                .styled(
                    Style::default()
                        .size(
                            Dimension::Percent(1.0),
                            Dimension::Px(COMMAND_PALETTE_ROW_HEIGHT),
                        )
                        .flex(0.0, 0.0)
                        .text(theme.typography.font_size, theme.palette.text_muted)
                        .pointer_events(false),
                ),
        );
        if let Some(body) = body.as_mut() {
            body.element.children.push(empty.element);
        } else {
            list = list.child(empty);
        }
    }
    let first = ((offset / COMMAND_PALETTE_ROW_HEIGHT).floor() as usize).saturating_sub(1);
    let end = (((offset + geometry.list_height) / COMMAND_PALETTE_ROW_HEIGHT).ceil() as usize)
        .saturating_add(1);
    for (index, entry) in entries.into_iter().enumerate() {
        if virtualized && (index < first || index >= end) {
            continue;
        }
        let name = format!("{}    {}", entry.label, entry.shortcut);
        let enabled = entry.action.is_some();
        let element = Element::text(entry.root.element, entry.root.key, &name)
            .semantic(
                Semantic::new(SemanticRole::ListBoxOption, &entry.label)
                    .description(if enabled {
                        &entry.shortcut
                    } else {
                        "Unavailable"
                    })
                    .selected(entry.highlighted)
                    .activatable(enabled),
            )
            .styled(
                Style::default()
                    .size(
                        Dimension::Percent(1.0),
                        Dimension::Px(COMMAND_PALETTE_ROW_HEIGHT),
                    )
                    .flex(0.0, 0.0)
                    .clipped()
                    .text(
                        theme.typography.font_size,
                        if enabled {
                            theme.palette.text
                        } else {
                            theme.palette.text_disabled
                        },
                    )
                    .background(if entry.highlighted {
                        theme.palette.selection
                    } else {
                        theme.palette.menu_surface
                    })
                    .focusable(enabled && !virtualized)
                    .action_boundary(!enabled)
                    .pointer_events(enabled),
            );
        let mut row = Widget::from_element(element);
        if virtualized {
            row.element.style = row
                .element
                .style
                .absolute(0.0, index as f32 * COMMAND_PALETTE_ROW_HEIGHT);
        }
        if let Some(action) = entry.action {
            row = row.on_click(action);
        }
        if let Some(body) = body.as_mut() {
            // Move through the public composition path, preserving the row's action map.
            body.actions.extend(row.actions);
            body.element.children.push(row.element);
        } else {
            list = list.child(row);
        }
    }
    if let Some(body) = body {
        list = list.child(body);
    }
    editor.style = editor.style.flex(0.0, 0.0);
    let panel = Widget::from_element(
        Element::container(ids.panel.element, ids.panel.key)
            .semantic(Semantic::new(SemanticRole::Dialog, "Command palette"))
            .styled(
                Style::default()
                    .absolute(geometry.panel.x, geometry.panel.y)
                    .size(
                        Dimension::Px(geometry.panel.width),
                        Dimension::Px(geometry.panel.height),
                    )
                    .clipped()
                    .axis(Axis::Column)
                    .gap(8.0)
                    .padding(vesta_gpui::Insets::all(12.0))
                    .background(theme.palette.menu_surface)
                    .border(1.0, theme.palette.border),
            )
            .child(
                Element::text(ids.title.element, ids.title.key, "Commands").styled(
                    Style::default()
                        .size(
                            Dimension::Percent(1.0),
                            Dimension::Px(theme.typography.heading_font_size * 1.5),
                        )
                        .flex(0.0, 0.0)
                        .text(theme.typography.heading_font_size, theme.palette.text)
                        .pointer_events(false),
                ),
            )
            .child(editor),
    )
    .child(list);
    Widget::from_element(
        Element::container(ids.backdrop.element, ids.backdrop.key).styled(
            Style::default()
                .absolute(0.0, 0.0)
                .size(
                    Dimension::Px(viewport.width),
                    Dimension::Px(viewport.height),
                )
                .background(Color::srgba(0.0, 0.0, 0.0, 0.48))
                .z_index(190),
        ),
    )
    .child(panel)
}
