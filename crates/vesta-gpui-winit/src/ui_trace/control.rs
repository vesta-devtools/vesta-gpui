//! Explicit developer-tool opt-in; no listener, worker or capture in ordinary use.
use serde_json::{Value, json};
use std::{
    fs::OpenOptions,
    io::{self, BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use vesta_gpui::trace::{TraceCapture, TraceConfig, TraceDetail};

pub(super) const ENVIRONMENT: &str = "VESTA_GPUI_TRACE_CONTROL";

pub(super) struct ControlServer {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    descriptor: PathBuf,
}
impl ControlServer {
    pub fn from_environment() -> io::Result<Option<Self>> {
        let Some(config) = std::env::var_os(ENVIRONMENT) else {
            return Ok(None);
        };
        let config: Value =
            serde_json::from_str(&config.to_string_lossy()).map_err(io::Error::other)?;
        let invalid = || {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid trace control descriptor/token",
            )
        };
        let descriptor = PathBuf::from(config["descriptor"].as_str().ok_or_else(invalid)?);
        let token = config["token"].as_str().ok_or_else(invalid)?.to_owned();
        Self::start(descriptor, token).map(Some)
    }

    pub(super) fn start(descriptor: PathBuf, token: String) -> io::Result<Self> {
        if !descriptor.is_absolute()
            || token.len() != 32
            || !token.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid trace control descriptor/token",
            ));
        }
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&descriptor)?;
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let descriptor_token = token.clone();
        let worker_descriptor = descriptor.clone();
        let worker = thread::Builder::new()
            .name("gpui-trace-control".into())
            .spawn(move || {
                let mut capture = None;
                while !worker_stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let _ = serve(stream, &token, &mut capture, &worker_stop);
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10))
                        }
                        Err(_) => break,
                    }
                }
                // Dropping the capture disables recording; the controller never owns app shutdown.
                let _ = std::fs::remove_file(worker_descriptor);
            })?;
        let server = Self {
            stop,
            worker: Some(worker),
            descriptor,
        };
        serde_json::to_writer(
            &mut file,
            &json!({"schema_version":1,"address":address.to_string(),"process_id":std::process::id(),"token":descriptor_token}),
        )?;
        file.flush()?;
        Ok(server)
    }
}
impl Drop for ControlServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let _ = std::fs::remove_file(&self.descriptor);
    }
}

struct ActiveCapture {
    capture: TraceCapture,
    max_events: usize,
}
fn status(capture: &Option<ActiveCapture>) -> Value {
    match capture {
        Some(capture) => {
            let stats = capture.capture.stats();
            let full = stats.events >= capture.max_events;
            let memory = capture.capture.memory_stats();
            let memory_full = memory.is_some_and(|s| s.limit_reached || s.hook_errors != 0);
            json!({"state":if stats.expired || full || memory_full {"expired"} else {"recording"},"events":stats.events,
                "memory_events":memory.map(|s|s.events),"memory_supported":vesta_gpui::trace::memory::allocator_installed(),
                "limit_reason":if memory_full {Some("memory_limit")} else if full {Some("event_limit")} else if stats.expired {Some("duration_limit")} else {None},
                "max_events":capture.max_events,"dropped_events":stats.dropped_events,"elapsed_ms":stats.elapsed.as_millis(),"process_id":std::process::id()})
        }
        None => {
            json!({"state":"idle","events":0,"process_id":std::process::id(),"memory_supported":vesta_gpui::trace::memory::allocator_installed()})
        }
    }
}
fn serve(
    mut stream: TcpStream,
    token: &str,
    capture: &mut Option<ActiveCapture>,
    cancel: &AtomicBool,
) -> io::Result<()> {
    let _quiet = vesta_gpui::trace::memory::suppress();
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;
    let mut line = String::new();
    BufReader::new(&stream).take(4097).read_line(&mut line)?;
    let request: Value = serde_json::from_str(&line).map_err(io::Error::other)?;
    if line.len() > 4096
        || !line.ends_with('\n')
        || request["token"] != token
        || cancel.load(Ordering::Relaxed)
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Trace control authentication failed",
        ));
    }
    let result: Result<Value, String> = (|| match request["op"].as_str() {
        Some("status") => Ok(status(capture)),
        Some("disconnect") => {
            capture.take();
            cancel.store(true, Ordering::Relaxed);
            Ok(json!({"state":"disconnected"}))
        }
        Some("start") => {
            if capture.is_some() {
                return Err("A capture is already active; stop it before starting another".into());
            }
            let events = request["max_events"].as_u64().unwrap_or(100_000);
            let seconds = request["duration_seconds"].as_u64().unwrap_or(30);
            if events == 0 || events > 200_000 || seconds == 0 || seconds > 300 {
                return Err("Capture limits: 1..200000 events and 1..300 seconds".into());
            }
            let detail = match request["detail"].as_str().unwrap_or("stages") {
                "stages" => TraceDetail::Stages,
                "detailed" => TraceDetail::Detailed,
                _ => return Err("Unknown trace detail".into()),
            };
            let memory = if request["memory"].is_null() {
                None
            } else {
                Some(
                    serde_json::from_value(request["memory"].clone())
                        .map_err(|e| format!("Invalid memory configuration: {e}"))?,
                )
            };
            *capture = Some(ActiveCapture {
                capture: TraceCapture::start_with_memory(
                    TraceConfig {
                        max_events: events as usize,
                        max_duration: Duration::from_secs(seconds),
                    },
                    detail,
                    memory,
                )
                .map_err(|e| e.to_string())?,
                max_events: events as usize,
            });
            Ok(status(capture))
        }
        Some("stop") => {
            let capture = capture.take().ok_or("No capture is active")?;
            let mut snapshot = capture.capture.stop();
            crate::trace_allocator::resolve_stacks(&mut snapshot);
            let data = snapshot.to_chrome_json().map_err(|e| e.to_string())?;
            writeln!(stream, "{}", json!({"state":"captured","bytes":data.len()}))
                .map_err(|e| e.to_string())?;
            for chunk in data.as_bytes().chunks(64 * 1024) {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Trace control closed".into());
                }
                stream.write_all(chunk).map_err(|e| e.to_string())?;
            }
            Ok(Value::Null)
        }
        _ => Err("Unknown trace control operation".into()),
    })();
    let response = match result {
        Ok(Value::Null) => return Ok(()),
        Ok(status) => status,
        Err(message) => json!({"state":"error","message":message}),
    };
    writeln!(stream, "{response}")
}
