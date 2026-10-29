//! The same caller-owned content projected through the public lazy factories.

mod profile;
mod tile_availability;

use super::*;
use std::collections::BTreeMap;
use vesta_gpui_widgets::{
    TableWindowIds, TreeExpansionState, TreeViewEntry, VariableListMetrics, VirtualListIds,
    VirtualListRange, VisibleTree, table_view_windowed_scrolled_with_content,
    tile_grid_windowed_with_retained_content, tree_view_windowed, variable_list_with_items,
    virtual_list_with_items,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    List,
    VariableList,
    Tree,
    Table,
    Tiles,
}

struct LazyProjection {
    scope: ElementScope,
    window: UiWindowId,
    kind: Kind,
    layout: RetainedLayoutEngine,
    semantics: AccessibilityTree,
    input: InputOwnership,
    profile: Option<profile::Frame>,
}

impl LazyProjection {
    fn frame(
        &mut self,
        records: &[Record],
        selection: &ListSelectionState<RecordId>,
        focused: RecordId,
        collapsed: bool,
        context: &mut TextInputContext,
    ) -> Result<(), String> {
        let mut timing = profile::Trace::new(self.profile.is_some());
        timing.start();
        let scope = self.scope;
        let theme = WidgetTheme::default();
        let width = self.profile.as_ref().map_or_else(
            || {
                if self.window == UiWindowId(1) {
                    640.0
                } else {
                    480.0
                }
            },
            |frame| frame.width,
        );
        let retained = records.iter().position(|record| record.id == focused);
        let range = VirtualListRange::compute(records.len(), 40.0, 160.0, 200_000.0, 2)
            .map_err(|error| format!("lazy range: {error:?}"))?;
        let mut built = Vec::new();
        let ids = VirtualListIds {
            root: shell(scope, 1),
            content: shell(scope, 2),
        };
        let selected = |record: &Record| selection.contains(&record.id);
        let mut expected = (range.first..range.end)
            .map(|i| records[i].id)
            .collect::<Vec<_>>();
        if let Some(index) = retained.filter(|i| *i < range.first || *i >= range.end) {
            expected.push(records[index].id);
        }
        let mut widget = match self.kind {
            Kind::List | Kind::VariableList => {
                let heights = records
                    .iter()
                    .map(|record| measured_row_height(record, collapsed))
                    .collect::<Vec<_>>();
                let metrics = VariableListMetrics::new(&heights)
                    .map_err(|error| format!("measured list geometry: {error:?}"))?;
                let variable_range = metrics
                    .range(160.0, 200_000.0, 2)
                    .map_err(|error| format!("measured list range: {error:?}"))?;
                if self.kind == Kind::VariableList {
                    let visible = variable_range.visible_rows();
                    expected = records[visible.clone()]
                        .iter()
                        .map(|record| record.id)
                        .collect();
                    if let Some(index) = retained.filter(|i| !visible.contains(i)) {
                        expected.push(records[index].id);
                    }
                }
                let row = |record: &Record| {
                    built.push(record.id);
                    list_item_from(
                        identity(scope, record.id, 0),
                        record,
                        selected(record),
                        Action::Select(record.id),
                        ControlState::Idle,
                        theme,
                        |item| content(scope, item, theme),
                    )
                };
                if self.kind == Kind::VariableList {
                    variable_list_with_items(
                        ids,
                        Dimension::Px(width),
                        records,
                        variable_range,
                        retained,
                        row,
                    )
                } else {
                    virtual_list_with_items(
                        ids,
                        Dimension::Px(width),
                        records,
                        range,
                        retained,
                        row,
                    )
                }
                .map_err(|error| format!("lazy list: {error:?}"))?
            }
            Kind::Tree => {
                let entries = records
                    .iter()
                    .enumerate()
                    .map(|(index, record)| TreeViewEntry {
                        id: record.id,
                        label: record.name.clone(),
                        depth: u8::from(index != 0),
                        has_children: index == 0,
                    })
                    .collect::<Vec<_>>();
                let by_id = records
                    .iter()
                    .map(|record| (record.id, record))
                    .collect::<BTreeMap<_, _>>();
                let mut expansion = TreeExpansionState::default();
                if !collapsed {
                    expansion.toggle(records[0].id);
                }
                let tree = VisibleTree::build(&entries, &expansion)
                    .map_err(|error| format!("lazy tree model: {error:?}"))?;
                let tree_range = VirtualListRange::compute(tree.len(), 40.0, 160.0, 200_000.0, 2)
                    .map_err(|error| format!("lazy tree range: {error:?}"))?;
                let tree_retained = tree.index_of(&focused);
                expected = (tree_range.first..tree_range.end)
                    .filter_map(|i| tree.entry(i).map(|entry| entry.id))
                    .collect();
                if let Some(index) =
                    tree_retained.filter(|i| *i < tree_range.first || *i >= tree_range.end)
                {
                    expected.push(tree.entry(index).ok_or("retained tree entry missing")?.id);
                }
                tree_view_windowed(
                    ids,
                    Dimension::Px(width),
                    &tree,
                    tree_range,
                    tree_retained,
                    |entry| {
                        built.push(entry.id);
                        let record = by_id[&entry.id];
                        tree_view_row_from(
                            TreeRowContentIds {
                                root: identity(scope, record.id, 0),
                                disclosure: identity(scope, record.id, 7),
                            },
                            record,
                            TreeViewRowState {
                                depth: entry.depth,
                                has_children: entry.has_children,
                                expanded: expansion.contains(&entry.id),
                                selected: selected(record),
                                control: ControlState::Idle,
                            },
                            TreeViewRowActions {
                                select: Action::Select(record.id),
                                toggle: Action::Toggle(record.id),
                            },
                            theme,
                            |item| content(scope, item, theme),
                        )
                    },
                )
                .map_err(|error| format!("lazy tree: {error:?}"))?
            }
            Kind::Table => {
                let header = shell(scope, 5);
                table_view_windowed_scrolled_with_content(
                    TableWindowIds {
                        root: shell(scope, 1),
                        header: shell(scope, 2),
                        viewport: shell(scope, 3),
                        content: shell(scope, 4),
                    },
                    vec![TableColumnContent {
                        header: shell(scope, 6),
                        content: WidgetContent::new(
                            "Records",
                            Widget::from_element(Element::text(
                                header.element,
                                header.key,
                                "Records",
                            )),
                        ),
                        width: width + 120.0,
                        sort_action: Some(Action::Sort),
                        resize: None,
                    }],
                    range,
                    if self.window == UiWindowId(1) {
                        36.0
                    } else {
                        72.0
                    },
                    retained,
                    |index| {
                        let record = &records[index];
                        built.push(record.id);
                        TableRowContent {
                            identity: identity(scope, record.id, 0),
                            cells: vec![TableCellContent {
                                identity: identity(scope, record.id, 8),
                                content: content(scope, record, theme),
                                selected: selected(record),
                                select_action: None,
                            }],
                            selected: selected(record),
                            select_action: Action::Select(record.id),
                        }
                    },
                    theme,
                )
                .map_err(|error| format!("lazy table: {error:?}"))?
            }
            Kind::Tiles => {
                let (widget, tile_range) = tile_grid_windowed_with_retained_content(
                    TileGridIds {
                        root: shell(scope, 1),
                        top: shell(scope, 2),
                        grid: shell(scope, 3),
                        bottom: shell(scope, 4),
                    },
                    records.len(),
                    TileGridGeometry {
                        viewport_width: width,
                        viewport_height: 160.0,
                        scroll_y: 200_000.0,
                        minimum_tile_width: if collapsed { 360.0 } else { 240.0 },
                        tile_height: 80.0,
                        gap: 6.0,
                    },
                    retained,
                    |index| {
                        let record = &records[index];
                        built.push(record.id);
                        TileGridContent {
                            root: identity(scope, record.id, 0),
                            content: content(scope, record, theme),
                            selected: selected(record),
                            action: Action::Select(record.id),
                        }
                    },
                    theme,
                )
                .map_err(|error| format!("lazy tiles: {error:?}"))?;
                expected = (tile_range.first..tile_range.end)
                    .map(|i| records[i].id)
                    .collect();
                if let Some(index) =
                    retained.filter(|i| !((tile_range.first..tile_range.end).contains(i)))
                {
                    expected.push(records[index].id);
                }
                // TileGrid is a lazy content projection; its host owns the
                // ScrollBox and must use the same offset as the range request.
                let viewport = shell(scope, 7);
                Widget::from_element(
                    Element::scroll_box(viewport.element, viewport.key, Point::new(0.0, 200_000.0))
                        .styled(Style::default().size(Dimension::Px(width), Dimension::Px(160.0))),
                )
                .child(widget)
            }
        };
        timing.finish(0);
        require(
            built == expected,
            "lazy factory constructed hidden, duplicate or wrong stable records",
        )?;
        require(
            built.len() <= 16,
            "lazy factory work grew with the 10,000-record data set",
        )?;
        if collapsed && self.kind == Kind::Tree {
            require(
                built == [records[0].id],
                "collapsed descendants reached the factory",
            )?;
        }
        widget.element.style.flow_direction = Some(if self.window == UiWindowId(1) {
            FlowDirection::LeftToRight
        } else {
            FlowDirection::RightToLeft
        });
        timing.start();
        let layout = self
            .layout
            .compute_with_text_context(&widget.element, Viewport::new(width, 220.0, 1.0), context)
            .map_err(|error| format!("lazy layout: {error}"))?;
        timing.finish(2);
        timing.start();
        let route = ActionRoute::build(&widget.element, widget.actions)
            .map_err(|error| format!("lazy route: {error:?}"))?;
        timing.finish(1);
        if self.kind == Kind::Table {
            let header = layout
                .node(scope.element(6))
                .ok_or("scrolled header missing")?;
            let first = built.first().ok_or("scrolled rows missing")?;
            let cell = layout
                .node(identity(scope, *first, 8).element)
                .ok_or("scrolled cell missing")?;
            require(
                (header.rect.x - cell.rect.x).abs() < 0.01,
                "scrolled compound header/cell positions disagree",
            )?;
        }
        let focus = FocusScope::new(self.window, &route, None);
        focus.reconcile(&mut self.input);
        let target = InputTarget {
            window: self.window,
            element: identity(scope, focused, 4).element,
        };
        let retained_focus = retained.is_some() && !(collapsed && self.kind == Kind::Tree);
        require(
            self.input.focus() == retained_focus.then_some(target),
            "lazy focus lost stable identity or survived removal/collapse",
        )?;
        timing.start();
        let update = self.semantics.build(
            &widget.element,
            &layout,
            self.input.focus().map(|t| t.element),
            "Lazy typed collections",
        );
        timing.finish(5);
        require(
            self.semantics.node_for_element(target.element).is_some() == retained_focus,
            "lazy semantics lost retained focus or kept a removed child",
        )?;
        let paint = if self.profile.is_some() {
            timing.start();
            let paint = PaintList::build(&widget.element, &layout);
            timing.finish(3);
            Some(paint)
        } else {
            None
        };
        timing.start();
        let hits = HitTestTree::build(&widget.element, &layout);
        timing.finish(4);
        let mut checked_hits = 0;
        for record in &built {
            let row = identity(scope, *record, 0).element;
            if self.kind == Kind::VariableList {
                let source = records
                    .iter()
                    .find(|item| item.id == *record)
                    .ok_or("measured source missing")?;
                require(
                    layout
                        .node(row)
                        .ok_or("measured row layout missing")?
                        .rect
                        .height
                        == measured_row_height(source, collapsed),
                    "measured row height changed in projection",
                )?;
            }
            let node_id = self.semantics.node_for_element(row);
            if let Some(node_id) = node_id {
                require(
                    update
                        .nodes
                        .iter()
                        .find(|(id, _)| *id == node_id)
                        .and_then(|(_, n)| n.is_selected())
                        == Some(selection.contains(record)),
                    "lazy row selection differs from caller state",
                )?;
            }
            for part in [4, 6] {
                let element = identity(scope, *record, part).element;
                let node = layout
                    .node(element)
                    .ok_or("lazy composite child geometry missing")?;
                let point = Point::new(
                    node.rect.x + node.rect.width / 2.0,
                    node.rect.y + node.rect.height / 2.0,
                );
                if !node.inherited_clip.contains(point) || point.y >= 220.0 {
                    continue;
                }
                let hit = hits.hit(point).ok_or("visible lazy child not hittable")?;
                require(
                    route.action_for_hit(hit).copied()
                        == (part == 4).then_some(Action::Open(*record)),
                    "lazy child incorrectly activated row selection",
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
                    "lazy foreign-window focus accepted",
                )?;
                checked_hits += 1;
            }
        }
        require(
            checked_hits >= 2,
            "lazy projection did not verify visible composite child hits",
        )?;
        if self.profile.is_some() {
            self.profile = Some(profile::Frame {
                width,
                stages_ns: timing.stages_ns,
                rows: built.len(),
                nodes: layout.nodes().len(),
                paint: paint.as_ref().map_or(0, |list| list.commands().len()),
                hits: hits.len(),
                semantics: update.nodes.len(),
            });
        } else {
            println!(
                "lazy window={} {:?}: data={}, built={}, hits={}, retained={retained_focus}",
                self.window.0,
                self.kind,
                records.len(),
                built.len(),
                checked_hits
            );
        }
        Ok(())
    }
}

fn measured_row_height(record: &Record, resized: bool) -> f32 {
    if record.id.0 % 3 == 0 {
        if resized { 80.0 } else { 64.0 }
    } else {
        40.0
    }
}

pub(super) fn run(context: &mut TextInputContext, prototype: &Record) -> Result<(), String> {
    let mut records = (102_u16..10_102)
        .map(|id| Record {
            id: RecordId(id),
            name: format!("Record {id}"),
            note: prototype.note.clone(),
        })
        .collect::<Vec<_>>();
    let focused = records[1].id;
    let mut selection = ListSelectionState::single(focused);
    let mut projections = Vec::new();
    for window in [UiWindowId(1), UiWindowId(2)] {
        for kind in [
            Kind::List,
            Kind::VariableList,
            Kind::Tree,
            Kind::Table,
            Kind::Tiles,
        ] {
            let scope = ElementIdAllocator
                .allocate_scope()
                .map_err(|error| format!("lazy scope: {error:?}"))?;
            let mut input = InputOwnership::default();
            input.set_focus(Some(InputTarget {
                window,
                element: identity(scope, focused, 4).element,
            }));
            projections.push(LazyProjection {
                scope,
                window,
                kind,
                layout: Default::default(),
                semantics: Default::default(),
                input,
                profile: None,
            });
        }
    }
    for phase in 0..4 {
        if phase == 1 {
            records[1..].reverse();
        }
        if phase == 3 {
            records.retain(|record| record.id != focused);
        }
        selection.reconcile(&records.iter().map(|record| record.id).collect::<Vec<_>>());
        for projection in &mut projections {
            projection.frame(&records, &selection, focused, phase == 2, context)?;
            if phase == 2 && projection.kind == Kind::Tiles {
                tile_availability::qualify(projection, &records, &selection, focused, context)?;
            }
        }
    }
    println!(
        "40 lazy projections passed: 10,000 typed records, fixed/variable heights, bounded factories, retained focus, reorder, measured resize, collapse, deletion, child routing and source isolation."
    );
    Ok(())
}

pub(super) fn profile(context: &mut TextInputContext, prototype: &Record) -> Result<(), String> {
    profile::run(context, prototype)
}
