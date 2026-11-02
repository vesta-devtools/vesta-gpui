# Capture-owned GPU submission evidence

Status: Accepted; scoped capture/native qualification complete; full delivery active.
Date: 2026-11-02.
Extends: [0034](0034-Native-Trace-Analysis-Application.md),
[0035](0035-Agent-First-Trace-Evidence-Queries.md).

Keep the existing GPUI CPU trace schema 2 and optional standard event export.
Add an optional, typed `gpui_capture` extension, schema 1, to JSON and the same
HTML data block. Core owns only portable capture facts and validation of timestamp
arithmetic; it never owns a graphics object, query pool, scheduler or file I/O.
The Vulkan renderer remains the sole owner of actual timestamp queries and their
existing nonblocking completion path. Winit still exports the frozen snapshot.

An active capture automatically opts presented renderer recordings into GPU
timestamps. Ordinary launch remains disabled. Each recording joins its actual
profiling generation/submission, capture-local window/thread and recorded CPU
frame key when one exists. Profiling generations identify a particular renderer
timestamp stream, not a persistent physical device or successful presentation.
Device/queue capabilities and hardware descriptions are evidence, not asset IDs.
Window identities are capture-local; native handles are never serialized.

The completion token holds its original capture owner. It cannot append to a
successor recording. Stop/expiry freezes pending observations; late completion
cannot change exported facts. GPU records have a separate bounded budget derived
from the CPU event budget, with explicit loss. Unsupported, failed, pending and
measured-zero are distinct states. Readback does not wait on the UI thread.

Raw device ticks, valid bits, period, measured elapsed time and observed completion
upper bound stay available. CPU recording timestamps associate a submission with
its host attempt, but are not a GPU execution-start estimate. There is currently
no calibrated CPU/device clock mapping. Display durations separately; do not
overlay them on a fabricated shared clock or call them presentation latency.

The independent analyzer admits this extension strictly, including references,
uniqueness, capabilities and bounded-wrap arithmetic. Old CPU-only inputs retain
their coverage. Machine queries and native views use one imported evidence model,
source-addressed keys and the existing cancellation, paging and work limits.
Preserve original acceptance; new real timestamp evidence is required for VT05.

Native qualification increment (2026-11-02): reuse the standard Standalone's
existing two-window Surface/Device recovery fixtures with automatic capture.
An explicit private `--smoke-hidden` requires `--smoke` and keeps both windows
hidden/nonactive; it does not change ordinary smoke or user window policy.
Invisible-window redraws use the existing host event path with a fixed 30-second
fixture deadline and normal shutdown; native paint events alone are insufficient.
Capture real command timestamps and completion outcomes before/after controlled
fault notification, verify source/frame/window/generation joins through the
machine interface, and restore the ordinary standard build after feature tests.
Injected notification is not physical device-loss evidence or calibrated timing.
Scoped results and retained failures are recorded in [TRACE-PLAN](../TRACE-PLAN.md).
