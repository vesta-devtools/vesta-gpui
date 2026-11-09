# Dense table text preparation and clip batching

Status: Implemented; local software and scoped native pixel/draw checks passed.
Date: 2026-11-09.

## Evidence and scope

Continue decision 0040's independent BUI-inspired work in the existing Apache-2.0
architecture. No BUI source or upstream GPUI adapter is used. The public table
projection already bounds rows and preserves stable cell identity, typed actions,
focus, AccessKit and Agent inspection. Removing those Elements would require a
second presentation mechanism across all those consumers.

First measure the existing pipeline with `table_pipeline`, a Standalone example
using Host-supplied Segoe UI and Microsoft YaHei fonts. Its 32-row, 8/24-column
tables contain 264/792 header and cell strings. The initial warmed CPU samples
showed median paint preparation of 95.066/300.075 ms, versus tree projection of
0.218/0.657 ms. The 256-entry positioned-run cache is smaller than either text
working set; the 512-entry shaping cache is smaller than the larger set. Repeated
sequential frames churn those caches even though their byte budgets have room.
Changing cell ownership is not the first useful response to that evidence.

## Decision

Retain the current projection, layout solver, cell identities and rich content
path. Increase the existing per-cache static-text entry ceiling from 512 to
1024 while retaining each cache's 128-KiB source-byte budget. Increase positioned
run retention from 256 to 1024 entries and from 8192 to 32768 aggregate glyphs,
retaining its eight-MiB conservative referenced-glyph byte budget. Metadata and
retained shaping layouts can increase; these limits do not describe total process
memory. Larger or changing working sets can still miss.

Separate the unchanged **8192 glyphs per returned run** admission limit from
aggregate cache retention. The existing eight-MiB per-run byte and 2048-physical-
pixel font limits, checked arithmetic, failure identity and callback atomicity
remain unchanged. Snapshots stay retained with pointer-based cache keys. Fonts
remain Host-supplied, with the same context-owned font invalidation. There is no
new cache owner, dependency, public configuration or editing buffer.

The Vulkan painter checks whether **every original glyph quad in one run** lies
inside the existing integer physical scissor. Only then may that run share the
viewport scissor. Otherwise the complete run keeps its original scissor. This
lets existing adjacent atlas batching merge fitting text across cell boundaries.
It preserves quad coordinates, UVs and atlas insertion order, including clipped
glyphs. Transparent atlas gutters, glyph colors, paint order, intervening
backgrounds, selection, focus rings, editor decorations and descriptor/resource
lifetimes remain in their old paths.

A preliminary quad/UV-cropping experiment reduced draw calls but changed native
pixels, including a visible text shift at 150% DPI. That experiment was rejected;
its raw captures and comparison remain local red evidence. The final containment
path matches the original RGBA bytes in all six recorded native scenes. Larger
working sets and overflowing runs can still retain more draw calls.

This is renderer-local batching. The public PaintList and every cell Element stay
intact, as do layout, hit testing, ActionRoute, AccessKit and UiSnapshot. Rich cell
widgets continue through ordinary projection. No shadow cell identity index or
second text engine is introduced. Public signatures and enum variants are unchanged.

## Validation and limits

The portable cache regression uses 792 actual supplied-font snapshots and more
than 8192 aggregate glyphs, proves retained snapshot/run identity on the second
pass, and checks byte/count bounds. Existing resource tests still reject an
oversized single run atomically and verify eviction releases snapshots.

The scissor test compares coverage at center and off-center sample positions,
across five DPI scales and clips excluding each of the four edges. The production
glyph-run regression exercises both promotion and overflow fallback with a real
supplied font. The CPU pipeline example checks the
current cell's hit, typed action, tab order, AccessKit node, Agent name and complete
bounded projection outside its timed stages.

The optional `--gpu` mode creates a hidden native window, renders the same table
through the real Vulkan renderer, captures RGBA bytes and records actual UI draw
calls, scissor updates and pipeline binds. CPU preparation entries are reported
separately and must not be called draw calls. Native runs use the same fixed
physical target at each scale, so higher DPI naturally clips more table content.

Reproduction and final measurements are recorded in [PERFORMANCE.md](../PERFORMANCE.md).
The normal `target/research` output is local evidence, not a release artifact.
No fixed FPS, physical-display latency, general screen-reader qualification or
downstream Agent/Builder adoption follows from these checks.

Final production-source qualification passed 264 workspace tests across 43
harnesses, including 47 Standalone tests; the subsequently added production
promotion/overflow regression also passes (265 distinct tests in combination).
Both doctests, full-workspace all-target/all-feature strict Clippy, renderer base
strict Clippy, seven Node contracts, resolved eight-library architecture,
affected-package formatting and whitespace checks pass. The earlier unrelated
Trace lint recorded in decision 0040 is historical; this later full strict run
passes against the concurrent checkout. Trace/Perfetto changes remain separately
owned. No source seal, publication, commit or downstream dependency update is
part of this implementation.
