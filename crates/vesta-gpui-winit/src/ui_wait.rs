//! Bounded read-only condition polling on the IO worker, never on the UI thread.
use serde_json::{Value, json};
use std::{
    sync::mpsc::{RecvTimeoutError, SyncSender, sync_channel},
    time::{Duration, Instant},
};
// A condition poll spends the remaining overall budget, never a new host timeout.
pub(super) fn once<R>(
    value: Value,
    dispatch: &impl Fn(Value, SyncSender<Result<R, &'static str>>) -> Result<(), &'static str>,
    serialize: &impl Fn(R) -> Result<Value, &'static str>,
    retire_transport: &mut bool,
    wait_deadline: Option<Instant>,
) -> Result<Value, &'static str> {
    if wait_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return Err("wait_timeout");
    }
    let (sender, receiver) = sync_channel(1);
    dispatch(value, sender)?;
    let timeout = wait_deadline.map_or(Duration::from_secs(10), |deadline| {
        deadline.saturating_duration_since(Instant::now())
    });
    let value = match receiver.recv_timeout(timeout) {
        Ok(value) => value?,
        Err(RecvTimeoutError::Timeout) if wait_deadline.is_some() => return Err("wait_timeout"),
        Err(_) => {
            *retire_transport = true;
            return Err("host_unavailable");
        }
    };
    serialize(value)
}

pub(super) fn wait<R>(
    value: Value,
    dispatch: &impl Fn(Value, SyncSender<Result<R, &'static str>>) -> Result<(), &'static str>,
    serialize: &impl Fn(R) -> Result<Value, &'static str>,
    retire_transport: &mut bool,
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
        match once(
            request.clone(),
            dispatch,
            serialize,
            retire_transport,
            Some(deadline),
        ) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[test]
    fn condition_budget_retires_only_its_reply_and_preserves_transport() {
        let held = RefCell::new(Vec::new());
        let request = json!({"schema_version":1,"request_id":"bounded","op":"wait",
            "instance":"fixture","window":"1","after_revision":"0","timeout_ms":25});
        let mut retire = false;
        let start = Instant::now();
        let result = wait(
            request,
            &|_, reply: SyncSender<Result<Value, &'static str>>| {
                held.borrow_mut().push(reply);
                Ok(())
            },
            &Ok,
            &mut retire,
        );
        assert_eq!(result, Err("wait_timeout"), "elapsed {:?}", start.elapsed());
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(!retire, "a condition deadline must not retire stdio");
        assert_eq!(held.borrow().len(), 1);
        assert!(
            held.borrow()[0].send(Ok(json!({"revision":"1"}))).is_err(),
            "an expired request must reject its late reply"
        );

        let attempts = Cell::new(0);
        let resumed = json!({"schema_version":1,"request_id":"successor","op":"wait",
            "instance":"fixture","window":"1","after_revision":"1","timeout_ms":500,
            "expected_context":{"scope":"same-host"}});
        let result = wait(
            resumed.clone(),
            &|request, response| {
                assert_eq!(request["op"], "snapshot");
                assert_eq!(request["request_id"], "successor");
                assert_eq!(request["expected_context"], resumed["expected_context"]);
                attempts.set(attempts.get() + 1);
                response
                    .send(if attempts.get() == 1 {
                        Err("not_ready")
                    } else {
                        Ok(json!({"revision":"2","nodes":[]}))
                    })
                    .expect("live reply");
                Ok(())
            },
            &Ok,
            &mut retire,
        )
        .expect("condition after transient not_ready");
        assert_eq!(result["wait_satisfied"], true);
        assert_eq!(attempts.get(), 2);
        assert!(!retire);

        assert_eq!(
            wait(resumed, &|_, _| Ok(()), &Ok::<Value, _>, &mut retire),
            Err("host_unavailable")
        );
        assert!(retire, "a disconnected host must retire transport");
    }
}
