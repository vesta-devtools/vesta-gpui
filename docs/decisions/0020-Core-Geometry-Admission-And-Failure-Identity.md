# Core geometry admission and failure identity

Status: Accepted; local geometry/error boundary qualification passed; full Core review remains open.
Date: 2026-10-31.

## Scope

Continue GF02's complete Core family audit. This decision closes the coherent
value/layout admission boundary and its caller migration, not the entire family.
Keep the earlier text, trace, inspection and input qualifications at their exact
identities. The family review must still cover render payload visibility and
primitive semantics as well as the remaining exported contracts.

The current code has three concrete leads: `Rect::intersection` uses floating
min/max that can erase NaN operands; layout style/scroll input and completed
geometry lack one numeric admission rule; and public `ElementScopeExhausted`
cannot enter a Host's ordinary standard-error chain. The last has an actual
compile failure in the preceding projection audit. Reproduce the others before
claiming them. A separate unexported `Style.polyline` payload type and the
continuous-polyline comment/segment implementation mismatch remain render
review candidates; do not silently sign or downgrade those contracts here.

## Decision

Core value types own basic numeric validity. Add `Size::is_valid` for finite
nonnegative extents and `Rect::is_valid` for those extents plus finite origins
and finite computed edges. A positive extent must advance its corresponding
edge at f32 precision. Zero area is valid, though it is empty and cannot
be hit. Raw constructors/fields remain value construction, not validation.
Containment/intersection reject invalid rectangles instead of recovering an
apparently valid hit/clip region. Keep finite negative origins and inclusive
edges. Low-level arithmetic such as `inset`, rotation and vertices retains
explicit valid-input preconditions; it is not another validation layer.

Reuse this value rule in current image and Dock consumers, preserving the image
path's positive-area policy. Remove duplicated finiteness/edge
predicates. No alternate geometry model, native policy or new dependency.

The existing layout-to-Taffy bridge owns finite numeric input admission for
dimensions, offsets, flex/grid/spacing and ScrollBox offsets. All stateless,
retained and content-measurement paths already call this bridge. Consolidate
its conversion and validation in one private cohesive module, retaining the
same Taffy solver. Preserve all finite input semantics, including existing
negative-value normalization and negative margins; keep specific aspect/baseline
errors. Nonfinite layout inputs carry the element ID in `LayoutError`.

Reject nonfinite, negative-extent or overflowing published node/clip/scroll
geometry, and invalid preferred/min-content sizes, with a typed element-bearing
layout error. Never publish a partial usable tree. Retained failures use the
existing reset/recovery path; do not add a readiness flag or second cache.
Visual-only payload validation is not moved into the layout solver.

Implement the standard `Display`/`Error` traits on the existing scope-exhaustion
unit type; do not add a wrapper, change IDs or force the global allocator to
exhaust in a test. Ordinary external Host propagation is the relevant contract.

## Qualification and migration

Preserve original external compile/runtime failures. Extend the existing error
consumer and use focused value/layout contracts that exercise shared clipping,
stateless/retained/measurement rejection and valid recovery. Cover finite
negative/zero behavior and finite-input arithmetic overflow. Do not add getters,
pixel snapshots, static enum counts or redundant per-helper tests.

Run affected Core/Dock contracts before frozen whole-SDK, documentation, strict
feature, tool and standard-build checks. Use directed existing Main/Float native
coverage only for changed preparation/geometry. Record actual standard artifact
and process identities and normally retire all owned processes. Preserve old
physical, reader and DPI evidence without expanding its scope.

New `LayoutError` cases require exhaustive-match migration in this preview;
matching typed causes replaces acceptance of invalid numeric layout. Keep
published-pin products unchanged until joint GF06 adoption. The complete Core
family and the GF01–GF06 programme remain active until their actual exits pass.

## Outcome

The original red01 preserves two ordinary Host compile diagnostics for scope
error propagation and three independent runtime failures. NaN rectangle min/max
created a visible region; all twelve invalid style/scroll cases were accepted by
all three layout paths (36 admissions); finite nested positions overflowed while
layout still reported success. No global allocator exhaustion was simulated.

One value predicate now serves Rect containment/intersection, image/Dock and
text metric sizes. The complete existing Style conversion moved into one private
layout bridge; its conversion tokens are unchanged, with only the finite-input
admission added to the old node-style body. No new solver/model/runtime/dependency
was introduced. The existing retained error reset recovers on the next valid
input. Specific typed failures retain the actual offending element identity.

Focused01 retained an extraction import failure. Focused02/03 found a fixture
identity mistake: rounded child geometry is valid empty geometry, and its
grandchild (ID 3) is the first overflowing published origin. The implementation
already rejected that correct ID; the contract now asserts it explicitly for
both engines. These failures remain recorded rather than being overwritten.

Focused04 and frozen full01 passed. The latter contains 227 contracts across
37 harnesses, no failures/ignored tests, CPU workload smoke, two existing
compile-fail documentation boundaries, all/base strict, six Node groups, format
and both ordinary standard builds. All twelve manifests/lock inputs are unchanged.

Actual standard Standalone 639d0624… / PID 56704, created
2026-10-31T06:42:29.3270177Z, passed the existing inputs probe with 64 qualified
Main and 64 qualified Float submissions, zero injected faults, normal exit and
complete descendant observation. Both windows stayed hidden/nonactive and no
OS input was supplied. Its executable and frozen runtime inputs match full01.
This does not add physical/DPI/reader evidence or a performance improvement claim.

Exact source, artifact, process and preserved-failure receipts are in
`target/research/core-family-20261031`. The complete Core export/caller review
remains open, including the render candidates above, the owner-registry failure
contract and the remaining text/semantic/inspection/trace/render boundaries. Those candidates
have not inherited these runtime results. Product adoption, remote CI and the
full programme are not complete.
