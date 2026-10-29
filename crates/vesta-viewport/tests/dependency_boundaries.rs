use std::{fs, path::PathBuf};

#[test]
fn viewport_contract_has_no_render_or_product_dependencies() {
    let manifest = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("read manifest");
    for forbidden in ["wgpu", "bevy", "vesta-studio", "vesta-graph", "vesta-scene"] {
        assert!(
            !manifest.contains(forbidden),
            "forbidden dependency: {forbidden}"
        );
    }
}
