//! Launch/attach and capture control; only explicit user actions create a target process.
use crate::{import, session::TraceError};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub(crate) struct Target {
    child: Option<Child>,
    directory: Option<tempfile::TempDir>,
    descriptor: PathBuf,
    token: String,
    address: Option<SocketAddr>,
    pub status: Value,
    started: Instant,
}
impl Target {
    pub fn launch(executable: &Path, arguments: &[String]) -> Result<Self, TraceError> {
        let executable = executable
            .canonicalize()
            .map_err(|e| TraceError::caused("trace_target_executable", e))?;
        if !executable.is_file()
            || arguments.len() > 64
            || arguments.iter().map(String::len).sum::<usize>() > 8192
        {
            return Err(TraceError::new(
                "trace_target_executable",
                "Choose a GPUI application executable",
            ));
        }
        let directory = tempfile::Builder::new()
            .prefix("vesta-trace-target-")
            .tempdir()
            .map_err(|e| TraceError::caused("trace_target_directory", e))?;
        let descriptor = directory.path().join("session.json");
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random)
            .map_err(|e| TraceError::new("trace_target_token", e.to_string()))?;
        let token = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        // No shell, PATH lookup, command concatenation or implicit capture.
        let child = Command::new(&executable)
            .args(arguments)
            .env(
                "VESTA_GPUI_TRACE_CONTROL",
                json!({"descriptor":descriptor,"token":token}).to_string(),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| TraceError::caused("trace_target_launch", e))?;
        let pid = child.id();
        Ok(Self {
            child: Some(child),
            directory: Some(directory),
            descriptor,
            token,
            address: None,
            status: json!({"state":"connecting","process_id":pid,"application":executable.file_name().and_then(|n| n.to_str())}),
            started: Instant::now(),
        })
    }
    pub fn connect(descriptor: PathBuf) -> Result<Self, TraceError> {
        let mut target = Self {
            child: None,
            directory: None,
            descriptor,
            token: String::new(),
            address: None,
            status: json!({"state":"connecting"}),
            started: Instant::now(),
        };
        target.read_descriptor()?;
        target.poll()?;
        Ok(target)
    }
    fn read_descriptor(&mut self) -> Result<(), TraceError> {
        let mut bytes = Vec::new();
        File::open(&self.descriptor)
            .and_then(|f| f.take(4097).read_to_end(&mut bytes))
            .map_err(|e| TraceError::caused("trace_target_descriptor", e))?;
        if bytes.len() > 4096 {
            return Err(TraceError::new(
                "trace_target_descriptor",
                "Session descriptor is too large",
            ));
        }
        let config: Value = serde_json::from_slice(&bytes)
            .map_err(|e| TraceError::caused("trace_target_descriptor", e))?;
        let address: SocketAddr = config["address"]
            .as_str()
            .unwrap_or("")
            .parse()
            .map_err(|e| TraceError::caused("trace_target_address", e))?;
        let token = config["token"].as_str().unwrap_or("");
        if config["schema_version"] != 1
            || !address.ip().is_loopback()
            || address.port() == 0
            || token.len() != 32
            || !token.bytes().all(|b| b.is_ascii_hexdigit())
            || !self.token.is_empty() && self.token != token
        {
            return Err(TraceError::new(
                "trace_target_descriptor",
                "Invalid local session identity",
            ));
        }
        self.token = token.to_owned();
        self.address = Some(address);
        Ok(())
    }
    pub fn poll(&mut self) -> Result<(), TraceError> {
        if let Some(child) = &mut self.child
            && let Some(exit) = child
                .try_wait()
                .map_err(|e| TraceError::caused("trace_target_process", e))?
        {
            self.status = json!({"state":"exited","exit_code":exit.code(),"process_id":child.id()});
            self.address = None;
            return Ok(());
        }
        if self.address.is_none() {
            if !self.descriptor.exists() {
                if self.started.elapsed() > Duration::from_secs(20) {
                    return Err(TraceError::new(
                        "trace_target_unavailable",
                        "Application did not expose GPUI trace control. It must use UiTraceRecording::from_arguments from this SDK.",
                    ));
                }
                return Ok(());
            }
            // The process may still be flushing its newly-created descriptor.
            if let Err(error) = self.read_descriptor() {
                if self.started.elapsed() < Duration::from_secs(2) {
                    return Ok(());
                }
                return Err(error);
            }
        }
        let (status, _) = self.request(json!({"op":"status"}))?;
        self.status = status;
        Ok(())
    }
    pub fn start(&mut self, limits: &Value) -> Result<(), TraceError> {
        let (status, _) = self.request(json!({"op":"start","max_events":limits["max_events"],
            "duration_seconds":limits["duration_seconds"],"detail":limits["detail"],"memory":limits["memory"]}))?;
        self.status = status;
        Ok(())
    }
    pub fn stop(&mut self) -> Result<Vec<u8>, TraceError> {
        let (_, data) = self.request(json!({"op":"stop"}))?;
        self.status["state"] = json!("idle");
        self.status["events"] = json!(0);
        self.status["elapsed_ms"] = json!(0);
        self.status
            .as_object_mut()
            .map(|m| m.remove("limit_reason"));
        Ok(data)
    }
    fn request(&self, mut request: Value) -> Result<(Value, Vec<u8>), TraceError> {
        let address = self.address.ok_or_else(|| {
            TraceError::new(
                "trace_target_connecting",
                "Wait for the application to connect",
            )
        })?;
        let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(500))
            .map_err(|e| TraceError::caused("trace_target_connection", e))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(if request["op"] == "stop" {
                30
            } else {
                5
            })))
            .map_err(|e| TraceError::caused("trace_target_connection", e))?;
        stream
            .set_write_timeout(Some(Duration::from_millis(500)))
            .map_err(|e| TraceError::caused("trace_target_connection", e))?;
        request["token"] = json!(self.token);
        writeln!(stream, "{request}").map_err(|e| TraceError::caused("trace_target_request", e))?;
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader
            .by_ref()
            .take(4097)
            .read_line(&mut line)
            .map_err(|e| TraceError::caused("trace_target_response", e))?;
        if line.len() > 4096 {
            return Err(TraceError::new(
                "trace_target_response",
                "Invalid response header",
            ));
        }
        let status: Value = serde_json::from_str(&line)
            .map_err(|e| TraceError::caused("trace_target_response", e))?;
        if status["state"] == "error" {
            return Err(TraceError::new(
                "trace_target_capture",
                status["message"].as_str().unwrap_or("Capture failed"),
            ));
        }
        let length = status["bytes"].as_u64().unwrap_or(0);
        if length > import::MAX_FILE_BYTES {
            return Err(TraceError::new(
                "trace_target_budget",
                "Capture exceeds the import limit",
            ));
        }
        let mut bytes = Vec::new();
        reader
            .take(length)
            .read_to_end(&mut bytes)
            .map_err(|e| TraceError::caused("trace_target_response", e))?;
        if bytes.len() as u64 != length {
            return Err(TraceError::new(
                "trace_target_response",
                "Capture transfer was interrupted",
            ));
        }
        Ok((status, bytes))
    }
}
impl Drop for Target {
    fn drop(&mut self) {
        // Leave the user's application running, but retire this tool's listener/capture.
        if self.directory.is_some() && self.address.is_some() {
            let _ = self.request(json!({"op":"disconnect"}));
        }
    }
}
