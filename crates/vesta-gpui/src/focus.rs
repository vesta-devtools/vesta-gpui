//! Window-owned focus navigation over the current enabled action tree (ADR-0306).

use crate::input_ownership::FocusRequest;
use crate::{ActionRoute, ElementId, InputOwnership, InputTarget, UiWindowId};

/// A transient view of one window's enabled tab stops, optionally restricted to a modal subtree.
/// Hosts retain their input state and choose the active subtree; this view owns no product state.
pub struct FocusScope<'a, A> {
    window: UiWindowId,
    actions: &'a ActionRoute<A>,
    root: Option<ElementId>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputReconciliation {
    pub lost_focus: bool,
    pub lost_capture: bool,
}

impl<'a, A> FocusScope<'a, A> {
    #[must_use]
    pub const fn new(
        window: UiWindowId,
        actions: &'a ActionRoute<A>,
        root: Option<ElementId>,
    ) -> Self {
        Self {
            window,
            actions,
            root,
        }
    }

    #[must_use]
    pub fn accepts(&self, target: InputTarget) -> bool {
        self.contains(target) && self.actions.focus_order().contains(&target.element)
    }

    /// Admit a platform action through the same live window/modal route as local input.
    /// Clickable non-tab-stop controls may activate; text/value edits require a tab stop.
    pub fn accessible_target(
        &self,
        semantics: &crate::AccessibilityTree,
        node: accesskit::NodeId,
        action: accesskit::Action,
    ) -> Option<InputTarget> {
        let target = InputTarget {
            window: self.window,
            element: semantics.element_for_node(node)?,
        };
        (self.contains(target)
            && semantics.supports_action(target.element, action)
            && (self.accepts(target)
                || action == accesskit::Action::Click
                    && self.actions.action_at(target.element).is_some()))
        .then_some(target)
    }

    /// Any live enabled target in this window and modal range, including non-tab-stop
    /// pointer capture owners and accessible action controls.
    #[must_use]
    pub fn contains(&self, target: InputTarget) -> bool {
        target.window == self.window
            && self.actions.path_to(target.element).is_some()
            && self
                .root
                .is_none_or(|root| self.actions.is_descendant_of(target.element, root))
    }

    /// Current root-to-target path, restricted to this window's live modal scope.
    /// Non-tab-stop pointer/scroll owners are valid; disabled or stale targets are not.
    #[must_use]
    pub fn path_to(&self, target: InputTarget) -> Option<Vec<ElementId>> {
        if !self.contains(target) {
            return None;
        }
        let path = self.actions.path_to(target.element)?;
        let start = self
            .root
            .and_then(|root| path.iter().position(|id| *id == root))
            .unwrap_or(0);
        Some(path[start..].to_vec())
    }

    /// Stable window-qualified targets in the current declarative tree order.
    #[must_use]
    pub fn order(&self) -> Vec<InputTarget> {
        self.actions
            .focus_order()
            .iter()
            .copied()
            .map(|element| InputTarget {
                window: self.window,
                element,
            })
            .filter(|target| {
                self.root
                    .is_none_or(|root| self.actions.is_descendant_of(target.element, root))
            })
            .collect()
    }

    pub fn traverse(&self, input: &mut InputOwnership, backwards: bool) -> bool {
        input.traverse(&self.order(), backwards)
    }

    /// A closed popup may restore only a live tab stop of its own window and active modal range.
    #[must_use]
    pub fn restore_target(&self, opener: InputTarget) -> Option<InputTarget> {
        self.accepts(opener).then_some(opener)
    }

    /// Identify an owning control from a focused child without dispatching its action.
    /// The callback sees nearest ancestors first and never sees actions outside this
    /// window's live focus/modal range. Child input remains owned by the exact target.
    pub fn ancestor_action<T>(
        &self,
        target: InputTarget,
        mut match_action: impl FnMut(&A) -> Option<T>,
    ) -> Option<T> {
        if !self.accepts(target) {
            return None;
        }
        for element in self.actions.path_to(target.element)?.into_iter().rev() {
            if self
                .root
                .is_some_and(|root| !self.actions.is_descendant_of(element, root))
            {
                break;
            }
            if let Some(found) = self.actions.action_at(element).and_then(&mut match_action) {
                return Some(found);
            }
        }
        None
    }

    /// Consume one deferred restore or traversal against this rebuilt tree. `None`
    /// means no request; `Some(None)` rejects an invalid target/window or empty order.
    /// Hosts apply an accepted target with their normal text/IME focus adapter and
    /// rebuild the focused projection before publishing the frame.
    #[must_use]
    pub fn resolve_request(&self, input: &mut InputOwnership) -> Option<Option<InputTarget>> {
        input.take_focus_request().map(|request| match request {
            FocusRequest::Target(target) => self.restore_target(target),
            FocusRequest::Traverse {
                window,
                anchor,
                backwards,
            } => {
                if window != self.window {
                    return None;
                }
                InputOwnership::traversal_target(&self.order(), anchor, backwards)
            }
        })
    }

    /// Reconcile focus after rebuilding, including removal, disablement and modal changes.
    pub fn retain_focus(&self, input: &mut InputOwnership) -> bool {
        input.retain_live_focus(&self.order())
    }

    /// Reconcile both owners against the same rebuilt tree. Hosts cancel previews
    /// before ending text sessions, then rebuild before publishing a changed frame.
    pub fn reconcile(&self, input: &mut InputOwnership) -> InputReconciliation {
        let captured = input.capture();
        let lost_focus = self.retain_focus(input);
        let lost_capture = input.retain_live_capture(|target| self.contains(target))
            || captured.is_some() && input.capture().is_none();
        InputReconciliation {
            lost_focus,
            lost_capture,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Element, ElementKey, PointerButton, Style};

    #[test]
    fn modal_navigation_and_restore_reject_stale_disabled_and_foreign_targets() {
        let id = ElementId::new;
        let control = |value| {
            Element::container(id(value), ElementKey::new(format!("control.{value}")))
                .styled(Style::default().focusable(true))
        };
        let root = Element::container(id(1), ElementKey::new("root"))
            .child(control(2))
            .child(
                Element::container(id(3), ElementKey::new("dialog"))
                    .child(control(4))
                    .child(control(5))
                    .child(
                        Element::container(id(6), ElementKey::new("disabled"))
                            .styled(Style::default().action_boundary(true))
                            .child(control(7)),
                    ),
            );
        let route =
            ActionRoute::build(&root, [(id(1), 10), (id(3), 30), (id(4), 40)]).expect("valid tree");
        let window = UiWindowId(10);
        let target = |value| InputTarget {
            window,
            element: id(value),
        };
        let scope = FocusScope::new(window, &route, Some(id(3)));
        assert_eq!(
            scope.ancestor_action(target(4), |action| Some(*action)),
            Some(40)
        );
        assert_eq!(
            scope.ancestor_action(target(4), |action| (*action == 30).then_some(*action)),
            Some(30)
        );
        assert_eq!(
            scope.ancestor_action(target(4), |action| (*action == 10).then_some(*action)),
            None
        );
        for value in [2, 7, 99] {
            assert_eq!(
                scope.ancestor_action(target(value), |action| Some(*action)),
                None
            );
        }
        assert_eq!(
            scope.ancestor_action(
                InputTarget {
                    window: UiWindowId(11),
                    ..target(4)
                },
                |action| Some(*action)
            ),
            None
        );
        let mut input = InputOwnership::default();
        input.set_focus(Some(target(2)));
        input.capture_pointer(target(2), PointerButton::Primary);
        assert!(scope.traverse(&mut input, false));
        assert_eq!(input.focus(), Some(target(4)));
        assert_eq!(input.capture(), None);
        scope.traverse(&mut input, true);
        assert_eq!(input.focus(), Some(target(5)));
        assert_eq!(scope.restore_target(target(4)), Some(target(4)));
        for value in [2, 7, 99] {
            assert_eq!(scope.restore_target(target(value)), None);
        }
        assert_eq!(
            scope.restore_target(InputTarget {
                window: UiWindowId(11),
                ..target(4)
            }),
            None
        );
        input.set_focus(Some(target(2)));
        assert!(scope.retain_focus(&mut input));
        assert_eq!(input.focus(), None);
        input.set_focus(Some(target(4)));
        for value in [2, 7, 99] {
            input.capture_pointer(target(value), PointerButton::Primary);
            assert_eq!(
                scope.reconcile(&mut input),
                InputReconciliation {
                    lost_focus: false,
                    lost_capture: true,
                }
            );
            assert_eq!(input.focus(), Some(target(4)));
            assert_eq!(input.capture(), None);
        }
        input.capture_pointer(target(5), PointerButton::Primary);
        assert_eq!(scope.reconcile(&mut input), InputReconciliation::default());
        assert!(input.capture().is_some());
        input.request_focus(target(4));
        let previous = input.focus();
        assert_eq!(input.pending_focus_target(window), Some(target(4)));
        assert_eq!(input.pending_focus_target(UiWindowId(11)), None);
        assert_eq!(
            input.pending_focus_target(window),
            Some(target(4)),
            "peeking does not consume"
        );
        assert_eq!(input.focus(), previous);
        assert_eq!(input.capture(), None);
        assert_eq!(scope.resolve_request(&mut input), Some(Some(target(4))));
        assert_eq!(input.pending_focus_target(window), None);
        assert_eq!(
            scope.resolve_request(&mut input),
            None,
            "requests are consumed once"
        );
        for value in [2, 7, 99] {
            input.request_focus(target(value));
            assert_eq!(
                input.pending_focus_target(window),
                Some(target(value)),
                "a projection hint cannot replace disabled/modal/missing validation"
            );
            assert_eq!(scope.resolve_request(&mut input), Some(None));
        }
        input.request_focus(target(4));
        input.set_focus(Some(target(5)));
        assert_eq!(
            scope.resolve_request(&mut input),
            None,
            "new input supersedes a pending restore"
        );
        input.request_focus(target(4));
        input.capture_pointer(target(5), PointerButton::Primary);
        assert_eq!(
            scope.resolve_request(&mut input),
            None,
            "a new gesture supersedes a restore"
        );
        input.request_focus(InputTarget {
            window: UiWindowId(11),
            ..target(4)
        });
        assert_eq!(input.pending_focus_target(window), None);
        assert_eq!(
            input.pending_focus_target(UiWindowId(11)),
            Some(InputTarget {
                window: UiWindowId(11),
                ..target(4)
            })
        );
        assert_eq!(scope.resolve_request(&mut input), Some(None));
        // Popup dismissal can remove tab stops before the next frame. Resolve
        // a staged step against that frame's live modal scope, only once.
        for (anchor, backwards, expected) in [
            (Some(target(4)), false, target(5)),
            (Some(target(5)), true, target(4)),
            (Some(target(99)), false, target(4)),
            (Some(target(99)), true, target(5)),
            (None, false, target(4)),
        ] {
            input.set_focus(anchor);
            input.capture_pointer(target(4), PointerButton::Primary);
            input.request_focus_traversal(window, backwards);
            assert_eq!(input.pending_focus_target(window), None);
            assert_eq!(input.focus(), anchor, "resolution waits for the new tree");
            assert_eq!(input.capture(), None);
            assert_eq!(scope.resolve_request(&mut input), Some(Some(expected)));
            assert_eq!(scope.resolve_request(&mut input), None);
        }
        input.request_focus_traversal(UiWindowId(11), false);
        assert_eq!(scope.resolve_request(&mut input), Some(None));
        input.request_focus_traversal(window, false);
        input.set_focus(Some(target(4)));
        assert_eq!(scope.resolve_request(&mut input), None);
        input.request_focus_traversal(window, false);
        input.capture_pointer(target(5), PointerButton::Primary);
        assert_eq!(scope.resolve_request(&mut input), None);
        input.request_focus_traversal(window, false);
        assert!(input.cancel_focus_request());
        assert_eq!(scope.resolve_request(&mut input), None);
        input.request_focus_traversal(window, false);
        input.request_focus(target(4));
        assert_eq!(scope.resolve_request(&mut input), Some(Some(target(4))));
        input.request_focus(target(5));
        input.request_focus_traversal(window, false);
        assert_eq!(scope.resolve_request(&mut input), Some(Some(target(5))));
        let missing = FocusScope::new(window, &route, Some(id(99)));
        input.request_focus_traversal(window, false);
        assert_eq!(missing.resolve_request(&mut input), Some(None));
        input.set_focus(Some(target(4)));
        missing.traverse(&mut input, false);
        assert_eq!(input.focus(), None);

        // Reordering is an identity-preserving rebuild, not a new focus/capture.
        let rebuild = |order: &[u64]| {
            let dialog = order.iter().fold(
                Element::container(id(3), ElementKey::new("dialog")),
                |dialog, value| dialog.child(control(*value)),
            );
            let root = Element::container(id(1), ElementKey::new("root"))
                .child(control(2))
                .child(dialog);
            let actions = [(id(1), 10), (id(3), 30)]
                .into_iter()
                .chain(order.contains(&4).then_some((id(4), 40)));
            ActionRoute::build(&root, actions).expect("rebuilt route")
        };
        let before = rebuild(&[4, 5, 8]);
        let reordered = rebuild(&[5, 4, 8]);
        let before = FocusScope::new(window, &before, Some(id(3)));
        let reordered = FocusScope::new(window, &reordered, Some(id(3)));
        input.set_focus(Some(target(4)));
        input.capture_pointer(target(3), PointerButton::Primary);
        assert!(reordered.contains(target(3)));
        assert!(
            !reordered.accepts(target(3)),
            "capture need not be a tab stop"
        );
        assert_eq!(reordered.order(), vec![target(5), target(4), target(8)]);
        assert_eq!(
            reordered.reconcile(&mut input),
            InputReconciliation::default()
        );
        assert_eq!(input.focus(), Some(target(4)));
        assert_eq!(
            input.capture().map(|capture| capture.target),
            Some(target(3))
        );

        input.request_focus_traversal(window, false);
        assert_eq!(
            input.focus(),
            Some(target(4)),
            "request waits for publication"
        );
        assert_eq!(input.capture(), None);
        let mut old_tree = input.clone();
        assert_eq!(before.resolve_request(&mut old_tree), Some(Some(target(5))));
        assert_eq!(reordered.resolve_request(&mut input), Some(Some(target(8))));
        assert_eq!(reordered.resolve_request(&mut input), None);

        let removed = rebuild(&[5, 8]);
        input.set_focus(Some(target(4)));
        input.capture_pointer(target(3), PointerButton::Primary);
        assert_eq!(
            FocusScope::new(window, &removed, Some(id(3))).reconcile(&mut input),
            InputReconciliation {
                lost_focus: true,
                lost_capture: true
            }
        );
        assert_eq!(input.focus(), None);
        assert_eq!(input.capture(), None);
    }
}
