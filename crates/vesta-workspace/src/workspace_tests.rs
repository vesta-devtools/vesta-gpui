use super::*;

fn three_pane_workspace() -> WorkspaceModel {
    let panes = vec![
        Pane::new(PaneId(1)),
        Pane::new(PaneId(2)),
        Pane::new(PaneId(3)),
    ];
    let root = WorkspaceNode::split(
        SplitId(1),
        SplitAxis::Row,
        0.7,
        WorkspaceNode::leaf(PaneId(1)),
        WorkspaceNode::split(
            SplitId(2),
            SplitAxis::Column,
            0.5,
            WorkspaceNode::leaf(PaneId(2)),
            WorkspaceNode::leaf(PaneId(3)),
        ),
    );
    let mut workspace = WorkspaceModel::with_tree(root, panes).expect("valid initial tree");
    for (pane, title) in [
        (PaneId(1), "Viewport"),
        (PaneId(2), "Details"),
        (PaneId(3), "Network"),
    ] {
        workspace
            .add_item(pane, title, PanelTypeId::new("test"))
            .expect("initial item");
    }
    workspace
}

fn add(mut workspace: WorkspaceModel, pane: PaneId, title: &str) -> (WorkspaceModel, ItemId) {
    let id = workspace
        .add_item(pane, title, PanelTypeId::new("test"))
        .expect("add item");
    (workspace, id)
}

#[test]
fn same_pane_merge_preserves_the_requested_pre_move_insertion_slot() {
    for (source, slot, expected) in [
        (2, 1, [0, 2, 1, 3]),
        (3, 2, [0, 1, 3, 2]),
        (1, 3, [0, 2, 1, 3]),
        (0, 4, [1, 2, 3, 0]),
        (2, 2, [0, 1, 2, 3]),
        (3, 0, [3, 0, 1, 2]),
    ] {
        let pane = PaneId(1);
        let mut model = WorkspaceModel::with_tree(WorkspaceNode::leaf(pane), [Pane::new(pane)])
            .expect("initial tree");
        let ids = (0..4)
            .map(|i| {
                model
                    .add_item(pane, i.to_string(), PanelTypeId::new("test"))
                    .expect("add tab")
            })
            .collect::<Vec<_>>();
        model
            .dock_item(ids[source], DockAction::Merge { pane, slot })
            .expect("merge at pre-move slot");
        assert_eq!(
            model
                .pane(pane)
                .expect("live pane")
                .items
                .iter()
                .map(|i| i.id)
                .collect::<Vec<_>>(),
            expected.map(|i| ids[i]),
            "source={source} slot={slot}"
        );
        let restored = WorkspaceSession::from_json(
            &WorkspaceSession::capture(&model)
                .expect("capture ordered model")
                .to_json()
                .expect("encode ordered session"),
        )
        .expect("decode ordered envelope")
        .restore(|_| true)
        .expect("restore ordered model");
        assert_eq!(restored.workspace, model);
    }
}

#[test]
fn constructors_and_rejected_mutations_preserve_the_session_invariants() {
    let mut model = three_pane_workspace();
    let before = model.clone();
    assert!(
        model
            .add_item(PaneId(999), "missing", PanelTypeId::new("test"))
            .is_err()
    );
    assert_eq!(model, before, "rejected add must not consume an identity");
    let item = model.active_item().expect("seeded active item").id;
    assert!(
        model
            .dock_item(
                item,
                DockAction::Float {
                    placement: WindowPlacement::new(0.0, 0.0, -1.0, 200.0)
                }
            )
            .is_err(),
        "invalid placement cannot enter the live model"
    );
    assert_eq!(model, before);
    assert_eq!(
        model.set_item_type(ItemId(999), PanelTypeId::new("test"), "missing"),
        Err(WorkspaceError::MissingItem(ItemId(999)))
    );
    assert_eq!(model, before);
    model
        .dock_item(
            item,
            DockAction::Float {
                placement: WindowPlacement::new(-100.0, 20.0, 300.0, 200.0),
            },
        )
        .expect("valid floating window");
    let floating = model.floating_windows()[0].id;
    let floated = model.clone();
    for placement in [
        WindowPlacement::new(0.0, 0.0, 0.0, 20.0),
        WindowPlacement::new(f32::MAX, 0.0, f32::MAX, 20.0),
    ] {
        assert_eq!(
            model.set_floating_placement(floating, placement),
            Err(WorkspaceError::InvalidWindowPlacement(floating))
        );
        assert_eq!(
            model, floated,
            "invalid native placement cannot corrupt persisted state"
        );
    }
    model
        .set_floating_title(floating, "Renamed")
        .expect("known window");
    WorkspaceSession::capture(&model).expect("narrow updates retain invariants");
    let repeated = WorkspaceNode::split(
        SplitId(1),
        SplitAxis::Row,
        0.5,
        WorkspaceNode::leaf(PaneId(1)),
        WorkspaceNode::leaf(PaneId(1)),
    );
    assert!(
        WorkspaceModel::with_tree(repeated, [Pane::new(PaneId(1))]).is_err(),
        "constructor must reject repeated tree ownership"
    );
    assert!(
        WorkspaceModel::with_tree(
            WorkspaceNode::leaf(PaneId(1)),
            [Pane::new(PaneId(1)), Pane::new(PaneId(2))]
        )
        .is_err(),
        "constructor must reject an orphan pane"
    );
    let mut inactive = Pane::new(PaneId(1));
    inactive.items.push(WorkspaceItem {
        id: ItemId(1),
        title: "inactive".into(),
        panel_type: PanelTypeId::new("test"),
    });
    assert!(
        WorkspaceModel::with_tree(WorkspaceNode::leaf(PaneId(1)), [inactive]).is_err(),
        "nonempty pane needs a valid active item"
    );
}

#[test]
fn exhausted_allocators_reject_compound_changes_without_losing_items() {
    let initial = three_pane_workspace();
    let json = WorkspaceSession::capture(&initial)
        .expect("capture seeded model")
        .to_json()
        .expect("encode seeded session");
    for kind in ["item", "pane", "split", "window"] {
        let mut value: serde_json::Value = serde_json::from_str(&json).expect("seed envelope");
        value["workspace"][format!("next_{kind}_id")] = u64::MAX.into();
        let mut model = WorkspaceSession::from_json(&value.to_string())
            .expect("decode exhausted envelope")
            .restore(|_| true)
            .expect("restore valid terminal cursor")
            .workspace;
        let before = model.clone();
        let item = model.active_item().expect("seeded active item").id;
        let rejected = match kind {
            "item" => model
                .add_item(PaneId(1), "exhausted", PanelTypeId::new("test"))
                .map(|_| ()),
            "pane" | "split" => model.dock_item(
                item,
                DockAction::Split {
                    pane: PaneId(1),
                    side: Side::Left,
                },
            ),
            "window" => model.dock_item(
                item,
                DockAction::Float {
                    placement: WindowPlacement::new(0.0, 0.0, 300.0, 200.0),
                },
            ),
            _ => unreachable!(),
        };
        assert_eq!(rejected, Err(WorkspaceError::IdentityExhausted(kind)));
        assert_eq!(
            model, before,
            "failed compound change must retain {kind} identities"
        );
        WorkspaceSession::capture(&model).expect("failed change preserves session invariants");
        assert_eq!(
            model.spawn_floating_window(
                "exhausted".into(),
                WindowPlacement::new(0.0, 0.0, 300.0, 200.0),
                "new".into(),
                PanelTypeId::new("test")
            ),
            if kind == "split" {
                Ok(FloatingWindowId(1))
            } else {
                Err(WorkspaceError::IdentityExhausted(kind))
            }
        );
        if kind != "split" {
            assert_eq!(model, before);
        }
        if kind == "pane" {
            let mut empty = WorkspaceModel::with_tree(
                initial.root().clone(),
                [
                    Pane::new(PaneId(1)),
                    Pane::new(PaneId(2)),
                    Pane::new(PaneId(3)),
                ],
            )
            .expect("empty tree");
            let only = empty
                .add_item(PaneId(3), "last", PanelTypeId::new("test"))
                .expect("only tab");
            let mut snapshot = serde_json::to_value(
                WorkspaceSession::capture(&empty).expect("capture empty tree"),
            )
            .expect("snapshot value");
            snapshot["workspace"]["next_pane_id"] = u64::MAX.into();
            let mut empty = WorkspaceSession::from_json(&snapshot.to_string())
                .expect("decode terminal cursor")
                .restore(|_| true)
                .expect("restore terminal cursor")
                .workspace;
            empty
                .close_item(only)
                .expect("close requires no new identity");
            assert_eq!(
                empty.root().panes(),
                vec![PaneId(1)],
                "retain an existing empty main pane"
            );
            WorkspaceSession::capture(&empty).expect("terminal pane allocator survives close");
        }
    }
}

#[test]
fn with_tree_rejects_duplicate_and_missing_identity() {
    let panes = vec![Pane::new(PaneId(1)), Pane::new(PaneId(1))];
    assert!(matches!(
        WorkspaceModel::with_tree(WorkspaceNode::leaf(PaneId(1)), panes),
        Err(WorkspaceError::DuplicatePane(id)) if id == PaneId(1)
    ));

    let panes = vec![Pane::new(PaneId(1))];
    assert!(matches!(
        WorkspaceModel::with_tree(WorkspaceNode::leaf(PaneId(99)), panes),
        Err(WorkspaceError::MissingPane(id)) if id == PaneId(99)
    ));
}

#[test]
fn with_tree_resumes_item_ids_after_pre_seeded_items() {
    let mut pane = Pane::new(PaneId(1));
    pane.items.push(WorkspaceItem {
        id: ItemId(42),
        title: "Seeded".to_owned(),
        panel_type: PanelTypeId::new("test"),
    });
    pane.active = Some(ItemId(42));
    let mut workspace =
        WorkspaceModel::with_tree(WorkspaceNode::leaf(PaneId(1)), [pane]).expect("valid tree");
    let fresh = workspace
        .add_item(PaneId(1), "Fresh", PanelTypeId::new("test"))
        .expect("add after seeded item");
    assert_ne!(fresh, ItemId(42), "allocator continues after seeded ids");
    assert_eq!(fresh, ItemId(43));
}

#[test]
fn with_tree_rejects_duplicate_item_ids() {
    let mut first = Pane::new(PaneId(1));
    first.items.push(WorkspaceItem {
        id: ItemId(7),
        title: "A".to_owned(),
        panel_type: PanelTypeId::new("test"),
    });
    first.active = Some(ItemId(7));
    let mut second = Pane::new(PaneId(2));
    second.items.push(WorkspaceItem {
        id: ItemId(7),
        title: "B".to_owned(),
        panel_type: PanelTypeId::new("test"),
    });
    second.active = Some(ItemId(7));
    let root = WorkspaceNode::split(
        SplitId(1),
        SplitAxis::Row,
        0.5,
        WorkspaceNode::leaf(PaneId(1)),
        WorkspaceNode::leaf(PaneId(2)),
    );
    assert!(matches!(
        WorkspaceModel::with_tree(root, [first, second]),
        Err(WorkspaceError::DuplicateItem(id)) if id == ItemId(7)
    ));
}

#[test]
fn failed_dock_never_loses_the_item() {
    let workspace = three_pane_workspace();
    let (mut workspace, item) = add(workspace, PaneId(2), "Keep me");
    let result = workspace.dock_item(
        item,
        DockAction::Merge {
            pane: PaneId(999),
            slot: 0,
        },
    );
    assert!(matches!(result, Err(WorkspaceError::MissingPane(_))));
    assert_eq!(
        workspace.item(item).map(|entry| entry.title.as_str()),
        Some("Keep me"),
        "a rejected dock must leave the item in place"
    );
    let result = workspace.dock_item(
        item,
        DockAction::Split {
            pane: PaneId(999),
            side: Side::Left,
        },
    );
    assert!(matches!(result, Err(WorkspaceError::MissingPane(_))));
    assert_eq!(
        workspace.item(item).map(|entry| entry.title.as_str()),
        Some("Keep me")
    );
    let result = workspace.dock_item(
        item,
        DockAction::WindowSplit {
            container: ContainerRef::Floating(FloatingWindowId(99)),
            side: Side::Right,
        },
    );
    assert!(matches!(result, Err(WorkspaceError::MissingContainer(_))));
    assert_eq!(
        workspace.item(item).map(|entry| entry.title.as_str()),
        Some("Keep me")
    );
    assert_eq!(
        workspace.panes().len(),
        3,
        "no pane was reclaimed by a failed dock"
    );
}

#[test]
fn non_finite_ratios_fall_back_instead_of_poisoning_layout() {
    let workspace = three_pane_workspace();
    let (mut workspace, _) = add(workspace, PaneId(1), "Any");
    workspace
        .set_split_ratio(ContainerRef::Main, SplitId(1), f32::NAN)
        .expect("NaN ratio falls back to the previous value");
    let ratio = match workspace.root() {
        WorkspaceNode::Split { ratio, .. } => *ratio,
        WorkspaceNode::Leaf(_) => 0.0,
    };
    assert!(ratio.is_finite(), "NaN must not enter the model");
    assert!(
        (ratio - 0.7).abs() < 1e-6,
        "fallback keeps the original 0.7"
    );

    let node = WorkspaceNode::split(
        SplitId(99),
        SplitAxis::Column,
        f32::NAN,
        WorkspaceNode::leaf(PaneId(1)),
        WorkspaceNode::leaf(PaneId(2)),
    );
    match node {
        WorkspaceNode::Split { ratio, .. } => assert!((ratio - 0.5).abs() < 1e-6),
        WorkspaceNode::Leaf(_) => unreachable!(),
    }
}

#[test]
fn merge_across_panes_reclaims_the_empty_source_pane() {
    let mut workspace = three_pane_workspace();
    let item = workspace.pane(PaneId(2)).expect("details pane").items[0].id;

    workspace
        .dock_item(
            item,
            DockAction::Merge {
                pane: PaneId(1),
                slot: 0,
            },
        )
        .expect("cross-pane merge");
    assert!(
        workspace.pane(PaneId(2)).is_none(),
        "empty source pane is reclaimed"
    );
    assert!(
        workspace
            .pane(PaneId(1))
            .expect("target")
            .items
            .iter()
            .any(|candidate| candidate.id == item)
    );
    assert_eq!(workspace.active_pane(), PaneId(1));
    assert_eq!(workspace.panes().len(), 2);
}

#[test]
fn dock_inside_a_floating_window_targets_that_window() {
    let workspace = three_pane_workspace();
    let (workspace, item) = add(workspace, PaneId(1), "Floaty");
    let (mut workspace, second) = add(workspace, PaneId(1), "Second");
    workspace
        .dock_item(
            item,
            DockAction::Float {
                placement: WindowPlacement::new(0.0, 0.0, 400.0, 300.0),
            },
        )
        .expect("float");
    let window = workspace.floating_windows().first().expect("one window").id;

    workspace
        .dock_item(
            second,
            DockAction::Split {
                pane: PaneId(4),
                side: Side::Right,
            },
        )
        .expect("split inside the floating window");
    let window = workspace.floating_window(window).expect("window exists");
    assert!(matches!(window.root, WorkspaceNode::Split { .. }));
    assert!(workspace.pane(PaneId(5)).is_some());
    assert!(
        workspace.pane(PaneId(1)).is_some(),
        "source keeps its other tabs"
    );
}

#[test]
fn closing_every_main_tab_keeps_exactly_one_pane() {
    let mut workspace = three_pane_workspace();
    for pane in [PaneId(1), PaneId(2), PaneId(3)] {
        let item = workspace.pane(pane).expect("pane").items[0].id;
        workspace.close_item(item).expect("close");
    }
    assert_eq!(
        workspace.panes().len(),
        1,
        "main window keeps exactly one pane"
    );
    assert!(workspace.pane(workspace.active_pane()).is_some());
}

#[test]
fn close_floating_window_fixes_active_pane() {
    let workspace = three_pane_workspace();
    let (mut workspace, item) = add(workspace, PaneId(1), "Floaty");
    workspace
        .dock_item(
            item,
            DockAction::Float {
                placement: WindowPlacement::new(0.0, 0.0, 400.0, 300.0),
            },
        )
        .expect("float");
    let window_id = workspace.floating_windows().first().expect("one window").id;
    assert_eq!(workspace.active_pane(), PaneId(4));
    workspace.close_floating_window(window_id).expect("close");
    assert_eq!(workspace.active_pane(), PaneId(1));
    assert!(workspace.pane(PaneId(4)).is_none());
    WorkspaceSession::capture(&workspace)
        .expect("capture valid layout")
        .restore(|_| true)
        .expect("closed window leaves a valid session");
}

#[test]
fn native_window_close_restores_items_without_changing_identity_or_order() {
    let workspace = three_pane_workspace();
    let (workspace, first) = add(workspace, PaneId(1), "Float first");
    let (mut workspace, second) = add(workspace, PaneId(1), "Float second");
    workspace
        .dock_item(
            first,
            DockAction::Float {
                placement: WindowPlacement::new(20.0, 30.0, 420.0, 320.0),
            },
        )
        .expect("float first item");
    let window = workspace.floating_windows()[0].id;
    let floating_pane = workspace.floating_windows()[0].root.panes()[0];
    workspace
        .dock_item(
            second,
            DockAction::Merge {
                pane: floating_pane,
                slot: 1,
            },
        )
        .expect("merge second item into floating window");

    let before = workspace.clone();
    assert!(matches!(
        workspace.restore_floating_window(window, PaneId(99)),
        Err(WorkspaceError::MissingPane(PaneId(99)))
    ));
    assert_eq!(workspace, before, "failed restore is atomic");

    assert_eq!(
        workspace
            .restore_floating_window(window, PaneId(1))
            .expect("restore native floating window"),
        vec![first, second]
    );
    assert!(workspace.floating_window(window).is_none());
    let pane = workspace.pane(PaneId(1)).expect("main pane remains");
    let restored = pane
        .items
        .iter()
        .filter(|item| item.id == first || item.id == second)
        .map(|item| item.id)
        .collect::<Vec<_>>();
    assert_eq!(restored, vec![first, second]);
}

#[test]
fn versioned_session_round_trips_dock_identity_and_reports_missing_panels() {
    // A fixed v1 wire sample protects the schema independently of today's writer.
    let v1 = include_str!("../tests/fixtures/session_v1.json");
    let legacy = WorkspaceSession::from_json(v1)
        .expect("decode fixed v1 sample")
        .restore(|_| true)
        .expect("restore v1 sample")
        .workspace;
    let current = WorkspaceSession::capture(&legacy)
        .expect("capture restored v1")
        .to_json()
        .expect("encode v1");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&current).expect("current JSON"),
        serde_json::from_str::<serde_json::Value>(v1).expect("v1 JSON")
    );
    let mut workspace = three_pane_workspace();
    let item = workspace.pane(PaneId(2)).expect("details").items[0].id;
    workspace
        .dock_item(
            item,
            DockAction::Float {
                placement: WindowPlacement::new(20.0, 30.0, 420.0, 320.0),
            },
        )
        .expect("float details");
    workspace
        .set_item_type(item, PanelTypeId::new("missing.inspector"), "Inspector")
        .expect("known item");
    let json = WorkspaceSession::capture(&workspace)
        .expect("capture valid layout")
        .to_json()
        .expect("encode session");
    let restored = WorkspaceSession::from_json(&json)
        .expect("decode session")
        .restore(|panel| panel.as_str() == "test")
        .expect("restore valid layout");
    assert_eq!(restored.workspace, workspace);
    assert_eq!(
        restored.unavailable_panels,
        vec![UnavailablePanel {
            item,
            panel_type: PanelTypeId::new("missing.inspector"),
        }]
    );
    assert_eq!(
        restored
            .workspace
            .item(item)
            .map(|item| item.panel_type.as_str()),
        Some("missing.inspector"),
        "an unavailable panel remains recoverable"
    );
}

#[test]
fn session_restore_rejects_stale_identity_and_unsupported_schema() {
    let workspace = three_pane_workspace();
    let json = WorkspaceSession::capture(&workspace)
        .expect("capture valid layout")
        .to_json()
        .expect("encode");
    let mut value: serde_json::Value = serde_json::from_str(&json).expect("JSON");
    value["workspace"]["next_item_id"] = 1.into();
    let session = WorkspaceSession::from_json(&value.to_string()).expect("decode envelope");
    let error = session.restore(|_| true).expect_err("stale allocator");
    assert_eq!(
        error,
        WorkspaceSessionError::InvalidWorkspace(WorkspaceError::InvalidAllocator("item"))
    );
    assert_eq!(
        std::error::Error::source(&error).and_then(|cause| cause.downcast_ref::<WorkspaceError>()),
        Some(&WorkspaceError::InvalidAllocator("item"))
    );

    value["version"] = 99.into();
    let session = WorkspaceSession::from_json(&value.to_string()).expect("decode envelope");
    assert_eq!(
        session.restore(|_| true),
        Err(WorkspaceSessionError::UnsupportedVersion(99))
    );

    let mut duplicate_leaf: serde_json::Value = serde_json::from_str(&json).expect("JSON");
    duplicate_leaf["workspace"]["root"]["Split"]["second"]["Split"]["first"] =
        serde_json::json!({ "Leaf": 1 });
    let session = WorkspaceSession::from_json(&duplicate_leaf.to_string()).expect("decode");
    assert_eq!(
        session.restore(|_| true),
        Err(WorkspaceSessionError::InvalidWorkspace(
            WorkspaceError::DuplicatePane(PaneId(1))
        ))
    );
}

#[test]
fn nested_split_promotes_the_surviving_child() {
    let mut workspace = three_pane_workspace();
    let other = workspace.pane(PaneId(3)).expect("network pane").items[0].id;
    // Merge the network item into details, reclaiming pane 3 and collapsing the
    // column split into a plain leaf.
    workspace
        .dock_item(
            other,
            DockAction::Merge {
                pane: PaneId(2),
                slot: 1,
            },
        )
        .expect("merge");
    assert!(workspace.pane(PaneId(3)).is_none());
    assert!(workspace.pane(PaneId(2)).is_some());
    let panes = workspace.panes();
    assert_eq!(panes.len(), 2, "column split collapses to two leaves");
    assert_eq!(panes[0].id, PaneId(1));
    assert_eq!(panes[1].id, PaneId(2));
    assert_eq!(panes[1].items.len(), 2);
}
