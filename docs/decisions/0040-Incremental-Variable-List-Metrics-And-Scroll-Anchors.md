# Incremental variable-list metrics and scroll anchors

Status: Accepted; implemented with local software validation on 2026-11-09.
Date: 2026-11-09.

## Problem and ownership

VariableListMetrics stores caller-measured heights in the current item order.
Previously any height change required rebuilding its entire prefix vector.
Standalone also retained an absolute scroll offset through height/order changes,
so the item under the viewport could move even when its stable identity survived.

Keep this geometry in Widgets and item identity, measurement, data mutation,
selection, focus and scroll policy in Host. Preserve the existing borrowed range,
row callback, Element/ScrollBox projection and content/visible/overscan limits.
Implement independently within this Apache-2.0 repository; introduce no BUI source,
dependency, renderer, text engine or measurement scheduler.

## Decision

Retain one f32 height per row and an f64 Fenwick index over blocks of 64 rows.
Construction is O(n), a point update O(log(blocks)), and prefix/range lookup
O(log(blocks) + 64). Sequential visible projection scans its rows once. The index
adds about 3.125% storage over the heights, rather than an f64 entry per row.
Continue rounding cumulative edges to f32 for the existing projection; content
admission keeps the existing 16,777,216 logical-pixel limit.

`update_height` validates index, height and resulting total before mutating. It
returns whether geometry changed; failed updates leave every field unchanged.
`ItemIndexOutOfBounds` is an additive error variant used by the new entries.
Downstream exhaustive matches must add it; old constructor/range failures remain
unchanged. This is a developer-preview source change, not a new package release.
Reordering or changing item count still requires rebuilding from caller data.

Computed fixed and variable ranges can capture a `ListScrollAnchor`: the first
visible index and an offset within that row, excluding overscan. It is derived UI
data, not an identity, version ticket, bottom-follow policy or second data model.
Host captures its existing stable item ID, resolves its new index after mutation,
and restores through `offset_for_anchor`. A smaller row clamps inside that row;
viewport/content boundaries take precedence. Host decides the removed-item policy.

Standalone consumes the same API for data/height changes and a selected-row height
edit. Its ordinary route, retained focused child, semantics and input reconciliation
remain authoritative. No new public runtime, dependency or package is required.
Purely fixed-height data actions keep their original scroll/focus repair policy;
the fixed anchor is consumed when switching into variable-height mode.
Only a row represented by a current window route is an observed scroll anchor.
Initial/hidden data preparation does not pin its unshown first item. This reuses
current route identities and adds no readiness flag or historical identity map.

## Qualification

Check updates against independently accumulated reference edges, rejection
atomicity, block boundaries, fractional geometry at the content limit, anchor
remapping/shrink and bounded projections with real layout/action/hit/semantics.
Exercise the existing Standalone consumer and compare incremental updates with
full rebuilding using equal deterministic inputs. CPU timings are workload samples,
not FPS, GPU execution, physical input, screen-reader or universal speed claims.
Record actual commands/results before marking this decision qualified.

### Local software results

- `cargo test --locked -p vesta-gpui-widgets --all-targets -j 2`: 38 tests pass;
  all example targets compile, including the equal-input update probe.
- `cargo test --locked --workspace --all-targets --all-features -j 2`: 260 tests
  pass across 41 harnesses, including all 47 Standalone tests. The unchanged
  paging/focus regressions exercise fixed and variable rows in both orders.
- `cargo test --locked --workspace --doc --all-features -j 2`: 2 doctests pass.
- Strict all-target/all-feature Clippy passes for the workspace excluding the
  separately developing `vesta-trace` binary. Strict renderer base-feature Clippy
  also passes. Full-workspace strict Clippy reports `collapsible_if` in the
  unrelated, uncommitted `apps/vesta-trace/src/perfetto/target.rs:121`; that file
  is preserved. This is not a claim of a green full-workspace strict run.
- The five existing Node contract suites pass all 7 tests. Resolved architecture
  still has eight libraries and its permitted dependency directions. Affected
  package formatting and `git diff --check` pass.

Raw final Cargo logs and workload receipts are under
`target/research/list-updates-20261109/`. The existing lockfile omitted two already
declared Trace dependencies (`getrandom` and `vesta-gpui-webview`); offline Cargo
metadata added those two dependency references, with no registry version changes.
Other ongoing Trace/Perfetto changes are outside this decision. Source sealing,
registry publication, remote CI, physical input and screen-reader qualification
are not performed by these software checks.

The equal-input CPU probe reports about 169–171 microseconds per complete rebuild
versus 0.155–0.164 microseconds per incremental edit/anchor restoration at 100,000
rows. It also records the higher bounded-scan query cost. See
[the performance checkpoint](../PERFORMANCE.md#incremental-measured-list-checkpoint--2026-11-09)
for the profile, raw ranges, shared-machine conditions and measurement limits.

### Dense-table follow-up

[Decision 0042](0042-Dense-Table-Text-Preparation-And-Clip-Batching.md) completes
the measured dense-table follow-up. Its independent checkpoint identifies text
cache churn as the CPU bottleneck and batches fitting text runs through the
existing atlas uploader. Cell Elements, rich content, focus, accessibility and
Agent queries stay intact. Its scoped native pixel/draw result and final software
checks are separate from this measured-list result and downstream product adoption.
