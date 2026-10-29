# GPUI 0.1.0 candidate API inventory

## Unified release boundary — 2026-10-28

This is the single cross-package inventory for the next Windows native2D
developer-preview candidate. Detailed Core/Widgets contracts linked below keep
their own scope and dates. "Supported candidate" means an intended0.1 contract,
pending the current exported RC's GPR-01–06 qualification; it is not an immutable
release or a signature for every public symbol. New source-workspace modules do
not inherit older SDK qualification merely because their package version is0.1.0.

| Package | Intended0.1 boundary | Explicit experimental or Host boundary |
| --- | --- | --- |
| `vesta-gpui` | Supported candidate: stable identity, typed Action, layout/retained invalidation, text/editor state, paint/hit/semantic projections and bounded read-only `UiSnapshot`. | Host supplies fonts, current complete trees, identities, service policy and author data. Inspection grants no command execution or OS access. |
| `vesta-gpui-widgets` | Supported candidate: the named Basic, text/focus/menu, numeric/scroll/collection, Markdown/theme and additive interaction contracts below. | Whole52-family parity, arbitrary editable rich documents and product-specific Graph/parameter semantics are outside this inventory. |
| `vesta-gpui-winit` | Supported candidate: native event/IME normalization, injected clipboard and AccessKit projection/action admission. | Host owns the event loop, text-session lifetime, permissions and native capture policy. Exact reader TextRange and unqualified IMEs/platforms remain gaps. |
| `vesta-gpui-vulkan` | Supported candidate: native2D renderer creation/presentation, retries/reconfiguration, image resolution/fitting, explicit retirement, bounded capture and opt-in CPU/GPU observation. | Vulkan driver/loader required. Capture mapping/encoding belongs off the UI thread; native-child/OS-compositor pixels and automatic device-loss recovery are not promised. `popup_monitor_bounds` is a read-only placement query, not a window manager. |
| `vesta-workspace` | Experimental: stable pane/tab/split/floating model and versioned layout data. | Not an author-document model; full Workspace qualification remains GPW01–06. |
| `vesta-gpui-workspace` | Experimental: projections and typed Dock/split/drag intent over that same workspace authority. | GPW04 physical held-Escape/cancellation, GPW05 actual cross-monitor DPI and dependent GPW06 are incomplete. No second display or synthetic input is inferred. |
| `vesta-gpui-webview` | Experimental: optional native browser ownership and bounded local asset handling. | Requires the platform WebView runtime; permissions, durable application profiles and page/product actions remain Host-owned. Not browser-WASM GPUI or a general renderer guarantee. |
| `vesta-viewport` | Experimental, optional: neutral preview camera/scene/geometry contracts used by native consumers. | Not a Core operation backend, Graph author model, full3D editor or engine interoperability guarantee. Plain2D library consumers need not enable it. |

Standalone is the official demonstration/acceptance Host; Motion is the authorized
six-study companion. Both are applications, separate from the eight libraries.
Neither introduces a second framework runtime. The source SDK must build their
actual packaged source/assets without Studio, Agent, Graph or domain checkout.

### Named additive contracts and retained qualification

| Named API / behavior | Source authority and qualification boundary |
| --- | --- |
| Read-only `UiSnapshot`, next-presented/offscreen capture and `GpuProfileStream` | Existing Core and renderer contracts below. Bounded observations are not invocation, OS screenshots, physical-display latency or a perpetual profiler. |
| `ImagePresentation`, `icon_button_with_presentation`, responsive Toolbar and Breadcrumb | The [Widgets inventory](../vesta-gpui-widgets/SUPPORTED-API.md) and renderer fitting contract below preserve original constructors, typed routes, disabled boundaries and Host resource/state ownership. Width/overflow limits remain explicit. |
| `CommandPaletteState`, scrolled palette and `SuggestionState`/request tickets | Widgets inventory; Host owns catalog/provider meaning, text/IME/history and typed execution. Stable source/query/session identities reject stale/foreign results. No provider I/O is added by Widgets. |
| `TemporaryEditorState::exchange_in` / `finish_into` and `TemporaryEditorExchange` | Widgets inventory; transfer the complete editor through the same slot, with one terminal restore. Original `open_in`/`finish` remain. GPTE01–05 source-workspace qualification does not certify new physical IME or an immutable SDK. |
| `TreeViewController`, `tree_focus_after_change` and explicit reveal/expand/collapse | Widgets inventory; borrow the existing validated preorder/selection/expansion states. Hidden live selection survives collapse, deleted identities retire and Host owns focus/scroll/author changes. GPTC01–05 qualify the bounded source-workspace composition, not tree drag/drop or native Reader. |
| Fixed and caller-measured variable-height List; retained List/Tree/Table/Tile content | Widgets GPV03/04/05 contracts. `VariableListMetrics` explicitly rebuilds measurements in O(n); it is not automatic row measurement. Tile's explicit retained-content constructor admits one offscreen item; the original non-retained constructor keeps its behavior. Host validates current stable IDs and reveal eligibility. |
| `MarkdownView` / `MarkdownInteraction` | Read-only headings, lists, emphasis/code, quotes, links, complete table cells and selection/copy. Host owns pages, reflow, edge-scroll scheduling, clipboard and navigation admission. [GPD01–06 and Host duties (historical context)](../../docs/VALIDATION.md#historical-evidence) are complete in their dated source/native scope. Images remain textual references and HTML is displayed as text; this is not a browser or an editable document runtime. |

The original UI-DOC-TABLE-001 user report still lacks its exact Host/input
binding. Generic Markdown tables passing does not establish that report's fix.
Precise platform TextRange/speech, actual cross-monitor DPI and physical
cross-HWND capture/drop retain their explicit limits. GPP01–06 have dated
reference workload qualification, including GPU command observations and native
receipt-to-present-return samples; they do not measure physical display latency
or qualify a later RC's changed source automatically.

### Compatibility and migration

| Consumer input | Required handling |
| --- | --- |
| Older same-version source SDK | Compare exact revision/tree and required exports. Matching `0.1.0` alone is insufficient. SDK17 and Motion01 remain historical qualified inputs; newer Tree/temporary-editor changes require the new common candidate. |
| Existing constructor or state APIs | Retain original signatures and semantics. New explicit presentation/retained-content/exchange helpers are opt-in; Host lifecycle and stable IDs remain authoritative. No hidden provider, clock, document write or alternative model is introduced. |
| Pre-release identity/theme corrections | Follow the [Core](../vesta-gpui/SUPPORTED-API.md) duplicate-key error and [Widgets](../vesta-gpui-widgets/SUPPORTED-API.md) progress/theme-token migration notes. These already documented source changes are not silently rewritten as compatible old artifacts. |
| Released supported0.1 contract | Compatible fixes use an explicit new patch and consumer adoption; incompatible supported changes require0.2 plus migration notes. Experimental capabilities advertise their limits and never silently replace supported behavior. |
| Local product development vs immutable adoption | All participating Hosts must resolve one family revision/content identity. Local path coherence is development integration; GPR-06 still requires immutable revision/tree pinning and affected-consumer qualification. |

This reconciliation changes documentation only. Current RC build, native delta
and performance binding remain pending; old screenshots, samples, failures and
fixed acceptance cursors are not relabelled as new qualification.

## Explicit target capacity for experimental viewport consumers

Current source implements opt-in `render_with_target_mode` for a Host that
prepares Scene capacity while its main frame has UI-only content. The original
`render` and `render_with_scene` signatures/inference remain. Explicit Ui plus
scene content returns `RendererError` at `frame_scene_mode`; Scene capacity with
no main scene remains usable for offscreen scene capture. Host preparation and
submission must use the same current extent/mode. Worker reconfiguration,
acquire/submission retirement, sampled depth and bounded capture stay unchanged.

Root default/viewport library/test contracts, the six-case mode matrix and strict
checks qualify the current implementation only. Existing RC2/RC3 identities are
not republished by these changes. Independent final SDK qualification and the
Host paired main/offscreen scene/depth/readback/main evidence remain separate.
Viewport stays experimental; no dummy scene, hidden rebuild or second renderer
is used to substitute for a missing Host connection.

### Numeric scene labels and Host font ownership

The public [numeric types](../vesta-viewport/src/display/labels.rs) carry world
anchors and exact u32 values in `NumericLabels`, plus Host style and an optional
`Arc<NumericAtlas>`. The Host supplies real immutable RGBA pixels for ten digit
cells, source identities/positions, display switches, camera and target scale.
Absent labels/atlas or disabled number switches intentionally draw no numbers;
an atlas being present is not proof of readable pixels or successful acceptance.

[Offscreen capture](src/capture/offscreen.rs) consumes the same scene label and
depth path on the existing Scene-capable WindowRenderer. Backend upload, image/
sampler/descriptors, in-flight command resources and retirement are renderer
owned. The Host neither creates private Vulkan label handles nor adds a second
font renderer. Reuse the same immutable atlas Arc; replacement requires a new
resource identity and Host image invalidation. The ordinary UI glyph cache and
`ImageResolver` do not automatically populate `NumericLabels.atlas`.

Host font discovery/selection, shaping/raster input, source projection, availability
and failure presentation remain outside the SDK. Shared TextInputContext and
TextRasterizer accept explicit Host font bytes; the SDK does not search system
fonts. Blocking ticket waits/mapping stay on a worker. Numeric scene labels do
not imply an arbitrary-Unicode scene text API or a Core execution backend.

## Explicit image fitting (ADR-0578)

The existing image resolver also serves `PaintCommand::FittedImage`. Native uses
Core `ImagePresentation::place` and `ImagePlacement::clipped` for explicit
Stretch/Contain/Cover, physical two-axis anchors and normalized UV. Legacy Image
continues stretching. Fit changes geometry/UV, rather than duplicating pixels or
adding fit to texture identity. Existing pending/missing/failed/invalid/over-budget
diagnostics, clipped-resource rejection, immutable pixel reuse, alpha blending
and normal Submission retirement remain. No resource I/O or new scheduling is
introduced. Each host still supplies the semantic name and authored layout size.

The 2D UI pipeline uses straight source-over RGB and alpha: source alpha has
factor One, destination alpha OneMinusSrcAlpha. In particular, translucent
images over an opaque UI target preserve target alpha255. This corrects the
previous alpha-squared result; 3D material blending is unchanged. Actual
Main/Float presented pixels and the original alpha191 failure are retained in
[the image ledger (historical context)](../../docs/VALIDATION.md#historical-evidence). The scope is
the GPUI render target, not OS compositor or every image-loading policy.

## Optional GPU command observations (ADR-0577)

`WindowRenderer::enable_gpu_profiling` is an explicit default-off backend API.
Enabling returns a shared consuming `GpuProfileStream`; keep it until the
existing `RetirementMonitor` has completed to receive a final retired submission.
`capabilities()` reports actual queue valid bits/period or structured unsupported
metadata. `try_drain()` never waits; `dropped_observations()` records full/busy
buffer losses. Clones drain the same bounded256-observation queue.

`GpuProfileKey` identifies a process-local profiling generation and recording
attempt; it is unrelated to author IDs, HWNDs or successful presentation counts.
Compare `gpu_profile_key()` before/after a renderer call and associate the
original context with that exact key. A later `FrameGpuProfile` cannot be put
into the next frame's CPU bucket. Re-enabling or replacing a renderer produces
a fresh generation; retain old streams separately until their retirement ends.

Timestamp marks bracket upload and the enabled scene/label/UI passes, with the final mark
recorded before optional capture commands. Their elapsed device span includes
dependency/scheduling stalls between the marks; it is not isolated GPU busy time
or CPU acquire/submit/present call timing. A submission-to-completion observation
upper bound rejects ambiguous counter wraps and impossible spans. Raw ticks are
retained for available pairs, including rejected spans. Incomplete/unavailable
queries produce explicit unknown values; zero is not substituted. Reading follows the existing completion fence and
never adds a fence wait, finish, redraw or thread. Query creation/recording adds
measurement overhead only when explicitly enabled. CPU host timestamps and GPU
ticks are separate clocks.

Standalone enables this API only with `--profile-gpu`, independently of
`--profile-ui`. Its bounded ledger preserves the original window, activity,
dimensions, DPI and render outcome for each key through normal retirement.
The2026-10-26 workspace qualification includes actual GPU timestamps, bounded
10000-frame normal rendering, and native input markers associated with the first
applicable successful Host frame and its exact GPU submission. Native receipt to
present-call return is the measured input boundary; GPU completion is observed
separately. Physical display latency remains unknown. These dated source-workspace
results do not qualify a different immutable SDK artifact automatically.

## On-demand capture (additive, ADR-0423)

`WindowRenderer::capture_next_frame(CaptureRegion)` queues one transfer from the
next successful presentation if the surface supports transfer-source usage.
`capture_offscreen` uses an independent image with the same preparation/record
path; its signature is stable across the optional viewport feature.
`capture_offscreen_with_scene` also accepts the current camera/scene when viewport
support is enabled. Neither reads an arbitrary retired swapchain image or claims
OS-composited pixels.

Regions have at most 4096 pixels per edge and 4,194,304 pixels total. A
`CaptureTicket` resolves after observed GPU completion, returning staging
`CaptureReadback`; its blocking wait and `rgba()` mapping belong on a worker.
Bytes are RGBA8/sRGB with render-target alpha. Hosts own PNG encoding, sensitivity,
native-child coverage and semantic frame correlation. Pending captures follow
the existing submission/retry/retirement chain. Cancellation and renderer drop
fail requests without manufacturing GPU completion.

These source additions do not publish or requalify an earlier immutable SDK.

Status: **proposed supported slice, awaiting release qualification**. This file
names the intended native 2D support boundary; it does not claim that the current
dirty source is an immutable release, that all native cases passed, or that every
exported Rust symbol is a frozen contract. Freeze/adopt only with GPR-01–06
evidence for a clean candidate. Breaking supported 0.1 contracts require 0.2 and
migration notes; compatible fixes require explicit patch adoption.

## Candidate native 2D slice

| Area / public authority | Supported behavior proposed for 0.1 | Boundary / qualification still required |
| --- | --- | --- |
| `vesta_gpui::VERSION` | Read-only compiled package version supplied by GPUI itself. | Package metadata does not identify an immutable SDK archive, qualify an API or prove consumer adoption. |
| `ElementIdAllocator`, `ElementScope`, `UiWindowId`, `InputTarget` | Stable caller-owned element/key/window identities; typed actions, never display-name or array-position dispatch. | Host supplies identities and lifetime. Reusing an identity for a new live owner is not permitted. |
| `Element`, `Style`, `LayoutEngine`, `RetainedLayoutEngine` | Box/Flex/Grid/Overlay, wrap, RTL flow, measured mixed text/baseline; deterministic derived geometry and declared invalidation. | Supplied fonts and viewport are inputs. Narrow/DPI cases retain their acceptance limits. No universal tree cache or automatic variable-height index. |
| `PaintList`, `HitTestTree`, `ActionRoute<A>`, `PointerClick` | Same-tree paint/hit/action geometry; child action boundaries and same-owner press/release activation. | Hosts publish current routes and discard removed/unpublished targets. Cross-HWND capture/drop is outside the qualified first slice. |
| `Widget<A>`, `WidgetIdentity`, `WidgetContent<A>`, `WidgetTheme` | Public reusable composition, theme tokens and typed host actions; text/icon/image display, buttons, toggle/check controls and feedback. | State, localization, images and resources belong to the host. A capability listed here is not a whole-family acceptance signature. |
| `button_with_presentation`, `radio_group`, `radio_group_key_action`, `progress_bar_state` | Optional primary/danger/loading command states; caller-owned stable radio values with one Tab stop and shared arrow policy; finite numeric or host-phase-driven busy progress. Original constructors remain. | Busy commands are inert, duplicate radio values and non-finite progress return structured errors; hosts own actions, selected focus and visible scheduling. The existing Basic source subset is frozen2026-10-15 after the [three-capability ledger (historical context)](../../docs/VALIDATION.md#historical-evidence)'s scoped tests/native/reader qualification. SDK release qualification remains separate. |
| `TextInputContext`, `TextInputState`, `text_field`, `text_area`, Search projection | Single/multiline text, selection/caret, Undo/Redo and constrained text behavior from the shared editor. | Fonts, clipboard and IME come through host ports. Basic Chinese IME and 100%/150%/200% cases must be bound to the release consumer; other IMEs/reader ranges remain explicit gaps. |
| `InputOwnership`, `FocusScope`, shared key/popup/session controllers | Window-qualified Tab focus, live target validation, focus visibility and explicit cancellation/return decisions. | Hosts apply decisions and close old sessions on blur/removal. Physical capture loss, late events and multi-window release remain separately qualified. |
| `Menu`, `MenuController`, `menu_popup`, choice popup, tabs, tooltip/progress | Typed menus/choices, keyboard decisions, focus-return and bounded popup geometry; composed projections use shared tokens. | The basic native examples do not yet demonstrate every popup/menu branch. Dismissal/focus must be qualified before signing this release boundary. |
| `ScrollAreaState`, `ScrollInputPath`, scroll reveal helpers | Clamped host-owned scrolling, nested remainder propagation and visible-target reveal from live layout. | Extreme narrow nowrap and floating interaction retain the ledger's remaining cases. The example demonstrates wheel scrolling, not every thumb/edge-held cancellation path. |
| `ClipMode`, `LayoutNode.scroll_content` | Explicit inherited visible overflow participates in scroll extents; ordinary Clip and nested ScrollBoxes bound propagation, while NoClip overlays remain detached. Retained negotiation invalidates when this boundary changes. | Callers must opt structural wrappers into Inherit; clipped content is not implicitly made scrollable. Shared LTR/RTL and retained/stateless tests pass; native evidence retains exact host/artifact scope and does not freeze this proposed inventory. |
| Public List/Tree/Table/Tile bounded factories | Lazy visible/overscan construction from stable caller records; fixed or caller-measured variable-height Lists and explicit retained focus for List/Tree/Table/Tile. | Caller owns measurements, current identity/index mapping and reveal. Automatic variable-row measurement is not delivered. Tile retention requires its explicit retained-content constructor. Complex editable tables keep GPV05's typed draft/Host validation limits. |
| `ImageResolver`, `ImageResolution`, `RenderOutput::image_diagnostics` | Explicit host loading/ready/missing/failed state, immutable image sharing and local fallback diagnostics. | Host owns I/O/retry/redraw; renderer budgets and maximum dimensions still apply. |
| `vesta_gpui_winit::Clipboard`, `TextInputBridge`, AccessKit adapter | Native event/IME normalization, system clipboard service injection and public semantic projection/actions. | Event loop/platform service policy remains in the host. Actual speech, precise platform TextRange, DPI and scaling input need native evidence, not aggregate screenshot focus. |
| `RendererCreation`, `RendererUpdates`, `WindowRenderer`, `FrameRetry`, `RetirementMonitor` | Native Vulkan 2D creation/presentation, worker reconfiguration, delayed retries and explicit resource retirement/errors. | Compatible driver required. Surface/device failure policy is the host's responsibility. Fault fixtures do not certify physical device-loss guarantees or driver deadlines. |

Source authorities are the named public exports in `vesta-gpui`,
`vesta-gpui-widgets`, `vesta-gpui-winit` and `vesta-gpui-vulkan`. The source SDK
includes these files and their existing tests; this inventory does not duplicate
their implementations or introduce a second control model.

The [Core identity, Action and layout contract](../vesta-gpui/SUPPORTED-API.md)
is frozen on2026-10-14 after GPC01–04 and GPL01–05 qualification. It records
per-window key uniqueness, scoped identities, structured route failures, the
ADR-0412 pre-release duplicate-key error migration and the existing layout,
measurement and retained-state boundary. Main/Float narrow mixed-text geometry
is qualified at100/125/150/200% with exact source/artifact evidence. This source
contract freeze is separate from the remaining widget/native exits and overall
SDK release qualification; the rest of this inventory remains proposed.

The [Basic source contract](../vesta-gpui-widgets/SUPPORTED-API.md) records
the audited existing command/choice/feedback/ordinary-tooltip/notification
source obligations and pre-freeze theme-token migration. It is frozen on
2026-10-15 after GPB01–05 qualification; this inventory's remaining release
status is unchanged.

## Standalone and evidence binding

The manual compute-to-viewport transfer measurement is product-owned under
Builder's real `app::viewport` test module. It is outside the eight-library SDK;
the original ignored/manual measurement and Host projection contracts are
relocated together, rather than discarded or reimplemented in the renderer.
SDK export requires already-isolated library/test inputs and rejects a returning
product measurement or product dependency. Historical SOURCE projections retain
their original evidence identities. This ownership change is not a new GPU
measurement or independent release qualification.


The existing [`vesta-gpui-standalone`](../../apps/vesta-gpui-standalone/Cargo.toml)
is the official demonstration/native acceptance host and is included separately
from the eight-library family. Run `cargo run --locked -p vesta-gpui-standalone`.
Use its existing incremental scenarios and bind affected evidence to the release
candidate. No additional sample application or Todo delivery is required.
The following existing development examples retain their narrower evidence:

- [`native_starter`](examples/native_starter.rs) and
  [`native_interaction`](examples/native_interaction.rs): public-contract OS
  consumers. See [quickstart and host ports](README.md). Source-controlled native
  smoke verifies integration, not physical input.
- `vesta-gpui-widgets/examples/interaction_sessions.rs`: portable route/focus,
  text-view, popup/menu, cancellation and nested scrolling decisions.
- `vesta-gpui-widgets/examples/composite_collections.rs`: caller-owned content,
  stable identity, bounded 10,000-record factories, child routing and supported
  retained focus. No OS window, physical input or variable-height guarantee.
- `text_labels` and `layout_performance`: text/projection examples with explicit
  font inputs. Their timings do not certify native tail latency or input latency.

Historical platform results remain bound to their original source/artifact,
environment and scenario. Retest affected cases against the candidate and record
commands, source/lock hashes, binary hashes, exit/lifecycle and limits. A metadata
resolution, headless test or source-controlled smoke cannot close independent
compilation or physical acceptance gates.

## Experimental / outside this support slice

Workspace/Dock/native floating windows; advanced Markdown selection/links and
long-document pagination; complex numeric/Ramp/Color/Table editing; WebView2;
optional 3D viewport; full browser input/IME/accessibility; native WASM feature
runtime/reloadable Preview Tab; other operating systems; full Studio host migration.
Existing implementations retain their own documented contracts and evidence.
Experimental does not mean missing, stable, or fully qualified. Plain native 2D
consumers require none of these integrations.

`TooltipController::layout_with_popup` is an optional experimental projection
port for caller-owned read-only rich content. It uses the shared owner/delay,
visible focus, clipped anchor and active-window lifetime; projected descendants
cannot intercept pointer input or join the focus/action route. The caller keeps
the supplied viewport bounds and owns the content/resources. Ordinary tooltips
continue through `layout`. Agent's changed-file diff preview uses this port;
file snapshots and diff policy belong to that consumer. Candidate09 predates the
port and cannot satisfy this newer consumer's API requirement. Qualify the exact
new source/consumer input before immutable adoption; a library's unchanged
`0.1.0` string is not API/content identity or native preview qualification.

`MenuPickerPopupSpec` and `menu_picker_popup` are an optional experimental
presentation of the existing shared menu controller/routes: caller-owned title,
grouped choices and trailing current-choice marks, with viewport-bounded layout.
The caller owns the catalog, availability, action mapping and stable identities.
Application menus continue through `menu_popup`; this is not another control or
provider runtime. Current Agent's model picker requires this newer export;
candidate13 predates it. Include the exact shared API in the next common SDK
input and qualify its affected consumer/layout/native behavior before immutable
adoption. Source presence or a matching `0.1.0` version does not certify it.

Known unresolved boundaries include precise platform TextRange/reader speech,
physical cross-window capture/drop, actual cross-monitor DPI, automatic variable
row measurement and physical-display latency. Dated GPP01–06 measurements include
named CPU/GPU/idle/resource and native receipt-to-present-return samples; current
RC source/artifact binding and affected scaling/input/lifecycle qualification
are still required. Avoid a universal FPS, p99 guarantee or whole-framework
production-readiness claim.

Historical context links explain the original evidence scope; see the independent validation and release procedure.
