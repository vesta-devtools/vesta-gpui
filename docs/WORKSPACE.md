# Workspace model and persistence

`vesta-workspace` owns the engine-independent logical pane/tab/split model.
`vesta-gpui-workspace` borrows it to produce UI and gestures; the Host owns native
windows, renderer/input lifetimes, application content and persistence I/O.
There is one private state field representation. A live `WorkspaceModel` owns
validated state; a `WorkspaceSession` owns a cloned, untrusted snapshot until
restore. The snapshot is not a second live model or a native window registry.

Value construction belongs to `types`; live operations and field ownership to
`model`; snapshot encoding/version admission to `session`; shared read-only
invariant checks to `validation`. The value layer does not call the mutable
model layer for primitive normalization.

## Public values

`PaneId`, `ItemId`, `SplitId`, `FloatingWindowId` are separate stable identity
types. Their value must not depend on title, index, Element ID, address or OS
handle. Zero is reserved. The model allocates monotonically and rejects an
exhausted cursor before mutation; `u64::MAX` is a terminal cursor, never minted.
Projection has its own smaller UI identity range and can reject model IDs that
the engine-independent model accepts.

`PanelTypeId` is an opaque Host registry key, not a registration or availability
check. `WorkspaceItem` contains an identity, title and that key. `Pane` contains
ordered items and its active item; `new` constructs an empty pane and
`active_item` performs the lookup. These are construction/snapshot values;
live model queries return shared references, not mutable access to them.

`WorkspaceNode` is a leaf or recursive split with `SplitAxis` row/column and a
ratio. `leaf`, `split`, `panes` construct/traverse values in authored depth-first
order; `split` clamps finite ratios to `0.05..=0.95`, using 0.5 for nonfinite
input. Directly constructed invalid values are rejected at model admission.
`WindowPlacement` is logical top-left/size data, not screen/DPI authority.
Negative origins are valid; admitted sizes are positive and all coordinates,
extents and resulting edges finite. `FloatingWindow` is a logical container;
it can be presented as a native window or an in-window overlay.

`ContainerRef`, `Side`, `DockAction` describe semantic operations. A merge slot
is in the **pre-move** tab order and clamps to the target's available range.
Only removal before that slot shifts it left. Split targets a pane, window split
targets a whole container, and Float needs a valid placement. These intents do
not perform native input or OS window operations.

## Live model operations

| API | State/failure contract |
| --- | --- |
| `with_tree` | Validate one main tree and complete pane set through the same invariants used by restore. Each nonempty pane explicitly selects an existing active item. Reject duplicate/reserved IDs, repeated or missing ownership, orphan panes, invalid active state/ratios or exhausted initial cursors. No silent repair. |
| `root`, `panes`, `pane`, `item` | Shared queries; `panes` traverses the main tree only, `item` searches all containers. |
| `active_pane`, `active_item` | The currently selected pane/item, including floating containers. |
| `floating_windows`, `floating_window`, `container_of_pane`, `container_of_item` | Shared logical ownership queries; no OS handle or mutable tree escape. |
| `add_item`, `activate`, `set_item_type` | Typed item operations. A missing target is an error, including type/title changes; rejected calls preserve content, selection and allocators. |
| `spawn_floating_window` | Check placement and item/pane/window ID capacity first; return `Result<FloatingWindowId, WorkspaceError>`. |
| `set_floating_title`, `set_floating_placement` | Narrow updates to an existing window; placement validates before assignment. No identity/topology mutation. Late events for retired windows may be explicitly ignored by a Host. |
| `dock_item` | Preflight targets and all required allocators before removal. Failing operations preserve the entire model. Successful operations update active selection and reclaim empty panes. |
| `set_split_ratio` | Resolve one container/split. Finite input clamps to `0.05..=0.95`; nonfinite input preserves the current ratio. Missing targets fail unchanged. |
| `close_item` | Return the removed item and reclaim empty panes. An all-empty main tree retains an existing empty pane; closing does not mint an identity. |
| `close_floating_window` | Deliberately discard this logical container's items. This is a Host-approved destructive operation, not the default OS close policy. |
| `restore_floating_window` | Atomically stage every floating item back into a main pane in authored pane/tab order, preserving item IDs. A native Host uses this when closing a presentation surface must retain its contents. |

Model errors are the single typed vocabulary for ownership, selection, geometry,
missing targets and allocation. First-error selection is deterministic, not an
aggregate diagnostic list. Match `WorkspaceError` variants, not Display strings.
An empty pane is valid; the recursive main tree always contains a leaf. There
is no separate empty-model state or fallback ID allocator.

## Versioned session boundary

`WorkspaceSession::capture` validates and clones the model. `to_json` serializes
the snapshot; `from_json` only decodes its versioned envelope. `restore` checks
version and all model invariants before returning `RestoredWorkspace`. Missing
Host panel types produce `UnavailablePanel` records while retaining their item
IDs, type keys and positions for later recovery. The availability callback is a
Host registry lookup, not permission to run a panel or mutate author data.

The v1 wire fields remain unchanged; a fixed v1 sample protects compatibility.
Session errors add JSON/version context or `InvalidWorkspace(WorkspaceError)`;
the latter preserves the typed cause through standard `Error::source`. Raw
`WorkspaceModel` Serialize is diagnostic output; Deserialize is deliberately not
implemented. Hosts must persist the versioned session and restore before use.
This package performs no file I/O, byte-budget policy, debounce, monitor clamping
or native-resource restoration. Hosts supply input limits and normal write
ownership. Tree/value construction does not impose a global node-count budget.

## Preview migration and qualification

Replace `floating_window_mut(...).placement/title` with the two narrow updates.
Handle the new Results from `spawn_floating_window` and `set_item_type`. Supply
valid active items for pre-seeded panes. Replace raw model deserialization with
`WorkspaceSession` restore. Session invariant matches now inspect the wrapped
`WorkspaceError`; unsupported-version handling is unchanged. `EmptyWorkspace`
is removed; resize reports a missing container through `MissingContainer`.

[Decision 0017](decisions/0017-Workspace-Invariants-And-Mutation-Authority.md)
records the actual ordering/allocator and admission reds. Existing model/Dock
tests cover identity, split/merge/close, session recovery and typed causes;
compile-fail docs protect the two removed admission escapes. A fixed v1 sample
and exhausted compound-operation tests protect persistent identity rather than
visual styling. Standalone uses the same new operations. The product handoff
binds final checks to source/artifact identities; Agent/Builder remain on their
published pin until the programme's immutable adoption exit. Existing physical
cross-monitor waivers are preserved, not replaced by these CPU contracts.

The projection family's remaining view/gesture/geometry review is tracked
separately in [the finite programme](REFINEMENT.md).
