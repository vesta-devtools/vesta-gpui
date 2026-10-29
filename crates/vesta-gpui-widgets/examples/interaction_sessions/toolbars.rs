//! Actual public layout and ownership oracle for ordered toolbar overflow.

use super::*;
use vesta_gpui::{AccessibilityTree, Style};
use vesta_gpui_widgets::{ToolbarIds, ToolbarItem, toolbar, toolbar_visible_count};

fn projection(
    scope: ElementScope,
    groups: &[u8],
    width: f32,
    expanded: bool,
    theme: WidgetTheme,
) -> Widget<Action> {
    let id = |part| WidgetIdentity::new(scope.element(part), format!("toolbar.part.{part}"));
    let items = groups
        .iter()
        .enumerate()
        .map(|(i, &group)| {
            let part = 10 + i as u32 * 4;
            ToolbarItem {
                root: id(part),
                glyph: id(part + 1),
                label: id(part + 2),
                separator: id(part + 3),
                icon: "+".into(),
                name: format!("Caller action {i}"),
                shortcut: Some("Enter".into()),
                group,
                state: if i == 2 {
                    ControlState::Disabled
                } else {
                    ControlState::Idle
                },
                action: Action::Select(FieldId(i as u32)),
            }
        })
        .collect();
    Widget::from_element(
        Element::container(scope.element(500), ElementKey::new("toolbar.host"))
            .styled(Style::default().size(Dimension::Px(500.0), Dimension::Px(400.0))),
    )
    .child(toolbar(
        ToolbarIds {
            root: id(1),
            row: id(2),
            overflow: id(3),
            overflow_label: id(4),
            hidden: id(5),
        },
        items,
        width,
        expanded,
        Action::Apply,
        theme,
    ))
}

pub(super) fn qualify(context: &mut TextInputContext) -> Result<(), String> {
    let mut frames = 0;
    let mut saw_relocated = false;
    for window in [UiWindowId(47), UiWindowId(53)] {
        let scope = ElementIdAllocator.allocate_scope().map_err(debug_error)?;
        let mut retained = RetainedLayoutEngine::default();
        let mut accessibility = AccessibilityTree::default();
        let mut identities = BTreeMap::new();
        for theme in [WidgetTheme::dark(), WidgetTheme::light()] {
            let extent = theme.metrics.control_height + 6.0;
            for groups in [
                &[][..],
                &[0][..],
                &[0, 0][..],
                &[0, 1][..],
                &[0, 0, 1, 1][..],
            ] {
                for width in [
                    0.0,
                    extent - 1.0,
                    extent,
                    68.0,
                    69.0,
                    70.0,
                    72.0,
                    73.0,
                    104.0,
                    106.0,
                    109.0,
                    200.0,
                ] {
                    for expanded in [false, true] {
                        let widget = projection(scope, groups, width, expanded, theme);
                        let body = &widget.element.children[0];
                        let row = &body.children[0];
                        let hidden = &body.children[1];
                        let layout = retained
                            .compute_with_text_context(
                                &widget.element,
                                Viewport::new(500.0, 400.0, 1.25),
                                context,
                            )
                            .map_err(debug_error)?;
                        let origin = layout.node(row.id).ok_or("missing toolbar row")?.rect.x;
                        let mut end = origin;
                        for child in &row.children {
                            let bounds = layout.node(child.id).ok_or("missing toolbar child")?.rect;
                            end = end.max(bounds.x + bounds.width);
                        }
                        // Inspect the real projected rectangles, not the capacity formula.
                        require(
                            end - origin <= width.max(extent) + 0.001,
                            &format!(
                                "toolbar exceeds budget: groups={groups:?} width={width} actual={}",
                                end - origin
                            ),
                        )?;
                        let route = ActionRoute::build(&widget.element, widget.actions)
                            .map_err(debug_error)?;
                        let overflow = row.children.iter().any(|n| n.id == scope.element(3));
                        let visible = row
                            .children
                            .iter()
                            .filter(|n| {
                                n.semantic
                                    .as_ref()
                                    .is_some_and(|s| s.label.starts_with("Caller action"))
                            })
                            .count();
                        require(
                            visible == toolbar_visible_count(groups, width, extent),
                            "count/projection diverged",
                        )?;
                        require(
                            overflow == (visible < groups.len()),
                            "overflow entry missing or spurious",
                        )?;
                        require(
                            hidden.children.len()
                                == if expanded { groups.len() - visible } else { 0 },
                            "collapsed actions remained projected",
                        )?;
                        accessibility.build(&widget.element, &layout, None, "Caller toolbar");
                        identities.retain(|i, _| *i < groups.len() && (*i < visible || expanded));
                        for i in 0..groups.len() {
                            let id = scope.element(10 + i as u32 * 4);
                            let present = i < visible || expanded;
                            let enabled = present && i != 2;
                            require(
                                route.action_for_hit(id)
                                    == enabled.then_some(&Action::Select(FieldId(i as u32))),
                                "relocated/disabled action ownership changed",
                            )?;
                            require(
                                route.focus_order().contains(&id) == enabled,
                                "hidden/disabled focus leaked",
                            )?;
                            if present {
                                let node = row
                                    .children
                                    .iter()
                                    .chain(&hidden.children)
                                    .find(|n| n.id == id)
                                    .ok_or("caller root identity lost")?;
                                require(
                                    node.semantic
                                        .as_ref()
                                        .is_some_and(|s| s.label == format!("Caller action {i}")),
                                    "full accessible name lost on relocation",
                                )?;
                                require(
                                    node.tooltip.as_deref()
                                        == Some(format!("Caller action {i} (Enter)").as_str()),
                                    "shortcut hint lost on relocation",
                                )?;
                                let semantic_id = accessibility
                                    .node_for_element(id)
                                    .ok_or("missing semantic root")?;
                                if let Some(previous) = identities.insert(i, semantic_id) {
                                    require(
                                        previous == semantic_id,
                                        "relocation replaced accessible identity",
                                    )?;
                                }
                                if i >= visible {
                                    saw_relocated = true;
                                }
                                let bounds = layout.node(id).ok_or("missing action bounds")?.rect;
                                if enabled {
                                    require(
                                        HitTestTree::build(&widget.element, &layout).hit(
                                            Point::new(
                                                bounds.x + bounds.width / 2.0,
                                                bounds.y + bounds.height / 2.0,
                                            ),
                                        ) == Some(id),
                                        "projected button lost its hit target",
                                    )?;
                                }
                            }
                        }
                        frames += 1;
                    }
                }
            }
            let expanded = projection(scope, &[0, 0, 1, 1], 70.0, true, theme);
            let expanded_route =
                ActionRoute::build(&expanded.element, expanded.actions).map_err(debug_error)?;
            let mut input = InputOwnership::default();
            let target = InputTarget {
                window,
                element: scope.element(22),
            };
            require(
                expanded_route.focus_order().contains(&target.element),
                "hidden enabled action absent",
            )?;
            input.set_focus(Some(target));
            input.capture_pointer(target, PointerButton::Primary);
            let closed = projection(scope, &[0, 0, 1, 1], 70.0, false, theme);
            let closed_route =
                ActionRoute::build(&closed.element, closed.actions).map_err(debug_error)?;
            FocusScope::new(window, &closed_route, None).reconcile(&mut input);
            require(
                input.focus().is_none() && input.capture().is_none(),
                "collapsed action retained ownership",
            )?;
        }
    }
    require(saw_relocated, "no overflow relocation was exercised")?;
    require(
        toolbar_visible_count(&[0, 0], 68.0, 34.0) == 0
            && toolbar_visible_count(&[0, 0], 70.0, 34.0) == 2
            && toolbar_visible_count(&[0, 1], 72.0, 34.0) == 1
            && toolbar_visible_count(&[0, 1], 73.0, 34.0) == 2,
        "critical width boundary changed",
    )?;
    require(
        toolbar_visible_count(&[0, 1], f32::INFINITY, 34.0) == 2
            && toolbar_visible_count(&[0, 1], f32::NAN, 34.0) == 0
            && toolbar_visible_count(&[0, 1], -1.0, 34.0) == 0
            && toolbar_visible_count(&[0, 1], 100.0, 0.0) == 0,
        "invalid/unbounded width contract changed",
    )?;
    println!(
        "Public toolbar contract: {frames} actual retained frames; two sources/themes, width/group boundaries, ordered actions, disabled hits and collapsed ownership pass. No OS/Reader claim."
    );
    Ok(())
}
