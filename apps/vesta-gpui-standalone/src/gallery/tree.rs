//! Existing virtual tree with complete controlled data actions.
use super::*;

pub(super) fn tree_section(
    scope: ElementScope,
    ids: &mut Ids,
    demo: &DemoState,
    focused: Option<usize>,
    theme: WidgetTheme,
    scrollbar: ScrollbarStyle,
    actions: &mut Vec<(ElementId, DemoAction)>,
) -> Element {
    let visible_tree = VisibleTree::build(&demo.tree_entries, &demo.tree_expansion)
        .expect("valid Gallery tree preorder");
    let tree_range = VirtualListRange::compute(
        visible_tree.len(),
        VIRTUAL_ROW_HEIGHT,
        TREE_VIEWPORT_HEIGHT,
        demo.tree_scroll.offset_y(),
        2,
    )
    .expect("bounded Gallery tree geometry");
    let mut tree_widget = tree_view_windowed(
        VirtualListIds {
            root: WidgetIdentity::new(scope.element(TREE_SCROLL), "gallery.tree.scroll"),
            content: ids.identity("gallery.tree.content"),
        },
        Dimension::Px(320.0),
        &visible_tree,
        tree_range,
        focused,
        |entry| {
            let id = entry.id;
            let object = CollectionObject {
                id,
                name: &entry.label,
                thumbnail: "demo://checkerboard",
            };
            tree_view_row_from(
                TreeRowContentIds {
                    root: WidgetIdentity::new(
                        scope.element(TREE_ROW_BASE + id as u32),
                        format!("gallery.tree.row.{id}"),
                    ),
                    disclosure: WidgetIdentity::new(
                        scope.element(TREE_DISCLOSURE_BASE + id as u32),
                        format!("gallery.tree.disclosure.{id}"),
                    ),
                },
                &object,
                TreeViewRowState {
                    depth: entry.depth,
                    has_children: entry.has_children,
                    expanded: demo.tree_expansion.contains(&id),
                    selected: demo.tree_selection.contains(&id),
                    control: ControlState::Idle,
                },
                TreeViewRowActions {
                    select: DemoAction::SelectVirtualTree(id),
                    toggle: DemoAction::ToggleVirtualTree(id),
                },
                theme,
                |item| virtual_object_content(scope, item, true, theme),
            )
        },
    )
    .expect("matching Gallery virtual tree data");
    tree_widget.element.semantic = Some(Semantic::new(SemanticRole::Tree, "Scene hierarchy"));
    tree_widget.element.style.max_width = Dimension::Percent(1.0);
    tree_widget.element.style.scrollbar = scrollbar;
    if let ElementKind::ScrollBox(offset) = &mut tree_widget.element.kind {
        *offset = demo.tree_scroll.offset();
    }
    let mut tree_section = section(
        ids,
        "Virtual tree · 1,000 objects",
        vec![
            widget_element(controls(scope, demo, theme), actions),
            widget_element(tree_widget, actions),
        ],
        theme,
    );
    tree_section.style.height = Dimension::Auto;
    tree_section
}

fn controls(scope: ElementScope, demo: &DemoState, theme: WidgetTheme) -> Widget<DemoAction> {
    use tree_collections::TreeDataAction;
    let identity = |part| {
        WidgetIdentity::new(
            scope.element(26_000_000 + part),
            format!("gallery.tree.data.{part}"),
        )
    };
    let root = identity(0);
    let mut widget = Widget::from_element(
        Element::container(root.element, root.key).styled(
            Style::default()
                .axis(Axis::Row)
                .flex_wrap(FlexWrap::Wrap)
                .gap(4.0)
                .max_width(Dimension::Percent(1.0)),
        ),
    );
    for (index, (label, action)) in [
        ("Reverse siblings", TreeDataAction::Reverse),
        ("Remove selected", TreeDataAction::RemoveSelected),
        ("Restore tree", TreeDataAction::Restore),
        ("Expand all", TreeDataAction::ExpandAll),
        ("Collapse all", TreeDataAction::CollapseAll),
        ("Reveal selected", TreeDataAction::RevealSelected),
    ]
    .into_iter()
    .enumerate()
    {
        let enabled = !matches!(
            action,
            TreeDataAction::RemoveSelected | TreeDataAction::RevealSelected
        ) || demo.tree_selection.selected_ids().next().is_some();
        let mut command = button(
            identity(1 + index as u32 * 2),
            identity(2 + index as u32 * 2),
            label,
            DemoAction::TreeData(action),
            if enabled {
                ControlState::Idle
            } else {
                ControlState::Disabled
            },
            theme,
        );
        command.element.style.max_width = Dimension::Percent(1.0);
        widget = widget.child(command);
    }
    widget
}
