//! Native browser ownership; page permissions and product actions stay with the host.
mod assets;
pub use assets::{AssetRoot, MAX_RESOURCE_BYTES, Resource};

#[cfg(any(windows, test))]
pub(crate) fn project_page_url(entry: &str) -> url::Url {
    let mut url = url::Url::parse("vesta-preview://localhost/").expect("fixed asset origin");
    // set_path escapes Unicode/URL delimiters but preserves existing % escapes.
    // Picker paths name literal files, so their percent signs must be escaped first.
    url.set_path(&entry.replace('%', "%25"));
    url
}
#[cfg(target_os = "windows")]
mod windows;

use raw_window_handle::HasWindowHandle;
use std::{fmt, sync::Arc};
use vesta_gpui::Rect;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebError {
    pub code: &'static str,
    pub message: String,
}
impl WebError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl fmt::Display for WebError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for WebError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewOwner {
    pub view: u64,
    pub generation: u64,
}
#[derive(Clone, Debug)]
pub enum Page {
    Html(String),
    Url(String),
    Assets {
        root: AssetRoot,
        entry: String,
    },
    /// A Host-approved installed app has a private durable browser profile.
    Application {
        root: AssetRoot,
        entry: String,
        profile: AppProfile,
    },
}

#[derive(Clone, Debug)]
pub struct AppProfile {
    pub id: String,
    pub data_root: std::path::PathBuf,
}
impl AppProfile {
    pub fn validate(&self) -> Result<(), WebError> {
        if self.id.is_empty()
            || self.id.len() > 64
            || self.id.split('.').any(|part| {
                part.is_empty()
                    || !part
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            })
            || !self.data_root.is_absolute()
            || self.data_root.components().any(|part| {
                matches!(
                    part,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
        {
            return Err(WebError::new(
                "web_app_profile_invalid",
                "Application profile needs a stable identity and a private absolute directory",
            ));
        }
        Ok(())
    }
}

#[cfg(any(windows, test))]
fn app_page_url(id: &str, entry: &str) -> Result<url::Url, WebError> {
    let mut url = url::Url::parse(&format!("vesta-app://{id}/"))
        .map_err(|e| WebError::new("web_app_profile_invalid", e.to_string()))?;
    url.set_path(&entry.replace('%', "%25"));
    Ok(url)
}
#[derive(Clone, Debug)]
pub enum EventKind {
    Loading(String),
    Loaded(String),
    Title(String),
    Message { origin: String, body: String },
}
#[derive(Clone, Debug)]
pub struct WebEvent {
    pub owner: ViewOwner,
    pub kind: EventKind,
}
pub type EventSink = Arc<dyn Fn(WebEvent) + Send + Sync>;

/// HTTPS pages and explicit loopback development servers; never arbitrary file URLs.
pub fn validate_url(value: &str) -> Result<String, WebError> {
    if value.len() > 8192 {
        return Err(WebError::new("web_url_invalid", "URL exceeds 8 KiB"));
    }
    let parsed = url::Url::parse(value.trim())
        .map_err(|e| WebError::new("web_url_invalid", e.to_string()))?;
    let loopback = parsed
        .host_str()
        .is_some_and(|host| host == "localhost" || host == "127.0.0.1" || host == "[::1]");
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || !(parsed.scheme() == "https" && parsed.host_str().is_some()
            || parsed.scheme() == "http" && loopback)
    {
        return Err(WebError::new(
            "web_url_disallowed",
            "Use HTTPS or a loopback development server",
        ));
    }
    Ok(parsed.to_string())
}

pub struct WebView {
    pub owner: ViewOwner,
    visible: bool,
    bounds: Option<Rect>,
    application: bool,
    #[cfg(target_os = "windows")]
    native: windows::NativeView,
}
impl WebView {
    /// The caller must dispose this view before its parent native window.
    pub fn create(
        parent: &impl HasWindowHandle,
        owner: ViewOwner,
        page: Page,
        events: EventSink,
    ) -> Result<Self, WebError> {
        #[cfg(target_os = "windows")]
        return Ok(Self {
            owner,
            visible: false,
            bounds: None,
            application: matches!(page, Page::Application { .. }),
            native: windows::NativeView::create(parent, owner, page, events)?,
        });
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, owner, page, events);
            Err(WebError::new(
                "web_platform_unavailable",
                "The native web carrier currently requires Windows WebView2",
            ))
        }
    }
    /// Hide before moving/revealing; the host decides when GPUI overlays occlude the slot.
    pub fn sync(&mut self, bounds: Option<Rect>, show: bool) -> Result<(), WebError> {
        let bounds = bounds.filter(|r| {
            r.width > 0.0
                && r.height > 0.0
                && [r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite())
        });
        let show = show && bounds.is_some();
        #[cfg(target_os = "windows")]
        {
            if self.visible && !show {
                self.native.visible(false)?;
            }
            if let Some(rect) = bounds.filter(|r| Some(*r) != self.bounds) {
                self.native.bounds(rect)?;
            }
            if !self.visible && show {
                self.native.visible(true)?;
            }
        }
        self.bounds = bounds;
        self.visible = show;
        Ok(())
    }
    pub fn visible(&self) -> bool {
        self.visible
    }
    pub fn navigate(&self, url: &str) -> Result<(), WebError> {
        if self.application {
            return Err(WebError::new(
                "web_app_navigation_disallowed",
                "Installed applications stay within their package origin",
            ));
        }
        let url = validate_url(url)?;
        #[cfg(target_os = "windows")]
        return self.native.load(&url);
        #[cfg(not(target_os = "windows"))]
        {
            let _ = url;
            Err(WebError::new(
                "web_platform_unavailable",
                "No native browser on this platform",
            ))
        }
    }
    pub fn reload(&self) -> Result<(), WebError> {
        #[cfg(target_os = "windows")]
        return self.native.reload();
        #[cfg(not(target_os = "windows"))]
        Err(WebError::new(
            "web_platform_unavailable",
            "No native browser on this platform",
        ))
    }
    pub fn evaluate(&self, script: &str) -> Result<(), WebError> {
        if script.len() > 64 * 1024 {
            return Err(WebError::new(
                "web_script_limit",
                "Host script exceeds 64 KiB",
            ));
        }
        #[cfg(target_os = "windows")]
        return self.native.evaluate(script);
        #[cfg(not(target_os = "windows"))]
        Err(WebError::new(
            "web_platform_unavailable",
            "No native browser on this platform",
        ))
    }
    pub fn focus_parent(&self) -> Result<(), WebError> {
        #[cfg(target_os = "windows")]
        return self.native.focus_parent();
        #[cfg(not(target_os = "windows"))]
        Ok(())
    }
}
