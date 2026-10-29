use std::{fs, path::PathBuf};

#[test]
fn standalone_has_no_studio_or_domain_dependencies() {
    let manifest = fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("read standalone manifest");
    for forbidden in [
        "vesta-studio",
        "vesta-graph",
        "vesta-scene",
        "vesta-executor",
        "vesta-sop",
        "vesta-agent",
        "bevy",
        "wgpu",
        "pollster",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "standalone must not depend on {forbidden}"
        );
    }
}
