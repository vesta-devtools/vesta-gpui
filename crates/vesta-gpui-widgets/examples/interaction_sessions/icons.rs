//! Actual public composition contract, with no renderer/resource I/O or OS input.

use super::*;
use vesta_gpui::{AccessibilityTree, PaintCommand, PaintList, SemanticToggle, Style};
use vesta_gpui_widgets::{
    CompactButtonKind, IconButtonContent, IconButtonPresentation, IconButtonSpec, icon_button,
    icon_button_with_presentation,
};

fn spec(state: ControlState, kind: CompactButtonKind) -> IconButtonSpec<'static> {
    IconButtonSpec {
        icon: "+",
        name: "Pin caller item",
        shortcut: Some("Space"),
        kind,
        state,
    }
}

fn projection(
    scope: ElementScope,
    spec: IconButtonSpec<'_>,
    presentation: IconButtonPresentation<'_>,
    theme: WidgetTheme,
) -> Widget<Action> {
    let id = |part| WidgetIdentity::new(scope.element(part), format!("icon.part.{part}"));
    Widget::from_element(
        Element::container(scope.element(1), ElementKey::new("icons")).styled(
            Style::default()
                .size(Dimension::Px(200.0), Dimension::Px(60.0))
                .axis(Axis::Row)
                .gap(12.0),
        ),
    )
    .on_click(Action::Open(FieldId(999)))
    .child(icon_button_with_presentation(
        id(2),
        id(3),
        spec,
        Action::Apply,
        presentation,
        theme,
    ))
    .child(icon_button_with_presentation(
        id(4),
        id(5),
        IconButtonSpec {
            state: ControlState::Disabled,
            ..spec
        },
        Action::Apply,
        IconButtonPresentation {
            content: IconButtonContent::Image("fixture://disabled"),
            selected: Some(true),
        },
        theme,
    ))
}

pub(super) fn qualify(context: &mut TextInputContext) -> Result<(), String> {
    let mut frames = 0;
    for window in [UiWindowId(41), UiWindowId(43)] {
        let scope = ElementIdAllocator.allocate_scope().map_err(debug_error)?;
        let mut retained = RetainedLayoutEngine::default();
        let mut semantics = AccessibilityTree::default();
        let mut ownership = InputOwnership::default();
        let mut semantic_id = None;
        for theme in [WidgetTheme::dark(), WidgetTheme::light()] {
            for kind in [CompactButtonKind::Icon, CompactButtonKind::Toolbar] {
                // Legacy and opt-in default remain exactly the same projection.
                let id = |part| WidgetIdentity::new(scope.element(part), format!("legacy.{part}"));
                let legacy = icon_button(
                    id(2),
                    id(3),
                    spec(ControlState::Idle, kind),
                    Action::Apply,
                    theme,
                );
                let explicit = icon_button_with_presentation(
                    id(2),
                    id(3),
                    spec(ControlState::Idle, kind),
                    Action::Apply,
                    IconButtonPresentation::default(),
                    theme,
                );
                require(
                    legacy == explicit,
                    "default icon presentation changed legacy output",
                )?;
                let mut root_bounds = None;
                for selected in [None, Some(false), Some(true)] {
                    for state in [
                        ControlState::Idle,
                        ControlState::Hovered,
                        ControlState::Pressed,
                        ControlState::Disabled,
                    ] {
                        for content in [
                            IconButtonContent::Glyph,
                            IconButtonContent::Image("fixture://wide"),
                            IconButtonContent::Image("fixture://tall"),
                        ] {
                            let widget = projection(
                                scope,
                                spec(state, kind),
                                IconButtonPresentation { content, selected },
                                theme,
                            );
                            let root = &widget.element.children[0];
                            let icon = &root.children[0];
                            require(
                                icon.id == scope.element(3)
                                    && !icon.style.pointer_events
                                    && !icon.style.focusable
                                    && icon.semantic.is_none(),
                                "decorative image became an independent owner",
                            )?;
                            let semantic =
                                root.semantic.as_ref().ok_or("missing Button meaning")?;
                            require(
                                semantic.label == "Pin caller item"
                                    && root.tooltip.as_deref() == Some("Pin caller item (Space)")
                                    && semantic.toggle
                                        == selected.map(|s| {
                                            if s {
                                                SemanticToggle::On
                                            } else {
                                                SemanticToggle::Off
                                            }
                                        }),
                                "name/tooltip/controlled selection changed",
                            )?;
                            let layout = retained
                                .compute_with_text_context(
                                    &widget.element,
                                    Viewport::new(200.0, 60.0, 1.25),
                                    context,
                                )
                                .map_err(debug_error)?;
                            let bounds = layout.node(root.id).ok_or("missing root layout")?.rect;
                            if let Some(previous) = root_bounds {
                                require(
                                    bounds == previous,
                                    "source or selection moved the Button shell",
                                )?;
                            }
                            root_bounds = Some(bounds);
                            let enabled = state != ControlState::Disabled;
                            let route = ActionRoute::build(&widget.element, widget.actions)
                                .map_err(debug_error)?;
                            require(
                                route.action_for_hit(icon.id) == enabled.then_some(&Action::Apply)
                                    && route.action_for_hit(scope.element(5)).is_none(),
                                "resource child/disabled root changed Action ownership",
                            )?;
                            require(
                                route
                                    .focus_order()
                                    .iter()
                                    .copied()
                                    .eq(enabled.then_some(root.id)),
                                "decorative or disabled content joined Tab order",
                            )?;
                            if enabled {
                                ownership.set_focus(Some(InputTarget {
                                    window,
                                    element: root.id,
                                }));
                                ownership.capture_pointer(
                                    InputTarget {
                                        window,
                                        element: root.id,
                                    },
                                    PointerButton::Primary,
                                );
                            }
                            FocusScope::new(window, &route, None).reconcile(&mut ownership);
                            require(
                                enabled
                                    || ownership.focus().is_none() && ownership.capture().is_none(),
                                "disabled icon retained focus/capture",
                            )?;
                            if enabled {
                                let center = Point::new(
                                    bounds.x + bounds.width / 2.0,
                                    bounds.y + bounds.height / 2.0,
                                );
                                require(
                                    HitTestTree::build(&widget.element, &layout).hit(center)
                                        == Some(root.id),
                                    "resource image intercepted root hit",
                                )?;
                            }
                            semantics.build(
                                &widget.element,
                                &layout,
                                ownership.focus().map(|f| f.element),
                                "Icons",
                            );
                            let current = semantics
                                .node_for_element(root.id)
                                .ok_or("missing root semantic identity")?;
                            if let Some(previous) = semantic_id {
                                require(
                                    current == previous,
                                    "image/source/selection replaced semantic identity",
                                )?;
                            }
                            semantic_id = Some(current);
                            require(
                                semantics.node_for_element(icon.id).is_none(),
                                "decorative resource created a second semantic node",
                            )?;
                            if let IconButtonContent::Image(source) = content {
                                require(PaintList::build(&widget.element, &layout).commands().iter().any(|c| matches!(c, PaintCommand::FittedImage { element, source: actual, presentation, .. } if *element == icon.id && actual == source && presentation.fit == vesta_gpui::ImageFit::Contain)), "resource token/Contain was lost in actual paint")?;
                            }
                            frames += 1;
                        }
                    }
                }
            }
        }
    }
    println!(
        "Public resource icon contract: {frames} frames; legacy/resource, two sources/themes/kinds, controlled selection, root hit/route/semantic and disabled focus/capture pass. No GPU/OS/reader claim."
    );
    Ok(())
}
