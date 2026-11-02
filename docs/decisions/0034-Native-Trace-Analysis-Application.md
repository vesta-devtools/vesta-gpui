# Native trace analysis application

Status: Accepted; implementation and acceptance remain in progress.
Date: 2026-11-01.
Base: 5260c71452c51cc9d2b8d3b6edbb8fff905a2f33.

The user requests a professional trace analyzer with a GPUI frontend, operated
as an independent native process. Build `apps/vesta-trace` in this repository,
using its normal `target/debug/vesta-trace.exe` entry. It is an application,
not a ninth framework library or a replacement runtime for the existing hosts.
Keep the eight-library production direction and immutable consuming-product
SDK policy. Do not introduce a browser-based product frontend.

The existing recorder owns captured facts and frame/parent identities. The
viewer reads versioned GPUI trace JSON or the embedded data in its offline HTML
export. Parse data without executing report scripts. The original files remain
unchanged. Unknown schemas, invalid identities, clocks and numeric ranges are
structured failures. Loss, clipped scopes, invalid nesting, unframed events and
unavailable GPU clocks remain visible, never inferred as successful work.

The application initially uses private modules for typed session import,
time-range analysis and native presentation. Analysis must remain independent of
the UI and native renderer. It derives views from captured events, retaining
stable event/process/thread identities and the producer's frame relationships.
The producer's per-frame summary remains a compatibility projection, not a second
editable session. Arbitrary-range analysis clips intervals and subtracts the
union of direct recorded children on the same thread; inclusive totals overlap
and self time still includes waits and uninstrumented work.

The native application owns asynchronous file I/O, cancellation/generations,
selection, navigation, filtering and analysis jobs. It draws only visible tracks
and events, using current geometry for both picking and paint. A zoomed-out
display may aggregate dense pixels while preserving counts and drill-down.
Neither capture nor analysis may silently become an application scheduler.
Keep normal shared controls, text/IME, focus, actions and resource retirement.

GPU timing remains renderer-owned. Join supported samples through explicit
device-generation/submission identities. Place them on a CPU clock only with
validated clock correspondence and disclosed uncertainty; uncalibrated device
durations belong on a separate domain. CPU submission timestamps are not GPU
execution timestamps. Do not invent cross-thread/task links from overlapping
time ranges. Full-function sampling, memory/context-switch profiling and complete
Unreal Insights suite parity are outside this finite UI-analysis delivery.

Record the VT01–VT06 scope and acceptance cursor in TRACE-PLAN.md. Keep existing
HTML/Chrome exports compatible; Perfetto is an optional interoperability check,
not the native product frontend. Finish with real-file and corruption contracts,
bounded large-session measurements, independent native navigation and selection,
actual host integration, truthful timing coverage and process/source identity.
No original physical/Reader/DPI gate is reopened or signed by this new viewer.
