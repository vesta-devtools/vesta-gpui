//! Tree visibility and row projection without owning document hierarchy.

use std::collections::BTreeSet;

use vesta_gpui::{
    Align, Axis, Dimension, Element, Insets, KeyStroke, Point, Semantic, SemanticRole, Style,
};

use crate::{
    ControlState, ICON_LINE_SIZE, ICON_LINE_STROKE, VirtualListError, VirtualListIds,
    VirtualListRange, Widget, WidgetContent, WidgetIdentity, WidgetTheme,
    disclosure_chevron_points, linear_collection_index, virtual_list_with_items,
};

/// One host-supplied preorder row. Identity is independent of row position and label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeViewEntry<K> {
    pub id: K,
    pub label: String,
    pub depth: u8,
    pub has_children: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeViewError {
    DuplicateId,
    InvalidDepth,
    LeafHasDescendants,
}

impl std::fmt::Display for TreeViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::DuplicateId => "duplicate tree item identity",
            Self::InvalidDepth => "invalid tree item depth",
            Self::LeafHasDescendants => "tree leaf has descendants",
        })
    }
}
impl std::error::Error for TreeViewError {}

/// UI expansion state; the host remains authoritative for the tree entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeExpansionState<K: Ord> {
    expanded: BTreeSet<K>,
}

impl<K: Ord> Default for TreeExpansionState<K> {
    fn default() -> Self {
        Self {
            expanded: BTreeSet::new(),
        }
    }
}

impl<K: Ord + Clone> TreeExpansionState<K> {
    #[must_use]
    pub fn contains(&self, id: &K) -> bool {
        self.expanded.contains(id)
    }

    pub fn toggle(&mut self, id: K) -> bool {
        if self.expanded.remove(&id) {
            false
        } else {
            self.expanded.insert(id);
            true
        }
    }

    /// Remove expanded identities that the host has deleted or turned into leaves.
    pub fn reconcile(&mut self, entries: &[TreeViewEntry<K>]) -> bool {
        let live = entries
            .iter()
            .filter(|entry| entry.has_children)
            .map(|entry| &entry.id)
            .collect::<BTreeSet<_>>();
        let before = self.expanded.len();
        self.expanded.retain(|id| live.contains(id));
        self.expanded.len() != before
    }
}

/// Visible rows after validating a flat preorder tree and applying expansion.
#[derive(Clone, Debug)]
pub struct VisibleTree<'a, K> {
    entries: &'a [TreeViewEntry<K>],
    indices: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeKeyAction<K> {
    Focus(K),
    SetExpanded { id: K, expanded: bool },
}

impl<'a, K: Ord + Clone> VisibleTree<'a, K> {
    pub fn build(
        entries: &'a [TreeViewEntry<K>],
        expansion: &TreeExpansionState<K>,
    ) -> Result<Self, TreeViewError> {
        let mut seen = BTreeSet::new();
        let mut ancestors = Vec::<(bool, bool)>::new();
        let mut indices = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            if !seen.insert(entry.id.clone()) {
                return Err(TreeViewError::DuplicateId);
            }
            let depth = usize::from(entry.depth);
            if depth > ancestors.len() {
                return Err(TreeViewError::InvalidDepth);
            }
            ancestors.truncate(depth);
            let visible = if let Some((parent_visible, parent_has_children)) = ancestors.last() {
                if !parent_has_children {
                    return Err(TreeViewError::LeafHasDescendants);
                }
                *parent_visible
            } else {
                true
            };
            if visible {
                indices.push(index);
            }
            ancestors.push((visible && expansion.contains(&entry.id), entry.has_children));
        }
        Ok(Self { entries, indices })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.indices.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    #[must_use]
    pub fn entry(&self, visible_index: usize) -> Option<&'a TreeViewEntry<K>> {
        self.indices
            .get(visible_index)
            .map(|index| &self.entries[*index])
    }

    #[must_use]
    pub fn index_of(&self, id: &K) -> Option<usize> {
        self.indices
            .iter()
            .position(|index| &self.entries[*index].id == id)
    }

    #[must_use]
    pub fn parent_of(&self, id: &K) -> Option<&'a TreeViewEntry<K>> {
        let source = self.entries.iter().position(|entry| &entry.id == id)?;
        let depth = self.entries[source].depth;
        if depth == 0 {
            return None;
        }
        self.entries[..source]
            .iter()
            .rev()
            .find(|entry| entry.depth == depth - 1)
    }

    /// Tree navigation uses the current validated visible order and stable identities.
    /// The host applies expansion and selection intents to its retained view state.
    #[must_use]
    pub fn key_action(
        &self,
        expansion: &TreeExpansionState<K>,
        id: &K,
        key: KeyStroke,
        page_size: usize,
    ) -> Option<TreeKeyAction<K>> {
        let index = self.index_of(id)?;
        let entry = self.entry(index)?;
        if entry.has_children {
            if key == KeyStroke::ArrowRight && !expansion.contains(id) {
                return Some(TreeKeyAction::SetExpanded {
                    id: id.clone(),
                    expanded: true,
                });
            }
            if key == KeyStroke::ArrowLeft && expansion.contains(id) {
                return Some(TreeKeyAction::SetExpanded {
                    id: id.clone(),
                    expanded: false,
                });
            }
        }
        let next = match key {
            KeyStroke::ArrowRight => self
                .entry(index.saturating_add(1))
                .filter(|child| child.depth > entry.depth)
                .unwrap_or(entry),
            KeyStroke::ArrowLeft => self.parent_of(id).unwrap_or(entry),
            _ => self.entry(linear_collection_index(index, self.len(), page_size, key)?)?,
        };
        Some(TreeKeyAction::Focus(next.id.clone()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeViewRowIds {
    pub root: WidgetIdentity,
    pub disclosure: WidgetIdentity,
    pub label: WidgetIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeRowContentIds {
    pub root: WidgetIdentity,
    pub disclosure: WidgetIdentity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeViewRowState {
    pub depth: u8,
    pub has_children: bool,
    pub expanded: bool,
    pub selected: bool,
    pub control: ControlState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeViewRowActions<A> {
    pub select: A,
    pub toggle: A,
}

/// Build only visible tree entries and optionally retain one offscreen focused entry.
/// Collapsed descendants never reach the factory. Row identities remain host supplied.
pub fn tree_view_windowed<K: Ord + Clone, A>(
    ids: VirtualListIds,
    width: Dimension,
    tree: &VisibleTree<'_, K>,
    range: VirtualListRange,
    retained_index: Option<usize>,
    mut row: impl FnMut(&TreeViewEntry<K>) -> Widget<A>,
) -> Result<Widget<A>, VirtualListError> {
    let mut widget =
        virtual_list_with_items(ids, width, &tree.indices, range, retained_index, |index| {
            row(&tree.entries[*index])
        })?;
    widget.element.semantic = Some(Semantic::new(SemanticRole::Tree, "Tree"));
    Ok(widget)
}

/// Construct arbitrary tree content from typed host data while sharing disclosure and selection.
#[must_use]
pub fn tree_view_row_from<T, A>(
    ids: TreeRowContentIds,
    item: &T,
    state: TreeViewRowState,
    actions: TreeViewRowActions<A>,
    theme: WidgetTheme,
    build: impl FnOnce(&T) -> WidgetContent<A>,
) -> Widget<A> {
    tree_view_row_with_content(ids, build(item), state, actions, theme)
}

/// One selectable tree row with an independent disclosure action.
#[must_use]
pub fn tree_view_row<A>(
    ids: TreeViewRowIds,
    label: impl Into<String>,
    state: TreeViewRowState,
    actions: TreeViewRowActions<A>,
    theme: WidgetTheme,
) -> Widget<A> {
    let label = label.into();
    let content = Widget::from_element(
        Element::text(ids.label.element, ids.label.key, &label).styled(
            Style::default()
                .size(Dimension::Auto, Dimension::Percent(1.0))
                .text(
                    theme.typography.font_size,
                    if state.control == ControlState::Disabled {
                        theme.palette.text_disabled
                    } else {
                        theme.palette.text
                    },
                )
                .pointer_events(false),
        ),
    );
    tree_view_row_with_content(
        TreeRowContentIds {
            root: ids.root,
            disclosure: ids.disclosure,
        },
        WidgetContent::new(label, content),
        state,
        actions,
        theme,
    )
}

#[must_use]
pub fn tree_view_row_with_content<A>(
    ids: TreeRowContentIds,
    mut content: WidgetContent<A>,
    state: TreeViewRowState,
    actions: TreeViewRowActions<A>,
    theme: WidgetTheme,
) -> Widget<A> {
    content.widget.element.style.min_width = Dimension::Px(0.0);
    content.widget.element.style.flex_grow = 1.0;
    content.widget.element.style.flex_shrink = 1.0;
    let label = content.label;
    let enabled = state.control != ControlState::Disabled;
    let mut semantic = Semantic::new(SemanticRole::TreeItem, &label).selected(state.selected);
    if state.has_children {
        semantic = semantic.expanded(state.expanded);
    }
    let background = theme
        .palette
        .selectable_background(state.selected, state.control);
    let mut disclosure_style = Style::default()
        .size(Dimension::Px(16.0), Dimension::Percent(1.0))
        .focusable(state.has_children && enabled)
        .pointer_events(state.has_children && enabled);
    if state.has_children {
        let inset_y = ((theme.metrics.control_height - ICON_LINE_SIZE) * 0.5).max(0.0);
        let points = disclosure_chevron_points(state.expanded)
            .map(|point| Point::new(point.x + 1.0, point.y + inset_y));
        disclosure_style = disclosure_style
            .polyline(points.to_vec(), ICON_LINE_STROKE)
            .background(if enabled {
                theme.palette.text_muted
            } else {
                theme.palette.text_disabled
            });
    }
    let mut disclosure =
        Element::container(ids.disclosure.element, ids.disclosure.key).styled(disclosure_style);
    if state.has_children {
        disclosure = disclosure.semantic(Semantic::new(
            SemanticRole::Button,
            if state.expanded { "Collapse" } else { "Expand" },
        ));
    }
    let element = Element::container(ids.root.element, ids.root.key)
        .semantic(semantic)
        .styled(
            Style::default()
                .size(
                    Dimension::Px(230.0),
                    Dimension::Px(theme.metrics.control_height),
                )
                .axis(Axis::Row)
                .align(Align::Center)
                .gap(6.0)
                .padding(Insets {
                    left: 8.0 + f32::from(state.depth) * 14.0,
                    right: 8.0,
                    top: 0.0,
                    bottom: 0.0,
                })
                .background(background)
                .radius(theme.metrics.radius)
                .pointer_events(enabled)
                .focusable(enabled)
                .action_boundary(!enabled),
        )
        .child(disclosure);
    let mut routed_actions = Vec::new();
    if enabled {
        routed_actions.push((ids.root.element, actions.select));
        if state.has_children {
            routed_actions.push((ids.disclosure.element, actions.toggle));
        }
    }
    Widget {
        element,
        actions: routed_actions,
    }
    .child(content.widget)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vesta_gpui::{ActionRoute, ElementId};

    #[test]
    fn visible_tree_keeps_identity_and_rejects_malformed_preorder() {
        let rows = [
            TreeViewEntry {
                id: 10,
                label: "Root".into(),
                depth: 0,
                has_children: true,
            },
            TreeViewEntry {
                id: 11,
                label: "Child".into(),
                depth: 1,
                has_children: false,
            },
            TreeViewEntry {
                id: 20,
                label: "Other".into(),
                depth: 0,
                has_children: false,
            },
        ];
        let mut expansion = TreeExpansionState::default();
        let collapsed = VisibleTree::build(&rows, &expansion).expect("collapsed tree");
        assert_eq!(collapsed.len(), 2);
        assert!(collapsed.index_of(&11).is_none());
        let mut built = Vec::new();
        let widget = tree_view_windowed(
            VirtualListIds {
                root: WidgetIdentity::new(ElementId::new(1), "tree"),
                content: WidgetIdentity::new(ElementId::new(2), "tree.content"),
            },
            Dimension::Px(240.0),
            &collapsed,
            VirtualListRange::compute(collapsed.len(), 28.0, 140.0, 0.0, 0).expect("tree range"),
            None,
            |entry| {
                built.push(entry.id);
                Widget::from_element(Element::text(
                    ElementId::new(100 + entry.id as u64),
                    vesta_gpui::ElementKey::new(format!("entry.{}", entry.id)),
                    &entry.label,
                ))
                .on_click(entry.id)
            },
        )
        .expect("typed tree projection");
        assert_eq!(
            built,
            vec![10, 20],
            "collapsed descendants must never reach the factory"
        );
        assert_eq!(
            widget
                .element
                .semantic
                .as_ref()
                .expect("tree semantic")
                .role,
            SemanticRole::Tree
        );
        let route = ActionRoute::build(&widget.element, widget.actions).expect("tree route");
        assert_eq!(route.action_for_hit(ElementId::new(120)), Some(&20));
        expansion.toggle(10);
        let expanded = VisibleTree::build(&rows, &expansion).expect("expanded tree");
        assert_eq!(expanded.len(), 3);
        assert_eq!(expanded.parent_of(&11).map(|parent| parent.id), Some(10));
        assert_eq!(
            expanded.key_action(&expansion, &10, KeyStroke::ArrowRight, 5),
            Some(TreeKeyAction::Focus(11))
        );
        assert_eq!(
            expanded.key_action(&expansion, &11, KeyStroke::ArrowLeft, 5),
            Some(TreeKeyAction::Focus(10))
        );
        assert_eq!(
            expanded.key_action(&expansion, &10, KeyStroke::ArrowLeft, 5),
            Some(TreeKeyAction::SetExpanded {
                id: 10,
                expanded: false
            })
        );
        assert_eq!(
            expanded.key_action(&expansion, &11, KeyStroke::PageDown, usize::MAX),
            Some(TreeKeyAction::Focus(20))
        );
        assert_eq!(
            expanded.key_action(&expansion, &99, KeyStroke::Enter, 5),
            None
        );
        assert_eq!(
            VisibleTree::build(&[rows[0].clone(), rows[0].clone()], &expansion).err(),
            Some(TreeViewError::DuplicateId)
        );
        assert_eq!(
            VisibleTree::build(&[rows[1].clone()], &expansion).err(),
            Some(TreeViewError::InvalidDepth)
        );
        assert_eq!(
            VisibleTree::build(&[rows[2].clone(), rows[1].clone()], &expansion).err(),
            Some(TreeViewError::LeafHasDescendants)
        );
        let mut refreshed_root = rows[0].clone();
        refreshed_root.has_children = false;
        assert!(expansion.reconcile(&[refreshed_root]));
        assert!(!expansion.contains(&10));
    }
}
