# Unavailable input sequences and shared preview cancellation

Status: Accepted; headless and current hidden Main/Float cancellation qualified.
Date: 2026-10-31.

## Findings to verify

The window-unavailable path cancels Core capture and `PointerClick`, while the
native text bridge owns a separate bounded click sequence. A budget exit has no
OS lifecycle event that necessarily clears that sequence. Verify whether a
later single press is incorrectly classified as a word-selection click.

Value previews are shared by Main and Float. Their cancellation restores the
model, but the current coordinator does not explicitly invalidate the other
window's presented command observation or request its repaint. A completed
presentation of the preview must not remain command-current after rollback.
Reproduce through actual editor/cancellation/query objects before fixing it.

## Decision

Keep one App and its existing window-qualified input/value owners. Separate the
native-ID lookup from the existing per-window cancellation body using one private
logical-window method. This makes the same production retirement path callable
without inventing native windows in headless contracts. No public SDK API, new
state model, transaction registry, scheduler or generic cancellation runtime.

Unavailable-window retirement clears only that window's existing text-pointer
sequence. Ordinary stable draws and the first press of a valid double-click pair
must not clear it. Retain text/history/composition and remembered logical focus;
cancel geometry-dependent captures/previews as before.

The existing value-cancellation coordinator owns shared publication notification.
Use its real cancellation outcomes to invalidate existing shared query state and
request Main/Float redraw once when a shared preview/overlay is retired. Empty or
foreign cleanup must not start a redraw loop. Do not fabricate a presented frame,
erase historical snapshots, or add another freshness/revision flag.

## Qualification

Retain a headless red result protecting the real text bridge, shared preview
rollback and another logical window's published observation. Verify scoped click
reset, preserved foreign click history and draft/focus, actual value restoration,
shared pending admission, idempotent cleanup and successful republishing.

Extend the existing explicit bounded-budget native probe rather than add another
application/entry point. Verify both actual native window adapters retire the
sequence and shared preview, reject old shared commands, and recover presentation.
Use a distinct current report scope for the new observations; do not expand old
reports or claim physical pointer input. Preserve the previously qualified budget,
ordinary, focus and deferred evidence at their actual source identities.

Run focused contracts, relevant family/feature checks and standard default builds.
No Vesta product, provider, author data, user window or existing accepted physical
scene is changed by this work. GF04/the full programme stay open for their other
requirements.

## Outcome

The headless baseline produced `own_click=Word`, retained foreign `Word`, and
`shared_pending=false` after the real slider restored 0.91 to 0.52. The repair
produces own `Caret`, foreign `Word` and shared pending. Its same contract also
checks old-command rejection, fresh publication, historical snapshot retention,
ordinary click-pair preservation, unchanged draft/logical focus, and empty/foreign
cleanup without repeatedly invalidating the other window.

An initial focused compile failed on private helper access. The window owner now
reuses the App-internal bridge selector; the test uses the existing command
boundary rather than widening the private control implementation. The failure is
retained. The full frozen qualification passed 213 contracts across 32 harnesses,
documentation, all/base strict checks, tools, format and both standard builds.

The native `cancellation` report has its own case identity. Its live numeric scrub
and text bridge fixtures use each actual window's real field/viewport/owner, not
OS pointer injection. Both windows passed 64 controlled one-pass exits, with
scoped click reset, numeric rollback and the other window's old command pending;
each step recovered through a real presentation. The 132 budget markers include
four warm-up exits. These are neither physical gestures nor natural eight-pass
exhaustion or display-latency evidence.

Eight report/corruption reviews retain the old report scopes and original budget
failure. The first native binary was `9a47fc8b…` / PID 11056. The final ordinary
standard build differed after the feature checks, so it was verified again at its
actual `f4ff1a36…` identity / PID 67812, without rebuilding or copying the exe.
Both native runs' Rust inputs match the full software qualification byte for byte.
Receipts are under `target/research/input-cancellation-20261031`,
`target/research/frame-settlement-20261031/{cancellation01,finalcancel01}` and
`target/research/refinement-text-host-20261031/cancel01`.
