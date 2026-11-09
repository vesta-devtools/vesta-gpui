# Capture-owned heap lifetimes and allocation stacks

Status: Accepted; implementation in progress under IN04 of decision 0043.
Date: 2026-11-09.

The requested Memory Insights alignment requires actual allocation/free lifetimes
and allocation callstacks, interval queries and explicit coverage. RSS counters
alone do not meet the request.

Provide an explicitly installed `UiTraceAllocator` at the consuming application's
global-allocator boundary. Delegate `System` allocation, zeroed allocation, free
and realloc exactly; never inspect allocation contents. Capture remains opt-in.
Only this narrow winit adapter module admits documented allocator forwarding and
Windows stack-unwind calls. The backend-neutral core remains entirely safe Rust.
Installing a global allocator in a library implicitly is forbidden; Motion opts
in as an SDK integration example.

Use a reentrancy/suppression guard and one capture-owned operation gate to order
allocation operations, including realloc/address reuse. Allocate the bounded raw
event buffer before enabling hooks. Allocator callbacks must not unwind, format
strings, symbolize or perform I/O. Resolve deduplicated stack addresses after
freezing on the host's I/O worker. Unsupported stacks and unresolved symbols stay
explicit. Capture-owned CPU and memory data share a monotonic origin and thread
registry; neither hook takes the CPU event-state lock.

The `gpui_memory` extension retains original allocation-operation IDs, addresses,
sizes, times, threads and stack IDs. Address reuse creates distinct allocation
lifetimes; realloc closes the old lifetime and opens a new one only on success.
Free/realloc of a pre-capture address is an unknown baseline operation, not a
fabricated allocation. Memory coverage ends on its own event/time limit; surviving
observed allocations at that end are not automatically leaks. Producer loss,
unknown baseline and stack/symbol coverage must accompany analysis.

The analyzer validates immutable input, builds checked lifetimes and byte/count
curves, and exposes bounded allocation, stack, time-point and interval-comparison
queries through the same engine as the native memory workspace. Keep original
evidence keys and source identity. Adding heap tracing does not establish GPU
resource tracking, native-library/driver heaps or system stack sampling coverage.

Qualify real SDK allocations with address reuse, alloc_zeroed, realloc success
and failure, cross-thread free, capture stop/expiration/limits, successor ownership,
reentrancy and shutdown. Reconcile original operations against independent
integer arithmetic, generated Agent schemas and actual native interactions.
Measure enabled/disabled cost before completing IN05.

The 2026-11-10 local foundation qualification is recorded in
`target/research/native-memory-20261110/qualification.json` and the current
[delivery plan](../TRACE-PLAN.md). Operation, stack-expansion and retained-symbol
budgets are distinct: missing stack metadata does not remove allocation events.
Actual Windows/System allocation and native/Agent workflows are qualified in
that scope; diagnostic failures and stack-budget pressure have explicit contract
simulations. Startup baseline, native/driver/GPU resource heaps, full execution
sampling, overhead and wider professional delivery remain open.

The allocator integration test checks a baseline address only before its first
free and a survivor only after its own allocation event. System may reuse either
address for a different lifetime; an earlier free is not evidence that the
survivor was freed during capture. The broader pre-push run exposed and corrected
that test assumption without changing allocator forwarding or recorded events.
