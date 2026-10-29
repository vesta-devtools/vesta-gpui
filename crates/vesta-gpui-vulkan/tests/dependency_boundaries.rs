use std::{fs, path::PathBuf, process::Command};

#[test]
fn renderer_does_not_depend_on_other_backends_or_author_models() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("manifest");
    let (production, examples) = manifest
        .split_once("[dev-dependencies]")
        .expect("public native examples declare their host-only dependencies");
    for host_dependency in ["vesta-gpui-widgets", "vesta-gpui-winit", "arboard"] {
        assert!(
            !production.contains(host_dependency),
            "example host leaked into renderer: {host_dependency}"
        );
        assert!(
            examples.contains(host_dependency),
            "native consumer dependency missing: {host_dependency}"
        );
    }
    assert!(manifest.contains("viewport = [\"dep:vesta-viewport\"]"));
    assert!(manifest.contains("vesta-viewport = { workspace = true, optional = true }"));
    // ADR-0544's opt-in transfer measurement uses compute/author crates only
    // in dev-dependencies. Check the actual production graph, including the
    // optional viewport, so that transitive renderer leaks still fail.
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--locked",
            "-p",
            "vesta-gpui-vulkan",
            "--all-features",
            "--edges",
            "normal,build",
            "--prefix",
            "none",
        ])
        .current_dir(&root)
        .output()
        .expect("inspect renderer production dependencies");
    assert!(
        output.status.success(),
        "cargo tree: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8_lossy(&output.stdout);
    for forbidden in [
        "bevy",
        "wgpu",
        "vesta-studio",
        "vesta-graph",
        "vesta-executor",
        "vesta-scene",
    ] {
        assert!(
            !production.contains(forbidden)
                && !tree.lines().any(|line| {
                    line.split_whitespace().next().is_some_and(|name| {
                        name == forbidden || (forbidden == "bevy" && name.starts_with("bevy"))
                    })
                }),
            "forbidden dependency: {forbidden}"
        );
    }
    let source = fs::read_to_string(root.join("src/lib.rs")).expect("source");
    assert!(source.contains("#![deny(unsafe_code)]"));
    // ADR-0640 adds only a read-only monitor query beside checked_pipeline.
    assert_eq!(source.matches("#[allow(unsafe_code)]").count(), 2);
    assert!(source.contains("mod popup_monitor;"));
    assert!(production.contains("windows-sys"));
    let platform = fs::read_to_string(root.join("src/popup_monitor.rs")).expect("platform query");
    assert!(platform.contains("GetMonitorInfoW"));
    for write in [
        "SetWindow",
        "ShowWindow",
        "SetForegroundWindow",
        "GWLP_HWNDPARENT",
    ] {
        assert!(
            !platform.contains(write),
            "popup query gained window mutation: {write}"
        );
    }
    for name in ["lib.rs", "painter.rs", "viewport.rs", "targets.rs"] {
        let source = fs::read_to_string(root.join("src").join(name)).expect("source");
        assert!(!source.contains("unsafe {"), "unsafe leaked into {name}");
    }
}
