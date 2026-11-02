# Vesta Trace

The primary delivery criterion is Agent-friendly evidence acquisition and
analysis. See the [versioned read-only query interface](AGENT-QUERIES.md) for
generated schemas, persistent JSONL requests, stable evidence IDs, paging and
budgets. The native viewer and machine clients share the same analysis.

The first machine slice passed nine contract tests, strict Clippy, real
Motion/Agent/Builder evidence workflows and an 808,000-event synthetic session;
60 actual wire pairs independently validate against its generated schemas.
This does not complete native interaction, GPU or consuming-host delivery gates.
The current analysis-4 slice adds automatic capture-owned Vulkan timestamp
evidence and a shared native/Agent GPU inspection path. Real samples, exact high
device ticks and native detail scrolling are qualified in the plan; remaining
interaction/accessibility release audit and final consumer integration exits
stay open. Multi-window/controlled recovery correlation has directed real-timestamp
and machine evidence; it does not prove physical device loss. The native AccessKit adapter, live search/action
projection and keyboard GPU-detail scrolling are now connected and qualified in
the directed CPU14/GPU19 states; this does not claim reader speech acceptance.

An independent native GPUI application for analyzing recorded UI work. Development
is active under [VT01–VT06](../../docs/TRACE-PLAN.md); the first native timeline is
not the completed professional-tool qualification.

Build from this repository's default Cargo target on Windows:

```powershell
cargo build --locked -p vesta-trace
.\target\debug\vesta-trace.exe path\to\capture.json
.\target\debug\vesta-trace.exe path\to\report.html
```

You can also start without a file and drop a capture onto its window. HTML input
is read as embedded JSON data; its scripts are never executed. The application
uses its own process and does not enable tracing of itself by default. Input
files remain unchanged.

Import and analysis offer Cancel (or Escape outside composition). A failed or
cancelled job restores the last validated source/view and retains the search
draft; Enter applies that draft again. Navigation against an old source cannot
replace a pending import. A newer dropped file can replace it. The status keeps
the active filename distinct from a failed import target, and generation checks
reject late success and failure messages. Cancelling deferred analysis after a
successful import keeps the newly loaded file.

The current timeline provides a frame overview, recorded thread/depth tracks,
slowest-frame navigation, zoom, pan, range selection, interval timer totals and
frame percentiles. It preserves unframed events. Dense views display bounded
pixel aggregates and disclose collapsed events; zoom to inspect individual work.
Search uses the shared text/IME editor. Eight inspection tabs expose timer totals,
callers, callees, individual occurrences, recorded trees, raw counter observations
capture quality and GPU submission evidence. Parent/frame buttons follow original event keys. The native
inspection jobs use the same bounded query engine as machine clients. The counter
tab shows indexed raw points and extrema-preserving guide curves; machine clients
can request the same bounded envelopes through `counter_series`. Stronger
navigation/accessibility and complete CPU/GPU association qualification remain
tracked by the plan.

| Action | Current control |
| --- | --- |
| Fit capture | Home outside statistics/GPU detail, or Fit all |
| Zoom | `+` / `-`, toolbar, or Ctrl+wheel around the pointer |
| Pan time | Right-button drag, or Shift+wheel |
| Scroll tracks | Wheel |
| Scroll table rows | Wheel over the table; page buttons for paginated evidence |
| Search captured category/name labels | Ctrl+F, then type; 180 ms debounce or Enter |
| Navigate / activate controls | Tab / Shift+Tab, then Enter / Space |
| Leave search | Escape (composition is cancelled first) |
| Select interval | Left-button drag in the timeline |
| Select recorded scope | Click a visible slice |
| Select slowest frame in analysis range | `W` or Slowest frame |
| Previous / next selected frame | Left / Right arrow or toolbar |
| Cancel an active gesture | Escape; focus loss also restores the gesture's original view |
| Cancel pending import or analysis | Cancel in the header, or Escape outside composition |
| Scroll GPU evidence | Focus its detail view with Tab; Page Up/Down, Home/End or Up/Down |
| Scroll statistics | Tab/click the statistics view; Page Up/Down, Home/End or Up/Down |
| Scroll thread tracks | Focus the timeline; Page Up/Down, End or Up/Down (Home fits) |

Inclusive time includes children and cannot be added across a hierarchy without
double counting. Self/unattributed time subtracts the union of direct recorded
children on the same thread and includes waits and uninstrumented work. Frame
percentiles describe full recorded attempts intersecting the selected range,
not presented/displayed frame latency. New captures may include actual GPU
submission durations; their device ticks are not calibrated to the CPU timeline.
See the [capture contract](../../docs/TRACING.md).

For command-line data inspection without opening a window:

```powershell
.\target\debug\vesta-trace.exe path\to\report.html --summary
.\target\debug\vesta-trace.exe path\to\report.html --summary --filter=layout
```

Import is bounded to 256 MiB / one million events with a checked hierarchy depth.
The production reader uses a single application-owned worker, a bounded request
queue, cancellation and generation checks before publishing immutable results.
Its accepted format is GPUI schema-v2 CPU Trace Event data, including older v2
reports without a detail field, plus the optional typed `gpui_capture` schema-1
GPU extension; unspecified detail is disclosed rather than
assumed. Unknown schemas/clocks, duplicate/cyclic identities and invalid numeric
ranges fail explicitly. Imported warnings do not disappear during filtering.

Focused software and first native smoke commands:

```powershell
cargo test --locked -p vesta-trace --bin vesta-trace
cargo clippy --locked -p vesta-trace --all-targets -- -D warnings
.\target\debug\vesta-trace.exe path\to\report.html --smoke-capture=target\trace-native-new
.\target\debug\vesta-trace.exe path\to\report.html --inspect-capture=target\trace-inspection-new
.\target\debug\vesta-trace.exe path\to\capture.json --navigation-capture=target\trace-navigation-new
.\target\debug\vesta-trace.exe path\to\report.html --trace-ui=target\trace-viewer-cpu-new.json
```

The smoke requires a new output directory, drives five existing production view
states through typed commands, saves genuine renderer captures on the worker,
then closes and verifies worker/GPU retirement. It is command-origin acceptance;
it is not physical input, Reader or cross-monitor evidence. These finite commands
do not substitute for the remaining professional-tool gates.

`--inspect-capture` drives fourteen CPU states, including normalized pointer/keyboard
input on current layout, forward/reverse Tab and Enter, query paging, superseded
jobs, a smaller native window and reloading the original source. Each state saves
a real GPU image before advancing. The same 45-second scenario limit applies;
the hidden scenario polls its own completion/resize without desktop input.
An input with GPU submissions adds five states for native submission picking,
scrolling raw evidence in both directions and original CPU frame navigation,
using the same deadline. The uncalibrated-clock notice stays visible.
The affected native checks also exercise current AccessKit text/actions,
undoable bounded replacement, foreign/invalidated target refusal, secondary-button
focus-loss cancellation and keyboard GPU scroll. They use actual shared host
handlers with command origin, not injected OS or reader input. Normal frames
build a semantic projection only while an assistive client is active.

`--navigation-capture` is a separate nine-state fixture for committed intervals,
held-preview statistics, anchored zoom, pointer/wheel pan, fit and resize
cancellation. It retains the 45-second limit and saves each real native state.
Statistics continue to name the completed interval while a held preview waits
for release. Conflicting inspection/navigation scene options reject.

Save the fixture's stdout byte-for-byte as UTF-8 with a process runner. Then use
the standard-library Python verifier for native/query/original-data agreement:

```text
python tools/verify/trace_navigation.py capture.json native-stdout.log --analyzer <absolute-standard-vesta-trace.exe> --expected-analyzer-sha256 <verified-SHA256> --output <new-evidence-directory>
```

The verifier opens one read-only query session, preserves source/analysis and
executable identities, and compares clipped calls/inclusive/self and whole-frame
percentiles with the original CPU records. It never drives a window. Keep raw
UTF-8 logs; Windows PowerShell 5's default redirection encoding is unsuitable.
This maintainer oracle does not replace the production query engine. Current
qualification and remaining exits are recorded in the delivery plan.

Keyboard scrolling keeps analysis and page history. A temporary loading view
retains logical focus until actual results can validate it. Replacing the
original source retires its platform-node namespace and pointer preview before
publishing new facts; global search intent remains separate from source selection.

The focused keyboard/source fixture uses two explicitly synthetic inputs:

```text
node tools/verify/trace_focus.mjs --prepare <new-fixture-directory>
target/debug/vesta-trace.exe <synthetic-focus-A.json> --focus-capture=<new-image-directory> --focus-source=<synthetic-focus-B.json>
node tools/verify/trace_focus.mjs --verify <fixture-manifest.json> <UTF-8-native-stdout.log>
```

The ten states cover keyboard reachability, loading focus, actual page-history
preservation and old-source-node/pointer retirement. Both paths are required
only for this finite fixture, not ordinary operation. Input timings are synthetic;
actual renderer images and action routing are checked without OS/reader input.

`--trace-ui` is opt-in self profiling through the existing SDK recorder. It
records `Trace.Main` attempts and the application projection phase `view.project`,
alongside automatic framework stages. Export happens after the event loop,
analysis worker and GPU retirement finish and refuses existing output files.
Machine-only modes reject this writer flag. The resulting trace includes actual
supported GPU submission timestamps automatically and can be read with this
application's own versioned queries. CPU scopes are not full-function sampling;
device timestamps do not imply calibrated CPU/GPU clocks or display latency.
