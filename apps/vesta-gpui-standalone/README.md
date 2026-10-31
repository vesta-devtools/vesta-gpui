# Vesta GPUI Standalone

## Frame settlement probe

From the SDK root, build the standard program, then select a new report path:

```powershell
cargo build --locked -p vesta-gpui-standalone
.\target\debug\vesta-gpui-standalone.exe --probe-frame-reflow=target/reflow-inputs.html
node tools/verify/frame_settlement.mjs target/reflow-inputs.html
```

`--probe-frame-case=documents` exercises Markdown reflow; `focus` stages logical
focus in both windows and checks remembered focus separately from the single
shared text-session owner. The default is `inputs`. Each isolated hidden run
changes each window's split 64 times forward/reverse through the actual draw
entries, preserves its draft, closes normally and retires GPU owners. It performs
no OS input, preference restore/save or user-workspace persistence. It cannot be
combined with ordinary smoke, UI stdio, native-input or another trace recording.

The verifier checks complete capture, one CPU frame and renderer attempt per
step, bounded preparation and explicit budget deferrals. An optional numeric
third argument binds the report to the launched process PID. Submission markers
are CPU-side facts, not physical display or input-latency measurements.

## Gallery interactions

Collections uses the shared controlled tree module. Its existing thousand-item
tree now offers Reverse siblings, Remove selected subtrees, Restore tree,
Expand all, Collapse all and Reveal selected. Collapse preserves hidden
selection; removal reconciles deleted identities and each window's tree focus.
Control navigation moves focus, Shift extends visible ranges, Control+Space
toggles, and Left/Right navigate hierarchy. Reveal opens only the selected path.
All changes affect synthetic Gallery view data. See the
[tree interaction contract (historical context)](../../docs/VALIDATION.md#historical-evidence).

Inline label editing and the command center temporarily borrow the Search
editor. Closing either restores its complete original text, selection and
Undo/Redo history. Inline editing keeps a blank draft open for correction;
plain Enter saves, Escape cancels, and moving to another field or a modal
cancels the draft. Save/Cancel button focus stays inside the editing session.
Composition and modified editing keys remain with the text bridge. The sample
label is host data; this is not an author-document rename operation.
See [the temporary-editor contract (historical context)](../../docs/VALIDATION.md#historical-evidence).

The existing Inputs Search field uses the controlled Editable Suggestions module.
Its 128 synchronous catalog entries support multiword and Chinese filtering,
disabled rows, bounded rendering, contained wheel and scrollbar dragging.
Up/Down and Page keys navigate; Alt+Home/End choose the first/last available row.
Enter or a row click accepts one fresh candidate as an undoable text edit.
Escape closes without changing text; Tab leaves without accepting. Ordinary
Home/End, Space and composing text stay with the editor. Provider I/O stays with
the host; this sample does not create a background service. Current qualification
is recorded in [the suggestion ledger (historical context)](../../docs/VALIDATION.md#historical-evidence).

Ctrl+Shift+P opens the searchable command center in its owning window. Its 24
existing actions include display/language, documents, feedback, sample controls
and workspace reset. Availability is evaluated from live host state; the palette
and the global keymap share shortcut descriptors. Type multiple words or Chinese
keywords, use arrows/Home/End/Page keys to select, Enter or a pointer click to
execute, and Escape or the backdrop to cancel. Space stays text input; Tab keeps
the search editor focused. Long lists use bounded rows, a contained wheel and a
draggable scrollbar. Closing restores the borrowed search query and owning-window
focus. This is a host catalog over shared GPUI controls, without another action
or document runtime. See [the command-center scope (historical context)](../../docs/VALIDATION.md#historical-evidence).

Collections and Layout reuse one controlled Breadcrumb path with distinct page
IDs. A wider window shows the path; a narrower one exposes the existing inline
ancestor list. Current locations stay read-only. Ancestor navigation and Open
child focus the next usable control in the owning window; Escape closes a
focused ancestor list and returns to that window's Path ancestors entry.
The sample changes only its existing demonstration depth, with no filesystem
navigation. See [the precise acceptance scope (historical context)](../../docs/VALIDATION.md#historical-evidence).

Controls and Layout reuse one responsive ordered Toolbar sample. The window
width provides a bounded capacity; narrow windows keep a prefix plus More,
while wide windows show all four actions including a disabled one. The existing
theme/value/pin actions remain host-owned. Expanded actions retain shortcut
hints; an action or Escape closes the list and returns hidden-action focus to
that window's More entry. This is the existing inline overflow, with no new
popup runtime. See [the fixed acceptance scope (historical context)](../../docs/VALIDATION.md#historical-evidence).

Controls and Layout include the same three shared resource-icon buttons, with
distinct stable identities per page so they also compose across native windows.
Icon and Toolbar actions
project the existing Pinned state and switch the existing wide/tall image token;
a disabled action projects the same state without joining Action/Tab order.
The existing glyph buttons and Toolbar entry remain. These samples use the
normal image resolver and are omitted in the explicit image-failure smoke so its
single-source upload/reuse assertions stay isolated. See [the precise contract (historical context)](../../docs/VALIDATION.md#historical-evidence).

Layout includes the shared explicit image-fitting example: wide/tall immutable
host pixels, Stretch/Contain/Cover, physical Min/Center/Max anchors, transparency,
an ancestor-clipped sample and the original legacy image entry. Main and native
Float use the same consumer. No file/network loading or image-specific layout
engine is added; captions identify the intended fitting behavior.

The existing `--smoke --smoke-frames=7 --smoke-image-failures` fixture uses the
explicit Cover entry to exercise Loading/Missing/Failed/invalid/oversized/Ready
and texture reuse. It suppresses the normal image matrix only in that fixture.
Actual presented PNGs can be checked with
`node tools/verify/gpui_image_presentation.mjs <capture-manifest.json>`; the
manifest binds normal exit, executable, current window/frame and declared source
recipe. That checker rejects offscreen or duplicate scenes and verifies fit,
clipping and source-over pixels. See [the exact scope and evidence (historical context)](../../docs/VALIDATION.md#historical-evidence).

Changed keyboard/IME input schedules one window-qualified caret reveal. Main and
Float resolve it from their current stable layout and focused paint snapshot,
using Core text viewport geometry and the shared ancestor scroll path. Focus,
drafts and undo history remain; later manual scrolling or owner retirement
supersedes the request. This is portable-qualified behavior under ADR-0465;
remaining native/DPI/IME combinations stay in the completion ledger.

Each window retains its derived TextArea wrap constraint and restores it before
building a snapshot. A changed constraint defers publication until the next
correctly wrapped frame; alternating window widths do not keep invalidating
each other. Pointer/key/IME input and inner wheel range use the source's presented
viewport width. The editor buffer, composition and undo history remain shared.
Field transitions enable follow-caret only in the destination window; another
window's manually scrolled TextArea keeps its detached view.

Swatch and RampPoint color editors share the Hex input. A Ramp color preview
commits to its captured stable point ID as one history entry; changing selection
does not retarget it. Session-required color actions reject another source before
menu or preview cleanup, preserving active ramp capture as well as the draft.

Independent winit/native Vulkan acceptance host for the reusable UI and optional viewport crates. It does
not depend on Vesta Studio, Graph, Scene, Executor, SOP, or Agent packages.

## Object collection query

The Inputs page's Search objects query filters both the basic object List and
the sample editable Table by case-insensitive name. An empty query restores all
rows. Sorting and selection use the original stable row/cell identities. A
matching cell keeps its draft and history; an edited row removed from the view
uses the existing source-loss cancellation policy for unaccepted text.

## Run

### Physical capture cancellation fixture

`--native-capture-fixture=main` or `--native-capture-fixture=float` prepares
disposable existing Gallery pages and one native floating window. Click a
different value on the source window's slider using real native input. After
the existing handler establishes its preview/capture, the fixture asks the
other owned window to take focus. It requires actual source blur, exact value
rollback, cleared capture and a later native release without a commit; stderr
records the delivered window and event order. It never injects input or builds
synthetic WindowEvents. A completed sequence exits normally after one second;
incomplete/reordered sequences fail, including normal close before completion,
with a five-minute upper bound. User preferences/layout are neither loaded nor
saved in this mode. Smoke and explicit mode flags are incompatible. This
qualifies only the observed boundary, separately from IME/DPI/presentation.

The `main-remove`, `float-remove`, `main-reset` and `float-reset` cases use a
real Tab drag instead of the slider. Native motion must start the existing
drag before the fixture invokes the normal CloseTab or WorkspaceReset action.
It requires gesture/preview cancellation, removal (or Reset reuse) of the
original ItemId, and a later native release that preserves the exact
post-mutation Workspace. Removed floating windows use ordinary synchronization.
Logs record the actual release window; source-window release does not qualify
foreign delivery. Missing motion/release and interrupted sequences fail.
`main-pane-remove` first prepares a single-tab source pane and checks that
removing its last Tab removes that Pane. `float-restore` exercises the existing
Close/RestoreFloating path, retaining ItemId while removing the native surface.
After both windows present, the fixture positions its Float inside Main once;
ordinary startup placement and normal Moved-event handling are unchanged.

The `main-key-replay` and `float-key-replay` cases use an actual native T key
press on the source window with no text editor focused. After the ordinary
shortcut toggles the shared theme once, the fixture requests focus on the other
owned window. Acceptance requires actual source blur, destination focus, a
destination `is_synthetic=true` Pressed event, an ordinary release and no second
theme action. The key observer logs only T metadata. If the helper releases too early for
Windows to generate a pressed replay, the case stays incomplete; it must not be
reported as a held-key pass. Post-release waiting is bounded to one second.
The existing five-minute upper bound and disabled persistence still apply.

`main-hide-number` and `float-hide-number` prepare the existing Inputs page for
a real Number scrub. A native press must establish the current window's editor;
a later native move must cross its existing4px threshold and change the preview.
Only then does the fixture invoke the normal Inputs-section visibility action.
Ordinary Host retirement must restore the initial value and move focus to the
section header before a later real release can preserve that state. The probe
never injects input, directly edits/cancels the model or rebuilds routes. Logs
identify the source and actual release window; a same-window release does not
qualify foreign delivery. Plain focus clicks do not complete the case. Success
allows30 seconds for inspection; the original five-minute deadline and failures
for incomplete cases remain. These opt-in cases do not change ordinary startup
or the existing observer/slider/replay cases and do not qualify IME/DPI themselves.

`main-narrow` prepares the same capture fixture with an actual320×422 native
Float through the existing WindowPlacement. This is explicit geometry setup;
it does not qualify a physical resize gesture. Existing Workspace split/restore
and native keyboard input can then qualify constrained editable text and wider
recovery without relying on an unobserved edge-drag result.

All capture-fixture cases also record native IME event metadata after normal
dispatch: phase, UTF-8 byte count, optional cursor range, source/session/focus
identities and the editor's composing flag. The trace excludes preedit and
committed text and is disabled on ordinary startup. Command input does not
enter this observer. A candidate window alone does not qualify inline preedit;
use the received event trace to distinguish missing delivery from lost state.

`--native-capture-fixture=observe` prepares the same nonpersistent Main/Float
environment for native text/focus/menu observation without a slider or replay
signature. It records the same actually received IME metadata and never requests
focus transfer after a slider press. Normal Close or the five-minute deadline
ends observation; stderr explicitly says `acceptance=unasserted`. Exit0 alone
does not qualify any input, IME, focus, capture or presentation behavior. The
other cases keep their original required signatures and incomplete-run failures.

`observe-narrow` combines this same unasserted observation mode with the existing
actual320×422 Float preparation. It changes only initial window geometry;
actual OS DPI must still be recorded independently. It neither adds an input
path nor changes any capture/replay completion requirement.

Observe mode also coalesces the existing bridge's Debug metadata after native
dispatch: requested IME allowed/cursor state, received enabled state and owning
field/window. It logs named control-key destination before dispatch so a window
closed by Enter retains its source identity. Neither trace logs editor text or
raw character keys, injects input or qualifies preedit merely from a request.

### Read-only UI query fixture

`--ui-query-stdio` opts into the additive ADR-0420 inspection API. The ordinary
launch does not capture snapshots or start the query worker. This local JSONL
fixture accepts `windows`, `snapshot`, `changes` and `wait`; it cannot invoke UI/product actions.
It is not an MCP server or a general remote transport.

```powershell
cargo build --locked -p vesta-gpui-standalone
.\target\debug\vesta-gpui-standalone.exe --ui-query-stdio
```

Send one UTF-8 JSON object per line on stdin; stdout contains only versioned JSON
replies and human diagnostics stay on stderr:

```json
{"schema_version":1,"request_id":"q1","op":"windows"}
```

The response discovers the process `instance`, participating window IDs and
whether each has a successful presentation. Use the returned decimal strings:

```json
{"schema_version":1,"request_id":"q2","op":"snapshot","instance":"<returned instance>","window":"1","limit":32}
```

Snapshots represent the last successfully presented native frame, including
floating windows, independent of AccessKit activation. A reply includes logical
viewport/scale, focus/modal identity, semantic state, bounds/clipping, bounded
text and committed editor selection. Passwords remain redacted even when the
password widget is visually revealed. Routed actions report Host admission as
unvalidated and `invokable:false`; no private DemoAction payload is serialized.
Coverage is explicitly `realized_tree_only`: virtual rows/unloaded data and
unsupplied Markdown selection are not claimed to be complete.

For the next page, repeat the returned `revision` and pass `next_after` as
`after`; optionally keep a fixed `root` to page a captured subtree. Limit is 1–256.
IDs are strings to preserve 64/128-bit identity. Queried revisions are pinned
independently of repaints, with eight pinned snapshots total and sixteen latest
window snapshots. Evicted revisions return `stale_snapshot`; closed windows
return `window_not_found`, and a foreign process instance is rejected. `not_ready`
means a window has not yet published a usable frame. Unknown/mutating requests,
unknown fields, unpinned cursors and excessive page limits are rejected.

The single diagnostic I/O worker handles at most one outstanding request, with
an 8 KiB input-line cap, 128-byte request ID and 10-second Host reply deadline.
Serialization/stdout never run on the UI thread. EOF ends the worker, not the app.
An oversized line returns `request_too_large` and ends the worker. There is no
mutation or program-quit query.

Run the actual two-window fixture against the default build:

```powershell
node tools/verify/gpui_ui_query.mjs
```

It owns a finite native smoke process and records executable hashes, requests,
responses and exit evidence under `target/research/gpui-ui-query`. This verifies
the read-only protocol/presentation boundary, not physical input or visual/DPI
qualification. Existing UI acceptance cursors remain unchanged.

### Semantic control and render capture

`--ui-control-stdio` enables the local command/capture channel as well as queries.
It supports current-owner command discovery, source-checked editor changes,
Undo/Redo, numeric values, focus/reveal/activation, and on-demand PNG capture by
window, element or rectangle. A supplied viewport selects offscreen rendering of
the same immutable realized tree. GPU completion, mapping and PNG encoding do
not block the normal UI thread. Password regions are rejected in pixels too.

Commands require the owner token returned by `commands`; repaints retain it,
while changed actions/semantics or removed targets invalidate it. The last 128
mutation request receipts are cached; retries within that documented window must
reuse the exact request ID and body. Query input is capped at 8 KiB and control
input at 256 KiB; every request still has a bounded reply deadline.

The complete protocol, boundaries and Agent product verbs are documented in
[GPUI UI access](../../docs/AGENT-ACCESS.md). Validate the actual native programs:

```powershell
node tools/verify/gpui_ui_query.mjs
node tools/verify/gpui_ui_control.mjs
node tools/verify/gpui_ui_control.mjs --agent
```

### Interactive Gallery

```powershell
cargo run -p vesta-gpui-standalone -- --mode=2d
cargo run -p vesta-gpui-standalone -- --mode=3d
```

The 2D mode opens with an app-style dockable workspace and multiple tabs. The title bar initially
shows the application menu icon and workspace title; clicking the icon expands Vesta/File/View/Window
and opens the application menu. Hover or Left/Right switches top-level menus (Right opens an
available submenu first); Escape, an outside click, or a command closes the menu bar. Context menus
remain independent. The 2D/3D switch stays centered on the window in both menu states. Below 800
logical pixels, compact labels keep all window controls reachable; theme and language actions remain
in the Vesta menu. The gallery has no product project-history model.
Tabs can be dragged between panes; the toolbar and Window menu expose Move, Split, Float, and
Reset. The generic dock projection accepts host-owned content for each active panel. The rest of
the widget gallery remains below the workspace and can be scrolled into view.

On Windows, the flat custom title bar supports dragging from its empty area, edge/corner resize,
double-click maximize/restore, and Minimize/Maximize/Close buttons. Other platforms retain native
window decorations. `T` toggles the Dark/Light theme, `1`/`2` select a mode, and `F` resets the 3D
camera.
In 3D mode, left-drag an empty viewport area to orbit, middle-drag to pan, use the mouse wheel to
dolly, and press `O` to switch perspective/orthographic projection.

For non-interactive GPU/shader startup checks:

```powershell
cargo run -p vesta-gpui-standalone -- --mode=2d --smoke
cargo run -p vesta-gpui-standalone -- --mode=3d --smoke
```

Native Vulkan is the sole renderer for main and floating windows; `--renderer` selection and the optional `native-vulkan` feature have been removed. `--no-default-features` still builds the native host. Vulkan uses a single-sample 2D pass and lazily enables depth and 4× MSAA for 3D when supported, resolving after the UI overlay. Presentation prefers Immediate, then Mailbox, then FIFO. `--smoke-frames=600` measures steady UI submission intervals; `--smoke-mode-cycle --smoke-resize` checks resource transitions. Rounded controls also use analytic
edge coverage. The default Dark/Light palettes use a low-contrast, flat editor style; apps can
override semantic theme tokens.

For bounded native CPU profiling of the existing application:

```powershell
cargo run --locked -p vesta-gpui-standalone -- --mode=2d --smoke --smoke-frames=120 --profile-ui
```

`--profile-ui` is off by default. Each window/mode/focus/activity context excludes
24 completed warmup frames and retains at most its first256 subsequent host-total
and optional nested renderer-total CPU durations; at most64 contexts are stored.
Exit output records the raw nanoseconds and nearest-rank p50/p95, separately from
the existing means/maxima over all completed post-warmup frames. Missing renderer
snapshots remain `null`; the renderer total is included in the host total. The
`ui_profile` distribution fields identify the bounded sample scope. Record actual
source/executable/lock, hardware/driver and run/lifecycle when using these values;
they do not measure GPU time, input latency, a universal FPS or physical input.
Old aggregate-only logs have no recoverable percentile distribution.

`--profile-gpu` independently opts into actual Vulkan command timestamps
(ADR-0577); `--profile-ui` alone still collects CPU observations only. GPU
profiling emits bounded JSON lines prefixed by `ui_gpu_capabilities`,
`ui_gpu_profile`, `ui_gpu_unresolved` and a final `ui_gpu_summary`. Original
window/mode/focus/activity/size/DPI and render outcome follow the exact
generation/submission key, including delayed results and failed/deferred
presentation attempts. Keys, raw ticks and nanoseconds are decimal strings.
The device interval brackets uploads and scene/UI commands, including dependency
and scheduling stalls; it is not isolated GPU busy time, physical presentation
or input-to-display latency. Query creation/writes have their own measurement
cost. No telemetry timer, extra redraw or completion wait is added.

At most64 streams,512 pending contexts and4096 observation lines are retained;
summary fields disclose context evictions, stream drops, omitted lines, unmatched
results and unresolved attempts. Missing/unsupported/ambiguous timing remains
unknown. Old streams survive renderer replacement and the existing shutdown
retirement path so its last submission can be drained without another redraw.
Actual source, standard executable, raw results and normal retirement still need
qualification before using GPU logs as performance evidence.

The same opt-in flag emits `ui_native_window_geometry` at main-window creation,
resize and scale-factor change, with its stable UI window ID, actual winit scale
factor, physical client size and derived logical size. These event observations
are separate from frame CPU samples. Disabled profiling performs no additional
geometry queries or output. Record them with actual input/geometry observations
when qualifying a scaling case; a metadata line alone does not pass DPI input,
cross-monitor behavior or accessibility.

The opt-in `--profile-ui` mode also exposes a `profiling` member in the existing
read-only `window_state` reply: event-loop batch/timer/cancelled-wait counts,
per-window native redraw deliveries and successful present calls, and the live
caret/tooltip/notice/Busy state. Queries themselves wake the event loop; distinguish
these cancelled waits from scheduled timer wakeups and process CPU counters.
Delivered native redraws are not a count of every coalesced redraw request or
OS context switch. A final `ui_lifecycle_exit` retains the bounded counters.

`ui_native_input_received` and `ui_native_input_presented` record only native
pointer press/release, wheel and non-synthetic key presses. Text/key contents are
not logged. A pre-build watermark binds each input to the first successful
present-return marker of its own window; deferred/failed frames, foreign windows
and input received after that build cannot satisfy it. The measured monotonic
interval is Host native receipt to present call return, including diagnostic
overhead; it does not measure OS pre-dispatch, GPU completion or physical display.
Stdio command-origin input cannot produce these native timing samples. There
are at most64 window counters/64 pending inputs and256 logs of each input kind;
omitted/evicted/unresolved observations are explicit, and no wakeup is added.

## Current status

Standalone is the source family's demonstration and integration host. Its current
contracts are exercised by the independent automated suite; historical product
acceptance remains described in [validation scope](../../docs/VALIDATION.md).
This release does not reproduce every old physical-input, screen-reader or DPI
measurement.

The gallery covers shared controls, Markdown/text, layout, scrolling, collections
and Workspace/Dock interactions. Native windows use the same stable identities,
focus/action routes and published geometry as structured UI access. Host-specific
Graph authoring, parameter schemas, model providers and asset persistence are not
part of this repository.

The optional viewport draws host-owned scene previews alongside the UI. GPU ID
picking and editable Rotate/Scale gizmo contracts are not promised here. Native
WebView content and OS dialogs remain external interaction boundaries.

See [Agent access](../../docs/AGENT-ACCESS.md), package API inventories and the
[independent release procedure](../../docs/RELEASING.md) for the current contract
and commands.
