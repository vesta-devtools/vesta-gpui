# Native collection capture and idle measurement

Status: Accepted; bounded fixture implemented, programme delivery remains separate.
Date: 2026-10-31.

GF05 still requires native capture cost, bounded collection retention and real
idle observations. The existing reflow fixture continuously schedules a 2 ms
driver and always captures stages. It cannot prove ordinary idle behavior or
compare tracing disabled/enabled. Do not reinterpret its historical results.

Extend the same explicit hidden standard Standalone fixture with one collection
workload in off/stages/detailed modes. Use the production Gallery and renderer,
the existing shared recorder, public snapshot protocol and lifecycle counters.
Keep 100,000 stable model items, a fixed repeated scroll sequence and bounded
Main/Float submissions; retain raw per-call/request wall samples and node counts.
No OS input, provider, author data, second renderer, thread pool or copied exe.

After settling, the fixture relinquishes its active driver to the ordinary idle
deadline chain, adding only its owned end deadline. Compare real lifecycle
snapshots before/after the quiet interval. An external owned process observer
may sample the exact native PID/creation time for CPU, memory and handles.
These are process/capture measurements, never GPU/display/input latency.

Off mode uses an existing disabled UiTraceRecording and creates no trace file.
All modes keep the existing isolated new HTML destination argument and validate
it before creating a window. Native measurements and summaries are bounded JSON
records on stderr. Trace modes export after normal shutdown. Use balanced mode
orders, exact source/font/executable identities and identical work signatures;
preserve failures and raw/noisy samples. Do not infer universal absence of leaks
or close product adoption/other consumer gates from this fixture alone.
