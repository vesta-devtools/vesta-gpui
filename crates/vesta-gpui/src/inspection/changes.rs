//! Differences between two observations of the same realized presentation tree.
use super::*;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiSnapshotChanges {
    pub added: Vec<ElementId>,
    pub changed: Vec<ElementId>,
    pub removed: Vec<ElementId>,
    pub metadata_changed: bool,
    pub complete: bool,
}
impl UiSnapshot {
    pub fn changes_since(&self, previous: &UiSnapshot) -> Result<UiSnapshotChanges, UiQueryError> {
        if self.id.instance != previous.id.instance || self.id.window != previous.id.window {
            return Err(UiQueryError::ForeignWindow);
        }
        if self.id.revision < previous.id.revision {
            return Err(UiQueryError::StaleSnapshot);
        }
        let structure_complete = !self.node_limit_reached
            && !self.depth_limit_reached
            && !previous.node_limit_reached
            && !previous.depth_limit_reached;
        let mut result = UiSnapshotChanges {
            added: Vec::new(),
            changed: Vec::new(),
            removed: Vec::new(),
            metadata_changed: self.viewport != previous.viewport
                || self.focus != previous.focus
                || self.modal_root != previous.modal_root,
            complete: structure_complete && !self.text_truncated && !previous.text_truncated,
        };
        for node in &self.nodes {
            match previous.indices.get(&node.id) {
                Some(index) if previous.nodes[*index] != *node => result.changed.push(node.id),
                None => result.added.push(node.id),
                _ => {}
            }
        }
        if structure_complete {
            result.removed = previous
                .nodes
                .iter()
                .filter(|node| !self.indices.contains_key(&node.id))
                .map(|node| node.id)
                .collect();
        }
        Ok(result)
    }
}
