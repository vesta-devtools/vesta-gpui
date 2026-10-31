//! Semantic intent over the current route. Hosts retain business admission and dispatch.
use crate::{ActionRoute, ElementId, FocusScope, InputTarget, UiSnapshot, UiWindowId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiTargetRef {
    pub instance: u128,
    pub window: UiWindowId,
    pub element: ElementId,
    pub owner: u64,
}

/// Repaints preserve owner tokens; changed actions/semantics and removed owners invalidate them.
pub struct UiOwnerRegistry<A> {
    instance: u128,
    next: u64,
    entries: std::collections::HashMap<(UiWindowId, ElementId), Owner<A>>,
}
struct Owner<A> {
    generation: u64,
    action: Option<A>,
    role: Option<crate::SemanticRole>,
    name: crate::UiSnapshotText,
    parent: Option<ElementId>,
}
impl<A> Default for UiOwnerRegistry<A> {
    fn default() -> Self {
        Self {
            instance: 0,
            next: 0,
            entries: Default::default(),
        }
    }
}
impl<A: Clone + PartialEq> UiOwnerRegistry<A> {
    /// Stage changed generations before publishing. Capacity failure preserves
    /// the previous instance, cursor and all windows' references.
    pub fn update(
        &mut self,
        snapshot: &UiSnapshot,
        route: &ActionRoute<A>,
    ) -> Result<(), &'static str> {
        let changed_instance = self.instance != snapshot.id.instance;
        let window = snapshot.id.window;
        let mut next = self.next;
        let mut staged = Vec::new();
        for node in snapshot.nodes() {
            let action = route.action_at(node.id).cloned();
            let unchanged = !changed_instance
                && self.entries.get(&(window, node.id)).is_some_and(|old| {
                    old.action == action
                        && old.role == node.role
                        && old.name == node.name
                        && old.parent == node.parent
                });
            if !unchanged {
                next = next.checked_add(1).ok_or("owner_generation_exhausted")?;
                staged.push((
                    node.id,
                    Owner {
                        generation: next,
                        action,
                        role: node.role,
                        name: node.name.clone(),
                        parent: node.parent,
                    },
                ));
            }
        }
        if changed_instance {
            self.entries.clear();
            self.instance = snapshot.id.instance;
        }
        let live = snapshot
            .nodes()
            .iter()
            .map(|node| node.id)
            .collect::<std::collections::HashSet<_>>();
        self.entries
            .retain(|(w, id), _| *w != window || live.contains(id));
        self.next = next;
        for (element, owner) in staged {
            self.entries.insert((window, element), owner);
        }
        Ok(())
    }
    pub fn reference(&self, window: UiWindowId, element: ElementId) -> Option<UiTargetRef> {
        self.entries
            .get(&(window, element))
            .map(|owner| UiTargetRef {
                instance: self.instance,
                window,
                element,
                owner: owner.generation,
            })
    }
    pub fn contains(&self, target: UiTargetRef) -> bool {
        self.reference(target.window, target.element) == Some(target)
    }
    pub fn forget(&mut self, window: UiWindowId) {
        self.entries.retain(|(w, _), _| *w != window);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum UiCommand {
    Activate,
    Focus,
    SetExpanded(bool),
    SetValue(f64),
    ReplaceText { expected: String, text: String },
    SelectAll,
    Undo,
    Redo,
    Reveal,
}

impl UiCommand {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Activate => "activate",
            Self::Focus => "focus",
            Self::SetExpanded(_) => "set_expanded",
            Self::SetValue(_) => "set_value",
            Self::ReplaceText { .. } => "replace_text",
            Self::SelectAll => "select_all",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::Reveal => "reveal",
        }
    }

    /// Shared structural admission; successful admission is not Host authorization.
    pub fn preflight<A>(
        &self,
        target: UiTargetRef,
        snapshot: &UiSnapshot,
        route: &ActionRoute<A>,
        modal: Option<ElementId>,
    ) -> Result<(), &'static str> {
        if target.instance != snapshot.id.instance {
            return Err("foreign_instance");
        }
        if target.window != snapshot.id.window {
            return Err("foreign_window");
        }
        let node = snapshot
            .nodes()
            .iter()
            .find(|node| node.id == target.element)
            .ok_or("target_not_captured")?;
        if route.path_to(target.element).is_none() {
            return Err("disabled_or_removed");
        }
        let scope = FocusScope::new(target.window, route, modal);
        let input = InputTarget {
            window: target.window,
            element: target.element,
        };
        if !scope.contains(input) {
            return Err("modal_blocked");
        }
        match self {
            Self::SetValue(value) => {
                if !value.is_finite() {
                    return Err("invalid_value");
                }
                if node.numeric.is_none()
                    || node.read_only
                    || node.value.redacted
                    || !scope.accepts(input)
                {
                    return Err("not_settable");
                }
                Ok(())
            }
            Self::Activate | Self::SetExpanded(_) if route.action_at(target.element).is_none() => {
                Err("action_unavailable")
            }
            Self::SetExpanded(_) if node.expanded.is_none() => Err("not_expandable"),
            Self::Focus if !scope.accepts(input) => Err("not_focusable"),
            Self::ReplaceText { .. } | Self::SelectAll | Self::Undo | Self::Redo => {
                if node.value.redacted {
                    return Err("sensitive_target");
                }
                if node.composing {
                    return Err("composing");
                }
                if !scope.accepts(input) {
                    return Err("not_focusable");
                }
                if node.read_only && !matches!(self, Self::SelectAll) {
                    return Err("read_only");
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Element, ElementKey, LayoutEngine, Semantic, SemanticRole, UiQueryLimits, UiSnapshotFrame,
        UiSnapshotId, Viewport,
    };

    fn capture(
        instance: u128,
        window: UiWindowId,
        ids: &[u64],
        action: u8,
        revision: u64,
    ) -> (UiSnapshot, ActionRoute<u8>) {
        let root = Element::container(ElementId::new(1), ElementKey::new("root")).children(
            ids.iter().map(|id| {
                Element::container(ElementId::new(*id), ElementKey::new(format!("node.{id}")))
                    .semantic(Semantic::new(SemanticRole::Button, format!("Node {id}")))
            }),
        );
        let route = ActionRoute::build(&root, [(ElementId::new(2), action)]).expect("route");
        let layout = LayoutEngine
            .compute(&root, Viewport::new(100.0, 100.0, 1.0))
            .expect("layout");
        let snapshot = UiSnapshot::capture(
            UiSnapshotFrame {
                id: UiSnapshotId {
                    instance,
                    window,
                    revision,
                },
                root: &root,
                layout: &layout,
                route: &route,
                focus: None,
                modal_root: None,
            },
            UiQueryLimits::default(),
            |_| None,
            |_| false,
        )
        .expect("snapshot");
        (snapshot, route)
    }

    fn baseline() -> UiOwnerRegistry<u8> {
        let mut owners = UiOwnerRegistry::default();
        for window in [UiWindowId(1), UiWindowId(2)] {
            let (snapshot, route) = capture(42, window, &[2, 3], 7, 1);
            owners.update(&snapshot, &route).expect("initial owners");
        }
        owners
    }

    #[test]
    fn owner_capacity_failures_preserve_instance_cursor_and_every_window_reference() {
        let main = UiWindowId(1);
        let other = UiWindowId(2);
        let mut corrupted = Vec::new();
        for (instance, available_cursor) in [(42, u64::MAX - 1), (43, u64::MAX)] {
            let mut owners = baseline();
            let references = [main, other]
                .into_iter()
                .flat_map(|window| (1..=3).map(move |id| (window, ElementId::new(id))))
                .map(|(window, id)| owners.reference(window, id).expect("live reference"))
                .collect::<Vec<_>>();
            owners.next = available_cursor;
            let (changed, route) = capture(instance, main, &[2, 4], 99, 2);
            assert_eq!(
                owners.update(&changed, &route),
                Err("owner_generation_exhausted")
            );
            if owners.instance != 42
                || owners.next != available_cursor
                || owners.entries.len() != references.len()
                || !references.iter().all(|target| owners.contains(*target))
            {
                corrupted.push(instance);
            }
        }
        assert!(
            corrupted.is_empty(),
            "failed owner update must preserve the previous reference set: {corrupted:?}"
        );

        let mut owners = baseline();
        let unchanged = owners
            .reference(other, ElementId::new(2))
            .expect("foreign reference");
        let removed = owners
            .reference(main, ElementId::new(3))
            .expect("removed reference");
        let original = owners
            .reference(main, ElementId::new(2))
            .expect("old action");
        owners.next = u64::MAX - 1;
        let (changed, route) = capture(42, main, &[2, 3], 99, 2);
        owners
            .update(&changed, &route)
            .expect("one remaining generation");
        let replacement = owners
            .reference(main, ElementId::new(2))
            .expect("replacement");
        assert!(!owners.contains(original) && owners.contains(unchanged));
        assert_eq!(replacement.owner, u64::MAX);
        owners
            .update(&changed, &route)
            .expect("unchanged repaint at capacity");
        assert_eq!(owners.reference(main, ElementId::new(2)), Some(replacement));
        let (trimmed, route) = capture(42, main, &[2], 99, 3);
        owners
            .update(&trimmed, &route)
            .expect("removals need no generation");
        assert!(!owners.contains(removed) && owners.contains(replacement));
        assert!(owners.contains(unchanged));
        owners.forget(other);
        assert!(!owners.contains(unchanged) && owners.contains(replacement));
    }
}
