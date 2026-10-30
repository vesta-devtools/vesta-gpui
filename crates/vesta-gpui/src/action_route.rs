//! Typed action routing through a declarative element tree.
//!
//! Hit testing identifies the innermost pointer target. A compound widget may
//! register its action on a parent while leaving interactive children inside
//! it, so dispatch walks towards the root until it finds the nearest action.

use std::collections::{HashMap, HashSet, hash_map::Entry};

use crate::{Element, ElementId, ElementKey, InputTarget, PointerButton, UiWindowId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionRouteError {
    DuplicateElement(ElementId),
    DuplicateElementKey(ElementKey),
    DuplicateAction(ElementId),
    UnknownActionTarget(ElementId),
}

/// A tree event visits ancestors before its target, then returns through them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventPhase {
    Capture,
    Target,
    Bubble,
}

#[derive(Clone, Debug)]
pub struct ActionRoute<A> {
    parents: HashMap<ElementId, Option<ElementId>>,
    blocked: HashSet<ElementId>,
    text_inputs: HashSet<ElementId>,
    actions: HashMap<ElementId, A>,
    focus_order: Vec<ElementId>,
}

impl<A> Default for ActionRoute<A> {
    fn default() -> Self {
        Self {
            parents: HashMap::new(),
            blocked: HashSet::new(),
            text_inputs: HashSet::new(),
            actions: HashMap::new(),
            focus_order: Vec::new(),
        }
    }
}

impl<A> ActionRoute<A> {
    pub(crate) fn inspection_parent(&self, element: ElementId) -> Option<Option<ElementId>> {
        self.parents.get(&element).copied()
    }
    pub fn build(
        root: &Element,
        actions: impl IntoIterator<Item = (ElementId, A)>,
    ) -> Result<Self, ActionRouteError> {
        let _trace = crate::trace::span("gpui.input", "action_route.build");
        root.validate_unique_identity(&mut HashSet::new(), &mut HashSet::new())
            .map_err(|duplicate| match duplicate {
                crate::element::DuplicateIdentity::Id(id) => ActionRouteError::DuplicateElement(id),
                crate::element::DuplicateIdentity::Key(key) => {
                    ActionRouteError::DuplicateElementKey(key)
                }
            })?;
        let mut route = Self::default();
        route.collect_parents(root, None, false);
        for (target, action) in actions {
            if !route.parents.contains_key(&target) {
                return Err(ActionRouteError::UnknownActionTarget(target));
            }
            match route.actions.entry(target) {
                Entry::Vacant(slot) => {
                    slot.insert(action);
                }
                Entry::Occupied(_) => return Err(ActionRouteError::DuplicateAction(target)),
            }
        }
        Ok(route)
    }

    fn collect_parents(&mut self, element: &Element, parent: Option<ElementId>, blocked: bool) {
        self.parents.insert(element.id, parent);
        if matches!(&element.kind, crate::ElementKind::TextInput { .. }) {
            self.text_inputs.insert(element.id);
        }
        let blocked = blocked || element.style.action_boundary;
        if blocked {
            self.blocked.insert(element.id);
        }
        if !blocked && element.style.focusable {
            self.focus_order.push(element.id);
        }
        for child in &element.children {
            self.collect_parents(child, Some(element.id), blocked);
        }
    }

    /// Present, enabled tab stops in element-tree order. The host supplies the
    /// current window identity when handing this order to `InputOwnership`.
    #[must_use]
    pub fn focus_order(&self) -> &[ElementId] {
        &self.focus_order
    }

    /// Whether an element belongs to a named subtree, including the subtree
    /// root itself. Useful for host-owned scroll-to-focus without duplicating
    /// the declarative tree's parent relationships.
    #[must_use]
    pub fn is_descendant_of(&self, element: ElementId, ancestor: ElementId) -> bool {
        let mut current = Some(element);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.parents.get(&id).copied().flatten();
        }
        false
    }

    /// The current root-to-element path. Rebuilds replace this transient path;
    /// callers retain stable element identities, not positions in the tree.
    #[must_use]
    pub fn path_to(&self, element: ElementId) -> Option<Vec<ElementId>> {
        if !self.parents.contains_key(&element) || self.blocked.contains(&element) {
            return None;
        }
        let mut path = Vec::new();
        let mut current = Some(element);
        while let Some(id) = current {
            path.push(id);
            current = self.parents.get(&id).copied().flatten();
        }
        path.reverse();
        Some(path)
    }

    /// Dispatch through the current tree. A handler returns `true` to stop
    /// propagation; disabled subtrees and stale targets receive no event.
    pub fn dispatch_event(
        &self,
        target: ElementId,
        mut handle: impl FnMut(EventPhase, ElementId) -> bool,
    ) -> bool {
        let Some(path) = self.path_to(target) else {
            return false;
        };
        for &element in &path[..path.len() - 1] {
            if handle(EventPhase::Capture, element) {
                return true;
            }
        }
        if handle(EventPhase::Target, target) {
            return true;
        }
        for &element in path[..path.len() - 1].iter().rev() {
            if handle(EventPhase::Bubble, element) {
                return true;
            }
        }
        false
    }

    /// Activate a focused action owner without hit testing, e.g. Enter/Space.
    #[must_use]
    pub fn action_at(&self, element: ElementId) -> Option<&A> {
        (!self.blocked.contains(&element))
            .then(|| self.actions.get(&element))
            .flatten()
    }

    /// Resolve a hit to the nearest action, preferring a nested control over
    /// its containing widget. An unknown hit never reaches a parent action.
    #[must_use]
    pub fn action_for_hit(&self, hit: ElementId) -> Option<&A> {
        self.target_for_hit(hit).map(|(_, action)| action)
    }

    /// Return the stable owner of the action as well as the action itself.
    /// Pointer gestures can use the owner to require press and release on the
    /// same compound control even when the exact child hit changes.
    #[must_use]
    pub fn target_for_hit(&self, hit: ElementId) -> Option<(ElementId, &A)> {
        if self.blocked.contains(&hit) {
            return None;
        }
        let mut current = Some(hit);
        while let Some(element) = current {
            let parent = self.parents.get(&element)?;
            if let Some(action) = self.actions.get(&element) {
                return Some((element, action));
            }
            // A text editor owns its gesture even without a click action. It must not
            // activate a containing row/cell merely because the host starts text selection.
            if self.blocked.contains(&element) || self.text_inputs.contains(&element) {
                return None;
            }
            current = *parent;
        }
        None
    }
}

/// A pointer click activates only when the same routed action target owns both
/// the press and release. Drag controls can instead consume their press directly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PointerClick {
    pressed: Option<(InputTarget, PointerButton)>,
}

impl PointerClick {
    pub fn press<A>(
        &mut self,
        window: UiWindowId,
        button: PointerButton,
        route: &ActionRoute<A>,
        hit: Option<ElementId>,
    ) -> bool {
        self.pressed = hit.and_then(|hit| {
            route
                .target_for_hit(hit)
                .map(|(element, _)| (InputTarget { window, element }, button))
        });
        self.pressed.is_some()
    }

    pub fn release<'a, A>(
        &mut self,
        window: UiWindowId,
        button: PointerButton,
        route: &'a ActionRoute<A>,
        hit: Option<ElementId>,
    ) -> Option<&'a A> {
        let (pressed, pressed_button) = self.pressed?;
        if pressed.window != window || pressed_button != button {
            return None;
        }
        self.pressed = None;
        let (released_element, action) = hit.and_then(|hit| route.target_for_hit(hit))?;
        (pressed.element == released_element).then_some(action)
    }

    pub fn cancel(&mut self) {
        self.pressed = None;
    }

    /// Retire an unavailable action owner before the next route is published.
    /// A foreign window does not inspect or consume this window's pending click.
    /// Returns whether a pending press was cancelled; restoring the owner later
    /// cannot revive it. The host supplies current model availability.
    pub fn retain_live(
        &mut self,
        window: UiWindowId,
        mut is_live: impl FnMut(ElementId) -> bool,
    ) -> bool {
        if self
            .pressed
            .is_some_and(|(target, _)| target.window == window && !is_live(target.element))
        {
            self.cancel();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ElementKey;

    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Action {
        Open,
        Clear,
    }

    fn compound_widget() -> Element {
        Element::container(ElementId::new(1), ElementKey::new("root")).child(
            Element::container(ElementId::new(2), ElementKey::new("search")).child(Element::text(
                ElementId::new(3),
                ElementKey::new("clear"),
                "×",
            )),
        )
    }

    #[test]
    fn nested_action_precedes_parent_and_unhandled_child_bubbles() {
        let tree = compound_widget();
        let route = ActionRoute::build(
            &tree,
            [
                (ElementId::new(1), Action::Open),
                (ElementId::new(3), Action::Clear),
            ],
        )
        .expect("valid action route");

        assert_eq!(
            route.action_for_hit(ElementId::new(3)),
            Some(&Action::Clear)
        );
        assert_eq!(route.action_for_hit(ElementId::new(2)), Some(&Action::Open));
        assert_eq!(route.action_for_hit(ElementId::new(99)), None);
        let tree = tree.child(
            Element::text_input(
                ElementId::new(4),
                ElementKey::new("row.editor"),
                crate::TextInputState::default().snapshot(),
                false,
            )
            .styled(crate::Style::default().focusable(true))
            .child(Element::container(
                ElementId::new(5),
                ElementKey::new("editor.button"),
            )),
        );
        let route = ActionRoute::build(
            &tree,
            [
                (ElementId::new(1), Action::Open),
                (ElementId::new(5), Action::Clear),
            ],
        )
        .expect("row with editor");
        assert_eq!(
            route.target_for_hit(ElementId::new(4)),
            None,
            "text selection must not become an ancestor row click"
        );
        assert!(route.focus_order().contains(&ElementId::new(4)));
        assert_eq!(
            route.action_for_hit(ElementId::new(5)),
            Some(&Action::Clear)
        );
        let explicit = ActionRoute::build(&tree, [(ElementId::new(4), Action::Clear)])
            .expect("explicit editor action");
        assert_eq!(
            explicit.action_for_hit(ElementId::new(4)),
            Some(&Action::Clear)
        );
    }

    #[test]
    fn route_rejects_stale_or_ambiguous_targets() {
        let tree = compound_widget();
        assert!(matches!(
            ActionRoute::build(&tree, [(ElementId::new(99), Action::Open)]),
            Err(ActionRouteError::UnknownActionTarget(id)) if id == ElementId::new(99)
        ));
        assert!(matches!(
            ActionRoute::build(
                &tree,
                [(ElementId::new(2), Action::Open), (ElementId::new(2), Action::Clear)]
            ),
            Err(ActionRouteError::DuplicateAction(id)) if id == ElementId::new(2)
        ));
        let duplicate_root = Element::container(ElementId::new(1), ElementKey::new("root")).child(
            Element::container(ElementId::new(1), ElementKey::new("duplicate-root")),
        );
        assert!(matches!(
            ActionRoute::<Action>::build(&duplicate_root, []),
            Err(ActionRouteError::DuplicateElement(id)) if id == ElementId::new(1)
        ));
        let duplicate_key = tree.child(Element::container(
            ElementId::new(4),
            ElementKey::new("search"),
        ));
        assert!(matches!(
            ActionRoute::<Action>::build(&duplicate_key, []),
            Err(ActionRouteError::DuplicateElementKey(key)) if key.as_str() == "search"
        ));
        assert!(matches!(
            crate::LayoutEngine.compute(&duplicate_key, crate::Viewport::new(400.0, 300.0, 1.0)),
            Err(crate::LayoutError::DuplicateElementKey(key)) if key.as_str() == "search"
        ));
    }

    #[test]
    fn pointer_click_requires_matching_action_owner_window_and_button() {
        let tree = compound_widget();
        let route = ActionRoute::build(&tree, [(ElementId::new(2), Action::Open)])
            .expect("valid action route");
        let mut click = PointerClick::default();
        let window = UiWindowId(1);
        assert!(click.press(
            window,
            PointerButton::Primary,
            &route,
            Some(ElementId::new(2))
        ));
        assert_eq!(
            click.release(
                UiWindowId(2),
                PointerButton::Primary,
                &route,
                Some(ElementId::new(3))
            ),
            None
        );
        assert_eq!(
            click.release(
                window,
                PointerButton::Secondary,
                &route,
                Some(ElementId::new(3))
            ),
            None
        );
        assert_eq!(
            click.release(
                window,
                PointerButton::Primary,
                &route,
                Some(ElementId::new(3))
            ),
            Some(&Action::Open)
        );
        assert!(!click.press(window, PointerButton::Primary, &route, None));
        assert_eq!(
            click.release(
                window,
                PointerButton::Primary,
                &route,
                Some(ElementId::new(3))
            ),
            None
        );
        assert!(click.press(
            window,
            PointerButton::Primary,
            &route,
            Some(ElementId::new(2))
        ));
        assert_eq!(
            click.release(window, PointerButton::Primary, &route, None),
            None
        );
        assert!(click.press(
            window,
            PointerButton::Primary,
            &route,
            Some(ElementId::new(3))
        ));
        assert!(!click.retain_live(UiWindowId(2), |_| panic!("foreign owner inspected")));
        assert!(!click.retain_live(window, |owner| owner == ElementId::new(2)));
        assert_eq!(
            click.release(
                window,
                PointerButton::Primary,
                &route,
                Some(ElementId::new(3))
            ),
            Some(&Action::Open)
        );
        assert!(click.press(
            window,
            PointerButton::Primary,
            &route,
            Some(ElementId::new(3))
        ));
        assert!(click.retain_live(window, |_| false));
        assert!(!click.retain_live(window, |_| true));
        assert_eq!(
            click.release(
                window,
                PointerButton::Primary,
                &route,
                Some(ElementId::new(3))
            ),
            None,
            "restoring the same owner cannot revive its cancelled press"
        );
    }

    #[test]
    fn inert_child_blocks_a_compound_parent_action() {
        let tree = Element::container(ElementId::new(1), ElementKey::new("parent")).child(
            Element::container(ElementId::new(2), ElementKey::new("disabled"))
                .styled(
                    crate::Style::default()
                        .pointer_events(false)
                        .action_boundary(true),
                )
                .child(Element::text(
                    ElementId::new(3),
                    ElementKey::new("disabled.label"),
                    "Disabled",
                )),
        );
        let route = ActionRoute::build(
            &tree,
            [
                (ElementId::new(1), Action::Open),
                (ElementId::new(3), Action::Clear),
            ],
        )
        .expect("valid action route");
        assert_eq!(route.action_for_hit(ElementId::new(1)), Some(&Action::Open));
        assert_eq!(route.action_for_hit(ElementId::new(2)), None);
        assert_eq!(route.action_for_hit(ElementId::new(3)), None);
        assert_eq!(route.action_at(ElementId::new(3)), None);
    }

    #[test]
    fn focus_order_excludes_inert_subtrees_and_keeps_nested_tab_order() {
        let tree = Element::container(ElementId::new(1), ElementKey::new("root"))
            .child(
                Element::container(ElementId::new(2), ElementKey::new("first"))
                    .styled(crate::Style::default().focusable(true))
                    .child(
                        Element::container(ElementId::new(3), ElementKey::new("nested"))
                            .styled(crate::Style::default().focusable(true)),
                    ),
            )
            .child(
                Element::container(ElementId::new(4), ElementKey::new("disabled"))
                    .styled(
                        crate::Style::default()
                            .action_boundary(true)
                            .focusable(true),
                    )
                    .child(
                        Element::container(ElementId::new(5), ElementKey::new("disabled.child"))
                            .styled(crate::Style::default().focusable(true)),
                    ),
            )
            .child(
                Element::container(ElementId::new(6), ElementKey::new("last"))
                    .styled(crate::Style::default().focusable(true)),
            );
        let route = ActionRoute::<Action>::build(&tree, []).expect("valid route");
        assert_eq!(
            route.focus_order(),
            &[ElementId::new(2), ElementId::new(3), ElementId::new(6)]
        );
        assert!(route.is_descendant_of(ElementId::new(3), ElementId::new(2)));
        assert!(!route.is_descendant_of(ElementId::new(6), ElementId::new(2)));
    }

    #[test]
    fn phased_route_orders_ancestors_and_stops_at_a_consuming_widget() {
        let tree = compound_widget();
        let route = ActionRoute::<Action>::build(&tree, []).expect("valid route");
        assert_eq!(
            route.path_to(ElementId::new(3)),
            Some(vec![
                ElementId::new(1),
                ElementId::new(2),
                ElementId::new(3)
            ])
        );
        let mut visited = Vec::new();
        assert!(route.dispatch_event(ElementId::new(3), |phase, id| {
            visited.push((phase, id));
            phase == EventPhase::Bubble && id == ElementId::new(2)
        }));
        assert_eq!(
            visited,
            [
                (EventPhase::Capture, ElementId::new(1)),
                (EventPhase::Capture, ElementId::new(2)),
                (EventPhase::Target, ElementId::new(3)),
                (EventPhase::Bubble, ElementId::new(2)),
            ]
        );
        assert!(!route.dispatch_event(ElementId::new(99), |_, _| true));
    }
}
