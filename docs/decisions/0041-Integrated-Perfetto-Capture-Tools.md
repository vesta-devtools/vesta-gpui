# Integrated Perfetto capture tools

Status: Superseded as product direction by [0043](0043-Native-Insights-Workflow-And-Agent-Capture.md); retained as optional technical proof.
Date: 2026-11-09.
Revises the preferred third-party tool in [0039](0039-Bundled-Third-Party-Trace-Tools.md)
and the future frontend restriction in [0034](0034-Native-Trace-Analysis-Application.md).

The user selects Perfetto to reduce generic analyzer development, and clarifies
that the tool must connect to an application and offer start/stop capture in the
same window, comparable to the capture workflow in Unreal Insights. An offline
file viewer alone does not meet this request.

Use `vesta-trace` as the capture controller and native desktop shell, hosting
the pinned, unmodified Perfetto UI through the existing WebView2 carrier.
Delegate generic CPU timeline interaction and analysis to Perfetto. Keep the
existing recorder, frame/window/submission identities, typed GPU evidence,
Agent queries and the native details viewer during migration.

Provide Launch/connect, start, stop/analyze and repeat capture. Stop transfers
an immutable snapshot off the target UI thread, saves the complete JSON and
automatically loads it in the embedded timeline. Live event counts are shown;
the timeline is populated after stopping, not streamed while recording.

`UiTraceRecording::from_arguments` admits the explicit developer-tool environment
configuration and owns its control worker for the host run. Ordinary use opens
no listener and starts no capture. The helper binds only loopback, authenticates
bounded requests and shares the existing process-local recorder. The core
retains no HTTP server, browser or event loop. Only one capture can be active.
The analyzer launches a selected application without a shell, or attaches through
an explicitly selected local session descriptor. It does not inject into arbitrary
processes. Closing the analyzer does not exit the consuming application.

Bundle a fixed official UI ZIP, verify its SHA-256 and file manifest, and resolve
it relative to the analyzer executable. Runtime data and resources stay on a
per-run local origin; no online fallback or external network resources are needed.
Windows WebView2 remains a runtime prerequisite. Preserve source archive release
ownership; the developer-tools bundle is produced separately.

The standard JSON importer handles CPU spans, instants and counters. GPUI's GPU
extension does not become a calibrated Perfetto track merely by loading that
JSON. Retain its diagnostics, saved source and native/Agent detail entry. Do not
claim full-function sampling, system tracing or complete Insights parity.

Qualify actual WebView2/WASM parsing against recorded event counts, real target
start/stop repeated captures and GPU evidence, invalid replacement rollback,
bounded local resources, worker/browser retirement and relocated packaged launch.
Keep command-origin, physical-input and reader evidence distinct.
