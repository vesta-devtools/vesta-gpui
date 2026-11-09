# Native Insights workflow and shared Agent capture

Status: Accepted; staged implementation in progress.
Date: 2026-11-09.
Supersedes the product directions in [0041](0041-Integrated-Perfetto-Capture-Tools.md)
and [0039](0039-Bundled-Third-Party-Trace-Tools.md).

The user chooses the self-developed analyzer, with both the professional analysis
capabilities and interaction of Unreal Insights as the reference. The host must
open Trace through GPUI, capture and analyze from the native tool, and offer an
equivalent workflow to Agents without screen automation. The completed earlier
finite analyzer qualification does not establish this larger scope as complete.

Keep `vesta-trace` native by default. Retain Perfetto only behind the explicit
`perfetto` feature and `--perfetto` switch as an optional technical proof. Reuse
the existing recorder, immutable source identity, range analysis and query engine.
Do not create an alternate Agent analysis implementation.

`UiTraceHost` owns an on-demand local capture endpoint. A host Trace action opens
the adjacent analyzer with its private descriptor; it starts no capture until
requested. `UiTraceRecording::from_arguments` remains the launch-from-tool entry.
The native analyzer performs target IPC, capture transfer and saving on an owned
worker, then imports the saved file through the existing analysis pipeline.
Compact capture controls stay in the title strip so analysis uses the workspace.

`--capture-stdio` adds a separate, versioned JSONL acquisition contract for
launch/connect/status/start/stop/detach. `--capture-schema` discovers its schema.
Stop returns the saved path, source SHA-256 and quality evidence; the existing
read-only `--query-stdio` contract analyzes that same file. Bound requests, event
counts, duration, target arguments and transfer size. Capture is process-local,
opt-in and single-active; do not inject into arbitrary processes or kill hosts.

Professional coverage must be delivered and qualified in stages:

| Stage | Exit requirement |
| --- | --- |
| IN01 Host and Agent capture | Real host Trace action opens the native tool; repeated start/stop/save/analyze; finite limit; normal close/retirement; Agent reproduces capture and source-addressed analysis. |
| IN02 Timing workspace | Frames/CPU threads/GPU evidence, range-linked inclusive/self/call statistics, callers/callees, counters and slow-frame navigation; discoverable keyboard interaction, dense tracks and full analysis workspace. GPU clock calibration or its absence is explicit. |
| IN03 Execution relationships | Named thread/process metadata and recorded async/task relationships; trustworthy CPU/GPU association; optional stack sampling/symbolization has explicit platform, overhead and coverage evidence. |
| IN04 Memory diagnostics | Opt-in allocation/free and resource lifetimes, baseline/unknown coverage, live bytes/count curves, interval comparisons and allocation stacks or tags. Shared bounded Agent queries and real-host reconciliation. RSS alone does not pass. |
| IN05 Professional delivery | Measured disabled/enabled overhead, bounded long/large captures and loss, lifecycle/failure/accessibility checks, packaged host integration and reproducible real CPU/GPU/memory diagnoses. |

Reference behavior is documented in Epic's [Timing Insights](https://dev.epicgames.com/documentation/en-us/unreal-engine/timing-insights-in-unreal-engine)
and [Memory Insights](https://dev.epicgames.com/documentation/en-us/unreal-engine/memory-insights-in-unreal-engine).
These define workflow targets, not a claim of present feature equivalence. Current
recorded CPU scopes are elapsed intervals rather than sampled CPU utilization;
GPU clocks remain uncalibrated. The first opt-in Rust heap lifetime/stack channel
and shared native/Agent diagnostics are implemented under [0045](0045-Capture-Owned-Heap-Lifetimes-And-Stacks.md);
this is not full native/GPU resource tracking or system stack sampling coverage.
