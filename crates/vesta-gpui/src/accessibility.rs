//! Backend-neutral control semantics projected to AccessKit by a host.

use std::collections::{HashMap, HashSet};

use accesskit::{Action, Node, NodeId, Role, Toggled, Tree, TreeUpdate};

use crate::{
    Element, ElementId, LayoutTree, Point, TextInputContext, TextInputSelection, TextInputState,
};

mod read_only;
pub use read_only::{
    ReadOnlyDocument, ReadOnlyDocumentSelection, ReadOnlyText, ReadOnlyTextPosition,
    ReadOnlyTextProjectionError,
};

const PLATFORM_ACTIONS: [Action; 4] = [
    Action::Focus,
    Action::Click,
    Action::SetValue,
    Action::SetTextSelection,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticRole {
    Label,
    Link,
    Button,
    TextInput,
    PasswordInput,
    MultilineTextInput,
    SearchInput,
    CheckBox,
    RadioButton,
    Switch,
    ComboBox,
    Slider,
    Splitter,
    SpinButton,
    Tab,
    List,
    ListItem,
    ListBox,
    ListBoxOption,
    Table,
    Row,
    Cell,
    ColumnHeader,
    Dialog,
    Tree,
    TreeItem,
    Menu,
    MenuItem,
    MenuItemCheckBox,
    ProgressIndicator,
    Status,
    Group,
    Document,
    ScrollView,
}

impl SemanticRole {
    fn accesskit(self) -> Role {
        match self {
            Self::Label => Role::Label,
            Self::Link => Role::Link,
            Self::Button => Role::Button,
            Self::TextInput => Role::TextInput,
            Self::PasswordInput => Role::PasswordInput,
            Self::MultilineTextInput => Role::MultilineTextInput,
            Self::SearchInput => Role::SearchInput,
            Self::CheckBox => Role::CheckBox,
            Self::RadioButton => Role::RadioButton,
            Self::Switch => Role::Switch,
            Self::ComboBox => Role::ComboBox,
            Self::Slider => Role::Slider,
            Self::Splitter => Role::Splitter,
            Self::SpinButton => Role::SpinButton,
            Self::Tab => Role::Tab,
            Self::List => Role::List,
            Self::ListItem => Role::ListItem,
            Self::ListBox => Role::ListBox,
            Self::ListBoxOption => Role::ListBoxOption,
            Self::Table => Role::Table,
            Self::Row => Role::Row,
            Self::Cell => Role::Cell,
            Self::ColumnHeader => Role::ColumnHeader,
            Self::Dialog => Role::Dialog,
            Self::Tree => Role::Tree,
            Self::TreeItem => Role::TreeItem,
            Self::Menu => Role::Menu,
            Self::MenuItem => Role::MenuItem,
            Self::MenuItemCheckBox => Role::MenuItemCheckBox,
            Self::ProgressIndicator => Role::ProgressIndicator,
            Self::Status => Role::Status,
            Self::Group => Role::Group,
            Self::Document => Role::Document,
            Self::ScrollView => Role::ScrollView,
        }
    }

    fn is_text_input(self) -> bool {
        matches!(
            self,
            Self::TextInput | Self::MultilineTextInput | Self::SearchInput
        )
    }

    fn is_clickable(self) -> bool {
        !matches!(
            self,
            Self::Label
                | Self::TextInput
                | Self::MultilineTextInput
                | Self::SearchInput
                | Self::Slider
                | Self::Splitter
                | Self::SpinButton
                | Self::ProgressIndicator
                | Self::Status
                | Self::List
                | Self::ListBox
                | Self::Table
                | Self::Cell
                | Self::Dialog
                | Self::Menu
                | Self::Tree
                | Self::Group
                | Self::Document
                | Self::ScrollView
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticToggle {
    Off,
    On,
    Mixed,
}

/// Semantic state is supplied by widgets, not inferred from their child text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SemanticNumeric {
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub step: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Semantic {
    pub role: SemanticRole,
    pub label: String,
    pub value: Option<String>,
    pub description: Option<String>,
    pub toggle: Option<SemanticToggle>,
    pub selected: Option<bool>,
    pub expanded: Option<bool>,
    pub numeric: Option<SemanticNumeric>,
    pub read_only: bool,
    /// The host is working on this control or status; separate from enablement.
    pub busy: bool,
    /// Explicit semantic activation independent of rectangular pointer hit testing.
    pub activation: bool,
}

impl Semantic {
    #[must_use]
    pub fn new(role: SemanticRole, label: impl Into<String>) -> Self {
        Self {
            role,
            label: label.into(),
            value: None,
            description: None,
            toggle: None,
            selected: None,
            expanded: None,
            numeric: None,
            read_only: false,
            busy: false,
            activation: false,
        }
    }

    #[must_use]
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    #[must_use]
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    #[must_use]
    pub fn activatable(mut self, activation: bool) -> Self {
        self.activation = activation;
        self
    }

    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    #[must_use]
    pub fn checked(mut self, value: bool) -> Self {
        self.toggle = Some(if value {
            SemanticToggle::On
        } else {
            SemanticToggle::Off
        });
        self
    }

    #[must_use]
    pub fn mixed(mut self) -> Self {
        self.toggle = Some(SemanticToggle::Mixed);
        self
    }

    #[must_use]
    pub fn selected(mut self, value: bool) -> Self {
        self.selected = Some(value);
        self
    }

    #[must_use]
    pub fn expanded(mut self, value: bool) -> Self {
        self.expanded = Some(value);
        self
    }

    #[must_use]
    pub fn numeric(mut self, value: f64, min: f64, max: f64, step: Option<f64>) -> Self {
        if value.is_finite() && min.is_finite() && max.is_finite() && min <= max {
            self.numeric = Some(SemanticNumeric {
                value: value.clamp(min, max),
                min,
                max,
                step: step.filter(|step| step.is_finite() && *step > 0.0),
            });
        }
        self
    }
}

/// Retains platform node identity across declarative rebuilds. One instance belongs to one window.
pub struct AccessibilityTree {
    ids: HashMap<ElementId, NodeId>,
    live: HashSet<ElementId>,
    focusable: HashSet<ElementId>,
    published_actions: HashMap<ElementId, [bool; PLATFORM_ACTIONS.len()]>,
    next_id: u64,
    node_scope: crate::ElementScope,
    text_projections: HashMap<ElementId, parley::layout::LayoutAccessibility>,
    editable_text: HashSet<ElementId>,
    read_only_documents: HashMap<ElementId, read_only::DocumentProjection>,
    active_read_only_documents: HashSet<ElementId>,
}

impl std::fmt::Debug for AccessibilityTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccessibilityTree")
            .field("ids", &self.ids)
            .field("live", &self.live)
            .field("focusable", &self.focusable)
            .field("next_id", &self.next_id)
            .field("text_projection_count", &self.text_projections.len())
            .field("editable_text", &self.editable_text)
            .finish()
    }
}

impl Default for AccessibilityTree {
    fn default() -> Self {
        Self {
            ids: HashMap::new(),
            live: HashSet::new(),
            focusable: HashSet::new(),
            published_actions: HashMap::new(),
            next_id: 1,
            node_scope: crate::ElementIdAllocator
                .allocate_scope()
                .expect("accessibility scope exhausted"),
            text_projections: HashMap::new(),
            editable_text: HashSet::new(),
            read_only_documents: HashMap::new(),
            active_read_only_documents: HashSet::new(),
        }
    }
}

impl AccessibilityTree {
    fn remember_actions(&mut self, element: ElementId, node: &Node) {
        self.published_actions.insert(
            element,
            PLATFORM_ACTIONS.map(|action| node.supports_action(action)),
        );
    }

    pub(crate) fn supports_action(&self, element: ElementId, action: Action) -> bool {
        PLATFORM_ACTIONS
            .iter()
            .position(|candidate| *candidate == action)
            .and_then(|index| {
                self.published_actions
                    .get(&element)
                    .map(|actions| actions[index])
            })
            .unwrap_or(false)
    }

    fn allocate(&mut self) -> NodeId {
        let local = u32::try_from(self.next_id).expect("accessibility id exhausted");
        let id = NodeId(self.node_scope.element(local).get());
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("accessibility id exhausted");
        id
    }

    fn id(&mut self, element: ElementId) -> NodeId {
        if let Some(id) = self.ids.get(&element) {
            *id
        } else {
            let id = self.allocate();
            self.ids.insert(element, id);
            id
        }
    }

    #[must_use]
    pub fn element_for_node(&self, node: NodeId) -> Option<ElementId> {
        self.ids.iter().find_map(|(element, id)| {
            (*id == node && self.live.contains(element)).then_some(*element)
        })
    }

    #[must_use]
    pub fn node_for_element(&self, element: ElementId) -> Option<NodeId> {
        self.live
            .contains(&element)
            .then(|| self.ids.get(&element).copied())
            .flatten()
    }

    /// Only nodes with explicit semantics are published. Anonymous layout/paint
    /// containers are flattened while retaining the semantic hierarchy.
    pub fn build(
        &mut self,
        root: &Element,
        layout: &LayoutTree,
        focus: Option<ElementId>,
        title: &str,
    ) -> TreeUpdate {
        let _trace = crate::trace::span("gpui.accessibility", "accessibility.build");
        self.live.clear();
        self.focusable.clear();
        self.published_actions.clear();
        self.editable_text.clear();
        self.active_read_only_documents.clear();
        let root_id = NodeId(0);
        let mut update = TreeUpdate {
            nodes: Vec::new(),
            tree: Some(Tree::new(root_id)),
            focus: root_id,
        };
        let mut window = Node::new(Role::Window);
        window.set_label(title);
        window.set_transform(accesskit::Affine::scale(f64::from(
            layout.viewport().scale_factor,
        )));
        for child in self.collect(root, layout, &mut update, false, focus, false) {
            window.push_child(child);
        }
        if let Some(focus) = focus
            .filter(|element| self.focusable.contains(element))
            .and_then(|element| self.node_for_element(element))
        {
            update.focus = focus;
        }
        update.nodes.push((root_id, window));
        self.text_projections
            .retain(|element, _| self.live.contains(element));
        self.read_only_documents
            .retain(|root, _| self.live.contains(root));
        update
    }

    fn collect(
        &mut self,
        element: &Element,
        layout: &LayoutTree,
        update: &mut TreeUpdate,
        disabled_ancestor: bool,
        focus: Option<ElementId>,
        in_document: bool,
    ) -> Vec<NodeId> {
        let disabled = disabled_ancestor || element.style.action_boundary;
        let is_document = element
            .semantic
            .as_ref()
            .is_some_and(|semantic| semantic.role == SemanticRole::Document);
        if is_document
            && !layout.node(element.id).is_some_and(|geometry| {
                geometry
                    .rect
                    .intersection(geometry.inherited_clip)
                    .is_some()
                    || focus == Some(element.id)
            })
        {
            return Vec::new();
        }
        let children = element
            .children
            .iter()
            .flat_map(|child| {
                self.collect(
                    child,
                    layout,
                    update,
                    disabled,
                    focus,
                    in_document || is_document,
                )
            })
            .collect::<Vec<_>>();
        let (Some(semantic), Some(geometry)) = (&element.semantic, layout.node(element.id)) else {
            return children;
        };
        let Some(visible) = geometry
            .rect
            .intersection(geometry.inherited_clip)
            .or_else(|| (in_document || focus == Some(element.id)).then_some(geometry.rect))
        else {
            return children;
        };
        let id = self.id(element.id);
        self.live.insert(element.id);
        let mut node = Node::new(semantic.role.accesskit());
        if semantic.role == SemanticRole::Label {
            node.set_value(semantic.label.as_str());
        } else {
            node.set_label(semantic.label.as_str());
        }
        node.set_bounds(accesskit::Rect {
            x0: f64::from(visible.x),
            y0: f64::from(visible.y),
            x1: f64::from(visible.right()),
            y1: f64::from(visible.bottom()),
        });
        if let Some(value) = &semantic.value {
            node.set_value(value.as_str());
        }
        if let Some(description) = semantic.description.as_ref().or(element.tooltip.as_ref()) {
            node.set_description(description.as_str());
        }
        if let Some(toggle) = semantic.toggle {
            node.set_toggled(match toggle {
                SemanticToggle::Off => Toggled::False,
                SemanticToggle::On => Toggled::True,
                SemanticToggle::Mixed => Toggled::Mixed,
            });
        }
        if let Some(selected) = semantic.selected {
            node.set_selected(selected);
        }
        if let Some(expanded) = semantic.expanded {
            node.set_expanded(expanded);
        }
        if let Some(numeric) = semantic.numeric {
            node.set_numeric_value(numeric.value);
            node.set_min_numeric_value(numeric.min);
            node.set_max_numeric_value(numeric.max);
            if let Some(step) = numeric.step {
                node.set_numeric_value_step(step);
            }
        }
        if disabled {
            node.set_disabled();
        } else if element.style.focusable {
            node.add_action(Action::Focus);
            self.focusable.insert(element.id);
        }
        if semantic.read_only {
            node.set_read_only();
        }
        if semantic.busy {
            node.set_busy();
        }
        if semantic.role == SemanticRole::Status {
            node.set_live(accesskit::Live::Polite);
        }
        if !disabled {
            if semantic.role == SemanticRole::PasswordInput && !semantic.read_only {
                node.add_action(Action::SetValue);
            }
            if semantic.role.is_text_input() {
                if !semantic.read_only {
                    node.add_action(Action::SetValue);
                }
                node.add_action(Action::SetTextSelection);
            } else if matches!(
                semantic.role,
                SemanticRole::Slider | SemanticRole::SpinButton
            ) {
                if !semantic.read_only {
                    node.add_action(Action::SetValue);
                }
            } else if semantic.role.is_clickable()
                && (element.style.pointer_events || semantic.activation)
            {
                node.add_action(Action::Click);
            }
        }
        node.set_children(children);
        self.remember_actions(element.id, &node);
        update.nodes.push((id, node));
        vec![id]
    }

    /// Add the editor's actual text runs and selection to a text-input node.
    /// Call after `build`, before publishing the update.
    pub fn enrich_text_input(
        &mut self,
        update: &mut TreeUpdate,
        element: ElementId,
        input: &mut TextInputState,
        context: &mut TextInputContext,
        origin: Point,
    ) {
        let Some(id) = self.node_for_element(element) else {
            return;
        };
        let Some(index) = update
            .nodes
            .iter()
            .position(|(candidate, _)| *candidate == id)
        else {
            return;
        };
        let (_, mut node) = update.nodes.swap_remove(index);
        let mut projection = self.text_projections.remove(&element).unwrap_or_default();
        input.accessibility_projection(
            context,
            &mut projection,
            update,
            &mut node,
            // Parley retains IDs for existing run paths and only calls this for new runs.
            || self.allocate(),
            origin,
        );
        self.text_projections.insert(element, projection);
        if !node.is_disabled() && !input.is_password() {
            self.editable_text.insert(element);
        }
        self.remember_actions(element, &node);
        update.nodes.push((id, node));
    }

    /// Resolve a platform selection using this window's text-run identities.
    /// Returns whether the request was accepted, including an unchanged selection.
    pub fn select_text_input(
        &self,
        element: ElementId,
        input: &mut TextInputState,
        context: &mut TextInputContext,
        selection: &accesskit::TextSelection,
    ) -> bool {
        self.resolve_text_input_selection(element, input, context, selection)
            .is_some_and(|selection| {
                input
                    .apply_accessibility_selection(context, selection)
                    .is_some()
            })
    }

    /// Validate the source tree's run identities without changing the editor's selection.
    /// Hosts apply the returned range after acquiring focus and synchronizing field drafts.
    pub fn resolve_text_input_selection(
        &self,
        element: ElementId,
        input: &mut TextInputState,
        context: &mut TextInputContext,
        selection: &accesskit::TextSelection,
    ) -> Option<TextInputSelection> {
        if !self.live.contains(&element) || !self.editable_text.contains(&element) {
            return None;
        }
        let projection = self.text_projections.get(&element)?;
        input.resolve_accessibility_projection(context, projection, selection)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Dimension, ElementKey, LayoutEngine, Style, TextEditKind, Viewport};

    use super::*;

    #[test]
    fn semantic_identity_survives_rebuild_and_disabled_subtree_has_no_actions() {
        let child = Element::container(ElementId::new(3), ElementKey::new("button"))
            .styled(
                Style::default()
                    .size(Dimension::Px(60.0), Dimension::Px(24.0))
                    .focusable(true),
            )
            .semantic(
                Semantic::new(SemanticRole::Button, "Run")
                    .activatable(true)
                    .busy(true),
            )
            .tooltip("Run the current graph");
        let tree = Element::container(ElementId::new(1), ElementKey::new("root"))
            .styled(Style::default().size(Dimension::Px(100.0), Dimension::Px(100.0)))
            .child(
                Element::container(ElementId::new(2), ElementKey::new("disabled"))
                    .styled(Style::default().action_boundary(true))
                    .child(child),
            )
            .child(
                Element::text(ElementId::new(4), ElementKey::new("label"), "Vesta 地形").semantic(
                    Semantic::new(SemanticRole::Label, "Vesta 地形")
                        .description("Mixed-script label"),
                ),
            )
            .child(
                Element::container(ElementId::new(5), ElementKey::new("status"))
                    .semantic(Semantic::new(SemanticRole::Status, "Ready"))
                    .styled(Style::default().size(Dimension::Px(80.0), Dimension::Px(20.0))),
            );
        let layout = LayoutEngine
            .compute(&tree, Viewport::new(100.0, 100.0, 1.0))
            .expect("semantic test layout");
        let mut semantics = AccessibilityTree::default();
        let first = semantics.build(&tree, &layout, Some(ElementId::new(3)), "Test");
        let id = semantics
            .node_for_element(ElementId::new(3))
            .expect("semantic node");
        assert_eq!(first.focus, NodeId(0));
        let button = first
            .nodes
            .iter()
            .find(|(candidate, _)| *candidate == id)
            .expect("button node");
        assert!(button.1.is_disabled());
        assert!(button.1.is_busy());
        assert_eq!(button.1.description(), Some("Run the current graph"));
        assert!(!button.1.supports_action(Action::Click));
        let label_id = semantics
            .node_for_element(ElementId::new(4))
            .expect("label node");
        let label = first
            .nodes
            .iter()
            .find(|(candidate, _)| *candidate == label_id)
            .expect("label in update");
        assert_eq!(label.1.role(), Role::Label);
        assert_eq!(label.1.value(), Some("Vesta 地形"));
        assert_eq!(label.1.description(), Some("Mixed-script label"));
        assert!(!label.1.supports_action(Action::Click));
        let status_id = semantics
            .node_for_element(ElementId::new(5))
            .expect("live status");
        assert_eq!(
            first
                .nodes
                .iter()
                .find(|(node, _)| *node == status_id)
                .expect("status node")
                .1
                .live(),
            Some(accesskit::Live::Polite)
        );
        let second = semantics.build(&tree, &layout, None, "Test");
        assert_eq!(semantics.node_for_element(ElementId::new(3)), Some(id));
        assert_eq!(second.focus, NodeId(0));

        let mut settled = tree.clone();
        settled.children[0].children[0]
            .semantic
            .as_mut()
            .expect("button state")
            .busy = false;
        let update = semantics.build(&settled, &layout, None, "Test");
        assert_eq!(semantics.node_for_element(ElementId::new(3)), Some(id));
        assert!(
            !update
                .nodes
                .iter()
                .find(|(node, _)| *node == id)
                .expect("updated same owner")
                .1
                .is_busy()
        );

        let checkable = |checked, disabled| {
            Element::container(ElementId::new(3), ElementKey::new("menu.toggle"))
                .styled(
                    Style::default()
                        .size(Dimension::Px(100.0), Dimension::Px(30.0))
                        .focusable(true)
                        .action_boundary(disabled),
                )
                .semantic(
                    Semantic::new(SemanticRole::MenuItemCheckBox, "Dark theme").checked(checked),
                )
        };
        for (checked, disabled) in [(true, false), (false, false), (true, true)] {
            let tree = checkable(checked, disabled);
            let layout = LayoutEngine
                .compute(&tree, Viewport::new(100.0, 100.0, 1.0))
                .expect("checkable menu layout");
            let update = semantics.build(&tree, &layout, Some(tree.id), "Test");
            assert_eq!(semantics.node_for_element(tree.id), Some(id));
            let node = &update
                .nodes
                .iter()
                .find(|(candidate, _)| *candidate == id)
                .expect("checkable menu node")
                .1;
            assert_eq!(node.role(), Role::MenuItemCheckBox);
            assert_eq!(
                node.toggled(),
                Some(if checked {
                    Toggled::True
                } else {
                    Toggled::False
                })
            );
            assert_eq!(node.is_disabled(), disabled);
            assert_eq!(node.supports_action(Action::Click), !disabled);
            assert_eq!(node.supports_action(Action::Focus), !disabled);
        }

        for role in [
            SemanticRole::Slider,
            SemanticRole::SpinButton,
            SemanticRole::PasswordInput,
        ] {
            for read_only in [false, true] {
                let semantic = Semantic::new(role, "Value").read_only(read_only);
                let semantic = if role == SemanticRole::PasswordInput {
                    semantic.value("••••")
                } else {
                    semantic.numeric(0.5, 0.0, 1.0, Some(0.1))
                };
                let tree = Element::container(ElementId::new(3), ElementKey::new("range"))
                    .styled(
                        Style::default()
                            .size(Dimension::Px(100.0), Dimension::Px(30.0))
                            .focusable(true),
                    )
                    .semantic(semantic);
                let layout = LayoutEngine
                    .compute(&tree, Viewport::new(100.0, 100.0, 1.0))
                    .expect("range layout");
                let update = semantics.build(&tree, &layout, Some(tree.id), "Test");
                let node = &update
                    .nodes
                    .iter()
                    .find(|(candidate, _)| *candidate == id)
                    .expect("range node")
                    .1;
                assert_eq!(node.is_read_only(), read_only);
                assert_eq!(
                    node.numeric_value(),
                    (role != SemanticRole::PasswordInput).then_some(0.5)
                );
                assert_eq!(
                    node.value(),
                    (role == SemanticRole::PasswordInput).then_some("••••")
                );
                assert!(node.supports_action(Action::Focus));
                assert_eq!(
                    node.supports_action(Action::SetValue),
                    !read_only,
                    "a read-only range must not advertise a value mutation"
                );
                let route = crate::ActionRoute::<()>::build(&tree, []).expect("range route");
                let scope = crate::FocusScope::new(crate::UiWindowId(1), &route, None);
                for action in [
                    Action::Focus,
                    Action::SetValue,
                    Action::SetTextSelection,
                    Action::Click,
                ] {
                    assert_eq!(
                        scope.accessible_target(&semantics, id, action).is_some(),
                        node.supports_action(action),
                        "platform admission must honor the published {action:?} permission"
                    );
                }
            }
        }
    }

    #[test]
    fn focused_clipped_row_remains_in_accessibility_tree() {
        let row = ElementId::new(3);
        let other = ElementId::new(4);
        let tree = Element::scroll_box(
            ElementId::new(1),
            ElementKey::new("scroll"),
            Point::new(0.0, 0.0),
        )
        .styled(Style::default().size(Dimension::Px(100.0), Dimension::Px(100.0)))
        .child(
            Element::container(ElementId::new(2), ElementKey::new("content"))
                .styled(Style::default().size(Dimension::Px(100.0), Dimension::Px(300.0)))
                .child(
                    Element::container(row, ElementKey::new("row"))
                        .styled(
                            Style::default()
                                .absolute(0.0, 200.0)
                                .size(Dimension::Px(100.0), Dimension::Px(20.0))
                                .focusable(true),
                        )
                        .semantic(Semantic::new(SemanticRole::ListItem, "Focused row")),
                )
                .child(
                    Element::container(other, ElementKey::new("other"))
                        .styled(
                            Style::default()
                                .absolute(0.0, 240.0)
                                .size(Dimension::Px(100.0), Dimension::Px(20.0))
                                .focusable(true),
                        )
                        .semantic(Semantic::new(SemanticRole::ListItem, "Other row")),
                ),
        );
        let layout = LayoutEngine
            .compute(&tree, Viewport::new(100.0, 100.0, 1.0))
            .expect("clipped list layout");
        let mut semantics = AccessibilityTree::default();
        semantics.build(&tree, &layout, None, "Test");
        assert!(semantics.node_for_element(row).is_none());
        assert!(semantics.node_for_element(other).is_none());
        let update = semantics.build(&tree, &layout, Some(row), "Test");
        assert_eq!(
            update.focus,
            semantics.node_for_element(row).expect("focused row")
        );
        assert!(semantics.node_for_element(other).is_none());
    }

    #[test]
    fn text_runs_keep_unique_ids_when_an_existing_run_gains_a_new_sibling() {
        let mut semantics = AccessibilityTree::default();
        let field = ElementId::new(2);
        let root = Element::container(ElementId::new(1), ElementKey::new("root"))
            .styled(Style::default().size(Dimension::Px(240.0), Dimension::Px(100.0)))
            .child(
                Element::container(field, ElementKey::new("field"))
                    .styled(Style::default().size(Dimension::Px(200.0), Dimension::Px(80.0)))
                    .semantic(Semantic::new(SemanticRole::TextInput, "Text")),
            );
        let layout = LayoutEngine
            .compute(&root, Viewport::new(240.0, 100.0, 1.0))
            .expect("text input layout");
        let mut context = TextInputContext::default();
        let bytes = include_bytes!("../tests/fixtures/MaterialIcons-Regular.ttf");
        assert!(context.register_font(std::sync::Arc::new(bytes.to_vec())));
        let mut input = TextInputState::new_multiline(13.0, "Material Icons", 200.0);
        input.reset(&mut context, "abc").expect("initial text");

        let mut update = semantics.build(&root, &layout, Some(field), "Test");
        semantics.enrich_text_input(
            &mut update,
            field,
            &mut input,
            &mut context,
            Point::new(0.0, 0.0),
        );
        let field_node = semantics.node_for_element(field).expect("field node");
        let first_run = update
            .nodes
            .iter()
            .find(|(id, _)| *id == field_node)
            .expect("field update")
            .1
            .children()[0];

        input
            .insert(&mut context, "\ndef", TextEditKind::Typing)
            .expect("second line");
        let mut update = semantics.build(&root, &layout, Some(field), "Test");
        semantics.enrich_text_input(
            &mut update,
            field,
            &mut input,
            &mut context,
            Point::new(0.0, 0.0),
        );
        let children = update
            .nodes
            .iter()
            .find(|(id, _)| *id == field_node)
            .expect("field update after edit")
            .1
            .children();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0], first_run);
        assert_ne!(children[0], children[1], "text runs must have unique IDs");
        let mut other_window = AccessibilityTree {
            next_id: 100,
            ..Default::default()
        };
        let mut other_update = other_window.build(&root, &layout, Some(field), "Other window");
        other_window.enrich_text_input(
            &mut other_update,
            field,
            &mut input,
            &mut context,
            Point::new(20.0, 10.0),
        );
        let other_field = other_window.node_for_element(field).expect("other field");
        let other_runs = other_update
            .nodes
            .iter()
            .find(|(id, _)| *id == other_field)
            .expect("other projection")
            .1
            .children();
        assert!(
            other_runs.iter().all(|id| !children.contains(id)),
            "run identity belongs to each platform tree"
        );
        let selection = accesskit::TextSelection {
            anchor: accesskit::TextPosition {
                node: first_run,
                character_index: 1,
            },
            focus: accesskit::TextPosition {
                node: first_run,
                character_index: 2,
            },
        };
        assert!(
            !other_window.select_text_input(field, &mut input, &mut context, &selection),
            "foreign run IDs cannot select this window's text"
        );
        assert!(semantics.select_text_input(field, &mut input, &mut context, &selection));
        assert_eq!(input.selected_text(), Some("b"));
        assert!(
            semantics.select_text_input(field, &mut input, &mut context, &selection),
            "a valid unchanged selection must still let the host acquire focus"
        );
        let mut update = semantics.build(&root, &layout, Some(field), "Test");
        semantics.enrich_text_input(
            &mut update,
            field,
            &mut input,
            &mut context,
            Point::default(),
        );
        assert_eq!(
            update
                .nodes
                .iter()
                .find(|(id, _)| *id == field_node)
                .expect("stable projection")
                .1
                .children()[0],
            first_run
        );
        let legacy_selection = accesskit::TextSelection {
            anchor: accesskit::TextPosition {
                node: first_run,
                character_index: 0,
            },
            focus: accesskit::TextPosition {
                node: first_run,
                character_index: 1,
            },
        };
        assert!(input.select_accessibility(&mut context, &legacy_selection));
        assert_eq!(
            input.selected_text(),
            Some("a"),
            "single-window hosts retain selection compatibility"
        );
        let disabled_root = root
            .clone()
            .styled(root.style.clone().action_boundary(true));
        let disabled_layout = LayoutEngine
            .compute(&disabled_root, Viewport::new(100.0, 100.0, 1.0))
            .expect("disabled field layout");
        let mut disabled_update = semantics.build(&disabled_root, &disabled_layout, None, "Test");
        semantics.enrich_text_input(
            &mut disabled_update,
            field,
            &mut input,
            &mut context,
            Point::default(),
        );
        let disabled_node = &disabled_update
            .nodes
            .iter()
            .find(|(id, _)| *id == field_node)
            .expect("disabled input")
            .1;
        assert!(disabled_node.is_disabled());
        assert!(!disabled_node.supports_action(Action::SetValue));
        assert!(
            !disabled_node.supports_action(Action::SetTextSelection),
            "run enrichment must not restore a disabled control's write actions"
        );
        assert!(
            semantics
                .resolve_text_input_selection(field, &mut input, &mut context, &selection)
                .is_none()
        );
        assert_eq!(input.selected_text(), Some("a"));
        // Continue after the disabled projection assertions so the read-only
        // enabled projection does not replace the tree those assertions inspect.
        let mut readonly = TextInputState::new_read_only(13.0, "sans-serif", None);
        readonly
            .reset(&mut context, "Read-only e\u{301}中文")
            .expect("owner source");
        let mut readonly_root = root.clone();
        readonly_root.children[0].semantic =
            Some(Semantic::new(SemanticRole::TextInput, "Selectable source").read_only(true));
        let mut readonly_update = semantics.build(&readonly_root, &layout, Some(field), "Test");
        let node = &readonly_update
            .nodes
            .iter()
            .find(|(id, _)| *id == field_node)
            .expect("read-only base")
            .1;
        assert!(node.is_read_only());
        assert!(node.supports_action(Action::SetTextSelection));
        assert!(!node.supports_action(Action::SetValue));
        semantics.enrich_text_input(
            &mut readonly_update,
            field,
            &mut readonly,
            &mut context,
            Point::default(),
        );
        let node = &readonly_update
            .nodes
            .iter()
            .find(|(id, _)| *id == field_node)
            .expect("read-only enriched")
            .1;
        assert!(node.is_read_only());
        assert!(node.supports_action(Action::SetTextSelection));
        assert!(!node.supports_action(Action::SetValue));
        assert_eq!(node.value(), Some("Read-only e\u{301}中文"));

        let mut direct_update = TreeUpdate {
            nodes: Vec::new(),
            tree: None,
            focus: NodeId(1),
        };
        let mut direct_node = Node::new(Role::TextInput);
        let mut next_direct = 10_000;
        input.accessibility(
            &mut context,
            &mut direct_update,
            &mut direct_node,
            || {
                next_direct += 1;
                NodeId(next_direct)
            },
            Point::default(),
        );
        let direct_run = direct_node.children()[0];
        let direct_selection = *direct_node.text_selection().expect("direct selection");
        let before = input.inspection_selection();
        let resolved = input
            .resolve_accessibility_selection(&mut context, &direct_selection)
            .expect("valid unchanged direct range");
        assert_eq!(
            input.inspection_selection(),
            before,
            "resolution is read-only"
        );
        assert_eq!(
            input.apply_accessibility_selection(&mut context, resolved),
            Some(false)
        );
        assert!(!input.select_accessibility(&mut context, &direct_selection));
        let foreign_selection = accesskit::TextSelection {
            anchor: accesskit::TextPosition {
                node: NodeId(u64::MAX),
                character_index: 0,
            },
            ..direct_selection
        };
        assert!(
            input
                .resolve_accessibility_selection(&mut context, &foreign_selection)
                .is_none()
        );
        input
            .preedit(&mut context, "ni", None)
            .expect("direct composition");
        input.accessibility(
            &mut context,
            &mut direct_update,
            &mut direct_node,
            || {
                next_direct += 1;
                NodeId(next_direct)
            },
            Point::default(),
        );
        assert_eq!(direct_node.children()[0], direct_run);
        assert!(
            direct_node.text_selection().is_none(),
            "IME hides its caret"
        );
        assert!(
            input
                .resolve_accessibility_selection(&mut context, &direct_selection)
                .is_none(),
            "composition retains ownership"
        );
        input.cancel_compose(&mut context);

        let mut update = semantics.build(&root, &layout, Some(field), "Test");
        semantics.enrich_text_input(
            &mut update,
            field,
            &mut input,
            &mut context,
            Point::default(),
        );
        let retired_selection = *update
            .nodes
            .iter()
            .find(|(id, _)| *id == field_node)
            .expect("published editor")
            .1
            .text_selection()
            .expect("published range");
        let mut removed = root.clone();
        removed.children.clear();
        let removed_layout = LayoutEngine
            .compute(&removed, Viewport::new(240.0, 100.0, 1.0))
            .expect("removed editor layout");
        semantics.build(&removed, &removed_layout, None, "Test");
        assert!(semantics.node_for_element(field).is_none());
        assert!(
            semantics.text_projections.is_empty(),
            "removed editors must release their retained run maps"
        );

        let mut returned = semantics.build(&root, &layout, Some(field), "Test");
        semantics.enrich_text_input(
            &mut returned,
            field,
            &mut input,
            &mut context,
            Point::default(),
        );
        assert_eq!(semantics.node_for_element(field), Some(field_node));
        assert!(
            semantics
                .resolve_text_input_selection(field, &mut input, &mut context, &retired_selection)
                .is_none(),
            "a run retired during removal must not select a returning editor"
        );
        let current_selection = *returned
            .nodes
            .iter()
            .find(|(id, _)| *id == field_node)
            .expect("returned editor")
            .1
            .text_selection()
            .expect("current range");
        assert!(
            semantics
                .resolve_text_input_selection(field, &mut input, &mut context, &current_selection)
                .is_some()
        );
    }
}
