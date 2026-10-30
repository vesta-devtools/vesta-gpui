use std::{fs, path::Path};

#[test]
fn winit_adapter_excludes_render_and_product_crates() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let content = fs::read_to_string(manifest).expect("winit adapter manifest readable");
    for forbidden in [
        "bevy",
        "wgpu",
        "vesta-studio",
        "vesta-graph",
        "vesta-scene",
        "vesta-executor",
    ] {
        assert!(
            !content.contains(&format!("{forbidden} =")),
            "winit adapter must not depend on {forbidden}"
        );
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(root.join("src/lib.rs")).expect("adapter source");
    assert!(source.contains("#![deny(unsafe_code)]"));
    assert_eq!(source.matches("#[allow(unsafe_code)]").count(), 1);
    assert!(source.contains("mod monitor;"));
    let platform = fs::read_to_string(root.join("src/monitor.rs")).expect("platform query");
    assert!(platform.contains("GetMonitorInfoW"));
    for write in [
        "SetWindow",
        "ShowWindow",
        "SetForegroundWindow",
        "GWLP_HWNDPARENT",
    ] {
        assert!(
            !platform.contains(write),
            "monitor query gained window mutation: {write}"
        );
    }
}
