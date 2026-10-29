//! Public paint/admission feedback composites; no raster or exact-color snapshots.
use super::*;
use vesta_gpui::Style;

pub(super) fn ramp_marks_on_data_colors(luminance: impl Fn(vesta_gpui::Color) -> f64) {
    use vesta_gpui::{Color, ElementIdAllocator, HitTestTree, PaintCommand, PaintList};

    let band = ElementIdAllocator.allocate_scope().expect("ramp band");
    let mut unreadable = Vec::new();
    for theme in [WidgetTheme::dark(), WidgetTheme::light()] {
        for color in [
            Color::rgb8(0, 0, 0),
            Color::rgb8(255, 255, 255),
            theme.palette.text,
            Color {
                alpha: 0.5,
                ..theme.palette.text
            },
            Color {
                alpha: 0.0,
                ..theme.palette.text
            },
        ] {
            for selected in [false, true] {
                let widget = ramp_editor(
                    RampIds {
                        root: identity(1),
                        track: identity(2),
                        band,
                    },
                    vec![RampPoint {
                        id: 7,
                        root: identity(3),
                        label: identity(4),
                        position: 0.5,
                        color,
                        selected,
                        select_action: 7_u8,
                    }],
                    RampSpec::new(RampInterpolation::Linear, 220.0),
                    theme,
                )
                .expect("caller RGBA ramp");
                let layout = LayoutEngine
                    .compute(&widget.element, Viewport::new(240.0, 60.0, 1.25))
                    .expect("ramp layout");
                let bounds = layout.node(ElementId::new(3)).expect("handle").rect;
                assert_eq!(
                    (bounds.width, bounds.height),
                    (RAMP_POINT_WIDTH, RAMP_POINT_HEIGHT)
                );
                let paint = PaintList::build(&widget.element, &layout);
                for element in [ElementId::new(3), band.element(0)] {
                    assert!(
                        paint.commands().iter().any(|command| matches!(command,
                            PaintCommand::FillRoundedRect { element: id, color: fill, .. }
                                if *id == element && *fill == color
                        )),
                        "feedback must preserve original data color and alpha"
                    );
                }
                let (mark_bounds, mark) = paint
                    .commands()
                    .iter()
                    .find_map(|command| match command {
                        PaintCommand::Text {
                            element,
                            bounds,
                            color,
                            ..
                        } if *element == ElementId::new(4) => Some((*bounds, *color)),
                        _ => None,
                    })
                    .expect("visible control-point mark");
                let backing = paint.commands().iter().find_map(|command| match command {
                    PaintCommand::FillRoundedRect { element, color, .. }
                        if *element == ElementId::new(4) =>
                    {
                        Some(*color)
                    }
                    _ => None,
                });
                if backing.is_some() {
                    assert!(
                        bounds.contains_rect(mark_bounds) && mark_bounds.height < bounds.height,
                        "a marker backing must leave the caller's swatch visible"
                    );
                }
                let fill = backing.unwrap_or(color);
                let (fill_luma, mark_luma) = (luminance(fill), luminance(mark));
                let ratio = (fill_luma.max(mark_luma) + 0.05) / (fill_luma.min(mark_luma) + 0.05);
                if fill.alpha < 1.0 || ratio < 3.0 {
                    unreadable.push((theme.mode, color, selected, ratio));
                }
                let route =
                    ActionRoute::build(&widget.element, widget.actions).expect("ramp route");
                assert_eq!(route.action_for_hit(ElementId::new(4)), Some(&7));
                assert_eq!(route.focus_order(), &[ElementId::new(3)]);
                let hit = HitTestTree::build(&widget.element, &layout)
                    .hit(Point::new(
                        bounds.x + bounds.width / 2.0,
                        bounds.y + bounds.height / 2.0,
                    ))
                    .expect("marker must still hit its handle");
                assert_eq!(hit, ElementId::new(3));
                assert_eq!(
                    widget
                        .element
                        .find(ElementId::new(3))
                        .expect("point")
                        .semantic
                        .as_ref()
                        .expect("slider")
                        .selected,
                    Some(selected)
                );
            }
        }
    }
    assert!(
        unreadable.is_empty(),
        "unreadable ramp markers: {unreadable:?}"
    );
}

pub(super) fn disabled_controls_keep_visible_feedback_without_an_input_route() {
    use vesta_gpui::{HitTestTree, PaintList};
    let mut missing_feedback = Vec::new();
    for theme in [WidgetTheme::dark(), WidgetTheme::light()] {
        let mut compare = |name: &str, available: Widget<u8>, disabled: Widget<u8>| {
            let layout = LayoutEngine
                .compute(&available.element, Viewport::new(320.0, 100.0, 1.25))
                .expect("available control layout");
            let disabled_layout = LayoutEngine
                .compute(&disabled.element, Viewport::new(320.0, 100.0, 1.25))
                .expect("disabled control layout");
            for node in layout.nodes() {
                assert_eq!(
                    node.rect,
                    disabled_layout
                        .node(node.id)
                        .expect("disabled identity")
                        .rect,
                    "availability keeps {name}'s stable geometry"
                );
            }
            assert_eq!(
                available.element.semantic, disabled.element.semantic,
                "disabled feedback preserves the caller's value and selection"
            );
            let route =
                ActionRoute::build(&disabled.element, disabled.actions).expect("disabled route");
            assert!(route.focus_order().is_empty());
            assert!(route.action_at(disabled.element.id).is_none());
            let bounds = disabled_layout
                .node(disabled.element.id)
                .expect("root")
                .rect;
            let disabled_hit = HitTestTree::build(&disabled.element, &disabled_layout)
                .hit(Point::new(bounds.x + 1.0, bounds.y + 1.0));
            assert!(
                disabled_hit
                    .and_then(|hit| route.action_for_hit(hit))
                    .is_none(),
                "the disabled boundary may intercept a hit, but cannot activate"
            );
            if PaintList::build(&available.element, &layout)
                == PaintList::build(&disabled.element, &disabled_layout)
            {
                missing_feedback.push(format!("{:?} {name}", theme.mode));
            }
        };
        let combo = |state| {
            combo_box(
                ComboBoxIds {
                    root: identity(2000),
                    label: identity(2001),
                    arrow: identity(2002),
                },
                "Current choice",
                "Available value",
                false,
                1_u8,
                state,
                theme,
            )
        };
        compare(
            "ComboBox",
            combo(ControlState::Idle),
            combo(ControlState::Disabled),
        );
        for value in [BinaryState::Off, BinaryState::On, BinaryState::Mixed] {
            let project = |state| {
                checkbox(
                    CheckboxIds {
                        root: identity(2000),
                        box_identity: identity(2001),
                        mark: identity(2002),
                        label: identity(2003),
                    },
                    "Current choice",
                    value,
                    1_u8,
                    state,
                    theme,
                )
            };
            compare(
                &format!("Checkbox {value:?}"),
                project(ControlState::Idle),
                project(ControlState::Disabled),
            );
        }
        for selected in [false, true] {
            let project = |state| {
                radio(
                    RadioIds {
                        root: identity(2000),
                        ring: identity(2001),
                        dot: identity(2002),
                        label: identity(2003),
                    },
                    "Current choice",
                    selected,
                    1_u8,
                    state,
                    theme,
                )
            };
            compare(
                &format!("Radio {selected}"),
                project(ControlState::Idle),
                project(ControlState::Disabled),
            );
        }
        let range = |state| {
            slider(
                SliderIds {
                    root: identity(2000),
                    track: identity(2001),
                    fill: identity(2002),
                    thumb: identity(2003),
                    value_label: identity(2004),
                },
                0.52,
                1_u8,
                state,
                theme,
            )
        };
        compare(
            "Slider",
            range(ControlState::Idle),
            range(ControlState::Disabled),
        );
        let item = |state| {
            list_item(
                identity(2000),
                identity(2001),
                "Current choice",
                true,
                1_u8,
                state,
                theme,
            )
        };
        compare(
            "Selected List",
            item(ControlState::Idle),
            item(ControlState::Disabled),
        );
        let option = |state| {
            combo_option(
                identity(2000),
                identity(2001),
                "Current choice",
                true,
                1_u8,
                state,
                theme,
            )
        };
        compare(
            "Selected Combo option",
            option(ControlState::Idle),
            option(ControlState::Disabled),
        );
        let record = (42_u64, "Current choice");
        let content = |record: &(u64, &str)| {
            WidgetContent::new(
                record.1,
                Widget::from_element(
                    Element::text(
                        identity(2001).element,
                        identity(2001).key,
                        format!("Typed {}", record.0),
                    )
                    .styled(Style::default().text(17.0, vesta_gpui::Color::rgb8(123, 160, 190))),
                ),
            )
        };
        let distinguish_selection = |selected: &Widget<u8>, unselected: Widget<u8>| {
            let viewport = Viewport::new(320.0, 100.0, 1.25);
            let selected_layout = LayoutEngine
                .compute(&selected.element, viewport)
                .expect("selected disabled layout");
            let unselected_layout = LayoutEngine
                .compute(&unselected.element, viewport)
                .expect("unselected disabled layout");
            assert_ne!(
                PaintList::build(&selected.element, &selected_layout),
                PaintList::build(&unselected.element, &unselected_layout),
                "disabled feedback must preserve a painted persistent selection"
            );
        };
        let typed_item = |selected, state| {
            list_item_from(
                identity(2000),
                &record,
                selected,
                1_u8,
                state,
                theme,
                content,
            )
        };
        let available = typed_item(true, ControlState::Idle);
        let disabled = typed_item(true, ControlState::Disabled);
        assert_eq!(
            available.element.children, disabled.element.children,
            "the selectable shell cannot recolor caller-owned typed content"
        );
        distinguish_selection(&disabled, typed_item(false, ControlState::Disabled));
        compare("Selected typed List", available, disabled);
        let typed_option = |selected, state| {
            combo_option_with_content(
                identity(2000),
                content(&record),
                selected,
                1_u8,
                state,
                theme,
            )
        };
        let available = typed_option(true, ControlState::Idle);
        let disabled = typed_option(true, ControlState::Disabled);
        assert_eq!(
            available.element.children, disabled.element.children,
            "the option shell preserves caller-owned typed content"
        );
        distinguish_selection(&disabled, typed_option(false, ControlState::Disabled));
        compare("Selected typed Combo option", available, disabled);
        for has_children in [false, true] {
            let project = |control| {
                tree_view_row(
                    TreeViewRowIds {
                        root: identity(2000),
                        disclosure: identity(2001),
                        label: identity(2002),
                    },
                    "Current choice",
                    TreeViewRowState {
                        depth: 0,
                        has_children,
                        expanded: has_children,
                        selected: true,
                        control,
                    },
                    TreeViewRowActions {
                        select: 1_u8,
                        toggle: 2_u8,
                    },
                    theme,
                )
            };
            compare(
                &format!("Selected Tree children={has_children}"),
                project(ControlState::Idle),
                project(ControlState::Disabled),
            );
            let typed_tree = |selected, control| {
                tree_view_row_from(
                    TreeRowContentIds {
                        root: identity(2000),
                        disclosure: identity(2002),
                    },
                    &record,
                    TreeViewRowState {
                        depth: 0,
                        has_children,
                        expanded: has_children,
                        selected,
                        control,
                    },
                    TreeViewRowActions {
                        select: 1_u8,
                        toggle: 2_u8,
                    },
                    theme,
                    content,
                )
            };
            let available = typed_tree(true, ControlState::Idle);
            let disabled = typed_tree(true, ControlState::Disabled);
            assert_eq!(
                available.element.children.get(1),
                disabled.element.children.get(1),
                "the tree shell cannot restyle caller-owned typed content"
            );
            distinguish_selection(&disabled, typed_tree(false, ControlState::Disabled));
            compare(
                &format!("Selected typed Tree children={has_children}"),
                available,
                disabled,
            );
        }
    }
    assert!(
        missing_feedback.is_empty(),
        "controls that reject input must show disabled feedback: {missing_feedback:?}"
    );
}
