use super::*;
use crate::{
    Dimension, ElementKey, LayoutEngine, Semantic, Style, TextInputAction, TextInputContext,
    TextInputState, Viewport,
};

fn id(value: u64) -> ElementId {
    ElementId::new(value)
}
fn identity(revision: u64) -> UiSnapshotId {
    UiSnapshotId {
        instance: 42,
        window: UiWindowId(1),
        revision,
    }
}

#[test]
fn snapshot_is_bounded_redacted_and_paged_by_exact_window_and_revision() {
    let secret = "never-export-凭证";
    let root = Element::container(id(1), ElementKey::new("root"))
        .styled(Style::default().size(Dimension::Px(300.0), Dimension::Px(300.0)))
        .child(Element::text(id(2), ElementKey::new("text"), "地形🙂ABC"))
        .child(
            Element::container(id(3), ElementKey::new("password"))
                .semantic(
                    Semantic::new(SemanticRole::PasswordInput, secret)
                        .value(secret)
                        .description(secret),
                )
                .tooltip(secret)
                .child(Element::text(
                    id(4),
                    ElementKey::new("secret.child"),
                    secret,
                )),
        )
        .child(
            Element::container(id(5), ElementKey::new("disabled"))
                .styled(Style::default().action_boundary(true))
                .child(Element::text(
                    id(6),
                    ElementKey::new("disabled.action"),
                    "Disabled",
                )),
        )
        .child(
            Element::container(id(7), ElementKey::new("modal")).child(Element::text(
                id(8),
                ElementKey::new("modal.action"),
                "Confirm",
            )),
        )
        .child(
            Element::text(id(9), ElementKey::new("offscreen"), "Away")
                .styled(Style::default().absolute(500.0, 500.0)),
        );
    let layout = LayoutEngine
        .compute(&root, Viewport::new(300.0, 300.0, 1.0))
        .expect("layout");
    let route =
        ActionRoute::build(&root, [(id(2), 0), (id(6), 1), (id(8), 2), (id(9), 3)]).expect("route");
    let frame = || UiSnapshotFrame {
        id: identity(1),
        root: &root,
        layout: &layout,
        route: &route,
        focus: Some(InputTarget {
            window: UiWindowId(1),
            element: id(8),
        }),
        modal_root: Some(id(7)),
    };
    let snapshot = UiSnapshot::capture(frame(), UiQueryLimits::default(), |_| None, |_| false)
        .expect("snapshot");
    let mut owners = crate::UiOwnerRegistry::default();
    owners.update(&snapshot, &route).expect("owners");
    let confirm = owners
        .reference(UiWindowId(1), id(8))
        .expect("confirm owner");
    crate::UiCommand::Activate
        .preflight(confirm, &snapshot, &route, Some(id(7)))
        .expect("modal target");
    let background = owners.reference(UiWindowId(1), id(2)).expect("background");
    assert_eq!(
        crate::UiCommand::Activate.preflight(background, &snapshot, &route, Some(id(7))),
        Err("modal_blocked")
    );
    owners.update(&snapshot, &route).expect("repaint");
    assert!(
        owners.contains(confirm),
        "unrelated repaint preserves command identity"
    );
    let replacement = ActionRoute::build(&root, [(id(2), 0), (id(6), 1), (id(8), 99), (id(9), 3)])
        .expect("replacement");
    owners
        .update(&snapshot, &replacement)
        .expect("changed action");
    assert!(
        !owners.contains(confirm),
        "reused element cannot receive an old owner's command"
    );
    owners.forget(UiWindowId(1));
    assert!(!owners.contains(background));
    assert!(
        snapshot
            .changes_since(&snapshot)
            .expect("same observation")
            .changed
            .is_empty()
    );
    let mut changed_root = root.clone();
    changed_root.children[0] = Element::text(id(2), ElementKey::new("text"), "changed source");
    let changed = UiSnapshot::capture(
        UiSnapshotFrame {
            root: &changed_root,
            id: identity(2),
            ..frame()
        },
        UiQueryLimits::default(),
        |_| None,
        |_| false,
    )
    .expect("new observation");
    let delta = changed.changes_since(&snapshot).expect("delta");
    assert_eq!(delta.changed, vec![id(2)]);
    assert!(delta.added.is_empty() && delta.removed.is_empty() && delta.complete);
    let debug = format!("{snapshot:?}");
    assert!(
        !debug.contains(secret),
        "all password descendants and semantic channels are redacted"
    );
    assert!(snapshot.nodes()[2].value.redacted);
    assert_eq!(
        snapshot.nodes()[1].action,
        UiActionAvailability::ModalBlocked
    );
    assert_eq!(snapshot.nodes()[5].action, UiActionAvailability::Disabled);
    assert_eq!(
        snapshot.nodes()[7].action,
        UiActionAvailability::HostUnvalidated
    );
    assert!(snapshot.nodes()[7].focused);
    assert!(snapshot.nodes()[8].visible_bounds.is_none());
    let page = snapshot
        .query(UiQuery {
            snapshot: identity(1),
            root: Some(id(7)),
            after: None,
            limit: 1,
        })
        .expect("page");
    assert_eq!(page.next_after, Some(id(7)));
    let next = snapshot
        .query(UiQuery {
            snapshot: identity(1),
            root: Some(id(7)),
            after: page.next_after,
            limit: 1,
        })
        .expect("next");
    assert_eq!(next.nodes[0].id, id(8));
    assert!(next.next_after.is_none());
    for (query, expected) in [
        (
            UiQuery {
                snapshot: identity(2),
                root: None,
                after: None,
                limit: 1,
            },
            UiQueryError::StaleSnapshot,
        ),
        (
            UiQuery {
                snapshot: UiSnapshotId {
                    window: UiWindowId(2),
                    ..identity(1)
                },
                root: None,
                after: None,
                limit: 1,
            },
            UiQueryError::ForeignWindow,
        ),
        (
            UiQuery {
                snapshot: identity(1),
                root: Some(id(7)),
                after: Some(id(2)),
                limit: 1,
            },
            UiQueryError::InvalidCursor,
        ),
        (
            UiQuery {
                snapshot: identity(1),
                root: Some(id(99)),
                after: None,
                limit: 1,
            },
            UiQueryError::NodeNotCaptured,
        ),
    ] {
        assert_eq!(snapshot.query(query).expect_err("reject"), expected);
    }
    let limited = UiSnapshot::capture(
        frame(),
        UiQueryLimits {
            nodes: 2,
            text_bytes: 7,
            depth: 8,
        },
        |_| None,
        |_| false,
    )
    .expect("limits");
    assert!(limited.node_limit_reached && limited.text_truncated);
    let incomplete = limited
        .changes_since(&snapshot)
        .expect("partial observation");
    assert!(
        !incomplete.complete && incomplete.removed.is_empty(),
        "uncaptured nodes are not reported removed"
    );
    assert_eq!(limited.nodes()[1].text.value.as_deref(), Some("地形"));
    let shallow = UiSnapshot::capture(
        frame(),
        UiQueryLimits {
            depth: 1,
            ..Default::default()
        },
        |_| None,
        |_| false,
    )
    .expect("depth");
    assert_eq!(shallow.nodes().len(), 1);
    assert!(shallow.depth_limit_reached);
    let sensitive = UiSnapshot::capture(
        frame(),
        UiQueryLimits::default(),
        |_| None,
        |element| element == id(2),
    )
    .expect("sensitive");
    assert!(sensitive.nodes()[1].text.redacted);
}

#[test]
fn editor_observations_preserve_utf8_selection_and_never_reveal_password_or_compose_positions() {
    let mut context = TextInputContext::default();
    assert!(context.register_font(std::sync::Arc::new(
        include_bytes!("../../tests/fixtures/MaterialIcons-Regular.ttf").to_vec(),
    )));
    let mut editor = TextInputState::new_multiline(14.0, "Material Icons", 200.0);
    editor.reset(&mut context, "你好🙂").expect("editor");
    editor.act(&mut context, TextInputAction::SelectAll);
    assert_eq!(editor.inspection_selection(), Some((0, "你好🙂".len())));
    let root = Element::text_input(id(1), ElementKey::new("editor"), editor.snapshot(), true)
        .styled(Style::default().size(Dimension::Px(200.0), Dimension::Px(100.0)));
    let layout = LayoutEngine
        .compute(&root, Viewport::new(200.0, 100.0, 1.0))
        .expect("layout");
    let route = ActionRoute::<u8>::build(&root, []).expect("route");
    let snapshot = UiSnapshot::capture(
        UiSnapshotFrame {
            id: identity(1),
            root: &root,
            layout: &layout,
            route: &route,
            focus: None,
            modal_root: None,
        },
        UiQueryLimits::default(),
        |_| {
            Some(UiFieldContent {
                text: editor.text(),
                selection: editor.inspection_selection(),
                read_only: false,
                composing: false,
                sensitive: false,
            })
        },
        |_| false,
    )
    .expect("capture");
    assert_eq!(snapshot.nodes()[0].value.value.as_deref(), Some("你好🙂"));
    assert_eq!(snapshot.nodes()[0].selection, Some((0, "你好🙂".len())));
    let mut password = TextInputState::new_password(14.0, "sans-serif", 128);
    password.reset(&mut context, "private").expect("password");
    assert_eq!(password.inspection_selection(), None);
    editor
        .preedit(&mut context, "n", Some((1, 1)))
        .expect("composition");
    assert!(editor.is_composing());
    assert_eq!(editor.inspection_selection(), None);
    assert_eq!(
        editor.text(),
        "你好🙂",
        "inspection uses committed text during preedit"
    );
    let foreign = UiSnapshot::capture(
        UiSnapshotFrame {
            id: identity(1),
            root: &root,
            layout: &layout,
            route: &route,
            focus: Some(InputTarget {
                window: UiWindowId(2),
                element: id(1),
            }),
            modal_root: None,
        },
        UiQueryLimits::default(),
        |_| None,
        |_| false,
    );
    assert_eq!(
        foreign.expect_err("foreign focus"),
        UiQueryError::ForeignWindow
    );
}
