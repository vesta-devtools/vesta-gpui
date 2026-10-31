//! A local, bounded request/reply worker. Application parsing and authority stay with the Host.
use serde_json::{Value, json};
use std::{
    io::{BufRead, Read, Write},
    sync::mpsc::SyncSender,
};

/// Explicitly start the process-lifetime stdin/stdout JSONL worker.
///
/// Call once from a native process host. This detached worker exclusively locks
/// stdin/stdout until EOF, an I/O/size failure or a lost ordinary host response.
/// There is no cancellation/join handle: closing a window cannot unblock stdin.
/// Embedded controllers reuse their host's connection instead of starting one.
/// Dispatch must enqueue without blocking; serialization must also be nonblocking.
pub fn start_ui_stdio<R: Send + 'static>(
    dispatch: impl Fn(Value, SyncSender<Result<R, &'static str>>) -> Result<(), &'static str>
    + Send
    + 'static,
    serialize: impl Fn(R) -> Result<Value, &'static str> + Send + 'static,
) -> std::io::Result<()> {
    start_ui_stdio_with_limit(8192, dispatch, serialize)
}

/// The same process-lifetime worker with an explicit 8–256 KiB input-line bound.
/// Ordinary replies wait at most ten seconds; condition waits share their own
/// deadline. A condition timeout replies without retiring the connection.
/// This cannot preempt a blocking host callback or blocked stdout pipe.
pub fn start_ui_stdio_with_limit<R: Send + 'static>(
    input_limit: usize,
    dispatch: impl Fn(Value, SyncSender<Result<R, &'static str>>) -> Result<(), &'static str>
    + Send
    + 'static,
    serialize: impl Fn(R) -> Result<Value, &'static str> + Send + 'static,
) -> std::io::Result<()> {
    if !(8192..=262144).contains(&input_limit) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "UI stdio input limit must be 8–256 KiB",
        ));
    }
    std::thread::Builder::new().name("gpui-access-stdio".into()).spawn(move || {
        let mut input = std::io::stdin().lock();
        let mut output = std::io::stdout().lock();
        loop {
            let mut line = Vec::new();
                let read = input.by_ref().take(input_limit as u64 + 1).read_until(b'\n', &mut line);
            if matches!(read, Ok(0)) { break; }
                if read.is_err() || line.len() > input_limit {
                    let response = json!({"schema_version":1,"request_id":null,"error":{"code":if line.len()>input_limit {"request_too_large"} else {"input_read_failed"}}});
                let _ = writeln!(output,"{response}").and_then(|()|output.flush());
                break;
            }
            let value = serde_json::from_slice::<Value>(&line).unwrap_or(Value::Null);
            let id = value["request_id"].as_str().filter(|id| !id.is_empty() && id.len()<=128).map(str::to_owned);
            let mut retire_transport = false;
            let result = if id.is_none() || value["schema_version"] != 1 {
                Err("invalid_request")
            } else if value["op"] == "wait" {
                crate::ui_wait::wait(value, &dispatch, &serialize, &mut retire_transport)
            } else {
                crate::ui_wait::once(value, &dispatch, &serialize, &mut retire_transport, None)
            };
            let response = match result {
                Ok(result)=>json!({"schema_version":1,"request_id":id,"result":result}),
                Err(code)=>json!({"schema_version":1,"request_id":id,"error":{"code":code}}),
            };
            if writeln!(output,"{response}").and_then(|()|output.flush()).is_err() || retire_transport { break; }
        }
    })?;
    Ok(())
}
