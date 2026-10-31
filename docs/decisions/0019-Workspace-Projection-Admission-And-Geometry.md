# Workspace projection admission and geometry ownership

Status: Accepted; complete local family/software/native qualification passed.
Date: 2026-10-31.

## Scope and findings

Complete the existing GF02 Workspace projection family review against the
validated model and current Standalone/Agent/Builder consumers. The projection
borrows the model; it never owns author state, native windows or another model
generation. Review all view/header, geometry/frame, drag/resize, reveal and
overlay exports, including the Host freshness/admission requirements.

The preceding audit identified four concrete candidates to reproduce: duplicate
header command IDs yield ambiguous Element identity; split geometry can emit
NaN or accept overflowing bounds; reveal checkpoints advance before the active
tab is realized; and frame construction silently skips missing layout nodes,
which can shift pre-move insertion slots. Preserve the original failing receipts.

## Decision

Replace the misleading unit identity error with one typed projection error for
identity overflow and invalid/duplicate command IDs. Keep current valid stable
IDs and UI ranges. Check header IDs before emitting controls, including inactive
header inputs; no replacement wrapper, command allocator or registry.

Make `DockFrame::from_layout` fallible. Reuse Core `LayoutError` for missing or
invalid layout data; do not introduce a competing layout error vocabulary. All
required node identities must exist, even when a tab is offscreen. Retain full
tab rectangles in authored order and full split bounds; pointer-facing pane
regions respect inherited clipping. Zero-area layout is legitimate and yields
no interactive area. Missing nodes or nonfinite/negative bounds must not publish
a partial interactive frame. Host callers retire an old frame on failure.

Give shared numeric admission one private geometry owner, used by split helpers,
resize sessions and window routing. Derived/input rectangles require finite
origins/extents/edges; hittable rectangles also require positive size. Invalid
scalar split input returns the documented neutral ratio. Low-level region/preview
value construction keeps explicit validated-input preconditions. No native/DPI
policy, new scheduler or second layout algorithm enters the library.

Only record a reveal checkpoint after both the active tab and its scroll
viewport are realized. Use the existing checkpoint and scroll maps; no extra
readiness/generation state. Preserve manual scrolling on unchanged geometry.

Document one coherent Host input frame: regions, layout, current model and
window coordinate mapping belong to that same preparation. The Host owns
native/fallback presentation, front ordering, occlusion, capture and invalidation;
existing release gates recheck model ownership. Logical container identity is
not an OS window handle. Do not invent a library model revision or infer native
window policy from projection geometry.

The direction review also reproduced two RTL failures (red02): a physical side
drop projects onto the opposite side, and a resize handle at authored ratio 0.25
is interpreted near 0.75. Carry actual `LayoutNode.flow_direction` in pane/split
frame records. Translate physical horizontal sides/deltas into authored model
order at that derived geometry boundary; vertical semantics are unchanged.
Resize sessions consume the existing complete `DockSplitFrame` snapshot, rather
than reconstructing a session from disconnected bounds/axis/direction arguments.
Scalar helpers require explicit direction. No default-LTR compatibility path
may silently preserve these demonstrated bugs; record the preview migration.

Qualify focused public contracts, existing composites and whole-SDK software
checks, then directed native Main/Float coverage of changed preparation. Record
the preview migration and current artifact/process identities. Keep prior
physical/reader evidence and waivers, with no new physical claim. Agent/Builder
remain on their published pin until the shared immutable adoption exit.

## Outcome

The original red01 produced four failing contracts: duplicate header identity,
nonfinite split admission, premature reveal checkpoint and partial-tab insertion
order. Red02 independently reproduced physical horizontal drop reversal and
mirrored resize ratios under actual RTL layout. They remain immutable evidence.
Existing composite tests now cover both directions, apply real model actions
and reproject their results. One new public-frame contract protects missing tab
identities and clipping without introducing pixel snapshots or per-helper tests.

Focused04 and the actual Standalone all-target caller check passed. Full01's
contracts passed, but its strict step rejected three redundant test-fixture
field names introduced by the migration. After that correction, frozen full02
passed 224 contracts / 36 harnesses, the CPU workload smoke, two existing
compile-fail documentation boundaries, all/base strict checks, six Node groups,
format and both ordinary standard builds. No manifest or lockfile changed.

Actual ordinary `target/debug/vesta-gpui-standalone.exe` 2fb2719f… / PID 27024,
created 2026-10-31T06:23:36.0121699Z, then passed the existing inputs reflow probe:
64 qualified Main and 64 qualified Float submissions, no injected fault, normal
exit and complete descendant observation. Frozen runtime inputs and executable
identity match full02. No physical input, new DPI/reader claim, product update,
remote CI or programme completion follows from this finite native qualification.

The complete export/caller audit, six production-module hashes and final source/
artifact/process receipt live under `target/research/workspace-projection-20261031`.
The [public contract](../DOCK.md) records all Host preconditions and preview
migrations. The local projection family is closed. Other families, GF03–GF06
and immutable product adoption remain open. A separate Core finding surfaced
when a fixture used ordinary `?` on `ElementScopeExhausted`; that missing standard
error trait is explicitly retained in the Core ledger, not reported as fixed.
