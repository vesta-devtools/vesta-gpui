use std::{path::Path, process::Command};

fn depends_on(tree: &str, forbidden: &str) -> bool {
    tree.lines()
        .filter_map(|line| line.split_whitespace().next())
        .any(|name| name == forbidden || (forbidden == "bevy" && name.starts_with("bevy_")))
}

#[test]
fn workspace_state_excludes_render_host_and_graph_crates() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--locked",
            "-p",
            "vesta-workspace",
            "--edges",
            "normal",
            "--prefix",
            "none",
        ])
        .current_dir(workspace_root)
        .output()
        .expect("cargo tree should be available during workspace tests");

    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let dependency_tree = String::from_utf8_lossy(&output.stdout);
    // A real CI checkout path contains the forbidden package name. Paths are
    // not dependency identities; actual forbidden package rows still fail.
    let ci_path =
        "vesta-workspace v0.1.0 (D:/a/vesta-gpui/vesta-gpui/crates/vesta-workspace)\nserde v1.0.0";
    assert!(!depends_on(ci_path, "vesta-gpui"));
    assert!(depends_on("vesta-gpui v0.1.0 (D:/a/toolkit)", "vesta-gpui"));
    assert!(depends_on("bevy_render v0.18.1", "bevy"));
    for forbidden in [
        "bevy",
        "wgpu",
        "winit",
        "taffy",
        "vesta-gpui",
        "vesta-core",
        "vesta-data",
        "vesta-graph",
        "vesta-executor",
    ] {
        assert!(
            !depends_on(&dependency_tree, forbidden),
            "vesta-workspace must not depend on {forbidden}:\n{dependency_tree}"
        );
    }
}
