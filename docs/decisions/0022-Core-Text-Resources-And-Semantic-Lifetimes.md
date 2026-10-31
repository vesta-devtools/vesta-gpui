# Core text resources and semantic lifetimes

Status: Accepted; local complete Core source-family qualification passed at recorded inputs.
Date: 2026-10-31.

## Scope

Continue the existing GF02 Core family and GF04/GF05 resource review. The
window-owned accessibility projection and CPU text rasterizer already serve
Standalone, Agent, Builder and the Vulkan painter. No new service or runtime is
needed. Preserve actual failures before changing their qualification status.

## Decision

Retain semantic platform IDs for elements still present in the supplied complete
Element tree, including clipped or temporarily nonsemantic elements. Retire
mappings for elements absent from that tree; do not accumulate every page or
virtualized row ever visited during a window's lifetime. Returning unrealized
owners retain their caller-owned ElementId but receive a fresh platform node ID.
Never recycle platform IDs; stale IDs cannot address a new incarnation. Existing
live, focus, action and text-run admission remains distinct. Keep the existing
monotonic allocator and its explicit exhaustion limit; no reuse pool, historical
tombstone map or second identity authority. Review actual collection callers and
verify clipping stability, bounded churn and stale-node rejection.

Validate the geometry consumed by both raster entry paths before cache lookup
or glyph work. One private derived viewport owns the bounds-relative physical
origin/clip conversion. Empty clipping performs no glyph work; invalid or
unrepresentable geometry returns the existing InvalidViewport. Keep composed
bitmap's existing dimension/pixel bounds; glyph visitation must remain usable
for larger atlas-backed surfaces without allocating a full surface bitmap.

Admit finite positive styled font size and representable line height before
Parley shaping; retain the existing width fallback and byte/range contracts.
Before Swash scaling, reject physical font ems above 2048 pixels (the square
root of the existing four-megapixel bitmap budget). Bound one returned glyph
run by the existing 8192-glyph/eight-MiB run-cache limits, counting referenced
glyph bytes conservatively. Expose GlyphBudgetExceeded distinctly from invalid
viewport geometry. Check positioned integer arithmetic rather than saturating
or panicking. Callbacks begin only after the complete run succeeds. Rejected
work may retain already produced cache entries within their existing budgets.
Host-supplied font parsing and third-party scratch allocations are not a sandbox
or a hard whole-process memory ceiling; state those limits explicitly.

Keep the original bounded cache ownership and snapshot retention that prevents
pointer-key reuse. Do not create a global font/cache registry, OS font discovery,
input dispatcher, new dependencies or new editing buffer. Publish the complete
text/semantic resource contract and reconcile inspection/trace boundaries with
actual exports and consumers. Only qualify the full family once its remaining
source/caller review and relevant checks are complete; preparation alone does
not close Core, the programme or immutable Vesta adoption.

## Acceptance

Retain red evidence for historical-ID growth and invalid raster/styled inputs.
Exercise clipping/return lifetimes, both owned/borrowed callback entry points,
cache hits, failure recovery, physical font/run limits and actual supplied-font
glyphs. Use existing text/document/inspection/trace suites for their unchanged
contracts. Run relevant packages, strict checks and the whole SDK before
commit. Reuse the ordinary hidden/nonactive Main/Float integration fixture only
when runtime inputs change, record exact artifacts/processes and clean up.
No new physical input, reader, DPI or general performance claim follows.

## Recorded compatibility change

The first focused run exposed the old composite editor test's explicit promise
that a fully removed semantic owner regains its original platform ID. That is
incompatible with removing historical mappings, rather than an unrelated test
failure to hide. Migrate that assertion to a fresh platform ID and rejection of
the old node; retain all original stale/foreign run, selection and multi-window
checks. Clipping or disabled state while still in the complete layout retains
identity. Record this preview change in the supported contract and release
migration; products must resolve current nodes through their owning tree rather
than persist AccessKit IDs as author identity. Core ElementId semantics do not
change. The original focused failure remains evidence.

## Reproduction and qualification

Red02 preserved 61 passed / three failed: the second eight-row page retained
sixteen mappings; seventeen invalid raster input/path combinations were accepted;
all five invalid styled font sizes were accepted. Red01's fixture type/import
compile failure, focused01's explicit old restoration-contract failure and
focused02's two fixture unwrap strict failures remain original evidence.

The final composite resource test covers sixteen page incarnations, clipping
stability, old-node rejection, real supplied-font callbacks, both visitor APIs,
physical-font/run rejection without a callback prefix, ordinary recovery and
snapshot release after run/bitmap eviction. Existing editor/document/inspection/
trace assertions remain, including every old stale/foreign run check. The
complete 41-module/eight-group source and caller review adds no dependency,
runtime or alternate authority.

Final full01: 232 passed / zero failed / zero ignored, 38 harnesses and CPU smoke;
two existing compile-fail docs; all/base strict, six tool groups, format and both
standard builds passed on frozen inputs. Actual ordinary Standalone 0d17deb5… /
PID 20340 passed Main64 and Float64 actual submissions, no injection, hidden and
nonactive, normal exit0 with no live descendants/observation errors. Final source,
artifact and cleanup identities are bound under
`target/research/core-text-resources-20261031`. This locally closes the Core
source family; it does not qualify physical input/reader/DPI, consuming products,
other family reviews or the whole refinement programme.
