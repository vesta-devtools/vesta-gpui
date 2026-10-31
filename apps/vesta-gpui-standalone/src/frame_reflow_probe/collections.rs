//! Bounded native collection work followed by the ordinary idle policy.
use super::*;
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
};

const WARM: usize = 64;
const SAMPLES: usize = 128;
const IDLE: Duration = Duration::from_secs(4);

enum Phase {
    Frames,
    Settling(Instant),
    Idle { until: Instant, before: Value },
    Done,
}

pub(super) struct Probe {
    floating: FloatingWindowId,
    items: [ItemId; 2],
    mode: &'static str,
    step: usize,
    aligned: [bool; 2],
    pending: Option<(Instant, f64, usize)>,
    phase: Phase,
    samples: Vec<Value>,
}

impl Probe {
    pub(super) fn new(
        floating: FloatingWindowId,
        items: [ItemId; 2],
        detail: Option<vesta_gpui::trace::TraceDetail>,
    ) -> Self {
        Self {
            floating,
            items,
            mode: match detail {
                None => "off",
                Some(vesta_gpui::trace::TraceDetail::Stages) => "stages",
                Some(vesta_gpui::trace::TraceDetail::Detailed) => "detailed",
            },
            step: 0,
            aligned: [false; 2],
            pending: None,
            phase: Phase::Frames,
            samples: Vec::with_capacity(SAMPLES),
        }
    }
    pub(super) fn active_driver(&self) -> bool {
        matches!(self.phase, Phase::Frames)
    }
    pub(super) fn passive_deadline(&self) -> Option<Instant> {
        match self.phase {
            Phase::Settling(until) | Phase::Idle { until, .. } => Some(until),
            _ => None,
        }
    }

    pub(super) fn advance(
        &mut self,
        app: &mut App,
        event_loop: &ActiveEventLoop,
    ) -> Result<bool, String> {
        match &self.phase {
            Phase::Done => return Ok(true),
            Phase::Settling(until) => {
                if Instant::now() < *until {
                    return Ok(false);
                }
                if app
                    .gpu
                    .as_ref()
                    .and_then(GpuState::frame_retry_deadline)
                    .is_some()
                    || app
                        .native_windows
                        .values()
                        .any(|host| host.surface.frame_retry_deadline().is_some())
                {
                    self.phase = Phase::Settling(Instant::now() + Duration::from_millis(50));
                    return Ok(false);
                }
                let before = app
                    .lifecycle_profile
                    .snapshot()
                    .ok_or("collection_profile_missing_lifecycle")?;
                eprintln!(
                    "native_collections_idle_begin {}",
                    json!({"schema_version":1,"mode":self.mode,"idle_ms":IDLE.as_millis(),"lifecycle":before})
                );
                self.phase = Phase::Idle {
                    until: Instant::now() + IDLE,
                    before,
                };
                return Ok(false);
            }
            Phase::Idle { until, before } => {
                if Instant::now() < *until {
                    return Ok(false);
                }
                let after = app
                    .lifecycle_profile
                    .snapshot()
                    .ok_or("collection_profile_missing_lifecycle")?;
                let presents = |v: &Value| {
                    v["windows"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|w| w["successful_present_calls"].as_u64().unwrap_or(0))
                        .sum::<u64>()
                };
                let redraws = |v: &Value| {
                    v["windows"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|w| w["redraw_deliveries"].as_u64().unwrap_or(0))
                        .sum::<u64>()
                };
                let checks = json!({"no_idle_present":presents(before)==presents(&after),"no_idle_redraw":redraws(before)==redraws(&after),
                    "no_periodic_timer":after["timer_wakeups"].as_u64().unwrap_or(u64::MAX).saturating_sub(before["timer_wakeups"].as_u64().unwrap_or(0))<=1,
                    "no_poll_batches":before["poll_batches"]==after["poll_batches"],"no_input":before["native_input_received"]==after["native_input_received"]});
                eprintln!(
                    "native_collections_profile {}",
                    json!({"schema_version":1,"mode":self.mode,"model_items":app.demo.virtual_ids.len(),
                    "warm_submissions":WARM,"measured_submissions":self.samples.len(),"samples":self.samples,"idle_before":before,"idle_after":after,"checks":checks,
                    "physical_input":false,"gpu_or_display_latency":false,"frame_timing_excludes_snapshot_validation":true})
                );
                if !checks
                    .as_object()
                    .is_some_and(|checks| checks.values().all(|v| v.as_bool() == Some(true)))
                {
                    return Err("collection_profile_ordinary_idle_failed".into());
                }
                self.phase = Phase::Done;
                return Ok(true);
            }
            Phase::Frames => (),
        }
        if app.demo.virtual_ids.len() != 100_000 {
            return Err("collection_profile_model_size_changed".into());
        }
        let Some((id, available)) = app
            .native_windows
            .iter()
            .find(|(_, host)| host.logical_id == self.floating)
            .map(|(id, host)| (*id, host.surface.available()))
        else {
            return Ok(false);
        };
        if !available || app.gpu.as_ref().is_none_or(|gpu| !gpu.available()) {
            return Ok(false);
        }
        if self.step == WARM + SAMPLES {
            self.phase = Phase::Settling(Instant::now() + Duration::from_millis(500));
            return Ok(false);
        }
        let source = self.step % 2;
        let (owner, gallery, hidden) = if source == 0 {
            (
                STANDALONE_WINDOW,
                app.scopes.gallery,
                app.window.as_ref().is_some_and(|window| {
                    window.is_visible() == Some(false) && !window.has_focus()
                }),
            )
        } else {
            let host = app
                .native_windows
                .get(&id)
                .ok_or("collection_profile_float_lost")?;
            (
                host.ui_window,
                host.scopes.gallery,
                host.window.is_visible() == Some(false) && !host.window.has_focus(),
            )
        };
        if !hidden {
            return Err("collection_profile_requires_hidden_nonactive_windows".into());
        }
        let index = [0, 10_000, 50_000, 99_990, 0][(self.step / 2) % 5];
        if self.pending.is_none() {
            app.demo.virtual_scroll = ScrollAreaState::new(index as f32 * VIRTUAL_ROW_HEIGHT);
            self.pending = Some((Instant::now(), 0.0, 0));
        }
        app.frame_meter.reset();
        let before = app.ui_query.next(owner);
        let start = Instant::now();
        if source == 0 {
            app.redraw()
        } else {
            app.redraw_native_window(id)
        }
        let pending = self
            .pending
            .as_mut()
            .ok_or("collection_profile_missing_pending")?;
        pending.1 += start.elapsed().as_secs_f64() * 1e6;
        pending.2 += 1;
        let after = app.ui_query.next(owner);
        if after.is_none() || after == before {
            return Ok(false);
        }
        let stamp = after.ok_or("collection_profile_missing_snapshot")?;
        let (reply, receiver) = std::sync::mpsc::sync_channel(1);
        app.answer_ui_query(
            event_loop,
            ui_query::Request {
                operation: ui_query::Operation::Snapshot {
                    instance: stamp.instance,
                    window: owner,
                    revision: None,
                    root: None,
                    after: None,
                    limit: 4096,
                },
                reply,
            },
        );
        let (_, ui_query::Reply::Snapshot(snapshot)) = receiver
            .try_recv()
            .map_err(|_| "collection_profile_missing_reply")??
        else {
            return Err("collection_profile_unexpected_reply".into());
        };
        if snapshot.node_limit_reached || snapshot.depth_limit_reached {
            return Err("collection_profile_snapshot_truncated".into());
        }
        let virtual_root = gallery.element(VIRTUAL_SCROLL);
        let node = snapshot
            .nodes()
            .iter()
            .find(|node| node.id == virtual_root)
            .ok_or("collection_profile_virtual_root_missing")?;
        if !self.aligned[source] {
            let y = node
                .bounds
                .ok_or("collection_profile_missing_virtual_geometry")?
                .y;
            let old = app
                .demo
                .page_scroll
                .get(&self.items[source])
                .map_or(0.0, |state| state.offset_y());
            app.demo.page_scroll.insert(
                self.items[source],
                ScrollAreaState::new((old + y - 120.0).max(0.0)),
            );
            self.aligned[source] = true;
            self.pending = None;
            return Ok(false);
        }
        if node
            .visible_bounds
            .is_none_or(|bounds| bounds.height < 80.0)
            || node
                .scroll_content
                .is_none_or(|(_, height)| height < 1_000_000.0)
        {
            return Err("collection_profile_virtual_content_not_visible".into());
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let mut descendants = HashSet::from([virtual_root]);
        let mut subtree_nodes = 0;
        for node in snapshot.nodes() {
            if node.id != virtual_root
                && node
                    .parent
                    .is_none_or(|parent| !descendants.contains(&parent))
            {
                continue;
            }
            descendants.insert(node.id);
            subtree_nodes += 1;
            node.id.hash(&mut hasher);
            node.name.value.hash(&mut hasher);
            node.text.value.hash(&mut hasher);
            for bounds in [node.bounds, node.visible_bounds] {
                bounds
                    .map(|r| {
                        [
                            r.x.to_bits(),
                            r.y.to_bits(),
                            r.width.to_bits(),
                            r.height.to_bits(),
                        ]
                    })
                    .hash(&mut hasher)
            }
        }
        let (began, call_us, attempts) = self
            .pending
            .take()
            .ok_or("collection_profile_missing_pending")?;
        if self.step >= WARM {
            self.samples.push(json!({"step":self.step-WARM,"source":source,"index":index,"nodes":snapshot.nodes().len(),"virtual_nodes":subtree_nodes,
                "work_signature":format!("{:016x}",hasher.finish()),"call_wall_us":call_us,"request_wall_us":began.elapsed().as_secs_f64()*1e6,"attempts":attempts}));
        }
        self.step += 1;
        Ok(false)
    }
}
