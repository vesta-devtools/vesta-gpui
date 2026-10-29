//! Small native consumer of the public SDK. See ../README.md for the host ports.
#![forbid(unsafe_code)]
#[path = "native/mod.rs"]
mod native;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    native::run(native::Mode::Starter)
}
