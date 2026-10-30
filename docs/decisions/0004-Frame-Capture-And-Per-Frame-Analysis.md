# Frame capture and per-frame analysis

Status: Accepted; implementation and qualification recorded separately.
Date: 2026-10-30.

The user clarified the requirement as the Unreal Insights workflow: enable a
capture, collect frames, select one frame, inspect its recorded call hierarchy
and durations. ADR-0003's bounded timer recorder alone does not meet this goal.
Retain it as the event transport, not as evidence of completed frame profiling.
Do not replace this requirement with a system sampling profiler or a requirement
to annotate every host function. Epic's tracing developer guide likewise uses
predefined framework events/scopes and separate frame events (see the
[official guide](https://dev.epicgames.com/documentation/en-us/unreal-engine/developer-guide-to-tracing-in-unreal-engine)); it does not imply
automatic capture of every arbitrary native function.

Add an explicit frame scope at each host's existing CPU frame entry. All GPUI
scope/marker/counter events on that thread inherit the current frame and parent
scope automatically. Frame identity is capture-local and independent of author
IDs, rendered frames or GPU submissions. Early returns still close the CPU
attempt; open scopes clipped by a stopped/expired capture remain unfinished.
Worker threads without an explicit frame entry remain unframed rather than being
assigned to a coincidentally overlapping UI frame. Invalid scope nesting is
reported, not silently treated as a trustworthy call tree.

One shared analyzer owns frame enumeration, recorded call hierarchy, per-label
call counts, inclusive wall time and self/unattributed wall time. Self time
subtracts the union of recorded same-thread direct-child intervals, clipped to
the parent. It includes uninstrumented work and waits; it is not exclusive CPU
time. Preserve incomplete/lost-event warnings and do not infer native display or
GPU execution from CPU scopes or successful submission.

Provide an offline, self-contained HTML report from the same analyzed data used
by the JSON export. The report offers frame selection, timing overview and
expandable call trees. It makes no network requests, executes no input-derived
code and uses text nodes for labels. Escape embedded JSON to prevent script-tag
termination. Do not introduce a second analyzer in the viewer.

Native hosts can use one shared opt-in recording launcher plus one scope at each
CPU frame entry. SDK examples use the same entry and export path. File I/O stays
in the native adapter's explicit post-run export, outside the base recorder and
the active UI event loop. Existing product applications require coordinated SDK
adoption and rebuild; do not close the user's active Agent for this work.

This provides the framework's recorded scope tree, not every Rust/native function,
sampled stacks or a .utrace-compatible file. GPUI maintains its common scopes;
additional business instrumentation remains optional. Qualify hierarchy,
accounting, truncation and export with real headless/native source paths while
keeping synthetic workloads distinct from the original divider latency defect.
