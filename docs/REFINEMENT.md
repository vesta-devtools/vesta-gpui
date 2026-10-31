# Concise and reliable framework programme

Requested: 2026-10-31. Status: Active.
Authority: this repository. Products track adoption in their own integration records.
Decision: [0006](decisions/0006-Explicit-Architecture-And-Refinement-Gates.md).

## Completion standard

Every crate/module must have a justified responsibility and dependency direction.
Every state has one authoritative owner. Public API states input/output,
lifetime, failure and caller responsibilities. Demonstrated duplicate or unused
layers are removed with a migration when relevant; uncertain cases stay open.
No new widget, platform promise or abstraction is added without a real use case.
Clarity requires code and checks as well as documentation.

The finite gates below define this programme. Their completion is not a claim of
universal platform support or a final release. Progress is recorded by actual
exits, not estimated feature percentages or the number of tests.

| Gate | Required result | Evidence and status |
| --- | --- | --- |
| GF01 — Architecture authority | Current eight-library graph, state/lifecycle ownership map, resolved dependency boundary gate and corruption checks | Completed 2026-10-31: actual all-feature Cargo metadata accepted; corruption contract rejects reverse/optional/target/build/transitive native and UI leaks; no new Rust/native qualification |
| GF02 — API and module reduction | Audit actual public surfaces against current documentation and callers; remove unjustified wrappers/exports, untangle mixed responsibilities, provide migrations for changed public contracts | In progress; text/ownership/composition, private fixture and API-authority changes qualified; remaining families open |
| GF03 — Frame and layout contract | Audit main/float/consumer preparation; reproduce continuous-width failures, define one clear reconciliation/invalidation/commit contract, verify text/scroll/hit/paint agreement and bounded retry | In progress; Main/Float and native Agent CPU/deferral/controlled-budget contracts qualified at recorded inputs; Builder ordinary bridge qualified; Builder budget/fault and current joint integration remain open |
| GF04 — Interaction and lifecycle | Close concrete existing input/text/IME/focus/popup/selection/resource gaps; make transport, cancellation, error and ownership limits explicit; preserve existing accepted scenarios | In progress; click/shared-preview cancellation and stdio deadline/lifetime contracts qualified; remaining interaction/resource audit open |
| GF05 — Performance and diagnostics | Reproducible static/resize/text/collection workloads, disabled/enabled trace cost, bounds/retention/idle checks; select and fix measured bottlenecks with comparable evidence | In progress; four bounded CPU workloads and strict evidence verifier implemented; repeated cost comparison removes duplicate recorder checks; native/consumer/resource review remains open |
| GF06 — Usability and delivery | Small reusable host recipe, capability-oriented docs, matching exports, clean feature checks and CI, exact SDK source identity and immutable Vesta consumer validation | Open |

Each gate is split into concrete findings as its review runs. New findings must
state source location, failure/redundancy, intended owner, minimal change and
acceptance; additions to scope require a recorded reason. Gate completion does
not erase earlier failed runs, waived scenarios or unsupported platform limits.

## GF05 bounded CPU checkpoint

[Decision 0013](decisions/0013-Reproducible-Cpu-Workload-And-Trace-Cost-Baseline.md)
and [the performance procedure](PERFORMANCE.md) define the one benchmark owner,
trace admission simplification, reproducible inputs and evidence limits. Six
runs retain 6,912 CPU pipeline frames and 288 complete traces; work signatures,
counts and event structure agree. The final full02 passed the existing 215
contracts, benchmark smoke, strict/doc/tool/format checks and standard builds.
The no-argument Cargo failure is preserved and corrected. This closes the
headless workload/cost slice, not GF05: native/consumer shared capture ownership,
resource review and current immutable joint adoption remain open.

## Baseline and initial findings

The starting source is public `9e31f07d8dc4fa351418bc7321b4afff45783d0d`,
sealed tree `780f82079f05610d844218175a0852105fb6536d3c3751028a4bd423ae48b546`.
Actual local inventory is under `target/research/framework-refinement-20261031`.
It found ten workspace packages: eight libraries and two examples; 353 Rust
source files / 116,130 lines **including tests and both examples**. These counts
are inventory, not product complexity or a new qualification.

- Existing package directions agree with [the map](ARCHITECTURE.md). Preserve
  this structure while reviewing internal responsibilities; there is no evidence
  yet that merging or splitting crates would improve it.
- At baseline, Standalone `main.rs` had 3,028 lines and `gallery.rs` 1,539. Review
  their remaining state, event/command and projection responsibilities. Extract
  only cohesive responsibilities while preserving the same App owner; splitting
  files alone does not resolve ownership.
- At baseline, Core `text_input.rs` had 1,221 lines. Review editor-state/context responsibilities
  and error interoperability. At baseline `TextInputError` had no Display/Error
  implementation; the finding below addresses that specific failure contract.
- Public API inventories combine historical amendments with current contracts.
  Reorganize them around actual capabilities and exports, preserving historical
  evidence references without presenting them as current independent acceptance.
- Native host preparation returns between text/layout updates in more than one
  host. The actual Agent divider capture and fixed 64-step consumer regression
  motivate GF03. Do not assume the same defect in Standalone before reproducing it.
- Native adapter stdio starts a detached process-lifetime worker and exposes no
  shutdown/join handle. The current native callers and embedded reuse were
  reviewed under [decision 0014](decisions/0014-Stdio-Wait-Budget-And-Process-Lifetime.md):
  retain this explicit process boundary, not a flag-based cancellation promise.
  The review reproduced a 25 ms condition wait taking 10.01 seconds and retiring
  transport; the bounded shared-deadline reply path and real process EOF/size/
  successor-request contracts are now qualified in the GF04 checkpoint below.

## GF02 findings in progress

[Decision 0007](decisions/0007-Text-Ownership-Errors-And-Example-Composition.md)
defines the current bounded change. These are findings within GF02, not newly
invented programme exits.

- **Text error interoperability:** ordinary host `?` conversion originally failed
  to compile. Existing typed variants now implement the standard error traits;
  ellipsis/native wrappers retain their authoritative cause. An external consumer
  regression protects conversion, downcasting, failed-reset buffer preservation
  and wrapped clipboard/edit causes. No error-string matching or new error model.
- **Font/editor ownership:** the private text context module owns font identity,
  registration and cache invalidation. The existing editor owns buffer/history.
  Public names and measurement semantics remain the same.
- **Example composition:** state, action dispatch, pointer input, native services
  and stateless construction move to the owners in [the map](ARCHITECTURE.md#standalone-composition).
  One App remains authoritative. All 88 original root function bodies, including
  the const ID constructor, match their recorded token hashes after migration.
  The frozen run02 passed 212 contracts, documentation, all/base strict checks,
  three Node tool groups, format and both standard builds with no input drift.
  The prior run01 passed contracts but failed strict checking for test-module
  placement; the failure is retained and fixed without weakening the check.
- **Example-only public scene:** caller review found the fixed
  `ViewportScene::acceptance_scene` used only by Standalone and library tests,
  with no Vesta product caller. It moves to the private demonstration owner;
  existing tests keep their private fixture and assertions. Migration is explicit
  in rc.2 notes. No additional geometry or scene abstraction is introduced.
  Run03 caught three renderer test callers missed by the initial search; they
  now use the same input data from a private test-only fixture, preserving all
  assertions. The failed compile remains part of the evidence.
- **API support authority:** the single family support/migration matrix moves
  from the renderer document to [SDK documentation](SUPPORTED-API.md). The
  renderer links it and retains its own contracts; no second export/state list.
  Both reductions follow [decision 0008](decisions/0008-Example-Scene-And-Api-Boundary-Ownership.md).
  Final frozen run05 passed all 212 contracts across 32 harnesses, doc tests,
  all/base strict checks, three Node tool groups, format and both standard builds.
  All 52 original viewport function bodies match, and the demo/test scene data
  retain the original fields. The fresh resolved architecture check, generated
  public rustdoc with warnings denied, and 55 local document links passed in
  docs01. The public fixture is absent from generated API documentation.
  The run04 controller and Cargo were verified absent on resumption, without a
  terminal receipt. Its unchanged source snapshot and interruption remain in
  `run04/recovery.json`; it is not counted as a passing run.
- **Historical acceptance wording:** the old API matrix still called GPW04–06
  incomplete, although the authoritative 2026-10-29 product audit recorded three
  owner-waived/dependency exits. The current matrix now distinguishes that
  acceptance from actual physical observations and unsupported platform limits.
  The original evidence and broader historical percentages are not rewritten.
- **Retained error-family interoperability:** the wider inventory found 17 flat
  public error enums missing the standard traits, plus three typed wrappers
  (`MarkdownError`, `MarkdownProjectionError`, `WebLinkOpenError`) with no source
  chain. The external consumer compile failures and wrapper runtime failures are
  retained in `target/research/public-errors-20261031`. The repair follows
  [decision 0012](decisions/0012-Public-Error-Interoperability.md), preserving the
  variants, owner and validation behavior without another wrapper/dependency.
  All four focused consumer harnesses passed (nine contracts). Frozen errors01
  then passed 215 contracts across 33 harnesses, documentation, all/base strict
  checks, three Node tool groups, format and both standard builds without input
  drift. The 37 named error definitions remain unchanged; all six new wrapper
  source branches retain their original typed cause. No new native interaction
  or Vesta adoption is claimed. This family is closed; GF02's other findings and
  the programme remain open.

The remaining public families, gallery composition, host frame convergence and
transport lifetime are not signed off by this subset. Qualification inputs and
the original compile failure are retained under
`target/research/refinement-text-host-20261031`.

This checkpoint qualifies the changed source/software responsibilities, not the
whole programme. No new native interaction, physical DPI/reader observation,
remote CI or Vesta adoption is claimed. Final result paragraphs and the source
receipt are documentation-only updates after the frozen commands; production
inputs must still match those commands byte for byte.

## Validation discipline

### GF03 current finding and evidence

[Decision 0009](decisions/0009-Bounded-Host-Frame-Settlement.md) keeps the policy
with the host. In `frame-settlement-20261031/before01`, the exact hidden native
split sweep reached the renderer only 13/64 times in Main and 1/64 in Float.
The unchanged draft and normal process exit did not make that a passing result.
The same default workload in after01 reached/submitted 64/64 in each window,
with two preparation passes at most. Documents01 submitted 64/64 per window with
at most three passes and retained each window's Markdown selection/copy binding.
Focus02 submitted 64/64 per window with Main at most three passes and Float two,
while retaining independent focus memories and one Float caret-session owner.
These are logical/native CPU paths, not physical input or presentation timing.

Focus01's final fixture assertion incorrectly expected another window's remembered
focus to be erased. It failed and is retained. Existing `set_window_focus` and
`WindowTextSession` contracts require separate window memories and one active
editor owner; focus02 checks both memories and that owner instead. No production
focus behavior was changed to satisfy the fixture. The existing 45 default
Standalone contracts passed. Frozen frame01 then passed all 212 contracts across
32 harnesses, documentation, all/base strict checks, three Node tool groups,
formatting and both standard builds, with unchanged inputs. The final two-example
build has a different executable identity from focus02 despite identical Rust
inputs; its exact standard executable passed the directed finalfocus01 run with
the same 64/64 per-window submissions and three/two-pass bounds. Earlier scenes
retain their actual executable hashes rather than inheriting this new identity.

The verifier was rebound to each actual native PID. Eight actual-report and
corruption reviews retain the original red result and reject a missing step,
missing renderer call, capture loss and foreign PID. Corrupted reports are not
new native evidence. Receipts are in `target/research/frame-settlement-20261031`.

The deferred-input review reproduced another native failure. An injected acquire
deferral retained a 360-wide text viewport after the CPU hit/focus layout became
174 wide. The same immutable PaintList now supplies text views before renderer
entry through one private helper using the existing snapshot viewport algorithm.
The existing caret regression uses that production helper; its formerly separate
test projection is removed. No public API, text algorithm or scheduling layer is
added. RenderOutput remains a completed-renderer observation for other callers.

Deferredafter03 used the existing fault-injection feature: 64/64 draws in each
window entered the renderer but correctly submitted zero frames at the injected
step. All 128 current text bounds/clipped hit tests agreed and no presented
revision advanced; every step subsequently recovered a real successful submission.
There were 128 actual target rebuild deliveries. Successful probe frames also
compare all text fields with the production painter's output. Preparedfocus01
on the final ordinary standard binary submitted 64/64 per window with at most
three passes, retaining both focus memories and the single caret owner.
Frozen prepared01 passed 212 contracts across 32 harnesses, documentation,
all/base strict checks, three Node tool groups, formatting and both standard builds.
Rust inputs match both native builds; feature/executable identities stay separate.

The original deferredbefore01 native exit1, deferredafter01 missing-import compile
failure and deferredafter02 fixture's premature target access remain recorded.
The final fixture waits for actual presented-revision recovery rather than a fixed
number of ticks. Six actual-report/corruption checks preserve the red result and
reject stale geometry, false publication, absent recovery and missing ordinary
submission. Their modified reports are verifier tests, not native observations.

The bounded-exit review follows [decision 0010](decisions/0010-Prepared-Input-Readiness-And-Budget-Exit.md).
Actual budgetbefore03 native 24240 exhausted a controlled one-pass limit but still
admitted native-event classification and a real semantic Focus request, retaining
its pointer capture. Windows now own explicit CPU-input readiness, set only after
coherent text/hit installation. Native/accessibility admission blocks partial
preparation, lifecycle events remain available, and existing query dirty state
blocks semantic/logical commands until presented publication. Cancellation and
admission move from the GPU module to the window owner; no SDK runtime/API is added.

Budgetcursorafter01 verified 64 controlled exits per window: no renderer entry,
no presented revision, released captures, preserved focus/draft, rejected semantic
and logical requests, lifecycle admission and recovery after every step. A separate
cursor red showed that blocking motion lost the latest coordinate; the fix samples
motion/leave while suppressing hit/gesture dispatch. All 128 cursor observations
passed. Its 132 total budget markers include four warm-up exits; the workload itself
contains exactly 128. This deliberately reduced limit exercises the production
exit, not a naturally observed eight-pass exhaustion. Budgetcursordeferred01 on
the fault-feature binary retained valid CPU input through 128 acquire deferrals
and actual recoveries. The final ordinary eight-pass binary passed budgetfocus01,
with 64/64 submissions per window and three/two passes. Frozen budget02 passed
212 contracts across 32 harnesses, documentation, all/base strict checks, tools,
formatting and both standard builds. All three native Rust inputs match it exactly.
Twelve actual-report/corruption checks passed, preserving historical report scopes;
new optional cursor/readiness observations are explicit rather than retroactive.

Original private-method compile failure, probe control overwrite, insufficient
turnaround width delta and stale-cursor failures remain recorded. The query-enabled
earlier probe accidentally entered normal stdio startup; current in-process probes
are isolated from that branch. Earlier geometry evidence remains at its actual
identity, but the old assertion that no stdin worker started is not substantiated.

The external Builder bridge review now has a reproduced immutable-snapshot
failure: its text height changed from 243.6 to 365.40002 while the old candidate
still accepted host focus. The product fixes its own pure UI retry and explicit
facade readiness. It removes its renderer text callback and projects the same
prepared snapshot geometry, keeping business/author/Worker updates once per draw.
An unmounted facade now returns unchanged instead of exhausting the host's loop.
The existing settings contract also caught a real duplicate local identity 550;
only the fixed reset control moves, with no new registry or scope.

Vesta ADR-0684 and its external
`Docs/Research/Evidence/builder-embedded-frame-20261031/qualification.json` own
the full product evidence. Agent 84 and Builder 29 contracts passed, with one
existing manual GPU profile ignored; strict checks and format passed. Actual
standard Builder PID 53108 completed five native resize/mount/unmount/restore
cases, 372 submissions and normal GPU retirement. Private controls retained their
identities and rejected command focus. The old full Builder script's menu/status
assumptions and Creation Scene prerequisite remain unqualified. Its unsuccessful
edits are retained as research, while `--builder-frame` qualifies only the changed
no-Scene frame contract. Original failures and an executable replacement caught
before launch are preserved; a normal standard rebuild supplied the final identity.

That product still consumes public 9e31f07 through all eight selectors, rather
than this SDK's newer local source. Its 2,561 runtime inputs were stable through
the actual native run; later Agent/Data source changes are separately listed and
do not inherit the result. No SDK native rerun, physical-input result, remote CI
or immutable joint adoption is claimed by this external checkpoint.

GF03 remaining consumer work is specific. Builder's host budget exhaustion and
GPU deferral/loss need directed qualification; a headless facade suspension check
does not sign them. The independent Agent findings below now have a bounded
qualification; its successor shared Core configuration and joint SDK adoption
remain separate. Motion has no editor or stateful width-feedback reconciliation:
its source keeps one presented hit/layout frame until submission and invalidates
it on resize/scale/target preparation. It needs no speculative eight-pass loop;
this is a source review, not additional Motion native acceptance.

Vesta ADR-0686 and its external
`Docs/Research/Evidence/agent-input-projection-20261031/qualification.json` record
the native Agent closure. The real baseline retained old text geometry in all
three injected acquire deferrals, then retained capture/click history after a
controlled one-pass budget exit. The facade's existing three-state candidate
authority now belongs to the same App as the native path. One private method
installs hit geometry, CPU layout and all text viewports; the misleading
accessibility-layout cache becomes input-layout state, and semantic publication
no longer writes it. One suspension path preserves draft/logical focus and retires
capture/click sequences. Embedded controllers do not own a native renderer loop.

Agent 84 and Builder 29 contracts, one original ignored manual GPU test, and both
strict checks retain their actual receipts. A final two-file pointer-observation
delta passed fresh feature strict and native checks. Actual FI Desktop 9fc192c5 /
PID 49728 passed text/layout agreement, pending presented commands, retirement,
partial-input denial, cached cursor/leave observations and real subsequent
submission. The native process exited normally with GPU retirement and no
backend, author data or OS input. The original failures and fixture corrections
remain recorded. This is logical input in a real native renderer, not natural
eight-pass exhaustion, physical interaction or display latency.

Ordinary Desktop b11e5187 and Builder 9d7ae288 were restored and rebuilt. The
554 selected UI inputs remain current; shared Core/Runtime and Cargo.lock changed
during the final build, so the current complete product delivery is still pending
its Core owner. The receipt retains those exact configuration/source deltas and
the probe's original executable/dependency identity. No new immutable SDK adoption,
remote CI, full Builder fault qualification or programme completion follows.

GF05 must also resolve Builder's shared frame scopes: its current entry point
does not start the shared capture. Hook up the existing bounded capture convention
or remove those unused scopes before signing diagnostics; the separate existing
product recorder is not evidence that these scopes were collected. No trace cost
or performance improvement is signed by this functional checkpoint.

### GF04 current findings and evidence

[Decision 0011](decisions/0011-Unavailable-Input-Sequences-And-Shared-Preview-Cancellation.md)
closes the two cancellation leads with actual failures. A headless baseline
restored the shared slider from 0.91 to 0.52 yet kept the other window's query
current, and interpreted the next press as `Word`. Unavailable retirement now
clears only its existing text-bridge click sequence. The value-cancellation
coordinator aggregates real retirement outcomes and invalidates/redraws shared
projections once; empty or foreign cleanup does not create a redraw loop.
Native-ID resolution delegates to one private logical-window retirement body.
There is no new public API, freshness flag, document model or cancellation runtime.

The same focused contract preserves foreign click history, text/logical focus,
historical snapshots and valid ordinary click pairs. It rejects the old shared
command until a fresh presentation, and checks idempotent/foreign cleanup. The
initial private-method access compile failure remains recorded; the production
owner reuses its internal bridge selector and the test uses existing command
dispatch. Frozen cancel01 passed 213 contracts across 32 harnesses, documentation,
all/base strict checks, tools, format and both default standard builds.

The new private native report case `cancellation` separately requires all three
new observations. Actual Main/Float each passed 64 controlled one-pass exits,
scoped click reset, live numeric preview rollback, rejected old command in the
other window, and actual presentation recovery. There are 132 budget markers
including four warm-up exits. Eight actual-report/corruption checks reject stale
clicks, missing samples/rollback/shared pending and a foreign PID, while retaining
the historical budget report's older scope and original red result.

The final standard build has a different executable identity from the first
native run, with identical Rust inputs. It was therefore reverified directly at
`f4ff1a36…` / PID 67812: both windows again passed every step. Earlier PID 11056
retains its actual `9a47fc8b…` identity. This is controlled native CPU/window-path
qualification, with no physical input, natural eight-pass exhaustion, display
timing, Root adoption or remote CI claim. Evidence is in
`target/research/input-cancellation-20261031` and the related frame/software runs.

The known stdio callers opt in once at native startup, matching the documented
process-lifetime transport. No current consumer requirement justifies a generic
transport runtime. [Decision 0014](decisions/0014-Stdio-Wait-Budget-And-Process-Lifetime.md)
retains the explicit no-join/no-cancel limit and fixes the reproduced condition
budget defect: a 25 ms request took 10.0106849 seconds and retired transport.
The existing reply function now shares the condition deadline across polls,
reply waits and sleep. Expiry rejects a late reply yet permits the next request;
actual channel disconnection retires transport. No new API, wrapper, dependency
or runtime thread is introduced.

The public test exercises invalid input, bounded wait, a healthy successor and
EOF in an actual child process. Another child must exit on oversized input while
stdin remains open; closing the pipe cannot supply a false pass. Both children
are normal Cargo test harness processes with explicit handles and reader joins.
Final focused03 passed 15 adapter contracts and strict checking. Frozen
stdio-lifecycle/full02 passed 217 contracts / 34 harnesses, CPU workload smoke,
documentation, all/base strict checks, six Node contracts, format and standard
builds. All inputs stayed unchanged; the original red and earlier fixture
versions remain under `target/research/stdio-lifecycle-20261031`. This closes the
bounded transport finding, not the remaining interaction/resource or native and
current consumer-adoption exits.

GF01's maintained gate is `tools/verify/architecture.mjs`, with its focused
contract and Windows workflow step. It requires full Cargo metadata and enabled
declared library features; `--no-deps` is not accepted. Production paths include
normal/build edges, and development-only test dependencies do not imply runtime
coupling. The source inventory/qualification receipts for this checkpoint are
under `target/research/framework-refinement-20261031`.

Keep the previous 210 SDK software contracts and their exact source identity.
Use focused checks for changed code, final family/feature checks for delivery,
and native evidence only for the scenes actually affected. Test current normal,
error, cancellation and resource-bound behavior, rather than static labels or
private helper constants. Use this repository's default Cargo target; preserve
the user's live Agent and other owners' processes. Historical source archives and
release receipts are immutable. A changed development tree must be explicitly
resealed and qualified before publication; old green CI is not its qualification.
