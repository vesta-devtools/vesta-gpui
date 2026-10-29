//! Portable typed-content example. Run with a host-provided TTF/OTF file.
//! Projects UI data without opening an OS window or using a renderer.

#[path = "composite_collections/windowed.rs"]
mod windowed;

use std::sync::Arc;
use vesta_gpui::{
    AccessibilityTree, ActionRoute, Align, Axis, Dimension, Element, ElementId, ElementIdAllocator,
    ElementScope, FlowDirection, FocusScope, HitTestTree, InputOwnership, InputTarget,
    PaintCommand, PaintList, Point, Rect, RetainedLayoutEngine, Semantic, SemanticRole, Style,
    TextEditKind, TextInputContext, TextInputState, UiWindowId, Viewport,
};
use vesta_gpui_widgets::{
    ControlState, ListSelectionState, SelectionModifiers, TableCellContent, TableColumnContent,
    TableRowContent, TileGridContent, TileGridGeometry, TileGridIds, TreeRowContentIds,
    TreeViewRowActions, TreeViewRowState, Widget, WidgetContent, WidgetIdentity, WidgetTheme,
    button, combo_option_with_content, list_item_from, table_view_with_content, text_field,
    tile_grid_with_content, tree_view_row_from,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RecordId(u16);

struct Record {
    id: RecordId,
    name: String,
    note: TextInputState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Select(RecordId),
    Open(RecordId),
    Toggle(RecordId),
    Sort,
}

#[derive(Clone, Copy, Debug)]
enum View {
    List,
    Tree,
    ComboOptions,
    Table,
    Tiles,
}

// Caller-owned identities derive from the stable typed ID, never its row position.
fn identity(scope: ElementScope, record: RecordId, part: u32) -> WidgetIdentity {
    WidgetIdentity::new(
        scope.element(100 + u32::from(record.0) * 16 + part),
        format!("record.{}.part.{part}", record.0),
    )
}

fn shell(scope: ElementScope, part: u32) -> WidgetIdentity {
    WidgetIdentity::new(scope.element(part), format!("collection.part.{part}"))
}

// The same complete content subtree can be put in any collection shell.
fn content(scope: ElementScope, record: &Record, theme: WidgetTheme) -> WidgetContent<Action> {
    let root = identity(scope, record.id, 1);
    let image = identity(scope, record.id, 2);
    let name = identity(scope, record.id, 3);
    let editor = identity(scope, record.id, 6);
    let mut field = text_field(editor, record.note.snapshot(), false, theme).semantic(
        Semantic::new(SemanticRole::TextInput, format!("Note for {}", record.name)),
    );
    field.style.width = Dimension::Px(120.0);
    let widget = Widget::from_element(
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
        (if record.id.0 % 2 == 0 {
            Element::fitted_image(
                image.element,
                image.key,
                "example://thumbnail",
                vesta_gpui::ImagePresentation::new(vesta_gpui::ImageFit::Cover),
            )
        } else {
            Element::image(image.element, image.key, "example://thumbnail")
        })
        .styled(
            Style::default()
                .size(Dimension::Px(22.0), Dimension::Auto)
                .aspect_ratio(1.0)
                .pointer_events(false),
        ),
    ))
    .child(Widget::from_element(
        Element::text(name.element, name.key, &record.name).styled(
            Style::default()
                .flex(1.0, 1.0)
                .min_width(Dimension::Px(0.0))
                .text(theme.typography.font_size, theme.palette.text)
                .pointer_events(false),
        ),
    ))
    .child(Widget::from_element(field))
    .child(button(
        identity(scope, record.id, 4),
        identity(scope, record.id, 5),
        "Open",
        Action::Open(record.id),
        ControlState::Idle,
        theme,
    ));
    WidgetContent::new(&record.name, widget)
}

fn project(
    scope: ElementScope,
    view: View,
    records: &[Record],
    selection: &ListSelectionState<RecordId>,
    width: f32,
    theme: WidgetTheme,
) -> Result<Widget<Action>, String> {
    let selected = |record: &Record| selection.contains(&record.id);
    if matches!(view, View::Table) {
        let header = shell(scope, 4);
        return table_view_with_content(
            shell(scope, 1),
            shell(scope, 2),
            vec![TableColumnContent {
                header: shell(scope, 3),
                content: WidgetContent::new(
                    "Records",
                    // Arbitrary header content owns its appearance, just like a row's children.
                    Widget::from_element(
                        Element::text(header.element, header.key, "Records").styled(
                            Style::default().text(theme.typography.font_size, theme.palette.text),
                        ),
                    ),
                ),
                width,
                sort_action: Some(Action::Sort),
                resize: None,
            }],
            records
                .iter()
                .map(|record| TableRowContent {
                    identity: identity(scope, record.id, 0),
                    cells: vec![TableCellContent {
                        identity: identity(scope, record.id, 8),
                        content: content(scope, record, theme),
                        selected: selected(record),
                        select_action: None,
                    }],
                    selected: selected(record),
                    select_action: Action::Select(record.id),
                })
                .collect(),
            theme,
        )
        .map_err(|error| format!("table: {error:?}"));
    }
    if matches!(view, View::Tiles) {
        return tile_grid_with_content(
            TileGridIds {
                root: shell(scope, 1),
                top: shell(scope, 2),
                grid: shell(scope, 3),
                bottom: shell(scope, 4),
            },
            records
                .iter()
                .map(|record| TileGridContent {
                    root: identity(scope, record.id, 0),
                    content: content(scope, record, theme),
                    selected: selected(record),
                    action: Action::Select(record.id),
                })
                .collect(),
            TileGridGeometry {
                viewport_width: width,
                viewport_height: 300.0,
                scroll_y: 0.0,
                minimum_tile_width: 360.0,
                tile_height: 80.0,
                gap: 6.0,
            },
            theme,
        )
        .map(|(widget, _)| widget)
        .map_err(|error| format!("tiles: {error:?}"));
    }
    let root = shell(scope, 1);
    let role = match view {
        View::List | View::Tiles => SemanticRole::List,
        View::Tree => SemanticRole::Tree,
        View::ComboOptions => SemanticRole::ListBox,
        View::Table => SemanticRole::Table,
    };
    let mut widget = Widget::from_element(
        Element::container(root.element, root.key)
            .semantic(Semantic::new(role, "Records"))
            .styled(Style::default().size(Dimension::Px(width), Dimension::Auto)),
    );
    for record in records {
        let row = match view {
            View::List => list_item_from(
                identity(scope, record.id, 0),
                record,
                selected(record),
                Action::Select(record.id),
                ControlState::Idle,
                theme,
                |item| content(scope, item, theme),
            ),
            View::Tree => tree_view_row_from(
                TreeRowContentIds {
                    root: identity(scope, record.id, 0),
                    disclosure: identity(scope, record.id, 7),
                },
                record,
                TreeViewRowState {
                    depth: 0,
                    has_children: false,
                    expanded: false,
                    selected: selected(record),
                    control: ControlState::Idle,
                },
                TreeViewRowActions {
                    select: Action::Select(record.id),
                    toggle: Action::Toggle(record.id),
                },
                theme,
                |item| content(scope, item, theme),
            ),
            View::ComboOptions => combo_option_with_content(
                identity(scope, record.id, 0),
                content(scope, record, theme),
                selected(record),
                Action::Select(record.id),
                ControlState::Idle,
                theme,
            ),
            View::Table | View::Tiles => return Err("unreachable collection view".into()),
        };
        widget = widget.child(row);
    }
    Ok(widget)
}

struct Projection {
    scope: ElementScope,
    window: UiWindowId,
    view: View,
    layout: RetainedLayoutEngine,
    semantics: AccessibilityTree,
    input: InputOwnership,
}

#[derive(PartialEq)]
struct FrameAppearance {
    geometry: Vec<(ElementId, Rect)>,
    actions: Vec<(ElementId, Action)>,
    focus_order: Vec<ElementId>,
    focus: Option<InputTarget>,
    paint: PaintList,
}

impl Projection {
    fn frame(
        &mut self,
        records: &[Record],
        selection: &ListSelectionState<RecordId>,
        context: &mut TextInputContext,
        theme: WidgetTheme,
    ) -> Result<FrameAppearance, String> {
        let width = if self.window == UiWindowId(1) {
            640.0
        } else {
            480.0
        };
        let mut widget = project(self.scope, self.view, records, selection, width, theme)?;
        widget.element.style.flow_direction = Some(if self.window == UiWindowId(1) {
            FlowDirection::LeftToRight
        } else {
            FlowDirection::RightToLeft
        });
        let layout = self
            .layout
            .compute_with_text_context(&widget.element, Viewport::new(width, 360.0, 1.0), context)
            .map_err(|error| error.to_string())?;
        let actions = widget.actions.clone();
        let route = ActionRoute::build(&widget.element, widget.actions)
            .map_err(|error| format!("route: {error:?}"))?;
        let focus = FocusScope::new(self.window, &route, None);
        focus.reconcile(&mut self.input);
        let hits = HitTestTree::build(&widget.element, &layout);
        if matches!(self.view, View::Tiles) {
            let grid = self.scope.element(3);
            let flow = layout
                .node(grid)
                .ok_or("grid geometry missing")?
                .flow_direction;
            let columns = widget
                .element
                .find(grid)
                .and_then(|element| element.style.grid_columns.as_ref())
                .map(Vec::len)
                .ok_or("grid columns missing")?;
            let next = vesta_gpui_widgets::tile_grid_next_index_in_direction(
                1,
                records.len(),
                columns,
                vesta_gpui_widgets::TileGridDirection::Right,
                flow,
            )
            .ok_or("tile navigation missing")?;
            require(
                next == if flow == FlowDirection::RightToLeft {
                    0
                } else {
                    2.min(records.len() - 1)
                },
                "tile keyboard direction does not match this source layout",
            )?;
        }
        let update = self.semantics.build(
            &widget.element,
            &layout,
            self.input.focus().map(|target| target.element),
            "Typed collections",
        );
        for record in records {
            let row = identity(self.scope, record.id, 0).element;
            let semantic_id = self
                .semantics
                .node_for_element(row)
                .ok_or("row semantics missing")?;
            let published = update
                .nodes
                .iter()
                .find(|(id, _)| *id == semantic_id)
                .and_then(|(_, node)| node.is_selected());
            require(
                published == Some(selection.contains(&record.id)),
                "collection shell lost stable multi-selection semantics",
            )?;
            for part in [4, 6] {
                let element = identity(self.scope, record.id, part).element;
                let node = layout.node(element).ok_or("missing composite child")?;
                let hit = hits
                    .hit(Point::new(
                        node.rect.x + node.rect.width / 2.0,
                        node.rect.y + node.rect.height / 2.0,
                    ))
                    .ok_or("child is not hittable")?;
                let expected = (part == 4).then_some(Action::Open(record.id));
                require(
                    route.action_for_hit(hit).copied() == expected,
                    "nested control incorrectly triggered row selection",
                )?;
                require(
                    focus.accepts(InputTarget {
                        window: self.window,
                        element,
                    }),
                    "live child is not focusable",
                )?;
                require(
                    !focus.accepts(InputTarget {
                        window: if self.window == UiWindowId(1) {
                            UiWindowId(2)
                        } else {
                            UiWindowId(1)
                        },
                        element,
                    }),
                    "foreign source accepted",
                )?;
                require(
                    self.semantics.node_for_element(element).is_some(),
                    "composite child missing from semantic tree",
                )?;
            }
        }
        let paint = PaintList::build(&widget.element, &layout);
        if let Some(target) = self.input.visible_focus() {
            let focused = PaintList::build_with_focus(
                &widget.element,
                &layout,
                Some(target.element),
                theme.palette.focus,
            );
            require(
                focused.commands().iter().any(|command| {
                    matches!(
                        command, PaintCommand::StrokeRoundedRect { element, color, .. }
                            if *element == target.element && *color == theme.palette.focus
                    )
                }),
                "deliberate child focus lost its current theme feedback",
            )?;
        }
        println!(
            "window={} {:?}: records={}, paint={}, semantics={}, focus={:?}",
            self.window.0,
            self.view,
            records.len(),
            paint.commands().len(),
            update.nodes.len(),
            self.input.focus()
        );
        Ok(FrameAppearance {
            geometry: layout
                .nodes()
                .iter()
                .map(|node| (node.id, node.rect))
                .collect(),
            actions,
            focus_order: route.focus_order().to_vec(),
            focus: self.input.focus(),
            paint,
        })
    }

    fn theme_round_trip(
        &mut self,
        records: &[Record],
        selection: &ListSelectionState<RecordId>,
        context: &mut TextInputContext,
        theme: WidgetTheme,
        appearance: &FrameAppearance,
    ) -> Result<(), String> {
        let light = self.frame(records, selection, context, theme.toggled())?;
        require(
            (
                &appearance.geometry,
                &appearance.actions,
                &appearance.focus_order,
                appearance.focus,
            ) == (
                &light.geometry,
                &light.actions,
                &light.focus_order,
                light.focus,
            ),
            "theme replacement changed stable geometry, child actions or focus ownership",
        )?;
        require(
            appearance.paint != light.paint,
            "theme replacement left the composed paint stale",
        )?;
        let images = |frame: &FrameAppearance| {
            frame
                .paint
                .commands()
                .iter()
                .filter(|command| {
                    matches!(
                        command,
                        PaintCommand::Image { .. } | PaintCommand::FittedImage { .. }
                    )
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        require(
            images(appearance) == images(&light),
            "theme replacement rewrote caller image content",
        )?;
        let restored = self.frame(records, selection, context, theme.toggled().toggled())?;
        require(
            &restored == appearance,
            "theme round-trip did not restore the original live projection",
        )?;
        Ok(())
    }
}

fn require(condition: bool, message: &str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| message.to_owned())
}

fn main() -> Result<(), String> {
    let font = std::env::args_os().nth(1).ok_or(
        "usage: cargo run -p vesta-gpui-widgets --example composite_collections -- <font.ttf>",
    )?;
    let mut context = TextInputContext::default();
    require(
        context.register_font(Arc::new(
            std::fs::read(font).map_err(|error| format!("read host font: {error}"))?,
        )),
        "invalid host font",
    )?;
    let mut records = Vec::new();
    for (id, name) in [(11, "River"), (23, "Forest"), (42, "Mountain")] {
        let mut note = TextInputState::new(13.0, "sans-serif");
        note.reset(&mut context, "Ready")
            .map_err(|error| format!("note: {error:?}"))?;
        records.push(Record {
            id: RecordId(id),
            name: name.into(),
            note,
        });
    }
    if std::env::args_os().nth(2).as_deref() == Some(std::ffi::OsStr::new("--profile-lazy")) {
        return windowed::profile(&mut context, &records[0]);
    }
    let selected = RecordId(23);
    let mut selection = ListSelectionState::single(selected);
    let mut projections = Vec::new();
    for window in [UiWindowId(1), UiWindowId(2)] {
        for view in [
            View::List,
            View::Tree,
            View::ComboOptions,
            View::Table,
            View::Tiles,
        ] {
            let scope = ElementIdAllocator
                .allocate_scope()
                .map_err(|error| format!("scope: {error:?}"))?;
            let mut input = InputOwnership::default();
            input.set_focus(Some(InputTarget {
                window,
                element: identity(scope, selected, 4).element,
            }));
            projections.push(Projection {
                scope,
                window,
                view,
                layout: Default::default(),
                semantics: Default::default(),
                input,
            });
        }
    }
    for phase in 0..3 {
        if phase == 1 {
            records.reverse();
            let record = records
                .iter_mut()
                .find(|record| record.id == selected)
                .ok_or("missing record")?;
            record.name = "Renamed forest".into();
            record
                .note
                .insert(&mut context, " / edited", TextEditKind::Typing)
                .map_err(|error| format!("edit: {error:?}"))?;
            require(
                record.note.text() == "Ready / edited",
                "editor update missing",
            )?;
            let order = records.iter().map(|record| record.id).collect::<Vec<_>>();
            selection.select(
                RecordId(42),
                &order,
                SelectionModifiers {
                    range: true,
                    additive: false,
                },
            );
            for _ in 0..2 {
                selection.select(
                    RecordId(11),
                    &order,
                    SelectionModifiers {
                        range: false,
                        additive: true,
                    },
                );
            }
            require(
                selection.selected_ids().copied().collect::<Vec<_>>() == [selected, RecordId(42)],
                "range/additive selection changed stable identity",
            )?;
        } else if phase == 2 {
            records.retain(|record| record.id != selected);
        }
        selection.reconcile(&records.iter().map(|record| record.id).collect::<Vec<_>>());
        for projection in &mut projections {
            let appearance =
                projection.frame(&records, &selection, &mut context, WidgetTheme::dark())?;
            if phase == 0 {
                projection.theme_round_trip(
                    &records,
                    &selection,
                    &mut context,
                    WidgetTheme::dark(),
                    &appearance,
                )?;
                let mut configured = WidgetTheme::dark().with_density(1.25);
                configured.typography.font_size *= 1.25;
                configured.typography.small_font_size *= 1.25;
                configured.typography.heading_font_size *= 1.25;
                let configured_frame =
                    projection.frame(&records, &selection, &mut context, configured)?;
                projection
                    .theme_round_trip(
                        &records,
                        &selection,
                        &mut context,
                        configured,
                        &configured_frame,
                    )
                    .map_err(|error| format!("configured density/typography: {error}"))?;
            }
            let target = InputTarget {
                window: projection.window,
                element: identity(projection.scope, selected, 4).element,
            };
            require(
                projection.input.focus() == (phase != 2).then_some(target),
                "focus identity changed during reorder or survived deletion",
            )?;
            require(
                projection
                    .semantics
                    .node_for_element(target.element)
                    .is_some()
                    == (phase != 2),
                "deleted child retained live semantics",
            )?;
        }
    }
    require(
        selection.selected_ids().copied().collect::<Vec<_>>() == [RecordId(42)],
        "deletion retained stale selection or cleared a surviving selected record",
    )?;
    println!(
        "30 projections passed: typed composition, child routing, multi-selection, reorder, edit, deletion, source isolation."
    );
    println!(
        "Theme round-trips passed: same typed List/Tree/Combo/Table/Tile sources retain geometry, child actions, selection, focus and images; current ordinary paint and focus tokens rebuild."
    );
    windowed::run(&mut context, &records[0])?;
    Ok(())
}
