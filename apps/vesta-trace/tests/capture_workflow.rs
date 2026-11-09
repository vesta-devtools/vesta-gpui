//! Real SDK host + persistent Agent controller + immutable analysis evidence.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[test]
fn host_fixture() {
    if std::env::var_os("VESTA_CAPTURE_FIXTURE").is_none() {
        return;
    }
    let recording =
        vesta_gpui_winit::UiTraceRecording::from_arguments(["fixture"]).expect("SDK control");
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        {
            let _frame = vesta_gpui::trace::frame("CaptureWorkflow.Main");
            let _work = vesta_gpui::trace::span("fixture", "real.work");
        }
        thread::sleep(Duration::from_millis(2));
    }
    recording.finish().expect("host retires helper");
}

fn ask(input: &mut impl Write, output: &mut impl BufRead, id: &str, command: Value) -> Value {
    let response = response(input, output, id, command);
    assert_eq!(response["result"]["status"], "ok", "{response}");
    response["result"]["data"].clone()
}
fn response(input: &mut impl Write, output: &mut impl BufRead, id: &str, command: Value) -> Value {
    writeln!(
        input,
        "{}",
        json!({"schema_version":1,"request_id":id,"command":command})
    )
    .expect("request");
    input.flush().expect("flush");
    let mut line = String::new();
    assert!(output.read_line(&mut line).expect("response") > 0);
    let response: Value = serde_json::from_str(&line).expect("one JSON response");
    assert_eq!(response["request_id"], id);
    response
}

#[test]
fn repeated_agent_capture_uses_native_importer_and_source_identity() {
    let directory = tempfile::tempdir().expect("capture outputs");
    let destination = directory.path().join("captures");
    std::fs::write(&destination, b"blocked destination").expect("owned failed-save fixture");
    let binary = env!("CARGO_BIN_EXE_vesta-trace");
    let mut agent = Command::new(binary)
        .args([
            "--capture-stdio",
            &format!("--output-dir={}", destination.display()),
        ])
        .env("VESTA_CAPTURE_FIXTURE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("agent controller");
    let mut input = agent.stdin.take().expect("stdin");
    let mut output = BufReader::new(agent.stdout.take().expect("stdout"));
    let launched = ask(
        &mut input,
        &mut output,
        "launch",
        json!({"op":"launch","executable":std::env::current_exe().expect("fixture exe"),
        "arguments":["--exact","host_fixture","--nocapture"]}),
    );
    let pid = launched["process_id"].as_u64().expect("real target PID");
    let started = Instant::now();
    while ask(&mut input, &mut output, "status", json!({"op":"status"}))["state"] != "idle" {
        assert!(started.elapsed() < Duration::from_secs(5));
        thread::sleep(Duration::from_millis(10));
    }
    let mut sources = Vec::new();
    for cycle in 0..2 {
        ask(
            &mut input,
            &mut output,
            "start",
            json!({"op":"start","duration_seconds":1,"max_events":if cycle == 0 {5000} else {64}}),
        );
        assert_eq!(
            response(
                &mut input,
                &mut output,
                "active-detach",
                json!({"op":"detach"})
            )["result"]["code"],
            "trace_capture_active"
        );
        thread::sleep(Duration::from_millis(80));
        if cycle == 0 {
            assert_eq!(
                response(&mut input, &mut output, "failed-save", json!({"op":"stop"}))["result"]["code"],
                "trace_capture_directory"
            );
            assert_eq!(
                ask(&mut input, &mut output, "unsaved", json!({"op":"status"}))["state"],
                "unsaved"
            );
            std::fs::remove_file(&destination).expect("retire owned failed-save fixture");
        } else {
            let full = ask(&mut input, &mut output, "full", json!({"op":"status"}));
            assert_eq!(full["state"], "expired");
            assert_eq!(full["limit_reason"], "event_limit");
        }
        let saved = ask(&mut input, &mut output, "stop", json!({"op":"stop"}));
        assert!(saved["frames"].as_u64().expect("frames") > 0, "{saved}");
        assert!(saved["events"].as_u64().expect("events") > 0);
        let path = saved["path"].as_str().expect("saved capture");
        let mut query = Command::new(binary)
            .args([path, "--query-stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("read-only analyzer");
        writeln!(
            query.stdin.take().expect("query stdin"),
            "{}",
            json!({"schema_version":1,"request_id":"evidence","query":{"op":"session"}})
        )
        .expect("evidence query");
        let result = query.wait_with_output().expect("query complete");
        assert!(result.status.success());
        let result: Value = serde_json::from_slice(&result.stdout).expect("evidence response");
        assert_eq!(result["result"]["status"], "ok");
        assert_eq!(result["source"]["sha256"], saved["source"]["sha256"]);
        sources.push(saved["source"]["sha256"].clone());
        assert_eq!(
            ask(&mut input, &mut output, "idle", json!({"op":"status"}))["process_id"],
            pid,
            "cycle {cycle}"
        );
    }
    assert_ne!(sources[0], sources[1], "independent immutable snapshots");
    assert_eq!(
        ask(&mut input, &mut output, "detach", json!({"op":"detach"}))["state"],
        "detached"
    );
    drop(input);
    assert!(agent.wait().expect("controller EOF retirement").success());
}
