#[test]
fn browser_carrier_does_not_import_product_or_render_ownership() {
    let manifest = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .expect("manifest");
    for dependency in [
        "developer-agent",
        "bevy",
        "vesta-graph",
        "vesta-executor",
        "vesta-gpui-vulkan",
        "vesta-studio",
        "vesta-workspace",
        "vesta-gpui-widgets",
        "tao =",
    ] {
        assert!(
            !manifest.contains(dependency),
            "browser carrier must not depend on {dependency}"
        );
    }
}
