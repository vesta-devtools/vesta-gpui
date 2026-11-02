# Native trace accessibility and evidence focus

Status: Accepted; implementation and acceptance in progress.
Date: 2026-11-02.

The Trace application already uses shared controls and text editing, but has no
platform accessibility adapter. Explicit semantics alone do not publish a tree
or handle assistive actions. Add the existing winit AccessKit adapter to this
native host, using one event-loop envelope for worker and accessibility messages.
The bounded worker keeps its analysis ownership and does not depend on the view.

Publish the current realized GPUI layout, logical focus and shared search-text
projection after a successful native render. Descriptive labels include visible
statistics and GPU evidence. Use the SDK's AccessibilityTree and FocusScope for
all target/action admission; never maintain a second action or text-selection
resolver. Busy/invalidated layouts reject actions, as do unknown and retired
nodes. Search replacement and selection use shared undoable text APIs and the
existing bounded/debounced filter path. Assistive actions do not bypass IME or
the 256-byte query limit. Clicks use the same application commands as native
keyboard/pointer activation.
The normal adapter requests projection only for an active assistive client;
finite inspection explicitly materializes it for its directed boundary checks.

Keep committed navigation acceptance as a separate finite scene. Drive the
normal input path through forward/reverse range commits, anchored zoom, pointer
and wheel pan, fit and resize cancellation. Record the actual presented analysis
range and source identity so an independent machine client can query the same
interval and compare original-file arithmetic. This scene adds no input or
analysis implementation, and does not rerun unrelated previously qualified
inspection states merely to reach the new checks.
Uncommitted gesture intent and completed statistics are distinct. The analysis
header and its private projection metadata must name the interval that produced
the visible numbers; a held preview must not relabel the last completed report.

Virtualized statistics and thread tracks have stable keyboard focus stops.
Page/arrow scrolling uses current projected row/viewport limits, preserves the
analysis and paging state, and cannot steal editing keys from search. Home/End
apply to the focused statistics view; the existing timeline Home/Fit shortcut
remains available outside that view.
An in-flight projection cannot retire a completed-data logical focus merely
because its rows are temporarily hidden; validate or retire it when real results
publish. This retention never grants actions to stale/busy platform targets.

Replacing the immutable source retires its platform-node namespace and pointer
preview before publishing new facts. A reused presentation slot must never make
an old source's accessibility node select an unrelated new record. Keep global
search intent separate from source-owned selection/cursors. A focused finite
scene uses two explicitly synthetic captures with overlapping presentation slots
to check both keyboard access and source/pointer retirement without OS input.

Window focus loss cancels either pointer button's preview and releases its
capture without committing new analysis. The GPU evidence scroll view is a
stable focus stop; Page Up/Down, Home/End and vertical arrows scroll the shared
ScrollAreaState without changing the selected frame or analysis generation.

Keep platform publication and machine evidence distinct. Agents continue using
the versioned source-addressed query protocol, not a screenshot or accessibility
tree as their analytical data source. No credential/model/author dependency is
introduced. This is private application wiring, not a ninth SDK library.

Extend the existing finite native inspection only for the affected publication
and input boundary: actual tree/text projection, Focus/Click/SetValue/selection,
foreign/retired target rejection, search ownership and normal retirement. Retain
previous scene identities. These command-origin checks do not claim Narrator
speech, physical input or cross-display DPI acceptance.

Scoped implementation/software/native evidence is recorded in
[TRACE-PLAN](../TRACE-PLAN.md). Full application delivery remains active.
