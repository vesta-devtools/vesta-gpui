# Vesta Trace delivery plan

Requested and started: 2026-11-01. Status: Finite VT01–VT06 delivery qualified, 2026-11-03.
Decision: [0034](decisions/0034-Native-Trace-Analysis-Application.md).
Agent evidence contract: [0035](decisions/0035-Agent-First-Trace-Evidence-Queries.md).
GPU capture ownership: [0036](decisions/0036-Capture-Owned-Gpu-Submission-Evidence.md).
Native semantic/input boundary: [0037](decisions/0037-Native-Trace-Accessibility-And-Evidence-Focus.md).
Native work lifecycle: [0038](decisions/0038-Transactional-Native-Trace-Work-And-Cancellation.md).

## Current direction: native Insights alignment (2026-11-09)

The latest user decision is to continue self-developed native Trace, aligning
professional analysis and interaction with Unreal Insights. The host opens it
through GPUI for capture/save/analysis, and Agents use versioned acquisition and
the existing shared evidence engine. [Decision 0043](decisions/0043-Native-Insights-Workflow-And-Agent-Capture.md)
defines IN01–IN05. These larger exits are **in progress**, independently of the
earlier qualified finite VT01–VT06 scope.

IN01 implementation restores native default, adds `UiTraceHost` and Motion's
Trace action, an off-UI capture worker and `--capture-stdio`/`--capture-schema`.
Local IN01 foundation qualification is recorded at
`target/research/native-insights-20261109/qualification.json`, with relocated
development tools in `target/distribution/vesta-trace-native-dev-20261109-02/`.
Motion's actual Trace action opens the native analyzer on PID 63032; repeated
manual captures preserve 615/1106 frames and GPU submissions, and 10-second
limits automatically save/analyze 595/578-frame captures. Closing/reopening the
analyzer preserves the host and reconnects. A real Agent capture preserves 59
CPU frames and 58 GPU records; its expired capture is not stolen by the GUI.
Four native captures have 32 schema-validated query messages, plus 14 Agent
acquisition/query messages with matching source hashes. The current executable
also passes the five-step native and nineteen-step inspection/GPU scenes.

SDK lifecycle tests cover authorization, budgets, repeated capture, expiry,
on-demand ownership and retirement. The Agent integration test covers failed
save/retry without data loss, event-limit status, active-detach rejection and
source identity. Trace's 21 all-feature unit tests, two Motion tests, three SDK
trace lifecycle tests, two Agent workflow tests, strict Clippy and the eight-
library architecture check pass. Actual interactive startup exposed and repaired
the AccessKit-before-first-show ordering that hidden smoke did not cover.
Direct active-close qualification was interrupted by user input; automation
stopped and preserved the user's active windows. This remains distinct from the
implemented best-effort save-on-close path.

CPU/GPU clock calibration, system stack sampling and allocation/free memory
diagnosis remain open under IN02–IN05. Perfetto is optional proof only. This is a
local development qualification, not complete Insights parity or a source release;
the workspace contains concurrent changes and has not been sealed for publication.

IN02 frame-comparison work has local qualification: analysis version 5 introduces the
shared `frame_timings` operation and native Frames tab. It groups whole GPU
submissions by actual CPU frame identity, retains null versus zero and sample
coverage, supports CPU/GPU/time ordering, paging and raw-submission navigation.
The CPU range selects whole frames, with original overlap retained; GPU spans
are not clipped to a CPU interval. Focused contracts cover cross-window work,
partial/missing/zero samples, frame/text filtering, producer loss, cursor identity,
budget/cancellation, native/query equivalence and overflow rejection.

Evidence is in `target/research/native-frame-timings-20261109/`. The current
twenty-five-step native inspection covers minimum layout, CPU/GPU/time ordering,
source-addressed frame selection, longest-submission drill-down and paging; a
CPU-only 808,000-event source also passes the applicable native steps. Independent
Python integer arithmetic verifies all 615 real frames and all 8,000 synthetic
frames in three complete paginated orders, with 60 schema-checked messages.
Measured query-analysis costs in these local oracle runs peak at 1.236ms and
31.362ms respectively; the larger run's 57.9s wall time includes the independent
Python/schema work and import, not just product query cost. The first verifier
watchdog attempt failed because it counted that independent work while the
child waited on stdin; the corrected per-response watchdog run is preserved
separately from that failed attempt.

The actual native live-control scenario passes six command-origin stages:
automatic 2-second capture/save/analyze, repeated manual stop/save/analyze,
default Frames publication and close while recording. Three real Motion
snapshots preserve 107/17/32 CPU attempts and 98/18/32 GPU records; the third
snapshot is saved by the owned worker during analyzer retirement. The counts
remain separate because startup/unframed/in-flight work is not invented into
one-to-one CPU/GPU records. This closes the previously deferred direct active-
close check without operating the user's visible windows. IN02's wider timing
workspace and IN03–IN05 remain active; clock calibration, execution relationships,
stack sampling and allocation/free diagnosis are still open.

The local development package `target/distribution/vesta-trace-native-dev-20261109-03/`
has a per-file manifest and analysis-5 schemas. Its exact analyzer
`4f7aaadd45f4209e899774183b936ae07aa188ed6f420c7fbb5d381e402f4b0e`
passes the final GPU, CPU-only and live-capture scenarios in `final/`; the
GPU run includes 25 framebuffer images, and the live run saves its active capture
on close. Ninety schema-checked oracle messages cover the original real/large
inputs plus all three new live snapshots. Twenty-four all-feature unit tests,
two acquisition integration tests, strict all-target Clippy (including the
optional Perfetto feature), formatting, whitespace and eight-library architecture
checks pass. The per-scope evidence and explicit remaining work are summarized
in `target/research/native-frame-timings-20261109/qualification.json`.

## Observed heap lifetimes and native memory workspace (2026-11-10)

The IN03/IN04 foundation now includes explicit `UiTraceAllocator` installation
in Motion, capture-owned allocation/free/realloc operations, named shared
CPU/heap thread identities and Windows allocation-stack addresses. Local symbols
resolve after freeze on the I/O worker. CPU-v2/GPU-v1 remain compatible; the
optional `gpui_memory` extension is schema 1. Query analysis 6 shares immutable
allocation lifetimes, source-bound paging, time-point queries, stack grouping,
A–B cohorts and extrema-preserving byte/count observations with the native
**Memory** workspace. A successful realloc creates a linked new lifetime; failure
keeps the original. Pointer range overlap, malformed metadata and invalid times
reject. Baseline and later out-of-prefix totals remain unknown.

The heap recorder bounds operations to 200,000, depth to 32, unique stacks to
8,192 and stored stack frames to 131,072. Retained symbol strings and the lookup
cache each have a 4 MiB bound, with at most 4,096 address lookups and a cooperative
two-second resolution budget. Stack/symbol omissions are explicit and do not
remove lifetime operations. The global allocator forwards System operations
exactly; capture is independently opt-in. Core remains safe Rust; only the narrow
native adapter admits documented allocator forwarding/current-stack FFI.

The local development package
`target/distribution/vesta-trace-native-dev-20261110-02/` includes both executables,
matching debug symbols, schemas and guides. Its analyzer is
`7a79b39018374e9504d10e564cfa6e793f0e18ba27d0bbdeabed84b59d018f71`;
Motion is `de47d7586addff8485dcb47328be607d4e4bd13b984c1ca026222df4c313333d`.
The exact packaged analyzer passes thirteen real native memory stages (ten
framebuffer images) and the existing twenty-five timing inspection stages.
The A–B range is selected through actual input routes using busy-work instants;
it contains 25,138 transient lifetimes, with allocation/stack and cohort checks.
The test's hidden repaint cadence now includes pre-import controls; its existing
45-second deadline remains. A finite warm-up separates running-host qualification
from startup allocations before asynchronous GPU initialization.

`target/research/native-memory-20261110/qualification.json` collects the scoped
receipts. GUI and Agent captures each preserve 100,000 original operations, with
55,181/55,803 lifetimes respectively. They retain 15 CPU/14 GPU and 56 CPU/56 GPU
records; these counts are not forced into one-to-one associations. Independent
integer arithmetic validates three complete paginated orders, cohorts, actual
series extrema, raw stack evidence and generated schemas. A separate startup
capture retains 53,964 lifetimes, explicit stack/symbol budget exhaustion and
absent GPU evidence; it passes heap reconciliation without being signed as a
joint CPU/GPU workload. The Agent also repeats capture with memory off.

Twenty-nine all-feature analyzer contracts and two acquisition tests pass; the
updated five memory contracts also pass after strict pointer-range validation.
Two actual-allocator tests cover zeroed allocations, successful realloc,
cross-thread free, surviving blocks, shared thread IDs and local caller symbols.
Explicit diagnostic simulations cover reentrancy, panic-result preservation,
event/time caps and stack-budget exhaustion; no real OOM is forced. Two directed
core trace tests, strict all-target/all-feature Clippy, formatting, whitespace and
the eight-library architecture check pass. Raw startup and failed fixture
attempts remain distinct from the qualified warmed-host runs.

The recorded sample's stack groups identify allocation turnover in
`painter::prepare`, `ScenePainter::record` and Motion's draw path, with original
stack keys retained in `final02/diagnosis.json`. Turnover is not process-wide live
memory or a leak claim. Calibrated CPU/GPU execution, async/task relationships,
system stack sampling, native/driver/GPU resource lifetimes, richer memory
interaction and measured enabled/disabled/long-capture cost remain open under
IN02–IN05. This shared workspace is unsealed; the package is local development
evidence, not an immutable SDK release or completed Insights equivalence.

## Previous integrated Perfetto proof (2026-11-09)

[Decision 0041](decisions/0041-Integrated-Perfetto-Capture-Tools.md) records the
user-selected Perfetto path and integrated capture workflow. The tool must launch
or attach to a GPUI application, start/stop recording from its own controls, save
the result and load the embedded timeline. An offline viewer alone is insufficient.
Keep the old GPU evidence and Agent queries through the native details entry.
See the [tool guide](../apps/vesta-trace/PERFETTO.md) for build, packaging and use.

The implementation adds an opt-in target control worker, private session
descriptors, per-run loopback resource transport and pinned offline UI assets.
Local live qualification has recorded two real Motion captures with 100 frames /
100 GPU submissions each, exact Perfetto span/instant count agreement and normal
disconnection. Final software, packaging and interaction checks are recorded below
when complete; this does not claim system sampling or calibrated GPU tracks.

## Previous bundled-tool direction (2026-11-06)

[Decision 0039](decisions/0039-Bundled-Third-Party-Trace-Tools.md) sets the next
direction: integrate a mature profiler, with Tracy as the preferred first
candidate, and bundle its compatible analyzer with the GPUI SDK/developer tools.
Consumers should use one provided launch/connect entry without a separate trace
tool installation or PATH configuration. Keep capture opt-in and prioritize
integration and packaging over expanding the self-developed generic analysis UI.
Implementation, compatibility, migration of existing GPU/Agent evidence and
clean-environment distribution qualification are pending.

## Completed native analyzer delivery

Deliver a professional native GPUI trace analyzer as an independent application.
The finite release focuses on UI performance diagnosis: locate a slow frame or
range, inspect recorded work on each thread, follow callers/callees and counters,
and associate supported renderer timing with its actual submission. Preserve
the distinction between elapsed CPU intervals, GPU duration and presentation.
The primary differentiator is Agent-friendly acquisition and analysis: versioned
typed queries must retrieve bounded, source-addressed evidence directly, using
the same analysis as the native viewer. A GUI-only analyzer does not pass.

| Gate | Required exit | Current state |
| --- | --- | --- |
| VT01 — Session and ownership | Standalone application boundary; versioned JSON/HTML data import; strict identity/clock/numeric admission; visible loss/nesting/unframed diagnostics; unchanged original input | CPU-v2 JSON/HTML plus optional capture-schema-1 GPU admission qualified. Corrupt clock/identity/frame joins and invalid timestamp/wrap arithmetic reject; original byte provenance is retained. |
| VT02 — Native timeline | Real GPUI window with frame overview, ruler, multiple thread tracks, nested slices and counters; zoom/pan/fit; accurate event/range selection; filtering/search and keyboard access | Qualified. Navigation04 covers committed/preview range, zoom/pan/fit/resize. Focus05 adds table/track keyboard reachability, search ownership, busy logical-focus retention, paging-preserving view scroll, and source-bound platform-node/pointer retirement. GPU keyboard and shared assistive text/actions retain scoped evidence; final audit below preserves those identities. |
| VT03 — Analysis | Deterministic range clipping; calls/inclusive/self totals; caller/callee hierarchy; frame distribution and slow-frame navigation; cancelled/stale work never publishes; versioned Agent queries with stable evidence drill-down | Qualified. Seventeen contracts remain current. navigation04 adds 38 schema-validated queries comparing actual presented native ranges/totals/whole-frame percentiles against independent original-file arithmetic. Preview and committed reports stay distinct. Caller/callee, source-key and stale-job evidence retains its recorded scope; final audit below also joins actual consumer diagnosis. |
| VT04 — Scale and lifecycle | Bounded off-UI import/analysis and cancellation; visible-row/event rendering; large-session correctness/cost evidence; stable selection and focus across resize/reload; normal renderer and worker cleanup; bounded machine requests and source/query-bound paging | Qualified. Indexed projection and the unchanged 808,000-event fixture retain cost evidence. Lifecycle03 covers missing/malformed input, observed cancellation/stale replies, source-bound rollback and pending close. Panel-import01 repairs and verifies pending-panel intent across reload with fifty focused images. Final-software02 and final-native01 close local checks and current executable cleanup; final audit below preserves synthetic cost limits. |
| VT05 — Capture correlation | Named frame/window/thread tracks; explicit cross-thread relationships when recorded; genuine device/submission samples and clock-domain handling; unsupported capabilities remain explicit | Qualified. Automatic capture-owned timestamps and CPU frame/window/thread joins retain original 25-sample evidence; gpu-multi-recovery02 adds 80 actual samples across two windows and controlled Surface/Device restoration, 160 exact high ticks and 45 machine queries. Window identity stays stable as renderer timestamp generations change. CPU/device clocks remain uncalibrated and unavailable relationships explicit; final audit below retains this scope. |
| VT06 — Consumer delivery | Agent/Builder capture-save-open integration using the independent analyzer; discoverable generated machine schemas and real-capture Agent diagnosis workflow; current source and standard executable identities; actual native acceptance and all affected software/feature checks; documentation and cleanup | Qualified at immutable SDK 3bd9798 with exact successful CI, eight-pin adoption, guarded standard two-host builds, six actual admission/ownership cases, four real-capture Agent diagnosis workflows / sixty query pairs, and actual native capture-save-Open from both hosts. Final audit below closes the finite delivery; preserved failures remain failures. |

These gates are independently reported. Completing a parser, a mock timeline or
an automated scene does not complete the application or GPU/host integration.
Do not mark the whole goal complete until all six exits have evidence.

## Acceptance inputs

Keep the existing Motion report at
`target/research/frame-trace-20261030/run02/motion.frames.html` unchanged. The
recorded Agent/Builder exports in the consuming product are additional real
inputs, not newly collected performance measurements. Synthetic sessions cover
large tracks, numeric/corrupt inputs and overlap/clipping; label them accordingly.
New native tests use the standard analyzer binary and identify their command
origin. Physical input, reader output and cross-monitor DPI retain prior status.

## Current cursor

Agent-friendly acquisition and analysis is the first release acceptance workflow:
discover the host capability, acquire and finish a bounded capture, obtain the
actual artifact and its SHA-256, inspect coverage, narrow slow frames/ranges,
and follow stable keys back to original evidence through bounded queries. This
workflow must succeed without screenshots, UI clicks or dumping the whole capture
into a model context. Native viewing uses the same analysis; it does not replace
this machine contract. No new model provider or separate analysis engine is needed.

All six finite gates now have qualified evidence. The final audit joins the
unchanged VT01–VT05 source/native evidence to actual immutable delivery and
consumer runtime qualification; it does not rerun previously passed scenes.
The current runtime authority is published SDK
`3bd9798ca7a1e8ee981728613ee14c48f364ee26`, exact successful Windows CI
41481525315, standard analyzer `794d379c`, and the consuming workspace's
`target/research/vesta-trace-consumers-20261103/completion01/qualification.json`.
The earlier remaining-work paragraphs below retain their original chronology.
This final documentation update does not change runtime bytes or the consumer
pin, and does not overwrite the original sealed rc.2 archive or its receipts.
Retain earlier evidence at its original source identity; use final-native01 for
the current standard analyzer rather than historical focus05 or GPU executables.

Final consumer evidence, 2026-11-03: software06 passes the related three-package
contracts (128 passed / four pre-existing ignored) and strict all-feature checks,
with 4,755 guarded inputs unchanged. build02 produces current standard Agent
`9624bb8f` and Builder `702a4568`. runtime-combined01 retains five completed
runtime02 cases and supplements only Builder external ownership from runtime03;
the controller's original host-envelope assertion failure remains preserved.
The earlier Agent launch-argument rejection exposed a missing SDK launch adapter
and was fixed before these builds and cases. No capture semantics were copied.

diagnosis-root01 performs sixty bounded, schema-validated query pairs on four
actual captures, independently checking clipped inclusive/self arithmetic,
whole-frame statistics, original events/relationships/counters, and 82 GPU
samples / 164 exact high ticks and CPU frame associations. native01 actually
opens the standard analyzer from each host and records ten genuine images in
total. Its 10 and 9 GPU samples plus one explicit Builder pending record retain
the raw outcomes. Workers, GPU owners and registered test processes retire;
the three standard executables pass exclusive-open/hash checks without writes.

The finite scope remains recorded-stage UI diagnosis with shared native/Agent
analysis. Full-function sampling, calibrated CPU/device clocks, physical GPU
failure, complete Unreal Insights suite parity, product-wide Agent/Builder
acceptance, Reader and cross-monitor DPI are not newly signed. Formal 0.1.0
tag/registry publication is separate from this completed implementation goal.

Earlier qualification history:
Evidence:
`target/research/vesta-trace-20261101/check02/qualification.json` and
`cleanup02.json`; four contract tests, strict app Clippy, three historical inputs
and five actual GPU captures passed at that source. This is partial evidence,
not six completed gates. New machine evidence is
`target/research/vesta-trace-20261101/agent06/qualification.json` and
`schema-validation.json`: nine contract tests, strict app Clippy, standard build,
three unchanged historical real inputs and a 136,578,336-byte synthetic input.
Four headless sessions each executed 15 queries and closed at EOF with exit 0;
the analyzer's standard executable SHA-256 is
`0b3562214944cfa052214f23e3dc063e5994d77d6195a6ad683f9d413ba8e481`.
Runtime source guard had no drift. Machine queries did not initialize a window
or renderer. Root's SDK pin and other owners' programs are unchanged.
`machine-edge.json` additionally records an actual 1 ms cooperative deadline
error followed by a successful query, versioned import failure and exact u64
identity preservation using a lossless Python client.

Disk-full recovery removed only two superseded PDB files of this thread's new
Trace tests and invalidated its one corrupt test incremental graph. Logs under
the same research root retain the failed attempts and exact cleanup paths.
Shared libraries/caches, original captures and native image evidence are intact.
The disk has subsequently regained space (about 35 GiB observed); no broader
cache cleanup was performed by this thread. Recheck capacity before broad builds.

## Local software closure and pending-panel import repair — 2026-11-03

The original 808,000-event input exposed a real regression during final cost
acceptance: importing while Diagnostics analysis was pending restored an older
Events panel checkpoint. `postbuild-native01/scale-current` retains its normal
exit 1 and renderer/worker retirement. ADR0038 preserves the latest panel intent
on successful import while cancellation/failure still restores the validated
checkpoint. `panel-import01/qualification.json` records seventeen contracts,
strict app checks, the ordinary build and fifty actual images across Lifecycle,
Focus, GPU inspection and the original fourteen-state large-input scenario.
The unchanged source SHA-256 starts `be1e86c1`; sixty-four projection scopes had
local elapsed-wall median 8.82235 ms (nearest-rank p50 8.6585 ms), nearest-rank
p95 12.8801 ms and maximum 14.2404 ms. These are observations
of that synthetic workload under uncontrolled load, not an FPS or speedup claim.

`target/research/vesta-trace-20261102/final-software02/qualification.json` records
the final local checklist: 253 Rust tests, two doc tests, seven Node contracts,
format, all-feature workspace strict checks, base-Vulkan strict checks, eight-library
architecture validation, all-target/all-feature builds and ordinary restoration
of all three app entries. All commands exited 0 and guarded inputs had no drift.
The current ordinary executable SHA-256 values are:

- Trace: `794d379cb53bbdbb0475a60863b716b67ca38dc30ccea30254b05fbba36f0c6a`.
- Standalone: `5abe4cc3a162e46eb8dad7fa7b3197ed92082ed8b765d8c2bfacdeb34f876b38`.
- Motion: `9fc10325bcbde98947bdf0ba5d52fef78812f9f2eb8417655ddedaf8bcb3bc96`.

`final-native01/qualification.json` binds the first two current executables to
twenty-four actual images, normal exits and resource retirement. Trace self-capture
has twenty-five GPU samples; ordinary Standalone automatically captures twenty-six
across its two windows. Motion has build qualification, not new native acceptance.
`final-native01/machine-verification/` checks forty-five bounded query pairs against
the original multi-window/recovery captures. The current executable's generated
schema remains schema 1 / analysis 4. The fifty focused images above remain
source-bound evidence rather than newly repeated scenes for this executable.

`final-native01/round-cleanup-reconciled.json` records seventy-three registered
current-round PIDs absent and all three matching executable hashes with exclusive
read/write opens without writes. Two October 19 processes whose stored parent
number matches a reused PID were explicitly excluded by creation time; no foreign
process was touched. The SDK updater/source hold is released. These are local
software/native qualifications: source sealing, actual public CI, immutable SDK
adoption and final Root host runtime delivery remain open. The goal stays active.

## Native inspection — 2026-11-01

`target/research/vesta-trace-20261101/native-inspection04/qualification.json`
records eleven contract tests, strict app Clippy, the standard build and two
14-state hidden native runs at executable SHA-256
`16416d682d220bee732c3ef8a06aff32501c62a795bb0ea2c033b2adf53520e2`.
Motion and the unchanged 808,000-event synthetic input each have fourteen actual
GPU images. Checks cover shared search text/Undo/shortcut ownership, current-layout
pointer selection, caller/callee/occurrence/tree/counter/quality views, forward and
reverse Tab plus Enter activation, stale generations, resize and source reload.
The large case additionally traverses next/previous source-bound query pages.
Both exited 0 with worker join and GPU retirement. This is command-origin evidence,
not a physical-input, Reader, DPI or GPU execution-clock signoff.

`native-inspection01` retains the original post-selection timeout; `02` retains
the subsequent hidden-resize timeout. Local geometry/focus settlement now schedules
a bounded retry. The finite hidden scenario explicitly polls its own completion
and resize while retaining the 45-second deadline. Ordinary idle remains Wait.

`self-profile02/qualification.json` records the successor standard executable
`850fec30121bfb1476643f63282a3e2566a5da89442b3bd65be2d7bdc2f7b90a`,
eleven tests/strict, five native images and a real opt-in self CPU capture read
back through four machine queries. Since native-inspection04 only an optional
projection phase scope was added; the existing interaction evidence is retained
at its original source identity. `view.project` has 22 actual spans totaling
681,575,800 ns, with its recorded caller `Trace.Main` recovered by the same query
engine. This is elapsed wall time in one profiled synthetic workload, not a CPU
utilization measure or a speedup comparison. No-frame native statistics display
absence rather than fabricated zero percentiles.

Keep source guards, captures and cleanup receipts alongside those reports. No
application started in these scenarios is authorized to remain running. Root's
eight SDK pins, standard Agent/Builder instances and the original 118 UI/Reader/
DPI ledger remain unchanged. The overall VT01–VT06 goal is still active.

## Indexed projection and counters — 2026-11-02

Presentation indexes retain source event IDs and exact visible counts. Time bins
select actual duration representatives, keeping carry-in intervals and half-open
point boundaries distinct. A source/text-bound immutable filter result is reused
across navigation; replacement and cancellation cannot publish another source's
rows. Range trees are also used for raw counter extrema. There is no new runtime
or framework library, and interval statistics still use all matching captured facts.

`target/research/vesta-trace-20261101/plot-performance01/qualification.json`
records the actual prior standard executable (850fec30) followed by its normal
rebuild (cf7e3afd), without executable copies. Both profiled the same 808,000-event
input, filter and five native states. Projection p50 was 33,227,100→9,679,500 ns;
p95 43,987,000→13,701,200 ns. Calls differed (22/23), system load was uncontrolled
and the new view uses explicit pixel aggregation; these are local observations,
not a general throughput or FPS guarantee. The raw data and loss metadata stayed
unchanged. `native-inspection05` then passed Motion and large-input fourteen-state
interaction runs at cf7e3afd, with thirteen contracts and strict app checks.

`target/research/vesta-trace-20261102/counters02/qualification.json` records the
current ordinary executable SHA-256
`87f44bfa9306f3fe01e46c9bdc63e063fea9e1d5df84e0c71979e085d3c64ea2`:
fifteen contracts, strict Clippy, standard build, fourteen actual GPU images of
three synthetic counter series (90,000 raw observations), normalized point picking,
resize/reload and retirement all passed. Extrema and first/last raw representatives
preserve spikes; guide lines do not imply interpolated observations. Counter detail
retains the selected original event/value. Esc restores an interval preview and
releases capture without changing committed analysis or queuing redundant work.

The current generated schemas independently validate four actual wire pairs.
`counter_series` returns bounded raw representatives, exact counts and extrema
with source event keys; frame/lane/text/exact-label filters and budgets are tested.
Analysis version 2 distinguishes unavailable percentiles (`null`) from a genuine
zero-duration measurement; a saved analysis-v1 cursor was actually rejected on
its unchanged original input. These changes extend Agent evidence acquisition,
not an alternative analysis implementation.

The successor GPU slice below replaces that round's cursor. Reuse completed
scenes at their recorded source identity; do not restart the original UI/Reader/
DPI ledger. Keep the persistent goal active until all six exits pass.

## Automatic GPU capture and Agent evidence — 2026-11-02

ADR0036 adds the portable capture-schema-1 extension while retaining CPU schema 2.
The existing recorder automatically opts renderer recordings into real Vulkan
timestamps. Their original capture owns completion tokens and bounded metadata;
stop/expiry freezes pending work, late completion cannot enter a successor, and
native window handles never enter the file. Core owns facts/arithmetic, Vulkan
owns queries and existing fence completion, and Winit still exports the snapshot.
No new worker, graphics owner, library or default-on recording is introduced.

`target/research/vesta-trace-20261102/gpu02/qualification.json` records three Core
trace contracts, one Vulkan timestamp contract, one Winit export contract,
seventeen analyzer contracts, strict Clippy across the four affected packages and
the ordinary standard build. Its real opt-in self capture from executable
`b41f6dcd9df8a23ba38c7c1b91218bef8b70299f00636378430a86f83b0937e5`
has 25 real samples on NVIDIA GeForce RTX 3070 Ti, queue 0, 64 valid bits and
1 ns period, with no dropped or pending GPU records at final retirement. It joins
actual renderer generation/submission, capture-local window/thread and CPU frame
keys. The capture SHA-256 is
`06af60d03f0b5004f8624f4ef2620b7a17bfe5892bcaf21289ee512d6f60cbf0`.
Five capture states and seventeen inspection states each have actual GPU images.
The earlier gpu01 strict failure caught an omitted internal test-fixture field;
that red evidence remains, and gpu02 includes the actual correction/checks.

Visual inspection found a small-window detail defect: raw ticks were clipped and
clock qualifications were below the visible panel. The directed gpu03/gpu04
successors use the shared ScrollBox/ScrollAreaState, keep the clock notice fixed,
wrap all evidence fields and verify bottom/reverse-top wheel routing without
restarting analysis. The ruler also no longer overlaps its last labels. These
runs reuse gpu02's unchanged original capture and verify identical production
library source hashes; they do not count as new GPU timing samples.

The latest standard executable is
`cc7dd9de811812525eb3fd1702784263bb329aabd2033fed595305de747826a8`,
recorded by `gpu04/qualification.json`: seventeen analyzer tests, strict app
Clippy, ordinary build, nineteen actual GPU images, original submission picking,
scrolling both directions, frame navigation and normal worker/GPU retirement.
The optional GPU machine operations are `gpu_clocks`, `gpu_submissions` and
`gpu_submission`, using the same evidence as the native tab. Filtering uses CPU
recording context, never invented GPU execution start. Exact-key drill-down uses
the source index; enumeration retains paging, cancellation and work limits.

Current schema-1/analysis-4 returns raw device ticks as exact decimal strings.
`gpu04/lossless-validation.json` independently compares all 25 actual samples and
50 ticks above JS's safe integer range against the unchanged capture using a
lossless Python client. Nine current actual request/response pairs pass generated
Draft-07 schemas; an actual analysis-3 cursor rejects on the same source. Earlier
gpu02/03 JS-reserialized wire debug files had rounded two rows' raw numeric ticks;
those files and original stdout are retained, and are not exact-u64 evidence.
Current wire artifacts preserve raw response text. This was a client evidence
precision issue; the capture and native/Rust data were unchanged.

`gpu04/architecture.json` confirms eight production libraries. Source guards show
no code/build-input drift; `round-cleanup.json` verifies 39 registered process
identities and currently live direct children absent, and the standard PE can be
opened exclusively without writing. No owned interactive program remains. Root's
eight 40b8 SDK pins and Agent/Builder instances are unchanged; SOURCE's old seal
does not qualify this uncommitted new application or modified libraries.

Next: remaining native gesture/accessibility audit, multi-window/recovery GPU
correlation, then consuming Agent/Builder capture-save-open integration and final
affected package checks/seal. Device ticks are explicitly uncalibrated to the CPU
clock, include command/device scheduling stalls, and do not measure presentation
latency. No full-function sampling, Reader/DPI signoff or complete Unreal Insights
suite parity is claimed. The whole VT01–VT06 goal remains active.

## Shared consuming-host acquisition and diagnosis — 2026-11-02

The consuming project's ADR0747 adds one product-side
`vesta-desktop-diagnostics` library for its two real desktop
consumers. It owns the existing SDK capture/export lifecycle and actual artifact
identity, not a second recorder or analysis engine. Host menus launch this
independent native analyzer. Explicit opt-in UI-stdio controls start/stop/observe
bounded captures without granting author writes, caller-selected destinations,
model execution or external-program launch. Its integration authority and
remaining cases are maintained in that project's `Docs/DesktopTraceIntegration.md`.

Root evidence `target/research/vesta-trace-consumers-20261102/standard01` records
two shared contracts, the extended Agent contract, strict all-target checks for
three packages, ordinary two-host builds and 3,106 unchanged resolved inputs.
Actual Agent/Builder captures each opened this standard analyzer and produced
five native images before normal shutdown. `machine01` and `machine02` cover
disabled/enabled capability on both hosts, idempotent request replay, busy-start
refusal, caller-path refusal, actual ready bytes/SHA and independent frame queries.
Current Agent `68cbe3fc` produced 17 frames; current Builder `e0a05862` produced 20.
Both still consume immutable SDK `40b8be17`; these are CPU-schema-2 captures.
No dirty SDK change or newer analyzer is claimed to update a host's recorder.

`diagnosis01` uses twelve bounded queries against that current Builder capture:
quality, slow-frame paging, frame-clipped self hotspots, exact-key occurrences,
event/tree paging, recorded callers/callees, counters and explicit absent GPU
coverage. Every actual pair validates against the generated schema and retains
the original source SHA. Python preserves integer precision. The derived report
retains quality, source, selected frame and query costs, and labels its isolated
startup workload; it does not claim a splitter-drag diagnosis or automatic cause.
This exercises actual Agent evidence retrieval without screenshots or a model.

The first host controller missed another standard updater's fresh ownership
check; its feature evidence does not qualify updater coordination. Root records
the gap in `standard01/coordination-audit.json`. Later machine selection uses the
other owner's actual release and final Builder hash. Native Open on that exact
final PE remains a directed recheck, not inherited from the superseded PE.
New controllers require a fresh release/hash guard. Root's execution window is
respected; no foreign program is closed. Final host path/status hardening and
GPU-pin adoption remain. The whole VT01–VT06 goal stays active, with no additional
physical-input, Reader, DPI or full-function coverage claim.

## Native semantic publication and focus ownership — 2026-11-02

ADR0037 connects the existing winit AccessKit adapter to the native application.
One event-loop envelope carries bounded worker results and platform events; the
large result is boxed, not copied into each accessibility event. Current realized
GPUI semantics and the shared search text/run projection are published after a
successful native render. Visible labels include statistics, diagnostics and raw
GPU facts. Focus/Click/SetValue/SetTextSelection use the SDK's live FocusScope and
text APIs; they do not introduce another resolver, editor or analysis path.
The normal adapter builds projection only for an active assistive client.

The actual native `accessibility-native-red01` run reproduces a secondary-button
capture retained after focus loss. It retires renderer/worker and exits 1; the
failure stays unchanged. Focus loss now uses the existing gesture-cancellation
path for both pointer buttons and restores the preview without committing new
analysis. GPU raw evidence is a focusable shared scroll view. Page Up/Down,
Home/End and vertical arrows scroll it without changing frame/range or analysis.

Current evidence is `target/research/vesta-trace-20261102/accessibility05/`:
17 contracts, strict all-target Clippy, ordinary standard build, CPU14/GPU19 native
states and current query wires pass. Standard executable SHA-256 is
`fcdab74122b661ed469efdd6ffd2e5a7fca73414f91b2e484ef9dbda2a1954e9`.
The 437 code/build inputs remain unchanged. The directed states exercise actual
tree/text projection, assistive Focus and undoable bounded replacement, selection
round-trip/foreign-run rejection, shared button Click, invalidated-layout refusal,
focus-loss/late-release cancellation and all six GPU scrolling keys. Each state
has a genuine renderer image. Rechecking these affected scenes is justified by
the new native event/publication boundary; earlier evidence retains its version.
`accessibility03`'s large-event strict failure is retained; it was fixed by boxing
the native envelope, not by suppressing the lint.

`accessibility-basic05` separately exercises the ordinary five-state path and
saves an actual self capture. A lossless independent reader and current machine
queries agree on 23 frame attempts and zero `accessibility.build` scopes for this
inactive-client run. This verifies the default branch without claiming a general
performance comparison. Current generated schemas validate three new GPU-query
pairs and two new self-capture pairs; the earlier twelve host-diagnosis pairs are
revalidated against these contracts, not represented as rerun.

Resolved all-feature architecture still reports eight libraries. Final cleanup
finds all 37 registered PIDs and currently live direct children absent; the
standard PE opens exclusively without writes. No owned window remains and the
SDK Trace updater is released. This does not release another project's execution
window or adopt a new recorder into Root. Next: directed committed-range/zoom/pan
and broader focus/keyboard audit, remaining multi-window/recovery GPU association,
then final host integration, software/seal and immutable adoption. Native
projection/action evidence is not a Narrator-speech, physical-input or DPI claim.
The full VT01–VT06 goal remains active.

## Committed native navigation and original-data agreement — 2026-11-02

The separate nine-state `--navigation-capture` scene drives held preview,
forward/reverse range commits, pointer-anchored Ctrl-wheel zoom, right-button
pan, Shift-wheel pan, keyboard fit and a real native resize while a preview is
held. After resize it verifies the previous analysis identity, live-or-retired
focus, capture release and a late button release that cannot commit. No OS input
or unrelated original scene is required to reach these checks.

The actual `navigation-native-red01` run reproduces a misleading header: a held
selection relabels old statistics with the uncommitted interval. The failure and
its two real images remain unchanged; renderer and worker retire before exit 1.
The header now uses the actual completed report interval and says to release to
analyze the preview. Its private projection metadata and visible text derive
from that same interval. This protects the connection between displayed facts
and the Agent query selection, not a pixel or wording preference.

Maintainer verifier [trace_navigation.py](../tools/verify/trace_navigation.py)
uses only the Python standard library. Given an original JSON capture, completed
native stdout and an explicit expected standard-analyzer SHA, it opens one
immutable query process. It compares each native report against typed queries
and independent original-event interval arithmetic: calls, inclusive time, self
time from direct-child interval unions, timer counts, whole-frame counts and
nearest-rank percentiles. Integer identities/totals are lossless. It preserves
raw response lines, source/log/verifier hashes and analysis version. It is an
acceptance oracle, never a second production analysis service.

`target/research/vesta-trace-20261102/navigation04/qualification.json` records
17 contracts, strict all-target Clippy and an ordinary standard build, followed
by both unchanged real inputs: gpu02's 25-sample capture and Root machine02's
20-frame CPU capture. All eighteen native states and all 38 actual query pairs
agree with the independent arithmetic and generated schemas. Each native state
has a genuine GPU image. The legacy CPU14 entry is directedly checked because
scene dispatch changed; old GPU19 evidence retains its prior identity rather
than being represented as rerun. Mixed inspection/navigation mode rejects before
creating a window/output directory. The original 45-second scene budget stays.

Current standard SHA-256:
`655177ed64b9cbd02c5d8bdccc281fa6ebccfafc1f6f514d3668cd020fb3e7b2`.
All 439 code/build/verifier inputs remain unchanged through qualification.
Cleanup and final source checks accompany that receipt. This work updates only
the SDK application and maintainer verification, not Root's recorder pin or
standard hosts. Remaining exits are collection/track keyboard access and source
epoch/focus audit, GPU multi-window/recovery correlation, Root path/launch-time
status/current-PE Open, final package checks/seal and immutable adoption. The full
goal remains active; no original 118, physical/Reader/DPI or workload performance
claim follows from this navigation fixture.

## Keyboard views and source-bound presentation identity — 2026-11-02

Statistics and thread tracks now have stable keyboard focus stops. Current
projected row/page limits drive Page/arrow/Home/End scrolling; the timeline keeps
Home/Fit. Scrolling retains report identity, selected evidence and table page
history. Search owns its editing keys. The native options are one private typed
configuration; fixture-specific paths do not become framework capabilities.

Actual failure runs are retained. `focus-native-red01` proves a loading projection
retired a keyboard-selected row's logical focus before results could validate it.
`focus-native-red03` proves both an old source's accessibility node selecting a
different file's same element slot and a captured old pointer surviving when
global search focus was retained. Both runs retire their renderer/worker and
normally exit 1. Busy views now retain logical focus without admitting stale
actions. A successful load cancels old geometry/pointer intent, and a changed
original source SHA gets a fresh shared AccessibilityTree namespace before new
facts publish. Global search intent is preserved; source selection/cursors reset.
This uses the existing shared policies, not a second focus or identity resolver.

`target/research/vesta-trace-20261102/focus05/qualification.json` qualifies
17 contracts, strict all-target Clippy and the ordinary standard build, followed
by a ten-state focused fixture. Its two explicitly synthetic files have twelve
lanes, 109 scope groups and 120 occurrences of a common group. It reaches the
last statistics rows, activates by keyboard, renders a busy projection, pages
actual occurrences, scrolls tracks while keeping page history, replaces the
source during capture, rejects old nodes and accepts the new source's node.
All ten states have genuine GPU images; their input timings are synthetic.
The permanent [fixture/receipt tool](../tools/verify/trace_focus.mjs) regenerates
both qualified inputs byte-for-byte and verifies the source/retirement receipts.

Because keyboard routing and load publication changed, the current binary also
passes the directed nine-state real navigation/19-query arithmetic check and
the affected old nineteen-state GPU inspection, including same-source reload.
All 21 actual current wire pairs validate generated schema-1/analysis-4 contracts.
Two additional queries prove the selected new-file group addresses that file's
120 records; no platform node ID is mistaken for analytical evidence.

Current standard SHA-256:
`9a6ba77d34a7d5ae271001838b595b8ab1907fc06931306e3b980677dfe2463d`.
The 441 code/build/verifier inputs stayed unchanged. The permanent fixture tool
has its own byte-equivalence receipt. Thirteen manifest/lock inputs are unchanged,
so the eight-library dependency result is reused, not reported as rerun. Fresh
cleanup finds 29 registered PIDs/current direct children absent and the PE opens
exclusively without writes. No owned window remains; SDK Trace updater/source
hold is released. Original failures retain their exact executable identities.
Root hosts/pins remain untouched by this increment. Next: import/cancel/error
lifecycle audit, actual multi-window/recovery GPU correlation, then Root
configuration/status/Open and final immutable delivery. Full VT01–VT06 remains
active; these synthetic input/command-origin checks do not sign physical input,
reader speech, DPI, calibrated clocks or a workload performance improvement.

## Transactional import, cancellation and retirement — 2026-11-02

ADR0038 separates idle, import, analysis and finite capture work. The last
validated immutable source/report stays available through errors and cancellation.
One source-bound view checkpoint restores range, panel, selection, filter and
paging without cloning session/report data. Search draft remains global intent.
Import cancels old pointer geometry on admission, blocks old-source data commands
and defers search until publication. Explicit Cancel/Escape cancels the token,
drops unsent work and advances the generation before returning. A successful
load commits a canonical new-source report before optional draft/panel analysis;
cancelling that follow-up cannot restore another file. Busy semantic actions
admit current search and cancellation targets, with the shared focus validation.

`target/research/vesta-trace-20261102/lifecycle03/qualification.json` records
17 contracts, all-target strict Clippy and the ordinary standard build, all 0.
The seven-state hidden native fixture uses the existing synthetic A/B inputs.
It observes actual missing/malformed errors, cancelled and superseded worker
replies, a successful stale analysis result, the real Cancel hit target, restored
new-source canonical data, draft reapplication and normal close during import.
It has seven actual images. Failed compile/strict attempts in lifecycle01/02 are
retained as failures. No forged worker response or raised deadline is used.

Because the work/input/publication boundary changed, directed Focus10,
real-input Navigation9 with nineteen independent arithmetic queries and affected
GPU Inspection19 pass on the same binary: 45 genuine images in total. All 21
current wire pairs validate the generated schema-1/analysis-4 contracts. Original
synthetic/real inputs and all 443 code/build/verifier inputs remain unchanged.
Thirteen manifest/lock identities match focus05; the eight-library dependency
result is retained, not represented as rerun. Three failure/restored-data images
were visually inspected. Round cleanup finds 19 registered PIDs/current direct
children absent and verifies exclusive PE open without writing bytes.

Current standard SHA-256:
`0ff8e64f7e4a0f88e61bdb92562ebfc7cf1db9d5bf053f772bed76b896976ff4`.
SDK Trace updater/source hold is released; no owned window remains. Root source,
standard hosts and eight pins are untouched. Agent-first evidence remains a
required product exit. Remaining work is actual multi-window/recovery GPU
correlation, Root path/status/current-PE Open, final cost/requirement/software
checks, seal/publication and immutable adoption. The full goal stays active;
this synthetic command-origin fixture does not sign physical input, Reader,
DPI, calibrated GPU clocks or workload performance.

## Two-window capture and controlled recovery — 2026-11-02

`target/research/vesta-trace-20261102/gpu-multi-recovery02/qualification.json`
records the existing trace-capture contracts (3), GPU timestamp contract (1),
Standalone delayed-context contract (1), feature/ordinary all-target strict
checks and both standard builds, all 0. Only seven Standalone host files change;
Trace and its library inputs retain the qualified lifecycle03 identity. An
explicit private hidden-smoke mode drives actual redraw events with a 30-second
limit while rejecting visible/focused windows. Ordinary smoke policy is unchanged.

The first attempt (`gpu-multi-recovery01`) exposed the absent hidden-window
paint cadence. Its empty native fixture PID39220 had no targetable Sky window
or opted-in stdio close route and required an identity-verified abnormal stop.
That failed run and stop receipt remain original; they are not normal-retirement
or timing evidence. Its controller still restored the ordinary standard build.
The corrected driver exits normally through the original host lifecycle.

Three real runs produce 26 baseline, 27 Surface-restoration and 27
Device-restoration samples. Stable capture-local window identities remain 1/2.
Baseline renderer timestamp generations are Main1/Float2; Surface restoration
changes only Main to3, while Device restoration changes Main/Float to3/4.
Native scope checks retain CPU state and shared context; Surface keeps the
context while Device replaces/releases it. These are controlled notifications
after actual submissions on a healthy RTX3070Ti, not physical loss tests.

The lossless independent oracle in the evidence directory compares all 160
high device ticks, elapsed/bound arithmetic, original CPU-frame/thread references,
native profile context and window/generation joins. All 80 CPU recording contexts
fall within their original frames (1ns serialization tolerance). Forty-five
bounded persistent Agent query pairs page all records and follow one original
submission/frame per generation; every pair validates the generated
schema-1/analysis-4 contracts. CPU/device calibration remains explicitly false.
No JS reserialization of original raw u64 captures is used.

The ordinary standard Standalone is restored to SHA-256
`89b6415487878ef962a1eaa7d982ccbe11d6ec33b3994e236ce41f3c670ecff3`
and then actually runs the same hidden two-window baseline normally, capturing
26 further samples without the optional native profiling flag. The unchanged
standard Trace (the unchanged lifecycle03 identity) imports the four-generation Device input
and passes nineteen affected inspection states with actual images, including
raw-key/frame drill-down and detail scroll. Step15 was visually inspected.
All 442 code/build inputs stay unchanged through this round; the independent
oracle has its own hashed receipt. The first oracle omitted its own supervisor
PID; machine-verification02 records it and rechecks the same 45 pairs without
replacing original receipts. Fresh cleanup finds 33 registered PIDs/current
direct children absent and both standard PEs open exclusively without writes.
No owned window remains. SDK Standalone updater/source hold is released.

Original single-window, lifecycle, physical/Reader/DPI and Root host evidence
retain their exact identities. This increment does not update Root's source,
standard hosts or eight SDK pins, and makes no comparative performance claim.
Next: Root native-exe/status/current-PE Open, final cost/requirement/software
checks and immutable delivery. Full VT01–VT06 remains active.
