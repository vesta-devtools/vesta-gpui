# Bounded in-process UI tracing

Status: Accepted; implementation and qualification recorded separately.
Date: 2026-10-30.

Extended by [ADR-0004](0004-Frame-Capture-And-Per-Frame-Analysis.md). This recorder
alone is the transport foundation, not the completed frame-analysis workflow.

Provide one process-local, explicitly started diagnostic capture in the base UI
library. UI-facing packages and their hosts use the same recorder. Independent
workspace/viewport models retain their dependency boundaries; hosts can scope
calls into them without adding a UI dependency to those models. Starting a
capture does not require ETW, elevation, a service or a new runtime. Instrument
shared layout, text, paint, hit-test and renderer preparation boundaries. Hosts
can add static named scopes and explicit retry/skip markers around their own
tree construction and frame settlement. A library cannot infer host work that
happens before entering it, nor distinguish an application skipping render from
an idle application without such host markers.

Disabled instrumentation returns before reading the clock or taking a lock.
Active capture is bounded by event capacity and elapsed duration. It stops
accepting events at its deadline without creating a timer thread. Scope guards
belong to their original capture and thread: late drops cannot enter a successor
capture. Stopping clips open scopes and explicitly marks them unfinished. Full
buffers retain the prefix and count losses; they do not silently claim complete
coverage. Dropping a capture disables it without writing anything.

Record elapsed wall-clock intervals, not CPU utilization or sampled call stacks.
Use the existing Vulkan CPU-stage boundaries for timeline intervals without
changing its opt-in FrameCpuProfile or scheduling. GPU timestamp observations
remain the existing separate opt-in, generation/submission-qualified API; CPU
submit time is never labelled GPU execution time. Do not add waits, readbacks,
GPU queries or a second timing authority merely because CPU tracing is active.

Export Chrome Trace Event JSON compatible with Perfetto. The caller owns the
destination and file I/O. No automatic uploads, global file paths, user text,
document contents, screenshots or window titles are recorded. Labels are bounded
static strings; counters are finite numeric values. Thread lanes use capture-local
IDs, not inferred OS thread IDs. Explicit process ID identifies the export.
Capture data is transient diagnostic state, never author data or a cache input.

Keep the public surface additive. A headless example and focused cross-module
contracts verify automatic stages, lifecycle isolation, truncation, deadlines and
valid export. Timing thresholds and synthetic traces do not qualify native
latency, the live Agent executable or the reported divider defect. Consuming
products must update the fixed eight-package revision, integrate start/stop and
host scopes, and rebuild through their existing owner before claiming deployment.
