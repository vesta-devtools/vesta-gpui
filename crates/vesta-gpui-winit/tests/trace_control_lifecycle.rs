//! Separate real recorder process: authentication, repeat capture and helper retirement.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[test]
fn trace_control_child() {
    if std::env::var_os("VESTA_TRACE_TEST_CHILD").is_none() {
        return;
    }
    let recording =
        vesta_gpui_winit::UiTraceRecording::from_arguments(["host"]).expect("explicit helper");
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        {
            let _frame = vesta_gpui::trace::frame("ControlFixture.Main");
        }
        thread::sleep(Duration::from_millis(2));
    }
    recording.finish().expect("helper retired after event loop");
}

fn ask(address: &str, token: &str, mut request: Value) -> (Value, Vec<u8>) {
    let mut stream = TcpStream::connect(address).expect("connect to real helper");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("deadline");
    request["token"] = json!(token);
    writeln!(stream, "{request}").expect("request");
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).expect("response header");
    let header: Value = serde_json::from_str(&line).expect("JSON response");
    let length = header["bytes"].as_u64().unwrap_or(0);
    let mut data = Vec::new();
    reader
        .take(length)
        .read_to_end(&mut data)
        .expect("complete snapshot");
    assert_eq!(data.len() as u64, length);
    (header, data)
}

#[test]
fn authenticated_control_preserves_target_lifetime_and_recording_ownership() {
    let directory = tempfile::tempdir().expect("private session directory");
    let descriptor = directory.path().join("session.json");
    let token = "1234567890abcdef1234567890abcdef";
    let mut child = Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", "trace_control_child", "--nocapture"])
        .env("VESTA_TRACE_TEST_CHILD", "1")
        .env(
            "VESTA_GPUI_TRACE_CONTROL",
            json!({"descriptor":descriptor,"token":token}).to_string(),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("isolated application process");
    let started = Instant::now();
    let session: Value = loop {
        if let Ok(bytes) = std::fs::read(&descriptor)
            && let Ok(value) = serde_json::from_slice(&bytes)
        {
            break value;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "helper did not advertise its endpoint"
        );
        assert!(
            child.try_wait().expect("child status").is_none(),
            "target exited during startup"
        );
        thread::sleep(Duration::from_millis(10));
    };
    let address = session["address"].as_str().expect("endpoint");
    assert!(address.starts_with("127.0.0.1:"));
    assert_eq!(session["process_id"], child.id());
    let mut wrong = TcpStream::connect(address).expect("wrong-token connection");
    writeln!(wrong, "{}", json!({"op":"start","token":"wrong"})).expect("unauthorized request");
    let mut rejected = Vec::new();
    let _ = wrong.read_to_end(&mut rejected);
    assert!(rejected.is_empty());
    assert_eq!(
        ask(address, token, json!({"op":"status"})).0["state"],
        "idle"
    );
    assert_eq!(
        ask(address, token, json!({"op":"start","max_events":200001})).0["state"],
        "error"
    );
    assert_eq!(
        ask(address, token, json!({"op":"start","duration_seconds":1})).0["state"],
        "recording"
    );
    assert_eq!(
        ask(address, token, json!({"op":"start"})).0["state"],
        "error"
    );
    thread::sleep(Duration::from_millis(40));
    let (header, data) = ask(address, token, json!({"op":"stop"}));
    assert_eq!(header["state"], "captured");
    let capture: Value = serde_json::from_slice(&data).expect("actual recorder export");
    assert!(
        capture["gpui_trace"]["events"]
            .as_u64()
            .expect("event count")
            > 0
    );
    assert!(
        capture["traceEvents"]
            .as_array()
            .expect("events")
            .iter()
            .any(|e| e["name"] == "ControlFixture.Main")
    );
    assert_eq!(
        ask(address, token, json!({"op":"stop"})).0["state"],
        "error"
    );
    assert_eq!(
        ask(address, token, json!({"op":"start","duration_seconds":1})).0["state"],
        "recording"
    );
    thread::sleep(Duration::from_millis(1100));
    assert_eq!(
        ask(address, token, json!({"op":"status"})).0["state"],
        "expired"
    );
    let (_, data) = ask(address, token, json!({"op":"stop"}));
    let capture: Value = serde_json::from_slice(&data).expect("second recording");
    assert_eq!(capture["gpui_trace"]["expired"], true);
    assert_eq!(
        ask(address, token, json!({"op":"disconnect"})).0["state"],
        "disconnected"
    );
    assert!(
        child.try_wait().expect("application status").is_none(),
        "disconnect must leave the application running"
    );
    let started = Instant::now();
    while TcpStream::connect(address).is_ok() {
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "listener did not retire"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert!(child.wait().expect("normal target exit").success());
    assert!(
        !descriptor.exists(),
        "host retirement removes its session descriptor"
    );
}

#[test]
fn on_demand_host_owner_opens_no_capture_and_retires_its_capability() {
    let host = vesta_gpui_winit::UiTraceHost::new().expect("on-demand SDK owner");
    let descriptor = host.descriptor().to_path_buf();
    let config: Value = serde_json::from_slice(&std::fs::read(&descriptor).expect("descriptor"))
        .expect("descriptor JSON");
    let address = config["address"].as_str().expect("endpoint");
    let token = config["token"].as_str().expect("capability");
    assert_eq!(
        ask(address, token, json!({"op":"status"})).0["state"],
        "idle"
    );
    assert!(!vesta_gpui::trace::is_recording());
    assert_eq!(
        ask(address, token, json!({"op":"start","duration_seconds":1})).0["state"],
        "recording"
    );
    {
        let _frame = vesta_gpui::trace::frame("OnDemand.Host");
    }
    let (_, data) = ask(address, token, json!({"op":"stop"}));
    let capture: Value = serde_json::from_slice(&data).expect("on-demand export");
    assert!(
        capture["gpui_frames"]
            .as_array()
            .expect("frames")
            .iter()
            .any(|f| f["name"] == "OnDemand.Host")
    );
    drop(host);
    assert!(!descriptor.exists());
    assert!(TcpStream::connect(address).is_err());
    assert!(!vesta_gpui::trace::is_recording());
}
