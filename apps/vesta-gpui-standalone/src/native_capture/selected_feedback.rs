//! Opt-in native observation surface for the demonstrated typed selection repairs.
use super::*;
use vesta_gpui::{Semantic, SemanticRole};
use vesta_gpui_widgets::combo_option_with_content;

pub(super) const CASE: &str = "observe-selected-feedback";
const CONTROLS: u32 = 0x7f00_0000;
const LAYOUT: u32 = 0x7f00_0100;
const CONTROL: u32 = 0;
const CONTENT: u32 = 1;
const DISCLOSURE: u32 = 2;
const LABEL: u32 = 3;
const CONTAINER: u32 = 4;

#[derive(Clone, Copy, Debug)]
#[repr(u32)]
enum Kind {
    List = 0,
    Combo = 1,
    Tree = 2,
}

const KINDS: [Kind; 3] = [Kind::List, Kind::Combo, Kind::Tree];

impl Kind {
    fn key(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Combo => "combo",
            Self::Tree => "tree",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::List => "List",
            Self::Combo => "Combo",
            Self::Tree => "Tree",
        }
    }

    fn parent_role(self) -> SemanticRole {
        match self {
            Self::List => SemanticRole::List,
            Self::Combo => SemanticRole::ListBox,
            Self::Tree => SemanticRole::Tree,
        }
    }
}

fn identity(
    scope: ElementScope,
    page: u32,
    kind: Kind,
    disabled: bool,
    part: u32,
) -> WidgetIdentity {
    let slot = page + 16 + kind as u32 * 32 + u32::from(disabled) * 16 + part;
    WidgetIdentity::new(
        scope.element(slot),
        format!(
            "native.selected-feedback.{page}.{}.{disabled}.{part}",
            kind.key()
        ),
    )
}

pub(super) fn install(
    input: &UiBuildState<'_>,
    pages: &mut HashMap<&str, Vec<Element>>,
    actions: &mut Vec<(ElementId, DemoAction)>,
) {
    for (name, base) in [("gallery.controls", CONTROLS), ("gallery.layout", LAYOUT)] {
        let widget = page(input.scopes.gallery, base, input.theme);
        pages.insert(name, vec![widget_element(widget, actions)]);
    }
}

fn page(scope: ElementScope, base: u32, theme: WidgetTheme) -> Widget<DemoAction> {
    let mut result = Widget::from_element(
        Element::container(
            scope.element(base),
            ElementKey::new(format!("native.selected-feedback.{base}.page")),
        )
        .semantic(Semantic::new(SemanticRole::Group, "Selected typed content"))
        .styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Auto)
                .max_width(Dimension::Px(300.0))
                .min_width(Dimension::Px(0.0))
                .axis(Axis::Column)
                .gap(8.0)
                .padding(Insets::all(8.0)),
        )
        .child(
            Element::text(
                scope.element(base + 1),
                ElementKey::new(format!("native.selected-feedback.{base}.title")),
                "Selected typed content",
            )
            .styled(
                Style::default()
                    .text(theme.typography.heading_font_size, theme.palette.text)
                    .pointer_events(false),
            ),
        ),
    );
    let record = (42_u64, "Caller-owned choice");
    for kind in KINDS {
        for disabled in [false, true] {
            let control = identity(scope, base, kind, disabled, CONTROL);
            let body = identity(scope, base, kind, disabled, CONTENT);
            let content = |record: &(u64, &str)| {
                WidgetContent::new(
                    record.1,
                    Widget::from_element(
                        Element::text(
                            body.element,
                            body.key.clone(),
                            format!("Typed {}", record.0),
                        )
                        .styled(
                            Style::default()
                                .text(theme.typography.font_size + 4.0, theme.palette.text)
                                .pointer_events(false),
                        ),
                    ),
                )
            };
            let state = if disabled {
                ControlState::Disabled
            } else {
                ControlState::Idle
            };
            let action = DemoAction::StepNumber(1);
            let mut widget = match kind {
                Kind::List => list_item_from(control, &record, true, action, state, theme, content),
                Kind::Combo => {
                    combo_option_with_content(control, content(&record), true, action, state, theme)
                }
                Kind::Tree => tree_view_row_from(
                    TreeRowContentIds {
                        root: control,
                        disclosure: identity(scope, base, kind, disabled, DISCLOSURE),
                    },
                    &record,
                    TreeViewRowState {
                        depth: 0,
                        has_children: false,
                        expanded: false,
                        selected: true,
                        control: state,
                    },
                    TreeViewRowActions {
                        select: action,
                        toggle: action,
                    },
                    theme,
                    content,
                ),
            };
            widget.element.style.max_width = Dimension::Percent(1.0);
            let caption = format!(
                "{}: {}",
                kind.name(),
                if disabled { "Disabled" } else { "Available" }
            );
            let label = identity(scope, base, kind, disabled, LABEL);
            let container = identity(scope, base, kind, disabled, CONTAINER);
            result = result.child(
                Widget::from_element(
                    Element::container(container.element, container.key)
                        .semantic(Semantic::new(kind.parent_role(), &caption))
                        .styled(
                            Style::default()
                                .size(Dimension::Percent(1.0), Dimension::Auto)
                                .min_width(Dimension::Px(0.0))
                                .axis(Axis::Column)
                                .gap(3.0),
                        )
                        .child(
                            Element::text(label.element, label.key, caption).styled(
                                Style::default()
                                    .text(
                                        theme.typography.small_font_size,
                                        theme.palette.text_muted,
                                    )
                                    .pointer_events(false),
                            ),
                        ),
                )
                .child(widget),
            );
        }
    }
    result
}

#[cfg(test)]
pub(super) fn qualify() {
    use vesta_gpui::{ActionRoute, LayoutEngine};

    fn node(root: &Element, id: ElementId) -> Option<&Element> {
        if root.id == id {
            Some(root)
        } else {
            root.children.iter().find_map(|child| node(child, id))
        }
    }

    let verify = |root: &Element, route: &ActionRoute<DemoAction>, scope, base| {
        let viewport = Viewport::new(1280.0, 1000.0, 1.25);
        let layout = LayoutEngine
            .compute(root, viewport)
            .expect("feedback layout");
        let hits = HitTestTree::build(root, &layout);
        for kind in KINDS {
            let available = identity(scope, base, kind, false, CONTROL).element;
            let disabled = identity(scope, base, kind, true, CONTROL).element;
            let available_node = node(root, available).expect("available typed feedback consumer");
            let disabled_node = node(root, disabled).expect("disabled typed feedback consumer");
            let role = match kind {
                Kind::List => vesta_gpui::SemanticRole::ListItem,
                Kind::Combo => vesta_gpui::SemanticRole::ListBoxOption,
                Kind::Tree => vesta_gpui::SemanticRole::TreeItem,
            };
            for control in [available_node, disabled_node] {
                let semantic = control.semantic.as_ref().expect("shared control semantics");
                assert_eq!(semantic.role, role);
                assert_eq!(semantic.selected, Some(true));
            }
            assert_ne!(
                available_node.style.visual.background, disabled_node.style.visual.background,
                "the native consumer must project disabled presentation, not only block input"
            );
            let available_body = node(root, identity(scope, base, kind, false, CONTENT).element)
                .expect("available caller content");
            let disabled_body = node(root, identity(scope, base, kind, true, CONTENT).element)
                .expect("disabled caller content");
            assert_eq!(available_body.kind, disabled_body.kind);
            assert_eq!(available_body.style, disabled_body.style);
            assert_eq!(route.action_at(available), Some(&DemoAction::StepNumber(1)));
            assert!(route.focus_order().contains(&available));
            assert!(route.action_at(disabled).is_none());
            assert!(!route.focus_order().contains(&disabled));
            let a = layout.node(available).expect("available geometry");
            let b = layout.node(disabled).expect("disabled geometry");
            assert_eq!((a.rect.width, a.rect.height), (b.rect.width, b.rect.height));
            for geometry in [a, b] {
                assert!(
                    geometry
                        .rect
                        .intersection(geometry.inherited_clip)
                        .is_some()
                );
            }
            let hit = hits.hit(Point::new(
                b.rect.x + b.rect.width * 0.5,
                b.rect.y + b.rect.height * 0.5,
            ));
            assert!(hit.and_then(|id| route.action_for_hit(id)).is_none());
        }
    };

    let mut app = App::new();
    crate::tests::activate_gallery_page(&mut app, "gallery.controls");
    let ordinary =
        crate::tests::install_action_route_with_viewport(&mut app, Size::new(1280.0, 1000.0));
    assert!(
        node(
            &ordinary,
            identity(app.scopes.gallery, CONTROLS, Kind::List, false, CONTROL).element
        )
        .is_none()
    );
    assert!(
        app.ui_actions
            .focus_order()
            .iter()
            .any(|id| { app.ui_actions.action_at(*id) == Some(&DemoAction::SetSlider) })
    );
    app.native_capture = Fixture::from_case(CASE);
    assert!(selected_feedback_enabled(&app));
    for theme in [WidgetTheme::dark(), WidgetTheme::light()] {
        app.theme = theme;
        for (panel, base) in [("gallery.controls", CONTROLS), ("gallery.layout", LAYOUT)] {
            crate::tests::activate_gallery_page(&mut app, panel);
            let root = crate::tests::install_action_route_with_viewport(
                &mut app,
                Size::new(1280.0, 1000.0),
            );
            verify(&root, &app.ui_actions, app.scopes.gallery, base);
        }
        let floating_scope = vesta_gpui::ElementIdAllocator
            .allocate_scope()
            .expect("independent floating projection scope");
        let floating = page(floating_scope, LAYOUT, theme);
        let route =
            ActionRoute::build(&floating.element, floating.actions).expect("floating route");
        verify(&floating.element, &route, floating_scope, LAYOUT);
        for kind in KINDS {
            assert!(
                app.ui_actions
                    .action_at(identity(floating_scope, LAYOUT, kind, false, CONTROL).element)
                    .is_none()
            );
        }
    }

    let signature = |app: &App| {
        (
            app.demo.number,
            app.demo.slider,
            app.demo.checkbox,
            app.theme,
            app.text_state.text().to_owned(),
        )
    };
    let mut reference = App::new();
    reference.native_capture = Fixture::from_case("observe");
    reference
        .prepare_native_capture()
        .expect("existing observer preparation");
    let expected = signature(&reference);
    let mut prepared = App::new();
    prepared.native_capture = Fixture::from_case(CASE);
    prepared
        .prepare_native_capture()
        .expect("selected feedback preparation");
    assert_eq!(prepared.demo.workspace, reference.demo.workspace);
    assert_eq!(signature(&prepared), expected);
    let root =
        crate::tests::install_action_route_with_viewport(&mut prepared, Size::new(1280.0, 1000.0));
    verify(
        &root,
        &prepared.ui_actions,
        prepared.scopes.gallery,
        CONTROLS,
    );
}
