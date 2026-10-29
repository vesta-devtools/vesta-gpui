use std::{fs, path::Path};

#[test]
fn ui_crate_excludes_render_host_graph_and_workspace_crates() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let content = fs::read_to_string(manifest).expect("vesta-gpui manifest readable");
    for forbidden in [
        "bevy",
        "wgpu",
        "winit",
        "vesta-core",
        "vesta-data",
        "vesta-graph",
        "vesta-sop",
        "vesta-workspace",
        "arboard",
    ] {
        assert!(
            !content.contains(&format!("{forbidden} =")),
            "vesta-gpui must not declare a dependency on {forbidden}"
        );
    }
    let parley = content
        .lines()
        .find(|line| line.starts_with("parley ="))
        .expect("explicit layout dependency");
    assert!(
        parley.contains("default-features = false") && !parley.contains("\"system\""),
        "system font discovery must remain a host responsibility"
    );
    assert!(
        parley.contains("\"accesskit\""),
        "text semantics must come from Parley"
    );
    assert!(
        content.contains("accesskit = \"=0.21.1\""),
        "UI and host share the locked semantic schema"
    );
}
