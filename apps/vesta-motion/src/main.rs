//! Native GPUI showcase; the bounded display preview is scoped by ADR-0522.
#![forbid(unsafe_code)]

mod app;
mod model;
mod preview;
mod study;
mod view;

#[global_allocator]
static TRACE_ALLOCATOR: vesta_gpui_winit::UiTraceAllocator = vesta_gpui_winit::UiTraceAllocator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let recording = vesta_gpui_winit::UiTraceRecording::from_arguments(std::env::args_os())?;
    recording.finish_after_run(app::run())
}
