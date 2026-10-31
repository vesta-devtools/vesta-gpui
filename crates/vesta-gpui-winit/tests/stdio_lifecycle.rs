//! Real public transport in the normal Cargo test harness; no GUI or OS input.
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread::JoinHandle,
    time::{Duration, Instant},
};

const FIXTURE: &str = "VESTA_GPUI_STDIO_TEST_CHILD";
const TEST: &str = "stdio_budget_recovery_eof_and_size_exit_preserve_process_lifetime";

struct WorkerFinished(mpsc::Sender<()>);
impl Drop for WorkerFinished {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

fn child() {
    let (finished, receiver) = mpsc::channel();
    let finished = WorkerFinished(finished);
    let held = RefCell::new(Vec::new());
    vesta_gpui_winit::start_ui_stdio_with_limit(
        8192,
        move |request, response| {
            let _owner = &finished;
            if request["request_id"] == "held" {
                held.borrow_mut().push(response);
            } else {
                response
                    .send(Ok(json!({"revision":"2","nodes":[]})))
                    .map_err(|_| "fixture_reply_closed")?;
            }
            Ok(())
        },
        Ok::<Value, &'static str>,
    )
    .expect("explicit public worker");
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("worker must release dispatch on EOF or size exit");
}

struct Process {
    child: Child,
    replies: mpsc::Receiver<Value>,
    reader: Option<JoinHandle<()>>,
}
impl Process {
    fn start(role: &str) -> Self {
        let executable = std::env::current_exe().expect("normal test harness");
        let mut child = Command::new(&executable)
            .args(["--exact", TEST, "--nocapture"])
            .env(FIXTURE, role)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("own test child");
        writeln!(
            std::io::stdout().lock(),
            "stdio_child pid={} role={role} exe={}",
            child.id(),
            executable.display()
        )
        .expect("persist own child identity outside test-output capture");
        let stdout = child.stdout.take().expect("own stdout");
        let (sender, replies) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let line = line.expect("child stdout");
                // The enclosing test harness may print a prefix before JSONL.
                if let Some(offset) = line.find('{')
                    && let Ok(value) = serde_json::from_str::<Value>(&line[offset..])
                    && value["schema_version"] == 1
                {
                    let _ = sender.send(value);
                }
            }
        });
        Self {
            child,
            replies,
            reader: Some(reader),
        }
    }
    fn send(&mut self, bytes: &[u8]) {
        let input = self.child.stdin.as_mut().expect("open own stdin");
        input.write_all(bytes).expect("request write");
        input.flush().expect("request flush");
    }
    fn response(&self) -> Value {
        self.replies
            .recv_timeout(Duration::from_secs(3))
            .expect("bounded worker response")
    }
    fn finish(mut self) {
        drop(self.child.stdin.take());
        self.expect_terminal();
    }
    fn expect_terminal(&mut self) {
        assert!(self.child.wait().expect("own child terminal").success());
        self.reader
            .take()
            .expect("stdout reader")
            .join()
            .expect("reader terminal");
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        // Even a failed assertion closes only this child's stream and reaps it.
        // The child has its own finite worker-owner observation deadline.
        drop(self.child.stdin.take());
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

#[test]
fn stdio_budget_recovery_eof_and_size_exit_preserve_process_lifetime() {
    if std::env::var_os(FIXTURE).is_some() {
        child();
        return;
    }
    let mut process = Process::start("eof");
    process.send(b"not-json\n");
    let response = process.response();
    assert!(response["request_id"].is_null());
    assert_eq!(response["error"]["code"], "invalid_request");
    let started = Instant::now();
    process.send(b"{\"schema_version\":1,\"request_id\":\"held\",\"op\":\"wait\",\"instance\":\"fixture\",\"window\":\"1\",\"after_revision\":\"0\",\"timeout_ms\":25}\n");
    let response = process.response();
    assert_eq!(response["request_id"], "held");
    assert_eq!(response["error"]["code"], "wait_timeout");
    assert!(started.elapsed() < Duration::from_secs(1));
    process.send(b"{\"schema_version\":1,\"request_id\":\"healthy\",\"op\":\"snapshot\"}\n");
    let response = process.response();
    assert_eq!(response["request_id"], "healthy");
    assert_eq!(response["result"]["revision"], "2");
    process.finish();

    let mut process = Process::start("oversized");
    let mut bytes = vec![b'x'; 8193];
    bytes.push(b'\n');
    process.send(&bytes);
    let response = process.response();
    assert!(response["request_id"].is_null());
    assert_eq!(response["error"]["code"], "request_too_large");
    // Keep stdin open: the size error itself must terminate the worker.
    process.expect_terminal();
}
