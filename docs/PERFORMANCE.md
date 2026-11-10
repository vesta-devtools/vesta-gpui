# CPU workloads and recorder cost

The existing Standalone package owns one bounded Cargo benchmark, defined by
[decision 0013](decisions/0013-Reproducible-Cpu-Workload-And-Trace-Cost-Baseline.md).
It calls public Core/Widget APIs and production device-free `prepare_paint`.
It creates no window, GPU device, thread pool, author data or second recorder.

## Run and verify

Use Windows UI/CJK fonts and the pinned toolchain. Select a new output directory:

```powershell
cargo bench --locked --profile dev -p vesta-gpui-standalone --bench ui_workloads -- target/workload-01
node tools/verify/ui_workloads.mjs --report target/workload-01/report.json
```

The default Cargo target and explicit dev profile are intentional. This is a
separate normal Cargo benchmark harness, never a renamed application or a way to
bypass an occupied GUI. CI's all-target contract command runs only a small
correctness smoke through its no-argument entry (`--test` is also supported);
there are no machine-specific time thresholds.
No benchmark runtime, dependency, feature or public library API is added.

Report-only verification checks completeness and equivalence; it does not certify
the executable or source identity. For a qualification, retain the exact Cargo
command/PID/terminal exit, frozen source hashes before/after, executable SHA256,
font hashes, machine/toolchain and concurrent-load context. The recorded local
controller supplies these in `qualification.json`; its envelope can be checked
with `--qualified-run <run-directory>`. That mode verifies the archived report
hash and receipt consistency; it cannot re-attest an old executable after the
standard artifact has been rebuilt. Keep the original receipt with the samples.

## Work and measurements

Each workload runs the six balanced permutations of off/stages/detailed tracing.
Every batch uses a fresh equivalent fixture, four untraced warmup frames and
16 measured frames. There are 72 batches / 1,152 measured CPU pipeline frames.
Traces stop and export after measurement. Every mode must retain identical
geometry/work signatures and actual work counts for each input step.

| Workload | Fixed input and work |
| --- | --- |
| Static | 64 text rows, 900 by 480 logical viewport |
| Resize | Mixed Chinese/Latin editor content; repeated widths 900/720/480/720/900, at most eight layout passes |
| Edit | Alternating real insert and Undo on that editor, retaining its history owner |
| Collection | 100,000 stable item IDs, 480 viewport, 24 row height, overscan three, five scroll offsets and one retained offscreen item; at most 28 projected rows / 31 layout nodes |

Raw wall samples cover tree construction, reconciliation/editing, action/hit
routes, paint and CPU preparation, including trace entry/exit. They exclude
fixture/font setup, validation, object destruction and export. They are neither
complete native frame latency nor CPU utilization, GPU execution or display
latency. Cold raster preparation remains in the samples; do not remove slower
frames from percentiles. The verifier recomputes distributions, checks the
complete ordered matrix and correlates typed calls/counters with raw events.
The recorder remains the sole call-tree/self-time analyzer.

An idle recorder must invent no events. A 16-event budget retains 16 and reports
1,009 dropped events for the explicit overflow sequence. Deadline expiry and
stop/drop must release recording ownership. Six balanced million-iteration
disabled-hook/control pairs retain raw deltas, including negative/noisy results;
this experiment adds no background sampling or application scheduling.

## Recorded checkpoint — 2026-10-31

Local evidence is under `target/research/ui-workloads-20261031`. Two baseline
runs, two recorder-change runs and two final entry-fix runs retain all 6,912
pipeline frames and 288 complete traces. All use the same workload inputs/fonts;
all work signatures/counts match. The Ryzen 9 5950X / Windows
10.0.26200 machine was shared with other builds, not isolated for timing.

The collection baseline detailed/off medians were 0.2163/0.1426 ms and
0.2172/0.1431 ms. The recorder checked the same session/deadline twice before
each successful append. One private entry coordinator now keeps the single
authoritative append-time check and existing lock order. It introduces no cache,
generation state or public interface. Afterward the medians were
0.2074/0.1418 ms and 0.2155/0.1464 ms: observed detailed overhead relative to off
was about 46–47%, versus about 52% before. This small improvement is specific to
this workload and environment; tail latency was noisy and no general speedup
is claimed. Detailed capture still has a cost. Stage mode remains available.

The first all-target qualification passed the existing Rust contracts but
rejected the benchmark's output-required no-argument entry. The corrected
no-argument entry runs the small smoke. Two fresh explicit final runs retained
identical workload/event structure, with collection detailed/off medians
0.2135/0.1515 ms and 0.3258/0.2641 ms. This large disabled-mode variance on the
same final artifact reinforces the shared-machine limitation; none of these
numbers is a stable latency guarantee. The original failure and all samples stay
in the evidence, without discarding the slower run.

This is the recorded GF05 headless CPU slice. The native collection/idle matrix,
allocator-only ABBA and same-Model consumer comparison now have separate scoped
qualification. Current evidence and final immutable-delivery work live only in
the [finite programme ledger](REFINEMENT.md); this dated checkpoint retains its
original workload, executable and shared-machine limits.

## Incremental measured-list checkpoint — 2026-11-09

[Decision 0040](decisions/0040-Incremental-Variable-List-Metrics-And-Scroll-Anchors.md)
adds point height updates and caller-remapped scroll anchors to the existing
bounded variable-list projection. The one-time comparison source is retired
from maintained examples after qualification; its original source is retained
under `target/research/regression-prune-product-adoption-20261110/retired/`.

Local raw samples, executable/source hashes and machine/profile context are in
`target/research/list-updates-20261109/{workloads.txt,receipt.json}`. The Ryzen 9
5950X Windows machine was shared with other builds. The normal dev profile uses
opt-level 1 and dependency opt-level 3. Each path is warmed, then measured in four
blocks with alternating order, with 256 deterministic height edits per block.
Both paths capture/restore the same anchor and query the new range. Signatures
match; each final incremental index also equals a fresh full reconstruction.

| Rows | Full reconstruction, microseconds/edit | Incremental update, microseconds/edit |
| --- | --- | --- |
| 1,000 | 2.057–2.102 | 0.270–0.272 |
| 100,000 | 169.418–171.254 | 0.155–0.164 |

This compares two paths in the new implementation, not old/new application
versions or BUI versus VestaGPUI. It measures geometry updates plus anchor/range
work, excluding initial construction, window/rendering, source lookup, layout,
semantics and projection. Other Host work can still be O(n).

The space/update tradeoff also has a query cost: a separate 20,000-lookup probe
reported 27.6/111.1 ns for prefix-search reference/block-index range lookup at
1,000 rows, and 83.0/214.0 ns at 100,000. The reference is two searches in the old
prefix-vector representation, not the whole old public API with all validation.
The block index keeps f32 row heights plus about 3.125% additional block storage
on large lists. These samples support incremental mutation workloads; they do
not establish faster static lists, native frames, GPU work, FPS or display latency.

Portable checks preserve fractional-edge admission, failed-update atomicity and
layout/hit/action/focus/semantic identity. Standalone exercises its current
variable-list consumer, including height changes and stable-ID remapping; purely
fixed-height data mutations retain their existing scroll/focus repair policy.
Physical input and screen-reader acceptance remain separate.

## Dense table text checkpoint — 2026-11-09

[Decision 0042](decisions/0042-Dense-Table-Text-Preparation-And-Clip-Batching.md)
keeps the current cell Elements and semantics, fits a larger short-text working
set inside bounded caches, and lets fitting text runs share the viewport scissor
for existing atlas batching. Overflow retains its original clipping.

Run the Standalone consumer probe with real Windows Segoe UI and YaHei fonts:

```powershell
cargo run --locked -p vesta-gpui-standalone --example table_pipeline -- target/table-cpu.json
cargo run --locked -p vesta-gpui-standalone --example table_pipeline -- target/table-gpu.json --gpu
```

Outputs use create-new semantics. The first command measures CPU stages without
a window or GPU. It projects 32 rows, eight/24 columns, 96-logical-pixel tracks
and 264/792 text cells including headers, at 150% DPI. Selection changes each
frame. Four frames warm the same retained layout/text owners; 32 measured frames
follow. Assertions outside timing verify cell hit/action/tab identity, AccessKit,
Agent inspection and complete realized-node counts. Each path includes the same
projection, layout, routes, hits, semantics, inspection, paint and preparation.

Local before/final nearest-rank p50 CPU samples, milliseconds:

| Columns | Paint preparation before → final | Complete measured CPU pipeline before → final |
| --- | --- | --- |
| 8 | 95.053 → 0.584 | 96.081 → 1.488 |
| 24 | 300.005 → 2.190 | 302.810 → 4.904 |

The initial bottleneck is sequential cache churn, not cell tree construction
(initial projection p50 0.218/0.657 ms). A cache-only isolation run had preparation
p50 about 0.53/1.73 ms. These are sequential before/after runs, not an alternating
executable trial. They use the normal dev profile on the shared Ryzen 9 5950X
Windows machine during other builds. All raw samples remain recorded; the large
improvement characterizes this repeated short-text workload, not arbitrary cells,
larger working sets, FPS or physical-display latency. Cache entry ceilings and
aggregate glyph retention increased; byte budgets and per-run limits are listed
in [the text resource contract](TEXT-AND-OBSERVATION.md).

The explicit `--gpu` mode uses a hidden native window and the actual renderer,
on the local NVIDIA GeForce RTX 3070 Ti / Vulkan 1.3.281. It captures the third
successful frame of each scene at a fixed 2306×1026 physical target. Higher DPI
shows fewer cells because the target stays fixed. Selection and a focused cell
are present. The final RGBA bytes match the original bytes in all six scenes:

| Columns | Scale | Actual UI draw calls before → final | Scissor updates before → final |
| --- | --- | --- | --- |
| 8 | 100% | 307 → 85 | 297 → 85 |
| 8 | 150% | 235 → 76 | 225 → 76 |
| 8 | 175% | 199 → 61 | 189 → 61 |
| 24 | 100% | 851 → 117 | 825 → 117 |
| 24 | 150% | 469 → 126 | 450 → 125 |
| 24 | 175% | 331 → 93 | 315 → 93 |

CPU prepared entries are individual glyph/shape payloads, not these uploaded GPU
draw calls. The native result is a draw/coverage check; it claims no GPU duration,
screen-reader acceptance or consumer product integration. Overflowing labels and
intervening paint can limit batching. Rich content and retained focus continue
through the ordinary Element, text and input paths.

Local evidence lives under `target/research`: `table-pipeline-{baseline,cache,final,
summary}-20261109.json`, `table-gpu-{baseline,cache-only,contained}-20261109.json`,
their `.rgba` payloads, `table-gpu-contained-comparison-20261109.json` and
`table-receipt-20261109.json`. The failed preliminary cropped-quad experiment is
retained as `table-gpu-clipped-*` and its comparison; that path is not implemented.

## Streaming-list append checkpoint — 2026-11-09

[Decision 0044](decisions/0044-Streaming-Lists-And-Wide-Tables.md) extends the
existing geometry probe with equal-input append/splice work. Both paths warm,
then alternate order in four blocks of 256 appends. With 100,000 initial rows,
full reconstruction takes 224.588–248.488 microseconds per step and incremental
splice 0.508–1.523. Every step includes the same source-height append and
anchor/range work; final metrics equal a fresh full reconstruction and signatures
match. Ordinary Vec capacity growth can reallocate the height array. General
prepend/remove is still O(n).

These are normal dev-profile geometry samples on the shared Ryzen 9 5950X
machine, not text/UI frames, Agent latency or FPS. The slowest block is retained.
A final-source repetition records 211.219–229.789 / 0.445–0.721 microseconds for
the same rebuild/incremental paths; both full recordings remain in the evidence.
Original samples, source/executable hashes, the real-font eleven-state public
message consumer and current package-check logs are under
`target/research/streaming-lists-20261109/`. No native message-input or downstream
product qualification follows from this portable consumer checkpoint.

## Column-window CPU checkpoint — 2026-11-09

The public `wide_table --measure <font paths>` consumer compares the existing
all-column, row-windowed projector with `table_view_columns`, using 10,000
source records, 256 rich columns, four visible rows, one retained focused cell,
a 720 × 140 logical viewport and real Segoe UI/YaHei fonts. Frozen counts are
zero in both paths; the separate 512-column consumer exercises frozen ends.
Host row names, data, theme, integer scroll steps, visible child identities,
clipped geometry, hit actions, all 40 visible text paint commands and focused
AccessKit/Agent identity agree. Both use complete inspection within the same
4,096-node/64-KiB defaults. Host row names replace the legacy concatenation of
all cell labels, which otherwise exceeds the bounded text budget in this source.

After four warmups, sixteen samples alternate which path runs first. Full
construction realizes 1,280 cells / 3,081 Elements; column windows realize 51 /
149, including overscan and the focused cell. The paint-comparison run's CPU
total is 11.844–21.192 ms (median 14.064) for full columns and 0.412–1.222 ms
(median 0.764) for windows. The earlier run, before adding the exact text-paint
comparison outside timing, records medians 15.648 / 0.878 ms; both complete logs
retain slow samples and demonstrate shared-machine variation. After the final
consumer-only stable-row-name assertion, an executable-hash-bound repetition
records 9.544–14.049 / 0.456–0.818 ms, with medians 10.245 / 0.588. All three
recordings remain; this later observation does not replace the slower runs.

Timing includes tree construction, real-font retained layout, route/hit
construction, AccessKit, complete bounded Agent snapshot and paint generation.
It excludes source/column-metrics creation, assertion/signature validation,
renderer preparation, GPU submission, native input and physical display latency.
This is the normal dev opt-level 1/dependencies opt-level 3 profile on the shared
Ryzen 9 5950X. It is neither a native FPS claim nor a sparse-column metric index;
metrics construction/order/width changes remain O(n). Local raw logs and source/
binary/font receipts live under `target/research/wide-tables-20261109/`.

## Published-source consumer recheck — 2026-11-10

The immutable source copy of `3c9a247b546330b93d3d9b8ad37432b7d0c64c52`
passes its source verifier and matches all 617 published Git blobs. Its own
ordinary Cargo target builds the message, wide-table and geometry consumers;
both real-font consumers pass eleven states and their exact hit/action/focus,
AccessKit and complete Agent-snapshot assertions. No native window is opened.
The source receipt and all three executable hashes agree before/after the run.

With the same alternating equal-input method above, four 100,000-row append
blocks record 252.229–268.100 microseconds per step for reconstruction and
0.481–0.945 for incremental splice. Signatures and final reconstruction agree.
Pure query lookup is slower than the cached-prefix reference in this run:
91.4 versus 208.6 ns at 100,000 rows. The index targets changing geometry;
it does not make every unchanged-range lookup faster.

The equal-visible-work table comparison keeps 1,280/51 constructed cells and
3,081/149 Elements. Sixteen samples after four warmups record full-column totals
of 11.414–18.416 ms (median 14.134) and column-window totals of 0.436–0.885 ms
(median 0.651). All visible text paint, clipped child geometry/actions and retained
focus signatures agree. The workload, inclusions/exclusions and dev profile
remain as described above; shared-machine timing is not GPU/display latency or
an FPS claim. Previous slower samples remain intact. Fresh logs and the exact
source/executable/font receipt are in
`target/research/collection-closure-20261110/`.
