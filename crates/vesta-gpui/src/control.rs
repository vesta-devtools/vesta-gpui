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
    pub fn update(
        &mut self,
        snapshot: &UiSnapshot,
        route: &ActionRoute<A>,
    ) -> Result<(), &'static str> {
        if self.instance != snapshot.id.instance {
            self.entries.clear();
            self.instance = snapshot.id.instance;
        }
        let window = snapshot.id.window;
        let live = snapshot
            .nodes()
            .iter()
            .map(|node| node.id)
            .collect::<std::collections::HashSet<_>>();
        self.entries
            .retain(|(w, id), _| *w != window || live.contains(id));
        for node in snapshot.nodes() {
            let action = route.action_at(node.id).cloned();
            let unchanged = self.entries.get(&(window, node.id)).is_some_and(|old| {
                old.action == action
                    && old.role == node.role
                    && old.name == node.name
                    && old.parent == node.parent
            });
            if !unchanged {
                self.next = self
                    .next
                    .checked_add(1)
                    .ok_or("owner_generation_exhausted")?;
                self.entries.insert(
                    (window, node.id),
                    Owner {
                        generation: self.next,
                        action,
                        role: node.role,
                        name: node.name.clone(),
                        parent: node.parent,
                    },
                );
            }
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
