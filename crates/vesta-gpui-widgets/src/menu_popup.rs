//! Complete menu composition; descriptor actions and identities remain host-owned.
use crate::{
    ControlState, MenuController, MenuItemIds, MenuItemView, PopupDirection, Widget,
    WidgetIdentity, WidgetTheme, menu_item, place_popup,
};
use vesta_gpui::{
    Align, Axis, ContentConstraint, Dimension, Element, Insets, LayoutEngine, LayoutError, Menu,
    MenuItem, MenuItemId, Rect, ScrollbarGutter, Semantic, SemanticRole, Style, TextInputContext,
};

const CONTENT_INSET: f32 = 6.0;
const FRAME_WIDTH: f32 = 1.0;
const SEPARATOR_HEIGHT: f32 = 9.0;

pub struct MenuPopupSpec {
    pub root: WidgetIdentity,
    pub content: WidgetIdentity,
    pub anchor: Rect,
    pub bounds: Rect,
    pub direction: PopupDirection,
    pub z_index: i32,
    pub hovered: Option<vesta_gpui::ElementId>,
}

/// A model/option picker uses the same menu routes and lifecycle as application menus.
pub struct MenuPickerPopupSpec {
    pub popup: MenuPopupSpec,
    pub title: WidgetIdentity,
}

enum Presentation {
    Application,
    Picker(
        WidgetIdentity,
        bool,
        Option<Box<Element>>,
        Option<Box<Element>>,
        Option<Box<MenuPickerColumns>>,
    ),
}

/// Optional controls measured with the picker rows; title visibility is explicit.
pub struct MenuPickerContent<A> {
    pub show_title: bool,
    pub header: Option<Widget<A>>,
    pub footer: Option<Widget<A>>,
    pub columns: Option<MenuPickerColumns>,
}

/// Two adjacent groups retain the same menu item identities and action routes.
pub struct MenuPickerColumns {
    pub row: WidgetIdentity,
    pub left: WidgetIdentity,
    pub right: WidgetIdentity,
    pub divider: WidgetIdentity,
    pub split_before: MenuItemId,
    pub width: f32,
}

/// Measures with the consumer's font context and projects the same decoration in every host.
/// A child menu may use the returned root bounds as its next anchor. Identity and command
/// mapping callbacks are data adapters; neither callback executes a command.
pub fn menu_popup<A, B>(
    menu: &Menu<A>,
    controller: &MenuController,
    spec: MenuPopupSpec,
    theme: WidgetTheme,
    text: &mut TextInputContext,
    identity: impl FnMut(MenuItemId) -> MenuItemIds,
    action: impl FnMut(&vesta_gpui::MenuItem<A>) -> B,
) -> Result<Widget<B>, LayoutError> {
    project_popup(
        menu,
        controller,
        spec,
        theme,
        text,
        identity,
        action,
        Presentation::Application,
    )
}

/// Compact, grouped choices with a visible title and current-choice marks on the right.
pub fn menu_picker_popup<A, B>(
    menu: &Menu<A>,
    controller: &MenuController,
    spec: MenuPickerPopupSpec,
    theme: WidgetTheme,
    text: &mut TextInputContext,
    identity: impl FnMut(MenuItemId) -> MenuItemIds,
    action: impl FnMut(&vesta_gpui::MenuItem<A>) -> B,
) -> Result<Widget<B>, LayoutError> {
    menu_picker_popup_with_header(menu, controller, spec, theme, text, identity, action, None)
}

/// A measured control header participates in popup placement and shares its owner.
#[allow(clippy::too_many_arguments)]
pub fn menu_picker_popup_with_header<A, B>(
    menu: &Menu<A>,
    controller: &MenuController,
    spec: MenuPickerPopupSpec,
    theme: WidgetTheme,
    text: &mut TextInputContext,
    identity: impl FnMut(MenuItemId) -> MenuItemIds,
    action: impl FnMut(&vesta_gpui::MenuItem<A>) -> B,
    header: Option<Widget<B>>,
) -> Result<Widget<B>, LayoutError> {
    menu_picker_popup_with_content(
        menu,
        controller,
        spec,
        theme,
        text,
        identity,
        action,
        MenuPickerContent {
            show_title: true,
            header,
            footer: None,
            columns: None,
        },
    )
}

/// Header and footer participate in measurement before popup placement.
#[allow(clippy::too_many_arguments)]
pub fn menu_picker_popup_with_content<A, B>(
    menu: &Menu<A>,
    controller: &MenuController,
    spec: MenuPickerPopupSpec,
    theme: WidgetTheme,
    text: &mut TextInputContext,
    identity: impl FnMut(MenuItemId) -> MenuItemIds,
    action: impl FnMut(&vesta_gpui::MenuItem<A>) -> B,
    content: MenuPickerContent<B>,
) -> Result<Widget<B>, LayoutError> {
    let mut actions = Vec::new();
    let mut element = |widget: Option<Widget<B>>| {
        widget.map(|widget| {
            actions.extend(widget.actions);
            Box::new(widget.element)
        })
    };
    let header = element(content.header);
    let footer = element(content.footer);
    let mut popup = project_popup(
        menu,
        controller,
        spec.popup,
        theme,
        text,
        identity,
        action,
        Presentation::Picker(
            spec.title,
            content.show_title,
            header,
            footer,
            content.columns.map(Box::new),
        ),
    )?;
    actions.extend(popup.actions);
    popup.actions = actions;
    Ok(popup)
}

#[allow(clippy::too_many_arguments)]
fn project_popup<A, B>(
    menu: &Menu<A>,
    controller: &MenuController,
    spec: MenuPopupSpec,
    theme: WidgetTheme,
    text: &mut TextInputContext,
    mut identity: impl FnMut(MenuItemId) -> MenuItemIds,
    mut action: impl FnMut(&vesta_gpui::MenuItem<A>) -> B,
    presentation: Presentation,
) -> Result<Widget<B>, LayoutError> {
    if !spec.bounds.width.is_finite()
        || !spec.bounds.height.is_finite()
        || spec.bounds.width <= 0.0
        || spec.bounds.height <= 0.0
    {
        return Err(LayoutError::InvalidConstraint);
    }
    let mut content = Element::container(spec.content.element, spec.content.key)
        .styled(Style::default().axis(Axis::Column));
    let picker = matches!(presentation, Presentation::Picker(..));
    let inset = CONTENT_INSET;
    let mut title_height = if matches!(presentation, Presentation::Picker(_, true, ..)) {
        theme.metrics.control_height
    } else {
        0.0
    };
    let mut footer = None;
    let mut columns = None;
    if let Presentation::Picker(title, show_title, header, tail, groups) = presentation {
        footer = tail;
        columns = groups;
        if show_title {
            content = content.child(
                Element::text(title.element, title.key, &menu.label)
                    .semantic(Semantic::new(SemanticRole::Label, &menu.label))
                    .styled(
                        Style::default()
                            .size(Dimension::Auto, Dimension::Px(title_height))
                            .margin(Insets::symmetric(8.0, 0.0))
                            .text(theme.typography.small_font_size, theme.palette.text_muted)
                            .pointer_events(false),
                    ),
            );
        }
        if let Some(header) = header {
            title_height += LayoutEngine
                .measure_with_text_context(&header, ContentConstraint::default(), text)?
                .preferred
                .height;
            content = content.child(*header);
        }
    }
    let mut actions = Vec::new();
    let mut rows = Vec::new();
    for item in &menu.items {
        let ids = identity(item.id);
        if item.separator {
            let mut section = section_row(item, ids, theme);
            if picker && item.section_header.is_some() {
                // Group labels in a picker are not horizontal menu separators.
                section.children.pop();
            }
            rows.push(section);
            continue;
        }
        let state = if !item.enabled {
            ControlState::Disabled
        } else if controller.highlighted_in(menu.id) == Some(item.id)
            || spec.hovered == Some(ids.root.element)
        {
            ControlState::Hovered
        } else {
            ControlState::Idle
        };
        let mut control = menu_item(
            ids,
            MenuItemView {
                label: &item.label,
                shortcut: item.shortcut.as_deref(),
                checked: item.checked,
                has_submenu: item.submenu.is_some(),
            },
            action(item),
            state,
            theme,
        );
        control.element.tooltip = Some(item.tooltip.as_ref().unwrap_or(&item.label).clone());
        if let Some(resource) = &item.icon
            && (!picker || item.submenu.is_some())
            && item.checked != Some(true)
        {
            let slot = &mut control.element.children[0];
            let mut image = Element::image(slot.id, slot.key.clone(), resource);
            image.style = slot
                .style
                .clone()
                .size(Dimension::Px(18.0), Dimension::Px(16.0));
            *slot = image;
        }
        if picker {
            let mut check = control.element.children.remove(0);
            if item.checked.is_none() {
                check.style.width = Dimension::Px(0.0);
            }
            if let Some(resource) = item.icon.as_ref().filter(|_| item.submenu.is_none()) {
                // Leading category and trailing selection are independent. Reuse
                // the ordinary row's spare arrow identity for its trailing check.
                let mut icon = Element::image(check.id, check.key.clone(), resource);
                icon.style = check
                    .style
                    .clone()
                    .size(Dimension::Px(18.0), Dimension::Px(16.0));
                control.element.children.insert(0, icon);
                if let Some(arrow) = control.element.children.pop() {
                    check.id = arrow.id;
                    check.key = arrow.key;
                }
            } else if item.submenu.is_none() {
                control.element.children.pop();
            }
            control.element.children.push(check);
            control.element.style.visual.corner_radius = theme.metrics.radius.max(4.0);
        }
        actions.extend(control.actions);
        rows.push(control.element);
    }
    let split_before = columns.as_ref().map(|groups| groups.split_before);
    if let Some(groups) = columns {
        let split = menu
            .items
            .iter()
            .position(|item| item.id == groups.split_before)
            .unwrap_or(rows.len());
        let column = |id: WidgetIdentity, rows: Vec<Element>| {
            let mut element = Element::container(id.element, id.key).styled(
                Style::default()
                    .axis(Axis::Column)
                    .min_width(Dimension::Px(0.0))
                    .flex(1.0, 1.0),
            );
            element.style.flex_basis = Some(Dimension::Px(0.0));
            for row in rows {
                element = element.child(row);
            }
            element
        };
        let right = rows.split_off(split);
        content = content.child(
            Element::container(groups.row.element, groups.row.key)
                .styled(
                    Style::default()
                        .size(Dimension::Px(groups.width), Dimension::Auto)
                        .axis(Axis::Row)
                        .align(Align::Stretch)
                        .gap(8.0),
                )
                .child(column(groups.left, rows))
                .child(
                    Element::container(groups.divider.element, groups.divider.key).styled(
                        Style::default()
                            .size(Dimension::Px(1.0), Dimension::Auto)
                            .flex(0.0, 0.0)
                            .background(theme.palette.chrome_divider)
                            .pointer_events(false),
                    ),
                )
                .child(column(groups.right, right)),
        );
    } else {
        for row in rows {
            content = content.child(row);
        }
    }
    if let Some(footer) = footer {
        content = content.child(*footer);
    }
    let desired =
        LayoutEngine.measure_with_text_context(&content, ContentConstraint::default(), text)?;
    // Visual borders do not occupy layout space in GPUI.
    let width = if picker {
        (desired.preferred.width + inset * 2.0)
            .ceil()
            .clamp(320.0, if split_before.is_some() { 560.0 } else { 360.0 })
    } else {
        (desired.preferred.width + inset * 2.0).ceil().max(160.0)
    }
    .min(spec.bounds.width);
    let height = (desired.preferred.height + inset * 2.0)
        .min(if picker { 420.0 } else { spec.bounds.height })
        .min(spec.bounds.height);
    let viewport_height = (height - inset * 2.0).max(0.0);
    let scroll_y = menu_scroll_y(
        menu,
        controller,
        viewport_height,
        theme,
        title_height,
        split_before,
    );
    let size = vesta_gpui::Size::new(width, height);
    let position = place_popup(spec.anchor, size, spec.bounds, spec.direction);
    let mut scrollbar = theme.scrollbar_style();
    scrollbar.gutter = ScrollbarGutter::Overlay;
    Ok(Widget {
        element: Element::scroll_box(
            spec.root.element,
            spec.root.key,
            vesta_gpui::Point::new(0.0, scroll_y),
        )
        .semantic(Semantic::new(SemanticRole::Menu, &menu.label))
        .styled(
            Style::default()
                .absolute(position.x, position.y)
                .size(Dimension::Px(width), Dimension::Px(height))
                .axis(Axis::Column)
                .padding(Insets::all(inset))
                .background(theme.palette.menu_surface)
                .border(FRAME_WIDTH, theme.palette.chrome_divider)
                .radius(8.0)
                .z_index(spec.z_index)
                .scrollbar(scrollbar)
                .pointer_events(true),
        )
        .child(content),
        actions,
    })
}

fn row_height<A>(item: &MenuItem<A>, theme: WidgetTheme) -> f32 {
    if item.separator {
        if item.section_header.is_some() {
            theme.metrics.compact_control_height
        } else {
            SEPARATOR_HEIGHT
        }
    } else {
        theme.metrics.control_height
    }
}

/// Named and anonymous group boundaries block pointer fall-through without becoming commands.
fn section_row<A>(item: &MenuItem<A>, ids: MenuItemIds, theme: WidgetTheme) -> Element {
    let mut row = Element::container(ids.root.element, ids.root.key).styled(
        Style::default()
            .size(
                Dimension::Percent(1.0),
                Dimension::Px(row_height(item, theme)),
            )
            .axis(Axis::Row)
            .align(Align::Center)
            .gap(8.0)
            .padding(Insets::symmetric(10.0, 0.0))
            .pointer_events(true)
            .action_boundary(true),
    );
    if let Some(name) = &item.section_header {
        row = row
            .semantic(Semantic::new(SemanticRole::Label, name))
            .child(
                Element::text(ids.label.element, ids.label.key, name).styled(
                    Style::default()
                        .text(theme.typography.small_font_size, theme.palette.text_muted)
                        .pointer_events(false),
                ),
            );
    }
    row.child(
        Element::container(ids.arrow.element, ids.arrow.key).styled(
            Style::default()
                .size(Dimension::Auto, Dimension::Px(1.0))
                .flex(1.0, 1.0)
                .background(theme.palette.chrome_divider)
                .pointer_events(false),
        ),
    )
}

fn menu_scroll_y<A>(
    menu: &Menu<A>,
    controller: &MenuController,
    viewport_height: f32,
    theme: WidgetTheme,
    title_height: f32,
    split_before: Option<MenuItemId>,
) -> f32 {
    let mut top = title_height;
    let mut scroll_y = 0.0;
    for item in &menu.items {
        if Some(item.id) == split_before {
            top = title_height;
        }
        let row_height = row_height(item, theme);
        if controller.highlighted_in(menu.id) == Some(item.id) {
            scroll_y = (top + row_height - viewport_height).max(0.0);
        }
        top += row_height;
    }
    scroll_y
}

/// Row geometry used to anchor a submenu before the final retained layout is built.
#[must_use]
pub fn menu_submenu_anchor<A>(
    menu: &Menu<A>,
    child: vesta_gpui::MenuId,
    controller: &MenuController,
    popup: Rect,
    theme: WidgetTheme,
) -> Option<Rect> {
    let mut top = popup.y + CONTENT_INSET
        - menu_scroll_y(
            menu,
            controller,
            (popup.height - CONTENT_INSET * 2.0).max(0.0),
            theme,
            0.0,
            None,
        );
    for item in &menu.items {
        let height = row_height(item, theme);
        if item.submenu == Some(child) {
            return Some(Rect::new(popup.x, top, popup.width, height));
        }
        top += height;
    }
    None
}
