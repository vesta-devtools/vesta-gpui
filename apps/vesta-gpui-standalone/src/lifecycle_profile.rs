//! ADR-0314: opt-in Host wakeups and native-receipt to present-return observations.
use super::*;
use serde_json::{Value, json};
use std::collections::VecDeque;
use winit::event::StartCause;

const MAX_WINDOWS: usize = 64;
const MAX_PENDING: usize = 64;
const MAX_INPUT_RECORDS: u64 = 256;

#[derive(Default)]
struct WindowCounts {
    native_events: u64,
    redraw_deliveries: u64,
    presents: u64,
}

struct PendingInput {
    sequence: u64,
    window: UiWindowId,
    kind: &'static str,
    received: Instant,
}

struct CompletedInput {
    ordinal: u64,
    input: PendingInput,
    presented: Instant,
    present_index: u64,
    frame_watermark: u64,
}

struct State {
    origin: Instant,
    windows: HashMap<UiWindowId, WindowCounts>,
    batches: u64,
    timer_wakeups: u64,
    cancelled_waits: u64,
    polls: u64,
    waits: u64,
    wait_until: Option<Instant>,
    input_sequence: u64,
    pending: VecDeque<PendingInput>,
    evicted: u64,
    completed: u64,
    omitted_windows: u64,
}

impl State {
    fn new(now: Instant) -> Self {
        Self {
            origin: now,
            windows: HashMap::new(),
            batches: 0,
            timer_wakeups: 0,
            cancelled_waits: 0,
            polls: 0,
            waits: 0,
            wait_until: None,
            input_sequence: 0,
            pending: VecDeque::new(),
            evicted: 0,
            completed: 0,
            omitted_windows: 0,
        }
    }

    fn window(&mut self, window: UiWindowId) -> Option<&mut WindowCounts> {
        if !self.windows.contains_key(&window) && self.windows.len() == MAX_WINDOWS {
            self.omitted_windows += 1;
            return None;
        }
        Some(self.windows.entry(window).or_default())
    }

    fn receive(&mut self, window: UiWindowId, kind: &'static str, now: Instant) -> u64 {
        self.input_sequence += 1;
        if self.pending.len() == MAX_PENDING {
            self.pending.pop_front();
            self.evicted += 1;
        }
        self.pending.push_back(PendingInput {
            sequence: self.input_sequence,
            window,
            kind,
            received: now,
        });
        self.input_sequence
    }

    fn present(&mut self, window: UiWindowId, watermark: u64, now: Instant) -> Vec<CompletedInput> {
        let Some(counts) = self.window(window) else {
            return Vec::new();
        };
        counts.presents += 1;
        let present_index = counts.presents;
        let mut completed = Vec::new();
        let mut index = 0;
        while index < self.pending.len() {
            if self.pending[index].window == window && self.pending[index].sequence <= watermark {
                let input = self
                    .pending
                    .remove(index)
                    .expect("checked pending input index");
                self.completed += 1;
                completed.push(CompletedInput {
                    ordinal: self.completed,
                    input,
                    presented: now,
                    present_index,
                    frame_watermark: watermark,
                });
            } else {
                index += 1;
            }
        }
        completed
    }

    fn elapsed_ns(&self, now: Instant) -> String {
        now.saturating_duration_since(self.origin)
            .as_nanos()
            .to_string()
    }

    fn snapshot(&self, now: Instant) -> Value {
        let mut windows = self.windows.iter().collect::<Vec<_>>();
        windows.sort_by_key(|(id, _)| id.0);
        json!({
            "schema": 1,
            "elapsed_ns": self.elapsed_ns(now),
            "event_loop_batches": self.batches,
            "timer_wakeups": self.timer_wakeups,
            "cancelled_waits": self.cancelled_waits,
            "poll_batches": self.polls,
            "about_to_wait": self.waits,
            "wait_until_ns": self.wait_until.map(|at| self.elapsed_ns(at)),
            "windows": windows.into_iter().map(|(id, counts)| json!({
                "window": id.0.to_string(),
                "native_events": counts.native_events,
                "redraw_deliveries": counts.redraw_deliveries,
                "successful_present_calls": counts.presents,
            })).collect::<Vec<_>>(),
            "native_input_received": self.input_sequence,
            "native_input_completed": self.completed,
            "native_input_pending": self.pending.len(),
            "native_input_evicted": self.evicted,
            "omitted_receipt_logs": self.input_sequence.saturating_sub(MAX_INPUT_RECORDS),
            "omitted_completion_logs": self.completed.saturating_sub(MAX_INPUT_RECORDS),
            "omitted_window_events": self.omitted_windows,
            "boundary": "winit native receipt to first applicable successful present call return; not GPU completion or physical display",
        })
    }
}

#[derive(Default)]
pub(super) struct LifecycleProfiler(Option<Box<State>>);

impl LifecycleProfiler {
    pub fn from_args() -> Self {
        Self(
            std::env::args()
                .any(|argument| argument == "--profile-ui")
                .then(|| Box::new(State::new(Instant::now()))),
        )
    }

    pub fn new_events(&mut self, cause: StartCause) {
        if let Some(state) = &mut self.0 {
            state.batches += 1;
            match cause {
                StartCause::ResumeTimeReached { .. } => state.timer_wakeups += 1,
                StartCause::WaitCancelled { .. } => state.cancelled_waits += 1,
                StartCause::Poll => state.polls += 1,
                StartCause::Init => (),
            }
        }
    }

    pub fn waiting(&mut self, until: Option<Instant>) {
        if let Some(state) = &mut self.0 {
            state.waits += 1;
            state.wait_until = until;
        }
    }

    pub fn native_event(&mut self, window: UiWindowId, event: &NativeWindowEvent) {
        let Some(state) = &mut self.0 else {
            return;
        };
        if let Some(counts) = state.window(window) {
            counts.native_events += 1;
            counts.redraw_deliveries +=
                u64::from(matches!(event, NativeWindowEvent::RedrawRequested));
        }
        let kind = match event {
            NativeWindowEvent::MouseInput {
                state: ElementState::Pressed,
                ..
            } => "pointer_press",
            NativeWindowEvent::MouseInput {
                state: ElementState::Released,
                ..
            } => "pointer_release",
            NativeWindowEvent::MouseWheel { .. } => "wheel",
            NativeWindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } if event.state == ElementState::Pressed => "key_press",
            _ => return,
        };
        let now = Instant::now();
        let sequence = state.receive(window, kind, now);
        if sequence <= MAX_INPUT_RECORDS {
            eprintln!(
                "ui_native_input_received {}",
                json!({
                    "schema": 1, "sequence": sequence.to_string(), "window": window.0.to_string(),
                    "kind": kind, "received_ns": state.elapsed_ns(now), "origin": "winit_native",
                })
            );
        }
    }

    /// Snapshot before building the frame; later input cannot be credited to it.
    pub fn begin_frame(&self) -> u64 {
        self.0.as_ref().map_or(0, |state| state.input_sequence)
    }

    pub fn presented(&mut self, window: UiWindowId, watermark: u64) {
        let Some(state) = &mut self.0 else {
            return;
        };
        let now = Instant::now();
        for completion in state.present(window, watermark, now) {
            if completion.ordinal <= MAX_INPUT_RECORDS {
                eprintln!(
                    "ui_native_input_presented {}",
                    json!({
                        "schema": 1, "sequence": completion.input.sequence.to_string(),
                        "window": completion.input.window.0.to_string(), "kind": completion.input.kind,
                        "received_ns": state.elapsed_ns(completion.input.received),
                        "present_return_ns": state.elapsed_ns(completion.presented),
                        "receipt_to_present_return_ns": completion.presented.saturating_duration_since(completion.input.received).as_nanos().to_string(),
                        "host_present_index": completion.present_index.to_string(),
                        "frame_input_watermark": completion.frame_watermark.to_string(),
                        "boundary": "present call returned success; display completion unmeasured",
                    })
                );
            }
        }
    }

    pub fn snapshot(&self) -> Option<Value> {
        self.0.as_ref().map(|state| state.snapshot(Instant::now()))
    }
}

impl Drop for LifecycleProfiler {
    fn drop(&mut self) {
        if let Some(snapshot) = self.snapshot() {
            eprintln!("ui_lifecycle_exit {snapshot}");
        }
    }
}

impl App {
    pub(super) fn profile_window_state(&self, window: UiWindowId) -> Option<Value> {
        let mut profile = self.lifecycle_profile.snapshot()?;
        let (caret, busy_visible, tooltip) = if window == STANDALONE_WINDOW {
            (&self.caret_blink, self.busy_visible, &self.tooltip)
        } else {
            let host = self
                .native_windows
                .values()
                .find(|host| host.ui_window == window)?;
            (&host.caret_blink, host.busy_visible, &host.tooltip)
        };
        profile["caret_visible"] = json!(caret.visible());
        profile["caret_deadline_pending"] = json!(caret.deadline().is_some());
        profile["tooltip_deadline_pending"] = json!(tooltip.deadline(Instant::now()).is_some());
        profile["busy"] = json!(self.demo.busy);
        profile["busy_visible"] = json!(busy_visible);
        profile["notice_deadline_pending"] = json!(self.demo.toast_queue.next_deadline().is_some());
        Some(profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_applicable_present_is_window_owned_watermarked_and_bounded() {
        let start = Instant::now();
        let main = UiWindowId(1);
        let float = UiWindowId(2);
        let mut state = State::new(start);
        state.receive(main, "pointer_release", start);
        state.receive(float, "key_press", start);
        let watermark = state.input_sequence;
        state.receive(main, "wheel", start + Duration::from_millis(1));
        // No present callback for a deferred/failed render: its inputs stay pending.
        assert_eq!(state.pending.len(), 3);
        let first = state.present(main, watermark, start + Duration::from_millis(2));
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].input.sequence, 1);
        assert_eq!(first[0].present_index, 1);
        assert_eq!(state.pending.len(), 2);
        assert!(
            state
                .present(main, watermark, start + Duration::from_millis(3))
                .is_empty()
        );
        assert_eq!(
            state.present(float, 3, start + Duration::from_millis(4))[0]
                .input
                .sequence,
            2
        );
        assert_eq!(
            state.present(main, 3, start + Duration::from_millis(5))[0]
                .input
                .sequence,
            3
        );
        assert!(state.pending.is_empty());
        for _ in 0..MAX_PENDING + 1 {
            state.receive(main, "wheel", start);
        }
        assert_eq!(state.pending.len(), MAX_PENDING);
        assert_eq!(state.evicted, 1);
        let disabled = LifecycleProfiler::default();
        assert_eq!(disabled.begin_frame(), 0);
        assert!(disabled.snapshot().is_none());
    }
}
