//! Immutable, bounded observations of an existing presentation. No dispatch or I/O.
use std::collections::HashMap;
mod changes;
pub use changes::UiSnapshotChanges;

use crate::{
    ActionRoute, Element, ElementId, ElementKind, FocusScope, InputTarget, LayoutTree, Point, Rect,
    SemanticNumeric, SemanticRole, SemanticToggle, UiWindowId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiSnapshotId {
    pub instance: u128,
    pub window: UiWindowId,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct UiQueryLimits {
    pub nodes: usize,
    pub text_bytes: usize,
    pub depth: usize,
}

impl Default for UiQueryLimits {
    fn default() -> Self {
        Self {
            nodes: 4096,
            text_bytes: 64 * 1024,
            depth: 128,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UiSnapshotText {
    pub value: Option<String>,
    pub truncated: bool,
    pub redacted: bool,
}

/// Supplied by the authoritative editor owner while the presentation is captured.
pub struct UiFieldContent<'a> {
    pub text: &'a str,
    pub selection: Option<(usize, usize)>,
    pub read_only: bool,
    pub composing: bool,
    pub sensitive: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiActionAvailability {
    NoRoutedAction,
    Disabled,
    ModalBlocked,
    Offscreen,
    /// A typed Action exists; Host business admission has NOT been evaluated.
    HostUnvalidated,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiNode {
    pub id: ElementId,
    pub parent: Option<ElementId>,
    pub depth: usize,
    pub role: Option<SemanticRole>,
    pub name: UiSnapshotText,
    pub description: UiSnapshotText,
    pub text: UiSnapshotText,
    pub value: UiSnapshotText,
    pub tooltip: UiSnapshotText,
    pub bounds: Option<Rect>,
    pub visible_bounds: Option<Rect>,
    pub selected: Option<bool>,
    pub expanded: Option<bool>,
    pub toggle: Option<SemanticToggle>,
    pub numeric: Option<SemanticNumeric>,
    pub read_only: bool,
    pub busy: bool,
    pub composing: bool,
    pub focused: bool,
    pub selection: Option<(usize, usize)>,
    pub scroll_offset: Option<Point>,
    pub scroll_content: Option<(f32, f32)>,
    pub action: UiActionAvailability,
    /// A reference to a current direct typed action owner, not a callable command ID.
    pub action_owner: Option<ElementId>,
}

pub struct UiSnapshotFrame<'a, A> {
    pub id: UiSnapshotId,
    pub root: &'a Element,
    pub layout: &'a LayoutTree,
    pub route: &'a ActionRoute<A>,
    pub focus: Option<InputTarget>,
    pub modal_root: Option<ElementId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiQueryError {
    InvalidLimits,
    InconsistentPresentation,
    ForeignWindow,
    StaleSnapshot,
    NodeNotCaptured,
    InvalidCursor,
}

#[derive(Clone, Copy, Debug)]
pub struct UiQuery {
    pub snapshot: UiSnapshotId,
    pub root: Option<ElementId>,
    pub after: Option<ElementId>,
    pub limit: usize,
}

#[derive(Debug)]
pub struct UiQueryPage<'a> {
    pub nodes: Vec<&'a UiNode>,
    pub next_after: Option<ElementId>,
}

#[derive(Debug)]
pub struct UiSnapshot {
    pub id: UiSnapshotId,
    pub viewport: crate::Viewport,
    pub focus: Option<ElementId>,
    pub modal_root: Option<ElementId>,
    /// Completeness covers the supplied realized tree, never unrealized model data.
    pub node_limit_reached: bool,
    pub depth_limit_reached: bool,
    pub text_truncated: bool,
    nodes: Vec<UiNode>,
    indices: HashMap<ElementId, usize>,
}

struct Builder<'a, A> {
    frame: UiSnapshotFrame<'a, A>,
    limits: UiQueryLimits,
    remaining_text: usize,
    snapshot: UiSnapshot,
}

impl UiSnapshot {
    pub fn capture<'a, A>(
        frame: UiSnapshotFrame<'_, A>,
        limits: UiQueryLimits,
        mut fields: impl FnMut(ElementId) -> Option<UiFieldContent<'a>>,
        mut sensitive: impl FnMut(ElementId) -> bool,
    ) -> Result<Self, UiQueryError> {
        if limits.nodes == 0
            || limits.nodes > 4096
            || limits.text_bytes > 64 * 1024
            || limits.depth == 0
            || limits.depth > 128
        {
            return Err(UiQueryError::InvalidLimits);
        }
        if frame
            .focus
            .is_some_and(|focus| focus.window != frame.id.window)
        {
            return Err(UiQueryError::ForeignWindow);
        }
        if frame.id.instance == 0
            || frame.id.revision == 0
            || frame.layout.node(frame.root.id).is_none()
            || frame.route.inspection_parent(frame.root.id) != Some(None)
            || frame
                .modal_root
                .is_some_and(|root| frame.layout.node(root).is_none())
        {
            return Err(UiQueryError::InconsistentPresentation);
        }
        let focus = frame
            .focus
            .filter(|focus| {
                FocusScope::new(frame.id.window, frame.route, frame.modal_root).contains(*focus)
            })
            .map(|focus| focus.element);
        let mut builder = Builder {
            snapshot: Self {
                id: frame.id,
                viewport: frame.layout.viewport(),
                focus,
                modal_root: frame.modal_root,
                node_limit_reached: false,
                depth_limit_reached: false,
                text_truncated: false,
                nodes: Vec::new(),
                indices: HashMap::new(),
            },
            remaining_text: limits.text_bytes,
            frame,
            limits,
        };
        builder.visit(
            builder.frame.root,
            None,
            0,
            false,
            false,
            &mut fields,
            &mut sensitive,
        )?;
        Ok(builder.snapshot)
    }

    pub fn nodes(&self) -> &[UiNode] {
        &self.nodes
    }

    pub fn query(&self, query: UiQuery) -> Result<UiQueryPage<'_>, UiQueryError> {
        if query.snapshot.instance != self.id.instance || query.snapshot.window != self.id.window {
            return Err(UiQueryError::ForeignWindow);
        }
        if query.snapshot != self.id {
            return Err(UiQueryError::StaleSnapshot);
        }
        if query.limit == 0 || query.limit > 256 {
            return Err(UiQueryError::InvalidLimits);
        }
        let start = match query.root {
            Some(id) => *self.indices.get(&id).ok_or(UiQueryError::NodeNotCaptured)?,
            None => 0,
        };
        let end = if query.root.is_some() {
            self.nodes[start + 1..]
                .iter()
                .position(|node| node.depth <= self.nodes[start].depth)
                .map_or(self.nodes.len(), |offset| start + 1 + offset)
        } else {
            self.nodes.len()
        };
        let from = match query.after {
            Some(id) => {
                let index = *self.indices.get(&id).ok_or(UiQueryError::InvalidCursor)?;
                if index < start || index >= end {
                    return Err(UiQueryError::InvalidCursor);
                }
                index + 1
            }
            None => start,
        };
        let to = (from + query.limit).min(end);
        Ok(UiQueryPage {
            nodes: self.nodes[from..to].iter().collect(),
            next_after: (to < end).then(|| self.nodes[to - 1].id),
        })
    }
}

impl<A> Builder<'_, A> {
    fn text(&mut self, source: Option<&str>, secret: bool) -> UiSnapshotText {
        if secret {
            return UiSnapshotText {
                redacted: true,
                ..Default::default()
            };
        }
        let Some(source) = source else {
            return UiSnapshotText::default();
        };
        let mut end = source.len().min(self.remaining_text).min(4096);
        while !source.is_char_boundary(end) {
            end -= 1;
        }
        self.remaining_text -= end;
        let truncated = end < source.len();
        self.snapshot.text_truncated |= truncated;
        UiSnapshotText {
            value: Some(source[..end].to_owned()),
            truncated,
            redacted: false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn visit<'a>(
        &mut self,
        element: &Element,
        parent: Option<ElementId>,
        depth: usize,
        blocked: bool,
        secret: bool,
        fields: &mut impl FnMut(ElementId) -> Option<UiFieldContent<'a>>,
        sensitive: &mut impl FnMut(ElementId) -> bool,
    ) -> Result<(), UiQueryError> {
        if self.snapshot.nodes.len() == self.limits.nodes {
            self.snapshot.node_limit_reached = true;
            return Ok(());
        }
        if depth >= self.limits.depth {
            self.snapshot.depth_limit_reached = true;
            return Ok(());
        }
        // Layout identity/order must describe this exact tree, including disabled nodes.
        let layout = self
            .frame
            .layout
            .node(element.id)
            .ok_or(UiQueryError::InconsistentPresentation)?;
        if self.snapshot.indices.contains_key(&element.id)
            || self.frame.route.inspection_parent(element.id) != Some(parent)
        {
            return Err(UiQueryError::InconsistentPresentation);
        }
        let semantic = element.semantic.as_ref();
        let field = fields(element.id);
        let secret = secret
            || sensitive(element.id)
            || semantic.is_some_and(|semantic| semantic.role == SemanticRole::PasswordInput)
            || field.as_ref().is_some_and(|field| field.sensitive);
        let blocked = blocked || element.style.action_boundary;
        let modal_blocked = self
            .frame
            .modal_root
            .is_some_and(|root| !self.frame.route.is_descendant_of(element.id, root));
        let visible = layout
            .rect
            .intersection(layout.inherited_clip)
            .and_then(|rect| {
                rect.intersection(Rect::new(
                    0.0,
                    0.0,
                    self.frame.layout.viewport().width,
                    self.frame.layout.viewport().height,
                ))
            })
            .filter(|rect| rect.width > 0.0 && rect.height > 0.0);
        let action_owner = self.frame.route.action_at(element.id).map(|_| element.id);
        let action = if blocked {
            UiActionAvailability::Disabled
        } else if modal_blocked {
            UiActionAvailability::ModalBlocked
        } else if visible.is_none() {
            UiActionAvailability::Offscreen
        } else if action_owner.is_some() {
            UiActionAvailability::HostUnvalidated
        } else {
            UiActionAvailability::NoRoutedAction
        };
        let value = self.text(
            field
                .as_ref()
                .map(|field| field.text)
                .or_else(|| semantic.and_then(|s| s.value.as_deref())),
            secret,
        );
        let selection =
            field
                .as_ref()
                .and_then(|field| field.selection)
                .filter(|(anchor, focus)| {
                    !secret
                        && !value.truncated
                        && field.as_ref().is_some_and(|field| {
                            !field.composing
                                && field.text.is_char_boundary(*anchor)
                                && field.text.is_char_boundary(*focus)
                        })
                });
        let node = UiNode {
            id: element.id,
            parent,
            depth,
            role: semantic.map(|s| s.role),
            name: self.text(semantic.map(|s| s.label.as_str()), secret),
            description: self.text(semantic.and_then(|s| s.description.as_deref()), secret),
            text: self.text(
                match &element.kind {
                    ElementKind::Text(text) => Some(text),
                    _ => None,
                },
                secret,
            ),
            tooltip: self.text(element.tooltip.as_deref(), secret),
            value,
            bounds: Some(layout.rect),
            visible_bounds: visible,
            selected: semantic.and_then(|s| s.selected),
            expanded: semantic.and_then(|s| s.expanded),
            toggle: semantic.and_then(|s| s.toggle),
            numeric: (!secret)
                .then(|| semantic.and_then(|s| s.numeric))
                .flatten(),
            read_only: field
                .as_ref()
                .map_or_else(|| semantic.is_some_and(|s| s.read_only), |f| f.read_only),
            busy: semantic.is_some_and(|s| s.busy),
            composing: field.as_ref().is_some_and(|f| f.composing),
            focused: self.snapshot.focus == Some(element.id),
            selection,
            scroll_offset: match element.kind {
                ElementKind::ScrollBox(offset) => Some(offset),
                _ => None,
            },
            scroll_content: layout.scroll_content,
            action,
            action_owner,
        };
        self.snapshot
            .indices
            .insert(node.id, self.snapshot.nodes.len());
        self.snapshot.nodes.push(node);
        for child in &element.children {
            self.visit(
                child,
                Some(element.id),
                depth + 1,
                blocked,
                secret,
                fields,
                sensitive,
            )?;
            if self.snapshot.node_limit_reached || self.snapshot.depth_limit_reached {
                break;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
