//! ADR-0577: delayed GPU observations retain the original Host attempt context.
use super::frame_profile::FrameContext;
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use vesta_gpui::{Viewport, render::RenderOutput};
use vesta_gpui_vulkan::{
    GpuProfileKey, GpuProfileObservation, GpuProfileStream, RendererError, WindowRenderer,
};

const MAX_STREAMS: usize = 64;
const MAX_PENDING: usize = 512;
const MAX_RECORDS: u64 = 4096;

#[derive(Clone, Copy)]
pub(super) struct GpuFrameContext {
    pub frame: FrameContext,
    pub viewport: Viewport,
}

struct Attempt {
    key: GpuProfileKey,
    ordinal: u64,
    context: GpuFrameContext,
    outcome: Value,
}

impl Attempt {
    fn json(&self) -> Value {
        let context = self.context;
        json!({
            "host_attempt": self.ordinal.to_string(),
            "window": context.frame.window.0.to_string(),
            "mode": if context.frame.gallery { "2d" } else { "3d" },
            "text_focus": context.frame.text_focused,
            "active": context.frame.active,
            "logical_size": [context.viewport.width, context.viewport.height],
            "scale_factor": context.viewport.scale_factor,
            "render_outcome": self.outcome,
        })
    }
}

#[derive(Default)]
struct Ledger {
    pending: VecDeque<Attempt>,
    attempts: u64,
    samples: u64,
    unavailable: u64,
    missing_context: u64,
    evicted: u64,
    records: u64,
    omitted: u64,
}

impl Ledger {
    fn record(&mut self, key: GpuProfileKey, context: GpuFrameContext, outcome: Value) {
        self.attempts += 1;
        if self.pending.len() == MAX_PENDING {
            self.pending.pop_front();
            self.evicted += 1;
        }
        self.pending.push_back(Attempt {
            key,
            ordinal: self.attempts,
            context,
            outcome,
        });
    }

    fn observe(&mut self, observation: GpuProfileObservation) -> Value {
        let (key, mut value) = match observation {
            GpuProfileObservation::Sample(profile) => {
                self.samples += 1;
                (
                    profile.key,
                    json!({
                        "status": "available",
                        "raw_ticks": profile.raw_ticks.map(|tick| tick.to_string()),
                        "valid_bits": profile.valid_bits,
                        "period_ns": profile.period_ns,
                        "elapsed_ns": profile.elapsed.as_nanos().to_string(),
                        "completion_upper_bound_ns": profile.completion_upper_bound.as_nanos().to_string(),
                    }),
                )
            }
            GpuProfileObservation::Unavailable {
                key,
                reason,
                raw_ticks,
            } => {
                self.unavailable += 1;
                (
                    key,
                    json!({
                        "status": "unknown",
                        "reason": format!("{reason:?}"),
                        "raw_ticks": raw_ticks.map(|ticks| ticks.map(|tick| tick.to_string())),
                        "elapsed_ns": null,
                    }),
                )
            }
        };
        let context = self
            .pending
            .iter()
            .position(|attempt| attempt.key == key)
            .and_then(|index| self.pending.remove(index));
        if context.is_none() {
            self.missing_context += 1;
        }
        value["schema"] = json!(1);
        value["generation"] = json!(key.generation.to_string());
        value["submission"] = json!(key.submission.to_string());
        value["context"] = context.as_ref().map_or(Value::Null, Attempt::json);
        value
    }

    fn emit(&mut self, value: Value) {
        if self.records < MAX_RECORDS {
            eprintln!("ui_gpu_profile {value}");
            self.records += 1;
        } else {
            self.omitted += 1;
        }
    }
}

#[derive(Default)]
struct State {
    streams: HashMap<u64, GpuProfileStream>,
    ledger: Ledger,
}

impl State {
    fn attach(&mut self, renderer: &mut WindowRenderer) -> Result<(), RendererError> {
        let Some(stream) = renderer.enable_gpu_profiling(true)? else {
            return Ok(());
        };
        let capabilities = stream.capabilities();
        if self.streams.contains_key(&capabilities.generation) {
            return Ok(());
        }
        if self.streams.len() == MAX_STREAMS {
            renderer.enable_gpu_profiling(false)?;
            return Err(RendererError {
                code: "standalone_gpu_profile_generation_limit",
                stage: "host",
                fault: vesta_gpui_vulkan::RendererFault::Other,
                message: "bounded GPU profile stream limit reached".into(),
                element: None,
            });
        }
        eprintln!(
            "ui_gpu_capabilities {}",
            json!({
                "schema": 1,
                "generation": capabilities.generation.to_string(),
                "valid_bits": capabilities.valid_bits,
                "period_ns": capabilities.period_ns,
                "unavailable": capabilities.unavailable.map(|reason| format!("{reason:?}")),
                "scope": "device_upload_and_scene_ui_commands_including_dependency_and_scheduling_stalls",
                "pending_limit": MAX_PENDING,
                "record_limit": MAX_RECORDS,
            })
        );
        self.streams.insert(capabilities.generation, stream);
        Ok(())
    }

    fn drain(&mut self) {
        for stream in self.streams.values() {
            if let Some(observations) = stream.try_drain() {
                for observation in observations {
                    let value = self.ledger.observe(observation);
                    self.ledger.emit(value);
                }
            }
        }
    }
}

/// None keeps normal runs free of GPU queries, telemetry allocation and clocks.
#[derive(Default)]
pub(super) struct GpuProfiler(Option<Box<State>>);

impl GpuProfiler {
    pub fn from_args() -> Self {
        Self(
            std::env::args()
                .any(|argument| argument == "--profile-gpu")
                .then(|| Box::new(State::default())),
        )
    }

    pub fn render(
        &mut self,
        renderer: &mut WindowRenderer,
        context: GpuFrameContext,
        render: impl FnOnce(&mut WindowRenderer) -> Result<Option<RenderOutput>, RendererError>,
    ) -> Result<Option<RenderOutput>, RendererError> {
        let Some(state) = &mut self.0 else {
            return render(renderer);
        };
        state.attach(renderer)?;
        let before = renderer.gpu_profile_key();
        let output = render(renderer);
        if let Some(after) = renderer
            .gpu_profile_key()
            .filter(|key| Some(*key) != before)
        {
            let outcome = match &output {
                Ok(Some(_)) => json!({ "status": "presented" }),
                Ok(None) => json!({ "status": "deferred" }),
                Err(error) => json!({ "status": "failed", "code": error.code }),
            };
            state.ledger.record(after, context, outcome);
        }
        // The current call may publish an earlier submission. Associate first,
        // then consume; a synchronous finish may also publish this very attempt.
        state.drain();
        output
    }

    /// Called after the existing shutdown retirement observation, even on error.
    /// This never waits or schedules work to obtain missing telemetry.
    pub fn finish(&mut self, retirement_pending: bool, retirement_error: Option<&str>) {
        let Some(mut state) = self.0.take() else {
            return;
        };
        state.drain();
        for attempt in &state.ledger.pending {
            eprintln!(
                "ui_gpu_unresolved {}",
                json!({
                    "schema": 1,
                    "generation": attempt.key.generation.to_string(),
                    "submission": attempt.key.submission.to_string(),
                    "context": attempt.json(),
                    "status": "unknown",
                    "reason": "timestamp_not_published",
                    "elapsed_ns": null,
                })
            );
        }
        let mut streams = state.streams.values().collect::<Vec<_>>();
        streams.sort_by_key(|stream| stream.capabilities().generation);
        eprintln!(
            "ui_gpu_summary {}",
            json!({
                "schema": 1,
                "attempts": state.ledger.attempts,
                "samples": state.ledger.samples,
                "unavailable": state.ledger.unavailable,
                "missing_context": state.ledger.missing_context,
                "context_evictions": state.ledger.evicted,
                "unresolved": state.ledger.pending.len(),
                "records": state.ledger.records,
                "omitted_records": state.ledger.omitted,
                "streams": streams.iter().map(|stream| json!({
                    "generation": stream.capabilities().generation.to_string(),
                    "dropped": stream.dropped_observations(),
                })).collect::<Vec<_>>(),
                "retirement_pending": retirement_pending,
                "retirement_error": retirement_error,
            })
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use vesta_gpui::UiWindowId;
    use vesta_gpui_vulkan::{FrameGpuProfile, GpuTimingUnavailable};

    #[test]
    fn delayed_gpu_results_keep_original_context_and_explicit_unknowns() {
        let key = |generation, submission| GpuProfileKey {
            generation,
            submission,
        };
        let context = |window, active| GpuFrameContext {
            frame: FrameContext {
                window: UiWindowId(window),
                gallery: true,
                text_focused: false,
                active,
            },
            viewport: Viewport {
                width: 640.0,
                height: 480.0,
                scale_factor: 1.5,
            },
        };
        let mut ledger = Ledger::default();
        ledger.record(
            key(1, 1),
            context(7, false),
            json!({"status": "failed", "code": "present_out_of_date"}),
        );
        ledger.record(key(2, 1), context(8, true), json!({"status": "presented"}));
        let delayed = ledger.observe(GpuProfileObservation::Sample(FrameGpuProfile {
            key: key(1, 1),
            raw_ticks: [u64::MAX - 1, 1],
            valid_bits: 64,
            period_ns: 1.0,
            elapsed: Duration::from_nanos(3),
            completion_upper_bound: Duration::from_millis(1),
        }));
        assert_eq!(delayed["context"]["window"], "7");
        assert_eq!(delayed["context"]["active"], false);
        assert_eq!(delayed["context"]["render_outcome"]["status"], "failed");
        assert_eq!(delayed["raw_ticks"][0], (u64::MAX - 1).to_string());
        assert_eq!(delayed["elapsed_ns"], "3");
        let unknown = ledger.observe(GpuProfileObservation::Unavailable {
            key: key(2, 1),
            reason: GpuTimingUnavailable::ResultsUnavailable,
            raw_ticks: None,
        });
        assert_eq!(unknown["context"]["window"], "8");
        assert_eq!(unknown["status"], "unknown");
        assert_eq!(unknown["elapsed_ns"], Value::Null);
        assert_eq!(unknown["raw_ticks"], Value::Null);
        for submission in 1..=MAX_PENDING as u64 + 1 {
            ledger.record(
                key(3, submission),
                context(9, true),
                json!({"status": "presented"}),
            );
        }
        assert_eq!(ledger.pending.len(), MAX_PENDING);
        assert_eq!(ledger.evicted, 1);
        let missing = ledger.observe(GpuProfileObservation::Unavailable {
            key: key(3, 1),
            reason: GpuTimingUnavailable::ResultsUnavailable,
            raw_ticks: None,
        });
        assert_eq!(missing["context"], Value::Null);
        assert_eq!(ledger.missing_context, 1);
        assert_eq!(ledger.samples, 1);
        assert_eq!(ledger.unavailable, 2);
        assert_eq!(
            ledger.pending.front().map(|attempt| attempt.key),
            Some(key(3, 2))
        );
    }
}
