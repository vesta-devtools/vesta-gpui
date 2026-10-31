# Widget ownership and projection contracts

This is the finite Widgets family review for [GF02](REFINEMENT.md), with its
GF03–GF05 layout, lifetime and resource findings. Public exports are defined in
[`lib.rs`](../crates/vesta-gpui-widgets/src/lib.rs); this page is a responsibility
map, not a second catalogue. Capability-specific signatures and historical
qualification remain in [the supported API](../crates/vesta-gpui-widgets/SUPPORTED-API.md).
Changes follow [decision 0023](decisions/0023-Widget-Projection-And-Controlled-State-Authority.md)
and the Core dependency [decision 0024](decisions/0024-Precise-Cumulative-Layout-Finalization.md).

## One owner per kind of state

| Responsibility | Implementation owner | Inputs and outputs; caller responsibility |
| --- | --- | --- |
| Composition, theme, forms and chrome | Widget/WidgetContent, controls, property/set, theme, toolbar/breadcrumb/title/tab-switcher/drawer projections | Stable identities, existing values, explicit availability/theme and typed actions become Elements/actions. Host owns descriptors, persistence and document commands. |
| Controlled editing | Text/password/search/inline/combo, numeric/vector/color/ramp/slider projections and value-edit/suggestion states | Core owns shaping, editing history and composition. Host owns editor instances, drafts, validation, provider I/O and accepted value commits. Widgets return UI intent or preview/cancel data. |
| Key and scroll intent | Key dispatch/capture/choice policy, scroll_state and ScrollInputPath; pure popup_placement | Existing current focus/modal path, geometry and normalized keys yield intent/remainder. Host owns event delivery, capture, scroll offsets and reconciliation. |
| Popup lifecycle | MenuController, measured menu/picker, PopoverState, ModalStack, temporary editor and command palette | Stable descriptor path or current source/opener produces dismissal/focus intent. Host owns descriptor refresh, current-route validation, command execution and provider work. |
| Collections | Selection/tree/tab state; list/variable-list/tile/table projections and focus/navigation helpers | Host owns stable data/order and selection policy. Widgets derive bounded visible slices or typed navigation; indices address the current visual order, never durable identity. |
| Read-only documents | Markdown, rich/diff/document text, labels | Source and Core snapshots produce Elements, anchors, selection, copy text and inert link intent. Host bounds source/page/copy work and authorizes clipboard or navigation I/O. |
| Feedback | Toast/focus, tooltip/controller, progress, caret, status partition | Host supplies clocks/phases, explicit events and data. No Widget-owned event loop, timer thread, network service, renderer or author transaction. |

The old mixed `state.rs` is replaced by four private responsibility modules:
[`scroll_state`](../crates/vesta-gpui-widgets/src/scroll_state.rs),
[`menu_controller`](../crates/vesta-gpui-widgets/src/menu_controller.rs),
[`tab_bar_state`](../crates/vesta-gpui-widgets/src/tab_bar_state.rs) and
[`popup_placement`](../crates/vesta-gpui-widgets/src/popup_placement.rs).
Root exports/signatures are unchanged. Menu path/highlight storage preserves
the useful slice-returning public API; it is private UI state. Modal entries have
no such slice constraint, so ID and optional native metadata now have one owner
and one admission function. No replacement facade or crate is introduced.

## Projection and validation

`Widget` carries an Element tree and action entries. Child composition merges
those entries; it does not execute them or prove identity uniqueness. Build the
current Core ActionRoute before dispatch. Disabled/hidden projections remove
their input availability; custom content still needs stable unique IDs and the
current route's focus/modal checks. The raw `action_for` lookup does not supply
that admission or ancestor routing.

Fallible collection/menu APIs keep their typed errors. Measured menus validate
complete Core bounds and anchor Rects, requiring positive bounds, before calling
identity/action adapters. Projection callbacks adapt data; they must not perform
business commands. Some later table cell/column/identity failures can occur
after row factories ran. A Result therefore does not promise rollback of caller
side effects. Existing context/font caches can also be warmed before failure.

Infallible low-level geometry helpers and public raw range/resize DTOs keep
their documented preconditions. Supply valid Core geometry, finite motion and
values, ordered resize bounds and correctly derived ranges. They are not an
alternate validation boundary. Existing helper-specific fallback/normalization
rules remain; there is no universal NaN sanitizer or hidden geometry repair.
Scroll plan admission now reuses Core Size/Rect validity, including advancing
positive edges. Core alone owns final layout quantization and invalid output.

## Interaction and terminal ownership

Use native `open_in` entries for source-window ownership. Legacy local `open` /
`close` modes remain useful without native identity and cannot enforce a source
they were never given. In particular, an unqualified ModalStack entry can be
dismissed by a supplied source; it must not stand in for a qualified native
modal. Qualified modal dismissal requires its own source, ID, top entry and
allowed reason. Failed duplicate/full admission preserves the entire old entry.

Menu step/key/edge navigation accepts the active descriptor; a stale ancestor or
retired child cannot replace its highlight. Reconciliation handles removed,
disabled or retargeted descriptors; it never invokes a replacement command.
Popover/menu/editor/palette dismissal produces one local return intent. Host
validates it against the rebuilt route, respects a later explicit focus, and
does not restore focus on blur/owner loss/window teardown. Outside press keeps
its original action. Cancel previews before ending text sessions.

Value-edit and horizontal-edit state are copyable UI data, not command leases
or application transaction capabilities. Completion consumes the active state
of that instance. Copies are independent; Host serializes accepted document
commands and undo. Suggestion requests additionally require the exact owner,
query and monotonically supplied request ID; a late or duplicate response cannot
replace the current request. Provider tasks, cancellation and work budgets stay
in Host. No second catalogue or provider runtime is stored in Widgets.

## Work, retention and idle limits

- Fixed lists admit the existing finite content extent and visible/overscan
  limits. Range decisions use f64 and round a nonrepresentable maximum offset
  inward; the final visible item is retained without allocating all source rows.
  Variable metrics retain one prefix vector for caller-provided heights and
  borrow it when projecting. Host owns source storage and retained-item policy.
- Table views share one private projection for text/custom/windowed cells.
  Their visible range, extent, widths and resize contracts remain explicit.
  Tile grids retain existing u16 track/row limits and no new density quota;
  a legal row/column product can still be large. Host must choose a practical
  visible count/cell complexity and task budget. No all-input memory ceiling
  or native large-grid performance claim follows from arithmetic bounds.
- Tree/selection/tab state scales with caller data. Host reconciles stable
  identities after refresh. UI ordering and expanded keys are not a duplicate
  author schema; application persistence remains in Host/Workspace model.
- Markdown keeps parsed source and one shaped snapshot per prepared block.
  Paragraph text is limited to 16 KiB; source totals, block count, paging and
  copy limits remain explicit Host policy. Process-global monotonic block slots
  provide stable scope identities and have finite exhaustion, not recycling.
  `release_layout` releases derived snapshots while retaining parsed history.
  Unique unchanged blocks retain IDs; ambiguous duplicate matches get new IDs.
- Markdown geometry, pointer capture, vertical navigation and accessible ranges
  use the current source window/document/revision/page and shaped snapshot.
  Missing/stale shape or width returns a typed failure. Its edge-scroll clock
  uses existing 16 ms steps, bounded elapsed time and speed, and stops at a bound
  until pointer movement. Host supplies time and applies scroll changes.
- Toast capacity, tooltip delay, caret phase and progress redraw are explicit.
  Host advances clocks and prunes focus metadata with the same expiry clock;
  no polling worker is created. Status partition recomputes current retained
  widths in f64 after each removal, preserving priority and source order even
  when two finite extreme widths overflow a narrower running total. Its scan
  complexity is unchanged; this is a correctness fix, not a speed claim.

These are owned-data/work boundaries, not whole-process limits for arbitrary
custom children, fonts, malicious source or caller callbacks.

## Evidence and adoption scope

Review covers current production modules and actual Standalone, Agent and
Builder entry points, including measured menus, controlled editor cleanup,
collections and paged Markdown release. Token-reference census includes test
and bench candidates; it is not a Rust symbol resolver or proof of new Root
compilation. Existing qualified scenarios retain their recorded version/scope.

Original reds cover the final computed list range, drawable last-row zero height,
negative-half adjacent-box hit gap, stale menu descriptor step, status width
overflow and invalid measured-menu geometry. The two layout failures share one
Core finalization cause. Extend existing composites rather than adding pixel
snapshots or getter tests. Full SDK and affected hidden/nonactive Main/Float
qualification must use current frozen runtime source and actual standard binary.

Neither source review nor command-origin native preparation signs new physical
input, reader, DPI or speed exits. The original `UI-DOC-TABLE-001` user report's
exact Host/input remains unbound; generic Markdown table checks do not close it.
Root's immutable common adoption and the remaining GF03–GF06 exits remain in
[the programme](REFINEMENT.md).
