//! Embedded, offline Perfetto frontend. The existing query/native engine remains available.
mod bundle;
mod server;

use crate::session::TraceError;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use vesta_gpui::Rect;
use vesta_gpui_webview::{EventKind, Page, ViewOwner, WebEvent, WebRetirement, WebView};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

const OWNER: ViewOwner = ViewOwner {
    view: 1,
    generation: 1,
};

pub fn run(
    input: Option<PathBuf>,
    directory: Option<PathBuf>,
    smoke: bool,
    capture_smoke: bool,
    target: Option<PathBuf>,
    target_arguments: Vec<String>,
) -> Result<(), TraceError> {
    if !cfg!(target_os = "windows") {
        return Err(TraceError::new(
            "trace_perfetto_platform",
            "Embedded Perfetto currently requires Windows WebView2; use --native on other platforms",
        ));
    }
    let directory = match directory {
        Some(path) => path,
        None => std::env::current_exe()
            .map_err(|e| TraceError::caused("trace_perfetto_executable", e))?
            .parent()
            .ok_or_else(|| TraceError::new("trace_perfetto_executable", "Missing tools directory"))?
            .join("perfetto-ui"),
    };
    let bundle = bundle::Bundle::open(&directory)?;
    let server = server::Server::start(bundle, input, target, target_arguments)?;
    let event_loop = EventLoop::<WebEvent>::with_user_event()
        .build()
        .map_err(|e| TraceError::caused("trace_perfetto_event_loop", e))?;
    let proxy = event_loop.create_proxy();
    let mut host = Host {
        view: None,
        window: None,
        retirement: WebRetirement::default(),
        server,
        sink: Arc::new(move |event| {
            let _ = proxy.send_event(event);
        }),
        smoke: smoke || capture_smoke,
        capture_smoke,
        started: Instant::now(),
        probe_sent: false,
        accepted: false,
        error: None,
    };
    println!(
        "{}",
        serde_json::json!({"status":"perfetto_started","version":bundle::VERSION,"url":host.server.url})
    );
    let result = event_loop.run_app(&mut host);
    host.view.take();
    host.window.take();
    let server_result = host.server.shutdown();
    let retirement = host
        .retirement
        .observe(Duration::from_secs(15))
        .map_err(|e| TraceError::caused("trace_perfetto_retirement", e));
    result.map_err(|e| TraceError::caused("trace_perfetto_event_loop", e))?;
    server_result?;
    retirement?;
    if let Some(error) = host.error {
        return Err(error);
    }
    if (smoke || capture_smoke) && !host.accepted {
        return Err(TraceError::new(
            "trace_perfetto_smoke",
            "Closed before native acceptance completed",
        ));
    }
    println!(
        "{}",
        serde_json::json!({"status":"perfetto_retired","accepted":host.accepted})
    );
    Ok(())
}

struct Host {
    view: Option<WebView>, // Child is always disposed before its native parent.
    window: Option<Window>,
    retirement: WebRetirement,
    server: server::Server,
    sink: vesta_gpui_webview::EventSink,
    smoke: bool,
    capture_smoke: bool,
    started: Instant,
    probe_sent: bool,
    accepted: bool,
    error: Option<TraceError>,
}
impl Host {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: TraceError) {
        self.error = Some(error);
        event_loop.exit();
    }
    fn sync(&mut self) -> Result<(), TraceError> {
        if let (Some(window), Some(view)) = (&self.window, &mut self.view) {
            let size = window.inner_size().to_logical::<f64>(window.scale_factor());
            view.sync(
                Some(Rect::new(0.0, 0.0, size.width as f32, size.height as f32)),
                true,
            )
            .map_err(|e| TraceError::caused("trace_perfetto_bounds", e))?;
        }
        Ok(())
    }
}
impl ApplicationHandler<WebEvent> for Host {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| {
            let window = event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Vesta Trace · Perfetto")
                        .with_inner_size(LogicalSize::new(1280.0, 840.0))
                        .with_visible(!self.smoke),
                )
                .map_err(|e| TraceError::caused("trace_perfetto_window", e))?;
            let view = WebView::create(
                &window,
                OWNER,
                Page::Url(self.server.url.clone()),
                Arc::clone(&self.sink),
                &self.retirement,
            )
            .map_err(|e| TraceError::caused("trace_perfetto_webview", e))?;
            self.window = Some(window);
            self.view = Some(view);
            self.sync()
        })();
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: WebEvent) {
        if event.owner != OWNER || event_loop.exiting() {
            return;
        }
        if let EventKind::Message { origin, body } = event.kind {
            // A page cannot use arbitrary messages as host commands.
            if origin != self.server.origin
                && origin != format!("{}/", self.server.origin)
                && origin != format!("{}{}/", self.server.origin, self.server.prefix)
            {
                return;
            }
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&body) else {
                return;
            };
            match value["type"].as_str() {
                Some("ready") if self.capture_smoke && !self.probe_sent => {
                    self.probe_sent = true;
                    let script = format!(
                        "window.vestaLiveProbe = true;\n{}\n{}",
                        include_str!("perfetto/smoke.js"),
                        include_str!("perfetto/capture_smoke.js")
                    );
                    if let Some(view) = &self.view
                        && let Err(error) = view.evaluate(&script)
                    {
                        self.fail(
                            event_loop,
                            TraceError::caused("trace_perfetto_probe", error),
                        );
                    }
                }
                Some("posted") => {
                    println!(
                        "{}",
                        serde_json::json!({"status":"perfetto_posted","evidence":value})
                    );
                    if self.smoke && !self.probe_sent {
                        self.probe_sent = true;
                        if let Some(view) = &self.view
                            && let Err(error) = view.evaluate(include_str!("perfetto/smoke.js"))
                        {
                            self.fail(
                                event_loop,
                                TraceError::caused("trace_perfetto_probe", error),
                            );
                        }
                    }
                }
                Some("accepted") if self.smoke => {
                    self.accepted = true;
                    println!(
                        "{}",
                        serde_json::json!({"status":"perfetto_native_accepted","evidence":value})
                    );
                    event_loop.exit();
                }
                Some("error") if self.smoke => self.fail(
                    event_loop,
                    TraceError::new(
                        "trace_perfetto_smoke",
                        value["message"].as_str().unwrap_or("Analyzer failed"),
                    ),
                ),
                _ => (),
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.window.as_ref().is_none_or(|w| w.id() != id) {
            return;
        }
        let result = match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                Ok(())
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => self.sync(),
            WindowEvent::DroppedFile(path) => self.server.load(path),
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.smoke {
            if self.started.elapsed() > Duration::from_secs(60) {
                self.fail(
                    event_loop,
                    TraceError::new(
                        "trace_perfetto_timeout",
                        "Native Perfetto acceptance exceeded 60 seconds",
                    ),
                );
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(100),
            ));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.view.take();
        self.window.take();
    }
}
