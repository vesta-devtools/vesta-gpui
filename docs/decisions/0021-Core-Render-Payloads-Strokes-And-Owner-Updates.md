# Core render payloads, stroke topology and owner updates

Status: Accepted; local render/owner boundary qualification passed; full Core review remains open.
Date: 2026-10-31.

## Scope and existing callers

Continue the fixed GF02 Core family review, including its remaining GF04 owner
lifecycle contract. Widgets, Workspace chrome, Motion and the published-pin
Agent/Builder already use Style polylines. Vulkan consumes the one shared CPU
vertex function. The same products use the Core owner registry before publishing
structured observations. Neither boundary should acquire a second Host copy.

Three source candidates from decision 0020 need actual failures: the public
`Style.polyline` payload cannot be named outside its private module; the claimed
connected stroke is currently separate segment quads; and an owner update can
mutate old references before reporting generation exhaustion. Preserve original
compile/runtime evidence before treating these as reproduced defects.

## Decision

Re-export the existing Polyline value type from Core. Keep its fields and the
existing Style constructor; add no wrapper, conversion model or public module.
Document the value/paint/CPU preparation boundary, ownership, coordinate and
validation requirements against actual exports and callers.

Keep the current polyline entry point and backend-neutral ShapeVertex stream.
Use one private CPU stroke module with open butt endpoints and bevel joins.
Consecutive duplicate points add no segment; an exact repeated first point closes
the join. Ordinary bends share the inner offset intersection and one outer bevel
triangle, preventing a hole and duplicate coverage at that local join. Use f64
intermediates; no unbounded miter spike or new join/cap options. If the inner
intersection lies beyond an adjacent segment, retain its bounded segment ends
and fill only the outer wedge from the bend center. Reversals, very short runs
and self-intersecting strokes may overlap; do not promise global path-union or
single-coverage fill semantics there. Keep deterministic O(n) bounded-per-segment
preparation, with no GPU, I/O, cache or new dependency. Reject invalid input or
unrepresentable vertex output as an empty mesh, never partial/nonfinite vertices.
Do not replace an observed topology defect with a weaker comment.

Stage owner generations and changed entries locally before altering the existing
registry. If capacity fails, retain instance, cursor and every window's references.
On success apply existing instance/window retirement and the staged entries once.
An unchanged repaint or removals-only update can succeed at capacity. Retain the
existing stable error code and public Result signature; no registry clone,
second generation source, failure flag, scheduler or compatibility wrapper.

## Qualification

Use one external consuming projection for payload naming and value-to-paint
flow, a focused stroke topology contract for holes/local coverage, duplicate/
closed/reversed/large/invalid geometry, and one owner transaction contract for
failed same/new-instance updates, window isolation, final capacity and retirement.
These protect cross-module geometry and stable identity state, not exact pixels,
static labels or simple getters. Keep original red receipts and fixture failures.

Run relevant Core/Dock/Widget consumers, frozen whole-SDK feature/doc/strict/tool
checks and both ordinary standard builds. Reuse the existing directed hidden,
nonactive Main/Float probe for changed native integration, recording its actual
source/artifact/process identity and normal cleanup. CPU topology evidence is
not a new physical-pixel/display result. Preserve prior accepted/waived scenes.

The complete Core export/caller audit must still be reconciled before that family
closes; these three fixes alone cannot sign it. Published-pin products migrate
at the shared immutable GF06 adoption exit. No new physical/DPI/reader support,
remote CI or programme completion is inferred from local checks.

## Outcome

Red01 retains an actual external `Polyline` import rejection and two Core
runtime failures: an ordinary outer bend had zero coverage, and failed updates
corrupted both the same-instance and changed-instance reference sets. The other
60 Core contracts passed in that original run. No private capacity state is
presented as a naturally observed production exhaustion.

The existing payload is now nameable. One private CPU body serves the unchanged
entry point and vertex/shader format. Topology checks cover the original hole,
local inner single coverage, six bend angles, both directions, closed/duplicate
points, long finite paths, short/reversed runs and invalid/unrepresentable meshes.
The external consumer also follows the same value through layout, paint and
preparation without mutation. Staged owner changes retain old references on
failure; last-capacity, unchanged repaint, removal and foreign-window retirement
use the same original registry. No second generation source or dispatcher exists.

Focused01 passed Core/Widgets/Workspace consumers and strict checks. Frozen
full01 passed 230 contracts / 38 harnesses, no failures/ignored tests, CPU smoke,
two existing compile-fail documentation boundaries, all/base strict, six Node
groups, formatting and both ordinary standard builds. All twelve manifests/lock
inputs are unchanged; no production dependency or runtime was added.

Actual standard Standalone 7cb53376… / PID 61908, created
2026-10-31T07:07:36.4750902Z, passed the existing inputs probe with 64 Main and
64 Float qualified submissions, zero injected faults, normal exit and complete
descendant observation. Frozen runtime and executable identities match full01.
Both windows remained hidden/nonactive; no OS input, new physical-pixel/DPI/
reader result or measured speed improvement is claimed. Native integration
does not replace the separate CPU topology evidence.

Exact source/artifact/process and original failure receipts are under
`target/research/core-render-20261031`. The
[preparation contract](../RENDER-PREPARATION.md) covers the complete Core render
surface, including raw-helper preconditions and resource/Host limits. The full
Core export/caller and text/semantic/inspection/trace/raster resource review
remains open. Source-review leads there have not inherited this qualification.
Remote CI and immutable product adoption remain delivery exits.
