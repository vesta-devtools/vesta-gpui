//! Opt-in, bounded process-local elapsed-time tracing. No file I/O or worker thread.
//!
//! Start one [`TraceCapture`], run ordinary GPUI work, then export its stopped
//! snapshot. Put [`frame`] at the host frame entry; library stages automatically
//! inherit its frame and parent. Hosts can add optional [`span`] scopes and
//! [`instant`] retry/skip markers. These are wall-clock intervals, not CPU usage,
//! stack samples or GPU execution times. Do not put secrets in static labels.

use std::{
    collections::HashMap,
    fmt,
    marker::PhantomData,
    rc::Rc,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    thread::ThreadId,
    time::{Duration, Instant},
};

mod frames;
pub use frames::{FrameCall, FrameMarker, FrameProfile, FrameTimer};

const MAX_EVENTS: usize = 200_000;
const MAX_DURATION: Duration = Duration::from_secs(300);
static ENABLED: AtomicBool = AtomicBool::new(false);
static DETAILS_ENABLED: AtomicBool = AtomicBool::new(false);
static ACTIVE: Mutex<Option<Arc<Session>>> = Mutex::new(None);

/// Select event granularity without changing event/time budgets. Stage capture
/// omits intrinsic measurements and individual static/styled shaping scopes;
/// their elapsed work remains included in recorded parent stages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TraceDetail {
    Stages,
    #[default]
    Detailed,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // Diagnostics must not introduce a second panic during unwinding.
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Hard event and time budgets. Expiration is checked when observing/recording;
/// it does not schedule a background timer or close the application.
#[derive(Clone, Copy, Debug)]
pub struct TraceConfig {
    pub max_events: usize,
    pub max_duration: Duration,
}

impl Default for TraceConfig {
    fn default() -> Self {
        Self {
            max_events: 100_000,
            max_duration: Duration::from_secs(30),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceError {
    AlreadyRecording,
    InvalidBudget,
}

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::AlreadyRecording => "gpui_trace_already_recording",
            Self::InvalidBudget => {
                "gpui_trace_invalid_budget (1..=200000 events, 0 < duration <= 300s)"
            }
        })
    }
}
impl std::error::Error for TraceError {}

#[derive(Clone, Copy, Debug)]
pub struct TraceStats {
    pub events: usize,
    pub dropped_events: u64,
    pub unfinished_spans: usize,
    pub elapsed: Duration,
    pub expired: bool,
    pub invalid_nesting: u64,
}

/// The owner of a capture. Drop disables recording and discards data without I/O.
pub struct TraceCapture {
    session: Arc<Session>,
}

impl TraceCapture {
    pub fn start(config: TraceConfig) -> Result<Self, TraceError> {
        Self::start_with_detail(config, TraceDetail::Detailed)
    }

    pub fn start_with_detail(config: TraceConfig, detail: TraceDetail) -> Result<Self, TraceError> {
        if config.max_events == 0
            || config.max_events > MAX_EVENTS
            || config.max_duration.is_zero()
            || config.max_duration > MAX_DURATION
        {
            return Err(TraceError::InvalidBudget);
        }
        let mut active = lock(&ACTIVE);
        if active.as_ref().is_some_and(|session| session.is_live()) {
            return Err(TraceError::AlreadyRecording);
        }
        let session = Arc::new(Session {
            origin: Instant::now(),
            config,
            detail,
            state: Mutex::new(State {
                events: Vec::with_capacity(config.max_events.min(4096)),
                threads: HashMap::new(),
                open: HashMap::new(),
                invalid_nesting: 0,
                dropped: 0,
                stopped: None,
            }),
        });
        *active = Some(session.clone());
        DETAILS_ENABLED.store(detail == TraceDetail::Detailed, Ordering::Release);
        ENABLED.store(true, Ordering::Release);
        Ok(Self { session })
    }

    pub fn is_recording(&self) -> bool {
        self.session.is_live()
    }

    pub fn stats(&self) -> TraceStats {
        let mut state = lock(&self.session.state);
        self.session.expire(&mut state);
        self.session.stats_for(&state)
    }

    /// Stop and freeze a consistent prefix. Active scopes are clipped and marked
    /// unfinished; dropping them later cannot mutate this snapshot.
    pub fn stop(self) -> TraceSnapshot {
        self.disable();
        let mut state = lock(&self.session.state);
        self.session.expire(&mut state);
        let end = self.session.now().min(self.session.config.max_duration);
        state.close(end);
        TraceSnapshot {
            process_id: std::process::id(),
            config: self.session.config,
            detail: self.session.detail,
            stats: self.session.stats_for(&state),
            events: std::mem::take(&mut state.events),
        }
    }

    fn disable(&self) {
        let mut active = lock(&ACTIVE);
        if active
            .as_ref()
            .is_some_and(|s| Arc::ptr_eq(s, &self.session))
        {
            ENABLED.store(false, Ordering::Release);
            DETAILS_ENABLED.store(false, Ordering::Release);
            *active = None;
        }
    }
}

impl Drop for TraceCapture {
    fn drop(&mut self) {
        self.disable();
        let mut state = lock(&self.session.state);
        state.close(self.session.now().min(self.session.config.max_duration));
        // Guards can outlive the owner. They retain only the stopped shell, not
        // an abandoned event buffer or thread map. stop() has already moved any
        // requested snapshot out before this runs.
        state.events = Vec::new();
        state.threads = HashMap::new();
        state.open = HashMap::new();
    }
}

struct Session {
    origin: Instant,
    config: TraceConfig,
    detail: TraceDetail,
    state: Mutex<State>,
}
struct State {
    events: Vec<Event>,
    threads: HashMap<ThreadId, u32>,
    open: HashMap<u32, Vec<usize>>,
    invalid_nesting: u64,
    dropped: u64,
    stopped: Option<Duration>,
}
struct Event {
    category: &'static str,
    name: &'static str,
    thread: u32,
    at: Duration,
    kind: EventKind,
    parent: Option<usize>,
    frame: Option<usize>,
    is_frame: bool,
}
enum EventKind {
    Span {
        end: Option<Duration>,
        unfinished: bool,
    },
    Instant,
    Counter(f64),
}

impl State {
    fn close(&mut self, at: Duration) {
        if self.stopped.is_some() {
            return;
        }
        self.stopped = Some(at);
        self.open.clear();
        for event in &mut self.events {
            if let EventKind::Span { end, unfinished } = &mut event.kind
                && end.is_none()
            {
                *end = Some(at.max(event.at));
                *unfinished = true;
            }
        }
    }
}

impl Session {
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }
    fn expire(&self, state: &mut State) {
        if self.now() >= self.config.max_duration {
            state.close(self.config.max_duration);
        }
    }
    fn is_live(&self) -> bool {
        let mut state = lock(&self.state);
        self.expire(&mut state);
        state.stopped.is_none()
    }
    fn stats_for(&self, state: &State) -> TraceStats {
        TraceStats {
            events: state.events.len(),
            dropped_events: state.dropped,
            unfinished_spans: state
                .events
                .iter()
                .filter(|e| {
                    matches!(
                        e.kind,
                        EventKind::Span {
                            unfinished: true,
                            ..
                        }
                    )
                })
                .count(),
            elapsed: state
                .stopped
                .unwrap_or_else(|| self.now().min(self.config.max_duration)),
            expired: state.stopped == Some(self.config.max_duration),
            invalid_nesting: state.invalid_nesting,
        }
    }
    fn record(&self, category: &'static str, name: &'static str, kind: EventKind) -> Option<usize> {
        self.record_entry(category, name, kind, false)
    }

    fn record_entry(
        &self,
        category: &'static str,
        name: &'static str,
        kind: EventKind,
        is_frame: bool,
    ) -> Option<usize> {
        let mut state = lock(&self.state);
        self.expire(&mut state);
        if state.stopped.is_some() {
            return None;
        }
        if state.events.len() == self.config.max_events
            || !valid_label(category, name)
            || matches!(kind, EventKind::Counter(value) if !value.is_finite())
        {
            state.dropped = state.dropped.saturating_add(1);
            return None;
        }
        let next = state.threads.len() as u32 + 1;
        let thread = *state
            .threads
            .entry(std::thread::current().id())
            .or_insert(next);
        let index = state.events.len();
        let parent = if is_frame {
            None
        } else {
            state
                .open
                .get(&thread)
                .and_then(|stack| stack.last())
                .copied()
        };
        let frame = if is_frame {
            Some(index)
        } else {
            parent
                .and_then(|index| state.events.get(index))
                .and_then(|event| event.frame)
        };
        if matches!(kind, EventKind::Span { .. }) {
            state.open.entry(thread).or_default().push(index);
        }
        state.events.push(Event {
            category,
            name,
            thread,
            at: self.now().min(self.config.max_duration),
            kind,
            parent,
            frame,
            is_frame,
        });
        Some(index)
    }
}

fn valid_label(category: &str, name: &str) -> bool {
    !category.is_empty() && category.len() <= 64 && !name.is_empty() && name.len() <= 128
}

fn active() -> Option<Arc<Session>> {
    if !ENABLED.load(Ordering::Acquire) {
        return None;
    }
    let mut active = lock(&ACTIVE);
    let session = active.as_ref()?;
    if !session.is_live() {
        ENABLED.store(false, Ordering::Release);
        DETAILS_ENABLED.store(false, Ordering::Release);
        *active = None;
        return None;
    }
    Some(session.clone())
}

/// True only while an explicitly started capture accepts new events.
pub fn is_recording() -> bool {
    active().is_some()
}

/// One thread-local synchronous scope, automatically closed on early return or
/// unwinding. Intentionally !Send: do not move a scope across threads or await.
#[must_use = "keep the guard alive for the measured scope"]
#[derive(Default)]
pub struct TraceSpan {
    entry: Option<(Arc<Session>, usize)>,
    thread_bound: PhantomData<Rc<()>>,
}

pub fn span(category: &'static str, name: &'static str) -> TraceSpan {
    let entry = active().and_then(|session| {
        let index = session.record(
            category,
            name,
            EventKind::Span {
                end: None,
                unfinished: false,
            },
        )?;
        Some((session, index))
    });
    TraceSpan {
        entry,
        thread_bound: PhantomData,
    }
}

/// High-frequency framework detail. Keep separate from ordinary host spans so
/// stage capture does not hide host work or require hosts to maintain name lists.
pub(crate) fn detail_span(category: &'static str, name: &'static str) -> TraceSpan {
    if !DETAILS_ENABLED.load(Ordering::Acquire) {
        return TraceSpan::default();
    }
    let entry = active()
        .filter(|session| session.detail == TraceDetail::Detailed)
        .and_then(|session| {
            let index = session.record(
                category,
                name,
                EventKind::Span {
                    end: None,
                    unfinished: false,
                },
            )?;
            Some((session, index))
        });
    TraceSpan {
        entry,
        thread_bound: PhantomData,
    }
}

/// One CPU frame attempt. Put this once at the host's existing frame entry;
/// shared GPUI calls automatically inherit this frame and their parent scope.
/// Ending this guard does not assert GPU completion or physical presentation.
pub fn frame(name: &'static str) -> TraceSpan {
    let entry = active().and_then(|session| {
        let index = session.record_entry(
            "gpui.frame",
            name,
            EventKind::Span {
                end: None,
                unfinished: false,
            },
            true,
        )?;
        Some((session, index))
    });
    TraceSpan {
        entry,
        thread_bound: PhantomData,
    }
}

impl TraceSpan {
    /// Close a measured interval using the static label known at its end. Useful
    /// for renderer phase boundaries; original capture ownership is unchanged.
    pub fn finish_with_name(mut self, name: &'static str) {
        if let Some((session, index)) = &mut self.entry {
            let mut state = lock(&session.state);
            session.expire(&mut state);
            if state.stopped.is_none()
                && let Some(event) = state.events.get_mut(*index)
                && valid_label(event.category, name)
            {
                event.name = name;
            }
        }
    }
}

impl Drop for TraceSpan {
    fn drop(&mut self) {
        let Some((session, index)) = &self.entry else {
            return;
        };
        let mut state = lock(&session.state);
        session.expire(&mut state);
        if state.stopped.is_none()
            && let Some(event) = state.events.get_mut(*index)
            && let EventKind::Span { end, .. } = &mut event.kind
        {
            *end = Some(session.now().min(session.config.max_duration).max(event.at));
            let thread = event.thread;
            if let Some(stack) = state.open.get_mut(&thread) {
                if stack.last() == Some(index) {
                    stack.pop();
                } else if let Some(position) = stack.iter().position(|entry| entry == index) {
                    stack.remove(position);
                    state.invalid_nesting = state.invalid_nesting.saturating_add(1);
                }
            }
        }
    }
}

/// Thread-scoped marker, for example a host deciding to retry layout before render.
pub fn instant(category: &'static str, name: &'static str) {
    if let Some(session) = active() {
        session.record(category, name, EventKind::Instant);
    }
}

/// Finite numeric observation. No application strings or arbitrary JSON payloads.
pub fn counter(category: &'static str, name: &'static str, value: f64) {
    if let Some(session) = active() {
        session.record(category, name, EventKind::Counter(value));
    }
}

pub struct TraceSnapshot {
    process_id: u32,
    config: TraceConfig,
    detail: TraceDetail,
    stats: TraceStats,
    events: Vec<Event>,
}

impl TraceSnapshot {
    pub fn stats(&self) -> TraceStats {
        self.stats
    }

    /// Chrome Trace Event JSON. Only caller-initiated export allocates JSON; the
    /// caller owns file I/O and should keep large exports off the UI thread.
    pub fn to_chrome_json(&self) -> Result<String, serde_json::Error> {
        use serde_json::json;
        let micros = |d: Duration| d.as_secs_f64() * 1_000_000.0;
        let mut events = Vec::with_capacity(self.events.len());
        for (index, event) in self.events.iter().enumerate() {
            let mut item = json!({ "cat":event.category,"name":event.name,"pid":self.process_id,
                "tid":event.thread,"ts":micros(event.at) });
            let mut args = json!({"event_id":index as u64 + 1,
                "parent_id":event.parent.map(|id| id as u64+1),
                "frame_id":event.frame.map(|id| id as u64+1),"frame":event.is_frame});
            match event.kind {
                EventKind::Span { end, unfinished } => {
                    item["ph"] = json!("X");
                    item["dur"] = json!(micros(
                        end.unwrap_or(self.stats.elapsed).saturating_sub(event.at)
                    ));
                    args["unfinished"] = json!(unfinished);
                }
                EventKind::Instant => {
                    item["ph"] = json!("i");
                    item["s"] = json!("t");
                }
                EventKind::Counter(value) => {
                    item["ph"] = json!("C");
                    args["value"] = json!(value);
                }
            }
            item["args"] = args;
            events.push(item);
        }
        let frames: Vec<_> = self.frames().into_iter().map(|frame| json!({
            "ordinal":frame.ordinal,"frame_id":frame.frame_id,"name":frame.name,"thread":frame.thread,
            "start_us":micros(frame.start),"duration_us":micros(frame.duration),"unfinished":frame.unfinished,
            "calls":frame.calls.into_iter().map(|call| json!({"event_id":call.event_id,"parent_id":call.parent_id,
                "thread":call.thread,"category":call.category,"name":call.name,"start_us":micros(call.start),
                "inclusive_us":micros(call.inclusive),"self_us":micros(call.self_time),"unfinished":call.unfinished})).collect::<Vec<_>>(),
            "timers":frame.timers.into_iter().map(|timer| json!({"category":timer.category,"name":timer.name,
                "calls":timer.calls,"inclusive_us":micros(timer.inclusive),"self_us":micros(timer.self_time)})).collect::<Vec<_>>(),
            "markers":frame.markers.into_iter().map(|marker| json!({"name":marker.name,"at_us":micros(marker.at),"value":marker.value})).collect::<Vec<_>>()
        })).collect();
        serde_json::to_string(&json!({ "traceEvents":events,"displayTimeUnit":"ms",
            "gpui_frames":frames,
            "gpui_trace": { "schema_version":2,"clock":"monotonic_elapsed_wall_time",
                "detail":match self.detail {TraceDetail::Stages=>"stages",TraceDetail::Detailed=>"detailed"},
                "thread_ids":"capture_local","gpu_execution_timing":false,
                "max_events":self.config.max_events,"max_duration_ms":self.config.max_duration.as_millis(),
                "elapsed_us":micros(self.stats.elapsed),"events":self.stats.events,
                "dropped_events":self.stats.dropped_events,"unfinished_spans":self.stats.unfinished_spans,
                "invalid_nesting":self.stats.invalid_nesting,
                "unframed_events":self.events.iter().filter(|event| event.frame.is_none()).count(),
                "expired":self.stats.expired } }))
    }

    /// Self-contained offline frame picker and recorded-call report. Export
    /// outside the live UI loop. The viewer does not compute a second analysis.
    pub fn to_html_report(&self) -> Result<String, serde_json::Error> {
        let data = self
            .to_chrome_json()?
            .replace('&', "\\u0026")
            .replace('<', "\\u003c");
        Ok(include_str!("trace/report.html").replacen("__GPUI_TRACE_DATA__", &data, 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abandoned_capture_releases_storage_while_a_late_scope_keeps_its_shell() {
        let session = Arc::new(Session {
            origin: Instant::now(),
            config: TraceConfig::default(),
            detail: TraceDetail::Detailed,
            state: Mutex::new(State {
                events: Vec::with_capacity(4096),
                threads: HashMap::new(),
                open: HashMap::new(),
                invalid_nesting: 0,
                dropped: 0,
                stopped: None,
            }),
        });
        let index = session
            .record(
                "host",
                "late",
                EventKind::Span {
                    end: None,
                    unfinished: false,
                },
            )
            .expect("scope recorded");
        let scope = TraceSpan {
            entry: Some((session.clone(), index)),
            thread_bound: PhantomData,
        };
        drop(TraceCapture {
            session: session.clone(),
        });
        {
            let state = lock(&session.state);
            assert!(state.stopped.is_some());
            assert_eq!(
                state.events.capacity(),
                0,
                "late guard must not retain abandoned storage"
            );
            assert_eq!(state.threads.capacity(), 0);
        }
        drop(scope);
        assert!(lock(&session.state).events.is_empty());
    }
}
