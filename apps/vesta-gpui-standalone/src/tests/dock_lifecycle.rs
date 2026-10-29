//! Existing Dock cancellation and release contract; no native platform sign-off.
use super::*;
mod admission;
mod focus;
mod geometry;
mod placement;
mod presentation;

#[test]
fn dock_gestures_cancel_when_their_stable_targets_are_removed_or_reset() {
    presentation::host_overlay_matches_shared_authority();
    geometry::changed_window_retires_only_its_dock_coordinates();
    placement::minimized_events_preserve_restore_placement();
    admission::current_scope_rejects_cached_workspace_targets();
    focus::accepted_drop_uses_only_the_current_destination_route();
    relocated_tab_retires_its_old_window_gesture();
    splitter_retirement_preserves_replacement_capture();
    native_capture::qualify_floating_nowrap_preparation();
    native_capture::qualify_floating_area_preparation();
    native_capture::qualify_selected_feedback_preparation();
    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.controls");
    install_action_route(&mut app);
    let slider = app
        .ui_actions
        .focus_order()
        .iter()
        .copied()
        .find(|element| app.ui_actions.action_at(*element) == Some(&DemoAction::SetSlider))
        .expect("live slider");
    let target = InputTarget {
        window: STANDALONE_WINDOW,
        element: slider,
    };
    let initial_slider = app.demo.slider;
    for focus_slider in [false, true] {
        let header = app.scopes.header.element(MODE_2D);
        app.set_focus_target(Some(if focus_slider { slider } else { header }));
        app.slider_rect = Some(Rect::new(0.0, 0.0, 150.0, 28.0));
        app.cursor = Point::new(120.0, 14.0);
        app.input_ownership
            .capture_pointer(target, PointerButton::Primary);
        app.slider_drag = Some(ValueEditSession::new(target, initial_slider));
        app.demo.slider = 0.9;
        app.apply_window_action(STANDALONE_WINDOW, DemoAction::ToggleSliderEnabled);
        assert!(!app.demo.slider_enabled);
        assert_eq!(
            app.demo.slider, initial_slider,
            "disable retires the preview before queued input"
        );
        assert!(app.slider_drag.is_none() && app.input_ownership.capture().is_none());
        assert_eq!(
            app.window_focus(STANDALONE_WINDOW),
            Some(if focus_slider {
                slider_demo::enable_target(app.scopes.gallery)
            } else {
                header
            })
        );
        // The published route still contains SetSlider. Model admission must win.
        assert_eq!(
            app.ui_actions.action_at(slider),
            Some(&DemoAction::SetSlider)
        );
        app.apply_window_action(STANDALONE_WINDOW, DemoAction::SetSliderValue(0.77));
        app.apply_window_action(STANDALONE_WINDOW, DemoAction::SetSlider);
        assert_eq!(app.demo.slider, initial_slider);
        assert!(app.slider_drag.is_none() && app.input_ownership.capture().is_none());
        install_action_route(&mut app);
        assert!(app.ui_actions.action_at(slider).is_none());
        assert!(!app.ui_actions.focus_order().contains(&slider));
        app.apply_window_action(STANDALONE_WINDOW, DemoAction::ToggleSliderEnabled);
        install_action_route(&mut app);
        assert!(app.demo.slider_enabled);
        assert_eq!(
            app.ui_actions.action_at(slider),
            Some(&DemoAction::SetSlider)
        );
        assert!(
            !app.finish_slider_edit(),
            "re-enable cannot revive the disabled edit"
        );
    }
    app.set_focus_target(Some(slider));
    app.apply_window_action(STANDALONE_WINDOW, DemoAction::SetSlider);
    assert!(app.slider_drag.is_some() && app.input_ownership.capture().is_some());
    assert_ne!(
        app.demo.slider, initial_slider,
        "enabled slider still accepts normal preview"
    );
    assert!(app.finish_slider_edit());
    app.input_ownership
        .release_pointer(STANDALONE_WINDOW, PointerButton::Primary);
    app.apply_window_action(
        STANDALONE_WINDOW,
        DemoAction::SetSliderValue(initial_slider),
    );
    for focus_slider in [false, true] {
        let header = app.scopes.header.element(MODE_2D);
        let focus = if focus_slider { slider } else { header };
        app.set_focus_target(Some(focus));
        app.input_ownership
            .capture_pointer(target, PointerButton::Primary);
        app.slider_drag = Some(ValueEditSession::new(target, initial_slider));
        app.demo.slider = 0.9;
        app.apply_window_action(
            STANDALONE_WINDOW,
            DemoAction::ToggleGallerySection(GallerySectionId::Buttons),
        );
        assert_eq!(app.demo.slider, 0.9);
        assert!(
            app.slider_drag.is_some() && app.input_ownership.capture().is_some(),
            "unrelated section collapse preserves the live slider gesture"
        );
        app.apply_window_action(
            STANDALONE_WINDOW,
            DemoAction::ToggleGallerySection(GallerySectionId::Buttons),
        );
        app.apply_window_action(
            STANDALONE_WINDOW,
            DemoAction::ToggleGallerySection(GallerySectionId::Feedback),
        );
        assert_eq!(
            app.demo.slider, initial_slider,
            "section collapse restores before the queued release, even when focus is elsewhere"
        );
        assert!(app.slider_drag.is_none() && app.input_ownership.capture().is_none());
        assert_eq!(
            app.window_focus(STANDALONE_WINDOW),
            Some(if focus_slider {
                GallerySectionId::Feedback.element(app.scopes.gallery, 1)
            } else {
                header
            })
        );
        app.apply_window_action(
            STANDALONE_WINDOW,
            DemoAction::ToggleGallerySection(GallerySectionId::Feedback),
        );
        install_action_route(&mut app);
        assert_eq!(
            app.ui_actions.action_at(slider),
            Some(&DemoAction::SetSlider)
        );
        assert!(
            !app.finish_slider_edit(),
            "reopening the section cannot revive the old edit"
        );
    }
    let header = app.scopes.header.element(MODE_2D);
    let conditional = GallerySectionId::Conditional.element(app.scopes.gallery, 3);
    let child = app
        .ui_actions
        .focus_order()
        .iter()
        .copied()
        .find(|element| {
            app.ui_actions.is_descendant_of(*element, conditional)
                && app.ui_actions.action_at(*element) == Some(&DemoAction::StepNumber(1))
        })
        .expect("live conditional child");
    app.set_focus_target(Some(header));
    assert!(app.pointer_click.press(
        STANDALONE_WINDOW,
        PointerButton::Primary,
        &app.ui_actions,
        Some(child)
    ));
    app.apply_window_action(
        STANDALONE_WINDOW,
        DemoAction::ToggleGallerySection(GallerySectionId::Conditional),
    );
    assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(header));
    assert!(
        app.pointer_click
            .release(
                STANDALONE_WINDOW,
                PointerButton::Primary,
                &app.ui_actions,
                Some(child)
            )
            .is_none(),
        "section hiding retires a pending click even without focus inside it"
    );
    app.apply_window_action(
        STANDALONE_WINDOW,
        DemoAction::ToggleGallerySection(GallerySectionId::Conditional),
    );
    install_action_route(&mut app);
    app.set_focus_target(Some(slider));
    app.input_ownership
        .capture_pointer(target, PointerButton::Primary);
    app.slider_drag = Some(ValueEditSession::new(target, initial_slider));
    app.demo.slider = 0.9;
    app.apply_window_action(STANDALONE_WINDOW, DemoAction::WorkspaceReset);
    assert_eq!(
        app.demo.slider, initial_slider,
        "Reset restores a source-generation preview before reusing UI identities"
    );
    assert!(app.slider_drag.is_none());
    assert!(app.input_ownership.capture().is_none());
    assert!(
        !app.finish_slider_edit(),
        "a release from the replaced Workspace cannot commit"
    );
    install_action_route(&mut app);
    assert_eq!(
        app.ui_actions.action_at(slider),
        Some(&DemoAction::SetSlider),
        "same stable ID is live in the replacement"
    );
    let controls = activate_gallery_page(&mut app, "gallery.controls");
    install_action_route(&mut app);
    app.set_focus_target(Some(slider));
    app.input_ownership
        .capture_pointer(target, PointerButton::Primary);
    app.slider_drag = Some(ValueEditSession::new(target, initial_slider));
    app.demo.slider = 0.9;
    let inactive = app
        .demo
        .workspace
        .panes()
        .into_iter()
        .flat_map(|pane| pane.items.iter())
        .find(|item| item.panel_type.as_str() == "gallery.inputs")
        .expect("inactive Inputs")
        .id;
    for unrelated in [ItemId(u64::MAX), inactive] {
        app.apply_window_action(
            STANDALONE_WINDOW,
            DemoAction::Workspace(WorkspaceViewAction::CloseTab(unrelated)),
        );
        assert!(
            app.slider_drag.is_some() && app.input_ownership.capture().is_some(),
            "unrelated CloseTab retains the source preview"
        );
        assert_eq!(app.demo.slider, 0.9);
    }
    app.apply_window_action(
        STANDALONE_WINDOW,
        DemoAction::Workspace(WorkspaceViewAction::CloseTab(controls)),
    );
    assert_eq!(
        app.demo.slider, initial_slider,
        "CloseTab must restore before the immediately queued release"
    );
    assert!(app.slider_drag.is_none() && app.input_ownership.capture().is_none());
    assert!(
        !app.finish_slider_edit(),
        "closed page release cannot commit through the old route"
    );
    app.apply_window_action(STANDALONE_WINDOW, DemoAction::WorkspaceReset);
    let controls = activate_gallery_page(&mut app, "gallery.controls");
    install_action_route(&mut app);
    let header = app.scopes.header.element(MODE_2D);
    app.set_focus_target(Some(header));
    app.input_ownership
        .capture_pointer(target, PointerButton::Primary);
    app.slider_drag = Some(ValueEditSession::new(target, initial_slider));
    app.demo.slider = 0.9;
    app.apply_window_action(
        STANDALONE_WINDOW,
        DemoAction::Workspace(WorkspaceViewAction::CloseTab(controls)),
    );
    assert_eq!(app.demo.slider, initial_slider);
    assert!(app.slider_drag.is_none() && app.input_ownership.capture().is_none());
    assert_eq!(
        app.window_focus(STANDALONE_WINDOW),
        Some(header),
        "capture-only retirement preserves unrelated live focus"
    );
    assert!(!app.finish_slider_edit());
    app.apply_window_action(STANDALONE_WINDOW, DemoAction::WorkspaceReset);
    activate_gallery_page(&mut app, "gallery.inputs");
    install_action_route(&mut app);
    app.set_text_focus(Some(TextFieldKind::Number));
    let number = InputTarget {
        window: STANDALONE_WINDOW,
        element: TextFieldKind::Number.element(app.scopes.gallery),
    };
    app.number_scrub = Some(NumberScrub::new(number, 0.0, 128, 0.0).expect("number preview"));
    app.input_ownership
        .capture_pointer(number, PointerButton::Primary);
    app.demo.number = 400;
    app.number_state
        .reset(&mut app.text_context, "400")
        .expect("preview draft");
    app.apply_window_action(STANDALONE_WINDOW, DemoAction::WorkspaceReset);
    assert_eq!(
        app.demo.number, 128,
        "Reset cancels the preview before numeric draft exit"
    );
    assert_eq!(app.number_state.text(), "128");
    assert!(app.number_scrub.is_none() && app.input_ownership.capture().is_none());
    assert!(!app.finish_number_scrub());
    activate_gallery_page(&mut app, "gallery.inputs");
    install_action_route(&mut app);
    app.set_text_focus(Some(TextFieldKind::Number));
    let initial_number = app.demo.number;
    let initial_cursor = app.cursor;
    app.begin_number_scrub();
    app.cursor.x += 16.0;
    assert!(app.update_number_scrub());
    assert_ne!(app.demo.number, initial_number);
    assert!(app.input_ownership.capture().is_none());
    app.apply_window_action(
        STANDALONE_WINDOW,
        DemoAction::ToggleGallerySection(GallerySectionId::Inputs),
    );
    assert_eq!(
        app.demo.number, initial_number,
        "section removal restores a real Number scrub without generic capture"
    );
    assert_eq!(app.number_state.text(), initial_number.to_string());
    assert!(app.number_scrub.is_none());
    assert!(
        !app.finish_number_scrub(),
        "late release cannot accept the preview"
    );
    assert_eq!(
        app.window_focus(STANDALONE_WINDOW),
        Some(GallerySectionId::Inputs.element(app.scopes.gallery, 1)),
        "section header retains replacement focus after the retired release"
    );
    app.cursor = initial_cursor;
    app.apply_window_action(
        STANDALONE_WINDOW,
        DemoAction::ToggleGallerySection(GallerySectionId::Inputs),
    );
    activate_gallery_page(&mut app, "gallery.inputs");
    install_action_route(&mut app);
    app.set_text_focus(Some(TextFieldKind::Number));
    app.number_state
        .reset(&mut app.text_context, "42")
        .expect("ordinary draft");
    app.apply_window_action(STANDALONE_WINDOW, DemoAction::WorkspaceReset);
    assert_eq!(
        app.demo.number, 42,
        "Reset preserves ordinary numeric draft submission"
    );
    assert!(app.text_session.is_none());
    activate_gallery_page(&mut app, "gallery.inputs");
    install_action_route(&mut app);
    let vector = app.demo.vector;
    assert_eq!(vector[1], None);
    app.set_text_focus(Some(TextFieldKind::Vector(1)));
    app.vector_states[1]
        .reset(&mut app.text_context, "17")
        .expect("ordinary mixed component draft");
    app.apply_window_action(STANDALONE_WINDOW, DemoAction::WorkspaceReset);
    assert_eq!(app.demo.vector, [vector[0], Some(17.0), vector[2]]);
    assert_eq!(app.demo.number, 42);
    assert!(app.text_session.is_none());
    let original = dock_split_ratio(&app.demo.workspace, ContainerRef::Main, SplitId(1))
        .expect("initial split");
    let session = vesta_gpui_workspace::SplitResizeSession::new(
        InputTarget {
            window: STANDALONE_WINDOW,
            element: ElementId::new(1),
        },
        ContainerRef::Main,
        SplitId(1),
        SplitAxis::Row,
        Rect::new(0.0, 0.0, 800.0, 400.0),
        original,
    )
    .expect("split preview");
    app.workspace_resize = Some(session);
    assert!(app.update_workspace_resize(STANDALONE_WINDOW, Point::new(760.0, 200.0)));
    assert!(!app.finish_workspace_resize_for(UiWindowId(2), false));
    assert!(
        app.workspace_resize.is_some(),
        "foreign cancel cannot end the owning drag"
    );
    assert_ne!(
        dock_split_ratio(&app.demo.workspace, ContainerRef::Main, SplitId(1)),
        Some(original)
    );
    app.cancel_window_value_edits(STANDALONE_WINDOW);
    assert_eq!(
        dock_split_ratio(&app.demo.workspace, ContainerRef::Main, SplitId(1)),
        Some(original)
    );
    assert!(
        app.workspace_resize.is_none(),
        "Escape/blur restore the starting ratio"
    );
    app.workspace_resize = Some(session);
    app.update_workspace_resize(STANDALONE_WINDOW, Point::new(760.0, 200.0));
    assert!(app.finish_workspace_resize_for(STANDALONE_WINDOW, true));
    assert_ne!(
        dock_split_ratio(&app.demo.workspace, ContainerRef::Main, SplitId(1)),
        Some(original)
    );
    let item = app.demo.workspace.pane(PaneId(1)).expect("left pane").items[0].id;
    app.demo
        .workspace
        .dock_item(
            item,
            DockAction::Float {
                placement: WindowPlacement::new(20.0, 20.0, 360.0, 220.0),
            },
        )
        .expect("floating item");
    let floating = app.demo.workspace.floating_windows()[0].id;
    app.workspace_drag = Some(WorkspaceTabDrag {
        item,
        origin_container: ContainerRef::Floating(floating),
        origin: Point::new(20.0, 20.0),
        started: true,
        pointer_container: Some(ContainerRef::Floating(floating)),
        cursor: Point::new(20.0, 20.0),
        target: None,
    });
    app.floating_move = Some(FloatingMoveDrag {
        window: floating,
        origin: Point::new(20.0, 20.0),
        placement: WindowPlacement::new(20.0, 20.0, 360.0, 220.0),
    });
    let floating_drag = app.workspace_drag;
    app.cancel_window_editors(STANDALONE_WINDOW);
    assert!(
        app.workspace_drag.is_some(),
        "foreign window cleanup cannot end a floating origin's drag"
    );
    app.cancel_workspace_drag_for_container(ContainerRef::Main);
    assert!(
        app.workspace_drag.is_some(),
        "an unrelated suspended main display cannot consume a floating drag"
    );
    app.workspace_drag = floating_drag;
    let drag = app.workspace_drag.as_mut().expect("floating source");
    drag.pointer_container = Some(ContainerRef::Main);
    drag.target = Some(DockDropTarget {
        action: DockAction::Merge {
            pane: PaneId(1),
            slot: 0,
        },
        preview: Rect::new(0.0, 0.0, 100.0, 100.0),
        insertion: None,
    });
    app.cancel_workspace_drag_for_container(ContainerRef::Floating(floating));
    assert!(
        app.workspace_drag.is_none(),
        "a suspended origin retires its drag"
    );
    app.finish_workspace_drag(ContainerRef::Floating(floating));
    assert!(
        app.demo.workspace.floating_window(floating).is_some(),
        "a stale release cannot merge the floating source into main"
    );
    app.workspace_drag = floating_drag;
    let drag = app
        .workspace_drag
        .as_mut()
        .expect("retained release candidate");
    drag.pointer_container = Some(ContainerRef::Main);
    drag.target = Some(DockDropTarget {
        action: DockAction::Merge {
            pane: PaneId(1),
            slot: 0,
        },
        preview: Rect::new(0.0, 0.0, 100.0, 100.0),
        insertion: None,
    });
    assert!(
        !app.finish_workspace_drag_with_admission(ContainerRef::Main, |_| {
            panic!("foreign release cannot consult the owning display")
        })
    );
    assert!(
        app.workspace_drag
            .expect("foreign release retains drag")
            .target
            .is_some()
    );
    assert!(app.finish_workspace_drag(ContainerRef::Floating(floating)));
    assert!(app.workspace_drag.is_none());
    assert!(!app.finish_workspace_drag(ContainerRef::Floating(floating)));
    assert!(
        app.demo.workspace.floating_window(floating).is_some(),
        "unpublished displays reject release without requiring another pointer move"
    );
    app.workspace_drag = floating_drag;
    app.workspace_resize = Some(
        vesta_gpui_workspace::SplitResizeSession::new(
            InputTarget {
                window: STANDALONE_WINDOW,
                element: ElementId::new(1),
            },
            ContainerRef::Main,
            SplitId(1),
            SplitAxis::Row,
            Rect::new(0.0, 0.0, 800.0, 400.0),
            dock_split_ratio(&app.demo.workspace, ContainerRef::Main, SplitId(1))
                .expect("live split"),
        )
        .expect("split session"),
    );
    app.apply_workspace_action(WorkspaceViewAction::CloseFloating(floating));
    assert!(app.workspace_drag.is_none());
    assert!(app.floating_move.is_none());
    assert!(
        app.dock_drag_visual(ContainerRef::Floating(floating))
            .is_none()
    );
    assert!(app.workspace_resize.is_some());

    let right_items = app
        .demo
        .workspace
        .pane(PaneId(2))
        .expect("right pane")
        .items
        .iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    for item in right_items {
        app.apply_workspace_action(WorkspaceViewAction::CloseTab(item));
    }
    assert!(app.workspace_resize.is_none());

    app.apply_demo_action(DemoAction::WorkspaceReset);
    let item = app
        .demo
        .workspace
        .pane(PaneId(1))
        .expect("reset pane")
        .items[0]
        .id;
    app.workspace_drag = Some(WorkspaceTabDrag {
        item,
        origin_container: ContainerRef::Main,
        origin: Point::new(20.0, 20.0),
        started: true,
        pointer_container: Some(ContainerRef::Main),
        cursor: Point::new(20.0, 20.0),
        target: None,
    });
    app.workspace_resize = Some(
        vesta_gpui_workspace::SplitResizeSession::new(
            InputTarget {
                window: STANDALONE_WINDOW,
                element: ElementId::new(1),
            },
            ContainerRef::Main,
            SplitId(1),
            SplitAxis::Row,
            Rect::new(0.0, 0.0, 800.0, 400.0),
            dock_split_ratio(&app.demo.workspace, ContainerRef::Main, SplitId(1))
                .expect("live split"),
        )
        .expect("split session"),
    );
    app.apply_demo_action(DemoAction::WorkspaceReset);
    assert!(app.workspace_drag.is_none());
    assert!(app.workspace_resize.is_none());

    let item = app.demo.workspace.pane(PaneId(1)).expect("main pane").items[0].id;
    vesta_gpui_workspace::begin_tab_drag(
        &mut app.workspace_drag,
        item,
        ContainerRef::Main,
        Point::new(20.0, 20.0),
    );
    app.cancel_window_editors(STANDALONE_WINDOW);
    assert!(
        app.workspace_drag.is_none(),
        "ordered window cleanup retires its tab gesture before clearing capture"
    );
}

fn relocated_tab_retires_its_old_window_gesture() {
    let mut app = App::new();
    let pane = app.demo.workspace.active_pane();
    let item = app.demo.workspace.active_item().expect("source tab").id;
    app.workspace_drag = Some(WorkspaceTabDrag {
        item,
        origin_container: ContainerRef::Main,
        origin: Point::new(30.0, 14.0),
        started: true,
        pointer_container: Some(ContainerRef::Main),
        cursor: Point::new(300.0, 200.0),
        target: Some(DockDropTarget {
            action: DockAction::Merge { pane, slot: 0 },
            preview: Rect::new(0.0, 0.0, 400.0, 300.0),
            insertion: None,
        }),
    });
    assert!(app.float_workspace_item(item, WindowPlacement::new(20.0, 20.0, 360.0, 220.0)));
    let after = app.demo.workspace.clone();
    app.reconcile_workspace_gestures();
    assert!(
        app.workspace_drag.is_none(),
        "the stable tab now belongs to a different window"
    );
    assert!(!app.finish_workspace_drag_with_admission(ContainerRef::Main, |_| true));
    assert_eq!(
        app.demo.workspace, after,
        "late release cannot undo the accepted relocation"
    );
}

fn splitter_retirement_preserves_replacement_capture() {
    let target = InputTarget {
        window: STANDALONE_WINDOW,
        element: ElementId::new(1),
    };
    for commit in [false, true] {
        for (replacement, button) in [
            (target, PointerButton::Secondary),
            (target, PointerButton::Middle),
            (
                InputTarget {
                    element: ElementId::new(2),
                    ..target
                },
                PointerButton::Primary,
            ),
            (target, PointerButton::Primary),
        ] {
            let mut app = App::new();
            let original = dock_split_ratio(&app.demo.workspace, ContainerRef::Main, SplitId(1))
                .expect("original ratio");
            app.workspace_resize = Some(
                vesta_gpui_workspace::SplitResizeSession::new(
                    target,
                    ContainerRef::Main,
                    SplitId(1),
                    SplitAxis::Row,
                    Rect::new(0.0, 0.0, 800.0, 400.0),
                    original,
                )
                .expect("split preview"),
            );
            app.input_ownership
                .capture_pointer(target, PointerButton::Primary);
            assert!(app.update_workspace_resize(STANDALONE_WINDOW, Point::new(760.0, 200.0)));
            let preview = dock_split_ratio(&app.demo.workspace, ContainerRef::Main, SplitId(1));
            assert_ne!(preview, Some(original));
            app.input_ownership.capture_pointer(replacement, button);
            let expected = if replacement == target && button == PointerButton::Primary {
                None
            } else {
                app.input_ownership.capture()
            };
            assert!(app.finish_workspace_resize_for(STANDALONE_WINDOW, commit));
            assert_eq!(
                app.input_ownership.capture(),
                expected,
                "split retirement cannot clear replacement target/button ownership"
            );
            assert_eq!(
                dock_split_ratio(&app.demo.workspace, ContainerRef::Main, SplitId(1)),
                if commit { preview } else { Some(original) },
                "capture retirement does not change accepted commit/rollback"
            );
            assert!(app.workspace_resize.is_none());
            assert!(!app.finish_workspace_resize_for(STANDALONE_WINDOW, !commit));
            assert_eq!(app.input_ownership.capture(), expected);
        }
    }
}
