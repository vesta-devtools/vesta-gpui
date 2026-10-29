//! Version 1 projection shared by participating native Hosts; no private Action payloads.
use serde_json::{Value, json};
use vesta_gpui::{Rect, UiNode, UiQuery, UiSnapshot, UiSnapshotText};
fn text(value: &UiSnapshotText) -> Value {
    json!({"value":value.value,"truncated":value.truncated,"redacted":value.redacted})
}
fn rect(value: Option<Rect>) -> Value {
    value.map_or(
        Value::Null,
        |r| json!({"x":r.x,"y":r.y,"width":r.width,"height":r.height}),
    )
}
fn node(node: &UiNode) -> Value {
    json!({"id":node.id.get().to_string(),"parent":node.parent.map(|id|id.get().to_string()),
        "role":node.role.map(|role|format!("{role:?}")),"name":text(&node.name),
        "description":text(&node.description),"text":text(&node.text),"value":text(&node.value),
        "tooltip":text(&node.tooltip),"bounds":rect(node.bounds),"visible_bounds":rect(node.visible_bounds),
        "selected":node.selected,"expanded":node.expanded,"toggle":node.toggle.map(|v|format!("{v:?}")),
        "numeric":node.numeric.map(|v|json!({"value":v.value,"min":v.min,"max":v.max,"step":v.step})),
        "read_only":node.read_only,"busy":node.busy,"composing":node.composing,"focused":node.focused,
        "selection":node.selection.map(|(anchor,focus)|json!({"anchor_byte":anchor,"focus_byte":focus})),
        "scroll_offset":node.scroll_offset.map(|p|json!({"x":p.x,"y":p.y})),"scroll_content":node.scroll_content,
        "action":{"availability":format!("{:?}",node.action),"owner":node.action_owner.map(|id|id.get().to_string()),"invokable":false}})
}
pub fn ui_snapshot_json(snapshot: &UiSnapshot, query: UiQuery) -> Result<Value, &'static str> {
    let page = snapshot.query(query).map_err(|error| match error {
        vesta_gpui::UiQueryError::InvalidCursor => "invalid_cursor",
        vesta_gpui::UiQueryError::NodeNotCaptured => "node_not_captured",
        vesta_gpui::UiQueryError::ForeignWindow => "foreign_window",
        vesta_gpui::UiQueryError::StaleSnapshot => "stale_snapshot",
        vesta_gpui::UiQueryError::InvalidLimits => "invalid_limits",
        _ => "invalid_query",
    })?;
    Ok(
        json!({"instance":snapshot.id.instance.to_string(),"window":snapshot.id.window.0.to_string(),
        "revision":snapshot.id.revision.to_string(),"presentation":"last_presented",
        "viewport":{"width":snapshot.viewport.width,"height":snapshot.viewport.height,"scale_factor":snapshot.viewport.scale_factor},
        "focus":snapshot.focus.map(|id|id.get().to_string()),"modal_root":snapshot.modal_root.map(|id|id.get().to_string()),
        "coverage":{"scope":"realized_tree_only","node_limit":snapshot.node_limit_reached,"depth_limit":snapshot.depth_limit_reached,
            "text_truncated":snapshot.text_truncated,"unrealized_model_data":"unknown","markdown_selection":"not_supplied"},
        "nodes":page.nodes.into_iter().map(node).collect::<Vec<_>>(),"next_after":page.next_after.map(|id|id.get().to_string())}),
    )
}

pub fn ui_changes_json(current: &UiSnapshot, previous: &UiSnapshot) -> Result<Value, &'static str> {
    let changes = current
        .changes_since(previous)
        .map_err(|_| "foreign_or_stale_snapshot")?;
    let ids = |nodes: Vec<vesta_gpui::ElementId>| {
        nodes
            .into_iter()
            .map(|id| id.get().to_string())
            .collect::<Vec<_>>()
    };
    Ok(
        json!({"instance":current.id.instance.to_string(),"window":current.id.window.0.to_string(),"revision":current.id.revision.to_string(),"since":previous.id.revision.to_string(),
        "added":ids(changes.added),"changed":ids(changes.changed),"removed_from_realized_tree":ids(changes.removed),"metadata_changed":changes.metadata_changed,"complete":changes.complete,"coverage":"realized_tree_only"}),
    )
}
