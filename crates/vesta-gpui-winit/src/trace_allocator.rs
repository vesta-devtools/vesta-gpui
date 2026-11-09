//! Explicit System GlobalAlloc boundary. Pointer values are never dereferenced here.
use std::alloc::{GlobalAlloc, Layout, System};
use vesta_gpui::trace::memory::{self, AllocatorOperation, MAX_STACK_FRAMES};

/// Install explicitly in a consuming executable with `#[global_allocator]`.
/// Ordinary allocations delegate to System; heap capture is separately opt-in.
pub struct UiTraceAllocator;

fn stack(requested: usize, frames: &mut [usize; MAX_STACK_FRAMES]) -> usize {
    #[cfg(windows)]
    {
        if requested == 0 {
            return 0;
        }
        let mut pointers = [std::ptr::null_mut(); MAX_STACK_FRAMES];
        // SAFETY: the bounded output array has capacity for every requested
        // frame. The function only captures this thread's current stack; it
        // neither reads user allocation contents nor retains the output pointer.
        let count = unsafe {
            windows_sys::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace(
                2,
                requested.min(MAX_STACK_FRAMES) as u32,
                pointers.as_mut_ptr(),
                std::ptr::null_mut(),
            )
        } as usize;
        for (target, pointer) in frames.iter_mut().zip(pointers).take(count) {
            *target = pointer as usize;
        }
        count
    }
    #[cfg(not(windows))]
    {
        let _ = (requested, frames);
        0
    }
}

// SAFETY: every operation forwards exactly once to the same System allocator
// with the caller's unchanged pointer/layout/size. The safe observation gate
// retains the real result across diagnostic failures and cannot unwind through
// this boundary. Integer round-trips use exposed pointer provenance; no captured
// allocation or instruction address is dereferenced by the observer.
unsafe impl GlobalAlloc for UiTraceAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        memory::allocator_operation(
            AllocatorOperation::Alloc {
                size: layout.size(),
            },
            || {
                // SAFETY: GlobalAlloc's caller supplies a valid nonzero layout.
                (unsafe { System.alloc(layout) }) as usize
            },
            stack,
        ) as *mut u8
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        memory::allocator_operation(
            AllocatorOperation::Alloc {
                size: layout.size(),
            },
            || {
                // SAFETY: the same valid layout is forwarded, preserving zeroing.
                (unsafe { System.alloc_zeroed(layout) }) as usize
            },
            stack,
        ) as *mut u8
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        memory::allocator_operation(
            AllocatorOperation::Free {
                address: ptr as usize,
                size: layout.size(),
            },
            || {
                // SAFETY: the caller's live System allocation and original layout
                // are forwarded unchanged, once; no reads occur after deallocation.
                unsafe { System.dealloc(ptr, layout) };
                0
            },
            stack,
        );
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        memory::allocator_operation(
            AllocatorOperation::Realloc {
                address: ptr as usize,
                old_size: layout.size(),
                size: new_size,
            },
            || {
                // SAFETY: the GlobalAlloc contract supplies the live original
                // pointer/layout and valid new size. Null preserves the old block.
                (unsafe { System.realloc(ptr, layout, new_size) }) as usize
            },
            stack,
        ) as *mut u8
    }
}

/// Resolve after freezing, on the host's export/control worker, never in a hook.
pub(crate) fn resolve_stacks(snapshot: &mut vesta_gpui::trace::TraceSnapshot) {
    #[cfg(windows)]
    if let Some(memory) = snapshot.memory_mut() {
        use std::collections::BTreeMap;
        // Keep automatic export local; configured symbol-server paths remain
        // untouched and unsupported resolutions stay unknown.
        if ["_NT_SYMBOL_PATH", "_NT_ALT_SYMBOL_PATH"]
            .iter()
            .filter_map(std::env::var_os)
            .any(|value| {
                let value = value.to_string_lossy().to_lowercase();
                value.contains("srv") || value.contains("://") || value.contains("\\\\")
            })
        {
            return;
        }
        let started = std::time::Instant::now();
        let mut cache = BTreeMap::<String, (Option<String>, Option<String>, Option<u32>)>::new();
        let mut cached_bytes = 0usize;
        let mut assigned_bytes = 0usize;
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for stack in &mut memory.stacks {
                for frame in stack.frames.iter_mut().rev() {
                    if !cache.contains_key(&frame.address)
                        && cache.len() < 4096
                        && started.elapsed() < std::time::Duration::from_secs(2)
                    {
                        let mut result = (None, None, None);
                        if let Ok(address) =
                            u64::from_str_radix(frame.address.trim_start_matches("0x"), 16)
                        {
                            backtrace::resolve(address as *mut std::ffi::c_void, |symbol| {
                                if result.0.is_none() {
                                    result = (
                                        symbol
                                            .name()
                                            .map(|n| n.to_string())
                                            .filter(|s| !s.is_empty() && s.len() <= 4096),
                                        symbol
                                            .filename()
                                            .map(|p| p.to_string_lossy().into_owned())
                                            .filter(|s| s.len() <= 4096),
                                        symbol.lineno(),
                                    );
                                }
                            });
                        }
                        let bytes = result.0.as_ref().map_or(0, String::len)
                            + result.1.as_ref().map_or(0, String::len);
                        if cached_bytes + bytes <= 4 * 1024 * 1024 {
                            cached_bytes += bytes;
                        } else {
                            result = (None, None, None);
                            memory.symbol_budget_exhausted = true;
                        }
                        cache.insert(frame.address.clone(), result);
                    }
                    if let Some((function, file, line)) = cache.get(&frame.address) {
                        let bytes = function.as_ref().map_or(0, String::len)
                            + file.as_ref().map_or(0, String::len);
                        if assigned_bytes + bytes > 4 * 1024 * 1024 {
                            memory.symbol_budget_exhausted = true;
                            continue;
                        }
                        assigned_bytes += bytes;
                        frame.function = function.clone();
                        frame.file = file.clone();
                        frame.line = *line;
                    }
                }
            }
        }));
        memory.symbols_resolved = !memory.stacks.is_empty()
            && memory
                .stacks
                .iter()
                .flat_map(|s| &s.frames)
                .all(|f| f.function.is_some());
    }
    #[cfg(not(windows))]
    let _ = snapshot;
}
