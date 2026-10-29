use super::*;
use vesta_gpui::ActionRoute;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Action {
    FocusSearch,
    ClearSearch,
}

#[test]
fn compound_widget_maps_child_actions_and_routes_nested_hits() {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum ChildAction {
        Clear,
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum ParentAction {
        Focus,
        Search(ChildAction),
    }

    let root = ElementId::new(1);
    let child = ElementId::new(2);
    let leaf = ElementId::new(3);
    let clear = Widget::from_element(
        Element::container(child, ElementKey::new("search.clear")).child(Element::text(
            leaf,
            ElementKey::new("search.clear.icon"),
            "×",
        )),
    )
    .on_click(ChildAction::Clear)
    .map_action(ParentAction::Search);
    let search = Widget::from_element(Element::container(root, ElementKey::new("search")))
        .on_click(ParentAction::Focus)
        .child(clear);
    let route = ActionRoute::build(&search.element, search.actions).expect("valid compound");

    assert_eq!(route.action_for_hit(root), Some(&ParentAction::Focus));
    assert_eq!(
        route.action_for_hit(leaf),
        Some(&ParentAction::Search(ChildAction::Clear))
    );
    for state in [ControlState::Idle, ControlState::Disabled] {
        struct Item {
            label: &'static str,
            image: &'static str,
        }
        let item = Item {
            label: "Explicit row label",
            image: "test://image",
        };
        let build = |item: &Item| {
            WidgetContent::new(
                item.label,
                Widget::from_element(
                    Element::container(child, ElementKey::new("row.content"))
                        .styled(Style::default().focusable(true))
                        .child(Element::image(
                            leaf,
                            ElementKey::new("row.image"),
                            item.image,
                        )),
                )
                .on_click(ParentAction::Search(ChildAction::Clear)),
            )
        };
        let row = list_item_from(
            WidgetIdentity::new(root, "row"),
            &item,
            true,
            ParentAction::Focus,
            state,
            WidgetTheme::dark(),
            build,
        );
        assert_eq!(
            row.element
                .semantic
                .as_ref()
                .expect("list item semantics")
                .label,
            item.label
        );
        let route = ActionRoute::build(&row.element, row.actions).expect("arbitrary row content");
        if state == ControlState::Disabled {
            assert_eq!(route.action_for_hit(leaf), None);
            assert!(route.focus_order().is_empty());
        } else {
            assert_eq!(
                route.action_for_hit(leaf),
                Some(&ParentAction::Search(ChildAction::Clear))
            );
            assert_eq!(route.focus_order(), &[root, child]);
        }
    }
}

#[test]
fn computed_condition_disables_or_omits_a_compound_child() {
    use vesta_gpui::{
        AccessibilityTree, FocusScope, HitTestTree, InputOwnership, InputTarget, LayoutEngine,
        Point, PointerButton, PointerClick, Semantic, SemanticRole, UiWindowId, Viewport,
    };
    let parent_id = ElementId::new(1);
    let child_id = ElementId::new(2);
    let leaf_id = ElementId::new(3);
    let child = || {
        Widget::from_element(
            Element::container(child_id, ElementKey::new("child"))
                .semantic(Semantic::new(SemanticRole::Button, "Clear"))
                .styled(
                    Style::default()
                        .focusable(true)
                        .size(Dimension::Px(80.0), Dimension::Px(30.0)),
                )
                .child(Element::text(leaf_id, ElementKey::new("leaf"), "Clear")),
        )
        .on_click(Action::ClearSearch)
    };
    let disabled = child()
        .conditioned(WidgetAvailability {
            visible: true,
            enabled: false,
        })
        .expect("disabled child remains visible");
    let parent = Widget::from_element(Element::container(parent_id, ElementKey::new("parent")))
        .on_click(Action::FocusSearch)
        .child(disabled);
    let route = ActionRoute::build(&parent.element, parent.actions).expect("valid route");
    assert_eq!(route.action_for_hit(leaf_id), None);
    assert!(!route.focus_order().contains(&child_id));

    assert!(
        child()
            .conditioned(WidgetAvailability {
                visible: false,
                enabled: true,
            })
            .is_none()
    );

    let window = UiWindowId(7);
    let target = InputTarget {
        window,
        element: child_id,
    };
    let mut ownership = InputOwnership::default();
    let mut click = PointerClick::default();
    let mut semantics = AccessibilityTree::default();
    let mut previous_node = None;
    // A press cannot survive disabling/removing its owner; restoration uses
    // the same IDs without reviving focus, capture or that earlier click.
    for (visible, enabled) in [(true, true), (true, false), (false, true), (true, true)] {
        let mut projection = Widget::from_element(
            Element::container(parent_id, ElementKey::new("parent"))
                .styled(Style::default().size(Dimension::Px(160.0), Dimension::Px(60.0))),
        )
        .on_click(Action::FocusSearch);
        if let Some(child) = child().conditioned(WidgetAvailability { visible, enabled }) {
            projection = projection.child(child);
        }
        let route =
            ActionRoute::build(&projection.element, projection.actions).expect("current route");
        let layout = LayoutEngine
            .compute(&projection.element, Viewport::new(160.0, 60.0, 1.5))
            .expect("current geometry");
        let hit = HitTestTree::build(&projection.element, &layout)
            .hit(Point::new(10.0, 10.0))
            .expect("parent or child hit");
        let repairs = FocusScope::new(window, &route, None).reconcile(&mut ownership);
        let update = semantics.build(
            &projection.element,
            &layout,
            ownership.focus().map(|focus| focus.element),
            "Conditions",
        );
        if visible {
            let node = semantics
                .node_for_element(child_id)
                .expect("visible child semantics");
            if let Some(previous) = previous_node {
                assert_eq!(
                    node, previous,
                    "a persistent owner keeps its semantic identity"
                );
            }
            previous_node = Some(node);
            let child_node = &update
                .nodes
                .iter()
                .find(|(id, _)| *id == node)
                .expect("child node")
                .1;
            assert_eq!(child_node.is_disabled(), !enabled);
            assert_eq!(
                route.action_for_hit(hit),
                enabled.then_some(&Action::ClearSearch)
            );
        } else {
            assert!(layout.node(child_id).is_none());
            assert!(semantics.node_for_element(child_id).is_none());
            assert_eq!(route.action_for_hit(hit), Some(&Action::FocusSearch));
            previous_node = None;
        }
        if visible && enabled {
            assert!(ownership.focus().is_none() && ownership.capture().is_none());
            assert!(
                click
                    .release(window, PointerButton::Primary, &route, Some(hit))
                    .is_none()
            );
            ownership.set_focus(Some(target));
            ownership.capture_pointer(target, PointerButton::Primary);
            click.press(window, PointerButton::Primary, &route, Some(hit));
        } else {
            assert!(ownership.focus().is_none() && ownership.capture().is_none());
            if visible {
                assert!(repairs.lost_focus && repairs.lost_capture);
            }
            assert!(
                click
                    .release(window, PointerButton::Primary, &route, Some(hit))
                    .is_none()
            );
        }
    }
}
