# Streaming lists, wide tables and source delivery

Status: Accepted; implementation in progress.
Date: 2026-11-09.

## Objective and ownership

Continue the completed 0040/0042 list/table checkpoint with a finite collection
programme. Keep the eight-library architecture and existing Element, layout,
typed action, focus, AccessKit and Agent-query projection. Host data, stable
record identities, provider work and product persistence remain Host-owned.
Implement independently in this Apache-2.0 repository without BUI source.

The current SDK is developed independently from Agent and Builder. Library,
public-example and Standalone checks do not require rebuilding either product.
Product dependency adoption and its integration checks happen at the selected
immutable revision, with the consuming application's owner.

## Stage exits

| Stage | Required result | State |
| --- | --- | --- |
| CL00 Candidate preparation | Reconcile current API migrations and scoped measurements; identify concurrent source ownership; produce the final source inventory and exact checks when the delivery batch is stable | Migration notes now cover 0040/0042 and the new list APIs. Final inventory/package remains pending while the chosen source batch changes. Trace is concurrently changing its native capture workflow under decision 0043. |
| CL01 Structural list updates | Validated append/prepend/replace/remove geometry; stable Host-remapped scroll anchor; failed updates atomic; incremental append measured against full reconstruction | Local software/public-consumer checkpoint passes. General prepend/remove remains O(n), not a sparse index. |
| CL02 Measured message viewport | Budgeted visible-row measurement and explicit width/font/content invalidation; streaming tail following; user scrolling pauses following; prepend preserves the viewed item; empty/remove/resize and retained child focus handled | `LazyListMetrics`, the public consumer and Standalone integration are implemented. Initial Main/Float physical and semantic-command checkpoints are recorded below; expanded native reflow/input/accessibility qualification remains. |
| CL03 Wide tables | Construct visible columns plus frozen and explicitly retained focused cells; declared column identity/width/order; aligned headers and rows; coherent horizontal scrolling, clipping, RTL, action/focus/semantic/Agent behavior | Core attachment, bounded column projection, the public consumer and the native 10,000 × 512 table are implemented. Initial physical horizontal scroll/range-copy and semantic capture/focus checkpoints pass within the scope below; expanded native qualification remains. |
| CL04 Consumer and delivery closure | Real public consumers and Standalone integration; equivalent-work performance evidence; affected and full software checks; scoped native visual/input evidence; current source inventory/package and reviewable release notes | Pending. Public commit identity, push/release and downstream adoption follow maintenance requirements. |

Range-selection/TSV convenience follows the same Host-owned selection and
clipboard contracts when the wide-table consumer demonstrates the needed
behavior. This programme adds no provider, document model or clipboard service.

## First implementation: measured geometry splice

Add `VariableListMetrics::splice(range, heights)` to the existing geometry
owner. It describes changes in the Host's current visual order, not records or
stable IDs. Admit the complete range, heights, new count and total before any
mutation. Invalid requests preserve every height and Fenwick entry. Unchanged
height slices return false. Existing failure variants remain authoritative.

Appending and equal-length height replacement update the block index without
reconstructing unrelated rows. General insertion/removal, including prepending,
rebuilds the current flat height index in O(n). State that limit explicitly;
this is not a sparse ten-million-message index or automatic text measurement.
Ordinary Vec capacity growth can still copy height storage during append.
The Host captures its first visible stable record before changing data, maps
that record to its new index, then restores the existing `ListScrollAnchor`.

## Validation

Use seeded differential geometry tests across 64-row Fenwick boundaries, mixed
splices and point updates, fractional edges and extent/count failures. Exercise
append growth across tree powers of two and final-item removal. Validate actual
list projection, retained child focus, hits, typed actions, AccessKit and Agent
inspection in public consumers. Performance receipts identify source/profile,
the included work and full-rebuild equivalence; no universal FPS claim follows.

The previous 265-test/six-scene checkpoint remains dated evidence for 0040/0042.
New changed bytes require their own checks. Source sealing is the final stable
batch step, not a way to inherit old qualification for ongoing changes.

## Local list checkpoint — 2026-11-09

Widgets pass all 44 tests and their example targets, with strict all-target
Clippy. Standalone's 47 existing tests pass against the changed list index.
The message consumer explicitly loads Segoe UI and YaHei from Host paths; its
eleven states cover tail append/stream, reader scroll-up, prepend, append while
reading, narrow/wide reflow, jump to latest, empty and restart. Each projection
builds 0–11 rows from 0–1010 messages, with at most eight measurement attempts per
settlement pass and one/two passes in this run. It retains a focused offscreen
child and checks actual hit/routing, AccessKit focus and Agent snapshot identity.

The equal-input geometry probe warms both paths and alternates four blocks of
256 appends. At 100,000 initial rows, the recorded rebuild path takes
224.588–248.488 microseconds per step; incremental splice takes 0.508–1.523.
Each includes the same source-height append and anchor/range work. Final metrics
equal a fresh full reconstruction and signatures match. This is geometry-only
dev-profile work on a shared Ryzen 9 5950X machine, excluding text measurement,
identity lookup, projection/layout, rendering and physical display latency. The
slowest incremental block remains in the raw evidence.
A final-source repetition records 211.219–229.789 / 0.445–0.721 microseconds for
rebuild/incremental respectively; both complete recordings remain, illustrating
shared-machine variation rather than replacing the earlier slow observation.

Local source/executable receipts and original logs live under
`target/research/streaming-lists-20261109/`. These checks add no native message
interaction, product dependency change or current sealed release. The independent
Agent currently retains its own realization/reading/tail owners; later adoption
must reuse that policy rather than add another scroll controller.

## Wide-table implementation boundary

`TableColumnMetrics` owns only immutable widths/prefix edges in the Host's current
visual order. Construction is O(n); borrowed windows use binary edge lookup and
bounded projection. Frozen logical-start/end counts select contiguous prefixes/
suffixes, not a second identity/order model. The Host supplies every shell,
band, header, cell and arbitrary child identity. Only the retained focused row
builds its extra offscreen focused cell; header retention does not realize that
column across all rows. Existing ordinary/row-window constructors keep signatures.
The new row DTO takes an explicit Host label rather than joining whichever cells
are realized. This keeps a record's accessible name stable and bounded across
column windows; rich cell labels remain explicit as before.

An initial layout-offset compensation experiment exposed a rounding mismatch:
the solver rounds authored offsets, whereas ScrollBox translates by its exact
offset after layout. Compensating a fractional offset can move frozen geometry.
Reject that approach. Add Core's declarative direct-child `ScrollAttachment`
rule instead. Natural layout and content extents are unchanged; the existing
geometry collector omits the immediate parent's selected translation axes once.
The full-width table content attaches on X; Widgets project its scrolling band
from that same Host offset. The original ScrollBox retains full range, scrollbar,
input and Agent scroll authority. Parent clipping, RTL, paint, hits, AccessKit
and inspection still consume the one LayoutTree. There is no new solver, renderer
clip API, event owner or persistent sticky-element state.

The public `wide_table` consumer keeps typed stable record/column IDs while
scrolling, revealing a focused child, reversing records, moving a column,
resizing, changing direction/theme, clearing and restarting. It also copies a
small rectangle from Host values across unrealized cells, demonstrating that
this operation needs no Widget-owned data/clipboard service. This fixture's
values contain no TSV delimiters; general quoting/export policy is not added.
The Core field's default is unchanged `Content`; complete external Style
literals need the additive field or the normal default update syntax.

## Local wide-table checkpoint — 2026-11-09

Core's 75 tests and Widgets' 47 tests pass, including the current three column
tests and arbitrary-content example compilation. Checks cover full-scan edge
equivalence, admission limits, retained header/cell/row identity, horizontal-only
focus retention, aligned resizable headers, frozen-band clips/hits, LTR/RTL,
fractional offsets and logical scale inputs. The Core attachment test toggles
all four modes through retained/stateless layout, paint, hits and AccessKit;
natural scroll extent stays unchanged. These logical scale inputs are not
native monitor/DPI or screen-reader acceptance.

The 10,000 × 512 real-font public consumer passes eleven states, including a
physical-to-logical scroll/reveal check after switching to RTL. It realizes
0–100 body cells, rather than all 5.12 million source positions, and preserves
complete bounded Agent snapshots and explicit stable row labels. The CPU
comparison uses 256 columns, four visible rows and matching zero frozen counts:
1,280 constructed cells / 3,081 Elements become 51 / 149, with matching visible
text paint, geometry, actions and retained focus. Sixteen alternating samples
after four warmups record medians 14.064 / 0.764 ms. Included/excluded work and
the earlier slower sample set remain in [Performance](../PERFORMANCE.md). The
final executable-hash-bound repetition records 10.245 / 0.588 ms, retaining
all prior samples rather than replacing the slower observations.

The workspace all-target/all-feature test pass and two doc tests are followed
by affected-source tests after the final row-label/retention adjustments. Strict
workspace all-target/all-feature Clippy and renderer base-feature Clippy pass;
seven source-tool tests and the resolved eight-library architecture pass. The
original moving-worktree full test run is not a current sealed SDK/package
receipt. Scoped logs, a working-source inventory and executable/font hashes
live under `target/research/wide-tables-20261109/`.

Expanded CL02/CL03 native scroll/focus/input/visual qualification and the final
stable delivery inventory/package remain open; the integration checkpoint follows. Keep the
goal active. The separate native Trace changes continue under their own ledger;
no consumer product dependency was changed and no public revision was created.

## Native Host integration checkpoint — 2026-11-10 (Asia/Shanghai)

Standalone now has a Messages & Data workspace tab. Its shared demonstration
source belongs to DemoState; each Main/Float Host owns a View with its own lazy
message measurements, anchor/tail policy, table scrolling and cell selection.
Actual pane widths feed the existing bounded layout-pass loop. The Host diffs
stable message IDs to apply append/prepend splices, invalidates changed text,
and remaps the reader anchor before budgeted measurement. The existing
ScrollGeometry/ScrollStates, wheel/thumb propagation, typed actions, key
precedence, deferred focus requests, semantics and Agent access are extended.
There is no parallel native event path or extra business table in a library.

The native table adds source-owned rectangle selection with Shift+click/arrows,
Home/End/Page navigation, frozen-aware focus reveal and bounded source-value
copy through HostClipboard. Selected endpoints use stable record/field IDs;
their rectangle follows current source order. The demo uses zero frozen ends
when a narrow pane cannot fit both requested ends plus a scrolling region,
retaining the user's preference for later expansion. General cell export,
provider work and arbitrary quoting are outside this demonstration.

All 50 Standalone tests pass, including three new integration tests for shared
source/two-width measurements, append/stream/history/reflow/empty and native
table keys/retention/source copying. Package-local strict Clippy (including
tests, without dependency lints) passes. The separate Trace owner is currently
adding memory/stack capture to Core/winit, so earlier full-family passes do not
qualify that changing source batch.

The actual Vulkan Main/Float fixture displays different row wrapping and column
windows. Physical Computer Use observations cover shared append, Main reading
history while Float follows, prepend without a visible anchor change,
horizontal scrolling with stationary frozen ends, Shift+Right range expansion
and Control+C reporting successful clipboard delivery. Original UIA trees are
under `target/research/native-collections-20261109/`; screenshots were inspected
inline. This initial physical run did not record an executable hash before
launch and is therefore separate from the executable-bound evidence below.
The Computer Use helper's `focused_element` continued reporting an older Note
editor even while the grid's focus ring and key navigation moved; do not use
that helper field as proof of native reader-focus correctness.

`tools/verify/streaming_collections.mjs` exercises the public native JSONL API
in a separately owned fixture process. The recorded passing run discovers both
presented windows, pages complete bounded snapshots, checks shared append,
different widths and retained Core focus after source reorder, captures three
next-presented GPU PNGs (Main, Float and RTL), closes through the discovered
command and requires exit zero plus an unchanged executable SHA-256. Evidence
is under `target/research/native-collections-agent/2026-11-09T16-12-36.271Z/`.
The earlier role-ambiguous probe failed and its receipt is retained. Semantic
commands/captures do not substitute for physical gestures or a screen reader.

## Expanded native checkpoint — 2026-11-10 (Asia/Shanghai)

The physical fixture now records its executable hash before launch and verifies
it again after normal exit: `337d13f5be6f29801ef4756c35f61a376ad37f881eb2760119ab1ce00b8bb580`.
Actual Main wheel input enters history; inserting three earlier messages and
appending a message preserves Message 1997 with its 88-pixel inset, while Float
follows the shared new tail. Physical Main cell click, Shift+Right and Control+C
move Core focus, extend selection and produce the actual `Selection copied`
text. Logical Float RTL navigation extends its own range without changing Main.
Native Main sizes include 920 and 400 logical pixels; Float reaches 320. The
original protocol, native input/projection logs, eight GPU captures and scoped
receipt are under `target/research/native-collections-physical-20261110/`.

This run found clipped narrow-pane hints and a freeze caption that described the
stored preference rather than its effective narrow fallback. Hints now wrap;
the fallback says `Freeze paused` with an explanatory tooltip. Reading/following
and copy notices use the existing polite Status semantic role. Existing line
endings outside edited README lines are preserved.

After those fixes, all 50 Standalone unit tests, its dependency-boundary test
and all-target workload smoke pass. The expanded public native API verifier
passes on executable `e3fff00b3655c64eb1102b4ecd46541aeb6002df08cbfb7b558d9d50ae009e6c`:
reader anchors survive prepend/append and 920-to-400 native reflow, the narrow
freeze fallback is exposed, Float RTL keys select fields 2–3, and Main focus is
unchanged. Its eight captures and normal-exit/unchanged-executable receipt live
under `target/research/native-collections-agent/2026-11-09T16-53-18.751Z/`.
The updated 400-pixel and RTL captures were visually inspected. Scale 1/1.5/2
captures are explicit offscreen GPU rendering checks; the actual Windows monitor
scale stays 1.25. These API gestures remain separate from physical input.

On a real physical Main click, Core's current button owner and the expected/
submitted AccessKit node, role and bounds agree in `NativeProjection`. The
Computer Use helper nevertheless reports an older Note editor as focused.
This observation remains unresolved; neither the helper field nor the submitted
projection establishes successful native screen-reader focus. Keep that check
open. No product dependency was migrated, release tag created or registry
publication performed. Current-source software/inventory closure is being
prepared for the separately authorized commit and push.
