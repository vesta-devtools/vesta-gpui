//! Capture IPC/file I/O never runs on the native presentation thread.
use crate::{
    capture::{Command, Session},
    session::TraceError,
};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use winit::event_loop::EventLoopProxy;

pub(super) struct Message {
    pub completed_command: bool,
    pub status: Value,
    pub saved: Option<PathBuf>,
    pub error: Option<String>,
}
pub(super) struct Worker {
    sender: SyncSender<Command>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Worker {
    pub fn new(
        proxy: EventLoopProxy<super::UserEvent>,
        output: Option<PathBuf>,
        initial: Option<Command>,
    ) -> Result<Self, TraceError> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = stop.clone();
        let thread = thread::Builder::new()
            .name("trace-capture-io".into())
            .spawn(move || {
                let mut session = Session::new(output);
                let mut initial = initial;
                let mut published = session.status();
                let mut published_error = None;
                while !cancelled.load(Ordering::Relaxed) {
                    let command = initial
                        .take()
                        .or_else(|| receiver.recv_timeout(Duration::from_millis(250)).ok());
                    let explicit = command.is_some();
                    if !explicit && session.status()["state"] == "detached" {
                        continue;
                    }
                    let mut result = session.execute(command.unwrap_or(Command::Status {}));
                    if result.is_ok() && session.should_finish() {
                        result = session.execute(Command::Stop {});
                    }
                    let status = session.status();
                    let saved = result
                        .as_ref()
                        .ok()
                        .and_then(|v| v["path"].as_str())
                        .map(PathBuf::from);
                    let error = result.err().map(|e| e.to_string());
                    if explicit
                        || status != published
                        || saved.is_some()
                        || error != published_error
                    {
                        published = status.clone();
                        published_error = error.clone();
                        if proxy
                            .send_event(super::UserEvent::Capture(Message {
                                completed_command: explicit,
                                status,
                                saved,
                                error,
                            }))
                            .is_err()
                        {
                            break;
                        }
                    }
                }
                if session.has_owned_capture() {
                    match session.execute(Command::Stop {}) {
                        Ok(saved) => eprintln!("trace_capture_saved_on_close {}", saved["path"]),
                        Err(error) => eprintln!("trace_capture_close: {error}"),
                    }
                }
            })
            .map_err(|e| TraceError::caused("trace_capture_worker", e))?;
        Ok(Self {
            sender,
            stop,
            thread: Some(thread),
        })
    }
    pub fn submit(&self, command: Command) -> Result<(), TraceError> {
        self.sender
            .try_send(command)
            .map_err(|e| TraceError::caused("trace_capture_busy", e))
    }
    pub fn finish(&mut self) -> Result<(), TraceError> {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| TraceError::new("trace_capture_worker", "Capture worker panicked"))?;
        }
        Ok(())
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}
