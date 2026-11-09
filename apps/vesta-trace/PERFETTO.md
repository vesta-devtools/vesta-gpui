# Vesta Trace with Perfetto

Historical integration proof, optional since decision 0043. Native Trace is the
default product. Build this proof with `cargo build --locked -p vesta-trace
--features perfetto` and launch with `vesta-trace.exe --perfetto`.

Vesta Trace provides capture controls and an embedded, offline Perfetto timeline
in one desktop tool. On Windows it uses the SDK's existing WebView2 carrier.
The WebView2 runtime must be installed. No browser, system tracing service,
administrator privileges or separately installed profiler is required.

## Capture from the tool

1. Run `vesta-trace.exe --perfetto`. The tools bundle includes `vesta-motion.exe` as a real
   GPUI example; its path is filled in automatically. You can enter your own
   application's executable instead.
2. Click **Launch & connect** and wait for **Connected**.
3. Click **Start capture**, then use your application normally.
4. Click **Stop & analyze**. The captured timeline opens in this same window.
   Reaching the chosen time/event limit also stops and loads the capture.
   Analysis fills the entire content area. The small **Capture** overlay returns
   to recording controls; **Analyze** restores the full timeline without reloading
   it. **Diagnostics** opens quality notices and the saved path on demand.
5. Select a slice to inspect its recorded work. Use W/S to zoom, A/D to pan,
   Ctrl+wheel to zoom, or drag to select a time range.

Stopping saves the complete GPUI JSON to `%LOCALAPPDATA%/VestaGPUI/Traces/`.
The saved path is shown below the timeline. **GPU & capture details** opens
the existing native evidence viewer for that file. The same file supports
`--query-stdio` and `--summary` for Agent and command-line analysis.

Disconnecting or closing the analyzer retires the listener/capture of an
application launched by this tool; it leaves the application running. A separately
attached application retains its listener. Neither operation exits the target
application or modifies its documents.

## Integrate a consuming GPUI application

Applications using `vesta_gpui_winit::UiTraceRecording::from_arguments` and
`finish_after_run` from this SDK already support tool-controlled capture.
Keep the recording owner alive across the event loop, and keep the existing
`trace::frame` guard at each frame entry. Common framework scopes are automatic.

Ordinary launch creates no listener, worker or recording. **Launch & connect**
sets the explicit `VESTA_GPUI_TRACE_CONTROL` environment variable for that child
process only. It names a private session descriptor and random capability token.
The target's control worker binds an ephemeral IPv4 loopback port, authenticates
bounded commands, and serializes the stopped snapshot away from its UI thread.
No HTTP server or browser is added to the GPUI core.

For an already-running development application, start it with that environment
variable containing `{"descriptor":"<absolute new session.json>","token":"<32 hexadecimal characters>"}`.
The target creates that descriptor with its port, process ID and token. Enter the
descriptor path under **Connect to an application already running with trace
control**. Treat the descriptor as a local debugging capability; use a private
directory. This is explicit attachment, not automatic discovery or process injection.

To supply application arguments without a shell:

```powershell
.\vesta-trace.exe --target=D:\MyApp\my-app.exe --target-arg=--some-option
```

## Existing files and compatibility

**Open capture…** accepts the existing GPUI schema-v2 JSON and offline HTML
reports. HTML is parsed as inert data; its scripts are never executed. Existing
input limits, capture identity, loss diagnostics and GPU extension validation
apply before data reaches Perfetto. Original files are unchanged.

Perfetto's standard timeline shows CPU spans, instant markers and counters.
The GPUI GPU extension is retained, but is not automatically converted into
calibrated GPU tracks. The notice and native detail entry make that distinction
visible. Existing Agent queries and their stable evidence identities are retained.

```powershell
.\vesta-trace.exe capture.json
.\vesta-trace.exe report.html
.\vesta-trace.exe capture.json --native
.\vesta-trace.exe capture.json --query-stdio
```

The old native acceptance/self-profiling flags continue to select the native
viewer. Explicit `--perfetto` cannot be combined with native or machine modes.
`--perfetto-ui=<directory>` overrides the adjacent UI bundle for development;
the normal tools entry resolves it beside its executable, independently of PATH
and the current working directory. Missing, mismatched or changed assets fail
explicitly; there is no online fallback.

## Build and package

From a repository checkout:

```powershell
cargo build --locked -p vesta-trace -p vesta-motion
python -B tools/perfetto/bundle.py --assets-only --output target/debug/perfetto-ui
.\target\debug\vesta-trace.exe
```

The output must be a new directory. The bundler downloads the official pinned
Perfetto v59.1 ZIP, verifies its SHA-256, validates archive paths/budgets and
records per-file hashes. Upstream JS/WASM/CSS bytes are unmodified. The app
verifies this manifest before opening the window. Runtime content is served
only from the local bundle, with external network access blocked by CSP.

To create a portable tools directory (release build by default):

```powershell
python -B tools/perfetto/bundle.py --output target/distribution/vesta-trace-perfetto
```

For an already-built debug tool and demo, supply `--executable` and
`--demo-executable`. Keep `vesta-trace.exe`, `vesta-motion.exe`, `perfetto-ui/`
and the supplied notices together. This developer-tools bundle is separate from
an application's ordinary release package and the SDK source archive.

## Focused validation

```powershell
python -B -m unittest discover -s tools/perfetto -p test_bundle.py
cargo test --locked -p vesta-trace --bin vesta-trace
cargo clippy --locked -p vesta-trace -p vesta-gpui-winit -p vesta-gpui-webview --all-targets -- -D warnings
.\target\debug\vesta-trace.exe capture.json --perfetto-smoke
.\target\debug\vesta-trace.exe --perfetto-capture-smoke --target-arg=--hidden --target-arg=--observe-seconds=30
```

The offline smoke uses actual WebView2 and Perfetto's WASM SQL engine to compare
parsed slices (including instant markers) with the imported events and requires
rendered tracks. The live smoke drives the actual control buttons, launches a
real Motion process, records/stops twice and verifies both parsed captures,
frames and disconnection. Private probes are version-pinned and test-only; the
production bridge uses Perfetto's documented PING/PONG and postMessage API.
These command-origin checks do not claim physical-input or screen-reader acceptance.

Upstream references: [embedding](https://perfetto.dev/docs/visualization/embedding-the-ui),
[Chrome JSON import](https://perfetto.dev/docs/getting-started/other-formats),
[v59.1 release](https://github.com/google/perfetto/releases/tag/v59.1).
