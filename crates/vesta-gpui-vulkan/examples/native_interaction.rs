//! Editable text, system clipboard, typed actions, focus, scroll and host images.
#![forbid(unsafe_code)]
#[path = "native/mod.rs"]
mod native;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    native::run(native::Mode::Interaction)
}
