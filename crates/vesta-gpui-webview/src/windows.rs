use super::*;
use std::borrow::Cow;
use wry::{NewWindowResponse, PageLoadEvent, PermissionResponse, WebViewBuilder};
mod resources;
use resources::{Resources, Source};

pub(super) struct NativeView {
    view: wry::WebView,
    resources: Resources,
}
impl Drop for NativeView {
    fn drop(&mut self) {
        self.resources.close();
    }
}
fn response(status: u16, mime: &str, bytes: Vec<u8>) -> wry::http::Response<Cow<'static, [u8]>> {
    let mut response = wry::http::Response::new(Cow::Owned(bytes));
    *response.status_mut() = wry::http::StatusCode::from_u16(status).expect("fixed status");
    response.headers_mut().insert(
        "content-type",
        wry::http::HeaderValue::from_str(mime).expect("fixed MIME"),
    );
    response.headers_mut().insert(
        "x-content-type-options",
        wry::http::HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        "cache-control",
        wry::http::HeaderValue::from_static("no-store"),
    );
    response
}
fn bounds(rect: Rect) -> wry::Rect {
    wry::Rect {
        position: wry::dpi::LogicalPosition::new(f64::from(rect.x), f64::from(rect.y)).into(),
        size: wry::dpi::LogicalSize::new(f64::from(rect.width), f64::from(rect.height)).into(),
    }
}
fn browser_error(error: wry::Error) -> WebError {
    WebError::new("web_native_unavailable", error.to_string())
}
fn permitted_navigation(url: &str, has_assets: bool) -> bool {
    url == "about:blank"
        || validate_url(url).is_ok()
        || has_assets
            && url::Url::parse(url).is_ok_and(|url| {
                url.scheme() == "vesta-preview" && url.host_str() == Some("localhost")
                    || url.scheme() == "http" && url.host_str() == Some("vesta-preview.localhost")
            })
}
fn permitted_app_navigation(value: &str, id: &str) -> bool {
    value == "about:blank"
        || url::Url::parse(value).is_ok_and(|url| {
            url.username().is_empty()
                && url.password().is_none()
                && url.port().is_none()
                && (url.scheme() == "vesta-app" && url.host_str() == Some(id)
                    || url.scheme() == "http" && url.host_str() == Some(&format!("vesta-app.{id}")))
        })
}
fn app_response(
    mut reply: wry::http::Response<Cow<'static, [u8]>>,
) -> wry::http::Response<Cow<'static, [u8]>> {
    reply.headers_mut().insert("content-security-policy", wry::http::HeaderValue::from_static(
        "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-src 'none'; worker-src 'none'; object-src 'none'"));
    reply
}

impl NativeView {
    pub(super) fn create(
        parent: &impl HasWindowHandle,
        owner: ViewOwner,
        page: Page,
        events: EventSink,
        retirement: &WebRetirement,
    ) -> Result<Self, WebError> {
        let has_assets = matches!(page, Page::Assets { .. } | Page::Html(_));
        let app = if let Page::Application { profile, .. } = &page {
            profile.validate()?;
            Some(profile.clone())
        } else {
            None
        };
        let application = app.is_some();
        let profile = if application {
            None
        } else {
            Some(
                tempfile::Builder::new()
                    .prefix("vesta-webview-")
                    .tempdir()
                    .map_err(|e| WebError::new("web_profile_unavailable", e.to_string()))?,
            )
        };
        let directory = app
            .as_ref()
            .map(|p| p.data_root.clone())
            .or_else(|| profile.as_ref().map(|p| p.path().to_path_buf()));
        let mut context = wry::WebContext::new(directory);
        let app_id = app.as_ref().map(|p| p.id.clone());
        let mut builder = WebViewBuilder::new_with_web_context(&mut context)
            .with_visible(false)
            .with_focused(false)
            .with_incognito(!application)
            .with_bounds(bounds(Rect::new(0.0, 0.0, 1.0, 1.0)))
            .with_navigation_handler(move |url| {
                app_id.as_ref().map_or_else(
                    || permitted_navigation(&url, has_assets),
                    |id| permitted_app_navigation(&url, id),
                )
            })
            .with_permission_handler(|_| PermissionResponse::Deny)
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
            .with_download_started_handler(|_, _| false);
        let title_events = Arc::clone(&events);
        builder = builder.with_document_title_changed_handler(move |title| {
            title_events(WebEvent {
                owner,
                kind: EventKind::Title(title.chars().take(256).collect()),
            });
        });
        let load_events = Arc::clone(&events);
        builder = builder.with_on_page_load_handler(move |event, url| {
            if url.len() <= 8192 {
                load_events(WebEvent {
                    owner,
                    kind: match event {
                        PageLoadEvent::Started => EventKind::Loading(url),
                        PageLoadEvent::Finished => EventKind::Loaded(url),
                    },
                });
            }
        });
        builder = builder.with_ipc_handler(move |request| {
            if request.body().len() <= 32 * 1024 {
                events(WebEvent {
                    owner,
                    kind: EventKind::Message {
                        origin: request.uri().to_string(),
                        body: request.into_body(),
                    },
                });
            }
        });
        let (builder, resources) = match page {
            Page::Html(html) => {
                if html.len() > MAX_RESOURCE_BYTES as usize {
                    return Err(WebError::new(
                        "web_resource_limit",
                        "HTML exceeds resource limit",
                    ));
                }
                // A real local origin also gives the IPC callback a valid URI;
                // WebView2's opaque NavigateToString page has no resource origin.
                let resources =
                    Resources::start(Source::Html(html.into_bytes()), profile, false, retirement)?;
                let requests = resources.clone();
                (
                    builder
                        .with_asynchronous_custom_protocol(
                            "vesta-preview".into(),
                            move |_, request, reply| {
                                requests.request(request, reply);
                            },
                        )
                        .with_url("vesta-preview://localhost/index.html"),
                    resources,
                )
            }
            Page::Url(url) => {
                let url = validate_url(&url)?;
                let resources = Resources::start(Source::Empty, profile, false, retirement)?;
                (builder.with_url(url), resources)
            }
            Page::Assets { root, entry } | Page::Application { root, entry, .. } => {
                let resources =
                    Resources::start(Source::Assets(root), profile, application, retirement)?;
                let requests = resources.clone();
                let url = match &app {
                    Some(profile) => super::app_page_url(&profile.id, &entry)?,
                    None => super::project_page_url(&entry),
                };
                (
                    builder
                        .with_asynchronous_custom_protocol(
                            if application {
                                "vesta-app".into()
                            } else {
                                "vesta-preview".into()
                            },
                            move |_, request, reply| {
                                requests.request(request, reply);
                            },
                        )
                        .with_url(url.as_str()),
                    resources,
                )
            }
        };
        let view = match builder.build_as_child(parent) {
            Ok(view) => view,
            Err(error) => {
                resources.close();
                return Err(browser_error(error));
            }
        };
        Ok(Self { view, resources })
    }
    pub(super) fn visible(&self, visible: bool) -> Result<(), WebError> {
        self.view.set_visible(visible).map_err(browser_error)
    }
    pub(super) fn bounds(&self, rect: Rect) -> Result<(), WebError> {
        self.view.set_bounds(bounds(rect)).map_err(browser_error)
    }
    pub(super) fn load(&self, url: &str) -> Result<(), WebError> {
        self.view.load_url(url).map_err(browser_error)
    }
    pub(super) fn reload(&self) -> Result<(), WebError> {
        self.view.reload().map_err(browser_error)
    }
    pub(super) fn evaluate(&self, script: &str) -> Result<(), WebError> {
        self.view.evaluate_script(script).map_err(browser_error)
    }
    pub(super) fn focus_parent(&self) -> Result<(), WebError> {
        self.view.focus_parent().map_err(browser_error)
    }
}

#[cfg(test)]
mod app_tests {
    use super::*;
    #[test]
    fn failed_native_creation_retires_its_resource_owner() {
        struct Unavailable;
        impl HasWindowHandle for Unavailable {
            fn window_handle(
                &self,
            ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError>
            {
                Err(raw_window_handle::HandleError::Unavailable)
            }
        }
        let retirement = WebRetirement::default();
        let result = WebView::create(
            &Unavailable,
            ViewOwner {
                view: 1,
                generation: 1,
            },
            Page::Html("<!doctype html>".into()),
            Arc::new(|_| {}),
            &retirement,
        );
        assert_eq!(
            result.err().expect("parent is unavailable").code,
            "web_native_unavailable"
        );
        retirement
            .observe(std::time::Duration::from_secs(2))
            .expect("failed create does not strand resource ownership");
    }
    #[test]
    fn installed_app_origin_stays_private_and_blocks_foreign_navigation() {
        for allowed in [
            "vesta-app://vesta.todo/index.html",
            "http://vesta-app.vesta.todo/index.html",
            "about:blank",
        ] {
            assert!(permitted_app_navigation(allowed, "vesta.todo"));
        }
        for denied in [
            "https://example.com",
            "http://localhost:8000",
            "vesta-app://other.app/index.html",
            "http://vesta-app.vesta.todo.evil/index.html",
            "vesta-app://user@vesta.todo/index.html",
            "file:///C:/private",
            "javascript:alert(1)",
        ] {
            assert!(!permitted_app_navigation(denied, "vesta.todo"));
        }
        let url =
            super::super::app_page_url("vesta.todo", "package%20开发/index.html").expect("URL");
        assert!(url.path().contains("%2520"));
        let reply = app_response(response(200, "text/html", Vec::new()));
        assert!(
            reply.headers()["content-security-policy"]
                .to_str()
                .expect("CSP")
                .contains("connect-src 'self'")
        );
        assert!(
            !reply.headers()["content-security-policy"]
                .to_str()
                .expect("CSP")
                .contains("script-src 'unsafe-inline'")
        );
    }
}
