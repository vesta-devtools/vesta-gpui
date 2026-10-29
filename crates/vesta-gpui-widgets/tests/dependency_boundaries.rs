use std::{fs, path::Path};

#[test]
fn widgets_remain_backend_and_product_independent() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let content = fs::read_to_string(manifest).expect("widget manifest readable");
    assert!(
        content
            .lines()
            .any(|line| line.starts_with("pulldown-cmark =")
                && line.contains("\"=0.13.0\"")
                && line.contains("default-features = false")),
        "Markdown is a locked parser without HTML emission or a web view"
    );
    for forbidden in [
        "bevy",
        "wgpu",
        "winit",
        "vesta-studio",
        "vesta-world-builder-ui",
        "vesta-world-builder-host",
        "vesta-graph",
        "vesta-scene",
        "vesta-executor",
        "developer-agent-client",
        "developer-agent-engine",
        "git2",
        "ureq",
        "reqwest",
    ] {
        assert!(
            !content.contains(&format!("{forbidden} =")),
            "widgets must not depend on {forbidden}"
        );
    }
}
