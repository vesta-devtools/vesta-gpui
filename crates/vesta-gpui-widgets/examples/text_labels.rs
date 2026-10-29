//! Public exact-width static labels. Supply host fonts; no OS window or renderer.
use std::sync::Arc;
use vesta_gpui::{
    AccessibilityTree, ActionRoute, Dimension, Element, ElementIdAllocator, ElementKey,
    ElementKind, HitTestTree, LayoutEngine, PaintCommand, PaintList, Point, Style, TextInputAction,
    TextInputContext, TextInputState, Viewport, text_input_wrap_width,
};
use vesta_gpui_widgets::{
    TextContent, WidgetIdentity, WidgetTheme, ellipsized_label, selectable_label,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = std::env::args_os().skip(1).collect::<Vec<_>>();
    if paths.is_empty() {
        return Err("provide at least one host font file".into());
    }
    let mut context = TextInputContext::default();
    for path in paths {
        if !context.register_font(Arc::new(std::fs::read(path)?)) {
            return Err("host font registration failed".into());
        }
    }
    let scope = ElementIdAllocator
        .allocate_scope()
        .map_err(|error| format!("label scope: {error:?}"))?;
    let label_id = scope.element(2);
    let mut semantics = AccessibilityTree::default();
    let mut projections = 0;
    let mut selectable_projections = 0;
    for theme in [WidgetTheme::dark(), WidgetTheme::light()] {
        for source in [
            "Create a world from a brief, then refine terrain and details.",
            "从创作意图生成世界，再细化地形与细节。",
            "مرحبا بالعالم — Vesta 世界创作",
            "Ae\u{301}👨‍👩‍👧‍👦 / family and combining graphemes / 地形",
        ] {
            for width in [0.0, 48.0, 160.0, 320.0] {
                let label = ellipsized_label(
                    WidgetIdentity::new(label_id, "caller.label"),
                    TextContent::Localized {
                        key: "caller.message",
                        value: source,
                    },
                    width,
                    theme,
                    &mut context,
                )?;
                let original = label.semantic.as_ref().ok_or("missing label semantic")?;
                assert_eq!(original.label, source);
                assert!(!label.style.focusable && !label.style.pointer_events);
                let root = Element::container(scope.element(1), ElementKey::new("caller.root"))
                    .styled(
                        Style::default()
                            .size(Dimension::Px(width), Dimension::Auto)
                            .pointer_events(false),
                    )
                    .child(label);
                let layout = LayoutEngine.compute_with_text_context(
                    &root,
                    Viewport::new(width.max(1.0), 100.0, 1.0),
                    &mut context,
                )?;
                let route = ActionRoute::<u8>::build(&root, [])
                    .map_err(|error| format!("route: {error:?}"))?;
                assert!(route.focus_order().is_empty());
                let bounds = layout.node(label_id).ok_or("missing label layout")?.rect;
                let ElementKind::TextInput {
                    snapshot,
                    focused: false,
                } = &root.children[0].kind
                else {
                    return Err("label must use shared shaped text".into());
                };
                assert!(snapshot.layout_width() <= text_input_wrap_width(bounds));
                assert!(
                    snapshot
                        .viewport(bounds, bounds, Point::default())
                        .caret
                        .is_none()
                );
                let paint = PaintList::build(&root, &layout);
                assert!(paint.commands().iter().any(|command|
                    matches!(command, PaintCommand::TextInput { element, .. } if *element == label_id)));
                assert_eq!(
                    HitTestTree::build(&root, &layout).hit(Point::new(
                        bounds.x + bounds.width * 0.5,
                        bounds.y + bounds.height * 0.5,
                    )),
                    None
                );
                let update = semantics.build(&root, &layout, None, "Caller labels");
                if width > 0.0 {
                    let node = semantics
                        .node_for_element(label_id)
                        .ok_or("label not published")?;
                    assert!(
                        update
                            .nodes
                            .iter()
                            .any(|(id, node_value)| *id == node
                                && node_value.value() == Some(source))
                    );
                }
                projections += 1;

                let mut state = TextInputState::new_read_only(13.0, "sans-serif", None);
                state
                    .reset(&mut context, source)
                    .map_err(|error| format!("owner source: {error:?}"))?;
                state.act(&mut context, TextInputAction::SelectAll);
                assert_eq!(state.selected_text(), Some(source));
                let selectable = selectable_label(
                    WidgetIdentity::new(label_id, "caller.selectable"),
                    TextContent::Localized {
                        key: "caller.message",
                        value: source,
                    },
                    &state,
                    true,
                    theme,
                )?;
                let root =
                    Element::container(scope.element(1), ElementKey::new("caller.selection.root"))
                        .styled(
                            Style::default()
                                .size(Dimension::Px(width), Dimension::Auto)
                                .pointer_events(false),
                        )
                        .child(selectable);
                let layout = LayoutEngine.compute_with_text_context(
                    &root,
                    Viewport::new(width.max(1.0), 100.0, 1.0),
                    &mut context,
                )?;
                let route = ActionRoute::<u8>::build(&root, [])
                    .map_err(|error| format!("selection route: {error:?}"))?;
                assert_eq!(route.focus_order(), &[label_id]);
                let bounds = layout
                    .node(label_id)
                    .ok_or("missing selection layout")?
                    .rect;
                let view = state.snapshot().viewport(bounds, bounds, Point::default());
                let mut update =
                    semantics.build(&root, &layout, Some(label_id), "Caller selection");
                semantics.enrich_text_input(
                    &mut update,
                    label_id,
                    &mut state,
                    &mut context,
                    view.origin,
                );
                let published = semantics
                    .node_for_element(label_id)
                    .ok_or("selection not published")?;
                let node = &update
                    .nodes
                    .iter()
                    .find(|(id, _)| *id == published)
                    .ok_or("selection node missing")?
                    .1;
                assert!(node.is_read_only());
                assert_eq!(node.value(), Some(source));
                assert!(node.text_selection().is_some());
                if width > 0.0 {
                    assert_eq!(
                        HitTestTree::build(&root, &layout).hit(Point::new(
                            bounds.x + bounds.width * 0.5,
                            bounds.y + bounds.height * 0.5
                        )),
                        Some(label_id)
                    );
                    state.point(
                        &mut context,
                        view.to_layout(Point::new(
                            bounds.x + bounds.width * 0.5,
                            bounds.y + bounds.height * 0.5,
                        )),
                        false,
                        false,
                    );
                    assert_eq!(state.text(), source);
                }
                selectable_projections += 1;
            }
        }
    }
    println!(
        "{projections} ellipsis and {selectable_projections} selectable public label projections: full source, shared glyphs, caller-owned read-only selection"
    );
    Ok(())
}
