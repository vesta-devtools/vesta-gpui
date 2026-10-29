//! Adaptive fixed-height tiles with stable identities and bounded projection.

use std::collections::HashSet;

use crate::{Widget, WidgetContent, WidgetIdentity, WidgetTheme};
use vesta_gpui::{
    Dimension, Element, ElementId, FlowDirection, GridCell, GridTrack, Semantic, SemanticRole,
    Style,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileGridGeometry {
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub scroll_y: f32,
    pub minimum_tile_width: f32,
    pub tile_height: f32,
    pub gap: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileGridRange {
    pub columns: usize,
    pub first: usize,
    pub end: usize,
    pub total_height: f32,
    pub top_height: f32,
    pub bottom_height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileGridError {
    InvalidGeometry,
    DuplicateElement(ElementId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileGridDirection {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

#[must_use]
pub fn tile_grid_next_index(
    current: usize,
    count: usize,
    columns: usize,
    direction: TileGridDirection,
) -> Option<usize> {
    tile_grid_next_index_in_direction(
        current,
        count,
        columns,
        direction,
        FlowDirection::LeftToRight,
    )
}

/// Physical horizontal keys follow the current Grid layout direction. Home/End
/// refer to the first/last declared item; vertical movement retains its row meaning.
#[must_use]
pub fn tile_grid_next_index_in_direction(
    current: usize,
    count: usize,
    columns: usize,
    direction: TileGridDirection,
    flow: FlowDirection,
) -> Option<usize> {
    if count == 0 || columns == 0 || current >= count {
        return None;
    }
    let direction = match (flow, direction) {
        (FlowDirection::RightToLeft, TileGridDirection::Left) => TileGridDirection::Right,
        (FlowDirection::RightToLeft, TileGridDirection::Right) => TileGridDirection::Left,
        _ => direction,
    };
    Some(match direction {
        TileGridDirection::Left => current.saturating_sub(1),
        TileGridDirection::Right => current.saturating_add(1).min(count - 1),
        TileGridDirection::Up => {
            if current < columns {
                current
            } else {
                current - columns
            }
        }
        TileGridDirection::Down => {
            if current / columns == (count - 1) / columns {
                current
            } else {
                current.saturating_add(columns).min(count - 1)
            }
        }
        TileGridDirection::Home => 0,
        TileGridDirection::End => count - 1,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TileGridIds {
    pub root: WidgetIdentity,
    pub top: WidgetIdentity,
    pub grid: WidgetIdentity,
    pub bottom: WidgetIdentity,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TileGridItem<A> {
    pub root: WidgetIdentity,
    pub label: WidgetIdentity,
    pub title: String,
    pub selected: bool,
    pub action: A,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TileGridContent<A> {
    pub root: WidgetIdentity,
    pub content: WidgetContent<A>,
    pub selected: bool,
    pub action: A,
}

pub fn tile_grid_range(
    count: usize,
    geometry: TileGridGeometry,
) -> Result<TileGridRange, TileGridError> {
    if ![
        geometry.viewport_width,
        geometry.viewport_height,
        geometry.scroll_y,
        geometry.minimum_tile_width,
        geometry.tile_height,
        geometry.gap,
    ]
    .into_iter()
    .all(f32::is_finite)
        || geometry.viewport_width <= 0.0
        || geometry.viewport_height <= 0.0
        || geometry.minimum_tile_width <= 0.0
        || geometry.tile_height <= 0.0
        || geometry.gap < 0.0
    {
        return Err(TileGridError::InvalidGeometry);
    }
    let available_width = geometry.viewport_width + geometry.gap;
    let minimum_width = geometry.minimum_tile_width + geometry.gap;
    let pitch = geometry.tile_height + geometry.gap;
    if ![available_width, minimum_width, pitch]
        .into_iter()
        .all(f32::is_finite)
    {
        return Err(TileGridError::InvalidGeometry);
    }
    let columns = (available_width / minimum_width)
        .floor()
        .clamp(1.0, f32::from(u16::MAX)) as usize;
    let rows = count.div_ceil(columns);
    let total_height = if rows == 0 {
        0.0
    } else {
        rows as f32 * pitch - geometry.gap
    };
    if !total_height.is_finite() {
        return Err(TileGridError::InvalidGeometry);
    }
    let scroll_y = geometry
        .scroll_y
        .max(0.0)
        .min((total_height - geometry.viewport_height).max(0.0));
    let first_row = ((scroll_y / pitch).floor() as usize).min(rows.saturating_sub(1));
    let end_row = (((scroll_y + geometry.viewport_height) / pitch).ceil() as usize)
        .max(first_row.saturating_add(1))
        .min(rows)
        .min(first_row.saturating_add(usize::from(u16::MAX)));
    let visible_rows = end_row.saturating_sub(first_row);
    let top_height = first_row as f32 * pitch;
    let visible_height = if visible_rows == 0 {
        0.0
    } else {
        visible_rows as f32 * pitch - geometry.gap
    };
    Ok(TileGridRange {
        columns,
        first: first_row.saturating_mul(columns).min(count),
        end: end_row.saturating_mul(columns).min(count),
        total_height,
        top_height,
        bottom_height: (total_height - top_height - visible_height).max(0.0),
    })
}

pub fn tile_grid<A>(
    ids: TileGridIds,
    items: Vec<TileGridItem<A>>,
    geometry: TileGridGeometry,
    theme: WidgetTheme,
) -> Result<(Widget<A>, TileGridRange), TileGridError> {
    let range = tile_grid_range(items.len(), geometry)?;
    let mut identities = HashSet::new();
    for id in [
        ids.root.element,
        ids.top.element,
        ids.grid.element,
        ids.bottom.element,
    ]
    .into_iter()
    .chain(
        items
            .iter()
            .flat_map(|item| [item.root.element, item.label.element]),
    ) {
        if !identities.insert(id) {
            return Err(TileGridError::DuplicateElement(id));
        }
    }
    tile_grid_projected(
        ids,
        items
            .into_iter()
            .enumerate()
            .skip(range.first)
            .take(range.end - range.first)
            .map(|(index, item)| {
                let text = Widget::from_element(
                    Element::text(item.label.element, item.label.key, &item.title).styled(
                        Style::default()
                            .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                            .text(theme.typography.font_size, theme.palette.text)
                            .pointer_events(false),
                    ),
                );
                (
                    index,
                    TileGridContent {
                        root: item.root,
                        content: WidgetContent::new(item.title, text),
                        selected: item.selected,
                        action: item.action,
                    },
                )
            }),
        range,
        geometry,
        theme,
    )
}

pub fn tile_grid_with_content<A>(
    ids: TileGridIds,
    items: Vec<TileGridContent<A>>,
    geometry: TileGridGeometry,
    theme: WidgetTheme,
) -> Result<(Widget<A>, TileGridRange), TileGridError> {
    let range = tile_grid_range(items.len(), geometry)?;
    tile_grid_projected(
        ids,
        items
            .into_iter()
            .enumerate()
            .skip(range.first)
            .take(range.end - range.first),
        range,
        geometry,
        theme,
    )
}

/// Construct content only for visible tiles. Stable identities come from the host's
/// typed objects, rather than from the current visual position or column count.
pub fn tile_grid_windowed_with_content<A>(
    ids: TileGridIds,
    count: usize,
    geometry: TileGridGeometry,
    item_at: impl FnMut(usize) -> TileGridContent<A>,
    theme: WidgetTheme,
) -> Result<(Widget<A>, TileGridRange), TileGridError> {
    tile_grid_windowed_with_retained_content(ids, count, geometry, None, item_at, theme)
}

/// Construct the visible tiles and at most one caller-qualified focused item.
/// A retained item's children keep their stable identities across scrolling and
/// column changes. The host must omit removed, disabled or otherwise stale focus.
pub fn tile_grid_windowed_with_retained_content<A>(
    ids: TileGridIds,
    count: usize,
    geometry: TileGridGeometry,
    retained_index: Option<usize>,
    mut item_at: impl FnMut(usize) -> TileGridContent<A>,
    theme: WidgetTheme,
) -> Result<(Widget<A>, TileGridRange), TileGridError> {
    let range = tile_grid_range(count, geometry)?;
    let retained = retained_index
        .filter(|index| *index < count && !((range.first..range.end).contains(index)));
    tile_grid_projected(
        ids,
        (range.first..range.end)
            .chain(retained)
            .map(|index| (index, item_at(index))),
        range,
        geometry,
        theme,
    )
}

fn tile_grid_projected<A>(
    ids: TileGridIds,
    items: impl Iterator<Item = (usize, TileGridContent<A>)>,
    range: TileGridRange,
    geometry: TileGridGeometry,
    theme: WidgetTheme,
) -> Result<(Widget<A>, TileGridRange), TileGridError> {
    let visible_rows = range
        .end
        .saturating_sub(range.first)
        .div_ceil(range.columns);
    let mut grid = Element::container(ids.grid.element, ids.grid.key).styled(
        Style::default()
            .size(Dimension::Px(geometry.viewport_width), Dimension::Auto)
            .grid(
                vec![GridTrack::Fr(1.0); range.columns],
                vec![GridTrack::Px(geometry.tile_height); visible_rows],
            )
            .gap(geometry.gap),
    );
    let mut actions = Vec::with_capacity(range.end - range.first);
    for (index, item) in items {
        let visible = (range.first..range.end).contains(&index);
        let local_row = if visible {
            (index - range.first) / range.columns
        } else {
            0
        };
        let mut style = Style::default()
            .size(Dimension::Percent(1.0), Dimension::Px(geometry.tile_height))
            .min_width(Dimension::Px(0.0))
            .grid_cell(
                GridCell::new(Some((index % range.columns + 1) as u16), 1),
                GridCell::new(Some((local_row + 1) as u16), 1),
            )
            .background(if item.selected {
                theme.palette.selection
            } else {
                theme.palette.surface
            })
            .border(1.0, theme.palette.border)
            .focusable(true)
            .clipped();
        if !visible {
            // Overlay the first visible track without materializing intervening
            // rows. Relative placement keeps the shared Grid's RTL mirroring.
            style.top = Some(
                (index / range.columns) as f32 * (geometry.tile_height + geometry.gap)
                    - range.top_height,
            );
        }
        let tile = Element::container(item.root.element, item.root.key)
            .semantic(
                Semantic::new(SemanticRole::ListItem, item.content.label).selected(item.selected),
            )
            .styled(style)
            .child(item.content.widget.element);
        actions.push((tile.id, item.action));
        actions.extend(item.content.widget.actions);
        grid = grid.child(tile);
    }
    let root = Element::container(ids.root.element, ids.root.key)
        .semantic(Semantic::new(SemanticRole::List, "Tiles"))
        .styled(Style::default().size(Dimension::Px(geometry.viewport_width), Dimension::Auto))
        .child(
            Element::container(ids.top.element, ids.top.key).styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Px(range.top_height))
                    .pointer_events(false),
            ),
        )
        .child(grid)
        .child(
            Element::container(ids.bottom.element, ids.bottom.key).styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Px(range.bottom_height))
                    .pointer_events(false),
            ),
        );
    let mut identities = HashSet::new();
    let mut pending = vec![&root];
    while let Some(element) = pending.pop() {
        if !identities.insert(element.id) {
            return Err(TileGridError::DuplicateElement(element.id));
        }
        pending.extend(element.children.iter().rev());
    }
    Ok((
        Widget {
            element: root,
            actions,
        },
        range,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(id: u64) -> WidgetIdentity {
        WidgetIdentity::new(ElementId::new(id), format!("tile.{id}"))
    }

    #[test]
    fn tile_range_is_bounded_and_reflow_keeps_actions_on_ids() {
        finite_range_arithmetic();
        let geometry = TileGridGeometry {
            viewport_width: 310.0,
            viewport_height: 120.0,
            scroll_y: 540.0,
            minimum_tile_width: 100.0,
            tile_height: 50.0,
            gap: 5.0,
        };
        let items = (0..1_000)
            .map(|id| TileGridItem {
                root: identity(100 + id * 2),
                label: identity(101 + id * 2),
                title: format!("Tile {id}"),
                selected: id == 28,
                action: id,
            })
            .collect::<Vec<_>>();
        let ids = TileGridIds {
            root: identity(1),
            top: identity(2),
            grid: identity(3),
            bottom: identity(4),
        };
        let (first, range) =
            tile_grid(ids.clone(), items.clone(), geometry, WidgetTheme::dark()).expect("tiles");
        assert_eq!(range.columns, 3);
        assert!(range.end - range.first < 20);
        assert_eq!(first.actions.len(), range.end - range.first);
        let (reflow, narrower) = tile_grid(
            ids,
            items,
            TileGridGeometry {
                viewport_width: 180.0,
                scroll_y: 1_500.0,
                ..geometry
            },
            WidgetTheme::dark(),
        )
        .expect("tiles");
        assert_eq!(narrower.columns, 1);
        let shared = (range.first..range.end)
            .find(|index| (*index >= narrower.first) && (*index < narrower.end))
            .expect("overlap");
        let id = ElementId::new(100 + shared as u64 * 2);
        assert_eq!(first.action_at(id), Some(&(shared as u64)));
        assert_eq!(reflow.action_at(id), Some(&(shared as u64)));
        assert!(
            first
                .element
                .find(id)
                .expect("visible tile")
                .style
                .focusable
        );
        let mut constructed = Vec::new();
        let (custom, custom_range) = tile_grid_windowed_with_content(
            TileGridIds {
                root: identity(1),
                top: identity(2),
                grid: identity(3),
                bottom: identity(4),
            },
            1_000,
            geometry,
            |index| {
                constructed.push(index);
                let base = 10_000 + index as u64 * 4;
                TileGridContent {
                    root: identity(base),
                    content: WidgetContent::new(
                        format!("Object {index}"),
                        Widget::from_element(Element::container(
                            ElementId::new(base + 1),
                            vesta_gpui::ElementKey::new(format!("tile.{base}.content")),
                        ))
                        .child(
                            Widget::from_element(
                                Element::container(
                                    ElementId::new(base + 2),
                                    vesta_gpui::ElementKey::new(format!(
                                        "tile.{base}.child.button"
                                    )),
                                )
                                .styled(Style::default().focusable(true)),
                            )
                            .on_click(2_000 + index),
                        ),
                    ),
                    selected: index == shared,
                    action: index,
                }
            },
            WidgetTheme::dark(),
        )
        .expect("lazy compound tiles");
        assert_eq!(custom_range, range);
        assert_eq!(constructed, (range.first..range.end).collect::<Vec<_>>());
        let route = vesta_gpui::ActionRoute::build(&custom.element, custom.actions)
            .expect("independent tile child actions");
        let shell = ElementId::new(10_000 + shared as u64 * 4);
        assert_eq!(route.action_at(shell), Some(&shared));
        assert_eq!(
            route.action_at(ElementId::new(shell.get() + 2)),
            Some(&(2_000 + shared))
        );
        assert!(
            route
                .focus_order()
                .contains(&ElementId::new(shell.get() + 2))
        );
        for width in [310.0, 180.0] {
            for flow in [FlowDirection::LeftToRight, FlowDirection::RightToLeft] {
                let geometry = TileGridGeometry {
                    viewport_width: width,
                    ..geometry
                };
                let range = tile_grid_range(1_000, geometry).expect("retained geometry");
                for retained in [0, 999, range.first, 1_000] {
                    let mut constructed = Vec::new();
                    let (mut tiles, actual_range) = tile_grid_windowed_with_retained_content(
                        TileGridIds {
                            root: identity(1),
                            top: identity(2),
                            grid: identity(3),
                            bottom: identity(4),
                        },
                        1_000,
                        geometry,
                        Some(retained),
                        |index| {
                            constructed.push(index);
                            let base = 10_000 + index as u64 * 4;
                            TileGridContent {
                                root: identity(base),
                                content: WidgetContent::new(
                                    "Retained child",
                                    Widget::from_element(
                                        Element::container(
                                            identity(base + 1).element,
                                            identity(base + 1).key,
                                        )
                                        .styled(
                                            Style::default().size(
                                                Dimension::Percent(1.0),
                                                Dimension::Percent(1.0),
                                            ),
                                        ),
                                    )
                                    .child(
                                        Widget::from_element(
                                            Element::container(
                                                identity(base + 2).element,
                                                identity(base + 2).key,
                                            )
                                            .styled(
                                                Style::default()
                                                    .size(Dimension::Px(20.0), Dimension::Px(20.0))
                                                    .focusable(true),
                                            ),
                                        )
                                        .on_click(2_000 + index),
                                    )
                                    .child(
                                        Widget::from_element(
                                            Element::container(
                                                identity(base + 3).element,
                                                identity(base + 3).key,
                                            )
                                            .styled(
                                                Style::default()
                                                    .size(Dimension::Px(900.0), Dimension::Px(1.0))
                                                    .pointer_events(false),
                                            ),
                                        ),
                                    ),
                                ),
                                selected: index == retained,
                                action: index,
                            }
                        },
                        WidgetTheme::dark(),
                    )
                    .expect("retained typed tiles");
                    assert_eq!(actual_range, range);
                    let extra = retained < 1_000 && !(range.first..range.end).contains(&retained);
                    assert_eq!(
                        constructed.len(),
                        range.end - range.first + usize::from(extra)
                    );
                    assert_eq!(
                        constructed.iter().filter(|id| **id == retained).count(),
                        usize::from(retained < 1_000)
                    );
                    tiles.element.style.flow_direction = Some(flow);
                    let mut viewport = Element::scroll_box(
                        ElementId::new(9),
                        vesta_gpui::ElementKey::new("tile.viewport"),
                        vesta_gpui::Point::new(0.0, geometry.scroll_y),
                    )
                    .styled(Style::default().size(
                        Dimension::Px(width),
                        Dimension::Px(geometry.viewport_height),
                    ))
                    .child(tiles.element);
                    viewport.style.flow_direction = Some(flow);
                    let layout = vesta_gpui::LayoutEngine
                        .compute(&viewport, vesta_gpui::Viewport::new(width, 120.0, 1.0))
                        .expect("retained layout");
                    let content = layout.node(ElementId::new(1)).expect("tile content").rect;
                    assert_eq!(
                        content.height, range.total_height,
                        "retention cannot allocate hidden tracks or alter extent"
                    );
                    assert_eq!(
                        layout
                            .node(ElementId::new(9))
                            .expect("tile viewport")
                            .scroll_content
                            .expect("tile extent")
                            .1,
                        range.total_height
                    );
                    let route = vesta_gpui::ActionRoute::build(&viewport, tiles.actions)
                        .expect("retained route");
                    for index in &constructed {
                        let id = ElementId::new(10_000 + *index as u64 * 4);
                        let row = layout.node(id).expect("tile rect").rect;
                        let top =
                            (*index / range.columns) as f32 * (geometry.tile_height + geometry.gap);
                        assert!(
                            (row.y - content.y - top).abs() < 1.0,
                            "retained tile must occupy its real row"
                        );
                        let tile_width = (width - (range.columns - 1) as f32 * geometry.gap)
                            / range.columns as f32;
                        let column = if flow == FlowDirection::RightToLeft {
                            range.columns - 1 - *index % range.columns
                        } else {
                            *index % range.columns
                        };
                        assert!(
                            (row.x - content.x - column as f32 * (tile_width + geometry.gap)).abs()
                                < 1.0,
                            "retained tile must follow actual column/RTL reflow"
                        );
                        assert_eq!(
                            route.action_at(ElementId::new(id.get() + 2)),
                            Some(&(2_000 + index))
                        );
                    }
                    if extra {
                        let child = ElementId::new(10_002 + retained as u64 * 4);
                        assert!(route.focus_order().contains(&child));
                        let node = layout.node(child).expect("retained child geometry");
                        let center = vesta_gpui::Point::new(node.rect.x + 5.0, node.rect.y + 5.0);
                        assert!(!node.inherited_clip.contains(center));
                        assert_ne!(
                            vesta_gpui::HitTestTree::build(&viewport, &layout).hit(center),
                            Some(child)
                        );
                    }
                }
            }
        }
        let duplicate = tile_grid_with_content(
            TileGridIds {
                root: identity(1),
                top: identity(2),
                grid: identity(3),
                bottom: identity(4),
            },
            vec![TileGridContent {
                root: identity(10),
                content: WidgetContent::new(
                    "Duplicate",
                    Widget::from_element(Element::image(
                        ElementId::new(1),
                        vesta_gpui::ElementKey::new("duplicate"),
                        "image",
                    )),
                ),
                selected: false,
                action: 0,
            }],
            TileGridGeometry {
                scroll_y: 0.0,
                ..geometry
            },
            WidgetTheme::dark(),
        );
        assert_eq!(
            duplicate,
            Err(TileGridError::DuplicateElement(ElementId::new(1)))
        );
    }

    fn finite_range_arithmetic() {
        let geometry = TileGridGeometry {
            viewport_width: 310.0,
            viewport_height: 120.0,
            scroll_y: 0.0,
            minimum_tile_width: 100.0,
            tile_height: 50.0,
            gap: 5.0,
        };
        let invalid = [
            TileGridGeometry {
                viewport_width: f32::MAX,
                minimum_tile_width: f32::MAX,
                gap: f32::MAX,
                ..geometry
            },
            TileGridGeometry {
                tile_height: f32::MAX,
                gap: f32::MAX,
                ..geometry
            },
        ];
        let mut outcomes = Vec::new();
        for invalid in invalid {
            for count in [0, 1] {
                let actual = std::panic::catch_unwind(|| tile_grid_range(count, invalid));
                outcomes.push(matches!(actual, Ok(Err(TileGridError::InvalidGeometry))));
            }
        }
        for extreme in [
            TileGridGeometry {
                scroll_y: f32::MAX,
                ..geometry
            },
            TileGridGeometry {
                viewport_width: 1.0,
                viewport_height: 1.0,
                minimum_tile_width: 1.0,
                tile_height: 1.0,
                gap: 0.0,
                scroll_y: f32::MAX,
            },
        ] {
            for count in [0, 1, 1_000, usize::MAX / 2, usize::MAX] {
                let actual = std::panic::catch_unwind(|| tile_grid_range(count, extreme));
                outcomes.push(match actual {
                    Ok(Ok(range)) => {
                        range.columns > 0
                            && range.first <= range.end
                            && range.end <= count
                            && (count == 0 || range.first < range.end)
                            && [range.total_height, range.top_height, range.bottom_height]
                                .into_iter()
                                .all(|height| height.is_finite() && height >= 0.0)
                    }
                    _ => false,
                });
            }
        }
        assert!(
            outcomes.iter().all(|passed| *passed),
            "finite Tile geometry must reject overflow or yield ordered finite ranges: {outcomes:?}"
        );
        for invalid in invalid {
            let mut constructed = 0;
            let result = tile_grid_windowed_with_retained_content::<()>(
                TileGridIds {
                    root: identity(1),
                    top: identity(2),
                    grid: identity(3),
                    bottom: identity(4),
                },
                10,
                invalid,
                Some(9),
                |_| {
                    constructed += 1;
                    panic!("invalid geometry must not construct content");
                },
                WidgetTheme::dark(),
            );
            assert_eq!(result, Err(TileGridError::InvalidGeometry));
            assert_eq!(constructed, 0);
        }
    }
}
