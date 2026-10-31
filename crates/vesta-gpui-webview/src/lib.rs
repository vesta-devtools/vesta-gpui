//! Native browser ownership; page permissions and product actions stay with the host.
mod assets;
mod retirement;
pub use assets::{AssetRoot, MAX_RESOURCE_BYTES, Resource};
pub use retirement::WebRetirement;

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

// Native effects are sequenced here; the native adapter supplies only these two
// operations. The cache remains owned by the same WebView, not a second model.
fn synchronize(
    visible: &mut bool,
    previous_bounds: &mut Option<Rect>,
    bounds: Option<Rect>,
    show: bool,
    mut set_bounds: impl FnMut(Rect) -> Result<(), WebError>,
    mut set_visible: impl FnMut(bool) -> Result<(), WebError>,
) -> Result<(), WebError> {
    let bounds = bounds.filter(|r| {
        r.width > 0.0
            && r.height > 0.0
            && [r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite())
    });
    let show = show && bounds.is_some();
    if *visible && !show {
        set_visible(false)?;
        *visible = false;
    }
    if let Some(rect) = bounds.filter(|r| Some(*r) != *previous_bounds) {
        set_bounds(rect)?;
        *previous_bounds = Some(rect);
    }
    if !*visible && show {
        set_visible(true)?;
        *visible = true;
    }
    *previous_bounds = bounds;
    *visible = show;
    Ok(())
}

pub struct WebView {
    visible: bool,
    bounds: Option<Rect>,
    application: bool,
    #[cfg(target_os = "windows")]
    native: windows::NativeView,
}
impl WebView {
    /// Create synchronously on the native event thread. Dispose this view before
    /// its parent window. Retain the Host's retirement group through creation
    /// errors too; after disposing all children, observe it off the live UI loop.
    pub fn create(
        parent: &impl HasWindowHandle,
        owner: ViewOwner,
        page: Page,
        events: EventSink,
        retirement: &WebRetirement,
    ) -> Result<Self, WebError> {
        #[cfg(target_os = "windows")]
        return Ok(Self {
            visible: false,
            bounds: None,
            application: matches!(page, Page::Application { .. }),
            native: windows::NativeView::create(parent, owner, page, events, retirement)?,
        });
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, owner, page, events, retirement);
            Err(WebError::new(
                "web_platform_unavailable",
                "The native web carrier currently requires Windows WebView2",
            ))
        }
    }
    /// Synchronize native geometry/visibility. Each successful operation remains
    /// recorded if a later operation fails; this is not an atomic transaction.
    /// The host decides when GPUI overlays occlude the native child slot.
    pub fn sync(&mut self, bounds: Option<Rect>, show: bool) -> Result<(), WebError> {
        #[cfg(target_os = "windows")]
        {
            let native = &self.native;
            synchronize(
                &mut self.visible,
                &mut self.bounds,
                bounds,
                show,
                |rect| native.bounds(rect),
                |show| native.visible(show),
            )
        }
        #[cfg(not(target_os = "windows"))]
        {
            synchronize(
                &mut self.visible,
                &mut self.bounds,
                bounds,
                show,
                |_| Ok(()),
                |_| Ok(()),
            )
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn synchronization_retains_completed_effects_across_native_failures() {
        let old = Rect::new(0.0, 0.0, 320.0, 240.0);
        let next = Rect::new(12.0, 18.0, 400.0, 300.0);
        let mut visible = true;
        let mut bounds = Some(old);
        let native_visible = Cell::new(true);
        let native_bounds = Cell::new(old);
        let error = synchronize(
            &mut visible,
            &mut bounds,
            Some(next),
            false,
            |_| {
                Err(WebError::new(
                    "fixture_bounds",
                    "controlled native move failure",
                ))
            },
            |value| {
                native_visible.set(value);
                Ok(())
            },
        )
        .expect_err("move failed after hide");
        assert_eq!(error.code, "fixture_bounds");
        assert_eq!(
            visible,
            native_visible.get(),
            "completed hide must remain observable"
        );
        assert_eq!(
            bounds,
            Some(old),
            "failed move preserves its prior observation"
        );
        synchronize(
            &mut visible,
            &mut bounds,
            Some(next),
            true,
            |rect| {
                native_bounds.set(rect);
                Ok(())
            },
            |value| {
                native_visible.set(value);
                Ok(())
            },
        )
        .expect("retry reveals the hidden child");
        assert!(visible && native_visible.get());
        assert_eq!(bounds, Some(native_bounds.get()));

        synchronize(
            &mut visible,
            &mut bounds,
            None,
            false,
            |_| Ok(()),
            |value| {
                native_visible.set(value);
                Ok(())
            },
        )
        .expect("hide before second case");
        let error = synchronize(
            &mut visible,
            &mut bounds,
            Some(next),
            true,
            |rect| {
                native_bounds.set(rect);
                Ok(())
            },
            |_| {
                Err(WebError::new(
                    "fixture_show",
                    "controlled native reveal failure",
                ))
            },
        )
        .expect_err("show failed after move");
        assert_eq!(error.code, "fixture_show");
        assert!(!visible && !native_visible.get());
        assert_eq!(
            bounds,
            Some(native_bounds.get()),
            "completed move must remain observable"
        );
        synchronize(
            &mut visible,
            &mut bounds,
            Some(next),
            true,
            |_| panic!("accepted bounds must not be resent on reveal retry"),
            |value| {
                native_visible.set(value);
                Ok(())
            },
        )
        .expect("retry only reveal");
        assert!(visible && native_visible.get());
    }
}
