//! Explicit opt-in native pixel/draw-call probe; the helper window stays hidden.
use super::*;
use std::{path::Path, time::Duration};
use vesta_gpui_vulkan::{CaptureRegion, WindowRenderer};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

struct Probe<'a> {
    fonts: &'a [Arc<Vec<u8>>],
    prefix: &'a Path,
    result: Option<Result<()>>,
}

impl Probe<'_> {
    fn capture(&self, event_loop: &ActiveEventLoop) -> Result<()> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Table pipeline probe")
                    .with_visible(false)
                    .with_inner_size(PhysicalSize::new(2306, 1026)),
            )?,
        );
        let mut renderer = WindowRenderer::new(event_loop, window.clone())?;
        let retirement = renderer.gpu_context().retirement_monitor();
        renderer.enable_cpu_profiling(true);
        let mut context = TextInputContext::default();
        for font in self.fonts {
            assert!(context.register_font(font.clone()));
        }
        let mut rasterizer = TextRasterizer::default();
        let mut engine = RetainedLayoutEngine::default();
        let mut records = Vec::new();
        let theme = WidgetTheme::dark();
        let options = RenderOptions {
            clear: [0.0, 0.0, 0.0, 1.0],
            selection: theme.palette.selection,
            caret: theme.palette.focus,
            caret_visible: false,
        };
        for columns in [8, 24] {
            for scale in [1.0, 1.5, 1.75] {
                let size = window.inner_size();
                let viewport =
                    Viewport::new(size.width as f32 / scale, size.height as f32 / scale, scale);
                let widget = tree(columns, 2)?;
                let layout =
                    engine.compute_with_text_context(&widget.element, viewport, &mut context)?;
                let paint = PaintList::build_with_focus(
                    &widget.element,
                    &layout,
                    Some(identity(1202).element),
                    theme.palette.focus,
                );
                for frame in 0..3 {
                    let ticket = renderer.capture_next_frame(CaptureRegion {
                        origin: [0, 0],
                        extent: [size.width, size.height],
                    })?;
                    let deadline = Instant::now() + Duration::from_secs(20);
                    let output = loop {
                        if let Some(output) = renderer.render(
                            viewport,
                            &paint,
                            &mut context,
                            &mut rasterizer,
                            None,
                            options,
                            true,
                        )? {
                            break output;
                        }
                        if Instant::now() >= deadline {
                            return Err("native table render deadline".into());
                        }
                        std::thread::sleep(Duration::from_millis(2));
                    };
                    let readback = ticket.wait(Duration::from_secs(10))?;
                    if frame == 2 {
                        let path = self
                            .prefix
                            .with_extension(format!("{columns}-{scale}.rgba"));
                        let rgba = readback.rgba()?;
                        let mut file = fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&path)?;
                        std::io::Write::write_all(&mut file, &rgba)?;
                        let profile = renderer.cpu_profile().expect("successful renderer profile");
                        records.push(json!({
                            "columns":columns,"scale":scale,"extent":[size.width,size.height],"rgba":path,
                            "draw_calls":output.draw_calls,"scissor_updates":profile.scissor_updates,
                            "prepared_entries":profile.prepared_draws,"pipeline_binds":profile.pipeline_binds,
                        }));
                    }
                }
            }
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.prefix)?;
        std::io::Write::write_all(
            &mut file,
            serde_json::to_string_pretty(&records)?.as_bytes(),
        )?;
        drop(renderer);
        drop(window);
        retirement.observe(Duration::from_secs(20))?;
        Ok(())
    }
}

impl ApplicationHandler for Probe<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.result.is_none() {
            self.result = Some(self.capture(event_loop));
        }
        event_loop.exit();
    }
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
}

pub(super) fn run(fonts: &[Arc<Vec<u8>>], prefix: &Path) -> Result<()> {
    let mut probe = Probe {
        fonts,
        prefix,
        result: None,
    };
    EventLoop::new()?.run_app(&mut probe)?;
    probe.result.ok_or("native probe did not start")?
}
