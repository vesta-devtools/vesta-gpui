# Native capture, host integration and Agents

The default `vesta-trace` window uses GPUI/Vulkan. No browser or profiler install
is required for this native path.

```powershell
cargo build --locked -p vesta-trace -p vesta-motion
.\target\debug\vesta-motion.exe
```

Click Motion's **Trace…** button. The native analyzer opens attached to the
running host. Choose the title-strip duration (10/30/60 seconds), **Start
capture**, interact with the host, then **Stop & analyze**. A duration/event limit
also freezes the capture and automatically opens the saved result. The compact
controls leave the timeline and inspection workspace available during analysis.
New captures open the **Frames** table for CPU/GPU comparison. Use **CPU ↓**,
**GPU ↓** or **Time ↑**, select a frame to inspect its CPU scopes, and follow
**Inspect longest GPU submission** to the original device-timestamp evidence.
GPU work sums associated whole submissions; clocks remain uncalibrated.
On a host with `UiTraceAllocator`, enable **Memory: on** before starting to
record allocation/free/realloc operations and allocation stacks. Open **Memory**
after capture. Drag an A–B range in its bytes/count tracks; choose **Live at B**,
**New at B**, **Released**, **Common** or **Transient** and select an allocation
to inspect the original stack and realloc lineage. The detail pane scrolls with
the wheel or keyboard. Memory stays off by default.
Repeat capture without restarting the host. Closing the analyzer leaves the host
running. Saving defaults to `%LOCALAPPDATA%/VestaGPUI/Traces/`; use
`--output-dir=<directory>` to select another destination. Existing files are never
overwritten. A failed save retains the frozen snapshot for another Stop attempt.

Alternatively start `vesta-trace.exe` and **Launch target** (the adjacent Motion
example by default), or select your host on the command line:

```powershell
.\target\debug\vesta-trace.exe --target=D:\MyApp\app.exe --target-arg=--my-option
.\target\debug\vesta-trace.exe --connect=C:\private\session.json
.\target\debug\vesta-trace.exe C:\captures\trace.json
```

The launched host must retain `UiTraceRecording::from_arguments` across its event
loop, as Motion does. Attach accepts an explicit SDK session descriptor; it does
not attach/inject into arbitrary process IDs. Keep the descriptor private because
it contains the local control capability. The tool never kills the host.

To support the optional Rust heap channel, explicitly install the SDK allocator
in the consuming executable (one global allocator per executable):

```rust
#[global_allocator]
static ALLOCATOR: vesta_gpui_winit::UiTraceAllocator = vesta_gpui_winit::UiTraceAllocator;
```

It forwards `System` allocation, zeroed allocation, free and realloc. Installing
it alone starts no capture. An executable that already owns a custom allocator
needs a separately reviewed integration; this wrapper is specifically for
`System`. CPU/GPU capture continues to work without this optional allocator.
Windows allocation stacks are captured as addresses and resolved locally after
freezing. Missing symbols remain explicit. Local symbolization has a cooperative
two-second lookup budget and a 4,096-address bound; a single OS lookup is not
preemptible. Configured symbol-server paths cause automatic resolution to skip.
Stack expansion is separately bounded to 8,192 unique stacks / 131,072 stored
frames; reaching that budget omits additional stack observations while retaining
allocation/free operations. Resolved function/file strings have a 4 MiB retained
budget (also 4 MiB in the lookup cache). Check omitted-stack, missing-symbol and
symbol-budget fields in `quality.memory`; incomplete stacks never imply complete
function coverage.

## Host SDK action

Keep an `Option<UiTraceHost>` in the host application. In its Trace menu/button
handler initialize it once and open the shipped adjacent analyzer:

```rust
use vesta_gpui_winit::UiTraceHost;
// App field: trace_host: Option<UiTraceHost>
if trace_host.is_none() {
    trace_host = Some(UiTraceHost::new()?);
}
if let Some(host) = &mut trace_host {
    host.open_default_analyzer()?;
    // Or host.open_analyzer(an_explicit_packaged_executable)?;
    // An authorized local Agent can attach using host.descriptor().
}
```

Retain this owner until the event loop returns, then drop it during host
retirement. It owns its helper thread and descriptor. Creating it starts no
recording; ordinary hosts that never create it have no helper/listener. A host
already launched by the tool reuses the environment-owned endpoint. The analyzer
is a separate process, and no shell/PATH lookup is used for the SDK entry.

## Agent acquisition and analysis

Discover both contracts:

```powershell
.\target\debug\vesta-trace.exe --capture-schema
.\target\debug\vesta-trace.exe --schema
```

Start one persistent `vesta-trace.exe --capture-stdio` process with piped stdin
and stdout. Send one JSON request per line and read its response before the next
dependent command. Requests use `schema_version: 1` and a request ID. For example:

```jsonl
{"schema_version":1,"request_id":"attach","command":{"op":"connect","descriptor":"C:/private/session.json"}}
{"schema_version":1,"request_id":"start","command":{"op":"start","duration_seconds":10,"max_events":100000,"detail":"stages"}}
{"schema_version":1,"request_id":"progress","command":{"op":"status"}}
{"schema_version":1,"request_id":"save","command":{"op":"stop"}}
{"schema_version":1,"request_id":"detach","command":{"op":"detach"}}
```

For tool-launched capture replace connect with
`{"op":"launch","executable":"D:/MyApp/app.exe","arguments":[]}` and poll
status until idle before starting. The capture process remains interactive and
responds on stdout with JSON only. EOF retires the controller; finish/save active
work explicitly first. Limits are 16 KiB/request, 1–300 seconds and 1–200,000
events, with bounded target arguments and snapshot transfer. One process-local
capture is active at a time; GUI and Agents coordinate access to that recorder.
Optional `"memory":{"max_events":100000,"stack_frames":32}` on `start` enables
heap capture; omit it for CPU/GPU only. The memory event budget is independently
bounded to 1..200,000; stack depth is 0..32 (zero omits stacks). Check status
`memory_supported` before requesting it. A memory limit also prompts the native
controller to stop, save and analyze the bounded prefix.

Stop returns `result.data.path`, `source.sha256`, counts and quality diagnostics.
Use the saved path with `vesta-trace.exe <path> --query-stdio` to query session
quality, slow frames, hotspots, callers/callees, counters and GPU submissions.
Check the source hash matches the acquisition response. See
[AGENT-QUERIES.md](AGENT-QUERIES.md) for typed queries, paging and budgets.

## Coverage

Current data includes instrumented CPU elapsed scopes, frame relationships,
counter observations and supported Vulkan submission timestamps. CPU/GPU clocks
are uncalibrated. Optional memory recording covers **observed Rust System global
allocator lifetimes**, with shared capture-local CPU/memory thread identities
and available thread names. Pre-capture and recorder-excluded blocks, native
library/driver/GPU heaps are outside its baseline. A surviving block is not
automatically a leak. Query `quality.memory` for the last known instant, limits,
hook errors, unknown frees/reallocations and stack resolution. Full-function
sampling, async task links and GPU resource lifetimes remain future work in
[decision 0043](../../docs/decisions/0043-Native-Insights-Workflow-And-Agent-Capture.md).
This workflow is the foundation of Insights alignment, not complete parity.

## Maintainer live acceptance

`--live-smoke` exercises the actual native capture hit/action routes with a
finite SDK host, automatic limit, repeat stop/save/analyze and save-on-close.
The analyzer window stays hidden, without desktop input:

```powershell
.\target\debug\vesta-trace.exe --live-smoke --target=D:\VestaGPUI\target\debug\vesta-motion.exe --target-arg=--hidden --target-arg=--observe-seconds=12 --output-dir=D:\captures\new-live-test
```

The normal `--inspect-smoke` / `--inspect-capture=<new-image-directory>` scene
also exercises the Frames table's CPU/GPU/time orders, frame and GPU drill-down,
paging and minimum layout. CPU-only sources retain explicit missing GPU timing.
`tools/verify/trace_frame_timings.py` independently checks JSON source arithmetic,
complete paginated orders and generated schemas; it is a maintainer oracle with
a 20,000-frame admission bound, not a product analysis path.

`--memory-smoke` accepts the same finite hidden target arguments and optional
`--smoke-capture=<new-directory>`. It exercises the real native Memory toggle,
recording, import, allocation/stack drill-down, A–B cohort controls and minimum
layout. `tools/verify/trace_memory.py` reconciles original heap operations with
independent integer lifetime arithmetic, all three complete paginated allocation
orders, cohorts, curves and generated schemas. `--source=<capture>` checks an
existing capture; `--host=<finite-Motion.exe>` also tests Agent acquisition and
a repeated capture with the heap channel off.
