//! Bounded JSONL transport. No renderer, filesystem paths or model dependencies.
use super::{execute, failure, protocol::*};
use crate::session::{Session, TraceError};
use std::{
    io::{BufRead, Read, Write},
    sync::atomic::AtomicBool,
    time::Instant,
};

pub fn schemas() -> serde_json::Value {
    serde_json::json!({
        "schema_version":PROTOCOL_VERSION,"analysis_version":ANALYSIS_VERSION,
        "request":schemars::schema_for!(Request),"response":schemars::schema_for!(Response),
        "transport":{"framing":"one_json_object_per_line","input_opened_once":true,
            "max_request_bytes":MAX_REQUEST_BYTES,"max_response_bytes":MAX_RESPONSE_BYTES,
            "cooperative_deadline":true,"sorting_and_serialization_preemptible":false}
    })
}

struct BoundedOutput {
    bytes: Vec<u8>,
}
impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > MAX_RESPONSE_BYTES {
            return Err(std::io::Error::other("trace_response_budget"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn encode(response: &Response) -> Result<Vec<u8>, TraceError> {
    let mut output = BoundedOutput { bytes: Vec::new() };
    serde_json::to_writer(&mut output, response)
        .map_err(|e| TraceError::caused("trace_response_budget", e))?;
    Ok(output.bytes)
}

pub fn process(session: &Session, bytes: &[u8], cancel: &AtomicBool) -> Vec<u8> {
    let started = Instant::now();
    let request = if bytes.len() > MAX_REQUEST_BYTES {
        Err(TraceError::new(
            "trace_request_budget",
            "request exceeds 64 KiB",
        ))
    } else {
        serde_json::from_slice::<Request>(bytes)
            .map_err(|e| TraceError::caused("trace_request_json", e))
    };
    let request_id = request
        .as_ref()
        .ok()
        .map(|r| r.request_id.clone())
        .or_else(|| {
            (bytes.len() <= MAX_REQUEST_BYTES)
                .then(|| serde_json::from_slice::<serde_json::Value>(bytes).ok())
                .flatten()
                .and_then(|v| {
                    v.get("request_id")
                        .and_then(|v| v.as_str())
                        .filter(|s| s.len() <= 128)
                        .map(String::from)
                })
        });
    let response = match &request {
        Ok(request) => execute(session, request, cancel),
        Err(error) => failure(
            Some(session),
            request_id.clone(),
            TraceError::new(error.code, &error.message),
        ),
    };
    let encoded = encode(&response).and_then(|bytes| {
        if matches!(response.result, Outcome::Ok { .. })
            && request
                .as_ref()
                .is_ok_and(|r| started.elapsed().as_millis() >= r.budget.deadline_ms as u128)
        {
            Err(TraceError::new(
                "trace_deadline",
                "query and bounded response serialization exceeded the cooperative deadline",
            ))
        } else {
            Ok(bytes)
        }
    });
    encoded.unwrap_or_else(|error|{
        // Never serialize a partial successful result or retry oversized quality text.
        let mut fallback=failure(None,request_id,error);fallback.source=Some(session.source.clone());
        encode(&fallback).unwrap_or_else(|_|br#"{"schema_version":1,"result":{"status":"error","code":"trace_response_encoding","message":"response encoding failed"}}"#.to_vec())
    })
}

/// Oversized lines produce one error and close, rather than treating the tail as
/// another request. EOF closes normally; a final line need not have a newline.
pub fn serve(
    session: &Session,
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    cancel: &AtomicBool,
) -> Result<(), TraceError> {
    loop {
        let mut bytes = Vec::new();
        let length = reader
            .by_ref()
            .take((MAX_REQUEST_BYTES + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .map_err(|e| TraceError::caused("trace_query_read", e))?;
        if length == 0 {
            return Ok(());
        }
        let oversized = bytes.len() > MAX_REQUEST_BYTES;
        let response = process(session, &bytes, cancel);
        writer
            .write_all(&response)
            .and_then(|()| writer.write_all(b"\n"))
            .and_then(|()| writer.flush())
            .map_err(|e| TraceError::caused("trace_query_write", e))?;
        if oversized {
            return Ok(());
        }
    }
}
