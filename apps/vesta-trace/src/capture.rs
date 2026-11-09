//! Shared capture lifecycle for the native UI and versioned Agent transport.
use crate::{capture_target::Target, import, session::TraceError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::OpenOptions,
    io::{BufRead, Read, Write},
    path::PathBuf,
    sync::atomic::AtomicBool,
};

const MAX_REQUEST: usize = 16 * 1024;
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Status {},
    Launch {
        executable: PathBuf,
        #[serde(default)]
        arguments: Vec<String>,
    },
    Connect {
        descriptor: PathBuf,
    },
    Start {
        #[serde(default = "seconds")]
        duration_seconds: u64,
        #[serde(default = "events")]
        max_events: u64,
        #[serde(default)]
        detail: Detail,
        #[serde(default)]
        memory: Option<MemoryOptions>,
    },
    Stop {},
    Detach {},
}
fn seconds() -> u64 {
    30
}
fn events() -> u64 {
    100_000
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MemoryOptions {
    #[serde(default = "memory_events")]
    pub max_events: usize,
    #[serde(default = "stack_frames")]
    pub stack_frames: usize,
}
impl Default for MemoryOptions {
    fn default() -> Self {
        Self {
            max_events: memory_events(),
            stack_frames: stack_frames(),
        }
    }
}
fn memory_events() -> usize {
    100_000
}
fn stack_frames() -> usize {
    32
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Detail {
    #[default]
    Stages,
    Detailed,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Request {
    schema_version: u32,
    request_id: String,
    command: Command,
}
#[derive(Serialize, JsonSchema)]
struct Response {
    schema_version: u32,
    request_id: Option<String>,
    result: Outcome,
}
#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Outcome {
    Ok { data: Value },
    Error { code: String, message: String },
}

pub struct Session {
    target: Option<Target>,
    output: PathBuf,
    // A failed disk write can be retried with stop, without losing the frozen capture.
    unsaved: Option<Vec<u8>>,
    owns_capture: bool,
}
impl Session {
    pub fn new(output: Option<PathBuf>) -> Self {
        let output = output.unwrap_or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join("VestaGPUI")
                .join("Traces")
        });
        Self {
            target: None,
            output,
            unsaved: None,
            owns_capture: false,
        }
    }
    pub fn execute(&mut self, command: Command) -> Result<Value, TraceError> {
        match command {
            Command::Status {} => {
                if let Some(target) = &mut self.target
                    && let Err(error) = target.poll()
                {
                    target.status["state"] = json!("unavailable");
                    return Err(error);
                }
                Ok(self.status())
            }
            Command::Launch {
                executable,
                arguments,
            } => {
                self.require_detached()?;
                self.target = Some(Target::launch(&executable, &arguments)?);
                Ok(self.status())
            }
            Command::Connect { descriptor } => {
                self.require_detached()?;
                self.target = Some(Target::connect(descriptor)?);
                Ok(self.status())
            }
            Command::Start {
                duration_seconds,
                max_events,
                detail,
                memory,
            } => {
                if self.unsaved.is_some() {
                    return Err(TraceError::new(
                        "trace_capture_unsaved",
                        "Retry stop to save the frozen capture first",
                    ));
                }
                if !(1..=300).contains(&duration_seconds) || !(1..=200_000).contains(&max_events) {
                    return Err(TraceError::new(
                        "trace_capture_limits",
                        "Use 1..300 seconds and 1..200000 events",
                    ));
                }
                if memory.is_some_and(|m| {
                    m.max_events == 0 || m.max_events > 200_000 || m.stack_frames > 32
                }) {
                    return Err(TraceError::new(
                        "trace_capture_memory_limits",
                        "Use 1..200000 memory events and 0..32 stack frames",
                    ));
                }
                let target = self.target()?;
                target.poll()?;
                target.start(&json!({"duration_seconds":duration_seconds,"max_events":max_events,"detail":detail,"memory":memory}))?;
                self.owns_capture = true;
                Ok(self.status())
            }
            Command::Stop {} => {
                if self.unsaved.is_none() {
                    self.unsaved = Some(self.target()?.stop()?);
                    self.owns_capture = false;
                }
                let bytes = self.unsaved.as_ref().expect("frozen snapshot");
                std::fs::create_dir_all(&self.output)
                    .map_err(|e| TraceError::caused("trace_capture_directory", e))?;
                let mut random = [0u8; 16];
                getrandom::fill(&mut random)
                    .map_err(|e| TraceError::new("trace_capture_identity", e.to_string()))?;
                let name = random
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>();
                let path = self.output.join(format!("gpui-{name}.json"));
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|e| TraceError::caused("trace_capture_save", e))?;
                file.write_all(bytes)
                    .and_then(|()| file.flush())
                    .map_err(|e| TraceError::caused("trace_capture_save", e))?;
                // Exactly the same importer and source identity used by native/Agent analysis.
                let capture = import::read(&path, &AtomicBool::new(false))?;
                self.unsaved = None;
                Ok(json!({"state":"saved","path":path,"source":capture.source,
                    "events":capture.events.len(),"frames":capture.frames.len(),"diagnostics":capture.diagnostics}))
            }
            Command::Detach {} => {
                if self.unsaved.is_some()
                    || self.target.as_ref().is_some_and(|t| {
                        t.status["state"] == "recording" || t.status["state"] == "expired"
                    })
                {
                    return Err(TraceError::new(
                        "trace_capture_active",
                        "Stop and save before detaching",
                    ));
                }
                self.target = None;
                Ok(self.status())
            }
        }
    }
    pub fn status(&self) -> Value {
        if self.unsaved.is_some() {
            return json!({"state":"unsaved"});
        }
        self.target
            .as_ref()
            .map(|t| t.status.clone())
            .unwrap_or_else(|| json!({"state":"detached"}))
    }
    pub fn should_finish(&self) -> bool {
        self.owns_capture && self.status()["state"] == "expired"
    }
    pub fn has_owned_capture(&self) -> bool {
        self.owns_capture || self.unsaved.is_some()
    }
    fn target(&mut self) -> Result<&mut Target, TraceError> {
        self.target.as_mut().ok_or_else(|| {
            TraceError::new(
                "trace_capture_detached",
                "Connect or launch a GPUI host first",
            )
        })
    }
    fn require_detached(&self) -> Result<(), TraceError> {
        if self.target.is_some() || self.unsaved.is_some() {
            Err(TraceError::new(
                "trace_capture_connected",
                "Detach before changing the target",
            ))
        } else {
            Ok(())
        }
    }
}

pub fn schemas() -> Value {
    json!({"schema_version":1,"request":schemars::schema_for!(Request),"response":schemars::schema_for!(Response),
        "transport":{"framing":"one_json_object_per_line","max_request_bytes":MAX_REQUEST},
        "analysis":"Use the saved path with --query-stdio; source.sha256 identifies the shared evidence"})
}
pub fn serve(
    output: Option<PathBuf>,
    reader: &mut impl BufRead,
    writer: &mut impl Write,
) -> Result<(), TraceError> {
    let mut session = Session::new(output);
    loop {
        let mut bytes = Vec::new();
        let length = reader
            .by_ref()
            .take((MAX_REQUEST + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .map_err(|e| TraceError::caused("trace_capture_input", e))?;
        if length == 0 {
            return Ok(());
        }
        let oversized = length > MAX_REQUEST;
        let request = if oversized {
            Err(TraceError::new(
                "trace_capture_request_budget",
                "Request exceeds 16 KiB; transport will close",
            ))
        } else {
            serde_json::from_slice::<Request>(&bytes)
                .map_err(|e| TraceError::caused("trace_capture_request", e))
        };
        let id = request.as_ref().ok().map(|r| r.request_id.clone());
        let result = request.and_then(|request| {
            if request.schema_version != 1
                || request.request_id.is_empty()
                || request.request_id.len() > 128
            {
                return Err(TraceError::new(
                    "trace_capture_protocol",
                    "Use schema_version 1 and a request_id of 1..128 bytes",
                ));
            }
            session.execute(request.command)
        });
        let response = Response {
            schema_version: 1,
            request_id: id,
            result: match result {
                Ok(data) => Outcome::Ok { data },
                Err(e) => Outcome::Error {
                    code: e.code.into(),
                    message: e.message,
                },
            },
        };
        serde_json::to_writer(&mut *writer, &response)
            .map_err(|e| TraceError::caused("trace_capture_output", e))?;
        writer
            .write_all(b"\n")
            .and_then(|()| writer.flush())
            .map_err(|e| TraceError::caused("trace_capture_output", e))?;
        if oversized {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_transport_rejects_unknown_version_fields_and_oversized_lines() {
        let input = br#"{"schema_version":2,"request_id":"version","command":{"op":"status"}}
{"schema_version":1,"request_id":"extra","command":{"op":"status","oops":true}}
{"schema_version":1,"request_id":"idle","command":{"op":"status"}}
{"schema_version":1,"request_id":"start","command":{"op":"start","duration_seconds":0}}
"#;
        let mut out = Vec::new();
        serve(None, &mut input.as_slice(), &mut out).expect("JSONL");
        let results: Vec<Value> = String::from_utf8(out)
            .expect("UTF8")
            .lines()
            .map(|l| serde_json::from_str(l).expect("response"))
            .collect();
        assert_eq!(results.len(), 4);
        assert_eq!(results[0]["result"]["code"], "trace_capture_protocol");
        assert_eq!(results[1]["result"]["status"], "error");
        assert_eq!(results[2]["result"]["data"]["state"], "detached");
        assert_eq!(results[3]["result"]["code"], "trace_capture_limits");
        let mut input = vec![b' '; MAX_REQUEST + 1];
        input.extend_from_slice(b"\n{}\n");
        let mut out = Vec::new();
        serve(None, &mut input.as_slice(), &mut out).expect("bounded close");
        assert_eq!(out.iter().filter(|b| **b == b'\n').count(), 1);
    }
}
