//! Strict heap-operation admission and immutable observed allocation lifetimes.
use crate::session::*;
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
};
pub use vesta_gpui::trace::memory::{MemoryCapture, MemoryOperation};

#[derive(Debug)]
pub struct Allocation {
    pub id: u64,
    pub address: u64,
    pub size: u64,
    pub allocated_ns: u64,
    pub freed_ns: Option<u64>,
    pub thread: u32,
    pub free_thread: Option<u32>,
    pub stack: Option<u64>,
    pub end_operation: Option<u64>,
    pub reallocated_from: Option<u64>,
    pub unknown_prior_reallocation: bool,
}
impl Allocation {
    pub fn live_at(&self, time: u64) -> bool {
        self.allocated_ns <= time && self.freed_ns.is_none_or(|end| end > time)
    }
    pub fn overlaps(&self, range: TimeRange) -> bool {
        if self.freed_ns == Some(self.allocated_ns) {
            range.contains(self.allocated_ns)
        } else {
            self.allocated_ns < range.end && self.freed_ns.is_none_or(|end| end > range.start)
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub operation: u64,
    pub time_ns: u64,
    pub bytes: u64,
    pub count: usize,
}
#[derive(Debug)]
pub struct MemorySession {
    pub capture: MemoryCapture,
    pub allocations: Vec<Allocation>,
    pub indices: BTreeMap<u64, usize>,
    pub stacks: BTreeMap<u64, usize>,
    pub points: Vec<Point>,
    pub unknown_frees: usize,
    pub unknown_reallocations: usize,
    pub realloc_failures: usize,
    pub peak_bytes: u64,
    pub peak_count: usize,
    pub observed_allocated_bytes: u64,
    pub resolved_frames: usize,
    pub unresolved_frames: usize,
    pub allocations_with_stacks: usize,
    pub potentially_truncated_stacks: usize,
}
fn invalid(message: &str) -> TraceError {
    TraceError::new("trace_memory_invalid", message)
}
fn checked_add(a: u64, b: u64) -> Result<u64, TraceError> {
    a.checked_add(b)
        .ok_or_else(|| invalid("Heap byte total overflows u64"))
}
pub fn admit(
    session: &mut Session,
    capture: MemoryCapture,
    cancel: &AtomicBool,
) -> Result<(), TraceError> {
    if capture.schema_version != 1
        || capture.process_id == 0
        || capture.provider != "rust_system_global_allocator"
        || capture.baseline_known
        || capture.elapsed_ns > ((1u64 << 53) - 1)
        || capture.max_events == 0
        || capture.max_events > 200_000
        || capture.events.len() > capture.max_events
        || capture.stack_frames > 32
        || capture.stacks.len() > capture.events.len()
        || capture.threads.len() > 1_000_000
        || capture.stack_frame_budget > 131_072
        || capture.stack_observations_dropped > capture.events.len()
    {
        return Err(invalid(
            "Unsupported memory schema/provider/baseline or record budget",
        ));
    }
    let process = u64::from(capture.process_id);
    if session
        .gpu
        .as_ref()
        .is_some_and(|g| g.process_id != capture.process_id)
    {
        return Err(invalid("Memory and GPU capture processes differ"));
    }
    if !session.events.is_empty() && !session.events.iter().any(|e| e.key.process == process) {
        return Err(invalid("Memory process has no relationship to CPU capture"));
    }
    let mut threads = BTreeMap::new();
    for t in &capture.threads {
        let lane = LaneKey {
            process,
            thread: u64::from(t.thread),
        };
        if t.thread == 0
            || t.name.as_ref().is_some_and(|n| n.len() > 512)
            || threads.insert(t.thread, t.name.clone()).is_some()
        {
            return Err(invalid("Invalid memory thread metadata"));
        }
        if session
            .thread_names
            .get(&lane)
            .is_some_and(|name| name != &t.name)
        {
            return Err(invalid("Memory and CPU thread names disagree"));
        }
        session.thread_names.insert(lane, t.name.clone());
    }
    let mut stacks = BTreeMap::new();
    for (index, stack) in capture.stacks.iter().enumerate() {
        if index % 1024 == 0 && cancel.load(Ordering::Relaxed) {
            return Err(TraceError::new(
                "trace_cancelled",
                "Memory admission cancelled",
            ));
        }
        if stack.id == 0
            || stacks.insert(stack.id, index).is_some()
            || stack.frames.is_empty()
            || stack.frames.len() > capture.stack_frames
        {
            return Err(invalid("Invalid stack identity/depth"));
        }
        for f in &stack.frames {
            let address = f
                .address
                .strip_prefix("0x")
                .filter(|s| {
                    !s.is_empty() && s.len() <= 16 && s.bytes().all(|b| b.is_ascii_hexdigit())
                })
                .and_then(|s| u64::from_str_radix(s, 16).ok());
            if address.is_none_or(|a| a == 0)
                || f.function
                    .as_ref()
                    .is_some_and(|s| s.is_empty() || s.len() > 16384)
                || f.file.as_ref().is_some_and(|s| s.len() > 16384)
                || capture.symbols_resolved && f.function.is_none()
            {
                return Err(invalid("Invalid stack address or symbol metadata"));
            }
        }
    }
    let resolved_frames = capture
        .stacks
        .iter()
        .flat_map(|s| &s.frames)
        .filter(|f| f.function.is_some())
        .count();
    let frame_count = capture.stacks.iter().map(|s| s.frames.len()).sum::<usize>();
    if capture.stack_frame_budget != 0 && frame_count > capture.stack_frame_budget {
        return Err(invalid(
            "Stored heap stack frames exceed their declared budget",
        ));
    }
    let potentially_truncated_stacks = capture
        .stacks
        .iter()
        .filter(|s| s.potentially_truncated)
        .count();
    let mut data = MemorySession {
        capture,
        allocations: Vec::new(),
        indices: BTreeMap::new(),
        stacks,
        points: Vec::new(),
        unknown_frees: 0,
        unknown_reallocations: 0,
        realloc_failures: 0,
        peak_bytes: 0,
        peak_count: 0,
        observed_allocated_bytes: 0,
        resolved_frames,
        unresolved_frames: frame_count - resolved_frames,
        allocations_with_stacks: 0,
        potentially_truncated_stacks,
    };
    let mut live = BTreeMap::<u64, usize>::new();
    let mut bytes = 0;
    let mut previous = 0;
    for event in &data.capture.events {
        if event.id % 1024 == 0 && cancel.load(Ordering::Relaxed) {
            return Err(TraceError::new(
                "trace_cancelled",
                "Memory admission cancelled",
            ));
        }
        if event.id != data.points.len() as u64 + 1
            || event.time_ns < previous
            || event.time_ns > data.capture.elapsed_ns
            || !threads.contains_key(&event.thread)
            || event.stack.is_some_and(|id| !data.stacks.contains_key(&id))
        {
            return Err(invalid(
                "Memory operation identity/time/thread/stack is invalid",
            ));
        }
        previous = event.time_ns;
        let close = |address: u64,
                     size: u64,
                     live: &mut BTreeMap<u64, usize>,
                     allocations: &mut Vec<Allocation>,
                     bytes: &mut u64|
         -> Result<Option<u64>, TraceError> {
            if address == 0
                || size == 0
                || size > isize::MAX as u64
                || address.checked_add(size).is_none()
            {
                return Err(invalid("Invalid free/reallocation address/size"));
            }
            let Some(index) = live.remove(&address) else {
                return Ok(None);
            };
            let row = &mut allocations[index];
            if row.size != size {
                return Err(invalid(
                    "Free/reallocation size differs from its original allocation",
                ));
            }
            *bytes = bytes
                .checked_sub(size)
                .ok_or_else(|| invalid("Live byte total underflows"))?;
            row.freed_ns = Some(event.time_ns);
            row.free_thread = Some(event.thread);
            row.end_operation = Some(event.id);
            Ok(Some(row.id))
        };
        let mut open = None;
        match event.operation {
            MemoryOperation::Alloc { address, size } => open = Some((address, size, None, false)),
            MemoryOperation::Free { address, size } => {
                if close(address, size, &mut live, &mut data.allocations, &mut bytes)?.is_none() {
                    data.unknown_frees += 1;
                }
            }
            MemoryOperation::Realloc {
                old_address,
                old_size,
                address,
                size,
            } => {
                let prior = close(
                    old_address,
                    old_size,
                    &mut live,
                    &mut data.allocations,
                    &mut bytes,
                )?;
                if prior.is_none() {
                    data.unknown_reallocations += 1;
                }
                open = Some((address, size, prior, prior.is_none()));
            }
            MemoryOperation::ReallocFailed {
                address,
                old_size,
                requested_size,
            } => {
                if address == 0
                    || old_size == 0
                    || requested_size == 0
                    || old_size > isize::MAX as u64
                    || requested_size > isize::MAX as u64
                {
                    return Err(invalid("Invalid failed realloc metadata"));
                }
                if live
                    .get(&address)
                    .is_some_and(|i| data.allocations[*i].size != old_size)
                {
                    return Err(invalid(
                        "Failed realloc changed the original allocation size",
                    ));
                }
                data.realloc_failures += 1;
            }
        }
        if let Some((address, size, prior, unknown)) = open {
            if address == 0
                || size == 0
                || size > isize::MAX as u64
                || live.contains_key(&address)
                || address.checked_add(size).is_none()
            {
                return Err(invalid(
                    "Allocation address/size is invalid or overlaps a live allocation",
                ));
            }
            let end = address + size;
            if live
                .range(..address)
                .next_back()
                .is_some_and(|(start, index)| {
                    start.saturating_add(data.allocations[*index].size) > address
                })
                || live
                    .range(address..)
                    .next()
                    .is_some_and(|(start, _)| *start < end)
            {
                return Err(invalid(
                    "Allocation address range overlaps another live lifetime",
                ));
            }
            bytes = checked_add(bytes, size)?;
            data.observed_allocated_bytes = checked_add(data.observed_allocated_bytes, size)?;
            let index = data.allocations.len();
            live.insert(address, index);
            data.indices.insert(event.id, index);
            data.allocations.push(Allocation {
                id: event.id,
                address,
                size,
                allocated_ns: event.time_ns,
                freed_ns: None,
                thread: event.thread,
                free_thread: None,
                stack: event.stack,
                end_operation: None,
                reallocated_from: prior,
                unknown_prior_reallocation: unknown,
            });
            data.allocations_with_stacks += usize::from(event.stack.is_some());
        }
        data.peak_bytes = data.peak_bytes.max(bytes);
        data.peak_count = data.peak_count.max(live.len());
        data.points.push(Point {
            operation: event.id,
            time_ns: event.time_ns,
            bytes,
            count: live.len(),
        });
    }
    session.bounds.start = 0;
    session.bounds.end = session
        .bounds
        .end
        .max(data.capture.elapsed_ns.saturating_add(1));
    session.diagnostics.push(Diagnostic{code:"trace_memory_baseline_unknown",message:"Only observed Rust global-allocator lifetimes are known; pre-capture, recorder-excluded, native/driver and GPU heaps are not a process-wide baseline. Surviving observed blocks are not automatically leaks.".into()});
    if data.capture.event_limit_reached || data.capture.hook_errors != 0 {
        session.diagnostics.push(Diagnostic {
            code: "trace_memory_partial",
            message: format!(
                "Memory coverage ends at {} ns; event limit {}, hook errors {}",
                data.capture.elapsed_ns, data.capture.event_limit_reached, data.capture.hook_errors
            ),
        });
    }
    session.memory = Some(data);
    Ok(())
}
