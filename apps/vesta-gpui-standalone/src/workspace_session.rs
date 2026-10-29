//! Local layout storage for the standalone acceptance host.

use std::{env, fs, path::PathBuf};

use vesta_workspace::{RestoredWorkspace, WorkspaceModel, WorkspaceSession};

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
