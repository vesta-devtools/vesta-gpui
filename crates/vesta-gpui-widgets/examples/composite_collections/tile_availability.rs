//! Disabled callers omit stale retention and reconcile through the shared route.
use super::*;
use vesta_gpui_widgets::{WidgetAvailability, tile_grid_range};

pub(super) fn qualify(
    projection: &LazyProjection,
    records: &[Record],
    selection: &ListSelectionState<RecordId>,
    focused: RecordId,
    context: &mut TextInputContext,
) -> Result<(), String> {
    let scope = projection.scope;
    let window = projection.window;
    let target = InputTarget {
        window,
        element: identity(scope, focused, 4).element,
    };
    // Keep the original projection's deletion precondition and input untouched.
    let mut input = projection.input.clone();
    require(
        input.focus() == Some(target),
        "tile availability precondition",
    )?;
    let width = if window == UiWindowId(1) {
        640.0
    } else {
        480.0
    };
    let geometry = TileGridGeometry {
        viewport_width: width,
        viewport_height: 160.0,
        scroll_y: 200_000.0,
        minimum_tile_width: 360.0,
        tile_height: 80.0,
        gap: 6.0,
    };
    let range = tile_grid_range(records.len(), geometry)
        .map_err(|error| format!("tile availability range: {error:?}"))?;
    let focused_index = records
        .iter()
        .position(|record| record.id == focused)
        .ok_or("tile availability record missing")?;
    require(
        !(range.first..range.end).contains(&focused_index),
        "tile availability must exercise an offscreen owner",
    )?;
    let expected = records[range.first..range.end]
        .iter()
        .map(|record| record.id)
        .collect::<Vec<_>>();
    let mut layout_engine = RetainedLayoutEngine::default();
    let mut semantics = AccessibilityTree::default();
    let mut previous_semantic = None;
    for enabled in [false, true] {
        // Retention belongs to the caller's current admissible focus, not history.
        let retained = enabled
            .then(|| input.focus())
            .flatten()
            .filter(|owner| *owner == target)
            .map(|_| focused_index);
        let mut built = Vec::new();
        let (tiles, actual) = tile_grid_windowed_with_retained_content(
            TileGridIds {
                root: shell(scope, 1),
                top: shell(scope, 2),
                grid: shell(scope, 3),
                bottom: shell(scope, 4),
            },
            records.len(),
            geometry,
            retained,
            |index| {
                let record = &records[index];
                built.push(record.id);
                TileGridContent {
                    root: identity(scope, record.id, 0),
                    content: content(scope, record, WidgetTheme::dark()),
                    selected: selection.contains(&record.id),
                    action: Action::Select(record.id),
                }
            },
            WidgetTheme::dark(),
        )
        .map_err(|error| format!("tile availability projection: {error:?}"))?;
        require(actual == range, "availability changed tile range")?;
        require(
            built == expected && built.len() <= 16,
            "disabled or retired owner reached the bounded factory",
        )?;
        let tiles = tiles
            .conditioned(WidgetAvailability {
                visible: true,
                enabled,
            })
            .ok_or("visible tile projection omitted")?;
        let viewport = shell(scope, 7);
        let mut host = Widget::from_element(
            Element::scroll_box(
                viewport.element,
                viewport.key,
                Point::new(0.0, geometry.scroll_y),
            )
            .styled(Style::default().size(Dimension::Px(width), Dimension::Px(160.0))),
        )
        .child(tiles);
        host.element.style.flow_direction = Some(if window == UiWindowId(1) {
            FlowDirection::LeftToRight
        } else {
            FlowDirection::RightToLeft
        });
        let layout = layout_engine
            .compute_with_text_context(&host.element, Viewport::new(width, 220.0, 1.0), context)
            .map_err(|error| format!("tile availability layout: {error}"))?;
        let route = ActionRoute::build(&host.element, host.actions)
            .map_err(|error| format!("tile availability route: {error:?}"))?;
        let focus = FocusScope::new(window, &route, None);
        focus.reconcile(&mut input);
        require(
            input.focus().is_none() && input.capture().is_none(),
            "disable failed to retire focus or enable resurrected its owner",
        )?;
        require(
            layout.node(target.element).is_none(),
            "retired tile still projected",
        )?;
        let update = semantics.build(&host.element, &layout, None, "Tile availability");
        require(
            semantics.node_for_element(target.element).is_none(),
            "retired offscreen child retained semantics",
        )?;
        let hits = HitTestTree::build(&host.element, &layout);
        let mut checked = 0;
        for record in &built {
            let child = identity(scope, *record, 4).element;
            let node = layout
                .node(child)
                .ok_or("tile availability child missing")?;
            let point = Point::new(
                node.rect.x + node.rect.width / 2.0,
                node.rect.y + node.rect.height / 2.0,
            );
            if !node.inherited_clip.contains(point) || point.y >= 220.0 {
                continue;
            }
            let hit = hits
                .hit(point)
                .ok_or("tile availability child not hittable")?;
            require(
                route.action_for_hit(hit).copied() == enabled.then_some(Action::Open(*record)),
                "disabled child action escaped the tile barrier",
            )?;
            require(
                focus.accepts(InputTarget {
                    window,
                    element: child,
                }) == enabled,
                "disabled child admitted focus or enabled child remained blocked",
            )?;
            let id = semantics
                .node_for_element(child)
                .ok_or("tile child semantics missing")?;
            let semantic = update
                .nodes
                .iter()
                .find(|(candidate, _)| *candidate == id)
                .ok_or("tile child semantic update missing")?;
            require(
                semantic.1.is_disabled() != enabled,
                "tile disabled semantics disagree",
            )?;
            if checked == 0 {
                if let Some(previous) = previous_semantic {
                    require(
                        previous == id,
                        "availability changed stable semantic identity",
                    )?;
                }
                previous_semantic = Some(id);
            }
            checked += 1;
        }
        require(
            checked > 0,
            "tile availability has no visible child assertion",
        )?;
        println!(
            "tile availability window={} enabled={enabled} built={} hits={checked} focus_retired=true",
            window.0,
            built.len()
        );
    }
    require(
        projection.input.focus() == Some(target),
        "availability qualification changed original deletion precondition",
    )?;
    Ok(())
}
