use std::{fs, path::Path};

#[test]
fn display_showcase_consumes_public_ui_without_graph_or_domain_execution() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("manifest");
    for required in [
        "vesta-gpui",
        "vesta-gpui-widgets",
        "vesta-gpui-winit",
        "vesta-gpui-vulkan",
        "vesta-viewport",
    ] {
        assert!(
            manifest.contains(&format!("{required}.workspace"))
                || manifest.contains(&format!("{required} ="))
        );
    }
    for forbidden in [
        "vesta-sop",
        "vesta-executor",
        "vesta-graph",
        "vesta-scene",
        "vesta-backend",
        "bevy",
        "egui",
        "wgpu",
        "webview",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "host display boundary: {forbidden}"
        );
    }
    assert!(
        !manifest
            .lines()
            .any(|line| line.trim_start().starts_with("gpui ="))
    );
    let source = fs::read_to_string(root.join("src/main.rs")).expect("source");
    assert!(source.contains("#![forbid(unsafe_code)]"));
}
