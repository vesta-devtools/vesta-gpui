use std::{collections::BTreeSet, process::Command};

#[test]
fn reusable_workspace_projection_has_no_product_or_renderer_dependency() {
    let output = Command::new(env!("CARGO"))
        .args(["tree", "-p", "vesta-gpui-workspace", "--prefix", "none"])
        .output()
        .expect("cargo tree should run");
    assert!(output.status.success());
    let tree = String::from_utf8(output.stdout).expect("cargo tree is utf8");
    let forbidden = BTreeSet::from([
        "vesta-studio",
        "vesta-graph",
        "vesta-scene",
        "vesta-executor",
        "bevy",
        "wgpu",
        "winit",
    ]);
    for name in forbidden {
        assert!(
            !tree.lines().any(|line| line.starts_with(name)),
            "forbidden dependency {name} found in:\n{tree}"
        );
    }
}
