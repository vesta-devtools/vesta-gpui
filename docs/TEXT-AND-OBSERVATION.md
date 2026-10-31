# Text, semantic and observation ownership

This completes the Core source-family contract alongside
[identity/layout/input](../crates/vesta-gpui/SUPPORTED-API.md),
[CPU render preparation](RENDER-PREPARATION.md) and [tracing](TRACING.md).
Rustdoc and source define actual exports. These pages describe ownership and
failure boundaries rather than maintain a second symbol registry or dispatcher.
Decision [0022](decisions/0022-Core-Text-Resources-And-Semantic-Lifetimes.md)
records the text/resource and platform-identity preview migration. Qualification
status belongs to [the finite programme](REFINEMENT.md).

## Text state and geometry

| Owner | Responsibility and lifetime |
| --- | --- |
| Host | Font bytes/stacks, document/draft truth, field identity, current window, clipboard/files, native IME, resource references and authorization |
| `TextInputContext` | Host-supplied Parley fonts, shaping context, measurement revision and bounded static shape caches; no editing buffer, system-font discovery or I/O |
| `TextInputState` | One transient editor, selection, marks, composition and bounded Undo/Redo; source changes are ordinary edits or explicit owner reset |
| `TextInputSnapshot` | Shared immutable glyph layout and derived selection/caret/composition geometry; presentation policy can change without another editor |
| `TextLayoutCursor` | A transient visual cursor retaining that exact shared layout; byte offsets and cursor geometry are not author identity |
| `TextInputViews` / text-session transition | Current window-owned field geometry and field/IME-owner transfer decisions; no draft, platform IME object or second focus owner |

The editor admits at most 16 KiB of source; password limits clamp to 1..16 KiB.
Constructors normalize nonfinite font size to 13 and clamp finite size to 8..48.
Static plain shaping deliberately shares that editor font policy. Styled
read-only shaping accepts a finite positive font size with representable 1.5x
line height; invalid size or invalid UTF-8 span range returns
`StyledTextError::InvalidRange` before shaping. Its existing invalid/nonpositive
width fallback remains one logical pixel. This does not silently narrow finite
positive styled font sizes to the editor range.

Editable input normalizes line breaks/tabs as documented in the Core contract;
read-only reset preserves source bytes. The 32-step/256-KiB history budget counts
retained source and mark-ID bytes, not all Parley layout/font allocation. Marks
are bounded host references, not parsed text or a domain registry. Owners use
retained IDs and `forget_marks` when releasing external resources; reset/history
clear and Undo/Redo retain their distinct meanings. Composition retains the
original editor until cancel/commit; ordinary edits do not take it over.
Password snapshots are masked unless the Host explicitly requests a revealed
presentation. This is a presentation/privacy contract, not zeroized memory.

Font registration and changed role stacks invalidate all static shape caches and
measurement tokens. Failed registration and unchanged stacks preserve the token.
The three static caches each admit at most 512 entries and 128 KiB of accounted
source bytes; ellipsis includes its derived display source. Keys/order/layout
metadata and font resources are additional allocations. Context and caller-held
snapshots can retain fonts/layouts until their last owner drops them. A context
is Host-local; there is no process-global font cache or automatic font loader.

Layout measurement, paint, text hit mapping, IME caret and semantic enrichment
must use the same current shaped snapshot and viewport. Hosts supply finite,
coherent bounds/clip/scroll values and refresh after a source/font/layout change.
`viewport`, decorations and point/cursor helpers are geometry operations, not a
second validity or commit boundary. `TextInputViews::from_layout` rejects
duplicate element/field mappings and foreign-window lookup; it does not prove
that a caller-supplied geometry set is current. The Host performs frame admission
and rejects stale asynchronous results. A held old snapshot remains valid data,
but cannot authorize interaction with its replacement.

## CPU raster resources

One rasterizer instance owns Swash scaling state and three derived caches. One
private checked viewport converts logical bounds/clip/origin into bounds-relative
physical coordinates. Both bitmap and glyph-visitation APIs use it before cache
lookup/work. Bounds/clip must be valid finite rectangles with nonnegative extents,
origin finite, scale finite and positive, and scaled relative coordinates/extents
representable in signed pixel coordinates. Invalid inputs return
`TextRasterError::InvalidViewport`. The effective clip intersects the bounds;
valid empty clipping emits no glyphs. Scroll/caret DTO fields are not consumed by
glyph rasterization: the already placed origin/clip carry their effect.

Composed bitmaps keep their 1..8192-pixel dimension and 4,194,304-pixel area
bounds. Owned `render` clones the pixels; `render_shared` returns an `Arc` and
reuses identical output. Glyph visitation can serve larger atlas-backed surfaces
without allocating a full surface bitmap. These paths share one prepared run;
bitmap composition borrows its glyphs rather than cloning every glyph/key.

Before Swash scaling, each physical font em must be positive, finite and at most
2048 pixels. One prepared run admits at most 8192 glyphs and eight MiB of
accounted glyph/key/image bytes. Referenced images are counted conservatively
even when shared. Exceeding either limit returns
`TextRasterError::GlyphBudgetExceeded`; positions use checked integer arithmetic.
The new error variant requires an exhaustive-match migration. Both owned and
borrowed visitors invoke callbacks only after the complete run succeeds. Empty
clipping is a successful no-op, not an oversized visible run's truncation.
Failure can retain intermediate cache entries within existing cache bounds;
it does not publish partial glyphs or a composed bitmap.

| Cache | Retention limit | Identity / eviction |
| --- | --- | --- |
| Glyph images | 4096 entries / eight MiB image bytes | Font blob/index, size, variation, glyph and subpixel identity; FIFO, shared images |
| Positioned runs | 256 entries / 8192 glyphs / eight MiB accounted bytes | Exact snapshot plus bounds-relative placement/clip/scale; LRU |
| Composed bitmaps | 64 entries / sixteen MiB pixel bytes | Exact snapshot, placement, selection, focus/caret and quantized colors; LRU |

Run/bitmap entries retain the snapshot behind pointer keys, preventing address
reuse from aliasing a different layout. Eviction/drop releases that retention;
returned `Arc`s and owned glyph images may outlive the cache. The opaque GlyphKey
is derived raster identity, not application/author identity. RasterGlyph's public
placement/dimension fields describe its immutable backing image; preserve those
dimensions when passing it to an atlas or asking for RGBA. The backing image and
key are not publicly constructible. These are trusted prepared payloads, not a
binary image decoder or arbitrary mutable image buffer.

Cache limits count specified resources, not every key/allocator overhead, live
caller-held output or third-party scratch allocation. Font bytes are supplied by
the Host; these limits do not sandbox malicious fonts or provide a hard process
memory ceiling. No raster entry starts a thread, performs OS I/O or silently
uploads/readbacks GPU data. The renderer owns atlas storage and GPU retirement.

## Semantic lifetimes

`Semantic` describes Host/widget meaning rather than inferring it from painted
strings. One `AccessibilityTree` belongs to one native window. Build it from the
same complete Element/LayoutTree presentation; anonymous containers flatten into
semantic hierarchy. The build is not another layout solver or coherence check.
The platform adapter publishes the resulting AccessKit update.

Platform node zero is the window root. Other node IDs come from this tree's
process-local scope. Semantic mappings survive rebuild, clipping, disablement or
temporary absence of semantics **while the element remains in the complete
layout**. Once absent from that layout, the mapping retires. A virtualized row or
page returning later retains its Host ElementId and receives a fresh platform
node ID. IDs never recycle; an old node cannot address the new incarnation.
Retained mappings are therefore bounded by current layout membership, rather
than all rows/pages ever visited. This is an intentional preview change to the
former fully-removed-owner restoration policy; clipping stability is preserved.

Live publication, focusability and published action capabilities rebuild each
frame. Element/node lookup only resolves currently published semantics. Disabled
owners advertise no activation/edit action. Text-run maps retire when their
element is no longer published; read-only document projection storage retires
when its root is no longer live. Current-build enrichment is required for
selection admission. Thus a retained old projection does not authorize an action.
Dropping the tree releases its remaining maps and layouts.

The monotonic local allocator admits at most `u32::MAX` semantic/text-run node
allocations during a tree lifetime; the scope allocator is also finite. Existing
infallible tree construction/build/enrichment panic on namespace exhaustion.
There is no wrap/reuse or hidden recovery policy. These are explicit hard
capacity limits, distinct from ordinary typed edit/projection errors and cache
budgets; no test claims to naturally exhaust billions of allocations.

Call editor enrichment after build and before publishing, with the matching
current input/context/origin. Per-window projection resolves stale/foreign runs
and then the Host applies the accepted byte range after focus/session transfer.
The direct editor projection remains intentional single-window compatibility;
its changed-only bool must not decide whether an accepted unchanged selection
acquires focus. Multi-window Hosts use the owning tree.

Read-only document enrichment borrows source and the actual shaped paragraphs,
with document root/revision and optional selection. It checks live descendant
membership, duplicate paragraphs, lengths, UTF-8 run/cluster ranges, separators,
finite origins and selection boundaries before publishing staged nodes. The
Host must supply the exact source for each snapshot: range checks cannot prove
byte equality against an independently shaped, same-length string. Core keeps
no second canonical document source. Reflow/revision replacement retires old run
positions; continuously published unchanged layouts keep their run identities.
Document/paragraph counts and source-size composition remain Host-bounded; there
is no implicit whole-document virtualization or platform TextRange guarantee.

## Inspection and trace

`UiSnapshot` is a bounded immutable observation of one committed tree/layout/
route bundle, identified by Host instance/window/revision. Capture checks
supplied identities and parent/layout membership for visited nodes, budgets all
text channels, and redacts password/sensitive descendants. It cannot establish
business authorization or freshness of a bundle the Host labels incorrectly.
Query pages borrow that snapshot; diffs suppress removal claims when structural
capture is incomplete. Callbacks provide current editor values and sensitivity,
not a second editor or external I/O. See [structured access](AGENT-ACCESS.md).

`UiOwnerRegistry` tokens, `UiCommand` structural preflight and Host business
admission are separate checks. Removed/replaced owners retire tokens; capacity
failure preserves the registry's previous state. Neither a platform node nor an
inspection/action reference is a permission or persisted document identity.

Tracing retains one explicitly started process-local capture, bounded by event
count and elapsed duration. Synchronous guards are thread-bound; stop freezes
unfinished work, drop discards abandoned storage, and late guards retain only
the stopped shell. The existing analyzer supplies frame calls/timers and the
offline report from the same data. Hosts own capture controls and file export.
Disabled hooks avoid clock/lock work. Recorded wall time, incomplete/lost events,
unframed work and uninstrumented functions retain the limits in
[the tracing contract](TRACING.md). Source review or CPU resource tests do not
prove GPU execution time, native latency, physical input, reader delivery or
immutable consuming-product adoption.
