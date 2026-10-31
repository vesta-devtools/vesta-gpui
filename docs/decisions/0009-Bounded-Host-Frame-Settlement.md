# Bounded host frame settlement

Status: Accepted; Main/Float settlement and stable CPU text projection qualified;
remaining GF03 boundaries open.
Date: 2026-10-31.

## Finding and evidence required

Main and Float presentation update derived document/wrap/scroll/focus state and
may return before the renderer, asking a later redraw to finish reconciliation.
The earlier Agent divider regression establishes a real consumer failure, but
does not prove that this example has the same failure. Capture a reproducible
forward/reverse split-width workload through each actual native draw entry first.

## Decision

Keep event-loop, application data and preparation policy in the host. Do not add
a Core scheduler, another layout engine, event queue or application model.
One draw attempt may reconcile its projection to a bounded fixed point before
submission. Layout/text/scroll/hit/paint and published semantics must describe
the same accepted projection. GPU unavailability, retirement and acquire retries
remain renderer deferrals rather than layout retries. A bounded-out preparation
does not publish a new observation or busy-spin inside the same native event.

Review Main/Float and existing consumer paths, including pending focus, popup
closure, wrap/scroll/viewport changes and failure. Retain one CPU frame trace
and cumulative frame timing per draw, with explicit preparation passes and
budget deferrals. No generic abstraction is admitted only to share a loop.

Add an explicitly requested, hidden, self-closing example probe. It uses the
actual Main/Float production entries and typed workspace ratio changes, never
OS input or another process's window. It does not read/write user preferences
or saved workspace layouts. Keep 64 forward/reverse changes per window, exact
text and stable owners, bounded capture and normal GPU retirement. Its verifier
requires complete frames and checks renderer entry separately from successful
submission. Retain the original red result and the same workload for the fix.

Pure host changes require focused contracts, strict checks and standard builds.
They do not requalify waived physical gestures, DPI, reader speech or unrelated
accepted scenes. GF03 stays open until its full stated contract is demonstrated.

## Qualified checkpoint

The actual Main/Float sweep reproduced 13/64 and 1/64 renderer entries before
the fix. The same Inputs workload after the fix submitted all 64/64 in each
window with at most two preparation passes. Documents and focus cases also
submitted 64/64 per window, using at most three passes, preserving the draft,
document copy binding, independent focus memories and single caret owner.
The first focus fixture failed by expecting a window's remembered focus to be
erased; that failure is retained and the fixture follows the existing contract.

Frozen family qualification passed 212 contracts, strict feature checks and both
standard builds. A directed final-focus run verifies the final standard binary.
Actual native processes exited and GPU resources retired normally. These are
CPU-path observations, not physical input or display latency measurements.
Receipts are in `target/research/frame-settlement-20261031`; the programme records
the prepared-input/backend-deferral and Builder bridge issues still to review.

## Prepared input during renderer deferral

The host accepts its stable CPU hit/routes/scroll projection before calling the
renderer, but installs text viewports only after successful submission. A deferred
acquire can therefore leave text pointer mapping from another layout. Reproduce
this with the existing finite acquire-fault feature and the same split sweep.
The intended owner is the prepared PaintList: a private example helper projects
its text commands using the existing snapshot viewport method, before renderer
entry. It shares that immutable geometry with hit/paint; it does not reimplement
text layout or publish a new presented query/accessibility snapshot on deferral.
Keep the existing RenderOutput geometry available to callers; no public API or
second input/frame runtime is needed. Retain a red capture before the change.

The native red capture retained a 360-wide text view for a new 174-wide input
layout. After the change, 128 acquire-injected steps retained coherent current
text/hit geometry without publishing a presented revision, then each recovered
through actual submission and target rebuilding. Successful frames compare the
private prepared views with the production painter's output. The final default
standard executable also passed the directed focus sweep; the family qualification
passed all 212 contracts and strict/build checks. Original compile and fixture
failures remain in the receipts. This closes the stable-CPU/backend-deferral
finding, not partial preparation on budget exhaustion or the consumer bridge.
