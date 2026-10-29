//! Versioned, read-only JSONL fixture. One bounded outstanding request, no network service.
use super::*;
use serde_json::{Value, json};
use vesta_gpui::UiQuery;

fn parse(value: &Value) -> Result<Operation, &'static str> {
    let object = value.as_object().ok_or("invalid_request")?;
    let id = value["request_id"].as_str().ok_or("invalid_request")?;
    if id.is_empty() || id.len() > 128 || value["schema_version"] != 1 {
        return Err("invalid_request");
    }
    let op = value["op"].as_str().ok_or("invalid_request")?;
    if op == "input" {
        return Ok(Operation::Input {
            request_id: id.into(),
            request: vesta_gpui_winit::UiInputRequest::parse(value)?,
        });
    }
    if matches!(op, "window_state" | "input_state") {
        if object.keys().any(|key| {
            !["schema_version", "request_id", "op", "instance", "window"].contains(&key.as_str())
        }) {
            return Err("invalid_request");
        }
        let instance = value["instance"]
            .as_str()
            .and_then(|v| v.parse().ok())
            .ok_or("invalid_request")?;
        let window = UiWindowId(
            value["window"]
                .as_str()
                .and_then(|v| v.parse().ok())
                .ok_or("invalid_request")?,
        );
        return Ok(if op == "window_state" {
            Operation::WindowState { instance, window }
        } else {
            Operation::InputState { instance, window }
        });
    }
    if op == "changes" {
        let keys = [
            "schema_version",
            "request_id",
            "op",
            "instance",
            "window",
            "since",
        ];
        if object.keys().any(|key| !keys.contains(&key.as_str())) {
            return Err("invalid_request");
        }
        return Ok(Operation::Changes {
            instance: value["instance"]
                .as_str()
                .ok_or("invalid_request")?
                .parse()
                .map_err(|_| "invalid_request")?,
            window: UiWindowId(
                value["window"]
                    .as_str()
                    .ok_or("invalid_request")?
                    .parse()
                    .map_err(|_| "invalid_request")?,
            ),
            since: value["since"]
                .as_str()
                .ok_or("invalid_request")?
                .parse()
                .map_err(|_| "invalid_request")?,
        });
    }
    let allowed = if op == "windows" {
        &["schema_version", "request_id", "op"][..]
    } else {
        &[
            "schema_version",
            "request_id",
            "op",
            "instance",
            "window",
            "revision",
            "root",
            "after",
            "limit",
        ][..]
    };
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("invalid_request");
    }
    match op {
        "windows" => Ok(Operation::Windows),
        "snapshot" => {
            let number = |name: &str| {
                value[name]
                    .as_str()
                    .ok_or("invalid_request")?
                    .parse::<u64>()
                    .map_err(|_| "invalid_request")
            };
            let optional = |name: &str| {
                if value.get(name).is_none() {
                    Ok(None)
                } else {
                    number(name).map(Some)
                }
            };
            let limit = value
                .get("limit")
                .map_or(Some(128), Value::as_u64)
                .ok_or("invalid_request")?;
            if !(1..=256).contains(&limit) {
                return Err("invalid_limits");
            }
            let revision = optional("revision")?;
            let after = optional("after")?.map(ElementId::new);
            if after.is_some() && revision.is_none() {
                return Err("invalid_cursor");
            }
            Ok(Operation::Snapshot {
                instance: value["instance"]
                    .as_str()
                    .ok_or("invalid_request")?
                    .parse()
                    .map_err(|_| "invalid_request")?,
                window: UiWindowId(number("window")?),
                revision,
                root: optional("root")?.map(ElementId::new),
                after,
                limit: limit as usize,
            })
        }
        _ => Err("unknown_query"),
    }
}

fn parse_control(value: &Value) -> Result<Operation, &'static str> {
    use vesta_gpui::{UiCommand, UiTargetRef};
    let object = value.as_object().ok_or("invalid_request")?;
    let id = value["request_id"]
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 128)
        .ok_or("invalid_request")?;
    if value["schema_version"] != 1 {
        return Err("invalid_request");
    }
    let op = value["op"].as_str().ok_or("invalid_request")?;
    if op == "capture" {
        return super::capture::parse(value);
    }
    if !matches!(op, "commands" | "invoke") {
        return parse(value);
    }
    let allowed = if op == "commands" {
        &["schema_version", "request_id", "op", "instance", "window"][..]
    } else {
        &[
            "schema_version",
            "request_id",
            "op",
            "instance",
            "window",
            "element",
            "owner",
            "command",
            "args",
        ][..]
    };
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("invalid_request");
    }
    let number = |key: &str| {
        value[key]
            .as_str()
            .ok_or("invalid_request")?
            .parse::<u64>()
            .map_err(|_| "invalid_request")
    };
    let instance = value["instance"]
        .as_str()
        .ok_or("invalid_request")?
        .parse::<u128>()
        .map_err(|_| "invalid_request")?;
    let window = UiWindowId(number("window")?);
    if op == "commands" {
        return Ok(Operation::Commands { instance, window });
    }
    let args = value.get("args");
    let command = match value["command"].as_str().ok_or("invalid_request")? {
        "activate" => UiCommand::Activate,
        "focus" => UiCommand::Focus,
        "reveal" => UiCommand::Reveal,
        "select_all" => UiCommand::SelectAll,
        "undo" => UiCommand::Undo,
        "redo" => UiCommand::Redo,
        "set_expanded" => UiCommand::SetExpanded(
            value["args"]["expanded"]
                .as_bool()
                .ok_or("invalid_request")?,
        ),
        "set_value" => UiCommand::SetValue(
            value["args"]["value"]
                .as_f64()
                .filter(|v| v.is_finite())
                .ok_or("invalid_request")?,
        ),
        "replace_text" => UiCommand::ReplaceText {
            expected: value["args"]["expected"]
                .as_str()
                .ok_or("invalid_request")?
                .into(),
            text: value["args"]["text"]
                .as_str()
                .ok_or("invalid_request")?
                .into(),
        },
        _ => return Err("unknown_command"),
    };
    let arg_keys: &[&str] = match command {
        UiCommand::SetExpanded(_) => &["expanded"],
        UiCommand::SetValue(_) => &["value"],
        UiCommand::ReplaceText { .. } => &["expected", "text"],
        _ => &[],
    };
    if args.is_some_and(|args| {
        args.as_object()
            .is_none_or(|args| args.keys().any(|key| !arg_keys.contains(&key.as_str())))
    }) {
        return Err("invalid_request");
    }
    Ok(Operation::Invoke {
        request_id: id.into(),
        target: UiTargetRef {
            instance,
            window,
            element: ElementId::new(number("element")?),
            owner: number("owner")?,
        },
        command,
    })
}

fn serialize(reply: Reply, operation: &Operation) -> Result<Value, &'static str> {
    match reply {
        Reply::Changes(current, previous) => vesta_gpui_winit::ui_changes_json(&current, &previous),
        Reply::Capture(reply) => reply.serialize(),
        Reply::Control(value) => Ok(value),
        Reply::Windows {
            instance,
            read_only,
            windows,
            truncated,
        } => Ok(json!({"instance":instance.to_string(),
            "windows":windows.iter().map(|(window,snapshot)|json!({"window":window.0.to_string(),
                "ready":snapshot.is_some(),"revision":snapshot.map(|s|s.revision.to_string())})).collect::<Vec<_>>(),
            "truncated":truncated,"read_only":read_only,"capabilities":if read_only {vec!["snapshot","changes","wait","window_state","input_state"]}else{vec!["snapshot","changes","wait","commands","invoke","input","capture","window_state","input_state"]}})),
        Reply::Snapshot(snapshot) => {
            let Operation::Snapshot {
                root, after, limit, ..
            } = operation
            else {
                return Err("invalid_request");
            };
            vesta_gpui_winit::ui_snapshot_json(
                &snapshot,
                UiQuery {
                    snapshot: snapshot.id,
                    root: *root,
                    after: *after,
                    limit: *limit,
                },
            )
        }
    }
}

pub(crate) fn start(proxy: EventLoopProxy<UserEvent>, control: bool) -> std::io::Result<()> {
    vesta_gpui_winit::start_ui_stdio_with_limit(
        if control { 262144 } else { 8192 },
        move |value, reply| {
            let operation = if control {
                parse_control(&value)
            } else {
                parse(&value)
            }?;
            proxy
                .send_event(UserEvent::UiQuery(Box::new(Request { operation, reply })))
                .map_err(|_| "host_closed")
        },
        |(operation, reply)| serialize(reply, &operation),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_protocol_rejects_mutations_unbounded_pages_and_unpinned_cursors() {
        assert!(parse(&json!({"schema_version":1,"request_id":"a","op":"windows"})).is_ok());
        for value in [
            json!({"schema_version":1,"request_id":"a","op":"invoke"}),
            json!({"schema_version":1,"request_id":"a","op":"windows","command":"click"}),
            json!({"schema_version":1,"request_id":"a","op":"snapshot","instance":"1","window":"1","limit":257}),
            json!({"schema_version":1,"request_id":"a","op":"snapshot","instance":"1","window":"1","after":"2"}),
        ] {
            assert!(parse(&value).is_err());
        }
    }
}
