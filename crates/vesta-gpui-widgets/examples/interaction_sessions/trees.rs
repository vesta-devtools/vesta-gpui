//! Independent controlled hierarchy consumer, no Gallery or native window.
use super::*;
use vesta_gpui::SemanticRole;
use vesta_gpui_widgets::{
    ListSelectionState, SelectionModifiers, TreeExpansionState, TreeViewController, TreeViewEntry,
    TreeViewError, TreeViewRowActions, TreeViewRowIds, TreeViewRowState, VirtualListIds,
    VirtualListRange, VisibleTree, reveal_collection_row, tree_focus_after_change, tree_view_row,
    tree_view_windowed,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TreeAction {
    Select(u32),
    Expand(u32),
}

fn entries() -> Vec<TreeViewEntry<u32>> {
    let mut rows = Vec::new();
    for root in 0..5 {
        let id = root * 2021;
        rows.push(TreeViewEntry {
            id,
            label: format!("Root {root}"),
            depth: 0,
            has_children: true,
        });
        for group in 0..20 {
            let id = id + 1 + group * 101;
            rows.push(TreeViewEntry {
                id,
                label: format!("Group {group}"),
                depth: 1,
                has_children: true,
            });
            for leaf in 1..=100 {
                rows.push(TreeViewEntry {
                    id: id + leaf,
                    label: format!("叶😀 {leaf}"),
                    depth: 2,
                    has_children: false,
                });
            }
        }
    }
    rows
}

pub(super) fn qualify(context: &mut TextInputContext) -> Result<(), String> {
    let original = entries();
    let last = original.last().ok_or("empty fixture")?.id;
    let mut frames = 0;
    for window in [UiWindowId(201), UiWindowId(209)] {
        let scope = ElementIdAllocator.allocate_scope().map_err(debug_error)?;
        for theme in [WidgetTheme::light(), WidgetTheme::dark()] {
            for width in [160.0, 320.0, 780.0] {
                let mut rows = original.clone();
                let mut expansion = TreeExpansionState::default();
                let mut selection = ListSelectionState::single(last);
                let mut retained = RetainedLayoutEngine::default();
                let mut scroll = ScrollAreaState::default();
                let mut focus = 0;
                macro_rules! frame {
                    () => {{
                        present(
                            context,
                            scope,
                            &mut retained,
                            &rows,
                            &expansion,
                            &selection,
                            focus,
                            &mut scroll,
                            theme,
                            width,
                            true,
                        )?;
                        frames += 1;
                    }};
                }
                frame!();
                {
                    let mut tree = TreeViewController::new(&rows, &mut expansion, &mut selection)
                        .map_err(debug_error)?;
                    require(tree.set_all_expanded(true), "expand all did not change")?;
                    require(
                        tree.visible().len() == 10105,
                        "large source omitted entries",
                    )?;
                    focus = tree
                        .key(
                            &focus,
                            KeyChord::new(KeyStroke::End, KeyModifiers::default()),
                            7,
                        )
                        .ok_or("End")?
                        .focus;
                }
                frame!();
                let saved = selection.clone();
                {
                    let mut tree = TreeViewController::new(&rows, &mut expansion, &mut selection)
                        .map_err(debug_error)?;
                    focus = tree
                        .key(
                            &focus,
                            KeyChord::new(
                                KeyStroke::Home,
                                KeyModifiers {
                                    control: true,
                                    ..Default::default()
                                },
                            ),
                            7,
                        )
                        .ok_or("CtrlHome")?
                        .focus;
                }
                require(selection == saved, "control navigation changed selection")?;
                frame!();
                {
                    let mut tree = TreeViewController::new(&rows, &mut expansion, &mut selection)
                        .map_err(debug_error)?;
                    tree.key(
                        &focus,
                        KeyChord::new(
                            KeyStroke::Space,
                            KeyModifiers {
                                control: true,
                                ..Default::default()
                            },
                        ),
                        7,
                    )
                    .ok_or("CtrlSpace")?;
                    require(
                        tree.selection().contains(&last) && tree.selection().contains(&0),
                        "toggle lost additive selection",
                    )?;
                    focus = tree
                        .key(
                            &focus,
                            KeyChord::new(
                                KeyStroke::ArrowRight,
                                KeyModifiers {
                                    shift: true,
                                    ..Default::default()
                                },
                            ),
                            7,
                        )
                        .ok_or("shift right")?
                        .focus;
                }
                frame!();
                {
                    let mut tree = TreeViewController::new(&rows, &mut expansion, &mut selection)
                        .map_err(debug_error)?;
                    tree.set_all_expanded(false);
                    require(
                        tree.select(last, SelectionModifiers::default()).is_none(),
                        "hidden input admitted",
                    )?;
                    require(!tree.set_expanded(&last, true), "leaf expansion admitted")?;
                }
                focus = tree_focus_after_change(&rows, &rows, &expansion, &focus)
                    .map_err(debug_error)?
                    .ok_or("collapse repair")?;
                require(
                    focus == 0,
                    "nested collapse missed nearest visible ancestor",
                )?;
                frame!();
                {
                    let mut tree = TreeViewController::new(&rows, &mut expansion, &mut selection)
                        .map_err(debug_error)?;
                    focus = tree.reveal(&last).ok_or("reveal path")?.focus;
                    require(
                        tree.visible().len() == 125,
                        "reveal opened unrelated branches",
                    )?;
                    tree.select(last, SelectionModifiers::default())
                        .ok_or("last select")?;
                }
                frame!();
                {
                    let mut tree = TreeViewController::new(&rows, &mut expansion, &mut selection)
                        .map_err(debug_error)?;
                    focus = tree
                        .key(
                            &focus,
                            KeyChord::new(
                                KeyStroke::ArrowUp,
                                KeyModifiers {
                                    shift: true,
                                    ..Default::default()
                                },
                            ),
                            7,
                        )
                        .ok_or("shift up")?
                        .focus;
                    require(
                        tree.selection().selected_ids().count() == 2,
                        "range did not use visible order",
                    )?;
                    for modifiers in [
                        KeyModifiers {
                            alt: true,
                            ..Default::default()
                        },
                        KeyModifiers {
                            super_key: true,
                            ..Default::default()
                        },
                    ] {
                        require(
                            tree.key(&focus, KeyChord::new(KeyStroke::ArrowDown, modifiers), 7)
                                .is_none(),
                            "modified navigation stolen",
                        )?;
                    }
                }
                frame!();
                let before = rows.clone();
                rows = rows
                    .chunks(2021)
                    .rev()
                    .flat_map(|chunk| chunk.iter().cloned())
                    .collect();
                TreeViewController::new(&rows, &mut expansion, &mut selection)
                    .map_err(debug_error)?;
                require(
                    tree_focus_after_change(&before, &rows, &expansion, &focus)
                        .map_err(debug_error)?
                        == Some(focus),
                    "reorder changed focus identity",
                )?;
                frame!();
                let before = rows.clone();
                rows.retain(|entry| entry.id < 8084);
                TreeViewController::new(&rows, &mut expansion, &mut selection)
                    .map_err(debug_error)?;
                focus = tree_focus_after_change(&before, &rows, &expansion, &focus)
                    .map_err(debug_error)?
                    .ok_or("delete repair")?;
                require(
                    focus < 8084 && selection.selected_ids().next().is_none(),
                    "deleted subtree retained keys",
                )?;
                frame!();
                let saved = (expansion.clone(), selection.clone());
                for malformed in [
                    vec![original[0].clone(), original[0].clone()],
                    vec![original[2].clone()],
                    vec![original[2].clone(), original[2].clone()],
                ] {
                    require(
                        TreeViewController::new(&malformed, &mut expansion, &mut selection)
                            .is_err(),
                        "invalid source accepted",
                    )?;
                    require(
                        saved == (expansion.clone(), selection.clone()),
                        "invalid source mutated borrowed state",
                    )?;
                }
                let mut leaf = original[0].clone();
                leaf.has_children = false;
                require(
                    matches!(
                        TreeViewController::new(
                            &[leaf, original[1].clone()],
                            &mut expansion,
                            &mut selection
                        ),
                        Err(TreeViewError::LeafHasDescendants)
                    ),
                    "leaf hierarchy accepted",
                )?;
                let before = std::mem::take(&mut rows);
                TreeViewController::new(&rows, &mut expansion, &mut selection)
                    .map_err(debug_error)?;
                require(
                    tree_focus_after_change(&before, &rows, &expansion, &focus)
                        .map_err(debug_error)?
                        .is_none(),
                    "empty tree retained focus",
                )?;
                frame!();
                rows = original.clone();
                let mut tree = TreeViewController::new(&rows, &mut expansion, &mut selection)
                    .map_err(debug_error)?;
                focus = tree.reveal(&last).ok_or("restore reveal")?.focus;
                frame!();
                // Offscreen focus remains one stable retained row, in addition to a bounded page.
                scroll.scroll_by(-1_000_000.0);
                present(
                    context,
                    scope,
                    &mut retained,
                    &rows,
                    &expansion,
                    &selection,
                    focus,
                    &mut scroll,
                    theme,
                    width,
                    false,
                )?;
                frames += 1;
            }
        }
        require(window != UiWindowId(0), "source identity")?;
    }
    println!(
        "controlled trees: {frames} retained frames; 10105 entries, two sources/themes, three widths; hierarchy, selection, refresh, bounded factories and focus intents"
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn present(
    context: &mut TextInputContext,
    scope: ElementScope,
    retained: &mut RetainedLayoutEngine,
    rows: &[TreeViewEntry<u32>],
    expansion: &TreeExpansionState<u32>,
    selection: &ListSelectionState<u32>,
    focus: u32,
    scroll: &mut ScrollAreaState,
    theme: WidgetTheme,
    width: f32,
    reveal: bool,
) -> Result<(), String> {
    let tree = VisibleTree::build(rows, expansion).map_err(debug_error)?;
    let height = theme.metrics.control_height;
    scroll.sync_content_height(tree.len() as f32 * height, 196.0);
    if let Some(index) = tree.index_of(&focus).filter(|_| reveal) {
        reveal_collection_row(scroll, index, height, 196.0);
    }
    let range = VirtualListRange::compute(tree.len(), height, 196.0, scroll.offset_y(), 2)
        .map_err(debug_error)?;
    let mut built = Vec::new();
    let id = |part| WidgetIdentity::new(scope.element(part), format!("hierarchy.{part}"));
    let mut widget = tree_view_windowed(
        VirtualListIds {
            root: id(1),
            content: id(2),
        },
        Dimension::Px(width),
        &tree,
        range,
        tree.index_of(&focus),
        |entry| {
            built.push(entry.id);
            let base = 100 + entry.id * 3;
            let mut row = tree_view_row(
                TreeViewRowIds {
                    root: id(base),
                    disclosure: id(base + 1),
                    label: id(base + 2),
                },
                &entry.label,
                TreeViewRowState {
                    depth: entry.depth,
                    has_children: entry.has_children,
                    expanded: expansion.contains(&entry.id),
                    selected: selection.contains(&entry.id),
                    control: ControlState::Idle,
                },
                TreeViewRowActions {
                    select: TreeAction::Select(entry.id),
                    toggle: TreeAction::Expand(entry.id),
                },
                theme,
            );
            row.element.style.width = Dimension::Percent(1.0);
            row
        },
    )
    .map_err(debug_error)?;
    if let ElementKind::ScrollBox(offset) = &mut widget.element.kind {
        *offset = scroll.offset();
    }
    require(built.len() <= 14, "row factory grew with source size")?;
    require(
        tree.index_of(&focus).is_none() || built.contains(&focus),
        "offscreen focused row was not retained",
    )?;
    require(
        built
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == built.len(),
        "retained row duplicated",
    )?;
    let layout = retained
        .compute_with_text_context(&widget.element, Viewport::new(width, 196.0, 1.25), context)
        .map_err(debug_error)?;
    let route = ActionRoute::build(&widget.element, widget.actions).map_err(debug_error)?;
    let hits = HitTestTree::build(&widget.element, &layout);
    require(
        widget
            .element
            .semantic
            .as_ref()
            .is_some_and(|s| s.role == SemanticRole::Tree),
        "tree semantic absent",
    )?;
    for key in built {
        let target = scope.element(100 + key * 3);
        let row = widget.element.find(target).ok_or("factory row absent")?;
        let semantic = row.semantic.as_ref().ok_or("row semantic absent")?;
        require(
            semantic.role == SemanticRole::TreeItem
                && semantic.selected == Some(selection.contains(&key)),
            "row selection semantic mismatch",
        )?;
        require(
            route.action_at(target) == Some(&TreeAction::Select(key)),
            "stable action mismatch",
        )?;
        let node = layout.node(target).ok_or("stable layout absent")?;
        let entry = tree
            .entry(tree.index_of(&key).ok_or("built hidden row")?)
            .ok_or("entry")?;
        let disclosure = scope.element(101 + key * 3);
        require(
            route.action_at(disclosure).is_some() == entry.has_children,
            "disclosure action lifetime mismatch",
        )?;
        require(
            semantic.expanded == entry.has_children.then_some(expansion.contains(&key)),
            "disclosure semantic mismatch",
        )?;
        let point = Point::new(
            node.rect.x + node.rect.width - 4.0,
            node.rect.y + node.rect.height * 0.5,
        );
        if point.y >= 0.0 && point.y < 196.0 {
            let hit = hits.hit(point).ok_or("visible row missed hit")?;
            require(
                route.action_for_hit(hit) == Some(&TreeAction::Select(key)),
                "hit routed to wrong identity",
            )?;
        }
    }
    Ok(())
}
