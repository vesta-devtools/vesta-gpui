# GPUI basic controls and feedback — 0.1 source contract

The [package boundary](../../docs/SUPPORTED-API.md) owns current candidate support;
the dates below retain their original evidence scope. Actual exports are defined
by this crate/rustdoc. Choose a capability below rather than treating a historical
exit label as a later SDK qualification.

| Capability | Contract |
| --- | --- |
| Identity and controlled composition | [Composition](#composition-identities-and-state) |
| Buttons, choices and basic presentations | [Commands and choices](#basic-commands-and-choices) |
| Text/images, progress and ordinary feedback | [Text and feedback](#text-images-and-feedback) |
| Tooltip and notification lifetime | [Tooltips](#ordinary-metadata-tooltips), [notifications](#notifications) |
| Focus and popup routing | [Focus](#focus-and-routing--01-source-contract), [menus/popovers](#menus-and-popovers--01-source-contract) |
| Numeric, property and color editing | [Composite editing](#numeric-and-composite-editing--01-source-contract-gpn0106) |
| Borrowed editor and asynchronous suggestions | [Temporary editor](#temporary-editor), [suggestions](#editable-suggestions) |
| Scroll geometry and captured sessions | [Scrolling](#scroll-geometry-and-propagation--01-source-contract-gps01gps02) |
| Controlled List, Tree, Table and Tile | [Typed content](#typed-collection-content--01-source-contract-gpv01), [selection](#collection-selection-and-navigation--01-source-contract-gpv02) |
| Tree mutation, reveal and focus | [Tree controller](#controlled-tree-interactions--adr-0657) |

<a id="temporary-editor"></a>

The temporary editor (ADR-0653) adds `TemporaryEditorState<T>::exchange_in`
and `finish_into`, returning `TemporaryEditorExchange<T>`. A host can move an
entire editor into suspension and restore it once without cloning or reducing it
to a string. Use the same slot at both boundaries. A live-session or foreign
opener refusal leaves the slot intact; a foreign-window exit cannot restore it.
The original `open_in`, `finish` and `TemporaryEditorExit<T>` remain available.
The caller ends its native text/IME session before restoration and revalidates
composition, draft validity and author permission before requesting Confirm.
Only the displaced temporary draft is returned for the host's typed action;
the helper performs no document write, persistence or text-history operation.
`inline_edit_key_action` reserves plain Enter/Escape while editing; composition,
foreign sources and modified chords remain with the host text bridge.
See [the temporary-editor ledger (historical context)](../../docs/VALIDATION.md#historical-evidence)
for the exact source-workspace qualification; platform and SDK gates are separate.

<a id="editable-suggestions"></a>

The additive Editable Suggestions module (ADR-0634, qualified on2026-10-28)
provides `SuggestionState<K>`, exact owner/query `SuggestionRequest` tickets with
caller-monotonic `SuggestionRequestId`, `SuggestionOption<K>` and the controlled
`editable_combo_controlled` projection/`SuggestionViewIds`. Provider I/O and text,
IME/history, accepted values and durable data remain host-owned. Responses apply
only to the current pending ticket; invalid duplicate IDs reject atomically.
Caller IDs must not be reused across controller replacement or retired targets.
The original three EditableCombo declarations remain unchanged. Compact status
rows retain the supplied content identity; short lists shrink within the maximum.
The six source-workspace exits pass with public296 and actual Main/Float coverage.
Native, physical/Reader and immutable SDK qualification retain their own scope in
[the module ledger (historical context)](../../docs/VALIDATION.md#historical-evidence).

Historical Basic baseline: **frozen on2026-10-15 after GPB01–05 qualification** in the
[completion ledger (historical context)](../../docs/VALIDATION.md#historical-evidence).
This inventory covers the existing basic capability. Advanced collections,
editors, rich tooltip content, Workspace and renderer release qualification
retain their separate contracts and gates.

The 2026-10-22 `graph_node`, embedded-pin metrics, compact property row/set and
parameter field/slider presentations are the active ADR-0534/0535 extraction from
Studio. They are outside this frozen Basic inventory and do not raise its platform
or SDK completion status. Their host supplies stable IDs, finite positive zoom,
Catalog semantics, author validation, input capture/cancellation and one terminal
commit. Existing constructors keep their original default presentation.

`modal_dialog_with_labels` (ADR-0636) reuses the shared modal with Host-provided
confirm/cancel captions, applied to both visible text and button semantics.
`modal_dialog` and `ModalDialogSpec` retain their existing source contracts and
default Confirm/Cancel labels. The Host owns locale, modal input/focus admission
and the lifetime of typed choices; this addition does not requalify platform gates.

## Composition, identities and state

Palette hosts must put the owning overlay root above their own chrome in the
same stacking context; a descendant's z-index cannot escape a lower parent.
A consumed modal backdrop press uses explicit cancellation and owner focus
return. The shared non-modal `OutsidePress` policy keeps its existing behavior.
GPCM01–06 are qualified by the [incremental ledger (historical context)](../../docs/VALIDATION.md#historical-evidence),
with original executable identities preserved for retained presentations.

`NumericComponent` provides textual X/Y/Z/W, U/V and R/G/B/A identities with
shared RGB-axis accents and muted disabled colors. Existing vector and RGB(A)
labels use it; HSV labels retain their own neutral semantics. Compact Builder
parameter overlays consume the same presentation without changing the original
field/action identities or adding a focus stop (ADR-0619). This is a source-level
presentation addition, separate from the frozen Basic/platform qualification.

The Command Center (ADR-0616) adds `command_palette_matches`,
`CommandPaletteState<K>`, responsive `command_palette_geometry` and the explicit
`command_palette_scrolled` projection with `CommandPaletteScrollIds`. The old
identity/entry fields and constructor signature remain compatible. Search uses
all whitespace-delimited words across host-supplied label, shortcut and keywords;
the host still owns catalog meaning, query text, IME and typed execution.
Selection follows enabled stable IDs across filtering/reordering, with a defined
first-enabled fallback. Wheel scrolling stays independent of selection; keyboard
navigation reveals the current row. The virtual projection keeps only visible
rows plus one row of overscan and keeps the editor as its sole Tab stop.
The host must validate the source window/session, contain wheel input, revalidate
availability on execution, and finish its existing popup/text lifetime. An enabled
popup root must not be marked as an inert Action Boundary, which disables its
descendants. See [the fixed acceptance ledger (historical context)](../../docs/VALIDATION.md#historical-evidence).

The Breadcrumb interaction patch (ADR-0607) preserves its existing public
declarations. Current locations and ancestors without an action keep their
Label role, reject activation/Tab and form an inert Action Boundary inside
clickable composites. Caller IDs, actions, path and open state remain authoritative;
the host retires removed focus/capture and owns navigation. Its existing width
selector remains heuristic, preserving first/current locations, rather than
guaranteeing an exact fit for arbitrary labels or fonts. [The fixed scope (historical context)](../../docs/VALIDATION.md#historical-evidence)
is separate from physical input, Reader and immutable SDK qualification.

The Toolbar width repair (ADR-0593) keeps all existing public signatures. Its
ordered prefix includes the actual row gaps, group divider and overflow entry.
Below one button's extent the overflow entry retains that minimum footprint;
the caller must allocate it. Invalid widths admit no prefix, positive infinity
admits all items with a valid extent. Expanded actions retain their root IDs,
names, shortcut hints, typed routes and disabled boundary. The host owns open
state, focus reconciliation and action execution. This is a source-workspace
patch, outside the frozen Basic inventory; [its ledger (historical context)](../../docs/VALIDATION.md#historical-evidence)
does not certify physical input, Reader or the whole Toolbar family.

`WidgetIdentity` carries a caller-owned stable ElementId and ElementKey.
`Widget<A>` projects an Element subtree and typed action associations. `child`
preserves child actions; `map_action` lifts actions into a caller's type. The
current complete tree must pass the shared Core identity/Action validation.
Widgets do not own author data, execution, application clocks or platform services.

Menu projection also accepts `MenuItem::icon(resource)`, an optional host-resolved
image token in the existing leading slot. It preserves the item identity, label,
semantic role and typed action; an active checkmark takes precedence. This
additive menu contract does not change the frozen basic-control qualification.

`ControlState` supplies Idle, Hovered, Pressed or Disabled presentation.
`WidgetAvailability` is an already-evaluated host condition: hidden omits the
whole subtree; disabled retains geometry while excluding activation and focus.
The host reconciles focus/capture and replaces routes from the current tree.
Surviving identities remain stable through rebuild/reorder/translation;
removal does not revive a pending press when the element returns.

## Basic commands and choices

`tab_switcher` composes ordinary caller-owned tabs into Underline page navigation
or a Segmented scope selector. `TabSwitcherEntry` supplies stable tab, label and
decorative indicator identities, selected state, availability and typed actions.
The host owns selection; enabled tabs retain Tab/Enter/Space and disabled tabs
retain the ordinary activation boundary. Presentation adds no document-tab or
navigation runtime. This is an additive source API, not immutable SDK acceptance.

`button`, `icon_button`, `toggle_button`, `toggle`, `checkbox`, `radio` and `tab`
retain their original caller-owned identity/action constructors. Enabled
controls expose their semantic role/name and current toggle/selection state;
disabled controls reject activation and leave the Tab order. Icon commands
retain an explicit semantic name. `BinaryState` distinguishes Off, On and Mixed
for checkboxes. The caller applies the typed action and rebuilds from its state.

`icon_button_with_presentation` adds opt-in `IconButtonPresentation` without
changing `IconButtonSpec` or `icon_button`. `IconButtonContent::Glyph` keeps the
existing spec glyph; `Image(source)` replaces only the stable decorative child
using the shared centered Contain image path. The full Button name, shortcut
Tooltip, root hit/Focus/Action route and disabled boundary stay with the shell.
`selected: None` is the old ordinary action; `Some(false/true)` projects the
caller-controlled checked state. Selected chrome uses existing theme tokens;
disabled chrome/activation rules take precedence. Original resource pixels,
resolver diagnostics, cache and Browser unsupported-image boundary remain.
No image tint, loading service or internal selection state is introduced.
The [resource icon ledger (historical context)](../../docs/VALIDATION.md#historical-evidence) qualifies
this additive source contract separately from the frozen basic inventory and
immutable SDK adoption.

`button_with_presentation` adds Normal, Primary and Danger tones and Loading.
Enabled toned captions contrast with their opaque fill; ordinary and disabled
text retain their theme projection. Loading decorates the existing label while
preserving its accessible name/identities, publishes busy/disabled semantics,
and removes activation and Tab participation. It creates no clock or worker.

`radio_group` accepts caller-owned stable values, RadioIds and typed actions.
Duplicate values return `RadioGroupError::DuplicateValue` with the conflicting
element identity. Empty groups are valid. The selected enabled option is the
single Tab stop; otherwise the first enabled option is the stop, without
changing the caller's selection. The original standalone `radio` is preserved.
`radio_group_key_action` consumes current enabled values in visual order:
arrows wrap and return Highlight, Space/Enter confirm, and Tab/Escape remain
host keys. The host commits selection and restores the corresponding live ID.

## Text, images and feedback

`CaretBlink` retains one window's live editable ElementId and exposes visible
phase/deadline. The host supplies time to update/tick, restarts on input, and
cancels with no owner on blur/removal. Its 500ms wakeups create no timer, thread
or redraw loop; the host combines the deadline with its ordinary idle policy.

Labels, validation messages, image tokens and `WidgetTheme` remain derived
projections over Core Elements. Text uses supplied fonts and the qualified Core
layout/shaping path. Images and their loading/missing/failed/recovery resources
belong to the host resolver; these widgets do no image I/O. Validation feedback
does not create or apply a domain rule. Theme metrics/palette/typography are
explicit inputs, and custom palettes own valid color values.

`progress_bar_state` returns `ProgressStateError` for a non-finite value/phase.
Finite determinate values clamp to0..1 and publish percentage range/value.
Indeterminate state publishes Working/busy without a fabricated numeric value;
its finite phase cycles, and the host schedules visible animation. The original
`progress_bar` convenience projection remains available. The shared progress
fill uses its dedicated palette token so its caption remains readable across
filled and unfilled track regions in the stock themes. `busy_indicator` and
status/validation projections expose the current host-owned state; Core Status
semantics publish a polite live setting. Projection is separate from actual
platform announcements and reader focus qualification.

## Ordinary metadata tooltips

Keep one `TooltipController` per source window and supply current route,
viewport, clipped anchors, deliberate visible focus, active state and time
through `TooltipFrame`. Pointer focus follows the host's hover delay;
keyboard/accessibility focus discovers metadata immediately. Empty hints and
fully visible text repeated verbatim are suppressed, while metadata remains
available to accessibility. Leave, blur, removed/clipped owners and modal scope
changes clear invalid sources. Hosts schedule the earliest live deadline across
controllers and apply `reveal_due` to its owning window.

Ordinary popups are passive, bounded, measured from the final popup tree/width
including border/padding, and flip/clamp against viewport bounds. Text retains
its full source; fixed viewport height still bounds the popup. The host supplies
valid source identity and redraws. `layout_with_popup`'s optional caller-owned
rich projection retains its separate proposed/experimental qualification.

## Notifications

`ToastQueue<A>` is a bounded FIFO of four notices with caller-owned `ToastId`,
typed action, message and deadline. Duplicate live ID, full queue and a deadline
at/before supplied time return `ToastQueueError`. Expiry removes all due entries,
including later entries with earlier deadlines; `next_deadline` is the earliest.
Hosts reconcile expiry before accepting a queued action/dismissal and own
timing, redraw, action application and dismissal. Widgets do not spawn a timer.

`toast_widget` publishes Status/message, optional typed action and a named
Dismiss notification button whose compact caption remains inside its hit frame.
`NotificationFocusState` retains window-qualified openers; return validates the
current route/modal scope and only occurs while focus belongs to that notice.
Removed/disabled/foreign openers cannot receive a return. The host applies the
return and prunes snapshots. Before clearing live ownership on blur, hosts may
call `observe_window_activation` with that window's current target. Recording a
notice copies this inactive return history, with current targets taking
precedence. Activation clears only the inactive fallback; `forget_window` clears
both the closed window's fallback and existing notice return references. This
history owns no live focus/capture and cannot activate a window. Hosts must
report activation and removal to keep it bounded by live source windows.
Actual announcements, held-expiry return and
floating-window composition retain the ledger's native qualification.

## Focus and routing — 0.1 source contract

The existing routing/interaction contract is frozen on2026-10-19 after all
original ADR-0419/GPF01–05 exits in
[the completion ledger (historical context)](../../docs/VALIDATION.md#historical-evidence). Dated source,
portable and native evidence remain distinct. This adds the qualified focus scope
to the retained basic/menu contracts without certifying unrelated exported widgets.

`route_key` validates the source window/text owner and returns composition keys
directly to text input before any other layer. Otherwise it visits Capture,
Menu, Popup, Gesture, TextCommand and FocusedControl in order, stopping at the
first consumer. A text owner then keeps editing/Tab/activation; ordinary
non-text Tab/activation precedes shortcuts, and a modal scope blocks global
shortcuts. Hosts supply current state, apply typed actions and use the shared
Core live `FocusScope`; the router does no OS input or document mutation.

## Menus and popovers — 0.1 source contract

This source contract is frozen on2026-10-18 after GPM01–05 qualification in
[the completion ledger (historical context)](../../docs/VALIDATION.md#historical-evidence). It combines
the ledger's dated Standalone native cases with the affected source consumers;
command-origin Desktop/Builder runtime checks retain their explicit origin.
General focus/input, text editing and immutable SDK adoption remain separate.

`MenuController` retains the owning window, stable menu/item path, highlights
and opener. Reconciliation keeps enabled stable choices across reorder and
removes invalid highlights/submenu descendants without activating a replacement.
Keyboard and pointer use that same path. Disabled entries, separators and
section headings do not activate. Submenu entry refuses any menu already on the
open path; initializing a foreign descriptor's edge cannot replace the current
highlight after that refusal. Projection uses `MenuItemIds` and existing
measured popup layout/hit/semantic paths; labels, shortcuts, checks and icons
do not replace item identity. The host owns descriptor updates, valid submenu
links, typed dispatch and rebuilding the current action route.

`menu_popup` and `menu_picker_popup` measure with caller-supplied fonts, project
the same stable `MenuItemIds`, and preserve full-label tooltip metadata unless
the descriptor supplies an explicit override. Explicit empty hints stay empty;
the existing TooltipController suppresses fully visible repeated labels and
reveals clipped labels. Identity/action callbacks adapt data and execute no
business action. Hosts publish the highlighted row's focus through the current
route after accepted opening/navigation/hover; a later explicit focus or close
supersedes any deferred request. Rebuilds do not force focus on every frame.

`PopoverState`, `PopupDismissal` and `ModalStack` retain explicit source-window
ownership. Only the top modal and its own window may dismiss it; duplicate/full
stack requests reject without replacing a live entry. Explicit/Confirm/Traverse
closures may request the opener, which still needs validation against the rebuilt
window/modal scope. Outside press keeps its original target/action; blur,
owner removal and window teardown do not restore stale focus. Foreign or repeated
closure requests do not consume a different live session. Popup Tab must be
resolved against the post-close route when the route's tab stops change.

Hosts cancel preview/capture ownership before ending text sessions, then rebuild
and publish current focus/semantics. Placement, return intent and local semantic
projection are separate from native capture, platform focus and reader evidence.

## Compatibility and adoption

Existing constructors retain their documented behavior unless a migration is
explicitly listed in [candidate notes](../../release/0.1.0-rc.2.md). The advanced
capabilities below have their own contracts and dated qualification; they are
not subordinate compatibility shims or a blanket extension of the Basic baseline.

## Numeric and composite editing — 0.1 source contract (GPN01–06)

The original GPN01 typed-draft/validation/terminal exit is qualified as of
2026-10-21. Its [source and dated native review (historical context)](../../docs/VALIDATION.md#historical-evidence)
freezes the existing draft helper contract and qualified Host integration below.
GPN03 mixed/vector composition is also qualified as of2026-10-21 by the
[original requirement/source/native review (historical context)](../../docs/VALIDATION.md#historical-evidence).
GPN04's existing color/Ramp composition is qualified by the
[full original source and native review (historical context)](../../docs/VALIDATION.md#historical-evidence).
GPN05 text/IME/clipboard/history composition is qualified by the
[current source and dated native review (historical context)](../../docs/VALIDATION.md#historical-evidence).
GPN02 pointer preview/threshold/terminal ownership is qualified by the
[full original pointer review (historical context)](../../docs/VALIDATION.md#historical-evidence).
All original GPN01–06 exits and the whole numeric/composite source contract are
qualified by the [complete family review (historical context)](../../docs/VALIDATION.md#historical-evidence).
Previously qualified basic controls and value/pointer sessions retain their
original scope. This is a0.1 source contract, not an immutable SDK publication.

`number_field` projects caller-formatted display text and typed decrement/
increment actions. `editable_number_field` projects a caller-owned Core
`TextInputSnapshot`, focus and numeric value/range/step metadata with those
actions. `NumberFieldIds` supplies each stable part identity; the editable input
uses `value_label`. These constructors do not parse or commit model values.
[Projection authority](src/controls.rs).

`vector_field` and `editable_vector_field` project three X/Y/Z components with
independent caller-supplied identities and typed step actions. `VectorFieldEdit`
supplies three independent snapshots/focus states and range/step metadata.
`VectorComponentValue::Mixed` remains distinct from a numeric value; display
formatting also treats nonfinite components as mixed. The host validates finite
commits, keeps per-component drafts/history and preserves semantic component
identity when composing or reordering surrounding rows. `property_row_widget`
wraps the supplied value and caller-evaluated availability/validation; its UI
message is not author-document validation. [Vector](src/vector_field.rs),
[property](src/property.rs).

This GPN03 source contract covers the existing `VectorComponentValue`,
`VectorFieldIds`, `VectorFieldSpec`, `VectorComponentEdit`, `VectorFieldEdit` and
both Vector constructors. X/Y/Z share the same projector and validation/terminal
adapter while keeping separate stable input/step IDs, snapshots and history.
Caller-owned mixed state is not numeric zero; hidden/disabled or removed source
owners cancel unaccepted composition and retire input without changing sibling
axes or another source. Re-enable does not revive retired ownership. The group
inherits its ancestor clip while preserving each one-line component's local
clip, so the shared reveal path can expose a narrow offscreen component.
Native preedit/source-close and relevant mixed-Y DPI terminals retain their
dated scopes; this does not claim a new all-axis native IME/clipboard matrix.

`resolve_value_draft` invokes the caller's parser once. `None` returns
`DraftResolution::Rejected` with the current value to restore; `Some` returns
`Accepted` and a `PartialEq`-based `changed` flag. The helper itself does not
impose a number type, finite check, range, formatting or history policy.
The host handles those through its typed contract and avoids new history for
unchanged accepted values. [Draft/session authority](src/value_edit.rs).

Accepted model values and Core text drafts remain separate. The qualified Host
Number adapter trims/parses i32, rejects overflow/invalid input and clamps its own
declared range. Vector components independently accept finite f32 values within
their declared range; mixed remains distinct from a numeric value. Draft Undo/
Redo does not commit the model. Admitted Enter, Tab and ordinary focus transfer
resolve the current draft; Escape restores its accepted value. Owner removal or
window unavailability cancels an unaccepted draft before retiring text ownership.
Foreign/late input cannot finish another source's session. Idle text projections
follow accepted step/overlay changes without overwriting a newly active draft.
These are UI adapter rules, not new numeric limits imposed by the helper or a
replacement for Core author-document commands/validation. Native Main/Float,
clipboard, actual IME and relevant-DPI records retain their individual dates,
artifacts and scopes; literal typing/paste is not IME proof. Whole composite
controllers and pointer gestures retain their separate qualification exits.

The qualified GPN05 Host integration routes current source-window/caret input
through the existing Core text state and per-window winit bridge. Selection,
clipboard Cut/Copy/Paste, preedit/commit and draft Undo/Redo operate on the same
caller-owned draft; none implicitly accepts a numeric or parent-color model.
Ordinary admitted terminals resolve through the existing typed parser, while
source loss cancels unaccepted composition before ownership retires. The caller
keeps accepted-model, text and parent-controller histories distinct. Foreign,
retired and late input cannot commit another source's numeric session. Actual
clipboard, received native IME and portable late-event tests retain their
separate evidence; a nonempty native late-IME delivery or arbitrary editor-by-
clipboard/IME/DPI matrix is not inferred. Existing text exports are unchanged.

`ValueEditSession` retains an `InputTarget` and immutable initial value. Only its
source window may finish; a foreign or repeated terminal returns `None` without
finishing the session. An admitted cancel returns the initial value, an unchanged
commit returns `Unchanged`, and a changed commit returns the before/after pair.
The host owns preview storage, model writes, capture, history and loss handling.

`HorizontalEditSession` uses that same session. Construction rejects nonfinite
origin coordinates with `InvalidGeometry`, and negative/nonfinite thresholds
with `InvalidThreshold`; threshold zero starts dragging immediately. Foreign,
nonfinite or overflowing pointer deltas cannot advance it. Crossing the threshold
keeps drag mode until termination. An accepted below-threshold release yields
`Click`; cancellation always yields `Cancel`, including before the threshold,
and never activates a pending click. A matching terminal consumes the session
once. It performs no value arithmetic, pointer capture or OS event delivery.

An individual stale numeric cancel restores its own immutable initial value and
retires only the matching stable target's Primary capture. A replacement owner
or another button survives. Whole-window unavailability retirement remains a
separate Host operation. The current exact-origin native click retains1 with
no Move/drag; the old246 event's missing motion/cause remains unknown. The
qualified numeric cancellation route composes numeric-specific tests with the
completed native key/capture foundation; no new physical held-numeric Escape
or universal loss-by-DPI run is claimed.

`color_picker` projects caller-owned byte-RGBA draft, RGB/HSV mode, channel display
strings and Hex snapshot, with typed mode/channel/eyedropper/confirm/cancel actions.
It does not implement platform eyedropper capture or commit/history policy.
`format_rgba_hex` emits `#RRGGBBAA`; `parse_rgba_hex` trims, accepts six/eight
ASCII hex digits with an optional `#`, defaults missing alpha to255 and rejects
other input. `rgba_to_hsva`, `hsva_to_rgba` and `color_channel_step` are UI byte-color
helpers, not Core color-space transforms. Channel stepping rejects an index
outside0–3, clamps byte/percentage channels and wraps hue as defined by the shared
implementation. [Picker](src/color_picker.rs), [byte helpers](src/color_edit.rs).

`ramp_editor` projects stable caller-supplied point IDs, positions/colors/selection
and typed point actions. `RampSpec` supplies width/interpolation; the constructor
returns `RampError` for invalid width/position/order and duplicate checked point/
element identities. Complete tree/key validation still belongs to Core.
`ramp_group_translation` returns keyed UI edits using the immutable starting
positions and live selected identities, preserving group spacing at the edges;
it rejects absent owners/positions or nonfinite movement. Geometry helpers and
`ramp_color_at` share the module's logical track/travel and UI interpolation.
The host applies edits and history through its existing model authority.
[Ramp authority and exact bounds](src/ramp.rs).

The qualified GPN04 source contract includes these existing Picker/byte-color/
Ramp exports and their signatures. The Host keeps accepted color, parent preview
and Hex text draft separate. RGB/HSV changes preserve an active Hex draft and
its text history; Hex acceptance updates parent preview, while whole Apply or
Cancel resolves only the source-qualified parent session. A Ramp color Apply
targets the captured stable point and creates one model-history change; text
Undo/Redo and whole Ramp Undo/Redo are distinct. Foreign/late actions cannot
close or change another source's session/preview/capture. Owner cancellation
returns to the admitted opener only when it still exists. Native records retain
their dates/artifacts; no new platform eyedropper, Core color transform, per-
channel native matrix or immutable SDK guarantee is added.

## Scroll geometry and propagation — 0.1 source contract (GPS01/GPS02)

The complete original GPS01/GPS02 geometry, clipping/extents and nested two-axis
propagation exits are qualified as of2026-10-21. The [source/consumer/native review (historical context)](../../docs/VALIDATION.md#historical-evidence)
preserves the original portable refusal/remainder cases and dated Main/Float/
narrow/relevant-DPI observations. Current focus-follow, captured-gesture lifecycle
and complete scrolling integration have the qualified GPS03/GPS04/GPS06 contracts
below.

`ScrollAreaState` and `ScrollbarDrag` keep the existing signatures, finite
geometry/range behavior, physical-direction remainder and LTR/RTL mapping.
Nonfinite movement or pointer updates are atomic no-ops; wheel movement is
returned unconsumed. Invalid/nonfinite content or viewport dimensions leave the
accepted range, offset and direction intact. Reveal rejects invalid rectangles,
overflowed edges or nonfinite margins. Initial nonfinite components normalize to
zero, independently, like existing negative components. A drag cannot publish
NaN after its range shrinks to zero. Layout diagnostics remain Core-owned.

Until the first accepted geometry synchronization, initial maximum offsets equal
the normalized initial offsets. Accepted content/viewport dimensions are finite
and nonnegative; each maximum is max(content-visible,0), and an existing offset
clamps to that maximum. Synchronization's bool reports an offset change
(`sync_layout` also reports a flow-direction change), so accepted range-only
growth can return false. The current `LayoutNode` supplies reserved-gutter
viewport/content dimensions and direction from the same Core projection used by
painting and hit testing; hosts do not create a competing extent or clip model.

`scroll_reveal_path` validates the target and all supplied scroll viewport
rectangles/margins before returning an executable plan. An invalid target or any
invalid viewport returns no steps, including any previously collected inner
steps; a rejected inner reveal cannot move an outer area. The callback supplies
current geometry, and the caller applies the returned plan afterward. `None`
continues to skip a non-scroll ancestor. Normal inner-to-outer ordering,
zero-size caret targets and finite negative margins remain unchanged; each
retained state still applies its existing nonnegative margin handling.

The geometry callback is a current read-only lookup; applying returned steps is
a separate caller action. The planner does not undo side effects a caller puts
inside that callback. `ScrollAreaState::reveal` limits each positive margin to
half the spare viewport space on that axis, so a nearly full target is not
clipped by the requested margin. Oversized targets retain the existing
leading/trailing-edge policy rather than changing authored dimensions.

The existing stateful regressions cover these boundaries together with ordinary
range/reveal/propagation and thumb geometry. Current Core53/Widgets27, Host39 and
the public interaction consumer retain exact source bindings; the original
native observations supply their own Main/Float/DPI scope. A logical scale or
callback-driven preparation is not relabelled as physical input. The immutable
`ScrollbarDrag` supplies coordinate arithmetic. The Host owns capture and its
terminal policy under the qualified lifecycle contract below.

`ScrollInputPath::new` obtains the current source/modal-qualified path from
`FocusScope::path_to`; a foreign, unavailable or outside-modal target returns
`None`. Build it for the current admitted input event: the owned path does not
revalidate a later tree or retain window/capture/scroll state. `begin_drag` walks
inner-to-outer until the caller supplies a drag, rejecting a nonfinite pointer
before invoking that callback. `scroll` returns nonfinite movement unchanged
without invoking the callback; otherwise it forwards the caller's remainder
inner-to-outer, stopping when both components are within `f32::EPSILON` of zero.
The caller selects the owning window's states and supplies valid remainder
semantics, normally through the same `ScrollAreaState` methods.
[Input-path authority](src/scroll_input.rs).

The qualified exports are `ScrollAreaState`, `ScrollAxis`, `ScrollbarDrag`,
`ScrollRevealViewport`, `ScrollRevealStep`, `scroll_reveal_path`,
`ScrollInputPath` and `scroll_wheel_path`, with their existing fields/methods.
Signatures, ordinary finite arithmetic and bool/Option/remainder failure behavior
are preserved. An already built input path is not a subscription to later tree
changes; the Host rebuilds it for each admitted current event and owns the
window's retained state. [Geometry authority](src/state.rs).

## Captured scrollbar sessions and loss — qualified original GPS04

The complete original GPS04 exit is qualified as of2026-10-21 by the
[source/consumer/native review (historical context)](../../docs/VALIDATION.md#historical-evidence).
Admit a current hit through the owning window's live action tree and retained
scroll geometry. Use the same Core scrollbar projection as paint/hit testing;
the native thumb takes precedence where it overlaps frame resize. Capture the
returned stable element in the actual owning window with Primary. Matching
Primary release ends that window's thumb session; foreign windows/buttons and
duplicate terminals cannot finish or revive it.

The Host retires old sessions on source/availability/modal loss, focus/window
loss, Escape and geometry replacement, including resize and scale change.
Targeted geometry retirement removes only the matching window/element/Primary
capture, preserving a replacement owner/button. Broader focus/modal/window
cleanup remains scoped to that window. Navigation cancellation retains the
reached offset, subject to ordinary clamping against accepted current geometry;
it does not use value-editor rollback or create an undo transaction. Retired
motion/release cannot resume the old session. `ScrollbarDrag` itself remains an
immutable coordinate mapper and does not own lifecycle or capture.

Actual Main/Float thumb and relevant100/125/150/200% evidence retains its original
dates/artifacts. Scrollbar-specific held source-hide, modal, actual OS blur and
actual resize have their own native records. Native key delivery during capture
reuses the completed GPF03 Main/Float Escape records with exact current keyboard,
key-precedence, admission and capture-terminal source. Current scrollbar Escape
and late/replacement ownership use the existing comprehensive regression. This
is qualified composition of those boundaries; the dated slider Escape records
retain their control scope. No new physical held-scrollbar Escape or held-DPI
transition run is claimed, and no all-widget/loss/DPI cross-product is implied.

## Current focus and caret reveal — qualified original GPS03

The complete original GPS03 exit is qualified as of2026-10-21 by the
[five-requirement review (historical context)](../../docs/VALIDATION.md#historical-evidence).
This composes existing Core text/focus and shared scroll contracts; it adds no
public pending-request controller or second layout model. The Host retains a
window-qualified stable target for deliberate focus/edit/navigation intent.
Foreign or unstable frames do not consume that request. A current stable frame
validates the live source/session, canonical field owner and modal FocusScope,
then derives the caret from its focused painted `TextInputViewport` and plans
against current full scroll-ancestor viewports. Superseded or removed owners
cannot revive a pending request. Apply the shared reveal plan before publishing
the replacement frame; do not submit a cached pre-reflow caret.

The Host preserves drafts, accepted values, history and unrelated retained scroll
states. Later manual scrolling supersedes that window's pending caret intent;
resize without a pending focus/edit request does not create unconditional
always-follow. Shared editor data/selection may have two presentations while
each window retains its own manual vertical Area viewport. Successful Float
redraws send the current text viewport to their own IME bridge. Caret blink phase
is not geometric visibility, and requested candidate geometry does not specify
the OS popup's exact pixels.

The review retains original field/composite/deferred virtual-focus and relevant
Main/Float/narrow/physical-DPI/IME evidence with its original dates and artifacts.
The final registered-Float gap uses real Main wheel and Float click/Home with
both Areas continuously presented: Main stays at its manual marker while Float
reveals the first line. Startup fixture data is command-origin preparation,
not OS IME. The complete scrolling contract below reconciles the original GPS
exits; numeric and collection qualification remain separately scoped.

## Narrow nowrap projection — qualified original GPS05

The complete original GPS05 narrow-nowrap/floating/relevant-DPI exit is
qualified by the same [dated source/native review (historical context)](../../docs/VALIDATION.md#historical-evidence).
No new constructor or private layout model is introduced. Existing Core
`Style`/`FlexWrap`/text-wrap/overflow and derived extent/clip behavior remain the
authority: intentional Clip and authored fixed dimensions stay intact; NoWrap
content is not silently wrapped to make a narrow viewport fit.

Original125% and actual100/150/200% Main internal124px and Float internal124px
within a560px window have separate artifact-bound real start/end wheel, split
restoration and window/source-isolation evidence. The earlier whole-window Float
resize is retained as its own proof and does not substitute for the internal
split. Restoring width releases the excess horizontal range. These source
contracts preserve the existing Core layout contract and are not an immutable
SDK or a universal OS wheel-unit/atomic-vector/RTL-by-DPI guarantee.

## Complete scrolling integration — 0.1 source contract (GPS06)

All original GPS01–06 exits are qualified by the
[completion review (historical context)](../../docs/VALIDATION.md#historical-evidence).
The existing eight scrolling exports above keep their signatures and failure
behavior. Their shared Core geometry, caller-owned state, current input path,
atomic ancestor reveal and Host lifecycle form one contract; panels do not add
competing scroll controllers. The focus/caret and narrow-nowrap clauses above
remain part of that contract.

The review binds current Host39, retained Core53/Widgets27 and the actually
executed public nested-scroll/reveal consumer. The three original Desktop
scrolling/file-list/trajectory consumers pass in the actual r49 Desktop71 run,
with179 direct inputs and1761 current source records independently reconciled.
Workspace Tab reveal retains its shared authority. The native starter/interaction
examples keep their compiler-only evidence. Existing native dates and failed
attempts are preserved. Compatible fixes retain0.1 source adoption; an incompatible
supported change requires the documented version migration. Numeric/collection
completion, immutable SDK publication and broader renderer/platform/performance
guarantees remain separate.

## Typed collection content — 0.1 source contract (GPV01)

The full original GPV01 typed-content exit is qualified as of2026-10-21. The
[source and consumer review (historical context)](../../docs/VALIDATION.md#historical-evidence)
binds the actual30 ordinary/40 lazy public projections, current shared source and
the original compatibility requirements. It freezes existing typed-content
signatures/projection behavior for List, Tree, Combo, Table and Tile. Selection/
navigation (GPV02), Tile offscreen focus/reflow (GPV04), complex Table editing
(GPV05) and whole native collection reuse/freeze (GPV06) remain separate.

Typed `list_item_from`/`list_item_with_content`, `combo_option_with_content`,
`tree_item_with_content`/`tree_view_row_from` and the Table/Tile content entries
project caller-owned records through the same widget shells as the text wrappers.
The caller supplies stable object/row/column/child identities and owns editing
buffers, selection and filtered order. Nested actions and text inputs retain
their own action boundaries; complete tree/route validation stays in Core.
[Content wrappers](src/controls.rs), [editable choice](src/editable_combo.rs).

`WidgetContent` keeps an explicit accessible label and the supplied complete
`Widget<A>` subtree. Typed record factories and text wrappers delegate to the
same List/Tree/Combo/Table/Tile projectors; geometry indices are not object IDs.
Tree disclosure/select actions and Table header/cell children remain distinct
from their parent actions. Core editor children keep text/focus ownership rather
than activating their row. A disabled shell blocks descendant actions through
the existing Core action boundary. Current clipping, hit testing and semantic
tree validation remain Core authorities. The executed consumer covers both
logical source windows, stable rebuild/reorder/edit/deletion and bounded lazy
composition with real child actions/editors. This is source/portable evidence;
it is not a new native collection or exhaustive future-custom-editor claim.

## Collection selection and navigation — 0.1 source contract (GPV02)

The full original GPV02 exit is qualified by the
[source/consumer/native review (historical context)](../../docs/VALIDATION.md#historical-evidence).
Actual current Main List and narrow Float Tree modifier/hierarchy operations
retain their source-window focus, selection and input boundaries. Typed hierarchy
fixtures and native Table filtering retain their distinct evidence.

`ListSelectionState<K>` keeps a selected-key set and range anchor, without a
window, input adapter or author document. `select` uses the caller's current
eligible/visible order: plain selection replaces, additive selection toggles,
range selects the inclusive anchor-to-target interval, and additive range keeps
the prior set. An absent target is a no-op. A hidden/missing anchor falls back to
the current target when selecting a range. `reconcile` takes the caller's live
data identities, drops deleted selections and replaces a deleted anchor with
the first remaining selected key in that order. To retain hidden Tree selection,
do not confuse a collapsed visible-order projection with deletion of live data.
Source-window/modifier/focus admission and disabled-item policy remain caller
responsibilities. [Selection authority](src/selection.rs).

`VisibleTree::build` validates the supplied preorder depths, leaf ancestry and
unique IDs through `TreeViewError`, then applies `TreeExpansionState`. Expansion
reconciliation removes deleted or no-longer-parent identities. `key_action`
returns stable focus/expansion intents over the current visible tree; it does
not mutate host data or selection. `linear_collection_index` rejects empty or
stale indices/unsupported keys, clamps endpoints and uses the caller's page row
count (at least one). Overscan or an extra retained row is not a visible page.
[Tree](src/tree_view.rs), [linear navigation](src/collection_navigation.rs).

`collection_focus_after_change` keeps a surviving key; otherwise it prefers a
supplied live ancestor, then a following/preceding survivor in the previous
order, then the first current key. An unknown old owner or empty destination
does not acquire an unrelated array-position identity. `reveal_collection_row`
is the fixed-height adapter over caller-synchronized `ScrollAreaState`; variable
rows use their current `VariableListMetrics::row_bounds` instead. Hosts reconcile
focus/capture against the rebuilt `FocusScope` and publish only current geometry.
[Repair/reveal authority](src/collection_focus.rs).

The Standalone's exact-row Ctrl+Space binding is a Host adapter over the same
selection actions, not a global modified-activation rule or a window guard inside
`ListSelectionState`. Hidden/collapsed data and source-window admission remain
caller-owned. The complete GPV01–06 source contract is qualified by the
[whole collection review (historical context)](../../docs/VALIDATION.md#historical-evidence);
the bounded constructors below retain their own documented preconditions.

## Table projection and complex editing — 0.1 source contract (GPV05)

The full original GPV05 exit is qualified as of2026-10-21. The
[source/consumer/dated native review (historical context)](../../docs/VALIDATION.md#historical-evidence)
binds current typed Table composition, track/resize/reveal/source ownership and
actual Main/Float filtering, Note history/terminals, IME and relevant DPI evidence.
Existing text/content ordinary/windowed/scrolled constructors and Table types,
`TableColumnResize`, `table_column_width_at` and `table_column_width_for_key`
retain their signatures and the source contract below.

`table_view_windowed_scrolled` and
`table_view_windowed_scrolled_with_content` project one caller-owned horizontal
offset through header and body, inheriting Core LTR/RTL rules. Vertical offset
stays in `VirtualListRange`. Nonfinite, negative or beyond-content-width offsets
return `InvalidViewport` before row factories run. The header uses a transparent
overlay ScrollBox projection with no separate state; the body owns the visible
scrollbar. The original constructors retain their zero-horizontal projection
and signatures. Their typed-content signatures/projection share qualified
GPV01; whole native-family integration/freezing remains under GPV06.

Text and typed-content Table constructors use the same declared logical column
widths for headers, resize handles and body cells. Constrained containers keep
these tracks and clip overflow instead of independently shrinking the header.
Header child actions remain distinct from sorting; nested cell controls remain
distinct from cell/row selection. Windowed projection keeps the bounded visible
rows and at most one retained offscreen row, whose children remain focusable but
clipped out of pointer hits. Existing signatures, stable identities and caller
selection/edit ownership remain unchanged.

Table constructors reject nonfinite/nonpositive column widths and resize metadata
whose finite minimum/maximum/keyboard step do not satisfy minimum>0,
maximum>=minimum and step>0, using `InvalidColumnWidth`/`InvalidResizeBounds`.
`table_column_width_at` and `table_column_width_for_key` are arithmetic helpers,
not those validation entry points: they expect accepted resize metadata and finite
input widths/motion. Pointer arithmetic clamps to the supplied bounds; keys use
one/four keyboard steps or the exact minimum/maximum. Hosts validate platform
movement before calling them. Do not infer structured rejection of malformed
metadata from the helpers' `f32` return type. [Authority](src/table_view.rs).

The focused regression binds column geometry to clipping and hit/action routing
in LTR/RTL wide/narrow containers; logical scale inputs are not native DPI or
platform input evidence. The caller owns query/order, stable row/cell/column and
child identities, selection, accepted values and editing buffers. Reorder does
not derive identity from visual position. Owner removal retires unaccepted text
and capture through the current route/session; a child editor retains its own
keys and cannot accidentally select/sort an ancestor. Header sort under ordinary
blur resolves the admitted draft; focus-preserving reorder keeps that draft.

Host resize preview uses the current displayed track geometry and a source-
qualified session; commit materializes the accepted widths and cancellation
restores the starting snapshot without clearing a replacement capture. Resize
alone does not create an always-follow intent. GPV02/GPV06 and whole collection
freeze retain their separate requirements. Ordinary Table content is separate
from document Markdown table rendering, including UI-DOC-TABLE-001.

## Tile range, content and retained focus — 0.1 source contract (GPV04)

The full original GPV04 exit is qualified as of2026-10-21. The
[requirement and dated native review (historical context)](../../docs/VALIDATION.md#historical-evidence)
binds the actual independent consumer, current Host and Main/Float/narrow/
relevant-DPI evidence. Existing `TileGridGeometry`, `TileGridRange`, `TileGridIds`,
`TileGridItem`, `TileGridContent`, `TileGridError`, `TileGridDirection`,
`tile_grid_range`, `tile_grid`, `tile_grid_with_content`,
`tile_grid_windowed_with_content`, `tile_grid_windowed_with_retained_content`,
`tile_grid_next_index` and `tile_grid_next_index_in_direction` retain their
signatures and the following source contract.

All Tile constructors share `tile_grid_range`. Nonfinite components, invalid
dimensions and overflowed width/height pitches return the existing
`TileGridError::InvalidGeometry` before a lazy factory runs, including empty
collections. Count-to-row rounding and row-to-index multiplication cannot panic
or produce reversed/out-of-count ranges. Representable ordinary geometry,
column/track limits, signatures and the existing projector remain; this adds no
new density quota. Invalid/empty geometry does not call the lazy factory.

`tile_grid_windowed_with_retained_content` builds the visible range and at most
one explicit offscreen focused item. Its caller maps the current stable item or
child identity to a current index, omitting removed/disabled or stale owners.
Out-of-range indices are ignored and visible retained items are built once.
The original `tile_grid_windowed_with_content` signature/behavior remains and
delegates with no retained item. All paths use the same Tile projector.

The retained tile occupies its actual column and row using a relative offset
in the first visible track, preserving inherited RTL layout without allocating
intervening rows or changing total content height. The host still owns its
ScrollBox, current offset, focus eligibility, reveal and selection. Retention
keeps focus/action/semantic identity; an offscreen child remains clipped out of
pointer hits. Removal/disable retires the old source through the current route;
re-enable does not resurrect its focus. Directional navigation uses the caller's
current measured columns/flow and then reveals the admitted target through the
shared ancestor path. Resize alone preserves manual viewport position and may
leave the selected focused tile offscreen; navigation can bring it back.

Its typed-content projection shares GPV01. GPV02/05/06 retain their separate
full exits; this signing does not assert a native10000-record performance bound,
global RTL platform matrix or reader focus result. It is not an immutable SDK;
portable bounds/retained-child checks remain distinct from actual native input.

## Fixed and variable-height typed lists — 0.1 source contract (GPV03)

The original GPV03 fixed/variable bounded-virtualization exit is qualified as of
2026-10-21. The [source and dated native evidence (historical context)](../../docs/VALIDATION.md#historical-evidence)
includes the current public consumer after shared-source integration and the
original Main/Float/relevant-DPI focus/reveal obligation. It preserves the
original fixed and caller-measured variable scope, signatures and failure rules;
other collection exits and the complete GPV06 freeze remain separate.

`VirtualListRange::compute` validates finite row height>=1, positive finite
viewport height and finite offset. It clamps the offset, caps content height at
16,777,216 logical pixels and rejects a conservative visible-row count above256
(including its boundary row). Overscan clamps to32 per side. Its public fields
remain public: `virtual_list` is the low-level Widget-returning projector and
does not revalidate caller-supplied geometry. Use a current computed range or
preserve the same invariants when constructing one. `virtual_list_with_items`
additionally rejects count mismatch and invalid index bounds before callbacks;
it does not convert all malformed public-field values into structured errors.
Both project only the supplied bounded range plus at most one in-range offscreen
retained index, without building a visible retained item twice. Stable identities
come from current caller records, not geometry indices.

`VariableListMetrics::new` validates caller-measured logical row heights and
stores immutable prefix positions. Heights are finite and at least1 logical
pixel; total content uses the existing16,777,216-pixel limit. Rebuild the metrics
when the caller's current item order or measurements change. This is an O(n)
explicit measurement update, not automatic row measurement or an application cache.

`range` borrows that same metrics authority and uses binary searches for the
current viewport. It admits at most256 actual visible rows, clamps overscan to32
per side, clamps the offset and rejects invalid/nonfinite or overly dense
geometry through the existing `VirtualListError`. Fractional offsets at large
coordinates round inward so the final row does not disappear at the content edge.
`row_bounds` exposes the same geometry for caller-owned reveal policy.

`variable_list_with_items` checks the typed data count before calling a factory.
It constructs only the bounded range and at most one explicit retained offscreen
item; a visible retained item is not built twice. Current typed records supply
their stable IDs, actions, content and semantic labels. The fixed-height APIs and
the new entry share one private ScrollBox/absolute-row projection. Geometry
indices do not become author identities. The original fixed-height signatures,
limits and behavior remain supported.

The qualified exports are `VirtualListError`, `VirtualListIds`,
`VirtualListRange`, `virtual_list`, `virtual_list_with_items`,
`VariableListMetrics`, `VariableListRange` and `variable_list_with_items`, with
their current public fields/methods. Host measurement, selection, current
stable-ID-to-index mapping, focus eligibility, scroll/reveal and lifecycle remain
caller-owned. No automatic measurement/cache, new collection-family variable
API, native factory/CPU/memory performance number or broad reader claim follows.
This is the complete original GPV03 source contract, not an immutable SDK
publication or completion of the other GPV01–06 exits. [Implementation](src/virtual_list.rs),
[measured geometry](src/virtual_list/variable.rs).

## Affected consumers — ADR-0465 source qualification

The numeric/property interaction consumer and the ordinary/lazy typed collection
consumer exercise the current public composition boundary. Standalone supplies
the current Number/Vector/Swatch/Ramp, selection, nested scrolling and Table
adapters. Workspace's `SplitResizeSession` delegates its target/terminal policy
to `ValueEditSession`; the existing Standalone dock regression exercises that
actual caller. Independent Developer Agent Desktop imports shared scrolling and
fixed-list types for its existing panels. Its three targeted library regressions
cover scrollbar capture/projection, changed-file popover and bounded trajectory.
The existing native_starter/native_interaction examples are compiler-checked;
that does not run native input or GPU presentation. Studio's local same-named
`ScrollbarDrag` is not an import of the Widgets type.

The [consumer evidence (historical context)](../../docs/VALIDATION.md#historical-evidence)
binds those current source scopes and records the initial concurrent source drift
separately from the stable targeted recheck. These checks complete the named
consumer/source reconciliation. The complete numeric and collection reviews
above bind the later actual445 numeric and427 ordinary/lazy consumers, current
129 shared sources,583 Host39 and dated Main/Float/narrow/DPI/IME records. The
three original Desktop consumer modules/tests remain exact to actualr49; this
does not requalify the whole current Agent application. All original GPN/GPS/GPV
exits are complete in the ledger. Earlier basic/text/focus/menu freezes retain their
dated scope. No immutable SDK publication, Host feature expansion or universal
performance/reader claim follows.

Original constructors remain. New tone/loading, radio-group and finite progress
entries are additive pre-freeze APIs. `ThemePalette` gained progress_fill and
tooltip surface/text/border fields: custom struct literals must supply them, while the stock
theme constructors supply defaults. No new public caption-color field is added.

The basic, menu and focus source contracts are frozen against the ledger's dated source/artifact
evidence. An incompatible supported0.1 change needs an explicit0.2 migration;
compatible fixes need explicit patch adoption. This
source inventory does not publish an immutable SDK, certify every exported
widget or imply completion of the original platform/52-family gates.

## Controlled tree interactions — ADR-0657

`TreeViewController<'a, K>` borrows the existing `TreeExpansionState<K>` and
`ListSelectionState<K>` against immutable host preorder entries. `new` validates
the complete hierarchy before pruning deleted identities; malformed input leaves
both states untouched. `visible`, `selection` and `expansion` expose projections;
`select`, `key`, `set_expanded`, `set_all_expanded` and `reveal` apply view intents.
Hidden descendants retain selection. Ranges use visible order, Control navigation
does not select, Control+Space toggles, and Alt/Super chords defer to the host.
An absent/hidden row cannot select or disclose through a stale action. `reveal`
opens only ancestors without selecting the target or opening unrelated branches.

`TreeViewUpdate<K>` returns stable `focus` and state `changed`; it never changes
platform focus. `tree_focus_after_change` validates both source revisions and
repairs one calling view to a visible ancestor or nearest surviving identity;
empty or unknown prior focus returns none. Source replacement and per-window
focus admission, scroll/reveal, author permissions and lifetime remain host-owned.
Use the existing `tree_view_windowed` with an optional focused-row index to keep
factories bounded. Validation and visible-order preparation still inspect the
complete caller source; this is not constant-time large-tree preparation.
All prior tree APIs remain available. Qualification is scoped
by the [tree ledger (historical context)](../../docs/VALIDATION.md#historical-evidence); no physical
Reader, drag/drop or immutable SDK publication follows from these APIs.

Historical context links explain the original evidence scope; see the independent validation and release procedure.
