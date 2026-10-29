//! Collection navigation and bounded focused-row projection for each window (ADR-0265).

use super::*;

pub(super) fn table_viewport_height(geometry: &ScrollGeometry, gallery: ElementScope) -> f32 {
    geometry
        .nodes
        .get(&gallery.element(TABLE_VIRTUAL_ROOT + 2))
        .map(|node| node.scroll_viewport().height)
        .filter(|height| height.is_finite() && *height > 0.0)
        .unwrap_or(TABLE_VIRTUAL_VIEWPORT_HEIGHT)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CollectionDataAction {
    Reverse,
    RemoveSelected,
    Restore,
    ToggleRowHeights,
}

impl DemoState {
    fn measured_virtual_rows(&self) -> vesta_gpui_widgets::VariableListMetrics {
        let heights = self
            .virtual_ids
            .iter()
            .map(|id| {
                if id % 3 == 0 {
                    VIRTUAL_ROW_HEIGHT * 2.0
                } else {
                    VIRTUAL_ROW_HEIGHT
                }
            })
            .collect::<Vec<_>>();
        vesta_gpui_widgets::VariableListMetrics::new(&heights).expect("bounded object row heights")
    }

    pub(super) fn virtual_content_height(&self) -> f32 {
        self.virtual_metrics.as_ref().map_or(
            self.virtual_ids.len() as f32 * VIRTUAL_ROW_HEIGHT,
            vesta_gpui_widgets::VariableListMetrics::content_height,
        )
    }

    fn virtual_page_rows(&self) -> Option<usize> {
        self.virtual_metrics.as_ref().map_or_else(
            || {
                fixed_page_rows(
                    self.virtual_ids.len(),
                    VIRTUAL_ROW_HEIGHT,
                    VIRTUAL_VIEWPORT_HEIGHT,
                    self.virtual_scroll.offset_y(),
                )
            },
            |metrics| {
                metrics
                    .range(VIRTUAL_VIEWPORT_HEIGHT, self.virtual_scroll.offset_y(), 0)
                    .ok()
                    .map(|range| range.len())
            },
        )
    }

    fn reveal_virtual_row(&mut self, index: usize) {
        let bounds = self.virtual_metrics.as_ref().map_or_else(
            || {
                (index < self.virtual_ids.len())
                    .then_some((index as f32 * VIRTUAL_ROW_HEIGHT, VIRTUAL_ROW_HEIGHT))
            },
            |metrics| metrics.row_bounds(index),
        );
        if let Some((top, height)) = bounds {
            self.virtual_scroll.reveal(
                Rect::new(0.0, top - self.virtual_scroll.offset_y(), 1.0, height),
                Rect::new(0.0, 0.0, 1.0, VIRTUAL_VIEWPORT_HEIGHT),
                0.0,
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CollectionFocus {
    List { id: usize, index: usize },
    Table(usize),
    Tile(usize),
    Tree { id: usize, index: usize },
}

fn focused_collection_action(
    actions: &ActionRoute<DemoAction>,
    focus: Option<ElementId>,
    root: Option<ElementId>,
) -> Option<DemoAction> {
    let focus = focus?;
    if !actions.focus_order().contains(&focus)
        || root.is_some_and(|root| !actions.is_descendant_of(focus, root))
    {
        return None;
    }
    actions.action_at(focus).copied()
}

pub(super) fn collection_focus(
    demo: &DemoState,
    actions: &ActionRoute<DemoAction>,
    focus: Option<ElementId>,
    root: Option<ElementId>,
) -> Option<CollectionFocus> {
    collection_for_action(demo, focused_collection_action(actions, focus, root)?)
}

/// Retain the owning row when an arbitrary child control has focus. Keyboard dispatch
/// still uses the exact focused control so the child keeps its own input semantics.
pub(super) fn collection_item_focus(
    demo: &DemoState,
    actions: &ActionRoute<DemoAction>,
    window: UiWindowId,
    focus: Option<ElementId>,
    root: Option<ElementId>,
) -> Option<CollectionFocus> {
    FocusScope::new(window, actions, root).ancestor_action(
        InputTarget {
            window,
            element: focus?,
        },
        |action| collection_for_action(demo, *action),
    )
}

fn collection_for_action(demo: &DemoState, action: DemoAction) -> Option<CollectionFocus> {
    match action {
        DemoAction::SelectVirtual(id) => demo
            .virtual_ids
            .iter()
            .position(|candidate| *candidate == id)
            .map(|index| CollectionFocus::List { id, index }),
        DemoAction::SelectVirtualTable(index) if index < TABLE_VIRTUAL_COUNT => {
            Some(CollectionFocus::Table(index))
        }
        DemoAction::SelectTile(index) if index < 9 => Some(CollectionFocus::Tile(index)),
        DemoAction::SelectVirtualTree(id) | DemoAction::ToggleVirtualTree(id) => {
            VisibleTree::build(&demo.tree_entries, &demo.tree_expansion)
                .ok()?
                .index_of(&id)
                .map(|index| CollectionFocus::Tree { id, index })
        }
        _ => None,
    }
}

/// A staged stable row must exist in the next bounded frame before focus admission.
pub(super) fn collection_projection_focus(
    demo: &DemoState,
    actions: &ActionRoute<DemoAction>,
    input: &InputOwnership,
    window: UiWindowId,
    gallery: ElementScope,
    root: Option<ElementId>,
) -> Option<CollectionFocus> {
    if root.is_none()
        && let Some(target) = input.pending_focus_target(window)
    {
        if let Some(index) = demo
            .virtual_ids
            .iter()
            .position(|id| gallery.element(VIRTUAL_ROW_BASE + *id as u32) == target.element)
        {
            return Some(CollectionFocus::List {
                id: demo.virtual_ids[index],
                index,
            });
        }
        let tree = VisibleTree::build(&demo.tree_entries, &demo.tree_expansion).ok()?;
        if let Some((id, index)) = demo.tree_entries.iter().find_map(|entry| {
            (gallery.element(TREE_ROW_BASE + entry.id as u32) == target.element)
                .then(|| tree.index_of(&entry.id).map(|index| (entry.id, index)))
                .flatten()
        }) {
            return Some(CollectionFocus::Tree { id, index });
        }
    }
    collection_item_focus(
        demo,
        actions,
        window,
        input
            .focus()
            .filter(|target| target.window == window)
            .map(|target| target.element),
        root,
    )
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum CollectionKeyUpdate {
    Move(CollectionFocus),
    ToggleTree(usize),
}

pub(super) fn fixed_page_rows(
    count: usize,
    row_height: f32,
    viewport_height: f32,
    offset: f32,
) -> Option<usize> {
    VirtualListRange::compute(count, row_height, viewport_height, offset, 0)
        .ok()
        .map(|range| range.len())
}

fn linear_next_index(current: usize, count: usize, page_rows: usize, key: &Key) -> Option<usize> {
    let chord = shortcut_chord(key.as_ref(), winit::keyboard::ModifiersState::empty())?;
    linear_collection_index(current, count, page_rows, chord.key)
}

pub(super) fn collection_key_update(
    demo: &DemoState,
    focus: CollectionFocus,
    tile_columns: usize,
    table_viewport_height: f32,
    key: &Key,
) -> Option<CollectionKeyUpdate> {
    let next = match focus {
        CollectionFocus::List { index, .. } => {
            let index = linear_next_index(
                index,
                demo.virtual_ids.len(),
                demo.virtual_page_rows()?,
                key,
            )?;
            CollectionFocus::List {
                id: demo.virtual_ids[index],
                index,
            }
        }
        CollectionFocus::Table(index) => CollectionFocus::Table(linear_next_index(
            index,
            TABLE_VIRTUAL_COUNT,
            fixed_page_rows(
                TABLE_VIRTUAL_COUNT,
                TABLE_VIRTUAL_ROW_HEIGHT,
                table_viewport_height,
                demo.virtual_table_scroll.offset_y(),
            )?,
            key,
        )?),
        CollectionFocus::Tile(index) => {
            let direction = match key {
                Key::Named(NamedKey::ArrowLeft) => TileGridDirection::Left,
                Key::Named(NamedKey::ArrowRight) => TileGridDirection::Right,
                Key::Named(NamedKey::ArrowUp) => TileGridDirection::Up,
                Key::Named(NamedKey::ArrowDown) => TileGridDirection::Down,
                Key::Named(NamedKey::Home) => TileGridDirection::Home,
                Key::Named(NamedKey::End) => TileGridDirection::End,
                _ => return None,
            };
            CollectionFocus::Tile(tile_grid_next_index(index, 9, tile_columns, direction)?)
        }
        CollectionFocus::Tree { id, .. } => {
            let visible = VisibleTree::build(&demo.tree_entries, &demo.tree_expansion).ok()?;
            let chord = shortcut_chord(key.as_ref(), winit::keyboard::ModifiersState::empty())?;
            let next = match visible.key_action(
                &demo.tree_expansion,
                &id,
                chord.key,
                fixed_page_rows(
                    visible.len(),
                    VIRTUAL_ROW_HEIGHT,
                    TREE_VIEWPORT_HEIGHT,
                    demo.tree_scroll.offset_y(),
                )?,
            )? {
                TreeKeyAction::SetExpanded { id, .. } => {
                    return Some(CollectionKeyUpdate::ToggleTree(id));
                }
                TreeKeyAction::Focus(id) => visible.index_of(&id)?,
            };
            CollectionFocus::Tree {
                id: visible.entry(next)?.id,
                index: next,
            }
        }
    };
    Some(CollectionKeyUpdate::Move(next))
}

impl App {
    pub(super) fn apply_collection_data(&mut self, action: CollectionDataAction) {
        let previous = self.demo.virtual_ids.clone();
        let focused = std::iter::once(STANDALONE_WINDOW)
            .chain(self.native_windows.values().map(|host| host.ui_window))
            .filter_map(|window| match self.collection_item_focus_for(window) {
                Some(CollectionFocus::List { id, .. }) => Some((window, id)),
                _ => None,
            })
            .collect::<Vec<_>>();
        match action {
            CollectionDataAction::Reverse => self.demo.virtual_ids.reverse(),
            CollectionDataAction::RemoveSelected => {
                self.demo
                    .virtual_ids
                    .retain(|id| !self.demo.virtual_selection.contains(id));
            }
            CollectionDataAction::Restore => {
                self.demo.virtual_ids = (0..VIRTUAL_ITEM_COUNT).collect()
            }
            CollectionDataAction::ToggleRowHeights => {
                self.demo.virtual_metrics = if self.demo.virtual_metrics.is_some() {
                    None
                } else {
                    Some(self.demo.measured_virtual_rows())
                };
            }
        }
        if self.demo.virtual_metrics.is_some() && action != CollectionDataAction::ToggleRowHeights {
            self.demo.virtual_metrics = Some(self.demo.measured_virtual_rows());
        }
        self.demo
            .virtual_selection
            .reconcile(&self.demo.virtual_ids);
        self.demo
            .virtual_scroll
            .sync_content_height(self.demo.virtual_content_height(), VIRTUAL_VIEWPORT_HEIGHT);
        for (window, id) in focused {
            if let Some(next) = vesta_gpui_widgets::collection_focus_after_change(
                &previous,
                &self.demo.virtual_ids,
                &id,
                None,
            ) && next != id
                && let Some(scopes) = self.window_scopes(window)
            {
                self.restore_after_rebuild(
                    window,
                    Some(scopes.gallery.element(VIRTUAL_ROW_BASE + next as u32)),
                );
            }
        }
        self.request_all_redraw();
    }

    fn collection_focus_for(&self, window: UiWindowId) -> Option<CollectionFocus> {
        collection_focus(
            &self.demo,
            self.window_actions(window)?,
            self.window_focus(window),
            self.focus_root_for_window(window),
        )
    }

    pub(super) fn focused_virtual_index(&self) -> Option<usize> {
        match self.retained_collection_focus()? {
            CollectionFocus::List { index, .. } => Some(index),
            _ => None,
        }
    }

    pub(super) fn focused_virtual_table_index(&self) -> Option<usize> {
        match self.retained_collection_focus()? {
            CollectionFocus::Table(index) => Some(index),
            _ => None,
        }
    }

    pub(super) fn focused_tree_index(&self) -> Option<usize> {
        match self.retained_collection_focus()? {
            CollectionFocus::Tree { index, .. } => Some(index),
            _ => None,
        }
    }

    fn retained_collection_focus(&self) -> Option<CollectionFocus> {
        collection_projection_focus(
            &self.demo,
            &self.ui_actions,
            &self.input_ownership,
            STANDALONE_WINDOW,
            self.scopes.gallery,
            self.focus_root_for_window(STANDALONE_WINDOW),
        )
    }

    fn collection_item_focus_for(&self, window: UiWindowId) -> Option<CollectionFocus> {
        collection_item_focus(
            &self.demo,
            self.window_actions(window)?,
            window,
            self.window_focus(window),
            self.focus_root_for_window(window),
        )
    }

    pub(super) fn handle_collection_key_for(&mut self, window: UiWindowId, key: &Key) -> bool {
        let menu_open = if window == STANDALONE_WINDOW {
            self.demo.menu.is_open()
        } else {
            self.native_windows
                .values()
                .any(|host| host.ui_window == window && host.pane_menu.is_some())
        };
        if menu_open || self.text_session_field(window).is_some() || self.owns_overlay(window) {
            return false;
        }
        let Some(focus) = self.collection_focus_for(window) else {
            return false;
        };
        let Some(scopes) = self.window_scopes(window) else {
            return false;
        };
        if let CollectionFocus::Tree { id, .. } = focus {
            return self.handle_tree_key(window, id, key);
        }
        if key == &Key::Named(NamedKey::Space)
            && self.window_modifiers(window) == winit::keyboard::ModifiersState::CONTROL
        {
            let action = self
                .window_actions(window)
                .and_then(|actions| actions.action_at(self.window_focus(window)?))
                .copied();
            let Some(action @ (DemoAction::SelectVirtual(_) | DemoAction::SelectVirtualTree(_))) =
                action
            else {
                return false;
            };
            self.action_window = window;
            self.apply_demo_action(action);
            return true;
        }
        let columns = if window == STANDALONE_WINDOW {
            self.tile_columns
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .map_or(1, |host| host.tile_columns)
        };
        let table_geometry = if window == STANDALONE_WINDOW {
            Some(&self.scroll_geometry)
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .map(|host| &host.scroll_geometry)
        };
        let table_height = table_geometry.map_or(TABLE_VIRTUAL_VIEWPORT_HEIGHT, |geometry| {
            table_viewport_height(geometry, scopes.gallery)
        });
        if matches!(focus, CollectionFocus::Table(_))
            && let Some(node) = table_geometry.and_then(|geometry| {
                geometry
                    .nodes
                    .get(&scopes.gallery.element(TABLE_VIRTUAL_ROOT + 2))
            })
        {
            // Shared offset bounds may have last been reconciled by another window.
            self.demo.virtual_table_scroll.sync_layout(node);
        }
        let Some(update) = collection_key_update(&self.demo, focus, columns, table_height, key)
        else {
            return false;
        };
        self.action_window = window;
        let modifiers = self.window_modifiers(window);
        let selection_modifiers = SelectionModifiers {
            additive: modifiers.control_key(),
            range: modifiers.shift_key(),
        };
        let select = !modifiers.control_key() || modifiers.shift_key();
        let (element, viewport) = match update {
            CollectionKeyUpdate::Move(CollectionFocus::List { id, index }) => {
                if select {
                    self.demo.virtual_selection.select(
                        id,
                        &self.demo.virtual_ids,
                        selection_modifiers,
                    );
                }
                self.demo.reveal_virtual_row(index);
                (
                    scopes.gallery.element(VIRTUAL_ROW_BASE + id as u32),
                    Some(scopes.gallery.element(VIRTUAL_SCROLL)),
                )
            }
            CollectionKeyUpdate::Move(CollectionFocus::Table(index)) => {
                self.apply_demo_action(DemoAction::SelectVirtualTable(index));
                vesta_gpui_widgets::reveal_collection_row(
                    &mut self.demo.virtual_table_scroll,
                    index,
                    TABLE_VIRTUAL_ROW_HEIGHT,
                    table_height,
                );
                (
                    scopes
                        .gallery
                        .element(TABLE_VIRTUAL_ROOT + 1_000 + index as u32 * 4),
                    Some(scopes.gallery.element(TABLE_VIRTUAL_ROOT + 2)),
                )
            }
            CollectionKeyUpdate::Move(CollectionFocus::Tile(index)) => {
                self.apply_demo_action(DemoAction::SelectTile(index));
                (
                    scopes.gallery.element(TILE_ROOT + 100 + index as u32 * 2),
                    None,
                )
            }
            CollectionKeyUpdate::Move(CollectionFocus::Tree { id, index }) => {
                if select {
                    self.apply_demo_action(DemoAction::SelectVirtualTree(id));
                }
                vesta_gpui_widgets::reveal_collection_row(
                    &mut self.demo.tree_scroll,
                    index,
                    VIRTUAL_ROW_HEIGHT,
                    TREE_VIEWPORT_HEIGHT,
                );
                (
                    scopes.gallery.element(TREE_ROW_BASE + id as u32),
                    Some(scopes.gallery.element(TREE_SCROLL)),
                )
            }
            CollectionKeyUpdate::ToggleTree(id) => {
                self.apply_demo_action(DemoAction::ToggleVirtualTree(id));
                return true;
            }
        };
        self.set_window_focus(window, Some(element));
        // A newly focused virtual row can be absent from the previous bounded frame.
        // Its viewport is already live, so reveal that viewport's outer ancestors now.
        self.reveal_window_focus(window, viewport.unwrap_or(element));
        true
    }
}
