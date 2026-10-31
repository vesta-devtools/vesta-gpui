use vesta_gpui::{
    ActionRoute, Element, ElementId, ElementKey, FocusScope, InputTarget, InteractionCleanupStage,
    LayoutEngine, Menu, MenuId, MenuItem, MenuItemId, Point, Rect, TextInputContext, UiWindowId,
    Viewport, finish_window_interaction,
};
use vesta_gpui_widgets::*;

fn identity(id: u64) -> WidgetIdentity {
    WidgetIdentity::new(ElementId::new(id), format!("consumer.{id}"))
}
fn target(window: u64, id: u64) -> InputTarget {
    InputTarget {
        window: UiWindowId(window),
        element: ElementId::new(id),
    }
}

#[path = "composite_interactions/errors.rs"]
mod errors;
#[path = "composite_interactions/feedback.rs"]
mod feedback;
#[path = "composite_interactions/validation.rs"]
mod validation;

#[test]
fn notification_queue_and_focus_preserve_bounds_deadlines_and_window_ownership() {
    use std::time::{Duration, Instant};
    use vesta_gpui::{Dimension, ElementKind, Style, text_intrinsic_size};

    let now = Instant::now();
    let notice = |id, expires_at| ToastNotice {
        id: ToastId(id),
        message: format!("Notice {id}"),
        tone: StatusTone::Information,
        action_label: Some("Increment".to_owned()),
        action: Some(30_u8),
        expires_at,
    };
    let mut queue = ToastQueue::default();
    for (id, seconds) in [(101, 20), (102, 4), (103, 12), (104, 9)] {
        queue
            .push(notice(id, now + Duration::from_secs(seconds)), now)
            .expect("bounded notice");
    }
    assert_eq!(
        queue.push(notice(101, now + Duration::from_secs(30)), now),
        Err(ToastQueueError::Duplicate(ToastId(101)))
    );
    assert_eq!(
        queue.push(notice(105, now + Duration::from_secs(30)), now),
        Err(ToastQueueError::Full)
    );
    assert_eq!(queue.next_deadline(), Some(now + Duration::from_secs(4)));
    assert!(queue.expire(now + Duration::from_secs(4)));
    assert_eq!(queue.current().map(|notice| notice.id), Some(ToastId(101)));
    assert_eq!(queue.next_deadline(), Some(now + Duration::from_secs(9)));
    assert_eq!(
        queue.push(
            notice(105, now + Duration::from_secs(4)),
            now + Duration::from_secs(4)
        ),
        Err(ToastQueueError::InvalidTimeout)
    );

    let active = queue.current().expect("live front notice");
    let theme = WidgetTheme::dark();
    let projection = |window, opener_state| {
        let base = window * 100;
        let root = identity(base);
        Widget::from_element(
            Element::container(root.element, root.key)
                .styled(Style::default().size(Dimension::Px(360.0), Dimension::Px(160.0))),
        )
        .child(button(
            identity(base + 1),
            identity(base + 2),
            "Open",
            10_u8,
            opener_state,
            theme,
        ))
        .child(button(
            identity(base + 3),
            identity(base + 4),
            "Other",
            20_u8,
            ControlState::Idle,
            theme,
        ))
        .child(toast_widget(
            ToastIds {
                root: identity(base + 10),
                text: identity(base + 11),
                action: identity(base + 12),
                action_label: identity(base + 13),
                dismiss: identity(base + 14),
                dismiss_label: identity(base + 15),
            },
            active,
            40_u8,
            theme,
        ))
    };
    let mut focus = NotificationFocusState::default();
    // Native blur clears live ownership before another window creates a notice.
    // Retain a notification-only return snapshot, rather than keeping focus live.
    let mut floating_input = vesta_gpui::InputOwnership::default();
    floating_input.set_focus(Some(target(2, 201)));
    focus.observe_window_activation(UiWindowId(2), false, floating_input.focus());
    floating_input.set_focus(None);
    assert_eq!(floating_input.focus(), None);
    focus.record(active.id, active.expires_at, [target(1, 101)]);
    for window in [1, 2] {
        // Returning to interact with the notice clears only the inactive
        // fallback, not the opener already captured for this notice.
        focus.observe_window_activation(UiWindowId(window), true, None);
        let base = window * 100;
        let widget = projection(window, ControlState::Idle);
        let layout = LayoutEngine
            .compute(&widget.element, Viewport::new(360.0, 160.0, 1.0))
            .expect("notice composition layout");
        let label = widget
            .element
            .find(ElementId::new(base + 15))
            .expect("dismiss label");
        let ElementKind::Text(caption) = &label.kind else {
            panic!("dismiss caption")
        };
        let desired = text_intrinsic_size(
            caption,
            label.style.visual.font_size,
            label.style.visual.font_role,
        );
        assert!(
            layout.node(label.id).expect("label geometry").rect.width >= desired.width,
            "compact dismissal must reserve glyph space inside its hit frame"
        );
        let route = ActionRoute::build(&widget.element, widget.actions).expect("notice route");
        let scope = FocusScope::new(UiWindowId(window), &route, None);
        let root = ElementId::new(base + 10);
        assert_eq!(
            focus.return_target(
                active.id,
                UiWindowId(window),
                Some(target(window, base + 12)),
                root,
                &scope
            ),
            Some(target(window, base + 1))
        );
        assert_eq!(
            focus.return_target(
                active.id,
                UiWindowId(window),
                Some(target(window, base + 3)),
                root,
                &scope
            ),
            None,
            "expiry must not steal outside focus"
        );
        assert_eq!(
            focus.return_target(
                active.id,
                UiWindowId(window),
                Some(target(3 - window, (3 - window) * 100 + 12)),
                root,
                &scope
            ),
            None,
            "foreign notice cannot restore this window"
        );
        let modal = FocusScope::new(UiWindowId(window), &route, Some(root));
        assert_eq!(
            focus.return_target(
                active.id,
                UiWindowId(window),
                Some(target(window, base + 12)),
                root,
                &modal
            ),
            None,
            "a modal boundary rejects an outside opener"
        );
        let disabled = projection(window, ControlState::Disabled);
        let disabled_route =
            ActionRoute::build(&disabled.element, disabled.actions).expect("rebuilt route");
        let scope = FocusScope::new(UiWindowId(window), &disabled_route, None);
        assert_eq!(
            focus.return_target(
                active.id,
                UiWindowId(window),
                Some(target(window, base + 12)),
                root,
                &scope
            ),
            None,
            "disabled opener cannot regain focus"
        );
    }
    focus.observe_window_activation(UiWindowId(2), false, Some(target(2, 201)));
    focus.record(ToastId(105), active.expires_at, [target(2, 203)]);
    assert_eq!(
        focus.opener(ToastId(105), UiWindowId(2)),
        Some(target(2, 203))
    );
    focus.observe_window_activation(UiWindowId(2), true, None);
    focus.record(ToastId(106), active.expires_at, []);
    assert_eq!(focus.opener(ToastId(106), UiWindowId(2)), None);
    focus.observe_window_activation(UiWindowId(2), false, Some(target(1, 101)));
    focus.record(ToastId(107), active.expires_at, []);
    assert_eq!(focus.opener(ToastId(107), UiWindowId(2)), None);
    focus.observe_window_activation(UiWindowId(2), false, Some(target(2, 201)));
    focus.record(ToastId(108), active.expires_at, []);
    assert_eq!(
        focus.opener(ToastId(108), UiWindowId(2)),
        Some(target(2, 201))
    );
    focus.forget_window(UiWindowId(2));
    assert_eq!(focus.opener(ToastId(105), UiWindowId(2)), None);
    assert_eq!(focus.opener(ToastId(108), UiWindowId(2)), None);
    focus.record(ToastId(109), active.expires_at, []);
    assert_eq!(focus.opener(ToastId(109), UiWindowId(2)), None);
    focus.prune(now + Duration::from_secs(20));
    assert!(focus.is_empty());
    assert!(queue.expire(now + Duration::from_secs(20)));
    assert!(queue.is_empty());
}

#[test]
fn basic_command_and_radio_states_survive_busy_rebuild_and_choice_changes() {
    feedback::disabled_controls_keep_visible_feedback_without_an_input_route();
    use vesta_gpui::{
        InputOwnership, KeyChord, KeyModifiers, KeyStroke, PointerButton, PointerClick,
    };
    // The stock light accent equals its ordinary text color: reusing that
    // caption made an enabled Primary command visually nameless. Check the
    // readability boundary, not fixed palette values or rasterized pixels.
    let luminance = |color: vesta_gpui::Color| {
        let linear = |channel: f32| {
            let channel = f64::from(channel);
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.red) + 0.7152 * linear(color.green) + 0.0722 * linear(color.blue)
    };
    feedback::ramp_marks_on_data_colors(luminance);
    for theme in [
        WidgetTheme::dark(),
        WidgetTheme::light(),
        WidgetTheme::light().with_accent(
            vesta_gpui::Color::rgb8(235, 220, 90),
            vesta_gpui::Color::rgb8(250, 240, 135),
        ),
    ] {
        for tone in [ButtonTone::Primary, ButtonTone::Danger] {
            for state in [
                ControlState::Idle,
                ControlState::Hovered,
                ControlState::Pressed,
            ] {
                let widget = button_with_presentation(
                    identity(1),
                    identity(2),
                    "Run",
                    10_u8,
                    state,
                    ButtonPresentation {
                        tone,
                        loading: false,
                    },
                    theme,
                );
                let fill = widget
                    .element
                    .style
                    .visual
                    .background
                    .expect("command fill");
                let caption = widget.element.children[0].style.visual.text_color;
                let (fill, caption) = (luminance(fill), luminance(caption));
                let ratio = (fill.max(caption) + 0.05) / (fill.min(caption) + 0.05);
                assert!(ratio >= 4.5, "enabled {tone:?} caption contrast {ratio}");
                assert_eq!(
                    widget.element.children[0].kind,
                    vesta_gpui::ElementKind::Text("Run".into())
                );
                let route = ActionRoute::build(&widget.element, widget.actions)
                    .expect("legible command route");
                assert_eq!(route.action_for_hit(ElementId::new(2)), Some(&10));
            }
        }
        for value in [BinaryState::On, BinaryState::Mixed] {
            let widget = checkbox(
                CheckboxIds {
                    root: identity(10),
                    box_identity: identity(11),
                    mark: identity(12),
                    label: identity(13),
                },
                "Selected value",
                value,
                20_u8,
                ControlState::Idle,
                theme,
            );
            let fill = luminance(
                widget
                    .element
                    .find(ElementId::new(11))
                    .expect("box")
                    .style
                    .visual
                    .background
                    .expect("box fill"),
            );
            let mark = luminance(
                widget
                    .element
                    .find(ElementId::new(12))
                    .expect("mark")
                    .style
                    .visual
                    .background
                    .expect("mark fill"),
            );
            let ratio = (fill.max(mark) + 0.05) / (fill.min(mark) + 0.05);
            assert!(
                ratio >= 3.0,
                "enabled {value:?} state mark contrast {ratio}"
            );
            let route =
                ActionRoute::build(&widget.element, widget.actions).expect("checkbox route");
            assert_eq!(route.action_for_hit(ElementId::new(10)), Some(&20));
        }
        for checked in [false, true] {
            let widget = toggle(
                ToggleIds {
                    root: identity(20),
                    track: identity(21),
                    knob: identity(22),
                    label: identity(23),
                },
                "Current choice",
                checked,
                30_u8,
                ControlState::Idle,
                theme,
            );
            let fill = luminance(
                widget
                    .element
                    .find(ElementId::new(21))
                    .expect("track")
                    .style
                    .visual
                    .background
                    .expect("track fill"),
            );
            let mark = luminance(
                widget
                    .element
                    .find(ElementId::new(22))
                    .expect("knob")
                    .style
                    .visual
                    .background
                    .expect("knob fill"),
            );
            let ratio = (fill.max(mark) + 0.05) / (fill.min(mark) + 0.05);
            assert!(
                ratio >= 3.0,
                "enabled toggle {checked} knob contrast {ratio}"
            );
            let route = ActionRoute::build(&widget.element, widget.actions).expect("toggle route");
            assert_eq!(route.action_for_hit(ElementId::new(20)), Some(&30));
        }
    }
    let mut click = PointerClick::default();
    for loading in [false, true, false] {
        let widget = button_with_presentation(
            identity(1),
            identity(2),
            "Run",
            10_u8,
            ControlState::Idle,
            ButtonPresentation {
                tone: ButtonTone::Primary,
                loading,
            },
            WidgetTheme::dark(),
        );
        assert_eq!(
            widget
                .element
                .semantic
                .as_ref()
                .expect("button semantics")
                .busy,
            loading
        );
        assert_eq!(
            widget
                .element
                .semantic
                .as_ref()
                .expect("button semantics")
                .label,
            "Run",
            "busy decoration must not replace the caller's accessible name"
        );
        let route = ActionRoute::build(&widget.element, widget.actions).expect("command route");
        assert_eq!(
            route.action_at(ElementId::new(1)),
            (!loading).then_some(&10)
        );
        assert_eq!(route.focus_order().is_empty(), loading);
        assert!(
            click
                .release(
                    UiWindowId(1),
                    PointerButton::Primary,
                    &route,
                    Some(ElementId::new(1))
                )
                .is_none(),
            "a busy transition must retire the pending click"
        );
        if !loading {
            click.press(
                UiWindowId(1),
                PointerButton::Primary,
                &route,
                Some(ElementId::new(1)),
            );
        }
    }
    let option = |value, disabled| {
        let base = 10 + u64::from(value) * 4;
        RadioGroupOption {
            value,
            ids: RadioIds {
                root: identity(base),
                ring: identity(base + 1),
                dot: identity(base + 2),
                label: identity(base + 3),
            },
            label: format!("Choice {value}"),
            action: value,
            state: if disabled {
                ControlState::Disabled
            } else {
                ControlState::Idle
            },
        }
    };
    for (order, selected, disabled, tab_stop) in [
        ([10_u8, 20, 30], 20, None, 20_u8),
        ([30, 20, 10], 20, Some(20), 30),
        ([30, 10, 20], 10, Some(20), 10),
    ] {
        let group = radio_group(
            identity(200),
            "Options",
            Some(&selected),
            order
                .into_iter()
                .map(|value| option(value, disabled == Some(value)))
                .collect(),
            WidgetTheme::dark(),
        )
        .expect("distinct stable values");
        let route = ActionRoute::build(&group.element, group.actions).expect("radio group route");
        assert_eq!(
            route.focus_order(),
            &[ElementId::new(10 + u64::from(tab_stop) * 4)]
        );
        let enabled = order
            .into_iter()
            .filter(|value| Some(*value) != disabled)
            .collect::<Vec<_>>();
        let current = enabled[enabled.len() - 1];
        let decision = radio_group_key_action(
            &enabled,
            Some(&current),
            KeyChord::new(KeyStroke::ArrowRight, KeyModifiers::default()),
        );
        assert_eq!(decision, Some(ChoiceKeyAction::Highlight(enabled[0])));
        assert!(
            radio_group_key_action(
                &enabled,
                Some(&current),
                KeyChord::new(KeyStroke::Tab, KeyModifiers::default())
            )
            .is_none()
        );
        assert!(
            radio_group_key_action(
                &enabled,
                Some(&current),
                KeyChord::new(
                    KeyStroke::ArrowLeft,
                    KeyModifiers {
                        control: true,
                        ..Default::default()
                    }
                )
            )
            .is_none()
        );
        let layout = LayoutEngine
            .compute(&group.element, Viewport::new(320.0, 200.0, 1.25))
            .expect("group geometry");
        for value in order {
            let id = ElementId::new(10 + u64::from(value) * 4);
            let rect = layout.node(id).expect("stable choice").rect;
            let hit = vesta_gpui::HitTestTree::build(&group.element, &layout)
                .hit(Point::new(rect.x + 1.0, rect.y + 1.0))
                .expect("choice hit");
            assert_eq!(
                route.action_for_hit(hit),
                (Some(value) != disabled).then_some(&value)
            );
        }
        let mut input = InputOwnership::default();
        input.set_focus(Some(target(2, 50)));
        assert!(!FocusScope::new(UiWindowId(1), &route, None).accepts(target(2, 50)));
        assert_eq!(
            input.focus(),
            Some(target(2, 50)),
            "another source keeps its owner"
        );
    }
    assert!(matches!(
        radio_group(
            identity(200),
            "duplicate",
            Some(&10),
            vec![option(10, false), option(10, false)],
            WidgetTheme::dark()
        ),
        Err(RadioGroupError::DuplicateValue(_))
    ));
    for (theme, state) in [WidgetTheme::dark(), WidgetTheme::light()]
        .into_iter()
        .flat_map(|theme| {
            [
                ProgressState::Determinate(0.52),
                ProgressState::Indeterminate { phase: 0.25 },
                ProgressState::Indeterminate { phase: 0.75 },
                ProgressState::Determinate(1.0),
            ]
            .into_iter()
            .map(move |state| (theme, state))
        })
    {
        let progress =
            progress_bar_state(identity(300), identity(301), identity(302), state, theme)
                .expect("explicit progress state");
        let caption = luminance(progress.children[1].style.visual.text_color);
        for background in [
            progress.style.visual.background.expect("progress track"),
            progress.children[0]
                .style
                .visual
                .background
                .expect("progress fill"),
        ] {
            let background = luminance(background);
            let ratio = (caption.max(background) + 0.05) / (caption.min(background) + 0.05);
            assert!(
                ratio >= 4.5,
                "progress caption across fill/track contrast {ratio}"
            );
        }
        let semantic = progress.semantic.as_ref().expect("progress semantics");
        let active = matches!(state, ProgressState::Indeterminate { .. });
        assert_eq!(semantic.busy, active);
        assert_eq!(semantic.numeric.is_none(), active);
        assert_eq!(
            semantic.value.is_none(),
            active,
            "unknown work never fabricates a percentage"
        );
        let route = ActionRoute::<u8>::build(&progress, []).expect("passive progress route");
        assert!(route.focus_order().is_empty());
        let layout = LayoutEngine
            .compute(&progress, Viewport::new(320.0, 200.0, 1.0))
            .expect("progress layout");
        assert!(
            vesta_gpui::HitTestTree::build(&progress, &layout)
                .hit(Point::new(10.0, 10.0))
                .is_none()
        );
    }
    for (state, error) in [
        (
            ProgressState::Determinate(f32::NAN),
            ProgressStateError::NonFiniteValue,
        ),
        (
            ProgressState::Indeterminate {
                phase: f32::INFINITY,
            },
            ProgressStateError::NonFinitePhase,
        ),
    ] {
        assert_eq!(
            progress_bar_state(
                identity(300),
                identity(301),
                identity(302),
                state,
                WidgetTheme::dark()
            ),
            Err(error)
        );
    }
}

#[test]
fn composite_sessions_reject_foreign_events_and_restore_before_text_exit() {
    property_set_preserves_host_content_and_section_admission();
    validation::preserves_host_context_content_and_admission();
    let mut editor = TemporaryEditorState::default();
    assert!(editor.open_in(
        UiWindowId(7),
        Some(target(7, 3)),
        "borrowed search".to_owned()
    ));
    assert!(!editor.open_in(UiWindowId(8), Some(target(8, 4)), "replacement".to_owned()));
    assert!(
        editor
            .finish(UiWindowId(8), PopupCloseReason::Confirm)
            .is_none()
    );
    let exit = editor
        .finish(UiWindowId(7), PopupCloseReason::WindowUnavailable)
        .expect("source close");
    assert_eq!(exit.saved, "borrowed search");
    assert!(!exit.commit);
    assert!(exit.dismissal.focus_request().is_none());
    assert!(
        editor
            .finish(UiWindowId(7), PopupCloseReason::Confirm)
            .is_none()
    );
    let mut stack = ModalStack::default();
    stack
        .open_in(
            UiWindowId(7),
            1_u8,
            Some(target(7, 3)),
            ModalDismissPolicy::default(),
        )
        .expect("parent");
    stack
        .open_in(
            UiWindowId(7),
            2,
            Some(target(7, 4)),
            ModalDismissPolicy::default(),
        )
        .expect("child");
    assert!(
        stack
            .dismiss(UiWindowId(7), 1, PopupCloseReason::Confirm)
            .is_none()
    );
    assert!(
        stack
            .dismiss(UiWindowId(8), 2, PopupCloseReason::Confirm)
            .is_none()
    );
    assert!(
        stack
            .dismiss(UiWindowId(7), 2, PopupCloseReason::OutsidePress)
            .is_none()
    );
    assert_eq!(
        stack
            .dismiss(UiWindowId(7), 2, PopupCloseReason::Confirm)
            .expect("top close")
            .focus_request(),
        Some(target(7, 4))
    );
    assert_eq!(stack.top(), Some(1));
    let mut value = 42;
    let mut draft = "42".to_owned();
    finish_window_interaction(|stage| match stage {
        InteractionCleanupStage::CancelPreviews => {
            value = 3;
            draft = "3".into();
        }
        InteractionCleanupStage::EndTextSession => {
            value = draft.parse().expect("restored draft");
        }
        _ => {}
    });
    assert_eq!(value, 3, "blur must not republish the cancelled preview");
}

fn property_set_preserves_host_content_and_section_admission() {
    for (visible, enabled, expanded) in [
        (true, true, true),
        (true, true, false),
        (true, false, true),
        (false, true, true),
    ] {
        let set = property_set(
            PropertySetIds {
                root: identity(800),
                title: identity(801),
                description: identity(802),
                scroll: identity(803),
                empty: identity(804),
            },
            PropertySetSpec {
                title: "Node parameters".into(),
                description: Some("Exact selected revision".into()),
                empty_text: "Select a node".into(),
                scroll_offset: Point::new(0.0, 11.0),
                sections: vec![PropertySetSection {
                    ids: CollapsibleIds {
                        root: identity(810),
                        header: identity(811),
                        disclosure: identity(812),
                        label: identity(813),
                    },
                    title: "Transform".into(),
                    expanded,
                    toggle_action: 1_u8,
                    availability: WidgetAvailability { visible, enabled },
                    content: button(
                        identity(820),
                        identity(821),
                        "Host value",
                        2_u8,
                        ControlState::Idle,
                        WidgetTheme::dark(),
                    ),
                }],
            },
            WidgetTheme::dark(),
        );
        let route =
            ActionRoute::build(&set.element, set.actions).expect("stable composite identities");
        assert_eq!(
            route.action_at(ElementId::new(811)),
            (visible && enabled).then_some(&1)
        );
        assert_eq!(
            route.action_at(ElementId::new(820)),
            (visible && enabled && expanded).then_some(&2)
        );
        assert_eq!(
            set.element.find(ElementId::new(820)).is_some(),
            visible && expanded
        );
        assert_eq!(set.element.find(ElementId::new(804)).is_some(), !visible);
        assert!(
            set.element.find(ElementId::new(803)).is_some(),
            "the scroll owner remains stable across selection/group changes"
        );
    }
}

#[test]
fn stable_collection_repair_and_group_edits_survive_reordering_and_removal() {
    assert_eq!(
        collection_focus_after_change(&[10, 20, 30], &[30, 10], &20, None),
        Some(30)
    );
    assert_eq!(
        collection_focus_after_change(&[10, 20, 30], &[30, 10], &20, Some(&10)),
        Some(10)
    );
    assert_eq!(
        collection_focus_after_change(&[10, 20, 30], &[30, 20, 10], &20, None),
        Some(20)
    );
    assert_eq!(collection_focus_after_change(&[10], &[], &10, None), None);
    let edits =
        ramp_group_translation(&[(10, 0.2), (20, 0.7)], &[20, 10], &[20, 10], 0.8).expect("group");
    assert!((edits[0].1 - 0.5).abs() < 0.0001 && edits[1].1 == 1.0);
    assert!(ramp_group_translation(&[(10, 0.2), (20, 0.7)], &[10, 20], &[10], 0.1).is_none());
    assert!(ramp_group_translation(&[(10, -2.0)], &[10], &[10], 0.1).is_none());
    let color = [45, 128, 210, 90];
    assert_eq!(parse_rgba_hex(&format_rgba_hex(color)), Some(color));
    let round_trip = hsva_to_rgba(rgba_to_hsva(color));
    assert!(
        color
            .iter()
            .zip(round_trip)
            .all(|(a, b)| (i16::from(*a) - i16::from(b)).abs() <= 2)
    );
    assert_eq!(
        color_channel_step(color, ColorPickerMode::Hsv, 3, -100).expect("alpha")[3],
        0
    );
}

#[test]
fn measured_menu_keeps_actions_and_reveals_the_highlight_in_a_small_window() {
    let mut text = TextInputContext::default();
    let valid = Rect::new(0.0, 0.0, 240.0, 110.0);
    let one = Menu::new(MenuId(1), "Actions").item(MenuItem::new(MenuItemId(1), "Run", ()));
    for bad in [
        Rect::new(f32::NAN, 0.0, 10.0, 10.0),
        Rect::new(0.0, f32::INFINITY, 10.0, 10.0),
        Rect::new(f32::MAX, 0.0, f32::MAX, 10.0),
        Rect::new(f32::MAX, 0.0, 1.0, 10.0),
        Rect::new(0.0, 0.0, -1.0, 10.0),
    ] {
        for (bounds, anchor) in [(bad, valid), (valid, bad)] {
            let rejected: Result<Widget<()>, _> = menu_popup(
                &one,
                &MenuController::default(),
                MenuPopupSpec {
                    root: identity(1),
                    content: identity(2),
                    anchor,
                    bounds,
                    direction: PopupDirection::Below,
                    z_index: 90,
                    hovered: None,
                },
                WidgetTheme::dark(),
                &mut text,
                |_| panic!("invalid popup geometry reached an identity adapter"),
                |_| panic!("invalid popup geometry reached an action adapter"),
            );
            assert!(matches!(
                rejected,
                Err(vesta_gpui::LayoutError::InvalidConstraint)
            ));
        }
    }
    assert!(text.register_font(std::sync::Arc::new(
        include_bytes!("../../vesta-gpui/tests/fixtures/MaterialIcons-Regular.ttf").to_vec()
    )));
    text.set_font_stack(vesta_gpui::FontRole::Ui, "Material Icons");
    let mut menu = Menu {
        id: MenuId(1),
        label: "Actions".into(),
        position: None,
        items: (0..12)
            .map(|index| {
                MenuItem::new(
                    MenuItemId(index),
                    format!("Command {index} · shortcut"),
                    index,
                )
                .enabled(index != 1)
                .icon("fixture/menu-action")
            })
            .collect(),
    };
    let mut controller = MenuController::default();
    menu.items.insert(
        0,
        MenuItem::section_header(MenuItemId(20), "Context and session inspection"),
    );
    menu.items.insert(7, MenuItem::separator(MenuItemId(21)));
    for item in &mut menu.items {
        match item.id {
            MenuItemId(9) => item.tooltip = Some(String::new()),
            MenuItemId(10) => item.tooltip = Some("Host details beyond the visible label".into()),
            MenuItemId(11) => {
                item.label = "Command 11 · complete long label for a constrained menu".into();
            }
            _ => {}
        }
    }
    controller.open_in(UiWindowId(7), menu.id, None);
    controller.select_edge(&menu, true);
    menu.items
        .iter_mut()
        .find(|item| item.id == MenuItemId(11))
        .expect("stable last command")
        .submenu = Some(MenuId(2));
    controller.enter_submenu(MenuId(2));
    controller.highlight(Some(MenuItemId(90)));
    assert_eq!(controller.highlighted_in(menu.id), Some(MenuItemId(11)));
    let widget = menu_popup(
        &menu,
        &controller,
        MenuPopupSpec {
            root: identity(1),
            content: identity(2),
            anchor: Rect::new(20.0, 20.0, 0.0, 0.0),
            bounds: Rect::new(4.0, 4.0, 240.0, 110.0),
            direction: PopupDirection::Below,
            z_index: 90,
            hovered: None,
        },
        WidgetTheme::dark(),
        &mut text,
        |item| {
            let base = 100 + item.0 * 5;
            MenuItemIds {
                root: identity(base),
                check: identity(base + 1),
                label: identity(base + 2),
                shortcut: identity(base + 3),
                arrow: identity(base + 4),
            }
        },
        |item| item.action,
    )
    .expect("shared popup");
    let icon = widget
        .element
        .find(ElementId::new(156))
        .expect("leading icon keeps its identity");
    assert!(
        matches!(&icon.kind, vesta_gpui::ElementKind::Image(source) if source == "fixture/menu-action")
    );
    let complete_label = menu
        .items
        .iter()
        .find(|item| item.id == MenuItemId(11))
        .expect("long command")
        .label
        .clone();
    for (id, hint) in [
        (145, ""),
        (150, "Host details beyond the visible label"),
        (155, complete_label.as_str()),
    ] {
        assert_eq!(
            widget
                .element
                .find(ElementId::new(id))
                .expect("command row")
                .tooltip
                .as_deref(),
            Some(hint),
            "menu metadata preserves explicit overrides and the default complete label"
        );
    }
    let route = ActionRoute::build(&widget.element, widget.actions.clone()).expect("route");
    assert_eq!(route.action_at(ElementId::new(155)), Some(&11));
    assert!(route.action_at(ElementId::new(105)).is_none());
    for section in [ElementId::new(200), ElementId::new(205)] {
        assert!(route.action_for_hit(section).is_none());
        assert!(!route.focus_order().contains(&section));
    }
    let heading = &widget.element.children[0].children[0];
    assert_eq!(
        heading
            .semantic
            .as_ref()
            .map(|semantic| (&semantic.role, semantic.label.as_str())),
        Some((
            &vesta_gpui::SemanticRole::Label,
            "Context and session inspection"
        ))
    );
    let tree = Element::container(ElementId::new(400), ElementKey::new("consumer.root"))
        .styled(vesta_gpui::Style::default().size(
            vesta_gpui::Dimension::Percent(1.0),
            vesta_gpui::Dimension::Percent(1.0),
        ))
        .child(widget.element.clone());
    let layout = LayoutEngine
        .compute_with_text_context(&tree, Viewport::new(260.0, 130.0, 1.0), &mut text)
        .expect("layout");
    let popup = layout.node(ElementId::new(1)).expect("popup").rect;
    let last = layout.node(ElementId::new(155)).expect("last row").rect;
    assert!(popup.x >= 4.0 && popup.x + popup.width <= 244.1);
    assert!(last.y >= popup.y && last.y + last.height <= popup.y + popup.height + 0.1);
    let label = layout
        .node(ElementId::new(157))
        .expect("constrained command label");
    let visible_label = label
        .rect
        .intersection(label.inherited_clip)
        .expect("partially visible command label");
    assert!(
        text.shape_static_text_wrapped(
            &complete_label,
            WidgetTheme::dark().typography.font_size,
            vesta_gpui::FontRole::Ui,
            None,
        )
        .expect("full label shaping")
        .layout_width()
            > visible_label.width,
        "the default hint protects an actually clipped label"
    );
    let hint_scope = vesta_gpui::ElementIdAllocator
        .allocate_scope()
        .expect("hint scope");
    let full_route = ActionRoute::build(&tree, widget.actions.clone()).expect("consumer route");
    let mut with_hint = tree.clone();
    let mut hints = TooltipController::new(std::time::Duration::from_secs(1));
    hints
        .layout(
            &mut with_hint,
            TooltipFrame {
                scope: hint_scope,
                viewport: Viewport::new(260.0, 130.0, 1.0),
                route: &full_route,
                focus: Some(ElementId::new(155)),
                focus_root: Some(ElementId::new(1)),
                cursor: Point::new(-1.0, -1.0),
                active: true,
                theme: WidgetTheme::dark(),
                now: std::time::Instant::now(),
            },
            &mut text,
        )
        .expect("clipped menu label hint");
    assert_eq!(
        with_hint
            .find(hint_scope.element(2))
            .and_then(|node| node.semantic.as_ref())
            .map(|s| s.label.as_str()),
        Some(complete_label.as_str())
    );
    let hint_route =
        ActionRoute::build(&with_hint, widget.actions.clone()).expect("passive hint route");
    assert_eq!(hint_route.focus_order(), full_route.focus_order());
    assert_eq!(hint_route.action_at(ElementId::new(155)), Some(&11));
    let anchor = menu_submenu_anchor(&menu, MenuId(2), &controller, popup, WidgetTheme::dark())
        .expect("revealed parent row anchor");
    assert!(
        (anchor.y - last.y).abs() < 0.1,
        "submenu anchor {anchor:?}, retained row {last:?}, popup {popup:?}"
    );
    let scope = FocusScope::new(UiWindowId(7), &route, None);
    let path = ScrollInputPath::new(&scope, target(7, 155)).expect("live scroll path");
    let mut calls = Vec::new();
    let remainder = path.scroll(Point::new(0.0, 20.0), |element, delta| {
        calls.push(element);
        if element == ElementId::new(1) {
            Point::new(0.0, 0.0)
        } else {
            delta
        }
    });
    assert_eq!(remainder, Point::new(0.0, 0.0));
    assert!(ScrollInputPath::new(&scope, target(8, 155)).is_none());
    let tree = Element::container(ElementId::new(400), ElementKey::new("consumer.root"))
        .styled(vesta_gpui::Style::default().size(
            vesta_gpui::Dimension::Percent(1.0),
            vesta_gpui::Dimension::Percent(1.0),
        ))
        .child(widget.element);
    assert!(
        LayoutEngine
            .compute(&tree, Viewport::new(260.0, 130.0, 1.0))
            .is_ok()
    );
    // Picker decoration changes measured rows and adds a title. The same
    // disabled/group boundaries, child hit routing and keyboard reveal must hold.
    menu.items.iter_mut().for_each(|item| {
        if !item.separator {
            item.checked = Some(item.id == MenuItemId(11));
        }
    });
    for theme in [WidgetTheme::dark(), WidgetTheme::light()] {
        let picker = menu_picker_popup(
            &menu,
            &controller,
            MenuPickerPopupSpec {
                popup: MenuPopupSpec {
                    root: identity(1),
                    content: identity(2),
                    anchor: Rect::new(200.0, 120.0, 60.0, 30.0),
                    bounds: Rect::new(4.0, 4.0, 256.0, 130.0),
                    direction: PopupDirection::Below,
                    z_index: 90,
                    hovered: None,
                },
                title: identity(3),
            },
            theme,
            &mut text,
            |item| {
                let base = 100 + item.0 * 5;
                MenuItemIds {
                    root: identity(base),
                    check: identity(base + 1),
                    label: identity(base + 2),
                    shortcut: identity(base + 3),
                    arrow: identity(base + 4),
                }
            },
            |item| item.action,
        )
        .expect("bounded picker");
        let route = ActionRoute::build(&picker.element, picker.actions).expect("picker route");
        for (id, hint) in [
            (145, ""),
            (150, "Host details beyond the visible label"),
            (155, complete_label.as_str()),
        ] {
            assert_eq!(
                picker
                    .element
                    .find(ElementId::new(id))
                    .expect("picker command row")
                    .tooltip
                    .as_deref(),
                Some(hint)
            );
        }
        assert_eq!(route.action_for_hit(ElementId::new(156)), Some(&11));
        assert!(route.action_for_hit(ElementId::new(106)).is_none());
        for non_command in [ElementId::new(3), ElementId::new(200), ElementId::new(205)] {
            assert!(route.action_for_hit(non_command).is_none());
            assert!(!route.focus_order().contains(&non_command));
        }
        let row = picker
            .element
            .find(ElementId::new(155))
            .expect("checked row");
        assert_eq!(
            row.semantic.as_ref().and_then(|semantic| semantic.toggle),
            Some(vesta_gpui::SemanticToggle::On)
        );
        let tree = Element::container(ElementId::new(400), ElementKey::new("picker.consumer"))
            .styled(vesta_gpui::Style::default().size(
                vesta_gpui::Dimension::Percent(1.0),
                vesta_gpui::Dimension::Percent(1.0),
            ))
            .child(picker.element);
        let layout = LayoutEngine
            .compute_with_text_context(&tree, Viewport::new(270.0, 150.0, 1.0), &mut text)
            .expect("picker layout");
        let popup = layout.node(ElementId::new(1)).expect("picker").rect;
        let last = layout.node(ElementId::new(155)).expect("revealed row").rect;
        assert!(popup.x >= 4.0 && popup.x + popup.width <= 260.1);
        assert!(last.y >= popup.y && last.y + last.height <= popup.y + popup.height + 0.1);
    }
}
