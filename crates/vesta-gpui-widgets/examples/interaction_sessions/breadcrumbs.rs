//! Caller-owned dynamic paths inside a clickable compound host.

use super::*;
use vesta_gpui::{AccessibilityTree, SemanticRole, Style};
use vesta_gpui_widgets::{BreadcrumbIds, BreadcrumbSegment, breadcrumb};

fn part(value: u32) -> u32 {
    100 + value * 3
}

fn projection(
    scope: ElementScope,
    path: &[u32],
    width: f32,
    expanded: bool,
    theme: WidgetTheme,
) -> Widget<Action> {
    let id = |part| WidgetIdentity::new(scope.element(part), format!("path.part.{part}"));
    let segments = path
        .iter()
        .map(|&value| {
            let label = match value {
                11 => "Vesta",
                29 => "项目",
                47 => "Assets",
                83 => "Cube",
                _ => "Caller item",
            };
            BreadcrumbSegment {
                root: id(part(value)),
                separator: id(part(value) + 1),
                text: id(part(value) + 2),
                label: label.into(),
                action: (value != 29).then_some(Action::Select(FieldId(value))),
            }
        })
        .collect();
    Widget::from_element(
        Element::container(scope.element(500), ElementKey::new("path.host"))
            .styled(Style::default().size(Dimension::Px(900.0), Dimension::Px(400.0))),
    )
    .on_click(Action::Apply)
    .child(breadcrumb(
        BreadcrumbIds {
            root: id(1),
            row: id(2),
            overflow: id(3),
            overflow_label: id(4),
            hidden: id(5),
        },
        segments,
        width,
        expanded,
        Action::Open(FieldId(999)),
        theme,
    ))
}

pub(super) fn qualify(context: &mut TextInputContext) -> Result<(), String> {
    let mut frames = 0;
    for window in [UiWindowId(59), UiWindowId(61)] {
        let scope = ElementIdAllocator.allocate_scope().map_err(debug_error)?;
        let mut retained = RetainedLayoutEngine::default();
        let mut accessibility = AccessibilityTree::default();
        let mut identities = BTreeMap::new();
        for theme in [WidgetTheme::dark(), WidgetTheme::light()] {
            for path in [
                &[][..],
                &[11][..],
                &[11, 29][..],
                &[11, 29, 47, 83][..],
                &[11, 47, 83][..],
                &[11, 83, 47][..],
                &[11, 47][..],
                &[11, 29, 47, 83][..],
            ] {
                for width in [80.0, 1000.0] {
                    for expanded in [false, true] {
                        let widget = projection(scope, path, width, expanded, theme);
                        let layout = retained
                            .compute_with_text_context(
                                &widget.element,
                                Viewport::new(900.0, 400.0, 1.25),
                                context,
                            )
                            .map_err(debug_error)?;
                        let hits = HitTestTree::build(&widget.element, &layout);
                        let route = ActionRoute::build(&widget.element, widget.actions)
                            .map_err(debug_error)?;
                        accessibility.build(&widget.element, &layout, None, "Caller path");
                        identities.retain(|id, _| {
                            widget.element.find(scope.element(part(*id))).is_some()
                        });
                        for (index, &value) in path.iter().enumerate() {
                            let id = scope.element(part(value));
                            let enabled = value != 29 && index + 1 < path.len();
                            if let Some(node) = widget.element.find(id) {
                                let bounds = layout.node(id).ok_or("missing path layout")?.rect;
                                let hit = hits
                                    .hit(Point::new(
                                        bounds.x + bounds.width / 2.0,
                                        bounds.y + bounds.height / 2.0,
                                    ))
                                    .ok_or("missing path hit")?;
                                require(
                                    hit == id
                                        && route.action_for_hit(hit)
                                            == enabled.then_some(&Action::Select(FieldId(value))),
                                    "current/unavailable breadcrumb leaked outer action or enabled ancestor lost ownership",
                                )?;
                                require(
                                    route.action_for_hit(scope.element(part(value) + 2))
                                        == enabled.then_some(&Action::Select(FieldId(value))),
                                    "decorative path text changed root action ownership",
                                )?;
                                require(
                                    route.focus_order().contains(&id) == enabled,
                                    "current/unavailable path entered Tab",
                                )?;
                                require(
                                    node.semantic.as_ref().is_some_and(|s| {
                                        s.role
                                            == if enabled {
                                                SemanticRole::Button
                                            } else {
                                                SemanticRole::Label
                                            }
                                    }),
                                    "path role changed independently of availability",
                                )?;
                                let semantic = accessibility
                                    .node_for_element(id)
                                    .ok_or("path semantic identity missing")?;
                                if let Some(previous) = identities.insert(value, semantic) {
                                    require(
                                        previous == semantic,
                                        "surviving path identity changed",
                                    )?;
                                }
                            } else {
                                require(
                                    !route.focus_order().contains(&id)
                                        && accessibility.node_for_element(id).is_none(),
                                    "omitted ancestor retained focus or semantics",
                                )?;
                            }
                        }
                        frames += 1;
                    }
                }
            }
            for next in [&[11, 29, 47, 83][..], &[11, 83, 47][..], &[11][..]] {
                let active = projection(scope, &[11, 29, 47, 83], 80.0, true, theme);
                let active_route =
                    ActionRoute::build(&active.element, active.actions).map_err(debug_error)?;
                let target = InputTarget {
                    window,
                    element: scope.element(part(47)),
                };
                require(
                    active_route.focus_order().contains(&target.element),
                    "active ancestor absent",
                )?;
                let mut input = InputOwnership::default();
                input.set_focus(Some(target));
                input.capture_pointer(target, PointerButton::Primary);
                let next_widget = projection(scope, next, 80.0, false, theme);
                let next_route = ActionRoute::build(&next_widget.element, next_widget.actions)
                    .map_err(debug_error)?;
                FocusScope::new(window, &next_route, None).reconcile(&mut input);
                require(
                    input.focus().is_none() && input.capture().is_none(),
                    "hidden/current/removed ancestor retained ownership",
                )?;
                FocusScope::new(window, &active_route, None).reconcile(&mut input);
                require(
                    input.focus().is_none() && input.capture().is_none(),
                    "reintroduced path revived an old gesture",
                )?;
            }
        }
    }
    println!(
        "Public breadcrumb contract: {frames} actual retained frames; two sources/themes, current/unavailable compound boundaries, controlled reorder/truncation, stable identities and ownership retirement pass. No OS/Reader claim."
    );
    Ok(())
}
