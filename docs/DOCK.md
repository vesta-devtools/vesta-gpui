# Workspace projection and Dock input

`vesta-gpui-workspace` borrows the validated [workspace model](WORKSPACE.md).
It derives Elements, typed intent and geometry. The application owns the model,
native windows, event loop, persistence and capture. No library object runs a
window, commits a Dock action or starts a second workspace runtime.

This is an experimental Windows-preview boundary. Current source qualification
is recorded in [the programme](REFINEMENT.md); earlier physical observations and
waivers retain their original identities and limits.

## Projection and identity

| Entry point | Derived content |
| --- | --- |
| `workspace_view` | Complete fallback workspace, with default interaction/header and placeholder panel content |
| `workspace_view_with_content` | The same projection, with Host-supplied panel Elements |
| `workspace_view_with_interaction` | Also supplies hovered/focused IDs and close-button visibility policy |
| `workspace_view_with_header` | Also supplies header state and optional main-pane zoom; includes logical floating windows in the same tree |
| `workspace_main_view_with_header` | Main container only, for a Host presenting floats in native windows |
| `workspace_container_view_with_header` | Exactly one container; `Ok(None)` if a requested floating container no longer exists |

These convenience functions share one private projection implementation. Zoom
changes the projection, never topology or saved state. A zoom ID outside the
selected container is ignored. Panel/header closures borrow each projected pane;
the Host supplies content for its active item. An absent panel Element uses the
existing placeholder. Those callbacks must remain projection work: they can run
again during layout settlement and must not commit author or business changes.

`WorkspaceView` returns its Element tree, `(ElementId, WorkspaceViewAction)`
bindings and `WorkspaceViewRegions`. `action_at` only resolves an existing
binding. Tab/close/navigation/header/resize/floating actions express intent;
the Host validates and applies the appropriate model operation or product action.
`PaneNavigationDirection` does not introduce library-owned pane history.

Use a stable allocated `ElementScope`, distinct from the Host's content/overlay
scopes. Stable model IDs map to fixed local ranges: each encoded value must fit
24 bits, and rendering header commands additionally limits their pane ID to 20 bits.
The mapping preserves existing valid identities; it does not truncate or recycle
them. Core layout still checks the complete tree for duplicate identities.

`PaneHeader` supplies decorative tab icons, optional navigation availability,
commands, horizontal scroll offset and optional scrollbar style. Icon fonts,
navigation meaning and enabled state belong to the Host. Command IDs must be
stable, unique within the pane and in `1..=15`. All supplied command IDs are
validated, including on an empty pane; they cannot be hidden by inactive content.
Projection returns `WorkspaceProjectionError::{IdentityOverflow,
InvalidHeaderCommandId, DuplicateHeaderCommandId}`. Header errors retain pane
and command IDs. The enum implements `Display` and `Error`; match variants rather
than messages. Projection failure does not mutate the borrowed model, although
the library cannot undo side effects in caller callbacks.

## One complete geometry frame

Build a `DockFrame` from regions and the **same completed** `LayoutTree` before
admitting geometry-dependent input. `DockFrame::from_layout` returns a Result:
every required pane/header/list/body, tab and split node must exist, even when
offscreen. Missing nodes return Core `LayoutError::MissingLayout`; invalid bounds
return `InvalidConstraint`. Origins, extents and computed edges must be finite;
extents cannot be negative. Zero-sized layout is legitimate and has no hit area.
Do not retain an old interactive frame when construction fails or is incomplete.

`WorkspacePaneRegion`, `WorkspaceTabRegion` and `WorkspaceSplitRegion` identify
nodes; they are not independently published hit geometry. `DockPaneFrame` stores
clipped root/body/tab viewport bounds and actual layout flow direction.
`DockTabFrame` keeps the full rectangles **in authored item order**, including
offscreen tabs needed to resolve the original pre-move insertion slot.
`DockSplitFrame` keeps the full split bounds, logical container/split/axis and
actual flow direction. `split_frame` returns that complete snapshot; `split_rect`
is a bounds-only lookup for callers that do not interpret a pointer or delta.

`DockFrame::target` resolves a finite local pointer: tab split zones, an outside
float, workspace edge, tab insertion, then pane body/edge targets. `workspace:
None` disables targets. An invalid or zero-area workspace also disables them.
Returned `DockDropTarget` contains model intent plus physical preview/insertion
rectangles, not an exact future layout-size promise. Apply its pre-move slot with
the model's existing Dock operation; do not renumber after removing the item.

Model `Side::Left/Right` means authored first/second ordering. Under RTL layout,
the projection translates physical horizontal drop sides into that ordering.
Top/bottom semantics are unchanged. Pane and split records carry the layout's
direction, so a Host must not replace it with an assumed LTR value.

Manual `DockFrame`/region/visual construction remains a low-level interface:
the caller must preserve complete matching identities, valid geometry, order,
direction and clipping. A frame cannot prove model freshness. The Host retires
or rebuilds it after model, layout, scale, clipping or container changes. Do not
combine unrelated region/model/layout revisions or route native windows through
a combined fallback frame as though each logical container were an OS handle.

## Drag admission and native routing

`WorkspaceTabDrag` is a transient transaction supplied by the Host.
`begin_tab_drag` seeds the supplied item/container; the Host must admit the press
against its current model and input frame. `update_tab_drag` tracks the
current destination and starts after `DOCK_DRAG_THRESHOLD` (6 logical units).
`leave_tab_drag` clears a departing destination without consuming another
container's origin. `finish_tab_drag` consumes the transaction once and rechecks
live item and target ownership. The Host applies its returned action; no helper
mutates the model or captures the OS pointer.

For suspended/incomplete native displays, use `finish_tab_drag_with_admission`
or origin-scoped `finish_tab_drag_from`. Release rechecks both displays even if
no move followed a resize. A rejected owned release retires the drag; a foreign
release cannot consume it. `cancel_tab_drag` retires the origin or clears just
the suspended destination. Cancel on lost capture, container removal, unavailable
geometry or the Host's Escape policy. `FloatingMoveDrag` is only the original
pointer/placement data for a Host-owned floating-window move.

`DockWindowGeometry` uses desktop **physical** outer/client bounds and a finite
positive scale. `route_dock_pointer_with_admission` maps the source's logical
cursor through screen coordinates into the first front-to-back destination.
Native decorations and suspended windows still occlude lower windows. Invalid
foreground bounds cannot establish that a lower window is exposed. The Host
supplies the actual stack, coherent frames and admission callback; the library
does not query windows, monitors or permission. `route_dock_pointer` is the
all-admitted convenience form and requires the caller to guarantee readiness.
Cross-HWND capture and physical mixed-monitor behavior retain their separately
documented qualification limits.

## Split editing and tab reveal

Construct `SplitResizeSession::new(target, frame, ratio)` from a current
`DockSplitFrame`, the owning `InputTarget` and current model ratio.
`SplitResizeError` rejects nonpositive/nonfinite/overflowing bounds or a ratio
outside finite `[0,1]`. The session holds the original geometry and value;
`ratio_for_pointer` ignores foreign windows and nonfinite points. `finish` uses
the existing `ValueEditSession` one-shot commit/rollback outcome. The Host applies
preview values and restores the original on cancellation; it retires the session
if the underlying window/split geometry ceases to be valid.

`split_ratio_at(rect, axis, direction, pointer, minimum)` resolves the authored
first-child fraction. Invalid geometry/pointer/nonfinite minimum returns `0.5`.
A negative finite minimum means zero; the ratio retains the existing 5% bound,
with the requested minimum capped at half the extent. Small splits resolve to
the neutral half. `split_ratio_from_delta` takes movement along the physical axis
in the frame's **local logical units**, translating horizontal RTL motion. The
Host converts native pixel movement using the window's scale before calling it;
infinite deltas explicitly mean the physical beginning/end. Its minimum is
`MIN_DOCK_PANE_EXTENT` (120 logical units). Keyboard-to-delta mapping belongs to
the Host. `dock_split_ratio` reads the current model ratio for a container/split.

`dock_edge` and `dock_edge_band` return the closest eligible physical side,
rejecting invalid geometry/point and nonfinite fraction/band. Negative thresholds
admit no edge. `dock_tab_split_regions` and `dock_edge_preview` are low-level
value constructors: supply finite nonnegative, nonoverflowing geometry and, for
preview, a fraction in `[0,1]`. They do not validate layout or own model intent.

`reveal_active_tabs` borrows the Host's existing scroll-state map and a
per-window/pane `TabRevealGeometry` checkpoint map. It requests settlement after
selection/order/viewport/content changes, then preserves manual wheel/thumb
scrolling on unchanged geometry. An unrealized active tab cannot certify a
checkpoint. The Host supplies completed geometry, adapts its state keys and
reprojects if the return value reports changed scroll. No second scroll model,
clock or automatic scrolling thread is introduced.

`tab_drag_visual` derives the current container's eligible preview/title.
`dock_drag_overlay` projects `DockDragVisual` into a separate Host overlay scope;
it owns no hit routing, action commit or native cursor. Supply validated visual
geometry and viewport. Overlay lifetime follows the same drag transaction.

## Preview migration

Replace `WorkspaceIdentityOverflow` with `WorkspaceProjectionError` and handle
its three causes. Handle `DockFrame::from_layout` failure by retiring the old
frame and its interactions. Preserve actual `flow_direction` when constructing
pane/split frame records. Scalar split helpers now require explicit direction;
resize session construction consumes the complete split frame instead of six
disconnected arguments. Existing standard Standalone callers are migrated.
Published-pin Agent/Builder callers must migrate together at immutable adoption;
matching package version `0.1.0` alone does not imply API compatibility.
