# Validation scope

## Current candidate

The published baseline is `0.1.0-rc.1`; the development tree prepares
`0.1.0-rc.2` with explicit [preview API migrations](../release/0.1.0-rc.2.md).
Package API versions remain `0.1.0`. This is a Windows native 2D developer preview, with
optional experimental Workspace, native WebView and viewport capabilities.

Source integrity, Rust compilation, automated contract tests, native operation
and consuming-application integration are separate evidence. A successful source
seal or compiler run cannot certify physical IME input, screen-reader speech,
cross-monitor DPI changes or the absence of all visual defects.

See [the release procedure](RELEASING.md) for the exact independent commands.
Actual local logs are written under `target/qualification/0.1.0-rc.1`; the
distribution sidecar records their outcomes and the exact source identity.
Absence of an outcome means it has not been qualified. The source is hosted at
[vesta-devtools/vesta-gpui](https://github.com/vesta-devtools/vesta-gpui).
There is no final 0.1.0 tag or crates.io publication. Adoption by a consuming
product is separate from the SDK's own qualification.

The repository now includes a Windows software-check workflow and a focused
native-profile CLI regression. Local fixtures exercise evidence validation and
module loading; they do not provide new native observations. A workflow must
actually run successfully before it can be reported as passing.

## Refinement checkpoint — 2026-10-31

The bounded-exit checkpoint makes CPU-input readiness explicit per window and
keeps presented-query freshness separate. Native 24240 demonstrated that a
controlled budget exit still admitted input/Focus and retained capture. Final
native 68296 verified 128 one-pass budget exits, no renderer/publication, capture
release, input/command rejection, lifecycle admission, focus/draft preservation,
motion/leave position caching and actual recovery per step. A separate native
failure retained the lost-cursor defect. The reduced fixture limit is not evidence
of a natural eight-pass exhaustion. Fault-feature native 48968 verified 128
acquire deferrals with coherent CPU input still usable. Final ordinary native
48800 submitted 64/64 per window using at most three/two passes. Frozen budget02
passed 212 contracts across 32 harnesses, documentation, strict/base/feature/tool/
format checks and both standard builds. Twelve current/historical-report and
corruption reviews passed. Rust inputs match all three native builds. Receipts
are in `target/research/frame-settlement-20261031`; original failed runs remain.
The earlier query-enabled probe also started normal stdio transport; its old
no-worker comment is corrected by isolating current in-process probes. Neither
this finding nor the fix erases earlier geometry results or supplies physical
input, reader, DPI, GPU/display timing, remote CI or consumer-deployment evidence.

The subsequent prepared-input checkpoint fixes a native deferred-acquire mismatch:
text mapping retained width 360 while the new hit layout used width 174. A private
helper now derives both windows' text geometry from the current PaintList before
rendering, using the existing snapshot algorithm. The fault-feature native run
verified 128 deferred steps, no submitted/presented revision at those steps,
matching text/clipped-hit bounds, and actual recovery between every step with
128 target rebuild deliveries. The final ordinary binary's directed focus sweep
submitted 64/64 per window and compared prepared text views with the painter.
Prepared01 passed all 212 contracts, documentation, strict/feature/tool/format
checks and both standard builds with unchanged source. Six real-report/corruption
reviews passed. Original semantic, compile and premature-fixture-access failures
remain under `target/research/frame-settlement-20261031`; software receipts are in
`target/research/refinement-text-host-20261031/prepared01`. This is injected native
CPU-path qualification, not physical input or GPU/display timing. Partial layout
budget exits and consumer integration remain open in GF03.

Main/Float now reconcile changed text/document/focus/scroll projections in a
bounded draw before the renderer. The actual native forward/reverse split sweep
originally reached the renderer 13/64 times in Main and 1/64 in Float. After the
fix, Inputs, Documents and focus workloads each submitted 64/64 per window with
at most three passes, retaining draft and selection/focus ownership contracts.
The final standard two-example build passed a directed focus identity check.
Frozen frame01 passed 212 contracts across 32 harnesses, documentation, all/base
strict checks, three Node tool groups, formatting and both standard builds.
Receipts retain the original red run and failed first focus assertion under
`target/research/frame-settlement-20261031`; family logs are in
`target/research/refinement-text-host-20261031/frame01`. No physical gestures or
GPU/display latency are inferred. GF03 remains open for the specific boundary
findings in [the programme](REFINEMENT.md).

The current refinement changes add standard text error propagation, separate the
font/cache owner from editor history, untangle Standalone composition and remove
a fixed experimental demo scene from the public viewport API. The family support
matrix has one owner in SDK documentation. Public migrations are in the rc.2 notes.

Frozen run05 passed 212 contracts across 32 harnesses, documentation tests,
all-feature and renderer-base strict checks, three Node tool groups, formatting
and both ordinary standard example builds. Input hashes stayed unchanged.
All 88 moved root function bodies and 52 viewport function bodies match their
prior implementation. The demo retains the original scene data; renderer tests
use matching private test inputs with their original assertions.

A fresh all-feature resolved dependency graph, public rustdoc with warnings
denied, and 55 local document links passed. Generated viewport documentation no
longer exports the demo fixture. Actual local receipts are under
`target/research/refinement-text-host-20261031/run05` and `docs01`.
The original missing-error-trait compile failure, strict test-module placement
failure, missed renderer test callers and interrupted run04 remain recorded.

This is local source/software qualification. It adds no native interaction or
physical input observations, no new remote CI result and no consuming-product
deployment. The refinement programme remains active. This result paragraph and
related status paragraphs are post-qualification documentation-only updates;
production bytes must retain the qualified hashes.

## On-demand stage capture — 2026-10-31

The shared native recorder now supports explicit start, stats and a frozen
stop-to-worker export handoff. Native launch capture selects stage detail;
the base start API preserves detailed scopes. Three high-rate measurement/shaping
hooks remain available in detailed mode rather than being deleted.

The final pinned Rust 1.89.0 batch passed all 210 workspace tests, documentation,
all-feature strict checks, renderer-base strict checks, both Node tool contracts,
formatting and both standard example builds. Extended existing contracts verify
detail filtering, retained frame/parent semantics, and background export while a
successor capture is active. Production inputs remained unchanged during the
batch. A two-second hidden native Motion run recorded 100 CPU frame attempts /
2,000 events, with zero loss, unfinished scopes or nesting errors; it is stage
mode, not a repeat of the older detailed capture or an Agent latency result.
Local command/source receipts are under
`target/research/agent-frame-trace-20261030/sdk01`. This outcome paragraph is a
post-qualification documentation-only update. Consumer adoption and native Agent
verification remain separate.

## Frame capture successor — 2026-10-30

ADR-0004 extends the initial timer recorder with frame identity, inherited call
parents, per-frame analysis, an offline report and the shared native launch path.
Its qualification is recorded separately from the first timer prototype below;
old passing counts are not transferred to changed code. The final pinned Rust
1.89.0 workspace all-target/all-feature run passed 210 tests, 0 failures and 0
ignored across 30 harnesses. Doctests, all-feature strict checks, renderer-base
strict checks, both Node tool contracts, formatting and both standard example
builds passed. Extended contracts protect frame/parent inheritance, unframed
worker isolation, clipped/overlapping child accounting, escaped offline export,
post-deadline completion labels and non-overwriting post-run file export.

The headless JSON and HTML captures each contain ten CPU frame attempts and 70
events. A standard-build native Motion process, hidden with a two-second observer,
captured 94 CPU frame attempts / 11,235 events; 81 attempts contain the successful
CPU present-submission marker. No events were dropped, unfinished or incorrectly
nested. Frame 15 was selected in the actual generated report and its renderer
children expanded in a browser. Its 132 recorded scopes include layout, text,
paint, input/accessibility projection and Vulkan phases. The observer exited
normally; the preview tab/server were closed. Local receipts are under
`target/research/frame-trace-20261030/run02`.

An earlier run failed the added late-completion assertion while its library had
already been compiled before the deadline fix; it is retained as a failed,
source-changing attempt, not a qualified frozen build. The final run's production
source guard remained unchanged. This section was updated afterwards to record
actual outcomes. Consumer deployment, physical presentation, GPU execution timing
and the original Agent divider defect remain separate; Standalone's new frame
entries were built/tested, not claimed as additional native interaction scenes.

## First timer prototype software checks — 2026-10-30 (historical)

The additive trace implementation passed the pinned Rust 1.89.0 workspace
all-target/all-feature suite: 208 passed, 0 failed, 0 ignored across 30 harnesses.
Three new public capture contracts and one abandoned-storage lifetime contract
protect lifecycle, bounds, thread/generation isolation and non-content export;
the existing production paint test additionally checks renderer phase bridging
without enabling the separate CPU profile API. Documentation checks and strict
all-feature and renderer-base checks passed. Both source/tool Node contracts and
format checks passed.

The real headless example exported 70 events, 0 losses and 0 unfinished scopes,
including host width-step/skip markers and automatic layout/text/paint/hit stages.
It uses synthetic width steps, not physical input. There is no native frame-time,
GPU execution or live Agent performance qualification in this result. Local
command/source receipts are retained at
`target/research/shared-ui-trace-20261030/run03`; earlier interrupted/failed attempts
are retained separately. Final source integrity and standard example builds are
recorded in the same parent evidence directory. This new work is not certified
by the earlier public CI run or deployed into an already running product.

## Historical evidence

Links described as historical context in the imported API inventories refer to
the earlier source-workspace acceptance programme. The original reports remain
with the maintainers at their recorded dates and hashes. They are not bundled
here, cannot be fetched from a private product repository by public users, and
are not independent verification of this candidate. The import receipt hash in
`IMPORT.json` preserves the exact original inventory identity.

Those reports include text/focus/menu, Markdown, layout, basic controls, numeric
editing, scrolling, collections, image/icon resources, toolbar/breadcrumb,
temporary editors, suggestions and command-center work. API behavior is described
in each package's current `SUPPORTED-API.md` and its implementation/tests.
The old product's Handoff and broad widget roadmap are not release requirements
or promises of unimplemented product functionality in this repository.

The earlier product accepted some remaining manual interaction checks by an
explicit human waiver on 2026-10-29: the remaining GPW04/05 interaction/visual
checks were accepted, and GPW06's dependent acceptance prerequisite was closed.
The original fixed module ledger recorded 115 completed exits and three accepted
by waiver/dependency closure. This is a historical denominator, not a score for
the current refinement programme or the entire framework.
That decision is retained as historical acceptance; it
does not create new physical measurements or implement unsupported capabilities.
Known software defects, independent build failures and unsupported APIs cannot
be waived by source packaging.
