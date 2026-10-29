//! Workspace container composition shared by the main and floating Gallery.

use super::*;

pub(super) fn build_workspace(
    input: &UiBuildState<'_>,
    mut pages: HashMap<&str, Vec<Element>>,
    ids: &mut Ids,
    actions: &mut Vec<(ElementId, DemoAction)>,
) -> (Element, WorkspaceViewRegions) {
    let scope = input.scopes.gallery;
    let workspace_scope = input.scopes.workspace;
    let demo = input.demo;
    let theme = input.theme;
    let scrollbar = theme.scrollbar_style();
    let scroll_offset = input.scroll_offset;
    let workspace = workspace_container_view_with_header(
        workspace_scope,
        &demo.workspace,
        input.workspace_container,
        theme,
        WorkspaceViewInteraction {
            hovered: input.hovered,
            focused: input.focused_element,
            close_on_hover: true,
        },
        demo.zoomed_panes.get(&input.workspace_container).copied(),
        |pane| {
            let history = demo.pane_history.get(&pane.id);
            let can_back = history
                .is_some_and(|history| history.target(PaneNavigationDirection::Back).is_some());
            let can_forward = history
                .is_some_and(|history| history.target(PaneNavigationDirection::Forward).is_some());
            let pane_width = demo
                .pane_widths
                .get(&pane.id)
                .copied()
                .unwrap_or(f32::INFINITY);
            let mut commands = Vec::new();
            if pane.active.is_some() {
                if pane_width < 280.0 {
                    commands.push(PaneHeaderCommand {
                        id: HEADER_OVERFLOW,
                        caption: "⋯".into(),
                        name: "Pane actions".into(),
                        enabled: true,
                    });
                } else {
                    commands.push(PaneHeaderCommand {
                        id: HEADER_NEW_TAB,
                        caption: "+".into(),
                        name: "New Gallery tab".into(),
                        enabled: true,
                    });
                    commands.push(PaneHeaderCommand {
                        id: HEADER_SPLIT_RIGHT,
                        caption: "◫".into(),
                        name: "Move active tab into a pane on the right".into(),
                        enabled: pane.items.len() > 1,
                    });
                    commands.push(PaneHeaderCommand {
                        id: HEADER_TOGGLE_ZOOM,
                        caption: "⤢".into(),
                        name:
                            if demo.zoomed_panes.get(&input.workspace_container) == Some(&pane.id) {
                                "Restore split panes"
                            } else {
                                "Zoom this pane"
                            }
                            .into(),
                        enabled: container_panes(&demo.workspace, input.workspace_container).len()
                            > 1,
                    });
                    if input.workspace_container != ContainerRef::Main {
                        commands.push(PaneHeaderCommand {
                            id: HEADER_OVERFLOW,
                            caption: "⋯".into(),
                            name: "Pane actions".into(),
                            enabled: true,
                        });
                    }
                }
            }
            PaneHeader {
                tab_scrollbar: Some(scrollbar),
                navigation: (pane.active.is_some() && pane_width >= 180.0)
                    .then_some((can_back, can_forward)),
                commands,
                tab_scroll_x: demo
                    .tab_scroll
                    .get(&pane.id)
                    .map_or(0.0, |scroll| scroll.offset().x),
                ..PaneHeader::default()
            }
        },
        |pane| {
            let item = pane.active_item()?;
            let item_local = u32::try_from(item.id.0).ok()?;
            if item.panel_type.as_str() == "gallery.blank" {
                return Some(
                    Element::text(
                        scope.element(GALLERY_PAGE_CONTENT_BASE + item_local),
                        ElementKey::new(format!("gallery.page.{}.blank", item.id.0)),
                        "New Gallery tab",
                    )
                    .styled(
                        Style::default()
                            .size(Dimension::Percent(1.0), Dimension::Px(40.0))
                            .padding(Insets::all(12.0))
                            .text(theme.typography.font_size, theme.palette.text_muted),
                    ),
                );
            }
            let Some(sections) = pages.remove(item.panel_type.as_str()) else {
                return Some(
                    Element::text(
                        scope.element(GALLERY_PAGE_CONTENT_BASE + item_local),
                        ElementKey::new(format!("gallery.page.{}.unavailable", item.id.0)),
                        format!("Panel unavailable: {}", item.panel_type.as_str()),
                    )
                    .styled(
                        Style::default()
                            .size(Dimension::Percent(1.0), Dimension::Px(40.0))
                            .padding(Insets::all(12.0))
                            .text(theme.typography.font_size, theme.palette.text_muted),
                    ),
                );
            };
            let page_content = Element::container(
                scope.element(GALLERY_PAGE_CONTENT_BASE + item_local),
                ElementKey::new(format!("gallery.page.{}.content", item.id.0)),
            )
            .styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Auto)
                    .axis(Axis::Column)
                    .gap(0.0)
                    .padding(Insets::all(0.0))
                    .background(theme.palette.canvas),
            )
            .children(sections);
            let mut page_content = page_content;
            page_content.style.clip = ClipMode::Inherit;
            Some(
                Element::scroll_box(
                    page_scroll_id(scope, item.id),
                    ElementKey::new(format!("gallery.page.{}.scroll", item.id.0)),
                    demo.page_scroll
                        .get(&item.id)
                        .map_or(Point::default(), |state| state.offset()),
                )
                .styled(
                    Style::default()
                        .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                        .scrollbar(scrollbar)
                        .background(theme.palette.canvas),
                )
                .child(page_content),
            )
        },
    )
    .ok()
    .flatten();
    let mut workspace_element = workspace.as_ref().map_or_else(
        || control(ids, "Workspace unavailable", theme.palette.danger, theme),
        |view| view.element.clone(),
    );
    workspace_element.style.height = Dimension::Percent(1.0);
    let workspace_regions = workspace
        .as_ref()
        .map_or_else(WorkspaceViewRegions::default, |view| view.regions.clone());
    if let Some(view) = workspace {
        actions.extend(
            view.actions
                .into_iter()
                .map(|(id, action)| (id, DemoAction::Workspace(action))),
        );
    }
    let vertical_chrome = if input.workspace_container == ContainerRef::Main {
        CHROME_HEIGHT + CHROME_STATUS_HEIGHT
    } else {
        0.0
    };
    let content_height = (input.viewport_size.height - vertical_chrome).max(0.0);
    let content = Element::container(ids.id(), ids.key("gallery.content"))
        .styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Px(content_height))
                .axis(Axis::Column)
                .gap(0.0)
                .background(theme.palette.canvas),
        )
        .child(
            Element::container(ids.id(), ids.key("workspace.host"))
                .styled(
                    Style::default()
                        .size(Dimension::Percent(1.0), Dimension::Px(content_height))
                        .min_height(Dimension::Px(0.0))
                        .background(theme.palette.canvas),
                )
                .child(workspace_element),
        );

    let scroll = Element::scroll_box(
        scope.element(3),
        ElementKey::new("gallery.scroll"),
        scroll_offset,
    )
    .styled(
        Style::default()
            .absolute(
                0.0,
                if input.workspace_container == ContainerRef::Main {
                    CHROME_HEIGHT
                } else {
                    0.0
                },
            )
            .scrollbar(scrollbar)
            .size(Dimension::Percent(1.0), Dimension::Px(content_height))
            .background(theme.palette.canvas),
    )
    .child(content);
    (scroll, workspace_regions)
}
