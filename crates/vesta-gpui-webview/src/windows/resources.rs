//! The existing per-view I/O worker also owns asynchronous profile retirement.
use super::{app_response, response};
use crate::{AssetRoot, WebError, WebRetirement};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

pub(super) enum Source {
    Html(Vec<u8>),
    Assets(AssetRoot),
    Empty,
}
struct Job {
    path: String,
    method: String,
    reply: wry::RequestAsyncResponder,
}

#[derive(Clone)]
pub(super) struct Resources {
    sender: mpsc::SyncSender<Option<Job>>,
    closed: Arc<AtomicBool>,
}
impl Resources {
    pub(super) fn start(
        source: Source,
        profile: Option<tempfile::TempDir>,
        application: bool,
        retirement: &WebRetirement,
    ) -> Result<Self, WebError> {
        let (sender, receiver) = mpsc::sync_channel::<Option<Job>>(32);
        let closed = Arc::new(AtomicBool::new(false));
        let worker_closed = closed.clone();
        let mut lease = retirement.lease();
        std::thread::Builder::new()
            .name("gpui-web-resources".into())
            .spawn(move || {
                lease.enter();
                while let Ok(Some(job)) = receiver.recv() {
                    if worker_closed.load(Ordering::Acquire) {
                        break;
                    }
                    let reply = if !matches!(job.method.as_str(), "GET" | "HEAD") {
                        response(405, "text/plain", Vec::new())
                    } else {
                        match &source {
                            Source::Html(bytes) if job.path == "/index.html" => response(
                                200,
                                "text/html; charset=utf-8",
                                if job.method == "HEAD" {
                                    Vec::new()
                                } else {
                                    bytes.clone()
                                },
                            ),
                            Source::Assets(root) => match root.read(&job.path) {
                                Ok(resource) => response(
                                    200,
                                    resource.content_type,
                                    if job.method == "HEAD" {
                                        Vec::new()
                                    } else {
                                        resource.bytes
                                    },
                                ),
                                Err(_) => {
                                    response(404, "text/plain", b"Resource unavailable".to_vec())
                                }
                            },
                            _ => response(404, "text/plain", Vec::new()),
                        }
                    };
                    if worker_closed.load(Ordering::Acquire) {
                        break;
                    }
                    job.reply.respond(if application {
                        app_response(reply)
                    } else {
                        reply
                    });
                }
                // No new reads after close; queued responders/capabilities release here.
                drop(receiver);
                drop(source);
                lease.finish(crate::retirement::retire_profile(profile));
            })
            .map_err(|e| WebError::new("web_resource_worker_unavailable", e.to_string()))?;
        Ok(Self { sender, closed })
    }
    pub(super) fn request(
        &self,
        request: wry::http::Request<Vec<u8>>,
        reply: wry::RequestAsyncResponder,
    ) {
        if self.closed.load(Ordering::Acquire) {
            reply.respond(response(503, "text/plain", Vec::new()));
            return;
        }
        let job = Job {
            path: request.uri().path().to_owned(),
            method: request.method().to_string(),
            reply,
        };
        if let Err(
            mpsc::TrySendError::Full(Some(job)) | mpsc::TrySendError::Disconnected(Some(job)),
        ) = self.sender.try_send(Some(job))
        {
            job.reply.respond(response(
                503,
                "text/plain",
                b"Resource queue unavailable".to_vec(),
            ));
        }
    }
    pub(super) fn close(&self) {
        self.closed.store(true, Ordering::Release);
        // If full, a queued job already wakes the receiver; no reserved slot,
        // UI wait or periodic idle timer is needed to deliver this close.
        let _ = self.sender.try_send(None);
    }
}
