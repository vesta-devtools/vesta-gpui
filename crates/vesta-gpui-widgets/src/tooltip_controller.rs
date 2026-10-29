//! Window-local metadata, timing and shared tooltip projection.

use crate::tooltip::TOOLTIP_PADDING;
use crate::{TooltipState, WidgetIdentity, WidgetTheme, tooltip_popup};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use vesta_gpui::{
    ActionRoute, ContentConstraint, Dimension, Element, ElementId, ElementKey, ElementKind,
    ElementScope, FontRole, HitTestTree, LayoutEngine, LayoutError, LayoutTree, Point, Rect,
    RetainedLayoutEngine, Size, Style, TextInputContext, Viewport,
};

pub struct TooltipController {
    base_layout: RetainedLayoutEngine,
    popup_layout: RetainedLayoutEngine,
    timer: TooltipState,
    anchors: HashMap<ElementId, Rect>,
    focused: Option<ElementId>,
    active: bool,
    delay: Duration,
}

pub struct TooltipFrame<'a, A> {
    pub scope: ElementScope,
    pub viewport: Viewport,
    pub route: &'a ActionRoute<A>,
    /// Deliberate keyboard/accessibility focus, normally `InputOwnership::visible_focus()`.
    pub focus: Option<ElementId>,
    pub focus_root: Option<ElementId>,
    pub cursor: Point,
    pub active: bool,
    pub theme: WidgetTheme,
    pub now: Instant,
}

impl TooltipController {
    /// Keep one controller per window. Hosts provide delay and timestamps.
    pub fn new(delay: Duration) -> Self {
        Self {
            base_layout: RetainedLayoutEngine::default(),
            popup_layout: RetainedLayoutEngine::default(),
            timer: TooltipState::default(),
            anchors: HashMap::new(),
            focused: None,
            active: false,
            delay,
        }
    }
    pub fn set_active(&mut self, active: bool, now: Instant) {
        self.active = active;
        if !active {
            self.focused = None;
            self.timer.hover(None, now);
        }
    }

    fn owner<A>(&self, target: Option<ElementId>, route: &ActionRoute<A>) -> Option<ElementId> {
        route
            .path_to(target?)?
            .into_iter()
            .rev()
            .find(|id| self.anchors.contains_key(id))
    }

    pub fn hover<A>(&mut self, target: Option<ElementId>, route: &ActionRoute<A>, now: Instant) {
        let owner = self.active.then(|| self.owner(target, route)).flatten();
        self.timer.hover(owner, now);
    }

    pub fn leave(&mut self, now: Instant) {
        self.timer.hover(None, now);
    }

    pub fn reveal_due(&mut self, now: Instant) -> bool {
        self.active && self.timer.reveal_due(self.focused, now, self.delay)
    }

    pub fn deadline(&self, now: Instant) -> Option<Instant> {
        self.active
            .then(|| self.timer.deadline(self.focused, now, self.delay))
            .flatten()
    }

    fn capture<A>(
        &mut self,
        root: &Element,
        layout: &LayoutTree,
        frame: &TooltipFrame<'_, A>,
        context: &mut TextInputContext,
    ) {
        fn collect(
            root: &Element,
            layout: &LayoutTree,
            context: &mut TextInputContext,
            out: &mut Vec<ElementId>,
        ) {
            if root.tooltip.as_deref().is_some_and(|text| {
                !text.is_empty() && !repeats_visible_text(root, text, layout, context)
            }) {
                out.push(root.id);
            }
            for child in &root.children {
                collect(child, layout, context, out);
            }
        }
        let mut owners = Vec::new();
        collect(root, layout, context, &mut owners);
        self.anchors = owners
            .into_iter()
            .filter_map(|id| {
                if frame
                    .focus_root
                    .is_some_and(|allowed| !frame.route.is_descendant_of(id, allowed))
                {
                    return None;
                }
                let node = layout.node(id)?;
                let anchor = node.rect.intersection(node.inherited_clip)?;
                Some((id, anchor))
            })
            .collect();
        self.focused = self
            .active
            .then(|| self.owner(frame.focus, frame.route))
            .flatten();
    }

    pub fn layout<A>(
        &mut self,
        root: &mut Element,
        frame: TooltipFrame<'_, A>,
        context: &mut TextInputContext,
    ) -> Result<LayoutTree, LayoutError> {
        self.layout_with_popup(root, frame, context, |_, _, _| None)
    }

    /// Project optional read-only rich content at the same delayed/focused owner.
    /// Returning None retains the ordinary text tooltip. Custom content must keep
    /// the supplied bounds and must not intercept input or add focusable controls.
    pub fn layout_with_popup<A>(
        &mut self,
        root: &mut Element,
        frame: TooltipFrame<'_, A>,
        context: &mut TextInputContext,
        mut popup: impl FnMut(ElementId, Rect, Rect) -> Option<Element>,
    ) -> Result<LayoutTree, LayoutError> {
        let now = frame.now;
        self.set_active(frame.active, now);
        let layout = self
            .base_layout
            .compute_with_text_context(root, frame.viewport, context)?;
        self.capture(root, &layout, &frame, context);
        self.hover(
            HitTestTree::build(root, &layout).hit(frame.cursor),
            frame.route,
            now,
        );
        let Some(owner) = self.timer.visible_owner(self.focused, now, self.delay) else {
            self.popup_layout.clear();
            return Ok(layout);
        };
        let Some(anchor) = self.anchors.get(&owner).copied() else {
            self.popup_layout.clear();
            return Ok(layout);
        };
        let Some(text) = root
            .find(owner)
            .and_then(|element| element.tooltip.as_deref())
        else {
            self.popup_layout.clear();
            return Ok(layout);
        };
        let bounds = Rect::new(
            8.0,
            8.0,
            (frame.viewport.width - 16.0).max(0.0),
            (frame.viewport.height - 16.0).max(0.0),
        );
        if bounds.width <= 20.0 || bounds.height <= 20.0 {
            self.popup_layout.clear();
            return Ok(layout);
        }
        if let Some(mut popup) = popup(owner, anchor, bounds) {
            fn read_only(element: &mut Element) {
                element.style.pointer_events = false;
                element.style.focusable = false;
                element.style.action_boundary = false;
                for child in &mut element.children {
                    read_only(child);
                }
            }
            read_only(&mut popup);
            root.children.push(popup);
            return self
                .popup_layout
                .compute_with_text_context(root, frame.viewport, context);
        }
        let width = bounds.width.min(360.0);
        let font_size = frame.theme.typography.small_font_size;
        let content = Element::text(
            frame.scope.element(2),
            ElementKey::new("window.tooltip.measure"),
            text,
        )
        .styled(
            Style::default()
                .max_width(Dimension::Px(width))
                .padding(TOOLTIP_PADDING)
                .text(font_size, frame.theme.palette.text)
                .font_role(FontRole::Ui)
                .text_wrap(true),
        );
        let desired = LayoutEngine
            .measure_with_text_context(
                &content,
                ContentConstraint {
                    width: Some(width),
                    height: None,
                },
                context,
            )?
            .preferred;
        let mut popup = tooltip_popup(
            WidgetIdentity::new(frame.scope.element(1), "window.tooltip"),
            WidgetIdentity::new(frame.scope.element(2), "window.tooltip.label"),
            text,
            Rect::new(anchor.x, anchor.y - 6.0, anchor.width, anchor.height + 12.0),
            bounds,
            Size::new(
                desired.width.clamp(20.0, width),
                desired.height.min(bounds.height),
            ),
            frame.theme,
        );
        // Measure the rendered tree at its final width: the border and padding
        // can wrap the label differently from the initial intrinsic estimate.
        popup.style.height = Dimension::Auto;
        let measured = LayoutEngine
            .measure_with_text_context(
                &popup,
                ContentConstraint {
                    width: Some(desired.width.clamp(20.0, width)),
                    height: None,
                },
                context,
            )?
            .preferred;
        let size = Size::new(
            desired.width.clamp(20.0, width),
            measured.height.ceil().min(bounds.height),
        );
        let position = crate::place_popup(
            Rect::new(anchor.x, anchor.y - 6.0, anchor.width, anchor.height + 12.0),
            size,
            bounds,
            crate::PopupDirection::Below,
        );
        popup.style = popup
            .style
            .absolute(position.x, position.y)
            .size(Dimension::Px(size.width), Dimension::Px(size.height));
        root.children.push(popup);
        self.popup_layout
            .compute_with_text_context(root, frame.viewport, context)
    }
}

/// Keep metadata intact for accessibility and reveal it only when it adds information.
fn repeats_visible_text(
    element: &Element,
    hint: &str,
    layout: &LayoutTree,
    context: &mut TextInputContext,
) -> bool {
    if let ElementKind::Text(text) = &element.kind
        && text == hint
        && element.style.visual.font_role != FontRole::Icon
        && let Some(node) = layout.node(element.id)
        && let Some(visible) = node.rect.intersection(node.inherited_clip)
        && let Ok(shaped) = context.shape_static_text_wrapped(
            text,
            element.style.visual.font_size,
            element.style.visual.font_role,
            element.style.text_wrap.then_some(node.rect.width),
        )
    {
        // Conservative containment handles alignment and clipped scroll ancestors alike.
        const TOLERANCE: f32 = 0.5;
        if visible.width + TOLERANCE >= node.rect.width
            && visible.height + TOLERANCE >= node.rect.height
            && shaped.layout_width() > 0.0
            && shaped.layout_height() > 0.0
            && shaped.layout_width() <= node.rect.width + TOLERANCE
            && shaped.layout_height() <= node.rect.height + TOLERANCE
        {
            return true;
        }
    }
    element
        .children
        .iter()
        .any(|child| repeats_visible_text(child, hint, layout, context))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vesta_gpui::{Dimension, ElementIdAllocator, ElementKey, LayoutEngine, Style};
    const TOOLTIP_DELAY: Duration = Duration::from_millis(500);

    #[test]
    fn tooltip_sessions_resolve_metadata_and_isolate_window_lifetimes() {
        let alloc = ElementIdAllocator;
        let gallery = alloc.allocate_scope().expect("content scope");
        let root_scope = alloc.allocate_scope().expect("root scope");
        let popup_scope = alloc.allocate_scope().expect("tooltip scope");
        let now = Instant::now();
        let owner = gallery.element(1);
        let nested = gallery.element(2);
        let child = gallery.element(3);
        let modal = gallery.element(4);
        let mut root = Element::container(root_scope.element(1), ElementKey::new("root"))
            .styled(Style::default().size(Dimension::Px(400.0), Dimension::Px(200.0)))
            .child(
                Element::container(owner, ElementKey::new("owner"))
                    .styled(Style::default().size(Dimension::Px(100.0), Dimension::Px(50.0)))
                    .tooltip("Outer")
                    .child(
                        Element::container(nested, ElementKey::new("nested"))
                            .styled(Style::default().size(Dimension::Px(40.0), Dimension::Px(30.0)))
                            .tooltip("Nested")
                            .child(Element::text(child, ElementKey::new("child"), "Child")),
                    ),
            )
            .child(Element::container(modal, ElementKey::new("modal")));
        let route = ActionRoute::<u8>::build(&root, []).expect("fixture route");
        let viewport = Viewport::new(400.0, 200.0, 1.0);
        let layout = LayoutEngine
            .compute(&root, viewport)
            .expect("fixture layout");
        let mut frame = TooltipFrame {
            scope: popup_scope,
            viewport,
            route: &route,
            focus: Some(child),
            focus_root: None,
            cursor: Point::new(-1.0, -1.0),
            active: true,
            theme: WidgetTheme::dark(),
            now,
        };
        let mut first = TooltipController::new(TOOLTIP_DELAY);
        let mut second = TooltipController::new(TOOLTIP_DELAY);
        let mut context = TextInputContext::default();
        assert!(context.register_font(std::sync::Arc::new(
            include_bytes!("../../vesta-gpui/tests/fixtures/MaterialIcons-Regular.ttf").to_vec(),
        )));
        context.set_font_stack(FontRole::Ui, "Material Icons");
        first.set_active(true, now);
        first.capture(&root, &layout, &frame, &mut context);
        assert_eq!(first.owner(Some(child), &route), Some(nested));
        assert_eq!(
            first.timer.visible_owner(first.focused, now, TOOLTIP_DELAY),
            Some(nested)
        );
        second.set_active(true, now);
        frame.focus = None;
        second.capture(&root, &layout, &frame, &mut context);
        second.hover(Some(child), &route, now);
        second.hover(Some(nested), &route, now + TOOLTIP_DELAY / 2);
        assert_eq!(second.deadline(now), Some(now + TOOLTIP_DELAY));
        first.set_active(false, now);
        assert_eq!(
            first
                .timer
                .visible_owner(first.focused, now + TOOLTIP_DELAY, TOOLTIP_DELAY),
            None
        );
        assert!(second.reveal_due(now + TOOLTIP_DELAY));
        let mut projected = root.clone();
        frame.focus = Some(child);
        let projected_layout = second
            .layout(&mut projected, frame, &mut context)
            .expect("tooltip projection");
        let popup = popup_scope.element(1);
        let popup_route = ActionRoute::<u8>::build(&projected, []).expect("popup route");
        assert_eq!(popup_route.focus_order(), route.focus_order());
        let popup_rect = projected_layout.node(popup).expect("popup layout").rect;
        let pointer = HitTestTree::build(&projected, &projected_layout).hit(Point::new(
            popup_rect.x + popup_rect.width * 0.5,
            popup_rect.y + popup_rect.height * 0.5,
        ));
        assert_ne!(pointer, Some(popup));
        assert_ne!(pointer, Some(popup_scope.element(2)));
        frame = TooltipFrame {
            scope: popup_scope,
            viewport,
            route: &route,
            focus: Some(child),
            focus_root: None,
            cursor: Point::new(-1.0, -1.0),
            active: true,
            theme: WidgetTheme::dark(),
            now,
        };
        frame.focus_root = Some(modal);
        second.capture(&root, &layout, &frame, &mut context);
        second.hover(Some(child), &route, now + TOOLTIP_DELAY);
        assert_eq!(
            second
                .timer
                .visible_owner(second.focused, now + TOOLTIP_DELAY, TOOLTIP_DELAY),
            None
        );
        frame.focus_root = None;
        root.children[0].style = root.children[0].style.clone().absolute(0.0, 250.0);
        let clipped_layout = LayoutEngine
            .compute(&root, viewport)
            .expect("clipped fixture");
        second.capture(&root, &clipped_layout, &frame, &mut context);
        assert_eq!(second.owner(Some(child), &route), None);
        root.children.clear();
        let removed_route = ActionRoute::build(&root, []).expect("removed target route");
        frame.route = &removed_route;
        second.capture(&root, &layout, &frame, &mut context);
        assert_eq!(second.owner(Some(child), &removed_route), None);

        // Visible labels stay quiet; resizing and ancestor clipping reveal the full title.
        root = Element::container(root_scope.element(1), ElementKey::new("root"))
            .styled(Style::default().size(Dimension::Px(400.0), Dimension::Px(200.0)))
            .child(
                Element::container(owner, ElementKey::new("owner"))
                    .styled(
                        Style::default()
                            .size(Dimension::Px(300.0), Dimension::Px(40.0))
                            .min_width(Dimension::Px(0.0))
                            .clipped(),
                    )
                    .tooltip("Session title")
                    .child(
                        Element::text(child, ElementKey::new("child"), "Session title").styled(
                            Style::default()
                                .size(Dimension::Percent(1.0), Dimension::Px(30.0))
                                .min_width(Dimension::Px(0.0)),
                        ),
                    ),
            )
            .child(
                Element::text(nested, ElementKey::new("icon"), "i")
                    .styled(Style::default().font_role(FontRole::Icon))
                    .tooltip("i"),
            );
        let route = ActionRoute::<u8>::build(&root, []).expect("label route");
        frame.route = &route;
        frame.focus = None;
        let layout = LayoutEngine.compute(&root, viewport).expect("label layout");
        second.capture(&root, &layout, &frame, &mut context);
        assert_eq!(second.owner(Some(child), &route), None);
        assert_eq!(second.owner(Some(nested), &route), Some(nested));
        assert_eq!(root.children[0].tooltip.as_deref(), Some("Session title"));
        root.children[0].style.width = Dimension::Px(24.0);
        let narrow = LayoutEngine.compute(&root, viewport).expect("narrow label");
        second.capture(&root, &narrow, &frame, &mut context);
        assert_eq!(
            second.owner(Some(child), &route),
            Some(owner),
            "owner={:?}, child={:?}",
            narrow.node(owner),
            narrow.node(child)
        );
        root.children[0].style.width = Dimension::Px(300.0);
        root.children[0].style = root.children[0].style.clone().absolute(380.0, 0.0);
        let clipped = LayoutEngine
            .compute(&root, viewport)
            .expect("clipped label");
        second.capture(&root, &clipped, &frame, &mut context);
        assert_eq!(second.owner(Some(child), &route), Some(owner));

        let target = vesta_gpui::InputTarget {
            window: vesta_gpui::UiWindowId(1),
            element: child,
        };
        let mut input = vesta_gpui::InputOwnership::default();
        input.set_focus(Some(target));
        input.observe_focus_input(vesta_gpui::FocusInput::Pointer);
        frame.focus = input.visible_focus().map(|focus| focus.element);
        second.capture(&root, &clipped, &frame, &mut context);
        second.hover(Some(child), &route, now);
        assert_eq!(
            second
                .timer
                .visible_owner(second.focused, now, TOOLTIP_DELAY),
            None
        );
        second.leave(now + TOOLTIP_DELAY);
        assert_eq!(
            second
                .timer
                .visible_owner(second.focused, now + TOOLTIP_DELAY, TOOLTIP_DELAY),
            None
        );
        input.traverse(&[target], false);
        frame.focus = input.visible_focus().map(|focus| focus.element);
        second.capture(&root, &clipped, &frame, &mut context);
        assert_eq!(
            second
                .timer
                .visible_owner(second.focused, now, TOOLTIP_DELAY),
            Some(owner)
        );
        assert_eq!(input.focus(), Some(target));

        // Rich previews share delay, keyboard focus and lifetime gates, and stay passive.
        root.children[0].style = Style::default()
            .size(Dimension::Px(300.0), Dimension::Px(40.0))
            .focusable(true);
        root.children[0].tooltip = Some("Preview details".into());
        let route = ActionRoute::<u8>::build(&root, []).expect("rich route");
        let mut rich = TooltipController::new(TOOLTIP_DELAY);
        let mut calls = 0;
        for (elapsed, active, focus, visible) in [
            (Duration::ZERO, true, None, false),
            (TOOLTIP_DELAY, true, None, true),
            (TOOLTIP_DELAY, false, None, false),
            (TOOLTIP_DELAY, true, Some(child), true),
        ] {
            let mut projected = root.clone();
            let layout = rich
                .layout_with_popup(
                    &mut projected,
                    TooltipFrame {
                        scope: popup_scope,
                        viewport,
                        route: &route,
                        focus,
                        focus_root: None,
                        cursor: Point::new(10.0, 10.0),
                        active,
                        theme: WidgetTheme::dark(),
                        now: now + elapsed,
                    },
                    &mut context,
                    |id, anchor, bounds| {
                        calls += 1;
                        assert_eq!(id, owner);
                        assert!(anchor.width > 0.0 && bounds.width <= viewport.width);
                        Some(
                            Element::container(popup, ElementKey::new("rich"))
                                .styled(
                                    Style::default()
                                        .size(Dimension::Px(100.0), Dimension::Px(80.0))
                                        .absolute(8.0, 60.0)
                                        .focusable(true),
                                )
                                .child(
                                    Element::text(
                                        popup_scope.element(2),
                                        ElementKey::new("rich.child"),
                                        "Details",
                                    )
                                    .styled(Style::default().focusable(true)),
                                ),
                        )
                    },
                )
                .expect("rich projection");
            assert_eq!(layout.node(popup).is_some(), visible);
            let projected_route =
                ActionRoute::<u8>::build(&projected, []).expect("passive rich route");
            assert_eq!(projected_route.focus_order(), route.focus_order());
            if visible {
                assert_ne!(
                    HitTestTree::build(&projected, &layout).hit(Point::new(10.0, 70.0)),
                    Some(popup)
                );
                assert!(
                    !projected
                        .find(popup_scope.element(2))
                        .expect("rich child")
                        .style
                        .pointer_events
                );
            }
        }
        assert_eq!(calls, 2, "content is only built when the owner is visible");
        root.children.clear();
        rich.layout_with_popup(
            &mut root,
            TooltipFrame {
                scope: popup_scope,
                viewport,
                route: &route,
                focus: Some(child),
                focus_root: None,
                cursor: Point::new(10.0, 10.0),
                active: true,
                theme: WidgetTheme::dark(),
                now: now + TOOLTIP_DELAY,
            },
            &mut context,
            |_, _, _| panic!("removed owner must not produce a preview"),
        )
        .expect("removed rich owner");

        // Independent source timers and measured long text stay bounded near
        // the bottom/right edge; hiding one owner cannot retire another source.
        let edge_source = || {
            Element::container(root_scope.element(1), ElementKey::new("root"))
                .styled(Style::default().size(Dimension::Px(400.0), Dimension::Px(200.0)))
                .child(
                    Element::container(owner, ElementKey::new("owner"))
                        .styled(
                            Style::default()
                                .absolute(370.0, 170.0)
                                .size(Dimension::Px(25.0), Dimension::Px(25.0))
                                .focusable(true),
                        )
                        .tooltip(
                            "Long explanation with mixed text 地形 مرحبا and a complete ending.",
                        ),
                )
        };
        let source = edge_source();
        let edge_route = ActionRoute::<u8>::build(&source, []).expect("edge source route");
        let other_scope = alloc.allocate_scope().expect("other tooltip source");
        let mut one = TooltipController::new(TOOLTIP_DELAY);
        let mut two = TooltipController::new(TOOLTIP_DELAY);
        for (controller, scope) in [(&mut one, popup_scope), (&mut two, other_scope)] {
            let mut tree = edge_source();
            let before = controller
                .layout(
                    &mut tree,
                    TooltipFrame {
                        scope,
                        viewport,
                        route: &edge_route,
                        focus: None,
                        focus_root: None,
                        cursor: Point::new(380.0, 180.0),
                        active: true,
                        theme: WidgetTheme::dark(),
                        now,
                    },
                    &mut context,
                )
                .expect("delay begins");
            assert!(before.node(scope.element(1)).is_none());
            assert_eq!(controller.deadline(now), Some(now + TOOLTIP_DELAY));
            let mut tree = edge_source();
            let shown = controller
                .layout(
                    &mut tree,
                    TooltipFrame {
                        scope,
                        viewport,
                        route: &edge_route,
                        focus: None,
                        focus_root: None,
                        cursor: Point::new(380.0, 180.0),
                        active: true,
                        theme: WidgetTheme::dark(),
                        now: now + TOOLTIP_DELAY,
                    },
                    &mut context,
                )
                .expect("measured edge tooltip");
            let popup = shown.node(scope.element(1)).expect("source popup").rect;
            assert!(
                popup.x >= 8.0
                    && popup.y >= 8.0
                    && popup.right() <= 392.0
                    && popup.bottom() <= 192.0
            );
            assert!(popup.bottom() <= 170.0, "bottom edge flips above the owner");
            let label = shown.node(scope.element(2)).expect("tooltip label");
            let shaped = context
                .shape_static_text_wrapped(
                    "Long explanation with mixed text 地形 مرحبا and a complete ending.",
                    WidgetTheme::dark().typography.small_font_size,
                    FontRole::Ui,
                    Some(label.rect.width),
                )
                .expect("final tooltip text measurement");
            let visible = label
                .rect
                .intersection(label.inherited_clip)
                .expect("visible tooltip label");
            assert!(
                shaped.layout_height() <= visible.height + 0.5,
                "final wrapped text must fit inside the popup clip: {} > {}",
                shaped.layout_height(),
                visible.height,
            );
            let route = ActionRoute::<u8>::build(&tree, []).expect("passive tooltip tree");
            assert_eq!(route.focus_order(), edge_route.focus_order());
            assert_eq!(
                tree.find(scope.element(2))
                    .expect("complete tooltip source")
                    .kind,
                ElementKind::Text(
                    "Long explanation with mixed text 地形 مرحبا and a complete ending.".into()
                )
            );
        }
        let mut hidden = edge_source();
        hidden.children.clear();
        let hidden_route = ActionRoute::<u8>::build(&hidden, []).expect("hidden source");
        let hidden_layout = one
            .layout(
                &mut hidden,
                TooltipFrame {
                    scope: popup_scope,
                    viewport,
                    route: &hidden_route,
                    focus: Some(owner),
                    focus_root: None,
                    cursor: Point::new(380.0, 180.0),
                    active: true,
                    theme: WidgetTheme::dark(),
                    now: now + TOOLTIP_DELAY,
                },
                &mut context,
            )
            .expect("owner removal");
        assert!(hidden_layout.node(popup_scope.element(1)).is_none());
        assert!(one.deadline(now + TOOLTIP_DELAY).is_none());
        assert_eq!(
            two.timer
                .visible_owner(two.focused, now + TOOLTIP_DELAY, TOOLTIP_DELAY),
            Some(owner)
        );
    }
}
