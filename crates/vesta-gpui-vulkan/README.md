# Native GPUI integration

The official demonstration and native acceptance application is the existing
[`vesta-gpui-standalone`](../../apps/vesta-gpui-standalone/Cargo.toml). From the
authoritative workspace or its exported SDK, run:

```powershell
cargo run --locked -p vesta-gpui-standalone
```

Standalone loads its existing Windows fonts from `C:\Windows\Fonts` and owns
system clipboard, AccessKit, input/event-loop and rendering policies. Rust1.89.0
and a compatible Vulkan driver/loader are required. Qualified behavior follows
the candidate API inventory and incremental native acceptance evidence.

## Existing development probes

The optional backend GPU command observation API is described in
[`SUPPORTED-API.md`](SUPPORTED-API.md). It is disabled by default. Standalone's
explicit `--profile-gpu` records actual command timestamps and original frame
context; the2026-10-26 workspace qualification includes normal rendering and
retirement. Existing `--profile` and Standalone `--profile-ui` do not enable GPU
queries. The latter also exposes passive native event/frame markers. Native
receipt to successful present-call return and matching GPU command completion
are separate observations; physical display latency remains unknown.

The small examples below remain optional integration/performance references.
Standalone supplies the application entry; additional examples and the Agent's
Todo development task are not GPUI release requirements.

These two Windows developer-preview examples compose public SDK contracts and
open real OS windows. They require Rust 1.89.0, a compatible Vulkan driver/loader
and fonts supplied by the host. Their shared implementation is in
[`examples/native`](examples/native/mod.rs); it is reference application code,
not another framework runtime or a dependency on Standalone/Agent.

From the authoritative workspace, or from an independently installed SDK on its
clean worker:

```powershell
cargo run --locked -p vesta-gpui-vulkan --example native_starter -- --font=C:\Windows\Fonts\segoeui.ttf
cargo run --locked -p vesta-gpui-vulkan --example native_interaction -- --font=C:\Windows\Fonts\segoeui.ttf --font=C:\Windows\Fonts\msyh.ttc
```

The starter displays a label, a caller-owned counter and public action buttons.
The interaction example adds single-line editing, selection/caret, system
Copy/Paste, Undo/Redo, Tab/Shift+Tab, a host image and a scroll box. Enter in Text
increments the counter. The system title bar supports resize, minimize/restore
and close. Run the same command again to reopen; no private application state or
Vesta project is needed. Font fallback depends on the supplied bytes and the
declared stack: registered families enter `sans-serif`, which this editor uses.
There is no implicit system-font discovery. No font or image I/O happens in the renderer.

Append `--smoke` for a finite **source-controlled native smoke**. It routes the
rendered Increment button through public hit testing/PointerClick, exercises
public text edit/Undo/Redo in the interaction example, requests a real OS resize,
presents at least three frames and exits the event loop. Successful shutdown
observes GPU retirement after the event loop ends. A 15-second smoke timeout or
GPU/host error exits with a failure; unfinished retirement is not reported as
complete. Smoke does not inject physical keyboard/mouse input and does not
qualify IME, clipboard, DPI, reader speech or the physical Close button.

Append `--profile` instead for a finite CPU measurement: a 16ms counter animation,
24 successful warmup frames and 96 sampled completed frames, then normal shutdown
and retirement observation. The existing public `FrameCpuProfile` supplies nine
nested renderer intervals; the host reports raw samples, mean, nearest-rank
p50/p95 and maximum as JSON lines. Host CPU includes renderer CPU, so do not add
them together. Constructors, warmup, idle, logging, GPU execution and end-to-end
input latency are outside the measured frame interval. Extent/scale changes while
sampling or a 30s probe timeout fail explicitly. `--profile` and `--smoke` cannot
be combined. Default examples retain invalidation-driven idle behavior.

Build this independent project with its ordinary default Cargo target. Do not
share another project's target or copy executables around a live file lock.
Examples keep their normal paths under `target/debug/examples`; test harnesses
are not application entry points.

## Host ports and ownership

| Port | Public contract | Reference host policy |
| --- | --- | --- |
| Fonts | `TextInputContext::register_font` | Repeated explicit `--font` arguments; invalid/unreadable input is an error. |
| Images | `ImageResolver::resolve_state`, `ImageResolution` | One immutable checker image owned by the host; unknown sources return Missing. |
| Clipboard | `vesta_gpui_winit::Clipboard` | Lazy `arboard` system clipboard, using the already pinned SDK consumer version. Errors are displayed as input diagnostics. |
| Input | `InputOwnership`, `FocusScope`, `PointerClick`, `TextInputBridge` | One stable window identity; host-owned model, routes and editor state. Blur cancels composition/capture. Removed or unpublished targets cannot receive actions. |
| Geometry | `RetainedLayoutEngine`, `PaintList`, `HitTestTree`, text view output | Layout, painting, hit testing and text input share the current tree. Input routes become current only after successful presentation. |
| Accessibility | `AccessibilityTree`, `AccessibilityAdapter` | Public roles/names/focus and text projections; validated Focus/Click/SetValue/SetTextSelection requests. Actual reader/TextRange qualification remains pending. The image is described by a Label, not an unsupported image role. |
| Redraw | winit `request_redraw`, `ControlFlow` | Wait when idle; redraw on invalidation. A focused active editor has an explicit 500ms caret timer; pending GPU work has a bounded retry wakeup. There is no permanent FPS loop. |
| GPU lifecycle | `RendererCreation`, `RendererUpdates`, `WindowRenderer`, `RetirementMonitor` | Worker-owned creation/reconfiguration, current extent before rendering, cancellation on exit, retirement observation after handing off owners. Fatal GPU failure ends this small host; automatic device-loss recovery is not promised. |

The examples deliberately have no Workspace/Dock, WebView, Graph/Task model,
provider credentials or product persistence. They do not freeze every public API
or certify a whole widget family. The
[candidate API inventory](SUPPORTED-API.md) and repository GPR-01–06 plan retain
the remaining independent-build, physical input, performance and release gates.
