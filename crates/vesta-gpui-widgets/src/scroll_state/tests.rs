//! Existing shared scrolling regressions, kept together without changing their cases.
use super::*;

#[test]
fn scroll_boundary_returns_wheel_remainder_for_an_ancestor() {
    reveal_margins_do_not_clip_nearly_full_targets();
    invalid_reveal_plans_preserve_all_ancestors();
    invalid_scroll_updates_preserve_the_retained_state();
    let mut stable = ScrollAreaState::default();
    stable.sync_content_height(1200.0, 200.0);
    stable.scroll_by(34.575012);
    let before = stable;
    assert!(!stable.reveal(
        Rect::new(0.0, 7.999985, 1.0, 19.0),
        Rect::new(0.0, 0.0, 200.0, 200.0),
        8.0
    ));
    assert_eq!(
        stable, before,
        "layout roundoff must not cause a redraw loop"
    );
    assert!(
        stable.reveal(
            Rect::new(0.0, 7.98, 1.0, 19.0),
            Rect::new(0.0, 0.0, 200.0, 200.0),
            8.0
        ),
        "a real small reveal must still move"
    );
    let mut inner = ScrollAreaState::default();
    inner.sync_content_height(220.0, 100.0);
    let mut outer = ScrollAreaState::default();
    outer.sync_content_height(500.0, 200.0);
    let remainder = inner.scroll_by(150.0);
    assert_eq!(inner.offset_y(), 120.0);
    assert_eq!(remainder, 30.0);
    assert_eq!(outer.scroll_by(remainder), 0.0);
    assert_eq!(outer.offset_y(), 30.0);

    let mut horizontal = ScrollAreaState::default();
    horizontal.sync_content_size(Size::new(420.0, 70.0), Size::new(200.0, 70.0));
    assert_eq!(horizontal.max(), Point::new(220.0, 0.0));
    assert_eq!(
        horizontal.scroll_by_point(Point::new(260.0, 25.0)),
        Point::new(40.0, 25.0)
    );
    assert_eq!(horizontal.offset(), Point::new(220.0, 0.0));
    horizontal.reveal(
        Rect::new(-20.0, 0.0, 30.0, 20.0),
        Rect::new(0.0, 0.0, 200.0, 70.0),
        8.0,
    );
    assert!(horizontal.offset().x < 220.0);

    // A nested focus reveal must offer the inner viewport, rather than the original
    // offscreen row, to the outer scroll state. Non-scroll ancestors are skipped.
    let outer_id = ElementId::new(1);
    let inner_id = ElementId::new(3);
    let inner_rect = Rect::new(0.0, 40.0, 100.0, 100.0);
    let row = Rect::new(0.0, 250.0, 100.0, 20.0);
    let steps = scroll_reveal_path(&[outer_id, ElementId::new(2), inner_id], row, |id| {
        if id == inner_id {
            Some(ScrollRevealViewport {
                rect: inner_rect,
                margin: 2.0,
            })
        } else if id == outer_id {
            Some(ScrollRevealViewport {
                rect: Rect::new(0.0, 0.0, 200.0, 200.0),
                margin: 8.0,
            })
        } else {
            None
        }
    });
    assert_eq!(steps.len(), 2);
    assert_eq!((steps[0].element, steps[0].target), (inner_id, row));
    assert_eq!((steps[1].element, steps[1].target), (outer_id, inner_rect));

    // Propagation itself is shared with hosts, including window/modal bounds.
    use vesta_gpui::{ActionRoute, Element, ElementKey, Style, UiWindowId};
    let leaf = ElementId::new(4);
    let blocked = ElementId::new(5);
    let root = Element::container(outer_id, ElementKey::new("outer"))
        .child(
            Element::container(inner_id, ElementKey::new("inner"))
                .child(Element::container(leaf, ElementKey::new("leaf"))),
        )
        .child(
            Element::container(blocked, ElementKey::new("disabled"))
                .styled(Style::default().action_boundary(true)),
        );
    let actions = ActionRoute::build(&root, [(leaf, ())]).expect("unique tree");
    let window = UiWindowId(10);
    let target = InputTarget {
        window,
        element: leaf,
    };
    let scope = FocusScope::new(window, &actions, None);
    inner.scroll_by(-inner.offset_y());
    outer.scroll_by(-outer.offset_y());
    let remaining = scroll_wheel_path(&scope, target, Point::new(40.0, 150.0), |id, mut delta| {
        if id == inner_id {
            delta.y = inner.scroll_by(delta.y);
        }
        if id == outer_id {
            delta.y = outer.scroll_by(delta.y);
        }
        delta
    });
    assert_eq!(remaining, Point::new(40.0, 0.0));
    assert_eq!((inner.offset_y(), outer.offset_y()), (120.0, 30.0));
    let modal_scope = FocusScope::new(window, &actions, Some(inner_id));
    let mut visited = Vec::new();
    scroll_wheel_path(&modal_scope, target, Point::new(0.0, 10.0), |id, delta| {
        visited.push(id);
        delta
    });
    assert_eq!(visited, vec![leaf, inner_id]);
    for invalid in [
        InputTarget {
            window: UiWindowId(11),
            ..target
        },
        InputTarget {
            element: blocked,
            ..target
        },
        InputTarget {
            element: ElementId::new(999),
            ..target
        },
        InputTarget {
            element: outer_id,
            ..target
        },
    ] {
        let movement = Point::new(0.0, 10.0);
        assert_eq!(
            scroll_wheel_path(&modal_scope, invalid, movement, |_, _| panic!(
                "invalid route must not mutate scroll"
            )),
            movement
        );
    }
    let mut calls = 0;
    scroll_wheel_path(&scope, target, Point::new(0.0, 10.0), |_, _| {
        calls += 1;
        Point::default()
    });
    assert_eq!(calls, 1, "fully consumed input cannot scroll an ancestor");
}

fn reveal_margins_do_not_clip_nearly_full_targets() {
    use vesta_gpui::{Dimension, Element, ElementKey, LayoutEngine, PositionMode, Style, Viewport};
    let mut failures = Vec::new();
    for flow in [FlowDirection::LeftToRight, FlowDirection::RightToLeft] {
        for large in [false, true] {
            let project = |offset: Point| {
                let mut target = Element::container(ElementId::new(3), ElementKey::new("target"))
                    .styled(Style::default().size(
                        Dimension::Px(if large { 158.0 } else { 30.0 }),
                        Dimension::Px(if large { 54.0 } else { 20.0 }),
                    ));
                target.style.position = PositionMode::Absolute;
                target.style.left = Some(if flow == FlowDirection::LeftToRight {
                    108.0
                } else {
                    238.0
                });
                target.style.top = Some(108.0);
                let root =
                    Element::scroll_box(ElementId::new(1), ElementKey::new("viewport"), offset)
                        .styled(
                            Style::default()
                                .size(Dimension::Px(174.0), Dimension::Px(70.0))
                                .flow_direction(flow)
                                .scrollbar(vesta_gpui::ScrollbarStyle {
                                    gutter: vesta_gpui::ScrollbarGutter::Reserve,
                                    ..vesta_gpui::ScrollbarStyle::default()
                                }),
                        )
                        .child(
                            Element::container(ElementId::new(2), ElementKey::new("content"))
                                .styled(
                                    Style::default()
                                        .size(Dimension::Px(500.0), Dimension::Px(500.0)),
                                )
                                .child(target),
                        );
                LayoutEngine
                    .compute(&root, Viewport::new(174.0, 70.0, 1.0))
                    .expect("margin projection")
            };
            let mut state = ScrollAreaState::new_2d(Point::new(100.0, 100.0));
            let before = project(state.offset());
            let viewport = before.node(ElementId::new(1)).expect("viewport");
            state.sync_layout(viewport);
            let rect = before.node(ElementId::new(3)).expect("target").rect;
            state.reveal(rect, viewport.scroll_viewport(), 8.0);
            let after = project(state.offset());
            let rect = after.node(ElementId::new(3)).expect("revealed target").rect;
            let visible = after
                .node(ElementId::new(1))
                .expect("current viewport")
                .scroll_viewport();
            println!(
                "GPS03-REVEAL-MARGIN flow={flow:?} large={large} target={rect:?} visible={visible:?} offset={:?}",
                state.offset()
            );
            if rect.x < visible.x
                || rect.y < visible.y
                || rect.right() > visible.right()
                || rect.bottom() > visible.bottom()
            {
                failures.push(format!("{flow:?}/{large}"));
            }
            if !large {
                assert_eq!(
                    state.offset(),
                    Point::new(
                        if flow == FlowDirection::LeftToRight {
                            100.0
                        } else {
                            106.0
                        },
                        100.0
                    )
                );
            }
        }
    }
    assert!(
        failures.is_empty(),
        "requested reveal margin clips target: {failures:?}"
    );
}

#[test]
fn thumb_drag_tracks_the_scroll_axis_and_clamps_at_the_content_edge() {
    let viewport = Rect::new(0.0, 0.0, 200.0, 70.0);
    let mut state = ScrollAreaState::default();
    state.sync_content_size(Size::new(420.0, 70.0), Size::new(200.0, 70.0));
    let (_, _, _, thumb) = scrollbar_geometry(viewport, Size::new(420.0, 70.0), state.offset());
    let thumb = thumb.expect("horizontal scrollbar thumb");
    let pointer = Point::new(thumb.x + 5.0, thumb.y + 3.0);
    let drag = state
        .begin_thumb_drag(viewport, pointer)
        .expect("thumb hit begins drag");
    assert_eq!(drag.axis(), ScrollAxis::Horizontal);
    let original = state;
    let mut invalid_updates = Vec::new();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for bad in [Point::new(value, pointer.y), Point::new(pointer.x, value)] {
            let changed = drag.update(&mut state, bad);
            if changed || state != original {
                invalid_updates.push(format!("{bad:?}: {state:?}"));
            }
            state = original;
        }
    }
    assert!(
        invalid_updates.is_empty(),
        "nonfinite thumb input must preserve the retained state: {invalid_updates:?}"
    );
    assert!(drag.update(&mut state, Point::new(pointer.x + 500.0, pointer.y)));
    assert_eq!(state.offset(), Point::new(220.0, 0.0));
    assert!(!drag.update(&mut state, Point::new(pointer.x + 600.0, pointer.y)));
    assert!(drag.update(&mut state, Point::new(pointer.x - 500.0, pointer.y)));
    assert_eq!(state.offset(), Point::new(0.0, 0.0));

    state.sync_content_size(Size::new(200.0, 280.0), Size::new(200.0, 70.0));
    let (_, thumb, _, _) = scrollbar_geometry(viewport, Size::new(200.0, 280.0), state.offset());
    let thumb = thumb.expect("vertical scrollbar thumb");
    let pointer = Point::new(thumb.x + 3.0, thumb.y + 3.0);
    let drag = state
        .begin_thumb_drag(viewport, pointer)
        .expect("vertical thumb hit");
    assert_eq!(drag.axis(), ScrollAxis::Vertical);
    assert!(drag.update(&mut state, Point::new(pointer.x, pointer.y + 500.0)));
    assert_eq!(state.offset(), Point::new(0.0, 210.0));

    let scroll = vesta_gpui::Element::scroll_box(
        ElementId::new(1),
        vesta_gpui::ElementKey::new("rtl"),
        Point::default(),
    )
    .styled(
        vesta_gpui::Style::default()
            .size(
                vesta_gpui::Dimension::Px(200.0),
                vesta_gpui::Dimension::Px(70.0),
            )
            .flow_direction(FlowDirection::RightToLeft)
            .scrollbar(vesta_gpui::ScrollbarStyle {
                gutter: vesta_gpui::ScrollbarGutter::Reserve,
                ..vesta_gpui::ScrollbarStyle::default()
            }),
    )
    .child(
        vesta_gpui::Element::container(ElementId::new(2), vesta_gpui::ElementKey::new("content"))
            .styled(vesta_gpui::Style::default().size(
                vesta_gpui::Dimension::Px(420.0),
                vesta_gpui::Dimension::Px(280.0),
            )),
    );
    let layout = vesta_gpui::LayoutEngine
        .compute(&scroll, vesta_gpui::Viewport::new(200.0, 70.0, 1.0))
        .expect("RTL geometry");
    let mut rtl = ScrollAreaState::default();
    assert!(rtl.sync_layout(layout.node(scroll.id).expect("scroll")));
    assert_eq!(
        rtl.scroll_by_point(Point::new(-300.0, 10.0)),
        Point::new(-70.0, 0.0)
    );
    assert_eq!(rtl.offset(), Point::new(230.0, 10.0));
    let node = layout.node(scroll.id).expect("reserved scroll");
    let before_invalid_layout = rtl;
    let mut invalid_layout = node.clone();
    invalid_layout.flow_direction = FlowDirection::LeftToRight;
    invalid_layout.scroll_content = Some((f32::NAN, 280.0));
    assert!(!rtl.sync_layout(&invalid_layout));
    assert_eq!(rtl, before_invalid_layout);
    assert_eq!(
        rtl.max().y,
        220.0,
        "the horizontal gutter changes vertical reach"
    );
    let (_, _, _, thumb) = node.scrollbar_geometry(rtl.offset());
    let thumb = thumb.expect("RTL thumb");
    let pointer = Point::new(thumb.x + 5.0, thumb.y + 3.0);
    let drag = rtl
        .begin_thumb_drag_in_layout(node, pointer)
        .expect("RTL thumb hit");
    assert!(drag.update(&mut rtl, Point::new(pointer.x + 500.0, pointer.y)));
    assert_eq!(
        rtl.offset().x,
        0.0,
        "moving right returns to the RTL logical start"
    );
    assert!(rtl.reveal(Rect::new(-100.0, 10.0, 30.0, 20.0), viewport, 2.0));
    assert!(
        rtl.offset().x > 0.0,
        "a physical target on the left advances RTL offset"
    );
    let (_, Some(thumb), _, _) = node.scrollbar_geometry(rtl.offset()) else {
        panic!("reserved vertical thumb");
    };
    let pointer = Point::new(thumb.x + thumb.width * 0.5, thumb.y + thumb.height * 0.5);
    let drag = rtl
        .begin_thumb_drag_in_layout(node, pointer)
        .expect("gutter hit");
    assert!(drag.update(&mut rtl, Point::new(pointer.x, pointer.y + 500.0)));
    assert_eq!(
        rtl.offset().y,
        rtl.max().y,
        "drag reaches the content beyond the bottom gutter"
    );
}

fn invalid_reveal_plans_preserve_all_ancestors() {
    let outer = ElementId::new(71);
    let inner = ElementId::new(73);
    let target = Rect::new(310.0, 340.0, 10.0, 10.0);
    let inner_view = ScrollRevealViewport {
        rect: Rect::new(140.0, 160.0, 100.0, 100.0),
        margin: 2.0,
    };
    let outer_view = ScrollRevealViewport {
        rect: Rect::new(0.0, 0.0, 100.0, 100.0),
        margin: 8.0,
    };
    let mut accepted = [ScrollAreaState::new_2d(Point::new(11.0, 13.0)); 2];
    for state in &mut accepted {
        state.sync_content_size(Size::new(800.0, 900.0), Size::new(100.0, 100.0));
    }
    let invalid_rects = [
        Rect::new(f32::NAN, 340.0, 10.0, 10.0),
        Rect::new(310.0, f32::INFINITY, 10.0, 10.0),
        Rect::new(310.0, 340.0, -10.0, 10.0),
        Rect::new(f32::MAX, 340.0, f32::MAX, 10.0),
    ];
    let mut cases = Vec::new();
    for invalid in invalid_rects {
        cases.push((invalid, None, inner_view, true));
        for owner in [inner, outer] {
            cases.push((
                target,
                Some(owner),
                ScrollRevealViewport {
                    rect: invalid,
                    margin: 2.0,
                },
                false,
            ));
        }
    }
    for margin in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for owner in [inner, outer] {
            cases.push((
                target,
                Some(owner),
                ScrollRevealViewport {
                    margin,
                    ..inner_view
                },
                false,
            ));
        }
    }
    let mut failures = Vec::new();
    for (target, invalid_owner, invalid_view, invalid_target) in cases {
        let mut calls = 0;
        let steps = scroll_reveal_path(&[outer, ElementId::new(72), inner], target, |id| {
            calls += 1;
            if Some(id) == invalid_owner {
                Some(invalid_view)
            } else if id == inner {
                Some(inner_view)
            } else if id == outer {
                Some(outer_view)
            } else {
                None
            }
        });
        let mut states = accepted;
        for step in &steps {
            states[usize::from(step.element == outer)].reveal(
                step.target,
                step.viewport.rect,
                step.viewport.margin,
            );
        }
        if !steps.is_empty() || states != accepted || (invalid_target && calls != 0) {
            failures.push(format!("target={target:?} owner={invalid_owner:?} steps={} states={states:?} calls={calls}", steps.len()));
        }
    }
    assert!(
        failures.is_empty(),
        "invalid nested reveal must preserve every ancestor: {failures:?}"
    );
    let caret = Rect::new(310.0, 340.0, 0.0, 0.0);
    let steps = scroll_reveal_path(&[outer, inner], caret, |id| {
        Some(if id == inner {
            ScrollRevealViewport {
                margin: -2.0,
                ..inner_view
            }
        } else {
            outer_view
        })
    });
    assert_eq!(
        steps.len(),
        2,
        "zero-size caret and finite negative margin remain valid"
    );
    assert_eq!((steps[0].target, steps[0].viewport.margin), (caret, -2.0));
    assert_eq!(steps[1].target, inner_view.rect);
}

fn invalid_scroll_updates_preserve_the_retained_state() {
    let mut accepted = ScrollAreaState::new_2d(Point::new(17.0, 19.0));
    accepted.sync_content_size(Size::new(420.0, 280.0), Size::new(200.0, 70.0));
    let mut failures = Vec::new();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let initial = ScrollAreaState::new_2d(Point::new(value, 19.0));
        if initial.offset() != Point::new(0.0, 19.0) || initial.max() != initial.offset() {
            failures.push(format!("initial {value:?}: {initial:?}"));
        }
        for delta in [Point::new(value, 40.0), Point::new(40.0, value)] {
            let mut state = accepted;
            let remainder = state.scroll_by_point(delta);
            if state != accepted
                || remainder.x.to_bits() != delta.x.to_bits()
                || remainder.y.to_bits() != delta.y.to_bits()
            {
                failures.push(format!("movement {delta:?}: {state:?}, {remainder:?}"));
            }
        }
        for size in [Size::new(value, 280.0), Size::new(420.0, value)] {
            let mut state = accepted;
            state.sync_content_size(size, Size::new(200.0, 70.0));
            if state != accepted {
                failures.push(format!("content {size:?}: {state:?}"));
            }
            state = accepted;
            state.sync_content_size(Size::new(420.0, 280.0), size);
            if state != accepted {
                failures.push(format!("visible {size:?}: {state:?}"));
            }
        }
        for (content, visible) in [(value, 70.0), (280.0, value)] {
            let mut state = accepted;
            state.sync_content_height(content, visible);
            if state != accepted {
                failures.push(format!("height {content:?}/{visible:?}: {state:?}"));
            }
        }
    }
    for (target, visible, margin) in [
        (
            Rect::new(f32::NAN, 250.0, 20.0, 20.0),
            Rect::new(0.0, 0.0, 200.0, 70.0),
            8.0,
        ),
        (
            Rect::new(250.0, 250.0, 20.0, 20.0),
            Rect::new(f32::INFINITY, 0.0, 200.0, 70.0),
            8.0,
        ),
        (
            Rect::new(250.0, 250.0, -20.0, 20.0),
            Rect::new(0.0, 0.0, 200.0, 70.0),
            8.0,
        ),
        (
            Rect::new(f32::MAX, 250.0, f32::MAX, 20.0),
            Rect::new(0.0, 0.0, 200.0, 70.0),
            8.0,
        ),
        (
            Rect::new(250.0, 250.0, 20.0, 20.0),
            Rect::new(0.0, 0.0, 200.0, 70.0),
            f32::NAN,
        ),
    ] {
        let mut state = accepted;
        if state.reveal(target, visible, margin) || state != accepted {
            failures.push(format!(
                "reveal {target:?}/{visible:?}/{margin:?}: {state:?}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "invalid scroll input must not corrupt finite retained state: {failures:?}"
    );
    let mut large = ScrollAreaState::default();
    large.sync_content_size(Size::new(f32::MAX, f32::MAX), Size::new(200.0, 70.0));
    large.scroll_by_point(Point::new(f32::MAX, f32::MAX));
    assert_eq!(large.offset(), large.max());
    assert_eq!(
        large.scroll_by_point(Point::new(-f32::MAX, -f32::MAX)),
        Point::default()
    );
    assert_eq!(large.offset(), Point::default());
}
