//! One application-owned worker: bounded HTTP, capture import and immutable data.
#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
use super::bundle::Bundle;
use crate::capture_target::Target;
use crate::{import, session::TraceError};
use serde_json::{Value, json};
use std::{
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const HTML: &str = include_str!("host.html");
const SCRIPT: &str = include_str!("host.js");
const HEAD_LIMIT: usize = 8192;
const CSP: &str = "default-src 'self'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' blob:; worker-src 'self' blob:; frame-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'";

pub(super) struct Server {
    pub url: String,
    pub origin: String,
    pub prefix: String,
    pending: SyncSender<PathBuf>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Server {
    pub fn start(
        bundle: Bundle,
        input: Option<PathBuf>,
        executable: Option<PathBuf>,
        arguments: Vec<String>,
    ) -> Result<Self, TraceError> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|e| TraceError::caused("trace_perfetto_listen", e))?;
        listener
            .set_nonblocking(true)
            .map_err(|e| TraceError::caused("trace_perfetto_listen", e))?;
        let address = listener
            .local_addr()
            .map_err(|e| TraceError::caused("trace_perfetto_listen", e))?;
        let origin = format!("http://{address}");
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random)
            .map_err(|e| TraceError::new("trace_perfetto_token", e.to_string()))?;
        let token = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let prefix = format!("/vesta/{token}");
        let url = format!("{origin}{prefix}/");
        let (pending, requests) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let executable = executable.unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap_or_default()
                .with_file_name("vesta-motion.exe")
        });
        let mut state = State {
            bundle,
            origin: origin.clone(),
            prefix: prefix.clone(),
            capture: None,
            revision: 0,
            error: Value::Null,
            target: None,
            executable,
            arguments,
        };
        let worker = thread::Builder::new()
            .name("trace-perfetto".into())
            .spawn(move || {
                if let Some(path) = input {
                    state.load_path(path, &worker_stop);
                }
                while !worker_stop.load(Ordering::Relaxed) {
                    if let Ok(path) = requests.try_recv() {
                        state.load_path(path, &worker_stop);
                    }
                    match listener.accept() {
                        Ok((stream, _)) => {
                            // Invalid/disconnected requests never terminate the tools server.
                            let _ = state.serve(stream, &worker_stop);
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10))
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|e| TraceError::caused("trace_perfetto_worker", e))?;
        Ok(Self {
            url,
            origin,
            prefix,
            pending,
            stop,
            worker: Some(worker),
        })
    }
    pub fn load(&self, path: PathBuf) -> Result<(), TraceError> {
        self.pending
            .try_send(path)
            .map_err(|e| TraceError::new("trace_perfetto_busy", e.to_string()))
    }
    pub fn shutdown(&mut self) -> Result<(), TraceError> {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| {
                TraceError::new("trace_perfetto_worker", "Perfetto worker panicked")
            })?;
        }
        Ok(())
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

struct Capture {
    json: Vec<u8>,
    status: Value,
    path: Option<PathBuf>,
}
struct State {
    bundle: Bundle,
    origin: String,
    prefix: String,
    capture: Option<Capture>,
    revision: u64,
    error: Value,
    target: Option<Target>,
    executable: PathBuf,
    arguments: Vec<String>,
}
impl State {
    fn load_path(&mut self, path: PathBuf, cancel: &AtomicBool) {
        let result = (|| {
            let file = File::open(&path).map_err(|e| TraceError::caused("trace_open", e))?;
            let mut bytes = Vec::new();
            file.take(import::MAX_FILE_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| TraceError::caused("trace_read", e))?;
            self.load(
                &bytes,
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Capture"),
                cancel,
            )?;
            if let Some(capture) = &mut self.capture {
                capture.path = Some(path.clone());
                capture.status["path"] = json!(path);
            }
            Ok(())
        })();
        self.record_error(result);
    }
    fn record_error(&mut self, result: Result<(), TraceError>) {
        self.error = match result {
            Ok(()) => Value::Null,
            Err(e) => json!({"code":e.code,"message":e.message}),
        };
    }
    fn load(&mut self, bytes: &[u8], title: &str, cancel: &AtomicBool) -> Result<(), TraceError> {
        let session = import::parse(bytes, cancel)?;
        let text = std::str::from_utf8(bytes).map_err(|e| TraceError::caused("trace_utf8", e))?;
        let json = import::json_text(text)?.as_bytes().to_vec();
        let status = json!({"title":title.chars().take(256).collect::<String>(), "source":session.source,
            "events":session.events.len(), "spans":session.events.iter().filter(|e| e.is_span()).count(),
            "slices":session.events.iter().filter(|e| !matches!(e.kind, crate::session::EventKind::Counter(_))).count(),
            "frames":session.frames.len(), "detail":session.detail,
            "dropped_events":session.dropped_events, "unfinished_spans":session.unfinished_spans,
            "diagnostics":session.diagnostics, "gpu_submissions":session.gpu.as_ref().map_or(0, |g| g.submissions.len())});
        self.capture = Some(Capture {
            json,
            status,
            path: None,
        });
        self.revision += 1;
        Ok(())
    }
    fn status(&self) -> Value {
        json!({"revision":self.revision,"capture":self.capture.as_ref().map(|c| &c.status),"error":self.error,"perfetto":super::bundle::VERSION,
            "target":self.target.as_ref().map(|t| &t.status),"executable":self.executable})
    }
    fn action(
        &mut self,
        action: &str,
        bytes: &[u8],
        cancel: &AtomicBool,
    ) -> Result<(), TraceError> {
        if bytes.len() > 16384 {
            return Err(TraceError::new(
                "trace_target_request",
                "Control request is too large",
            ));
        }
        let data: Value = serde_json::from_slice(bytes)
            .map_err(|e| TraceError::caused("trace_target_request", e))?;
        match action {
            "/launch" | "/connect" => {
                if self.target.is_some() {
                    return Err(TraceError::new(
                        "trace_target_connected",
                        "Disconnect the current application first",
                    ));
                }
                self.target = Some(if action == "/launch" {
                    let path = data["executable"]
                        .as_str()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| self.executable.clone());
                    Target::launch(&path, &self.arguments)?
                } else {
                    Target::connect(PathBuf::from(data["descriptor"].as_str().unwrap_or("")))?
                });
            }
            "/disconnect" => {
                self.target.take();
            }
            "/start" => self
                .target
                .as_mut()
                .ok_or_else(|| {
                    TraceError::new(
                        "trace_target_missing",
                        "Launch or connect to a GPUI application first",
                    )
                })?
                .start(&data)?,
            "/stop" => {
                let data = self
                    .target
                    .as_mut()
                    .ok_or_else(|| {
                        TraceError::new("trace_target_missing", "No connected application")
                    })?
                    .stop()?;
                self.load(&data, "Live GPUI capture", cancel)?;
                let directory = std::env::var_os("LOCALAPPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(std::env::temp_dir)
                    .join("VestaGPUI")
                    .join("Traces");
                std::fs::create_dir_all(&directory)
                    .map_err(|e| TraceError::caused("trace_capture_save", e))?;
                let mut random = [0_u8; 8];
                getrandom::fill(&mut random)
                    .map_err(|e| TraceError::new("trace_capture_save", e.to_string()))?;
                let name = random
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>();
                let path = directory.join(format!("gpui-{name}.json"));
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|e| TraceError::caused("trace_capture_save", e))?;
                file.write_all(&data)
                    .map_err(|e| TraceError::caused("trace_capture_save", e))?;
                if let Some(capture) = &mut self.capture {
                    capture.path = Some(path.clone());
                    capture.status["path"] = json!(path);
                }
            }
            "/native" => {
                let path = self
                    .capture
                    .as_ref()
                    .and_then(|c| c.path.as_ref())
                    .ok_or_else(|| {
                        TraceError::new(
                            "trace_capture_path",
                            "Save or open a capture file before opening native details",
                        )
                    })?;
                std::process::Command::new(
                    std::env::current_exe()
                        .map_err(|e| TraceError::caused("trace_native_executable", e))?,
                )
                .arg("--native")
                .arg(path)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| TraceError::caused("trace_native_launch", e))?;
            }
            _ => {
                return Err(TraceError::new(
                    "trace_target_request",
                    "Unknown capture action",
                ));
            }
        }
        Ok(())
    }
    fn serve(&mut self, mut stream: TcpStream, cancel: &AtomicBool) -> io::Result<()> {
        // Winsock inherits the listener's nonblocking mode. Writes of JS/WASM
        // larger than the socket buffer must wait rather than truncate at WouldBlock.
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(Duration::from_millis(500)))?;
        stream.set_write_timeout(Some(Duration::from_millis(500)))?;
        let request = match Request::read(&stream, cancel) {
            Ok(request) => request,
            Err(_) => {
                return reply(
                    &mut stream,
                    400,
                    "text/plain",
                    b"Invalid request",
                    false,
                    cancel,
                );
            }
        };
        let head = request.method == "HEAD";
        if request.host != self.origin.trim_start_matches("http://")
            || request.origin.as_ref().is_some_and(|o| o != &self.origin)
            || request
                .site
                .as_ref()
                .is_some_and(|s| !matches!(s.as_str(), "same-origin" | "none"))
        {
            return reply(
                &mut stream,
                403,
                "text/plain",
                b"Local origin required",
                head,
                cancel,
            );
        }
        let private = request.path.strip_prefix(&self.prefix);
        if request.method == "POST"
            && matches!(
                private,
                Some("/launch" | "/connect" | "/disconnect" | "/start" | "/stop" | "/native")
            )
        {
            let action = private.unwrap_or("").to_owned();
            let result = self.action(&action, &request.body, cancel);
            self.record_error(result);
            let code = if self.error.is_null() { 200 } else { 422 };
            return reply(
                &mut stream,
                code,
                "application/json",
                self.status().to_string().as_bytes(),
                false,
                cancel,
            );
        }
        if request.method == "POST" && private == Some("/capture") {
            let result = self.load_result(&request.body, cancel);
            self.record_error(result);
            let status = if self.error.is_null() { 200 } else { 422 };
            return reply(
                &mut stream,
                status,
                "application/json",
                self.status().to_string().as_bytes(),
                false,
                cancel,
            );
        }
        if !matches!(request.method.as_str(), "GET" | "HEAD") {
            return reply(
                &mut stream,
                405,
                "text/plain",
                b"Method not allowed",
                head,
                cancel,
            );
        }
        match private {
            Some("/") => reply(
                &mut stream,
                200,
                "text/html; charset=utf-8",
                HTML.as_bytes(),
                head,
                cancel,
            ),
            Some("/host.js") => reply(
                &mut stream,
                200,
                "text/javascript; charset=utf-8",
                SCRIPT.as_bytes(),
                head,
                cancel,
            ),
            Some("/status") => {
                if let Some(target) = &mut self.target
                    && let Err(error) = target.poll()
                {
                    target.status = json!({"state":"error","message":error.message});
                }
                reply(
                    &mut stream,
                    200,
                    "application/json",
                    self.status().to_string().as_bytes(),
                    head,
                    cancel,
                )
            }
            Some(path) if path == format!("/capture/{}", self.revision) => match &self.capture {
                Some(capture) => reply(
                    &mut stream,
                    200,
                    "application/json",
                    &capture.json,
                    head,
                    cancel,
                ),
                None => reply(
                    &mut stream,
                    404,
                    "text/plain",
                    b"No capture loaded",
                    head,
                    cancel,
                ),
            },
            Some(_) => reply(&mut stream, 404, "text/plain", b"Not found", head, cancel),
            None => {
                let path = request
                    .path
                    .split('?')
                    .next()
                    .unwrap_or("")
                    .trim_start_matches('/');
                let path = if path.is_empty() { "index.html" } else { path };
                if self.bundle.files.contains(path) {
                    match self.bundle.root.read_relative(path, 32 * 1024 * 1024) {
                        Ok(resource) => reply(
                            &mut stream,
                            200,
                            resource.content_type,
                            &resource.bytes,
                            head,
                            cancel,
                        ),
                        Err(_) => reply(
                            &mut stream,
                            404,
                            "text/plain",
                            b"Asset unavailable",
                            head,
                            cancel,
                        ),
                    }
                } else {
                    reply(&mut stream, 404, "text/plain", b"Not found", head, cancel)
                }
            }
        }
    }
    fn load_result(&mut self, bytes: &[u8], cancel: &AtomicBool) -> Result<(), TraceError> {
        self.load(bytes, "Opened capture", cancel)
    }
}

struct Request {
    method: String,
    path: String,
    host: String,
    origin: Option<String>,
    site: Option<String>,
    body: Vec<u8>,
}
impl Request {
    fn read(stream: &TcpStream, cancel: &AtomicBool) -> io::Result<Self> {
        let invalid = || io::Error::new(io::ErrorKind::InvalidData, "invalid HTTP request");
        let started = Instant::now();
        let mut reader = BufReader::new(stream);
        let mut lines = Vec::new();
        let mut total = 0;
        loop {
            let mut line = String::new();
            let count = reader
                .by_ref()
                .take((HEAD_LIMIT + 1 - total) as u64)
                .read_line(&mut line)?;
            total += count;
            if count == 0
                || total > HEAD_LIMIT
                || !line.ends_with("\r\n")
                || started.elapsed() > Duration::from_secs(5)
                || cancel.load(Ordering::Relaxed)
            {
                return Err(invalid());
            }
            if line == "\r\n" {
                break;
            }
            lines.push(line);
        }
        let parts: Vec<_> = lines
            .first()
            .ok_or_else(invalid)?
            .split_whitespace()
            .collect();
        if parts.len() != 3
            || !parts[1].starts_with('/')
            || parts[1].starts_with("//")
            || parts[2] != "HTTP/1.1"
        {
            return Err(invalid());
        }
        let (method, path) = (parts[0].to_owned(), parts[1].to_owned());
        let mut headers = std::collections::BTreeMap::new();
        for line in &lines[1..] {
            let (key, value) = line.trim_end().split_once(':').ok_or_else(invalid)?;
            if headers
                .insert(key.to_ascii_lowercase(), value.trim().to_owned())
                .is_some()
            {
                return Err(invalid());
            }
        }
        if headers.contains_key("transfer-encoding") {
            return Err(invalid());
        }
        let length = headers
            .get("content-length")
            .map(|v| v.parse::<usize>())
            .transpose()
            .map_err(|_| invalid())?
            .unwrap_or(0);
        if length as u64 > import::MAX_FILE_BYTES || method != "POST" && length != 0 {
            return Err(invalid());
        }
        let mut body = Vec::new();
        let mut buffer = [0_u8; 64 * 1024];
        while body.len() < length {
            if cancel.load(Ordering::Relaxed) || started.elapsed() > Duration::from_secs(30) {
                return Err(invalid());
            }
            let remaining = (length - body.len()).min(buffer.len());
            let count = reader.read(&mut buffer[..remaining])?;
            if count == 0 {
                return Err(invalid());
            }
            body.extend_from_slice(&buffer[..count]);
        }
        Ok(Self {
            method,
            path,
            host: headers.remove("host").ok_or_else(invalid)?,
            origin: headers.remove("origin"),
            site: headers.remove("sec-fetch-site"),
            body,
        })
    }
}

fn reply(
    stream: &mut TcpStream,
    status: u16,
    mime: &str,
    bytes: &[u8],
    head: bool,
    cancel: &AtomicBool,
) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status} Response\r\nContent-Length: {}\r\nContent-Type: {mime}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: {CSP}\r\nConnection: close\r\n\r\n",
        bytes.len()
    )?;
    if !head {
        let started = Instant::now();
        for chunk in bytes.chunks(64 * 1024) {
            if cancel.load(Ordering::Relaxed) || started.elapsed() > Duration::from_secs(30) {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "response cancelled",
                ));
            }
            stream.write_all(chunk)?;
        }
    }
    Ok(())
}
