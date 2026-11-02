# Vesta Trace delivery plan

Requested and started: 2026-11-01. Status: Finite VT01–VT06 delivery qualified, 2026-11-03.
Decision: [0034](decisions/0034-Native-Trace-Analysis-Application.md).
Agent evidence contract: [0035](decisions/0035-Agent-First-Trace-Evidence-Queries.md).
GPU capture ownership: [0036](decisions/0036-Capture-Owned-Gpu-Submission-Evidence.md).
Native semantic/input boundary: [0037](decisions/0037-Native-Trace-Accessibility-And-Evidence-Focus.md).
Native work lifecycle: [0038](decisions/0038-Transactional-Native-Trace-Work-And-Cancellation.md).

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
