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
| GF03 — Frame and layout contract | Audit main/float/consumer preparation; reproduce continuous-width failures, define one clear reconciliation/invalidation/commit contract, verify text/scroll/hit/paint agreement and bounded retry | In progress; Main/Float reflow, deferred geometry and controlled budget-exit admission qualified; remaining consumer review/integration open |
| GF04 — Interaction and lifecycle | Close concrete existing input/text/IME/focus/popup/selection/resource gaps; make transport, cancellation, error and ownership limits explicit; preserve existing accepted scenarios | Open |
| GF05 — Performance and diagnostics | Reproducible static/resize/text/collection workloads, disabled/enabled trace cost, bounds/retention/idle checks; select and fix measured bottlenecks with comparable evidence | Open |
| GF06 — Usability and delivery | Small reusable host recipe, capability-oriented docs, matching exports, clean feature checks and CI, exact SDK source identity and immutable Vesta consumer validation | Open |

Each gate is split into concrete findings as its review runs. New findings must
state source location, failure/redundancy, intended owner, minimal change and
acceptance; additions to scope require a recorded reason. Gate completion does
not erase earlier failed runs, waived scenarios or unsupported platform limits.

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
  shutdown/join owner. Document that fact now and review actual embedding needs
  before deciding whether a different transport lifetime is required.

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

Remaining GF03 consumer review is concrete: Builder's embedded Agent `layout()`
changed result currently asks for a later redraw but still submits the earlier
widget. Do not call that a reproduced Builder starvation defect or copy business/
author updates into a generic retry loop. Review the other consumer publication
boundaries too; the programme stays open until that scope has evidence.

GF04 also retains two source-audit leads at unavailable-window cancellation:
the text bridge's click history is separate from `PointerClick`/capture, and a
shared value-preview rollback must invalidate/redraw the other window. Reproduce
and trace their existing reset/notification paths before changing or signing them.

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
