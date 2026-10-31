# Native host integration

Start with the complete [native host recipe](../apps/vesta-gpui-standalone/examples/native_host.rs),
then use Standalone or Motion for richer controls and accessibility. The recipe
has one window and one multiline editor; it owns no author document, service,
workspace model or alternate framework runtime. It consumes existing public APIs.

```powershell
cargo run --locked -p vesta-gpui-standalone --example native_host
```

This Windows preview loads Segoe UI and Microsoft YaHei from Windows Fonts.
Missing files or rejected font data fail explicitly at startup. Replace that
host font policy when embedding. Clipboard is an injected service; the example
uses arboard and never reads/writes it merely because a window starts.

## Owners and derived data

| Owner | Responsibility |
| --- | --- |
| Host application | Text/history, native window, font inputs, clipboard, redraw policy and error presentation |
| `TextInputContext` | Font/measurement resources, distinct from the editor's text/history |
| `InputOwnership` and native bridge | Stable field capture and native modifier/IME/click-session adaptation |
| Current CPU projection | Text viewport/scroll derived from the same snapshot and bounds used by paint; never a second text model |
| Creation/update tickets and renderer slot | Existing asynchronous GPU creation/reconfiguration and presentation ownership |
| Retirement monitor | CPU observation after the event loop and after releasing all host GPU slots |

IDs and keys remain fixed for the surviving field. Its bounds depend only on
window size; content edits refresh its CPU text origin immediately so queued
pointer input cannot use the preceding editor visual. For multiple fields or
content-dependent geometry, use a complete current tree/route, focus scope and
window-qualified text views as demonstrated by Standalone. Do not generalize
this recipe's fixed-field assumption to a dynamic Inspector.

## Frame and native lifetime

1. Create the native window on the event thread and start `RendererCreation`.
   Poll its existing ticket through bounded event-loop wakeups. No UI callback
   waits for GPU construction or retirement.
2. Pass current physical extent through `RendererUpdates`. While target/layout
   geometry is incomplete, release field capture and reject geometry-dependent
   input. Retain the editor and logical target.
3. Build and lay out the current tree. If text wrap width changes, rebuild with
   the same editor. Reconciliation is bounded to eight passes. Install coherent
   CPU text geometry from the final PaintList before renderer acquisition.
4. Render at most once per draw. `None` is a deferred attempt; retain CPU data and
   honor the renderer's retry deadline. Successful output is checked against the
   prepared text viewport before publishing the native IME caret.
5. End the native text session on focus loss, occlusion, suspend or close without
   replacing application text/history. Closing callbacks stop scheduling before
   disposing GPU slots. Observe the existing retirement monitor only after the
   event loop returns. An observation timeout is not permission to destroy a
   worker-owned unfinished resource.

Ordinary idle use waits for native input/redraw or a real pending renderer job.
There is no unconditional frame timer. Fatal errors close this transient example;
a product must choose its own draft preservation, error UI and recovery policy.
Automatic device-loss recovery is not promised by this recipe.

## Finite qualification and tracing

Choose a new trace destination:

```powershell
cargo run --locked -p vesta-gpui-standalone --example native_host -- --smoke --trace-ui=target/host-recipe-01.json
```

The explicit hidden smoke drives this window through 760/480/900/480/760 logical
widths, applies logical text through the existing native adapter, performs Core
Undo and checks original text plus CPU/painter geometry. It closes normally and
reports GPU retirement. Hidden Windows windows do not deliver ordinary redraw
events; only this finite smoke advances the same draw entry from the event-loop
driver. It has a 20-second completion budget and does not change system DPI,
foreground another window or inject OS input. A source/binary identity and actual
PID/creation/exit must accompany qualification; a successful compile is separate.

The example uses the shared `UiTraceRecording` launcher and one frame guard,
with post-loop export. See [tracing](TRACING.md) for capture ownership, limits and
wall-time meaning, and [CPU workloads](PERFORMANCE.md) for cost comparison.

This small consumer demonstrates native rendering, editing and lifetime, not
menus, multiple fields/windows, automation transport, persistence, platform
TextRange, physical IME candidate selection or reader speech. Use
[structured UI access](AGENT-ACCESS.md) for its separate process/authority
contract. Native accessibility projection/action wiring is demonstrated in
Motion and Standalone; it remains the consuming host's responsibility.
