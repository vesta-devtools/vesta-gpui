# Candidate API boundaries

This is the single cross-package inventory for the next Windows native2D
developer-preview candidate. Detailed Core/Widgets contracts linked below keep
their own scope and dates. "Supported candidate" means an intended0.1 contract,
pending the current exported RC's GPR-01–06 qualification; it is not an immutable
release or a signature for every public symbol. New source-workspace modules do
not inherit older SDK qualification merely because their package version is0.1.0.

## Failure interoperability

Public named error types implement `Display` and `std::error::Error`, including
Core routing/query/raster, Widget validation/state, native window registration
and workspace split setup failures. Existing variants and payloads remain the
machine-readable authority. Hosts may propagate them with ordinary `?`, retain
their typed identity by downcasting, and display the contextual message. Display
wording is not a stable code or a string-matching interface.

Wrappers carrying a typed error expose that original cause through `source()`;
leaf variants have no cause. `TextInputBridgeError<E>` implements the standard
error trait conditionally when the injected clipboard error is `Error + 'static`;
the clipboard service itself has no new trait bound. Legacy string-valued
diagnostic payloads remain strings, not fabricated error objects. There is no
SDK-wide umbrella error or second validation layer. See
[decision 0012](decisions/0012-Public-Error-Interoperability.md).

## Package boundaries

| Package | Intended0.1 boundary | Explicit experimental or Host boundary |
| --- | --- | --- |
| `vesta-gpui` | Supported candidate: stable identity, typed Action, layout/retained invalidation, text/editor state, paint/hit/semantic projections and bounded read-only `UiSnapshot`. | Host supplies fonts, current complete trees, identities, service policy and author data. Inspection grants no command execution or OS access. |
| `vesta-gpui-widgets` | Supported candidate: the named Basic, text/focus/menu, numeric/scroll/collection, Markdown/theme and additive interaction contracts below. | Whole52-family parity, arbitrary editable rich documents and product-specific Graph/parameter semantics are outside this inventory. |
| `vesta-gpui-winit` | Supported candidate: native event/IME normalization, injected clipboard, AccessKit projection/action admission and the read-only `popup_monitor_bounds` placement query. | Host owns the event loop, text-session lifetime, permissions and native capture policy. Exact reader TextRange and unqualified IMEs/platforms remain gaps. Monitor queries do not manage windows. |
| `vesta-gpui-vulkan` | Supported candidate: native2D renderer creation/presentation, retries/reconfiguration, image resolution/fitting, explicit retirement, bounded capture and opt-in CPU/GPU observation. | Vulkan driver/loader required. Capture mapping/encoding belongs off the UI thread; native-child/OS-compositor pixels and automatic device-loss recovery are not promised. Platform placement queries belong to the winit adapter. |
| `vesta-workspace` | Experimental: stable pane/tab/split/floating model and versioned layout data. | Not an author-document model; full Workspace qualification remains GPW01–06. |
| `vesta-gpui-workspace` | Experimental: projections and typed Dock/split/drag intent over that same workspace authority. | The legacy programme accepted remaining GPW04/05 manual checks by owner waiver and closed GPW06's dependent acceptance prerequisite. No new held-input or cross-monitor observations were made; this is not a general platform guarantee. |
| `vesta-gpui-webview` | Experimental: optional native browser ownership and bounded local asset handling. | Requires the platform WebView runtime; permissions, durable application profiles and page/product actions remain Host-owned. Not browser-WASM GPUI or a general renderer guarantee. |
| `vesta-viewport` | Experimental, optional: neutral preview camera/scene/geometry contracts used by native consumers. | Not a Core operation backend, Graph author model, full3D editor or engine interoperability guarantee. Plain2D library consumers need not enable it. |

Standalone is the official demonstration/acceptance Host; Motion is the authorized
six-study companion. Both are applications, separate from the eight libraries.
Neither introduces a second framework runtime. The source SDK must build their
actual packaged source/assets without Studio, Agent, Graph or domain checkout.

### Named additive contracts and retained qualification

| Named API / behavior | Source authority and qualification boundary |
| --- | --- |
| Read-only `UiSnapshot`, next-presented/offscreen capture and `GpuProfileStream` | [Core](../crates/vesta-gpui/SUPPORTED-API.md) and [renderer contracts](../crates/vesta-gpui-vulkan/SUPPORTED-API.md). Bounded observations are not invocation, OS screenshots, physical-display latency or a perpetual profiler. |
| `ImagePresentation`, `icon_button_with_presentation`, responsive Toolbar and Breadcrumb | The [Widgets inventory](../crates/vesta-gpui-widgets/SUPPORTED-API.md) and [renderer fitting contract](../crates/vesta-gpui-vulkan/SUPPORTED-API.md#explicit-image-fitting-adr-0578) preserve original constructors, typed routes, disabled boundaries and Host resource/state ownership. Width/overflow limits remain explicit. |
| `CommandPaletteState`, scrolled palette and `SuggestionState`/request tickets | Widgets inventory; Host owns catalog/provider meaning, text/IME/history and typed execution. Stable source/query/session identities reject stale/foreign results. No provider I/O is added by Widgets. |
| `TemporaryEditorState::exchange_in` / `finish_into` and `TemporaryEditorExchange` | Widgets inventory; transfer the complete editor through the same slot, with one terminal restore. Original `open_in`/`finish` remain. GPTE01–05 source-workspace qualification does not certify new physical IME or an immutable SDK. |
| `TreeViewController`, `tree_focus_after_change` and explicit reveal/expand/collapse | Widgets inventory; borrow the existing validated preorder/selection/expansion states. Hidden live selection survives collapse, deleted identities retire and Host owns focus/scroll/author changes. GPTC01–05 qualify the bounded source-workspace composition, not tree drag/drop or native Reader. |
| Fixed and caller-measured variable-height List; retained List/Tree/Table/Tile content | Widgets GPV03/04/05 contracts. `VariableListMetrics` explicitly rebuilds measurements in O(n); it is not automatic row measurement. Tile's explicit retained-content constructor admits one offscreen item; the original non-retained constructor keeps its behavior. Host validates current stable IDs and reveal eligibility. |
| `MarkdownView` / `MarkdownInteraction` | Read-only headings, lists, emphasis/code, quotes, links, complete table cells and selection/copy. Host owns pages, reflow, edge-scroll scheduling, clipboard and navigation admission. [GPD01–06 and Host duties (historical context)](VALIDATION.md#historical-evidence) are complete in their dated source/native scope. Images remain textual references and HTML is displayed as text; this is not a browser or an editable document runtime. |

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
| Pre-release identity/theme corrections | Follow the [Core](../crates/vesta-gpui/SUPPORTED-API.md) duplicate-key error and [Widgets](../crates/vesta-gpui-widgets/SUPPORTED-API.md) progress/theme-token migration notes. These already documented source changes are not silently rewritten as compatible old artifacts. |
| Released supported0.1 contract | Compatible fixes use an explicit new patch and consumer adoption; incompatible supported changes require0.2 plus migration notes. Experimental capabilities advertise their limits and never silently replace supported behavior. |
| Local product development vs immutable adoption | All participating Hosts must resolve one family revision/content identity. Local path coherence is development integration; GPR-06 still requires immutable revision/tree pinning and affected-consumer qualification. |

This capability inventory is not a second export list or an acceptance ledger.
Current source-qualified checks are recorded in [validation](VALIDATION.md).
Historical native samples, explicit owner waivers and unsupported platform limits
retain their original scope; moving this inventory does not requalify them.
