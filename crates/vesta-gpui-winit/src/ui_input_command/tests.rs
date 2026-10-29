use super::*;
use vesta_gpui::{
    Dimension, Element, ElementKey, LayoutEngine, Semantic, SemanticRole, Style, UiQueryLimits,
    UiSnapshotFrame, UiSnapshotId, Viewport,
};

#[test]
fn bounded_input_preserves_target_geometry_and_rejects_partial_gestures_or_sensitive_routes_before_dispatch()
 {
    let base = json!({"schema_version":1,"request_id":"input-1","op":"input","instance":"42","window":"1","element":"2","owner":"7","revision":"1","steps":[{"kind":"drag","from":{"x":2,"y":2},"to":{"x":15,"y":18},"samples":4}]});
    let request = UiInputRequest::parse(&base).expect("bounded gesture");
    let mut example = ui_input_schema()["request_example"].clone();
    for key in ["instance", "window", "element", "owner", "revision"] {
        example[key] = base[key].clone();
    }
    UiInputRequest::parse(&example).expect("discovered wire example must parse");
    for steps in [
        json!([{"kind":"press"}]),
        json!([{"kind":"release"}]),
        json!([{"kind":"press"},{"kind":"press"},{"kind":"release"}]),
        json!([{"kind":"drag","from":{"x":0,"y":0},"to":{"x":1,"y":1},"samples":65}]),
        json!([{"kind":"key","key":"unknown_non_key"}]),
        json!([{"kind":"text","text":"x".repeat(vesta_gpui::TEXT_INPUT_MAX_BYTES+1)}]),
    ] {
        let mut invalid = base.clone();
        invalid["steps"] = steps;
        assert!(
            UiInputRequest::parse(&invalid).is_err(),
            "all validation precedes Host input"
        );
    }
    let root = Element::container(ElementId::new(1), ElementKey::new("root"))
        .styled(Style::default().size(Dimension::Px(300.0), Dimension::Px(200.0)))
        .child(
            Element::container(ElementId::new(2), ElementKey::new("surface"))
                .styled(Style::default().size(Dimension::Px(100.0), Dimension::Px(50.0)))
                .semantic(Semantic::new(SemanticRole::Button, "surface")),
        )
        .child(
            Element::container(ElementId::new(3), ElementKey::new("secret"))
                .styled(Style::default().size(Dimension::Px(100.0), Dimension::Px(50.0)))
                .semantic(
                    Semantic::new(SemanticRole::PasswordInput, "credential").value("never export"),
                ),
        );
    let viewport = Viewport::new(300.0, 200.0, 1.5);
    let layout = LayoutEngine.compute(&root, viewport).expect("layout");
    let route = ActionRoute::build(&root, [(ElementId::new(2), ())]).expect("route");
    let snapshot = UiSnapshot::capture(
        UiSnapshotFrame {
            id: UiSnapshotId {
                instance: 42,
                window: UiWindowId(1),
                revision: 1,
            },
            root: &root,
            layout: &layout,
            route: &route,
            focus: None,
            modal_root: None,
        },
        UiQueryLimits::default(),
        |_| None,
        |_| false,
    )
    .expect("snapshot");
    request
        .validate(&snapshot, &route, None)
        .expect("current surface");
    request
        .validate_hits(&snapshot, &route, |_| Some(ElementId::new(2)))
        .expect("actual hit");
    assert_eq!(
        request.validate_hits(&snapshot, &route, |_| Some(ElementId::new(3))),
        Err("target_occluded")
    );
    assert_eq!(
        request.validate_hits(&snapshot, &route, |_| None),
        Err("target_occluded")
    );
    let mut background = request.clone();
    background.target.element = ElementId::new(1);
    background
        .validate_hits(&snapshot, &route, |_| None)
        .expect("only the window root may address a blank Host background");
    let events = request
        .events(&snapshot, ModifiersState::CONTROL)
        .expect("events");
    let states = events
        .iter()
        .filter_map(|event| match event {
            UiInputEvent::MouseInput { state, .. } => Some(*state),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(states, [ElementState::Pressed, ElementState::Released]);
    let source = snapshot
        .nodes()
        .iter()
        .find(|node| node.id == ElementId::new(2))
        .and_then(|node| node.bounds)
        .expect("source bounds");
    assert!(events.iter().any(|event|matches!(event,UiInputEvent::CursorMoved{position,..} if *position==PhysicalPosition::new(f64::from(source.x+15.0)*1.5,f64::from(source.y+18.0)*1.5))),"logical local coordinates use the actual source DPI");
    assert!(
        matches!(events.last(),Some(UiInputEvent::ModifiersChanged(value)) if value.state()==ModifiersState::CONTROL),
        "the caller's native modifier state is restored"
    );
    let mut secret = request.clone();
    secret.target.element = ElementId::new(3);
    assert_eq!(
        secret.validate(&snapshot, &route, None),
        Err("sensitive_target")
    );
    let mut foreign = request.clone();
    foreign.target.instance = 0;
    assert_eq!(
        foreign.validate(&snapshot, &route, None),
        Err("foreign_instance")
    );
    assert!(
        matches!(command_key("F35"), Ok((Key::Named(NamedKey::F35), _))),
        "all pinned winit named keys use its authority"
    );
    let mut modified = base;
    modified["modifiers"] = json!(["shift"]);
    let modified = UiInputRequest::parse(&modified).expect("modified drag");
    assert_eq!(modified.modifiers, ModifiersState::SHIFT);
    // Repaints preserve evidence; a changed Host context, viewport or owner must not
    // redirect old coordinates onto a newly presented scene.
    let mut owners = vesta_gpui::UiOwnerRegistry::default();
    owners
        .update(&snapshot, &route)
        .expect("valid input evidence fixture");
    let mut bound = request.clone();
    bound.target = owners
        .reference(UiWindowId(1), ElementId::new(2))
        .expect("valid input evidence fixture");
    let proof = crate::UiInputObservation::capture(
        &snapshot,
        &owners,
        &route,
        &root,
        &layout,
        "scene-a".into(),
    );
    let repaint = UiSnapshot::capture(
        UiSnapshotFrame {
            id: UiSnapshotId {
                revision: 2,
                ..snapshot.id
            },
            root: &root,
            layout: &layout,
            route: &route,
            focus: None,
            modal_root: None,
        },
        UiQueryLimits::default(),
        |_| None,
        |_| false,
    )
    .expect("valid input evidence fixture");
    owners
        .update(&repaint, &route)
        .expect("valid input evidence fixture");
    let now = crate::UiInputObservation::capture(
        &repaint,
        &owners,
        &route,
        &root,
        &layout,
        "scene-a".into(),
    );
    proof
        .validate(&snapshot, &repaint, &now, &bound)
        .expect("unrelated repaint preserves usable input evidence");
    let other = crate::UiInputObservation::capture(
        &repaint,
        &owners,
        &route,
        &root,
        &layout,
        "scene-b".into(),
    );
    assert_eq!(
        proof.validate(&snapshot, &repaint, &other, &bound),
        Err("stale_input_context")
    );
    let mut mixed = bound.clone();
    mixed.target.owner += 1;
    assert_eq!(
        proof.validate(&snapshot, &repaint, &now, &mixed),
        Err("stale_input_owner")
    );
    let changed_layout = LayoutEngine
        .compute(&root, Viewport::new(400.0, 200.0, 1.5))
        .expect("valid input evidence fixture");
    let resized = UiSnapshot::capture(
        UiSnapshotFrame {
            id: UiSnapshotId {
                revision: 3,
                ..snapshot.id
            },
            root: &root,
            layout: &changed_layout,
            route: &route,
            focus: None,
            modal_root: None,
        },
        UiQueryLimits::default(),
        |_| None,
        |_| false,
    )
    .expect("valid input evidence fixture");
    let resized_proof = crate::UiInputObservation::capture(
        &resized,
        &owners,
        &route,
        &root,
        &changed_layout,
        "scene-a".into(),
    );
    assert_eq!(
        proof.validate(&snapshot, &resized, &resized_proof, &bound),
        Err("stale_input_geometry")
    );
    let mut context = vesta_gpui::TextInputContext::default();
    let mut editor = vesta_gpui::TextInputState::default();
    editor
        .insert(
            &mut context,
            &"x".repeat(4100),
            vesta_gpui::TextEditKind::Typing,
        )
        .expect("long editor source");
    let editor_tree = |editor: &vesta_gpui::TextInputState| {
        Element::container(ElementId::new(1), ElementKey::new("root"))
            .styled(Style::default().size(Dimension::Px(300.0), Dimension::Px(200.0)))
            .child(
                Element::text_input(
                    ElementId::new(2),
                    ElementKey::new("editor"),
                    editor.snapshot(),
                    true,
                )
                .styled(Style::default().size(Dimension::Px(100.0), Dimension::Px(50.0)))
                .semantic(Semantic::new(SemanticRole::TextInput, "editor").value(editor.text())),
            )
            .child(
                Element::container(ElementId::new(4), ElementKey::new("other-button"))
                    .styled(Style::default().size(Dimension::Px(100.0), Dimension::Px(50.0)))
                    .semantic(Semantic::new(SemanticRole::Button, "other")),
            )
    };
    let long_root = editor_tree(&editor);
    let long_layout = LayoutEngine
        .compute(&long_root, viewport)
        .expect("long layout");
    let long_route = ActionRoute::build(&long_root, [(ElementId::new(2), ())]).expect("long route");
    let capture =
        |root: &Element, layout: &vesta_gpui::LayoutTree, route: &ActionRoute<()>, revision| {
            UiSnapshot::capture(
                UiSnapshotFrame {
                    id: UiSnapshotId {
                        revision,
                        ..snapshot.id
                    },
                    root,
                    layout,
                    route,
                    focus: None,
                    modal_root: None,
                },
                UiQueryLimits::default(),
                |_| None,
                |_| false,
            )
            .expect("editor snapshot")
        };
    let long_before = capture(&long_root, &long_layout, &long_route, 1);
    owners
        .update(&long_before, &long_route)
        .expect("long owners");
    let long_proof = crate::UiInputObservation::capture(
        &long_before,
        &owners,
        &long_route,
        &long_root,
        &long_layout,
        "scene-a".into(),
    );
    let mut key = bound.clone();
    key.target = owners
        .reference(UiWindowId(1), ElementId::new(2))
        .expect("long target");
    key.steps = vec![UiInputStep::Key {
        key: "Backspace".into(),
        modifiers: None,
    }];
    editor.refresh(&mut context);
    editor.refresh(&mut context);
    let refreshed_root = editor_tree(&editor);
    let refreshed = capture(&refreshed_root, &long_layout, &long_route, 2);
    owners
        .update(&refreshed, &long_route)
        .expect("refresh owners");
    let refreshed_proof = crate::UiInputObservation::capture(
        &refreshed,
        &owners,
        &long_route,
        &refreshed_root,
        &long_layout,
        "scene-a".into(),
    );
    long_proof
        .validate(&long_before, &refreshed, &refreshed_proof, &key)
        .expect("unchanged frame refresh preserves complete editor evidence");
    editor
        .insert(
            &mut context,
            "outside visible query prefix",
            vesta_gpui::TextEditKind::Typing,
        )
        .expect("tail edit");
    let tail_root = editor_tree(&editor);
    let tail_layout = LayoutEngine
        .compute(&tail_root, viewport)
        .expect("tail layout");
    let tail_route = ActionRoute::build(&tail_root, [(ElementId::new(2), ())]).expect("tail route");
    let tail = capture(&tail_root, &tail_layout, &tail_route, 2);
    owners.update(&tail, &tail_route).expect("tail owners");
    assert_eq!(
        long_before.nodes()[1].value,
        tail.nodes()[1].value,
        "bounded query text cannot detect this tail edit"
    );
    assert!(tail.nodes()[1].value.truncated);
    let tail_proof = crate::UiInputObservation::capture(
        &tail,
        &owners,
        &tail_route,
        &tail_root,
        &tail_layout,
        "scene-a".into(),
    );
    assert_eq!(
        long_proof.validate(&long_before, &tail, &tail_proof, &key),
        Err("stale_input_edit"),
        "immutable editor source still rejects the old request"
    );
    let mut selection = key.clone();
    selection.steps = vec![
        UiInputStep::Move(Point::new(15.0, 18.0)),
        UiInputStep::Button {
            button: MouseButton::Left,
            pressed: true,
        },
        UiInputStep::Move(Point::new(35.0, 18.0)),
        UiInputStep::Button {
            button: MouseButton::Left,
            pressed: false,
        },
    ];
    assert_eq!(
        long_proof.validate(&long_before, &tail, &tail_proof, &selection),
        Err("stale_input_edit"),
        "text selection coordinates must retain their complete editor source"
    );
    selection.target = owners
        .reference(UiWindowId(1), ElementId::new(1))
        .expect("window root");
    assert_eq!(
        long_proof.validate(&long_before, &tail, &tail_proof, &selection),
        Err("stale_input_edit"),
        "a containing root cannot bypass the actual editor hit precondition"
    );
    selection.steps = vec![
        UiInputStep::Move(Point::new(15.0, 68.0)),
        UiInputStep::Button {
            button: MouseButton::Left,
            pressed: true,
        },
        UiInputStep::Button {
            button: MouseButton::Left,
            pressed: false,
        },
    ];
    long_proof
        .validate(&long_before, &tail, &tail_proof, &selection)
        .expect("an unrelated editor edit must not invalidate another button hit");
}
