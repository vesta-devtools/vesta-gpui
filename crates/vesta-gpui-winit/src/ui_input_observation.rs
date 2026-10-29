//! Immutable input evidence from the same presented tree, route and layout as a query.
use crate::{UiInputRequest, UiInputStep};
use std::collections::{HashMap, HashSet};
use vesta_gpui::{
    ActionRoute, Element, ElementId, HitTestTree, LayoutTree, Point, UiOwnerRegistry, UiSnapshot,
    UiSnapshotId,
};

pub struct UiInputObservation {
    id: UiSnapshotId,
    context: String,
    owners: HashMap<ElementId, u64>,
    action_owners: HashMap<ElementId, ElementId>,
    focusable: HashSet<ElementId>,
    // Retain the same immutable editor visual already owned by the frame. Equality
    // also detects edits beyond the bounded query text without exporting that text.
    editors: HashMap<ElementId, vesta_gpui::TextInputSnapshot>,
    hits: HitTestTree,
}
impl UiInputObservation {
    pub fn capture<A: Clone + PartialEq>(
        snapshot: &UiSnapshot,
        registry: &UiOwnerRegistry<A>,
        route: &ActionRoute<A>,
        root: &Element,
        layout: &LayoutTree,
        context: String,
    ) -> Self {
        fn editors(
            element: &Element,
            snapshot: &UiSnapshot,
            result: &mut HashMap<ElementId, vesta_gpui::TextInputSnapshot>,
        ) {
            if let vesta_gpui::ElementKind::TextInput {
                snapshot: visual, ..
            } = &element.kind
                && snapshot.nodes().iter().any(|node| {
                    node.id == element.id && !node.value.redacted && !node.name.redacted
                })
            {
                result.insert(element.id, visual.clone());
            }
            for child in &element.children {
                editors(child, snapshot, result);
            }
        }
        let mut editor_states = HashMap::new();
        editors(root, snapshot, &mut editor_states);
        Self {
            id: snapshot.id,
            context,
            owners: snapshot
                .nodes()
                .iter()
                .filter_map(|node| {
                    registry
                        .reference(snapshot.id.window, node.id)
                        .map(|target| (node.id, target.owner))
                })
                .collect(),
            action_owners: snapshot
                .nodes()
                .iter()
                .filter_map(|node| route.target_for_hit(node.id).map(|(id, _)| (node.id, id)))
                .collect(),
            focusable: route.focus_order().iter().copied().collect(),
            editors: editor_states,
            hits: HitTestTree::build(root, layout),
        }
    }
    pub fn validate(
        &self,
        source: &UiSnapshot,
        current: &UiSnapshot,
        now: &Self,
        request: &UiInputRequest,
    ) -> Result<(), &'static str> {
        if self.id != source.id || now.id != current.id || request.revision != source.id.revision {
            return Err("stale_snapshot");
        }
        if self.owners.get(&request.target.element) != Some(&request.target.owner) {
            return Err("stale_input_owner");
        }
        if matches!(
            request.steps.as_slice(),
            [UiInputStep::Window(_) | UiInputStep::Cancel]
        ) {
            return Ok(());
        }
        if self.context != now.context {
            return Err("stale_input_context");
        }
        let old = source
            .nodes()
            .iter()
            .find(|node| node.id == request.target.element)
            .ok_or("target_not_captured")?;
        let new = current
            .nodes()
            .iter()
            .find(|node| node.id == request.target.element)
            .ok_or("target_not_captured")?;
        if self.owners.get(&old.id) != now.owners.get(&old.id)
            || old.role != new.role
            || old.read_only != new.read_only
        {
            return Err("stale_input_target");
        }
        let pointer = request.steps.iter().any(|step| {
            matches!(
                step,
                UiInputStep::Move(_) | UiInputStep::Button { .. } | UiInputStep::Wheel { .. }
            )
        });
        if pointer {
            if source.viewport != current.viewport
                || old.bounds != new.bounds
                || old.visible_bounds != new.visible_bounds
            {
                return Err("stale_input_geometry");
            }
            let bounds = old.bounds.ok_or("target_not_visible")?;
            let visible = old.visible_bounds.ok_or("target_not_visible")?;
            let mut cursor = Point::new(
                visible.x + visible.width / 2.0,
                visible.y + visible.height / 2.0,
            );
            let mut held = false;
            for step in &request.steps {
                match step {
                    UiInputStep::Move(point) => {
                        cursor = Point::new(bounds.x + point.x, bounds.y + point.y);
                        if !held {
                            self.check_hit(now, cursor, old.parent.is_none())?;
                        }
                    }
                    UiInputStep::Button { pressed, .. } => {
                        if *pressed {
                            self.check_hit(now, cursor, old.parent.is_none())?;
                        }
                        held = *pressed;
                    }
                    UiInputStep::Wheel { .. } => {
                        self.check_hit(now, cursor, old.parent.is_none())?
                    }
                    _ => (),
                }
            }
        }
        if request
            .steps
            .iter()
            .any(|step| matches!(step, UiInputStep::Key { .. } | UiInputStep::Text(_)))
        {
            if self.editors.get(&old.id) != now.editors.get(&old.id) {
                return Err("stale_input_edit");
            }
            if self.focusable.contains(&old.id) != now.focusable.contains(&old.id)
                || old.value != new.value
                || old.selection != new.selection
            {
                return Err("stale_input_edit");
            }
            if source.focus != current.focus {
                return Err("stale_input_focus");
            }
            if let Some(focus) = source.focus {
                if self.editors.get(&focus) != now.editors.get(&focus) {
                    return Err("stale_input_edit");
                }
                if self.owners.get(&focus) != now.owners.get(&focus) {
                    return Err("stale_input_focus");
                }
                let a = source.nodes().iter().find(|node| node.id == focus);
                let b = current.nodes().iter().find(|node| node.id == focus);
                if a.map(|node| (&node.value, node.selection, node.composing))
                    != b.map(|node| (&node.value, node.selection, node.composing))
                {
                    return Err("stale_input_edit");
                }
            }
        }
        Ok(())
    }
    fn check_hit(&self, now: &Self, point: Point, window_root: bool) -> Result<(), &'static str> {
        let hits = (self.hits.hit(point), now.hits.hit(point));
        if hits == (None, None) && window_root {
            return Ok(());
        }
        let a = hits.0.ok_or("target_not_hittable")?;
        let b = hits.1.ok_or("target_not_hittable")?;
        if a != b || !self.owners.contains_key(&a) || self.owners.get(&a) != now.owners.get(&a) {
            return Err("stale_input_hit");
        }
        let owner = self.action_owners.get(&a);
        if owner != now.action_owners.get(&a)
            || owner.is_some_and(|id| self.owners.get(id) != now.owners.get(id))
        {
            return Err("stale_input_hit");
        }
        // Text hit positions depend on the complete editor visual, even when the
        // caller addresses a containing root and the bounded query prefix matches.
        if self.editors.get(&a) != now.editors.get(&a)
            || owner.is_some_and(|id| self.editors.get(id) != now.editors.get(id))
        {
            return Err("stale_input_edit");
        }
        Ok(())
    }
}
