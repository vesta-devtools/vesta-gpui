use super::*;
use std::{borrow::Cow, sync::mpsc};
use wry::{NewWindowResponse, PageLoadEvent, PermissionResponse, WebViewBuilder};

pub(super) struct NativeView {
    view: wry::WebView,
    _profile: Option<tempfile::TempDir>,
}
struct AssetJob {
    path: String,
    method: String,
    reply: wry::RequestAsyncResponder,
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
        builder = match page {
            Page::Html(html) => {
                if html.len() > MAX_RESOURCE_BYTES as usize {
                    return Err(WebError::new(
                        "web_resource_limit",
                        "HTML exceeds resource limit",
                    ));
                }
                // A real local origin also gives the IPC callback a valid URI;
                // WebView2's opaque NavigateToString page has no resource origin.
                let html = Arc::new(html.into_bytes());
                builder
                    .with_asynchronous_custom_protocol(
                        "vesta-preview".into(),
                        move |_, request, reply| {
                            let success = request.uri().path() == "/index.html"
                                && matches!(request.method().as_str(), "GET" | "HEAD");
                            reply.respond(if success {
                                response(
                                    200,
                                    "text/html; charset=utf-8",
                                    if request.method() == "HEAD" {
                                        Vec::new()
                                    } else {
                                        html.as_ref().clone()
                                    },
                                )
                            } else {
                                response(404, "text/plain", Vec::new())
                            });
                        },
                    )
                    .with_url("vesta-preview://localhost/index.html")
            }
            Page::Url(url) => builder.with_url(validate_url(&url)?),
            Page::Assets { root, entry } | Page::Application { root, entry, .. } => {
                // The resource worker alone performs file I/O; its bounded queue
                // closes when the native callback is disposed with the view.
                let (sender, receiver) = mpsc::sync_channel::<AssetJob>(32);
                std::thread::Builder::new()
                    .name("gpui-web-resources".into())
                    .spawn(move || {
                        while let Ok(job) = receiver.recv() {
                            let reply = if job.method != "GET" && job.method != "HEAD" {
                                response(405, "text/plain", Vec::new())
                            } else {
                                match root.read(&job.path) {
                                    Ok(resource) => response(
                                        200,
                                        resource.content_type,
                                        if job.method == "HEAD" {
                                            Vec::new()
                                        } else {
                                            resource.bytes
                                        },
                                    ),
                                    Err(_) => response(
                                        404,
                                        "text/plain",
                                        b"Resource unavailable".to_vec(),
                                    ),
                                }
                            };
                            job.reply.respond(if application {
                                app_response(reply)
                            } else {
                                reply
                            });
                        }
                    })
                    .map_err(|e| WebError::new("web_resource_worker_unavailable", e.to_string()))?;
                let url = match &app {
                    Some(profile) => super::app_page_url(&profile.id, &entry)?,
                    None => super::project_page_url(&entry),
                };
                builder
                    .with_asynchronous_custom_protocol(
                        if application {
                            "vesta-app".into()
                        } else {
                            "vesta-preview".into()
                        },
                        move |_, request, reply| {
                            let job = AssetJob {
                                path: request.uri().path().to_owned(),
                                method: request.method().to_string(),
                                reply,
                            };
                            if let Err(
                                mpsc::TrySendError::Full(job)
                                | mpsc::TrySendError::Disconnected(job),
                            ) = sender.try_send(job)
                            {
                                job.reply.respond(response(
                                    503,
                                    "text/plain",
                                    b"Resource queue unavailable".to_vec(),
                                ));
                            }
                        },
                    )
                    .with_url(url.as_str())
            }
        };
        let view = builder.build_as_child(parent).map_err(browser_error)?;
        Ok(Self {
            view,
            _profile: profile,
        })
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
