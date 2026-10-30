# On-demand stage capture and deferred export

Status: Accepted; implementation and qualification recorded separately.
Date: 2026-10-30.

The first interactive Standalone capture filled 100,000 events at approximately
6.5 seconds, before a 30-second user interaction could be completed. Preserve
that loss-reporting evidence and existing scope instrumentation. Do not address
this by silently increasing an unbounded buffer or removing useful common hooks.

Provide an explicit stage/detail selection in the shared collector. Existing
TraceCapture::start keeps its detailed behavior. A stage capture omits high-rate
intrinsic measurements and individual static/styled shaping scopes, while keeping
frame, layout, actual input refresh, paint, routing, accessibility and renderer
phases. Those parent intervals still include omitted work; self time remains
unattributed wall time, not exclusive CPU time. Exports record the selection.
Detailed hooks return before locks/clocks when detailed recording is disabled.

Extend the native adapter's existing recorder with an explicit start operation,
stats and stop-to-export handoff. Stopping freezes the shared snapshot; exporting
owns only that snapshot and the explicit non-overwriting destination and may run
on the host's background I/O worker. Reuse the same export implementation for
launch recording and interactive hosts. No framework-owned thread, automatic
upload, separate function table or second event collector is introduced.

Hosts own their menu/shortcut, output policy, progress display and worker lifetime.
They may stop on deadline or capacity and must show losses. Starting from the
settled target window avoids consuming the useful budget on launch/inspection.
The consuming Agent records one frame entry and a few host-owned construction,
divider and retry reasons; it reuses common GPUI stages without duplicating them.
