# Frame capture and UI tracing

GPUI provides one opt-in recorder shared by its libraries and the consuming app.
Start a capture, collect CPU frame attempts, then select a frame to inspect its
recorded call tree and timer totals. It works without ETW, administrator access,
a service or an external profiler. Common GPUI stages are already instrumented;
hosts do not annotate every library call or maintain a function-name table.
It is disabled until the host explicitly starts it. Disabled stage hooks return
before taking a lock or reading the clock. Active recording introduces overhead;
use a short capture when investigating a problem.

```rust
use vesta_gpui::trace::{self, TraceCapture, TraceConfig};

let capture = TraceCapture::start(TraceConfig::default())?;
// Keep `capture` in host diagnostic state, across ordinary application events.
{
    let _frame = trace::frame("MyApp.Main"); // once at the existing CPU frame entry
    {
        let _build = trace::span("my_app", "build.tree"); // optional business scope
        // Normal host tree construction goes here.
    }
    // Normal GPUI layout/paint calls record their stages automatically.
    trace::instant("my_app", "frame.skipped.wrap_changed"); // only on the real branch
    trace::counter("my_app", "layout.pass_count", 2.0);     // only an actual observation
}
let snapshot = capture.stop();
let json = snapshot.to_chrome_json()?;
let report = snapshot.to_html_report()?; // self-contained offline frame picker
let frames = snapshot.frames();         // the same typed analysis used by exports
// The host chooses the destination and performs I/O, off the UI thread for large exports.
Ok::<(), Box<dyn std::error::Error>>(())
```

The library automatically records ordinary/retained layout, intrinsic measurement,
static/styled text shaping, input refresh and actual wrap-width changes, paint-list
construction, hit-test/action-route construction and accessibility projection.
The Vulkan renderer records an attempt scope and its existing CPU phase boundaries:
readiness, targets, preparation, acquisition, upload, recording, command creation,
submission and explicit finish. Successful present submission has its own marker.
Device-free production paint preparation also joins the same capture.

Scopes include cache lookup time. A text shaping scope does not by itself prove a
cache miss. A renderer phase interrupted by return/error is labelled
`phase.interrupted`; the successful boundary was not reached. `present.submitted`
means the CPU submission call succeeded, not that a frame appeared on a monitor.
Put `trace::frame` around the existing host frame entry, including early returns.
Frames that skip the renderer still appear in the capture. Add a skip-reason marker
on the actual branch if that reason is needed; absence of rendering alone does not
prove a particular retry cause. A CPU attempt is not necessarily a displayed frame.

## Native example launch

Both SDK applications use the same native-adapter `UiTraceRecording` launcher:

```powershell
cargo run --locked -p vesta-gpui-standalone -- --trace-ui=target/standalone-frames.html
cargo run --locked -p vesta-motion -- --trace-ui=target/motion-frames.json
```

Use a new `.html` or `.json` path. Collection starts on launch with the default
30-second / 100,000-event budget. Close the example normally to export after its
event loop has returned. The time limit ends collection, not the application.
Without the argument there is no capture or output. An existing destination is
rejected, including a file created by another writer before final export.
Dropping the recorder without explicit finish discards data without file I/O.

The native launcher defaults to **stage** detail so repeated intrinsic and static
text measurements do not consume the budget before interaction. The base
`TraceCapture::start` retains detailed behavior; use
`TraceCapture::start_with_detail(config, TraceDetail::Stages)` for an explicit
stage capture. Stage capture preserves parent wall time, actual input refresh and
the common layout/paint/render phases. Exports state `gpui_trace.detail`; absence
of omitted detail is not evidence of zero cost or a cache hit.

Interactive hosts use `UiTraceRecording::start(path, config, detail)` only when
the user starts capture. `stats()` exposes deadline/loss information. `stop()`
freezes without file I/O and returns a `UiTraceExport`; send this owned value to
the host's I/O worker and call `write()` there. It uses the same non-overwriting
export implementation as launch recording. The host drains its worker when
closing; the framework does not create a thread or choose a global destination.

Another winit host can reuse the launcher and call
`recording.finish_after_run(run_application())` from `main`, plus put one
`trace::frame` guard at each frame entry. A host with its own diagnostics UI can
use `TraceCapture` directly for interactive start/stop. The base collector owns no
event loop, debug panel or disk I/O.

## Host responsibilities

The app owns its menu, keyboard command or debug-panel start/stop control, capture
ownership and output location. It also owns business scopes: tree construction,
Dock pointer handling, frame settlement/retry decisions and file/model work. The
SDK has no knowledge of an Agent conversation or a Builder graph. Add a frame
entry around the existing host frame and let RAII close it on every return.
Attach a skip marker on the actual early-return branch. Do not change input,
layout/hit-test ownership or render scheduling merely to produce a trace.

Only one process-local capture may be active. Starting another returns
`gpui_trace_already_recording`; an expired capture may be replaced. Capture-local
thread IDs distinguish lanes but are not OS thread IDs. A scope must stay on its
originating thread and must not cross an async await. Its lifetime must follow
normal nested synchronous scopes. Events automatically inherit the current parent
and frame on their thread. Worker events without their own explicit frame entry
remain unframed; overlapping timestamps do not invent cross-thread causality.
Frame numbers and event IDs belong only to that capture, not author identities.

The recorder belongs to the linked base-library instance. Resolve the GPUI family
to one revision/instance. A separately linked native DLL or another process has
its own static recorder; crossing that boundary needs a separate capture or an
explicit host diagnostic bridge. Rust recorder objects are not a cross-language
ABI or a cross-process transport.

Default budgets are 100,000 events and 30 seconds; configurable hard limits are
200,000 events and 300 seconds. The full buffer preserves its prefix and counts
losses. The deadline stops accepting events on the next observation; no timer
thread is started. `is_recording()`/`stats()` let the host update its indicator.
The host still calls `stop()` to collect the result. Dropping the capture disables
it without creating a file. Stopping or expiring clips open scopes and marks them
`unfinished`; late guard drops cannot alter a stopped snapshot or its successor.
Inspect `dropped_events`, `unfinished_spans`, `invalid_nesting` and `expired` before
drawing conclusions. Non-LIFO scope closure is counted as invalid nesting; its
tree and self times must not be treated as a valid nested execution history.

## What the times mean

Intervals measure monotonic **elapsed wall time** on the CPU side, including OS
scheduling and waits inside that interval. They are not sampled CPU utilization,
sampled call stacks, heap allocation counts or GPU execution durations. A call
tree contains recorded scopes, not every Rust/native function. Like framework
trace events, the common stages are maintained inside GPUI; arbitrary host code
needs an optional business scope if more detail is wanted.

The shared analyzer returns frames, each with its recorded calls, markers/counters
and timers aggregated by category/name. Inclusive time includes children. Self /
unattributed wall time subtracts the union of recorded same-thread direct-child
intervals clipped to their parent. It includes waits and uninstrumented work;
it is not exclusive CPU utilization. Inclusive parent/child totals overlap, so
adding them double-counts work. No frame-rate or presentation latency is inferred.

The existing Vulkan `enable_cpu_profiling`/`cpu_profile` API remains independent.
Its opt-in and return behavior are unchanged. Actual GPU timestamp profiling uses
the existing separately enabled, capability-checked `enable_gpu_profiling` stream
with its generation/submission identities; enabling this trace does not enable
GPU queries, add waits/readbacks or relabel CPU submission time as GPU time.

Trace hooks collect static category/name labels, capture-local thread IDs,
elapsed timestamps and finite numeric counters. They do not collect document
text, input values, titles, screenshots, author data or file paths. Host-provided
labels/counters are the host's responsibility. There is no upload. Export requires
an explicit host destination and action. Timestamps are capture-relative, not UTC.

## Export and try it

The JSON uses Chrome Trace Event `X` slices, thread-scoped instant markers and
`C` counters, supported by [Perfetto's external-format importer](https://perfetto.dev/docs/getting-started/other-formats).
Open the explicitly exported file in a compatible local trace viewer or choose
to load it into Perfetto. The SDK does not open a browser or upload the trace.
The top-level `gpui_trace` schema v2 records capture limits/losses, nesting errors
and unframed event count. Each timeline event carries capture-local parent/frame
identities. `gpui_frames` contains the same analyzed profiles as `frames()`.

The self-contained HTML report uses these profiles directly: select a frame from
the duration overview or frame-number field, expand recorded calls and compare
inclusive/self timer totals. Loss/clipping/nesting warnings stay visible. It
opens offline, makes no network requests and renders labels as text. Large lists
are expanded in bounded batches. Unframed events remain in the JSON timeline.

```powershell
cargo run --locked -p vesta-gpui --example trace_layout -- target/my-ui-trace.json
cargo run --locked -p vesta-gpui --example trace_layout -- target/my-ui-frames.html
```

Choose a new output path; the example refuses to overwrite an existing file.
It runs real headless layout/text/paint operations with synthetic width changes,
including ten explicit CPU attempts and host skip markers. It opens no window and produces no
native frame-time or GPU acceptance evidence. This module does not update already
running applications. A product must adopt the SDK revision, add host controls
and relevant host scopes, and rebuild before using this facility interactively.
