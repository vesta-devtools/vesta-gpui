# Widget projection and controlled state authority

Status: Accepted; local full-family source/software/native scope qualified 2026-10-31.
Date: 2026-10-31.

## Scope

Continue the finite GF02 Widgets family and related GF04/GF05 boundaries.
Review actual public exports and Standalone/Agent/Builder callers across
composition/theme/feedback, typed editing, popup lifecycle, collections and
documents. Reuse unchanged qualified text/Markdown/input scenes. This is not a
new widget catalogue, author model, scheduling service or platform promise.

Two source leads require original runtime evidence: a fixed-height computed
range can round its maximum offset to the content end and drop the final row
for an admitted fractional viewport; MenuController::step accepts a descriptor
other than the active menu, unlike its key and edge-navigation entries. Preserve
failures before claiming either defect. Public raw projection DTOs still have
explicit caller preconditions; do not reclassify every forged DTO as a new bug.

## Decision

Keep one fixed-range calculation and its existing public signatures, limits and
row projector. Compute boundary decisions with f64 and round a nonrepresentable
maximum offset inward, as the existing variable-height range already does. A
computed nonempty list must keep its final visible row without constructing the
whole data set. Preserve overscan, empty-list and retained-item semantics.

Reject a MenuController::step descriptor whose identity is not the current menu
before changing highlight/path/focus state. Retain the current typed key policy,
submenu reconciliation and slice-returning path API; do not add another session
generation or dispatcher.

Make ModalStack's private entry the sole owner of its ID and optional native
source/opener/policy. Replace the parallel entry/owner vectors with one entry
vector and one admission function. Preserve legacy unqualified local open/close,
window-qualified dismissal, duplicate/capacity errors and public signatures.
Do not replace a convenient public slice API elsewhere just to homogenize
private representations. State copies are UI data, never author permission or
exactly-once application transaction capabilities.

Replace the mixed private `state` module with four responsibility owners:
`scroll_state`, `menu_controller`, `tab_bar_state` and `popup_placement`.
Move the existing implementations and their focused tests; preserve root public
exports and signatures. This creates no facade, crate, registry or second state
model. Scroll validity reuses Core geometry, menu state owns its descriptor
path, tab state owns ordering, and placement is pure geometry.

Reuse Core Size/Rect validity where Widgets currently duplicate those numeric
rules; keep widget-specific positive extents and resource limits local. Map the
remaining actual source/export/caller surface to explicit ownership, projection
preconditions, failure/retirement and resource budgets. Resolve concrete findings
before signing the full family; source review alone is not qualification.

## Acceptance

The remaining chrome review found a third numeric source lead: finite status
item widths can overflow the cached f32 running total; subtracting hidden items
then leaves infinity and incorrectly hides a fitting survivor. Preserve that
red before fixing it. Compute current retained width from the retained set in
f64 rather than maintaining a subtractive cached total; priority/source-order
and existing width normalization stay unchanged. This adds no queue/cache or
new density policy and keeps the existing scan complexity.

The fallible measured-menu boundary must validate complete bounds and anchor
Rects with Core validity, retaining its positive bounds requirement. The current
dimension-only admission can reach user adapters and popup clamping with a NaN
origin or overflowing edge. Preserve a red before replacing that duplicate
partial validation; reject with the existing InvalidConstraint before adapters.
The standalone infallible placement arithmetic still requires valid caller
geometry; no new public error or general layout sanitization is introduced.

Focused01 retained an extra fixture failure: a quarter-logical-pixel viewport
was incorrectly expected to yield a pointer hit although the existing Core
solver rounds final layout edges to whole logical pixels. Preserve the admitted
fractional range and callback obligation; verify layout/hit agreement in an
independently computed drawable one-pixel viewport. Do not change Core rounding
or infer physical/DPI qualification from that fixture.

The subsequent drawable one-pixel fixture also failed with a real zero-height
last row, preserved in red02. This is a separate Core dependency, addressed by
[decision 0024](0024-Precise-Cumulative-Layout-Finalization.md); it is not the
fractional fixture limitation and cannot be passed by weakening the hit check.

Extend the existing large-list and submenu composites with the original failing
branches, preserving their assertions. Verify fractional final-row callback,
clip/hit/layout agreement and overscan bounds without a multi-million-row
allocation; verify stale parent/child descriptors do not replace the active
highlight. Reuse/extend modal normal, foreign, LIFO, failed-open and teardown
contracts for the private representation change. Run focused packages, then
whole-SDK/docs/all/base strict/tool/format checks and normal standard builds.
Only repeat affected hidden/nonactive integration scenes, bind runtime hashes
and process identities, and verify retirement/locks. No native physical input,
reader, DPI, general speed improvement or immutable product adoption follows
from preparation or the source review.

Final qualification: full02 preserves 232 existing tests/38 harnesses and CPU
smoke, two compile-fail docs, all/base strict, six Node groups, format and both
ordinary standard builds, all zero with unchanged guarded source. Existing
hidden/nonactive Inputs Main64/Float64 pass on the actual standard executable;
zero retry injection, normal exit, observed child retirement and two executable
locks verified. Local receipts: `target/research/widgets-family-20261031/`.
Original red01–04, focused fixture failures and red02's expected-count controller
failure remain. Full01 also passed; full02 additionally removes the residual
Core traversal marker. Common product adoption and remaining programme gates
remain open.
