# GPUI Core supported source contracts — 0.1

## Additive process-local trace — 2026-10-30

`trace::{TraceCapture, TraceConfig, TraceSnapshot, TraceStats, TraceSpan, TraceError}`
and `trace::{frame, span, instant, counter, is_recording}` provide explicit bounded
wall-time capture. Normal shared stages inherit current parent/frame automatically.
`TraceDetail` and `TraceCapture::start_with_detail` select stage or detailed
capture; high-rate measurement/shaping scopes remain available in detailed mode.
`TraceSnapshot::frames()` returns typed `FrameProfile`, `FrameCall`, `FrameTimer`
and `FrameMarker` analysis; `to_html_report()` exports an offline frame picker and
call tree from the same data as the JSON v2 `gpui_frames` extension.
Hosts own controls, business scopes and output I/O. Capture-local thread lanes,
loss/unfinished reporting and Chrome Trace Event JSON are documented in
[Tracing](../../docs/TRACING.md). Disabled hooks take no clock reading or lock;
no event stores input text, element labels, window titles or automatic files.
This is not CPU utilization, stack sampling or GPU execution time. Native latency
and consuming-product deployment are separate qualification.

Status: **0.1 identity, typed Action and dependency-isolation contract frozen on
2026-10-14**. The existing layout contract is also frozen on2026-10-14 after
GPL01–05 qualification. Both are recorded in the repository's GPUI
Core/layout/basic-control acceptance ledger. This is a source contract baseline;
immutable SDK publication and the remaining widget/platform gates are separate.

The existing focus, routing and interaction-ownership source contract is frozen
on2026-10-19 after the original GPF01–05 exits in
[the completion ledger (historical context)](../../docs/VALIDATION.md#historical-evidence). This includes
the qualified Core APIs below and their existing Widgets/winit adapters; it does
not publish an immutable SDK or close unrelated platform/Workspace gates.

## Identity and ownership

ADR-0465 adds `InputOwnership::pending_focus_target(window)` as a read-only,
source-qualified projection hint for bounded collections. It reports a matching
staged Target without consuming it or changing focus/capture; traversal and
foreign-source requests return None. Callers may construct the current stable
item before `FocusScope::resolve_request` validates its rebuilt enabled/modal
scope. A hint never admits focus or replaces request consumption/validation.
Existing APIs and behavior remain; this additive qualification does not publish
an immutable SDK or complete the collection/native gates.

`ElementId`, `ElementKey`, `ElementIdAllocator`, `ElementScope`, `UiWindowId` and
`InputTarget` identify caller-owned UI elements and input sources. Hosts retain
the scope and choose stable local IDs. Rebuild, reorder, label translation and
viewport changes preserve a surviving owner's identity. A removed owner must
not be replaced by a different owner under the same retained identity.

IDs and keys are unique within each complete element tree. Independent window
trees may use the same local key; scopes provide non-overlapping process-wide
ElementIds. `ElementIdAllocator` handles share one monotonic namespace and return
`ElementScopeExhausted` instead of wrapping. Scope IDs are process-local, not
document persistence IDs. Explicit `ElementId::new` callers own collision checks.

Layout and `ActionRoute::build` reject duplicate IDs and duplicate keys through
the same identity validator. No derived route is returned on failure.
`ActionRouteError` distinguishes duplicate element, duplicate key, duplicate
action and unknown action target. The rejected identity is carried in the error.

## Typed dispatch

### Read-only inspection (additive, ADR-0420, 2026-10-15)

`UiSnapshot::capture(UiSnapshotFrame, UiQueryLimits, fields, sensitive)` derives an
owned observation from one Host-committed Element/LayoutTree/ActionRoute bundle.
The Host supplies application lifetime/window/revision, current focus/modal range
and borrowed editor content; it must supply those from the same presentation.
The projection checks tree/route parent identities and layout membership. It is
not a second UI model, OS tree reader or dispatcher, and performs no I/O.

Capture is bounded to1–4096 realized nodes,0–65536 text bytes total,1–128 levels
and4096 bytes per textual field. UTF-8 boundaries are retained. Limit flags
explicitly report incomplete capture; completeness only concerns the supplied
realized tree, not virtual/unloaded model content. Numeric/layout information is
copied; text/value/name/description/tooltip channels are budgeted. Password-role,
password-state and Host-marked sensitive nodes redact all textual channels,
numeric values and selection positions, including descendant text.

`TextInputState::inspection_selection` reports committed UTF-8 anchor/focus byte
positions; password and active-composition positions are unavailable. Host
editor observations use committed text and report composition separately.
`None` value/selection means unavailable or not supplied, not empty text or an
invented collapsed selection.

`UiSnapshot::query(UiQuery)` pages up to256 nodes in deterministic tree order,
optionally under a captured root. Cursor anchors are ElementIds tied to the exact
snapshot identity. Foreign lifetime/window, stale revision, missing roots and
cursors outside the requested subtree produce typed `UiQueryError` values.

An action owner is a reference to a currently routed typed Action, not a public
command ID. `HostUnvalidated` explicitly means business admission is unknown;
disabled/modal/offscreen/no-action conditions are distinct. This interface
grants no invocation, file, domain or author capability. Existing frozen
identity/layout/dispatch qualifications do not automatically qualify this new
additive interface or a new SDK publication.

`MenuItem::icon(resource)` optionally describes a host-resolved leading image;
it carries no resource I/O or execution behavior. Labels and typed actions stay
independent of decoration. The Widgets menu projection retains the slot identity
and existing checkmark semantics (ADR-0341, 2026-10-15 additive menu metadata).

`ActionRoute<A>` uses caller-provided typed actions, the current tree's ancestry
and stable ElementIds. A nested action wins over its parent's action. A text
input owns selection/editing even if it has no click action. Disabled subtree
boundaries prevent descendant and ancestor activation. Unknown or removed hits
do not bubble into a current action. Capture/target/bubble propagation is ordered
and stops when a handler consumes the event.

`PointerClick` activates on matching current action owner, source window and
button at press/release. A mismatched or missing owner rejects the release.
Hosts cancel it when their source becomes unavailable; a restored element does
not resurrect a previously cancelled click. Long drag/edit sessions use their
separate explicit owner/cancellation contracts.
`PointerClick::retain_live(window, predicate)` is an additive reconciliation
entry for model availability changes before the next route rebuild. It inspects
only that window's pending stable action owner and returns whether it cancelled
the press; foreign windows do not invoke the predicate. A later restore cannot
revive it. The host supplies model availability and still owns ordered cleanup.
Existing press/release matching is unchanged. This additive reconciliation entry
is included in the2026-10-19 ADR-0419 focus source-contract freeze.

`FocusScope` validates window/modal membership against the current route.
`InputOwnership` retains transient focus/capture and deferred focus requests.
`request_focus_traversal(window, backwards)` stages one keyboard step until
`FocusScope::resolve_request` sees the rebuilt tree. It uses the same live,
modal-restricted order as immediate traversal, with the saved stable anchor;
a missing anchor starts at the first/last live stop. Foreign windows reject the
request. New focus, pointer gestures, replacement requests and window cleanup
supersede it. Hosts apply the resolved target through their normal text/IME and
reveal adapters before publishing. This additive navigation contract in ADR-0419
does not change the frozen identity/Action/layout behavior or exact-target restore.
Its qualified focus source contract is recorded by that ADR and the ledger.
Reconciliation drops removed or disabled owners; foreign windows cannot consume
their captures. Hosts end previews/text sessions in the documented cleanup order
before publishing replacement state. These APIs do not themselves generate OS
events, implement platform capture, store author data or provide a Graph runtime.

The2026-10-19 source audit distinguishes `FocusScope::contains` (live window/modal
membership, including non-tab pointer owners) from `accepts` (also a current tab
stop). A reordered tree retains the same stable focus and valid capture; deferred
traversal uses the rebuilt order rather than a saved index. An invalid or foreign
deferred request is consumed once and rejected. Reconciliation reports a lost
capture even when losing focus already cleared it. Hosts apply the existing
`finish_window_interaction` stages in order: CancelPreviews, EndTextSession,
DismissTransient, ClearOwnership. The candidate audit and its dated tests are
retained;322/323 physical held-key transfer and325/326 physical Escape during
active preview/capture close the remaining GPF01/03 native exits. All eight public
source files and unchanged affected Host adapters are rebound at final freeze;
the private scrollbar repair keeps its separate309/324 qualification.

The existing winit `is_user_key_press` admits Pressed and non-synthetic from the
same event. Focus-generated synthetic replay must not invoke an action again;
ordinary repeat policy remains Host-owned. Host value-edit adapters cancel the
captured preview on Escape or source loss before accepting a late release.
Native evidence binds its actual source, artifact and environment; it does not
imply OS input generation or general platform capture from these Core APIs.

## Menu descriptors — 0.1 source contract

The `MenuId`, `MenuItemId`, `MenuItem`, `Menu` and `MenuRegistry` source contract
is frozen on2026-10-18 after GPM01–05 qualification in
[the completion ledger (historical context)](../../docs/VALIDATION.md#historical-evidence). Stable IDs
remain independent of labels, decoration and item order. Registry replacement
validates the entire replacement before changing its live items; duplicate menu
or item identities return the existing structured error.

Original constructors and typed actions remain. Disabled rows, separators and
section headings are inert. Checks, shortcuts, optional icons and submenu links
are descriptor data; the host owns availability, resource resolution and action
application. `tooltip=None` supplies the full source label to presentation;
an explicit override, including an empty string, is preserved. Widgets apply the
existing visible-text suppression policy rather than changing the source label.
This menu freeze leaves text editing, general input ownership, platform reader
qualification and immutable SDK adoption under their existing gates.

## Dependency and service boundary

The Core library has no render device, event loop, clipboard, filesystem/network
service, product model, Graph/domain, engine or Workspace dependency. Text shaping
uses host-supplied fonts through the existing Parley context; system font discovery
is disabled. Paint, hit geometry and accessibility are derived projections.
The normal dependency closure and existing boundary regressions are checked in
the repository; platform adapters/renderers remain separate crates.

## Layout and measurement

`Style`, `Dimension`, `GridTrack`, `GridCell`, `Axis`, `FlexWrap`, `Align`,
`Justify`, `FlowDirection`, `PositionMode`, clips and explicit child slots feed
one shared Taffy-backed layout path. Supported behavior includes Box sizing,
Row/Column Flex growth/shrink/wrap, Grid tracks and cell spans, absolute overlays,
insets, authored minimum/maximum sizes, constrained aspect ratios, inherited RTL
flow and shaped first-baseline alignment. Authored fixed height/max-height stays
authoritative; wrapping does not silently enlarge it.

`LayoutEngine`, `RetainedLayoutEngine`, `ContentConstraint`, `DesiredSize`,
`TextLayoutRequest`, `TextLayoutMetrics`, `LayoutTree` and `LayoutNode` are derived
layout/measurement APIs. Outputs are in logical viewport coordinates and retain
stable element identities in depth-first order. Paint, hit testing and scroll
geometry consume these same rectangles and clips. An explicitly unclipped child
contributes its visible extent; reserved scroll gutters reflow content, and
scroll offsets affect the derived projection rather than author data.

Hosts use `compute_with_text_context` for the same font shaping and baselines as
paint, or provide their own metrics through the existing measurement entries.
`compute`/`text_intrinsic_size` retain their deterministic fallback estimation
contract; they do not discover system fonts. Wrapped bounded text is measured
conservatively before logical-edge rounding, while natural auto width reserves
its upward-rounded extent. Percentage caps resolve against a definite parent
during cross measurement. These rules prevent extra unmeasured lines and
unreachable scroll tails without changing authored dimensions.

Retained and stateless layout agree for the same current tree, viewport and
metric source. Retained state invalidates on relevant style/content/child/clip,
viewport, text-context or registered-font revision changes; removed owners are
retired and `clear` releases the derived tree. A window owns its retained engine.
Invalid viewport/constraint, duplicate identities, invalid aspect/baseline/text
metrics and solver errors retain their existing typed `LayoutError` variants.
No usable layout is published on a rejected computation.

Qualification includes the existing portable layout/measurement/retained/flow/
paint-hit regressions, the complete host-font constrained mixed-row regression,
and actual Windows Main/Float narrow-tail/wide restoration at100/125/150/200%.
The acceptance ledger binds their source and native artifact identities and
preserves earlier failures. This qualifies the existing layout capability; it
does not complete every OS interaction, text-editing, widget or renderer gate.

## Explicit image fitting (ADR-0578)

`Element::fitted_image` and `ElementKind::FittedImage` add explicit
`ImagePresentation`: `ImageFit::{Stretch, Contain, Cover}` and independent
physical `ImageAlign::{Min, Center, Max}` anchors. `PaintCommand::FittedImage`
preserves that descriptor, stable source token, element identity and current clip.
Backend exhaustive matches must handle this additive enum variant. Existing
`Element::image`/`ElementKind::Image`/`PaintCommand::Image` constructors and fields
remain: Native's legacy path stretches, while Studio's legacy path contains at
center. New explicit presentation has the same meaning in both image backends.

`ImagePresentation::place` accepts the authored logical rectangle and the original
source aspect as `Size`, returning `ImagePlacement` bounds and normalized UV.
Contain keeps the whole source with possible empty space; Cover fills the box by
cropping the source. `clipped` crops geometry and UV together without stretching;
`texture_uv` exposes top-left-origin corners. Empty/nonfinite/unrepresentable
geometry returns `None`. Arithmetic uses f64 intermediates. Anchors and texels
are physical; inherited RTL may move the layout box but does not flip the image.

Fitting does not determine intrinsic layout size, alter authored dimensions,
redefine the hit/semantic box or choose loading policy. Callers supply existing
semantics for an accessible name. Pixels, original aspect, source identity,
loading/retry and redraw remain with the host. Native retains its structured
image diagnostics, limits and per-renderer pixel cache. Studio retains its
existing host source cache and original aspect for downsampled textures. Browser
still returns `UnsupportedImage` because it has no image resolver. This addition
does not imply general media loading or whole Icon/Image family completion.

## Text shaping and editing — 0.1 source contract

The existing text shaping/editing contract described here is frozen on2026-10-19
after the original ADR-0419/GPT01–06 exits and affected-consumer reconciliation.
The [completion ledger (historical context)](../../docs/VALIDATION.md#historical-evidence) preserves the
dated supplied-font/grapheme/bidi/reflow, native IME/clipboard/Undo, Main/Float
and DPI observations, current platform actions and independent Windows focus
properties. Original constructors, host-owned fonts/state/I/O and documented
limits remain. This source freeze does not publish/adopt an immutable SDK or
complete general focus/capture/replay, raw platform TextRange, reader, native
media, latency or other existing release gates.

`TextInputContext` receives host font bytes and has no system-font discovery or
clipboard. `TextInputState` owns one transient editing buffer/history, not an
author document. Original single-line, multiline, password and read-only
constructors remain. Owner `reset` replaces the source and clears composition
and history; user Undo does not span an owner reset. Single-line edits normalize
CRLF/CR/LF/tab to spaces; multiline edits normalize hard breaks to LF and tabs
to spaces. Read-only source retains its original bytes and rejects content edits.

Ordinary input is bounded by `TEXT_INPUT_MAX_BYTES` (16KiB); password callers
choose a limit clamped to1..16KiB. Oversized input returns `TooLong` before
replacing the current source/selection/history. Undo/Redo share a bounded history
of32 snapshots and256KiB of source bytes. Typing and same-kind deletion coalesce
until `end_group` or navigation; paste and IME commit are separate edits.

Transient host references use `TextInputMark` UTF-8 ranges in that same buffer.
`insert_mark` inserts a bounded visible label with an opaque host ID; ordinary
typing never creates a reference. Editing moves ranges and removes overlapped
references atomically; navigation treats the label as a unit. Undo/Redo restore
the same IDs/ranges, IME preedit retains the committed ranges, and owner reset
clears them. `retained_mark_ids` includes bounded history so hosts can retain
referenced resources; `remove_mark` preserves the current cursor and
`forget_marks` retires invalid IDs from history. `clear_history` begins a new
host edit boundary without resetting committed text/references. No resource decoder, image,
provider or durable message identity belongs to the shared editor. Agent's
positioned image input is the current consumer; native image/caret qualification
is separate from the text/Undo contract tests.

Keyboard and pointer navigation use Parley positions and preserve valid visual
affinity; interior extended-grapheme endpoints are corrected before ordinary
insertion/deletion. This does not redefine explicit platform text ranges:
`TextInputSelection` is a transient range resolved through the current text-run
projection, checked again after the host's focus transition. Default password
snapshots are masked; explicit `revealed_snapshot` remains host-controlled.
Password state rejects platform text selection; read-only selection additionally
requires grapheme boundaries. This is not a raw platform TextRange guarantee.

Direct single-projection consumers retain `accessibility` and the changed-only
`select_accessibility` compatibility entry. The latter returns false for a valid
unchanged range as well as for a rejected range; it must not decide whether the
host acquires focus. `resolve_accessibility_selection` returns the existing
transient `TextInputSelection` for an accepted range, including an unchanged one,
without changing selection. Resolve before the host's focus/field transition,
then call `apply_accessibility_selection` to revalidate current offsets; its
`Some(false)` means accepted without a selection change, while `None` rejects.
Foreign run IDs, password/composing buffers and invalid read-only boundaries
cannot acquire focus through this path. Multi-window consumers use their owning
`AccessibilityTree::resolve_text_input_selection`, which also validates the
current live/enabled owner and that window's projection. The direct projection
keeps the existing IME-hidden-caret publication. The2026-10-19 repair/consumer
and separate native-focus qualifications are in the completion ledger; raw-reader
and platform TextRange gates retain their independent scope. Portable checks
alone do not constitute native focus qualification.

Semantic read-only Slider/SpinButton nodes retain their numeric values and focus
action but do not advertise SetValue. A window tree releases a text-run projection
when its element is no longer published. Reappearance preserves the element's
platform owner identity and creates fresh text runs; a range from before removal
is rejected. Continuously published text runs keep their existing identity. This
retirement rule leaves direct single-projection compatibility and other windows'
maps independent (ADR-0293 maintenance,2026-10-23).

`FocusScope::accessible_target` also requires that the current window projection
actually publishes the requested Focus/Click/SetValue/SetTextSelection action.
Text and document enrichment refresh this derived permission; merely being a
live tab stop cannot authorize an unsupported action. Existing window, enabled,
modal and non-tab-stop Click checks still apply, and unknown actions are rejected.
Writable password nodes advertise their existing SetValue replacement capability
while retaining only their masked value and no SetTextSelection/text-run exposure.

`preedit` retains the original source/selection, publishes composition geometry
in its snapshot, and clamps cursor offsets to UTF-8 character boundaries.
`text()` keeps returning the original source until commit. Cancel restores the
prior editor; `commit_ime` is one undoable edit. Ordinary navigation/point/edit
operations do not take ownership from an active composition. The host supplies
clipboard I/O and uses the winit bridge's Submit/Cancel/TraverseFocus response
through its own typed focus/document adapter.

Snapshot glyphs, selection, caret and composition use one shaped layout. Hosts
derive wrap width with `text_input_wrap_width`, update it on constrained reflow,
and use `TextInputViewport::to_layout` for clipped/scrolled pointer coordinates.
Decorated single-line editors may opt into snapshot `with_horizontal_insets`.
Its optional policy moves text, selection, caret, IME and pointer coordinates
together; default snapshots retain their original automatic padding. Negative
or non-finite insets become zero and excessive reservations are bounded by the
actual field width, preserving a usable text region. Hosts must use the same
snapshot policy for painting, pointer mapping and native IME placement.
`TextInputViews` rejects duplicate element/field mappings. A host must not retain
byte offsets or cursor geometry across a source/layout replacement as stable
author identity. `WindowTextSession`/`TextSessionTransition` distinguish a field
change from a same-field window transfer: the latter ends the old IME session
without resetting the shared draft. The winit host registers/removes each window's
bridge, clears its IME session on ownership loss, rejects stale commits until a
new Enabled event, and republishes the owning caret/DPI request as needed.

The qualified winit `TextInputBridge` text pointer adapter maps a same-target primary
click pair (450ms/5window-logical pixels) to Core word selection, and Shift to
anchor extension. Hosts observe input before dispatch even when another control
consumes it, clear click timing on programmatic caret relocation, and continue to
own primary capture through `InputOwnership`. This adds no word rules or editor
state to a Host. The source regression and retained native ordinary/Arabic
mixed-text selection, affected floating/DPI and editing observations are bound
in ADR-0419's ledger. General pointer capture/replay remains under GPF01–05.

The pending winit `is_user_key_press(state, is_synthetic)` admission excludes
winit-tagged focus replay and releases from press actions, focus presentation
and click-pair invalidation. Hosts read both fields from the same KeyboardInput
event before logical-key routing; this predicate is not a filter for the whole
event stream or legitimate modifier-release cleanup. Ordinary repeats retain
their existing meaning. Standalone's shared Main/Float adapter and recovery F5
use it; source metadata regressions do not sign native held-key/focus delivery
or another Host's private keyboard adapter.

The winit companion exposes `UiInputEvent`/`UiKeyEvent` as owned public input
projections and `UiInputEventSource`/`UiKeyEventSource` adapters. Existing native
WindowEvent/KeyEvent callers remain accepted by TextInputBridge, focus visibility
and text pointer observation. `UiInputRequest` validates bounded command batches;
`UiInputObservation` binds them to the same presented tree, route, layout, owner
registry and Host context. Hosts retain authorization and dispatch through their
existing logical handler with explicit Native/Command origin. Command input is
never native platform qualification. See [the public UI access guide](../../docs/AGENT-ACCESS.md).

## Compatibility

### Semantic control and observation changes (additive, ADR-0423)

`UiCommand` represents bounded semantic intent; `preflight` validates structural
window/modal/disabled/editor requirements without dispatching. Hosts retain their
business admission and existing typed handlers. `UiOwnerRegistry` derives current
`UiTargetRef` owner tokens from realized semantics and actual typed actions;
repaints preserve tokens, while removed or changed owners invalidate them.
Validate the registry token immediately before dispatch. No private Action value
is an external command or permission grant.

`UiSnapshot::changes_since` compares observations from one lifetime/window in
stable tree order. It reports changed/added nodes and removals from the realized
tree, not author-model deletion. Incomplete structures suppress removal claims;
text truncation prevents a complete-difference claim. This is an additive source
API and does not requalify an earlier immutable SDK.

ADR-0421's additive marked-reference work introduces
`TextInputError::InvalidMark`. Existing exhaustive consumers must handle that
variant explicitly; Studio's localized status adapter has been migrated. This
does not freeze the new marked-reference APIs or complete ADR-0419's remaining
native text/focus exits. The ordinary TooLong contract still preserves the
exact pre-edit selection and history when range correction precedes rejection.

The pre-freeze change in ADR-0412 adds
`ActionRouteError::DuplicateElementKey(ElementKey)` and removes the error's `Copy`
implementation. Consumers should handle the new error, borrow an existing error,
or explicitly clone it when retaining two copies. Existing variants and typed
action behavior remain. This is a recorded pre-release change, not a silent patch
to a previously published immutable SDK.

After qualification/freeze, incompatible changes to the supported identity,
dispatch, ownership, dependency and layout contracts require a minor-version migration
with explicit consumer adoption; compatible fixes require an explicit patch
upgrade. Freezing these Core contracts does not freeze every widget API,
certify the remaining OS capture/reader or cross-monitor behavior, publish an SDK,
or complete other capabilities.

Historical context links explain the original evidence scope; see the independent validation and release procedure.
