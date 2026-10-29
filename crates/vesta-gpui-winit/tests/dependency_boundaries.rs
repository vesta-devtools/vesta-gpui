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
}
