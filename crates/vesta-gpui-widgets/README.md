# Vesta GPUI Widgets

Reusable widgets built from `vesta-gpui` Elements and typed Actions. Callers own
business data, stable identities, editor state and document commands. Widgets do
not require an engine, window toolkit or GPU renderer.

Editor-specific node cards/pins, parameter value/slider presentations and path/menu
chrome belong to [vesta-world-builder-ui (historical context)](../../docs/VALIDATION.md#historical-evidence).
Studio and Builder consume that business library; these APIs are outside the
generic GPUI widget contract.
`property_row_with_presentation` / `property_set_with_presentation` optionally use
the compact Studio parameter layout; the standard constructors stay unchanged.
Their values are supplied by the host, which owns exact Catalog values, soft
ranges, drag/cancel state, validation and typed commits. These optional property
presentations remain outside the frozen Basic qualification.

`button_with_presentation` adds Normal, Primary and Danger tones to the ordinary
button. Loading decorates the existing caption with a visible busy mark, preserves
the supplied identities and accessible name, publishes busy/disabled semantics
and removes activation and Tab participation. The caller owns work,
completion and cancellation; the widget creates no worker or clock.

`radio_group` projects caller-owned stable values, RadioIds and typed actions.
Exactly one enabled choice participates in Tab: the selected choice, or the first
enabled choice when selection is absent/disabled. This fallback does not change
the caller's selection. Duplicate values return `RadioGroupError`. Supply current
enabled values in visual order to `radio_group_key_action`; arrows wrap and select
through Highlight, Enter/Space confirm, and Tab/Escape remain host keys. Apply the
returned action and restore focus using the caller's stable selected identity.

`progress_bar_state` accepts finite determinate values or an indeterminate host
phase. Determinate state exposes percentage range/value; indeterminate state
publishes busy/Working without a fabricated numeric percentage. The host owns
visible animation scheduling. Non-finite input returns `ProgressStateError`.
The original button, radio and progress constructors remain available.

Basic controls' portable state and source contracts are checked in
`tests/composite_interactions.rs`. Native layout, DPI and reader qualification
is recorded in the [three-capability ledger (historical context)](../../docs/VALIDATION.md#historical-evidence).
The existing Basic subset is frozen on2026-10-15. Its audited
[basic source contract](SUPPORTED-API.md) records host obligations, structured
errors and pre-freeze migration; other exported widgets retain their own gates.

`WidgetContent<A>` carries an explicit semantic label and a complete `Widget<A>`
subtree. List, Tree, Combo options, Table headers/cells and TileGrid preserve child
actions and editable TextInput boundaries. The text convenience APIs use the same
shells. Virtualized factories construct visible/overscan items and supported
retained focus items from caller data.

The [composite collections example](examples/composite_collections.rs) uses its own
typed `RecordId`, data and actions. One builder supplies an image token, name,
editable note and Open button to five public collection projections. It rebuilds
after reversing/renaming/editing records, range/additive selection and deleting a
selected record, using
retained layout, hit testing, action routing, focus reconciliation and semantics.
Two independently scoped source windows use LTR and RTL and reject foreign focus.
Tile navigation reads the current Grid `LayoutNode.flow_direction` through
`tile_grid_next_index_in_direction`: physical Left/Right follow visual direction,
Up/Down follow rows and Home/End follow declared item order. The original
`tile_grid_next_index` remains the LTR convenience API.

The same executable also runs 40 lazy projections over 10,000 typed records in
both source directions, using public windowed List, Tree, Table and Tile factories.
Each frame constructs at most nine composite items in this fixture. List, Tree
and Table retain one stable offscreen focused child; collapse and deletion clear
invalid focus and semantics. TileGrid builds only its visible range and is placed
inside a caller-owned ScrollBox using the same scroll offset. It currently does
not retain offscreen tiles. Every visible child is checked against real layout
hits, and both source windows reject the other's focus. Reordering preserves IDs.
Empty `TreeExpansionState<K>` requires no default value for the caller's ID type.

Before changing data, each original typed collection source also rebuilds through
Dark → Light → Dark. Ordinary paint changes and returns; stable geometry, child
actions, selection, live focus, and image commands stay intact. Deliberate focus
uses the current theme token. The host passes the same theme into its arbitrary
row and table-header content: collection shells do not guess or replace child
colors. This is a portable composition check, separate from native visual, DPI,
font and screen-reader acceptance.

Run with a font supplied by your host; the example does not discover system fonts:

```powershell
cargo run --locked -p vesta-gpui-widgets --example composite_collections -- C:\Windows\Fonts\segoeui.ttf
```

On other systems, pass a readable TTF/OTF file. The program opens no OS window and
prints projection results; a renderer must resolve `example://thumbnail`. These
results demonstrate portable composition and state contracts. Native interaction,
IME, DPI and screen-reader acceptance remain in the
[Standalone acceptance ledger (historical context)](../../docs/VALIDATION.md#historical-evidence).

The [interaction sessions example](examples/interaction_sessions.rs) is a separate
consumer of the public focus, text-view, popup, menu, key-routing, scroll and edit
APIs. Caller-owned typed fields appear in two independently scoped projections.
Actual layout hits activate a child button without selecting its row; an editor
blocks the parent click. A five-choice popup consumes navigation before shortcuts,
and composition/foreign owners cannot dispatch non-text actions. Closing a popup
preserves the original outside action; disabled menu highlights cannot activate.
Nested wheel movement uses both axes and returns excess to the caller. Deferred
focus reveals a scrolled-out editor through both viewports, then checks its actual
hit. Typed edit cancellation/commit and deletion preserve source ownership and
release stale focus/capture/geometry. The caller applies model changes and records
the single commit; this is not document persistence or a replacement undo system.

The same consumer projects a long mixed-language document in bounded logical-row
pages. `MarkdownView::selection_focus_row(window, selection)` validates the
selection before the caller chooses a page for an off-page navigation result.
Its index belongs to the current revision; stable anchors remain authoritative.
Whole table rows, empty cells, copy order and selection survive page changes and
narrow reflow. The caller retains page-size, loading and scroll/reveal policy.

The document sequence also consumes retained edge-scroll deadlines with a host
font, checks stationary and reverse selection at document boundaries, and keeps
following content out of the scroll. Current public accessibility projection
checks the stable anchor; foreign-window ticks reject, and release/Escape retire
capture and deadlines. Run only these document checks with the same executable:

```powershell
cargo run --locked -p vesta-gpui-widgets --example interaction_sessions -- C:\Windows\Fonts\segoeui.ttf --documents-only
```

These are caller-driven pointer/time calls, separate from actual OS-held input.

```powershell
cargo run --locked -p vesta-gpui-widgets --example interaction_sessions -- C:\Windows\Fonts\segoeui.ttf
```

This executable opens no OS window. Its supplied composing flag exercises routing;
it does not generate an actual platform IME session, held pointer gesture, DPI
transition or screen-reader speech.

The [layout performance probe](examples/layout_performance.rs) rebuilds 128
composite rows (513 Elements) and separately times declaration building, action
routing, layout, PaintList, hit testing and accessibility projection. It alternates
stateless, retained and public TooltipController paths in four blocks, including
steady frames, paint/scroll changes and text/viewport changes. The probe checks
layout, paint and sample hit equivalence before reporting results. It supplies no
GPU, native window, IME or screen reader and makes no FPS assertion.

```powershell
cargo run --locked -p vesta-gpui-widgets --example layout_performance -- C:\Windows\Fonts\segoeui.ttf
```

Keep the profile, executable identity and all printed blocks when comparing runs;
concurrent machine load can change timings. Stage intervals exclude native host
work and end-of-iteration teardown of the held projection outputs; semantics
includes constructing and dropping its returned update. The default dev profile uses
optimization level 1, with dependencies at level 3.

`ellipsized_label` takes an explicit outer logical width (including the shared
text viewport insets) and a host font context. Its single-line glyph snapshot
uses a measured Unicode grapheme prefix plus U+2026; full source, localization
key and stable widget identity remain available to accessibility. A hard-line
suffix also requires the marker; if the marker cannot fit, it paints empty.
This static projection has no editor focus/action or copy controller. Rebuild it
with the actual new width after layout/resize; the shared bounded font-aware
cache retains exact-width results. The public example covers mixed scripts,
combining/family graphemes, both themes, zero/narrow widths and full-source
semantics without an OS window:

```powershell
cargo run --locked -p vesta-gpui-widgets --example text_labels -- C:\Windows\Fonts\segoeui.ttf C:\Windows\Fonts\msyh.ttc C:\Windows\Fonts\seguiemj.ttf
```

`selectable_label` takes matching `TextContent` and a caller-owned
`TextInputState::new_read_only(font_size, font_stack, wrap_width)`. Owner `reset`
retains the exact source bytes; pointer/keyboard/assistive selection uses the
same Parley layout and text-run identities as editors. Its semantics is read-only
and omits SetValue. Route selection through the existing window text session and
disable the OS IME request for this field. The winit bridge also honors the state
mode even when its transient host read-only argument is false. Inject the host
clipboard as usual: Copy (and existing read-only Cut) copies the original selected
range; Paste performs no read and edits / IME / assistive replacement are refused.
The `text_labels` consumer additionally exercises 32 selectable projections.
This view reveals full-source selection through the existing text viewport;
static `ellipsized_label` remains an explicit nonselectable display projection.

The controlled tree module composes existing preorder, expansion and selection
types through `TreeViewController`; focus is a returned intent for each view.
It supports visible-order ranges, focus-only Control navigation, safe collapse,
ancestor reveal and stable source refresh. Keep author data and window focus in
the host. The original `interaction_sessions` example exercises 10,105 entries,
bounded retained rows, hit/action/semantic identity and invalid-source refusal.
See [the complete module ledger (historical context)](../../docs/VALIDATION.md#historical-evidence).

Historical context links explain the original evidence scope; see the independent validation and release procedure.
