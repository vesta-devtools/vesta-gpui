//! Safe, bounded heap-event ownership. The host adapter alone forwards raw allocator calls.
use super::{
    lock,
    threads::{Registry, TraceThread},
};
use serde::{Deserialize, Serialize};
use std::{
    cell::Cell,
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

pub const MAX_STACK_FRAMES: usize = 32;
static ENABLED: AtomicBool = AtomicBool::new(false);
static INSTALLED: AtomicBool = AtomicBool::new(false);
static ACTIVE: Mutex<Option<Arc<Session>>> = Mutex::new(None);
thread_local! {static QUIET:Cell<bool>=const{Cell::new(false)};}

/// Explicit memory budget independent from CPU event count. Baseline is unknown.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryConfig {
    pub max_events: usize,
    pub stack_frames: usize,
}
impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_events: 100_000,
            stack_frames: 32,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryCapture {
    pub schema_version: u32,
    pub process_id: u32,
    pub elapsed_ns: u64,
    pub max_events: usize,
    pub provider: String,
    pub baseline_known: bool,
    pub event_limit_reached: bool,
    pub hook_errors: u64,
    pub stack_frames: usize,
    pub symbols_resolved: bool,
    #[serde(default)]
    pub stack_frame_budget: usize,
    #[serde(default)]
    pub stack_observations_dropped: usize,
    #[serde(default)]
    pub symbol_budget_exhausted: bool,
    pub threads: Vec<TraceThread>,
    pub stacks: Vec<MemoryStack>,
    pub events: Vec<MemoryEvent>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryStack {
    pub id: u64,
    pub potentially_truncated: bool,
    pub frames: Vec<MemoryStackFrame>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryStackFrame {
    pub address: String,
    pub function: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryEvent {
    pub id: u64,
    pub time_ns: u64,
    pub thread: u32,
    pub stack: Option<u64>,
    pub operation: MemoryOperation,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum MemoryOperation {
    Alloc {
        #[serde(with = "address")]
        address: u64,
        size: u64,
    },
    Free {
        #[serde(with = "address")]
        address: u64,
        size: u64,
    },
    Realloc {
        #[serde(with = "address")]
        old_address: u64,
        old_size: u64,
        #[serde(with = "address")]
        address: u64,
        size: u64,
    },
    ReallocFailed {
        #[serde(with = "address")]
        address: u64,
        old_size: u64,
        requested_size: u64,
    },
}
/// Description supplied before the host forwards an allocator operation.
#[derive(Clone, Copy)]
pub enum AllocatorOperation {
    Alloc {
        size: usize,
    },
    Free {
        address: usize,
        size: usize,
    },
    Realloc {
        address: usize,
        old_size: usize,
        size: usize,
    },
}
struct RawEvent {
    at: u64,
    thread: u32,
    operation: MemoryOperation,
    frames: [usize; MAX_STACK_FRAMES],
    count: usize,
}
struct State {
    events: Vec<RawEvent>,
    ended: Option<u64>,
    full: bool,
}
struct Session {
    origin: Instant,
    duration: Duration,
    config: MemoryConfig,
    threads: Arc<Registry>,
    state: Mutex<State>,
    errors: AtomicU64,
}
pub(super) struct Recording(Arc<Session>);
#[derive(Clone, Copy, Debug)]
pub struct MemoryStats {
    pub events: usize,
    pub limit_reached: bool,
    pub hook_errors: u64,
}
pub struct Suppression {
    previous: bool,
    thread_bound: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl Drop for Suppression {
    fn drop(&mut self) {
        let _ = QUIET.try_with(|q| q.set(self.previous));
    }
}
/// Suppress diagnostic-owned allocations while touching recorder internals.
pub fn suppress() -> Option<Suppression> {
    QUIET
        .try_with(|q| Suppression {
            previous: q.replace(true),
            thread_bound: std::marker::PhantomData,
        })
        .ok()
}
pub fn allocator_installed() -> bool {
    INSTALLED.load(Ordering::Acquire)
}
pub fn is_recording() -> bool {
    ENABLED.load(Ordering::Acquire)
}

/// Serialize observed allocator operations (including realloc address reuse).
/// The actual allocation result survives any diagnostic panic; diagnostics never
/// unwind through GlobalAlloc. The adapter's operation must itself never unwind.
pub fn allocator_operation(
    kind: AllocatorOperation,
    perform: impl FnOnce() -> usize,
    stack: impl FnOnce(usize, &mut [usize; MAX_STACK_FRAMES]) -> usize,
) -> usize {
    if !INSTALLED.load(Ordering::Relaxed) {
        INSTALLED.store(true, Ordering::Release);
    }
    if !ENABLED.load(Ordering::Acquire) {
        return perform();
    }
    let Some(_guard) = QUIET
        .try_with(|q| {
            (!q.get()).then(|| Suppression {
                previous: q.replace(true),
                thread_bound: std::marker::PhantomData,
            })
        })
        .ok()
        .flatten()
    else {
        return perform();
    };
    let mut perform = Some(perform);
    let mut result = None;
    let mut owner = None;
    let observed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // Hold the operation gate across System's call, so a concurrent address
        // reuse cannot be recorded before the operation that released it.
        let active = lock(&ACTIVE);
        let session = active.as_ref().filter(|_| ENABLED.load(Ordering::Relaxed));
        owner = session.cloned();
        let mut state = session.map(|s| lock(&s.state));
        let value = perform.take().expect("one allocator forwarding call")();
        result = Some(value);
        let Some((session, state)) = session.zip(state.as_mut()) else {
            return;
        };
        let now = session.origin.elapsed().min(session.duration).as_nanos() as u64;
        if state.ended.is_some() {
            return;
        }
        if session.origin.elapsed() >= session.duration {
            state.ended = Some(now);
            ENABLED.store(false, Ordering::Release);
            return;
        }
        if state.events.len() == session.config.max_events {
            state.full = true;
            state.ended = Some(state.events.last().map_or(0, |e| e.at));
            ENABLED.store(false, Ordering::Release);
            return;
        }
        let operation = match kind {
            AllocatorOperation::Alloc { size } => {
                if value == 0 {
                    return;
                }
                MemoryOperation::Alloc {
                    address: value as u64,
                    size: size as u64,
                }
            }
            AllocatorOperation::Free { address, size } => MemoryOperation::Free {
                address: address as u64,
                size: size as u64,
            },
            AllocatorOperation::Realloc {
                address,
                old_size,
                size,
            } => {
                if value == 0 {
                    MemoryOperation::ReallocFailed {
                        address: address as u64,
                        old_size: old_size as u64,
                        requested_size: size as u64,
                    }
                } else {
                    MemoryOperation::Realloc {
                        old_address: address as u64,
                        old_size: old_size as u64,
                        address: value as u64,
                        size: size as u64,
                    }
                }
            }
        };
        let mut frames = [0; MAX_STACK_FRAMES];
        let count = if matches!(kind, AllocatorOperation::Free { .. }) {
            0
        } else {
            stack(session.config.stack_frames, &mut frames).min(session.config.stack_frames)
        };
        let thread = session.threads.current();
        state.events.push(RawEvent {
            at: now,
            thread,
            operation,
            frames,
            count,
        });
        if state.events.len() == session.config.max_events {
            state.full = true;
            state.ended = Some(now);
            ENABLED.store(false, Ordering::Release);
        }
    }));
    if observed.is_err()
        && let Some(owner) = owner
    {
        owner.errors.fetch_add(1, Ordering::Relaxed);
        if lock(&ACTIVE)
            .as_ref()
            .is_some_and(|s| Arc::ptr_eq(s, &owner))
        {
            ENABLED.store(false, Ordering::Release);
        }
        let mut state = lock(&owner.state);
        let end = state.events.last().map_or(0, |e| e.at);
        state.ended.get_or_insert(end);
    }
    result.unwrap_or_else(|| perform.take().expect("unperformed allocator call")())
}

impl Recording {
    pub fn stats(&self) -> MemoryStats {
        let state = lock(&self.0.state);
        MemoryStats {
            events: state.events.len(),
            limit_reached: state.full || state.events.len() == self.0.config.max_events,
            hook_errors: self.0.errors.load(Ordering::Relaxed),
        }
    }
    pub fn start(
        origin: Instant,
        duration: Duration,
        config: MemoryConfig,
        threads: Arc<Registry>,
    ) -> Result<Self, super::TraceError> {
        let _quiet = suppress();
        if !allocator_installed() {
            return Err(super::TraceError::MemoryUnavailable);
        }
        if config.max_events == 0
            || config.max_events > 200_000
            || config.stack_frames > MAX_STACK_FRAMES
        {
            return Err(super::TraceError::InvalidBudget);
        }
        let mut active = lock(&ACTIVE);
        if active
            .as_ref()
            .is_some_and(|s| s.origin.elapsed() < s.duration && lock(&s.state).ended.is_none())
        {
            return Err(super::TraceError::AlreadyRecording);
        }
        let session = Arc::new(Session {
            origin,
            duration,
            config,
            threads,
            state: Mutex::new(State {
                events: Vec::with_capacity(config.max_events),
                ended: None,
                full: false,
            }),
            errors: AtomicU64::new(0),
        });
        *active = Some(session.clone());
        ENABLED.store(true, Ordering::Release);
        Ok(Self(session))
    }
    fn detach(&self) {
        let mut active = lock(&ACTIVE);
        if active.as_ref().is_some_and(|s| Arc::ptr_eq(s, &self.0)) {
            ENABLED.store(false, Ordering::Release);
            *active = None;
        }
    }
    pub fn stop(self, cutoff: Duration) -> MemoryCapture {
        let _quiet = suppress();
        self.detach();
        let mut state = lock(&self.0.state);
        let elapsed = state
            .ended
            .unwrap_or_else(|| self.0.origin.elapsed().min(self.0.duration).as_nanos() as u64);
        let elapsed = elapsed.min(cutoff.as_nanos() as u64);
        state.ended = Some(elapsed);
        let raw = std::mem::take(&mut state.events);
        let full = state.full;
        drop(state);
        let mut ids = BTreeMap::<Vec<usize>, u64>::new();
        let mut stacks = Vec::new();
        let mut stored_frames = 0usize;
        let mut stack_observations_dropped = 0usize;
        let mut events = Vec::with_capacity(raw.len());
        for row in raw {
            if row.at > elapsed {
                continue;
            }
            let stack = if row.count == 0 {
                None
            } else {
                let addresses = row.frames[..row.count].to_vec();
                if let Some(id) = ids.get(&addresses) {
                    Some(*id)
                } else if stored_frames + addresses.len() > 131_072 || ids.len() >= 8192 {
                    stack_observations_dropped += 1;
                    None
                } else {
                    let next = ids.len() as u64 + 1;
                    stored_frames += addresses.len();
                    stacks.push(MemoryStack {
                        id: next,
                        potentially_truncated: row.count == self.0.config.stack_frames,
                        frames: addresses
                            .iter()
                            .map(|ip| MemoryStackFrame {
                                address: format!("0x{ip:x}"),
                                function: None,
                                file: None,
                                line: None,
                            })
                            .collect(),
                    });
                    ids.insert(addresses, next);
                    Some(next)
                }
            };
            events.push(MemoryEvent {
                id: events.len() as u64 + 1,
                time_ns: row.at,
                thread: row.thread,
                stack,
                operation: row.operation,
            });
        }
        MemoryCapture {
            schema_version: 1,
            process_id: std::process::id(),
            elapsed_ns: elapsed,
            max_events: self.0.config.max_events,
            provider: "rust_system_global_allocator".into(),
            baseline_known: false,
            event_limit_reached: full,
            hook_errors: self.0.errors.load(Ordering::Relaxed),
            stack_frames: self.0.config.stack_frames,
            symbols_resolved: false,
            stack_frame_budget: 131_072,
            stack_observations_dropped,
            symbol_budget_exhausted: false,
            threads: self.0.threads.snapshot(),
            stacks,
            events,
        }
    }
}
impl Drop for Recording {
    fn drop(&mut self) {
        let _quiet = suppress();
        self.detach();
        let mut state = lock(&self.0.state);
        state.ended.get_or_insert(0);
        state.events = Vec::new();
    }
}

mod address {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};
    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("0x{value:x}"))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        let value = String::deserialize(deserializer)?;
        let raw = value
            .strip_prefix("0x")
            .filter(|s| !s.is_empty() && s.len() <= 16 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| D::Error::custom("bounded hexadecimal allocation address required"))?;
        u64::from_str_radix(raw, 16).map_err(D::Error::custom)
    }
}
