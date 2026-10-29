use std::{path::Path, process::Command};

#[test]
fn workspace_state_excludes_render_host_and_graph_crates() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO"))
        .args(["tree", "-p", "vesta-workspace", "--edges", "normal"])
        .current_dir(workspace_root)
        .output()
        .expect("cargo tree should be available during workspace tests");

    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let dependency_tree = String::from_utf8_lossy(&output.stdout);
    for forbidden in ["bevy", "wgpu", "winit", "taffy", "vesta-gpui", "vesta-core"] {
        assert!(
            !dependency_tree.contains(forbidden),
            "vesta-workspace must not depend on {forbidden}:\n{dependency_tree}"
        );
    }
}
