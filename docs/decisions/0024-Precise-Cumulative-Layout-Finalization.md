# Precise cumulative layout finalization

Status: Accepted; local regression/software/native scope qualified 2026-10-31.
Date: 2026-10-31.

## Reproduced dependency

Decision 0023's fixed-list range repair materialized its last row, but red02's
ordinary one-logical-pixel viewport still could not hit it. The actual LayoutTree
published the final explicit one-pixel row at height zero near 16,777,216 logical
pixels. This is a Core finalization defect, not a reason to narrow the list
extent or waive its hit/paint contract. The prior Core source-family review
remains historical; this newly reproduced cross-family branch needs repair and
current qualification.

The pinned Taffy 0.12.2 std rounding uses `(value + 0.5).floor()` in f32. Near
the f32 integer precision edge, adding one half moves an already integral
coordinate to the next integer. Core also replaces local positions with Rust
round-half-away-from-zero while retaining Taffy's sizes, so negative half ties
can make positions and dimensions disagree. Preserve a focused Core red for
adjacent negative-half boxes as well as the original large-list hit failure.

## Decision

Taffy remains the only topology, style, cache and raw layout solver. Core owns
one finalization of its derived output. Keep the existing whole-logical-pixel
quantum and use cumulative raw origins in f64, with round-half-toward-positive
edges, for both local positions and sizes. Quantize scrollbar dimensions with
the same rule. Adjacent raw edges must share one final edge; explicit one-pixel
boxes at the admitted list extent must remain hittable. Fractional boxes may
still round to zero at this existing quantum; this is not DPI-dependent layout.

Replace the delegated f32 rounding traversal and cached rounding-origin copy
with one private finalization traversal. Its private output contains only the
location, size and scrollbar dimensions actually consumed by Core; do not copy
or round unused third-party box fields or expose that private representation.
Keep immutable public LayoutTree, existing error admission and retained rebuild
recovery. No new solver, style bridge, public API, generation, runtime, dependency
or geometry repair in a widget/Host.

The old RoundTree-only TraverseTree marker implementation/import is also
removed; raw compute algorithms require TraversePartialTree only. Full01 passed
before this final residual cleanup; final qualification must bind the cleaned
source, not borrow that earlier green.

This intentionally corrects the old negative-half local-position mismatch to
the common edge policy, rather than silently preserving two rounding rules.
Record the preview behavior change and qualify existing fractional/RTL/retained
tests, real text/layout/paint agreement, full SDK feature checks and the existing
hidden/nonactive Main/Float integration. Original Core review evidence cannot
qualify changed runtime input; no physical, reader, DPI or speed claim follows.
