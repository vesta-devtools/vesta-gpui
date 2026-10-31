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
