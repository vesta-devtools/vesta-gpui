//! Native GPUI showcase; the bounded display preview is scoped by ADR-0522.
#![forbid(unsafe_code)]

mod app;
mod model;
mod preview;
mod study;
mod view;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    app::run()
}
