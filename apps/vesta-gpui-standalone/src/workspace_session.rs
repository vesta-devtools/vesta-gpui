//! Local layout storage for the standalone acceptance host.

use std::{env, fs, path::PathBuf};

use vesta_workspace::{
    ItemId, Pane, PaneId, PanelTypeId, RestoredWorkspace, SplitAxis, SplitId, WorkspaceModel,
    WorkspaceNode, WorkspaceSession,
};

fn session_path() -> Option<PathBuf> {
    let base = env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(
        base.join("Vesta")
            .join("ui-standalone")
            .join("workspace-v1.json"),
    )
}

pub fn load() -> Result<Option<RestoredWorkspace>, String> {
    let Some(path) = session_path() else {
        return Ok(None);
    };
    let backup = path.with_extension("json.bak");
    let mut errors = Vec::new();
    for candidate in [&path, &backup] {
        if !candidate.exists() {
            continue;
        }
        let result = fs::read_to_string(candidate)
            .map_err(|error| error.to_string())
            .and_then(|json| WorkspaceSession::from_json(&json).map_err(|error| error.to_string()))
            .and_then(|session| {
                session
                    .restore(|panel| {
                        matches!(
                            panel.as_str(),
                            "gallery.controls"
                                | "gallery.inputs"
                                | "gallery.collections"
                                | "gallery.layout"
                                | "gallery.documents"
                                | "gallery.blank"
                        )
                    })
                    .map_err(|error| error.to_string())
            });
        match result {
            Ok(restored) => return Ok(Some(restored)),
            Err(error) => errors.push(format!("{}: {error}", candidate.display())),
        }
    }
    if errors.is_empty() {
        Ok(None)
    } else {
        Err(errors.join("; "))
    }
}

pub fn save(workspace: &WorkspaceModel) -> Result<(), String> {
    let Some(path) = session_path() else {
        return Ok(());
    };
    let json = WorkspaceSession::capture(workspace)
        .map_err(|error| error.to_string())?
        .to_json()
        .map_err(|error| error.to_string())?;
    let parent = path
        .parent()
        .ok_or("workspace session has no parent directory")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    let backup = path.with_extension("json.bak");
    fs::write(&temporary, json).map_err(|error| error.to_string())?;
    if path.exists() {
        if backup.exists() {
            fs::remove_file(&backup).map_err(|error| error.to_string())?;
        }
        fs::rename(&path, &backup).map_err(|error| error.to_string())?;
    }
    if let Err(error) = fs::rename(&temporary, &path) {
        if backup.exists() {
            let _ = fs::rename(&backup, &path);
        }
        return Err(error.to_string());
    }
    Ok(())
}

pub(super) fn demo_workspace() -> WorkspaceModel {
    let left = PaneId(1);
    let right = PaneId(2);
    let root = WorkspaceNode::split(
        SplitId(1),
        SplitAxis::Row,
        0.5,
        WorkspaceNode::leaf(left),
        WorkspaceNode::leaf(right),
    );
    let mut model = WorkspaceModel::with_tree(root, [Pane::new(left), Pane::new(right)])
        .expect("the static standalone workspace is valid");
    let controls = model
        .add_item(left, "Controls", PanelTypeId::new("gallery.controls"))
        .expect("the static left pane exists");
    model
        .add_item(left, "Inputs", PanelTypeId::new("gallery.inputs"))
        .expect("the static left pane exists");
    let collections = model
        .add_item(
            right,
            "Collections",
            PanelTypeId::new("gallery.collections"),
        )
        .expect("the static right pane exists");
    model
        .add_item(right, "Layout", PanelTypeId::new("gallery.layout"))
        .expect("the static right pane exists");
    ensure_documents_tab(&mut model);
    model
        .activate(collections)
        .expect("static gallery tab exists");
    model.activate(controls).expect("static gallery tab exists");
    model
}

pub(super) fn ensure_documents_tab(model: &mut WorkspaceModel) -> Option<ItemId> {
    if let Some(item) = model
        .root()
        .panes()
        .into_iter()
        .chain(
            model
                .floating_windows()
                .iter()
                .flat_map(|window| window.root.panes()),
        )
        .filter_map(|pane| model.pane(pane))
        .flat_map(|pane| pane.items.iter())
        .find(|item| item.panel_type.as_str() == "gallery.documents")
    {
        return Some(item.id);
    }
    let pane = model.panes().first()?.id;
    model
        .add_item(pane, "Documents", PanelTypeId::new("gallery.documents"))
        .ok()
}
