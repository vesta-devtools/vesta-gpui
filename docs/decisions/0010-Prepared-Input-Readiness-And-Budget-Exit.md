# Prepared input readiness at bounded layout exits

Status: Accepted; controlled budget exit, stable acquire deferral and ordinary
frame qualification completed; consumer integration remains in GF03.
Date: 2026-10-31.

## Finding

The bounded Main/Float preparation mutates derived routes/focus/scroll during
reconciliation. An exhausted draw retains the last hit/text maps, so another
event can see parts of different projections. Presented query publication alone
does not make the native input path safe. Preserve the already qualified stable
CPU projection on renderer deferral; address incomplete preparation separately.

## Decision

Each native window owns one private `input_layout_ready` bit. It is false before
the first complete projection and while a draw reconciles, and true only after
that draw has installed coherent text and hit geometry. It remains false at a
budget/early preparation exit; a stable CPU projection remains usable if acquire
later defers. This is validity of host-owned derived data, not another UI model,
renderer state, public scheduler or frame revision.

One native admission boundary handles unavailable CPU projection and unavailable
display, preserving lifecycle, close, focus, resize, redraw and modifier events.
GPU retry remains a GPU-only action. Pending layout rejects semantic/logical
input with `presentation_pending`; existing cancellation and window lifecycle
routes retain their admission. A budget exit cancels geometry-dependent captures and
previews through the existing cancellation path, preserving drafts/focus and
requesting a future redraw. Partial preparation publishes no presented snapshot.
Keep cancellation and admission with the window/input owner, not GPU state.
Blocked pointer motion must still update the native position cache without hit
testing, gestures or actions; otherwise a press after recovery can use the old
cursor coordinate. Keep pointer leave invalidation and modifier sampling too.

## Evidence required

Extend the existing explicitly requested hidden probe with an isolated `budget`
case. Its process-local one-pass limit exercises the actual production budget
exit using a real changed split width, without a nonconvergent loop or OS input.
Normal hosts retain eight passes. This is controlled budget-exit qualification,
not evidence that a natural eight-pass exhaustion was observed.

The prior query-enabled probe also reached the ordinary stdio startup branch,
which overwrote its control permission. Separate this private in-process query
fixture from explicit stdio startup; do not widen control-method visibility.
Exercise the existing request/reply dispatch synchronously through its bounded
channel. Earlier native geometry results retain their actual process behavior;
they cannot substantiate the old comment that no stdin worker started.

Retain the native red result. Per window, verify 64 changed-width draws reach
their limit without renderer entry/publication, reject native-event admission
and the real semantic command, release an armed capture and retain draft/focus.
Each step must then recover through a successful presented revision outside its
draw. Qualify the unchanged ordinary eight-pass path, focused contracts, strict
checks and standard build. Keep the Builder bridge and remaining programme gates
open until their own evidence exists.

## Outcome

The native baseline exhausted its controlled limit yet admitted input/Focus and
retained capture. The final budget run verified 128 exits, capture release,
semantic/logical rejection, lifecycle admission, preserved focus/draft, current
cursor/leave cache, no renderer/publication and successful recovery per step.
An additional stale-cursor native failure is retained. The fault-feature run
verified 128 acquire deferrals with stable CPU input still usable. The default
eight-pass standard binary passed the directed focus sweep with 64/64 submissions
per window. Frozen family qualification passed 212 contracts and all strict/build
checks; twelve report/corruption reviews passed. Actual receipts are under
`target/research/frame-settlement-20261031`, software logs under
`target/research/refinement-text-host-20261031/budget02`. These are controlled
native CPU paths and in-process requests, not physical input/display observations.
