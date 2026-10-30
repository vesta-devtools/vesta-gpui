# Example scene and API boundary ownership

Status: Accepted; qualification tracked in REFINEMENT.md.
Date: 2026-10-31.

## Findings

`ViewportScene::acceptance_scene` exports a fixed pair of cubes, IDs, colors and
selection. Actual repository callers are Standalone, two viewport tests and
three renderer tests; Vesta product sources have no caller. The first search
missed the renderer's tests; the failed family compile is retained. This is demonstration content, not a
neutral camera/scene contract. The private cube/array helpers serve only that
fixture and its tests.

The renderer API document also owns the entire eight-package support matrix and
family migration policy. That makes a renderer-specific file an unrelated
authority for base UI, widgets, native input and model-only libraries.

## Decision

Remove the experimental public fixture method from the development candidate.
Standalone owns the same scene in a private viewport_demo module. Viewport and
renderer tests keep their original input data in private test-only fixtures,
with all existing assertions. Fixed test input is not a public scene constructor
or a second production authority. The neutral
scene types, construction, selection, picking and camera APIs remain. Document
the source migration; do not retain a demo compatibility facade in the library.
Preserve the visible scene's IDs, geometry, transforms, colors and selected state.

Move the existing family support matrix and migration policy to the SDK docs
root, and replace the renderer's copy with a link. Per-package documents retain
their detailed contracts. This is relocation of one authority, not an additional
hand-maintained export catalog or a new acceptance ledger.

Use focused viewport/example checks, the existing full family qualification and
ordinary standard builds. Recheck public fixture absence, documentation links
and the resolved package graph. Pure content relocation does not rerun unrelated
native, DPI or reader scenes or restart the user's Agent.

## Outcome

Run05 and docs01 passed the full software checks, ordinary builds, resolved
graph, warning-free public rustdoc and local links. All 52 original viewport
function bodies and fixed scene fields are preserved, while generated public
API documentation excludes the demo method. Failed/interrupted runs remain
recorded; no new native or product-deployment result is inferred.
