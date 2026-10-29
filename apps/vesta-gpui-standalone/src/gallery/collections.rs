//! Typed collection content shared by ordinary and virtual List, Tree, Table and Tile consumers.

use super::*;

pub(super) fn sample_tile_grid(
    input: &UiBuildState<'_>,
    collection_content_width: f32,
) -> (Widget<DemoAction>, usize) {
    let scope = input.scopes.gallery;
    let demo = input.demo;
    let theme = input.theme;
    let tile_identity = |slot, part: &str| {
        WidgetIdentity::new(
            scope.element(TILE_ROOT + slot),
            format!("gallery.tile.{part}"),
        )
    };
    let tile_objects = (0_u8..9)
        .map(|id| CollectionObject {
            id,
            name: [
                "Asset 00", "Asset 01", "Asset 02", "Asset 03", "Asset 04", "Asset 05", "Asset 06",
                "Asset 07", "Asset 08",
            ][usize::from(id)],
            thumbnail: "demo://checkerboard",
        })
        .collect::<Vec<_>>();
    let (tiles, tile_range) = tile_grid_windowed_with_content(
        TileGridIds {
            root: tile_identity(0, "root"),
            top: tile_identity(1, "top"),
            grid: tile_identity(2, "grid"),
            bottom: tile_identity(3, "bottom"),
        },
        tile_objects.len(),
        TileGridGeometry {
            viewport_width: collection_content_width.min(340.0),
            viewport_height: 500.0,
            scroll_y: 0.0,
            minimum_tile_width: 90.0,
            tile_height: 38.0,
            gap: 4.0,
        },
        |index| {
            let object = &tile_objects[index];
            let id = u32::from(object.id);
            let content_identity = |slot, part: &str| {
                tile_identity(1_000 + id * 8 + slot, &format!("item.{id}.{part}"))
            };
            let mut commands = button(
                content_identity(4, "commands"),
                content_identity(5, "commands.label"),
                "…",
                DemoAction::OpenCommandPalette,
                ControlState::Idle,
                theme,
            );
            commands.element.style.width = Dimension::Px(theme.metrics.control_height);
            TileGridContent {
                root: tile_identity(100 + id * 2, &format!("item.{id}")),
                content: collection_object_content(object, content_identity, Some(commands), theme),
                selected: demo.tile_selected == index,
                action: DemoAction::SelectTile(index),
            }
        },
        theme,
    )
    .expect("static gallery tiles");
    let tiles = tiles
        .conditioned(WidgetAvailability {
            visible: true,
            enabled: demo.toggle,
        })
        .expect("visible Gallery Tile subtree");
    (tiles, tile_range.columns)
}

pub(super) fn virtual_object_table(
    input: &UiBuildState<'_>,
    table_width: f32,
    scrollbar: vesta_gpui::ScrollbarStyle,
) -> Widget<DemoAction> {
    let scope = input.scopes.gallery;
    let demo = input.demo;
    let theme = input.theme;
    let virtual_table_identity = |slot, part: &str| {
        WidgetIdentity::new(
            scope.element(TABLE_VIRTUAL_ROOT + slot),
            format!("gallery.virtual_table.{part}"),
        )
    };
    let virtual_table_range = VirtualListRange::compute(
        TABLE_VIRTUAL_COUNT,
        TABLE_VIRTUAL_ROW_HEIGHT,
        input.virtual_table_viewport_height,
        demo.virtual_table_scroll.offset_y(),
        2,
    )
    .expect("bounded Gallery table geometry");
    let mut virtual_table = table_view_windowed_scrolled(
        TableWindowIds {
            root: virtual_table_identity(0, "root"),
            header: virtual_table_identity(1, "header"),
            viewport: virtual_table_identity(2, "viewport"),
            content: virtual_table_identity(3, "content"),
        },
        ["Name", "Kind", "Size"]
            .into_iter()
            .enumerate()
            .map(|(column, title)| TableColumn {
                header: virtual_table_identity(10 + column as u32, &format!("column.{column}")),
                title: title.to_owned(),
                width: [0.45, 0.30, 0.25][column] * table_width,
                sort_action: None,
                resize: None,
            })
            .collect(),
        virtual_table_range,
        demo.virtual_table_scroll.offset().x,
        input.virtual_table_focused,
        |index| {
            let row_slot = 1_000 + index as u32 * 4;
            let values = [
                format!("Asset {index:05}"),
                ["Mesh", "Field", "Curve"][index % 3].to_owned(),
                (index % 512).to_string(),
            ];
            TableRow {
                identity: virtual_table_identity(row_slot, &format!("row.{index}")),
                cells: values
                    .into_iter()
                    .enumerate()
                    .map(|(column, value)| TableCell {
                        identity: virtual_table_identity(
                            row_slot + 1 + column as u32,
                            &format!("row.{index}.cell.{column}"),
                        ),
                        value,
                        selected: false,
                        select_action: Some(DemoAction::SelectVirtualTable(index)),
                    })
                    .collect(),
                selected: demo.virtual_table_selected == index,
                select_action: DemoAction::SelectVirtualTable(index),
            }
        },
        theme,
    )
    .expect("stable windowed Gallery table identities");
    virtual_table.element.style.max_width = Dimension::Percent(1.0);
    let viewport = virtual_table
        .element
        .children
        .iter_mut()
        .find(|child| child.id == scope.element(TABLE_VIRTUAL_ROOT + 2))
        .expect("stable table body identity");
    viewport.style.scrollbar = scrollbar;
    // The range measures visible content; allocation also retains room for gutters.
    viewport.style.height = Dimension::Px(TABLE_VIRTUAL_VIEWPORT_HEIGHT);
    viewport.style.max_width = Dimension::Percent(1.0);
    virtual_table
}

pub(super) fn sample_object_table(
    input: &UiBuildState<'_>,
    table_width: f32,
    normalized_query: &str,
    actions: &mut Vec<(ElementId, DemoAction)>,
) -> Element {
    let scope = input.scopes.gallery;
    let demo = input.demo;
    let theme = input.theme;
    let mut table_data = [
        (1_u8, "Cube", "Mesh", 24_u16),
        (2_u8, "Sphere", "Mesh", 48_u16),
        (3_u8, "Terrain", "Field", 12_u16),
    ];
    table_data.sort_by(|left, right| {
        let order = match demo.table_sort_column {
            1 => left.2.cmp(right.2),
            2 => left.3.cmp(&right.3),
            _ => left.1.cmp(right.1),
        };
        let order = order.then_with(|| left.0.cmp(&right.0));
        if demo.table_sort_descending {
            order.reverse()
        } else {
            order
        }
    });
    let table_identity = |slot, part: &str| {
        WidgetIdentity::new(
            scope.element(TABLE_ROOT + slot),
            format!("gallery.table.{part}"),
        )
    };
    let column_widths =
        window_controls::table_column_widths(&demo.table_column_fractions, table_width)
            .expect("finite normalized Table proportions");
    let table_columns = ["Name", "Kind", "Size"]
        .into_iter()
        .enumerate()
        .map(|(index, title)| {
            let title = if demo.table_sort_column == index as u8 {
                format!(
                    "{title} {}",
                    if demo.table_sort_descending {
                        "↓"
                    } else {
                        "↑"
                    }
                )
            } else {
                title.to_owned()
            };
            TableColumnContent {
                header: table_identity(10 + index as u32, &format!("column.{index}")),
                content: WidgetContent::new(
                    &title,
                    Widget::from_element(
                        Element::text(
                            scope.element(TABLE_ROOT + 1_000 + index as u32),
                            ElementKey::new(format!("gallery.table.column.{index}.content")),
                            &title,
                        )
                        .styled(
                            Style::default()
                                .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                                .text(theme.typography.font_size, theme.palette.text)
                                .pointer_events(false),
                        ),
                    ),
                ),
                width: column_widths[index],
                sort_action: Some(DemoAction::SortTable(index as u8)),
                resize: window_controls::table_resize_width_bounds(
                    &demo.table_column_fractions,
                    table_width,
                    index,
                )
                .map(|(min_width, max_width)| TableColumnResize {
                    handle: table_identity(20 + index as u32, &format!("column.{index}.resize")),
                    min_width,
                    max_width,
                    keyboard_step: 8.0,
                }),
            }
        })
        .collect();
    let table_rows = table_data
        .into_iter()
        .filter(|(_, name, _, _)| name.to_lowercase().contains(normalized_query))
        .map(|(id, name, kind, size)| TableRowContent {
            identity: table_identity(100 + u32::from(id) * 10, &format!("row.{id}")),
            cells: [name.to_owned(), kind.to_owned(), size.to_string()]
                .into_iter()
                .enumerate()
                .map(|(column, value)| {
                    let body_identity = |slot, part: &str| {
                        table_identity(
                            2_000 + u32::from(id) * 8 + slot,
                            &format!("row.{id}.body.{part}"),
                        )
                    };
                    let content = if column == 0 {
                        let preferred_note_width = 120.0;
                        let compact_note = column_widths[0] < preferred_note_width;
                        let editor = (id == 1).then(|| {
                            let mut editor = text_field(
                                WidgetIdentity::new(
                                    scope.element(TABLE_NOTE_FIELD),
                                    "gallery.table.row.1.note",
                                ),
                                input.table_note_snapshot.clone(),
                                input.table_note_focused,
                                theme,
                            );
                            editor.semantic =
                                Some(Semantic::new(SemanticRole::TextInput, "Object note"));
                            editor.style.width = if compact_note {
                                Dimension::Percent(1.0)
                            } else {
                                Dimension::Px(preferred_note_width)
                            };
                            editor.style.min_width =
                                Dimension::Px(if compact_note { 0.0 } else { 36.0 });
                            editor.style.flex_shrink = 1.0;
                            Widget::from_element(editor)
                        });
                        match editor {
                            Some(editor) if compact_note => WidgetContent::new(name, editor),
                            editor => collection_object_content(
                                &CollectionObject {
                                    id,
                                    name,
                                    thumbnail: "demo://checkerboard",
                                },
                                body_identity,
                                editor,
                                theme,
                            ),
                        }
                    } else {
                        WidgetContent::new(
                            &value,
                            Widget::from_element(
                                Element::text(
                                    scope.element(
                                        TABLE_ROOT + 3_000 + u32::from(id) * 4 + column as u32,
                                    ),
                                    ElementKey::new(format!(
                                        "gallery.table.row.{id}.value.{column}"
                                    )),
                                    &value,
                                )
                                .styled(
                                    Style::default()
                                        .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                                        .text(theme.typography.font_size, theme.palette.text)
                                        .pointer_events(false),
                                ),
                            ),
                        )
                    };
                    TableCellContent {
                        identity: table_identity(
                            101 + u32::from(id) * 10 + column as u32,
                            &format!("row.{id}.cell.{column}"),
                        ),
                        content,
                        selected: demo.table_selected_cell == Some((id, column as u8)),
                        select_action: Some(DemoAction::SelectTableCell(id, column as u8)),
                    }
                })
                .collect(),
            selected: demo.table_selected == id,
            select_action: DemoAction::SelectTable(id),
        })
        .collect();
    let mut table = table_view_with_content(
        table_identity(0, "root"),
        table_identity(1, "header"),
        table_columns,
        table_rows,
        theme,
    )
    .expect("static gallery table identities");
    table.element.style.width = Dimension::Px(table_width);
    table.element.style.min_width = Dimension::Px(table_width);
    widget_element(table, actions)
}

pub(super) struct CollectionObject<'a, K = u8> {
    pub id: K,
    pub name: &'a str,
    pub thumbnail: &'static str,
}

pub(super) const COLLECTION_OBJECTS: [CollectionObject<'static>; 3] = [
    CollectionObject {
        id: 0,
        name: "Cube",
        thumbnail: "demo://checkerboard",
    },
    CollectionObject {
        id: 1,
        name: "Sphere",
        thumbnail: "demo://checkerboard",
    },
    CollectionObject {
        id: 2,
        name: "Terrain",
        thumbnail: "demo://checkerboard",
    },
];

pub(super) fn collection_object_content<A, K>(
    object: &CollectionObject<'_, K>,
    mut identity: impl FnMut(u32, &str) -> WidgetIdentity,
    trailing: Option<Widget<A>>,
    theme: WidgetTheme,
) -> WidgetContent<A> {
    let root = identity(1, "content");
    let image = identity(2, "image");
    let name = identity(3, "name");
    let mut content = Widget::from_element(
        Element::container(root.element, root.key).styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                .axis(Axis::Row)
                .align(Align::Center)
                .gap(6.0)
                .pointer_events(false),
        ),
    )
    .child(Widget::from_element(
        Element::image(image.element, image.key, object.thumbnail).styled(
            Style::default()
                .size(Dimension::Px(22.0), Dimension::Auto)
                .aspect_ratio(1.0)
                .pointer_events(false),
        ),
    ))
    .child(Widget::from_element(
        Element::text(name.element, name.key, object.name).styled(
            Style::default()
                .size(Dimension::Auto, Dimension::Percent(1.0))
                .min_width(Dimension::Px(0.0))
                .flex(1.0, 1.0)
                .text(theme.typography.font_size, theme.palette.text)
                .pointer_events(false),
        ),
    ));
    if let Some(trailing) = trailing {
        content = content.child(trailing);
    }
    WidgetContent::new(object.name, content)
}

pub(super) fn virtual_object_content(
    scope: ElementScope,
    object: &CollectionObject<'_, usize>,
    tree: bool,
    theme: WidgetTheme,
) -> WidgetContent<DemoAction> {
    let (base, label_base, kind) = if tree {
        (21_000_000, TREE_LABEL_BASE, "tree")
    } else {
        (20_000_000, VIRTUAL_LABEL_BASE, "virtual")
    };
    let identity = |slot: u32, part: &str| {
        let element = if slot == 3 {
            label_base + object.id as u32
        } else {
            base + object.id as u32 * 8 + slot
        };
        WidgetIdentity::new(
            scope.element(element),
            format!("gallery.{kind}.{}.{}", object.id, part),
        )
    };
    collection_object_content(
        object,
        identity,
        Some(button(
            identity(4, "commands"),
            identity(5, "commands.label"),
            "Commands",
            DemoAction::OpenCommandPalette,
            ControlState::Idle,
            theme,
        )),
        theme,
    )
}

pub(super) fn virtual_object_list(
    scope: ElementScope,
    content: WidgetIdentity,
    demo: &DemoState,
    focused: Option<usize>,
    theme: WidgetTheme,
    scrollbar: ScrollbarStyle,
) -> Widget<DemoAction> {
    let ids = VirtualListIds {
        root: WidgetIdentity::new(scope.element(VIRTUAL_SCROLL), "gallery.virtual.scroll"),
        content,
    };
    let row = |item_id: &usize| {
        let name = format!("Object {item_id:05}");
        let object = CollectionObject {
            id: *item_id,
            name: &name,
            thumbnail: "demo://checkerboard",
        };
        list_item_from(
            WidgetIdentity::new(
                scope.element(VIRTUAL_ROW_BASE + *item_id as u32),
                format!("gallery.virtual.row.{item_id}"),
            ),
            &object,
            demo.virtual_selection.contains(item_id),
            DemoAction::SelectVirtual(*item_id),
            ControlState::Idle,
            theme,
            |item| virtual_object_content(scope, item, false, theme),
        )
    };
    let mut widget = if let Some(metrics) = &demo.virtual_metrics {
        let range = metrics
            .range(VIRTUAL_VIEWPORT_HEIGHT, demo.virtual_scroll.offset_y(), 2)
            .expect("bounded measured list geometry");
        vesta_gpui_widgets::variable_list_with_items(
            ids,
            Dimension::Px(320.0),
            &demo.virtual_ids,
            range,
            focused,
            row,
        )
    } else {
        let range = VirtualListRange::compute(
            demo.virtual_ids.len(),
            VIRTUAL_ROW_HEIGHT,
            VIRTUAL_VIEWPORT_HEIGHT,
            demo.virtual_scroll.offset_y(),
            2,
        )
        .expect("bounded list geometry");
        virtual_list_with_items(
            ids,
            Dimension::Px(320.0),
            &demo.virtual_ids,
            range,
            focused,
            row,
        )
    }
    .expect("matching typed list data");
    widget.element.semantic = Some(Semantic::new(SemanticRole::List, "Objects"));
    widget.element.style.max_width = Dimension::Percent(1.0);
    widget.element.style.scrollbar = scrollbar;
    if let ElementKind::ScrollBox(offset) = &mut widget.element.kind {
        *offset = demo.virtual_scroll.offset();
    }
    widget
}

pub(super) fn virtual_data_controls(
    scope: ElementScope,
    demo: &DemoState,
    theme: WidgetTheme,
) -> Widget<DemoAction> {
    let identity = |slot, part: &str| {
        WidgetIdentity::new(
            scope.element(25_000_000 + slot),
            format!("gallery.virtual.data.{part}"),
        )
    };
    let root = identity(0, "root");
    let mut widget = Widget::from_element(
        Element::container(root.element, root.key).styled(
            Style::default()
                .axis(Axis::Row)
                .flex_wrap(vesta_gpui::FlexWrap::Wrap)
                .align(Align::Start)
                .align_content(Align::Start)
                .max_width(Dimension::Percent(1.0))
                .gap(4.0),
        ),
    );
    for (index, label, action) in [
        (0u32, "Reverse", CollectionDataAction::Reverse),
        (1, "Remove selected", CollectionDataAction::RemoveSelected),
        (2, "Restore", CollectionDataAction::Restore),
        (
            5,
            "Toggle row heights",
            CollectionDataAction::ToggleRowHeights,
        ),
    ] {
        let enabled = action != CollectionDataAction::RemoveSelected
            || demo.virtual_selection.selected_ids().next().is_some();
        widget = widget.child(button(
            identity(1 + index * 2, label),
            identity(2 + index * 2, &format!("{label}.label")),
            label,
            DemoAction::CollectionData(action),
            if enabled {
                ControlState::Idle
            } else {
                ControlState::Disabled
            },
            theme,
        ));
    }
    let count = identity(8, "count");
    widget.child(Widget::from_element(
        Element::text(
            count.element,
            count.key,
            format!(
                "{} items{}",
                demo.virtual_ids.len(),
                if demo.virtual_metrics.is_some() {
                    " · variable heights"
                } else {
                    ""
                }
            ),
        )
        .styled(
            Style::default()
                .align_self(Align::End)
                .margin(Insets {
                    left: 6.0,
                    bottom: 4.0,
                    ..Default::default()
                })
                .text(theme.typography.font_size, theme.palette.text),
        ),
    ))
}
