//! Finite public carrier fixture. No renderer, network page, account or OS input.
use std::{
    error::Error,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use vesta_gpui::Rect;
use vesta_gpui_webview::{
    AppProfile, AssetRoot, EventKind, Page, ViewOwner, WebEvent, WebRetirement, WebView,
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

const HTML: &str = "<!doctype html><meta charset=utf-8><title>Carrier fixture</title><script>window.ipc.postMessage(JSON.stringify({phase:'html'}))</script>";
const ASSETS: &str = "<!doctype html><meta charset=utf-8><script>Promise.all([fetch('data.json').then(r=>r.json()),fetch('data.json',{method:'HEAD'}).then(r=>r.text()),fetch('data.json',{method:'POST'}).then(r=>r.status)]).then(([data,head,post])=>window.ipc.postMessage(JSON.stringify({phase:'assets',value:data.value,head:head.length,post})))</script>";
const SCRIPT: &str = "const old=localStorage.getItem('fixture');if(old===null)localStorage.setItem('fixture','retained');fetch('data.json').then(r=>r.json()).then(data=>window.ipc.postMessage(JSON.stringify({phase:'application',storage:old===null?'fresh':old,value:data.value})))";

struct Fixture {
    view: Option<WebView>, // Disposed before the parent window, including error paths.
    window: Option<Window>,
    retirement: WebRetirement,
    proxy: EventLoopProxy<WebEvent>,
    assets: AssetRoot,
    profile: PathBuf,
    stage: u64,
    asset_loads: usize,
    advance: Option<Instant>,
    started: Instant,
    error: Option<Box<dyn Error>>,
}
impl Fixture {
    fn owner(&self) -> ViewOwner {
        ViewOwner {
            view: 1,
            generation: self.stage + 1,
        }
    }
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl Into<Box<dyn Error>>) {
        self.error = Some(error.into());
        event_loop.exit();
    }
    fn create(&mut self) -> Result<()> {
        let page = match self.stage {
            0 => Page::Html(HTML.into()),
            1 => Page::Assets {
                root: self.assets.clone(),
                entry: "preview%20开发.html".into(),
            },
            2 | 3 => Page::Application {
                root: self.assets.clone(),
                entry: "application.html".into(),
                profile: AppProfile {
                    id: "fixture.lifecycle".into(),
                    data_root: self.profile.clone(),
                },
            },
            _ => return Err("web_fixture_stage_invalid".into()),
        };
        let proxy = self.proxy.clone();
        let mut view = WebView::create(
            self.window.as_ref().ok_or("web_fixture_parent_missing")?,
            self.owner(),
            page,
            Arc::new(move |event| {
                let _ = proxy.send_event(event);
            }),
            &self.retirement,
        )?;
        view.sync(Some(Rect::new(12.0, 18.0, 400.0, 280.0)), true)?;
        if !view.visible() {
            return Err("web_fixture_reveal_missing".into());
        }
        view.sync(Some(Rect::new(24.0, 22.0, 420.0, 290.0)), false)?;
        if view.visible() {
            return Err("web_fixture_hide_missing".into());
        }
        view.sync(Some(Rect::new(24.0, 22.0, 420.0, 290.0)), true)?;
        if self.stage == 0
            && (view.navigate("file:///C:/not-an-admitted-page").is_ok()
                || view.evaluate(&"x".repeat(65_537)).is_ok())
        {
            return Err("web_fixture_public_limits_not_enforced".into());
        }
        self.view = Some(view);
        println!(
            "web_created stage={} generation={}",
            self.stage,
            self.owner().generation
        );
        Ok(())
    }
    fn message(&mut self, event: WebEvent) -> Result<()> {
        if event.owner != self.owner() || self.view.is_none() || self.advance.is_some() {
            return Ok(());
        }
        let EventKind::Message { origin, body } = event.kind else {
            return Ok(());
        };
        let expected = if self.stage < 2 {
            [
                "http://vesta-preview.localhost/",
                "vesta-preview://localhost/",
            ]
        } else {
            [
                "http://vesta-app.fixture.lifecycle/",
                "vesta-app://fixture.lifecycle/",
            ]
        };
        if !expected.iter().any(|prefix| origin.starts_with(prefix)) {
            return Err("web_fixture_origin_mismatch".into());
        }
        let value: serde_json::Value = serde_json::from_str(&body)?;
        match self.stage {
            0 if value["phase"] == "html" => (),
            1 if value["phase"] == "assets"
                && value["value"] == 42
                && value["head"] == 0
                && value["post"] == 405 =>
            {
                self.asset_loads += 1;
                if self.asset_loads == 1 {
                    self.view
                        .as_ref()
                        .ok_or("web_fixture_view_missing")?
                        .reload()?;
                    return Ok(());
                }
            }
            2 if value["phase"] == "application"
                && value["storage"] == "fresh"
                && value["value"] == 42 => {}
            3 if value["phase"] == "application"
                && value["storage"] == "retained"
                && value["value"] == 42 => {}
            _ => {
                return Err(format!(
                    "web_fixture_payload_mismatch stage={} value={value}",
                    self.stage
                )
                .into());
            }
        }
        println!(
            "web_verified stage={} generation={} origin={} reloads={}",
            self.stage, event.owner.generation, origin, self.asset_loads
        );
        // Keep the owned view observable briefly for process/profile evidence.
        self.advance = Some(Instant::now() + Duration::from_millis(700));
        Ok(())
    }
}
impl ApplicationHandler<WebEvent> for Fixture {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<()> {
            self.window = Some(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("Vesta WebView lifecycle fixture")
                        .with_visible(false)
                        .with_inner_size(LogicalSize::new(640.0, 420.0)),
                )?,
            );
            self.create()
        })();
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: WebEvent) {
        if event_loop.exiting() {
            return;
        }
        if let Err(error) = self.message(event) {
            self.fail(event_loop, error);
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::CloseRequested) {
            event_loop.exit();
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if event_loop.exiting() {
            return;
        }
        let now = Instant::now();
        if now.duration_since(self.started) >= Duration::from_secs(60) {
            self.fail(event_loop, "web_fixture_timeout");
            return;
        }
        if self.advance.is_some_and(|at| now >= at) {
            self.advance = None;
            self.view.take();
            println!("web_disposed stage={}", self.stage);
            self.stage += 1;
            if self.stage == 4 {
                event_loop.exit();
                return;
            }
        }
        if self.view.is_none()
            && !self.retirement.pending()
            && let Err(error) = self.create()
        {
            self.fail(event_loop, error);
            return;
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_millis(25)));
    }
    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.view.take();
        self.window.take();
    }
}
fn main() -> Result<()> {
    let argument = std::env::args()
        .find_map(|a| a.strip_prefix("--fixture-dir=").map(PathBuf::from))
        .ok_or("Usage: webview_lifecycle --fixture-dir=<new-directory>")?;
    let directory = std::env::current_dir()?.join(argument);
    std::fs::create_dir(&directory)?; // Never overwrite an existing fixture/profile.
    let directory = directory.canonicalize()?;
    let assets = directory.join("assets");
    std::fs::create_dir(&assets)?;
    std::fs::write(assets.join("preview%20开发.html"), ASSETS)?;
    std::fs::write(
        assets.join("application.html"),
        "<!doctype html><meta charset=utf-8><script src=app.js></script>",
    )?;
    std::fs::write(assets.join("app.js"), SCRIPT)?;
    std::fs::write(assets.join("data.json"), "{\"value\":42}")?;
    let profile = directory.join("profile");
    println!("web_fixture_directory={}", directory.display());
    let event_loop = EventLoop::<WebEvent>::with_user_event().build()?;
    let mut fixture = Fixture {
        view: None,
        window: None,
        retirement: WebRetirement::default(),
        proxy: event_loop.create_proxy(),
        assets: AssetRoot::new(&assets)?,
        profile,
        stage: 0,
        asset_loads: 0,
        advance: None,
        started: Instant::now(),
        error: None,
    };
    let result = event_loop.run_app(&mut fixture);
    fixture.view.take();
    fixture.window.take();
    let retired = fixture.retirement.observe(Duration::from_secs(15));
    if let Err(error) = retired {
        eprintln!("web_fixture_retirement_failed: {error}");
        return Err(error.into());
    }
    println!("web_retirement_complete pending=false");
    result?;
    if let Some(error) = fixture.error {
        return Err(error);
    }
    if fixture.stage != 4 || fixture.asset_loads != 2 {
        return Err("web_fixture_incomplete".into());
    }
    println!("web_fixture_complete stages=4 reloads=2 parent_order=true");
    Ok(())
}
