//! Bounded read-only condition polling on the IO worker, never on the UI thread.
use serde_json::{Value, json};
use std::{
    sync::mpsc::{SyncSender, sync_channel},
    time::{Duration, Instant},
};
pub(super) fn once<R>(
    value: Value,
    dispatch: &impl Fn(Value, SyncSender<Result<R, &'static str>>) -> Result<(), &'static str>,
    serialize: &impl Fn(R) -> Result<Value, &'static str>,
    timed_out: &mut bool,
) -> Result<Value, &'static str> {
    let (sender, receiver) = sync_channel(1);
    dispatch(value, sender)?;
    serialize(
        receiver
            .recv_timeout(Duration::from_secs(10))
            .map_err(|_| {
                *timed_out = true;
                "host_unavailable"
            })??,
    )
}
pub(super) fn wait<R>(
    value: Value,
    dispatch: &impl Fn(Value, SyncSender<Result<R, &'static str>>) -> Result<(), &'static str>,
    serialize: &impl Fn(R) -> Result<Value, &'static str>,
    timed_out: &mut bool,
) -> Result<Value, &'static str> {
    let allowed = [
        "schema_version",
        "request_id",
        "op",
        "instance",
        "window",
        "after_revision",
        "condition",
        "timeout_ms",
        "expected_context",
    ];
    if value
        .as_object()
        .is_none_or(|v| v.keys().any(|key| !allowed.contains(&key.as_str())))
    {
        return Err("invalid_request");
    }
    let timeout = value
        .get("timeout_ms")
        .map_or(Some(1000), Value::as_u64)
        .filter(|n| (1..=5000).contains(n))
        .ok_or("invalid_limits")?;
    let after = value
        .get("after_revision")
        .map(|n| {
            n.as_str()
                .ok_or("invalid_request")?
                .parse::<u64>()
                .map_err(|_| "invalid_request")
        })
        .transpose()?;
    let condition = value.get("condition");
    if condition.is_none() && after.is_none() {
        return Err("invalid_request");
    }
    if let Some(condition) = condition {
        if condition.as_object().is_none_or(|v| {
            v.len() != 3
                || v.keys()
                    .any(|key| !["element", "property", "equals"].contains(&key.as_str()))
        }) {
            return Err("invalid_request");
        }
        condition["element"]
            .as_str()
            .ok_or("invalid_request")?
            .parse::<u64>()
            .map_err(|_| "invalid_request")?;
        match condition["property"].as_str() {
            Some("value" | "text") if condition["equals"].is_string() => {}
            Some("selected" | "expanded" | "focused") if condition["equals"].is_boolean() => {}
            _ => return Err("invalid_request"),
        }
    }
    let deadline = Instant::now() + Duration::from_millis(timeout);
    let mut request = json!({"schema_version":1,"request_id":value["request_id"],"op":"snapshot","instance":value["instance"],"window":value["window"],"limit":1});
    if let Some(context) = value.get("expected_context") {
        request["expected_context"] = context.clone();
    }
    if let Some(condition) = condition {
        request["root"] = condition["element"].clone();
    }
    loop {
        match once(request.clone(), dispatch, serialize, timed_out) {
            Ok(mut snapshot) => {
                let revision = snapshot["revision"]
                    .as_str()
                    .and_then(|r| r.parse::<u64>().ok())
                    .ok_or("invalid_snapshot")?;
                let matches = if let Some(condition) = condition {
                    let node = &snapshot["nodes"][0];
                    let property = condition["property"].as_str().ok_or("invalid_request")?;
                    if matches!(property, "value" | "text") {
                        if node[property]["redacted"] == true {
                            return Err("sensitive_target");
                        }
                        if node[property]["truncated"] == true {
                            return Err("incomplete_text");
                        }
                        node[property]["value"] == condition["equals"]
                    } else {
                        node[property] == condition["equals"]
                    }
                } else {
                    true
                };
                if after.is_none_or(|old| revision > old) && matches {
                    snapshot["wait_satisfied"] = Value::Bool(true);
                    return Ok(snapshot);
                }
            }
            Err("not_ready" | "node_not_captured") => {}
            Err(error) => return Err(error),
        }
        if Instant::now() >= deadline {
            return Err("wait_timeout");
        }
        std::thread::sleep(
            Duration::from_millis(50).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}
