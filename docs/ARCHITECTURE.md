# Architecture and ownership

This is the current implementation map. The [refinement programme](REFINEMENT.md)
tracks remaining audits; this document does not assert that every API is already
minimal. Changes to a boundary require a decision and its checks.

## Libraries

| Package | Owns | Receives from its caller | Does not own |
| --- | --- | --- | --- |
| `vesta-gpui` | Element identity; derived layout, paint and hit data; text/editing contracts; input ownership; semantics and bounded trace data | Element trees, fonts/text contexts, input intent, explicit capture configuration | OS event loop, native window, GPU device, application document or account |
| `vesta-gpui-widgets` | Reusable projections and typed interaction state | Stable IDs, values, actions, theme, explicit interaction owners | Business meaning, author persistence, provider I/O or a second text engine |
| `vesta-gpui-winit` | Native event/IME/accessibility adaptation; explicitly started stdio and trace transport helpers; monitor geometry | Existing native windows, dispatch, clipboard and host permissions | Application state, approvals, event-loop policy or renderer |
| `vesta-gpui-vulkan` | Paint preparation, GPU submission, surfaces, bounded resource retention and retirement | Window/device inputs, paint, caller text/image resources and optional scenes | Pane/session state, author data, input routing or model services |
| `vesta-workspace` | Stable pane/tab/split model and its serialization | Typed workspace operations and stable IDs | UI elements, native windows or GPU resources |
| `vesta-gpui-workspace` | Workspace UI projection, resolved geometry and gesture helpers | The existing workspace model, IDs, input ownership and host actions | A second workspace model, native window management or persistence policy |
| `vesta-viewport` | Backend-neutral camera, scene and geometry snapshots | Explicit immutable preview data | OS/GPU lifetime or authoring runtime |
| `vesta-gpui-webview` | Optional native browser carrier and explicit local-asset mounts | Native parent, bounds, navigation/content policy and mounted resources | Agent authority, approvals, application documents or a replacement renderer |

```mermaid
flowchart BT
  widgets[vesta-gpui-widgets] --> core[vesta-gpui]
  native[vesta-gpui-winit] --> core
  renderer[vesta-gpui-vulkan] --> core
  renderer -. optional .-> viewport[vesta-viewport]
  webview[vesta-gpui-webview] --> core
  dock[vesta-gpui-workspace] --> core
  dock --> widgets
  dock --> workspace[vesta-workspace]
  host[Application host] --> widgets
  host --> native
  host --> renderer
  host --> dock
  host -. optional .-> webview
```

Arrows mean production dependencies. Libraries never depend on examples or Vesta
products. The model and viewport leaves have no dependency on the UI family.
The architecture verifier checks resolved normal/build paths, including optional
and target-specific paths; native support is still Windows-preview scope.

## State and frame lifetime

1. The host owns durable application data and the native event loop. It accepts
   intent through its existing typed actions and rebuilds projections from that
   same data. A widget must not create a second author model.
2. `InputOwnership` owns transient focus/capture for explicit window/element IDs.
   The host adapts native events and reconciles it with the current live route.
   Independent windows retain their own remembered focus. `WindowTextSession`
   identifies the single caret/IME owner of a shared editor; transferring that
   session does not erase another window's remembered focus.
   Text editor state owns text/history/composition; a text context owns font and
   measurement resources. Moving one cannot silently replace the other.
3. Element trees describe the current projection. Layout, actions, hit geometry,
   accessibility and paint derive from that projection. Retained layout reuses
   solver storage; it does not become another UI model or layout algorithm.
4. The host completes necessary text/scroll/layout reconciliation before exposing
   a frame for input and rendering. The Agent consumer currently bounds its own
   settlement to eight passes. Standalone now bounds its Main/Float preparation
   the same way, with one trace/timing frame and at most one renderer attempt per
   draw. This is host policy, not a shared SDK scheduling API. A stable CPU
   PaintList supplies both hit and text-input geometry before renderer entry;
   acquire deferral does not mix those with an older text viewport. Presented
   queries, accessibility and IME publication still follow successful submission.
   The window owns an explicit validity bit for its input projection, false
   during preparation and true after coherent text/hit geometry is installed.
   Incomplete preparation blocks input and releases geometry-dependent capture;
   native lifecycle events remain available. The existing query dirty state
   separately rejects commands until a successful presented publication. These
   states express different facts, with one owner each. The product Agent instead
   shares its existing needs-layout / rebuild-required / prepared candidate state
   between native and embedded paths on one App. It installs hit, CPU layout and
   text geometry together; semantic publication cannot replace input geometry.
   These are host policies, not a new SDK state machine or scheduler. Current
   joint adoption and Builder budget/fault integration remain open in GF03.
5. The renderer retains only the resources needed by submitted work, then retires
   them through its existing owner. Submission, GPU completion and physical
   display are distinct facts. Recovery preserves caller-owned CPU state.
6. Trace capture is opt-in and bounded. Native adapter export can freeze data and
   hand file output to the host's worker. The stdio helper is explicitly started,
   process-lifetime stdin/stdout transport; its current API provides no join or
   cancellation handle. Its lifecycle suitability is an open GF04 review item.

## Public API admission

The [family boundary](SUPPORTED-API.md) owns the package support matrix and common
migration policy. Core, Widgets and renderer inventories own their detailed
contracts; the renderer does not define another package's support policy.

Every public addition must identify its caller, owner, necessary use case,
inputs/outputs, lifetime, errors and qualification. Keep pure operations separate
from I/O and platform adapters. A `pub` declaration alone is not an API design.
Private helpers stay private; example IDs/data and acceptance orchestration stay
in the examples. Optional renderer/preview/browser capabilities remain explicit.

## Standalone composition

The acceptance example has one `App` owner. Its root declares that owner, stable
IDs and composition; sibling modules borrow the same state rather than running
independent controllers. These are private binary modules, not SDK exports.

| Responsibility | Source owner |
| --- | --- |
| Demonstration values, pane history and defaults | `application_state` |
| Typed demonstration action dispatch | `demo_actions` |
| Main-window pointer and value gestures | `window_pointer` |
| Focus/text sessions, keyboard and menus | Existing `window_focus`, `window_keyboard`, `window_menus` |
| Native clipboard, resize edge and native observations | `window_platform` |
| Stateless root/overlay/control construction | `presentation`; gallery content stays in `gallery` |
| Workspace session storage and initial model | `workspace_session` |
| Fixed 3D demonstration scene | `viewport_demo`; library test fixtures are private |
| Event-loop and renderer lifetime | Existing `application_run`, `window_events`, `window_presentation`, GPU modules |

`presentation` returns derived elements/geometry. It neither commits application
values nor handles input. Platform modules adapt native boundaries; typed actions
still mutate the same demonstration state. The example's orchestration does not
become a shared frame scheduler or a second text/workspace implementation.

## Remaining audit

The Standalone package also owns the headless CPU benchmark fixture. It consumes
the real public pipeline without exporting fixture data or a scheduling API.
[Performance evidence](PERFORMANCE.md) is checked by the tool-side verifier;
recorded call analysis remains inside the existing recorder. The private recorder
entry coordinator owns append-time admission, retaining one global capture and
one session state; it adds no thread-local cache or generation model.

The current API inventories contain historical notes. GF02 will reconcile them
against actual exports; undocumented or redundant exports remain review items,
not implicitly supported features. Reuse the existing text/layout/render engines
and stable identities. Review evidence before merging packages or introducing a
trait, runtime, task pool or additional authoritative state.
