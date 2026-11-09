# Bundled third-party trace tools

Status: Accepted direction; implementation and distribution qualification pending.
Date: 2026-11-06.
Preferred tool and capture workflow revised on 2026-11-09 by
[0041](0041-Integrated-Perfetto-Capture-Tools.md): use integrated Perfetto capture
controls and the existing recorder. The bundling/opt-in requirements below remain.
Revises the future analyzer direction in [0034](0034-Native-Trace-Analysis-Application.md).

## Decision

Future GPUI trace work should integrate a mature profiler and ship its analyzer
with the GPUI SDK/developer-tools package. A consumer who obtains that package
must be able to profile a GPUI application without separately installing a trace
tool, finding an executable or configuring PATH. Prioritize this integration
over expanding the self-developed generic trace analysis UI.

[Tracy Profiler](https://github.com/wolfpld/tracy) is the preferred first candidate:
integrate its capture client with GPUI's existing framework scopes, bundle a
compatible analyzer executable and provide an explicit development entry that
starts it and connects to the application. Final client/analyzer versions,
supported-platform packaging and actual CPU/GPU coverage require qualification.
This records a direction, not completed Tracy support.

Bundling means the analyzer is supplied and launched as part of the SDK's tools;
it may retain its own native window and process. Embedding a third-party UI as a
GPUI widget is not required. Preserve the existing Chrome Trace Event export for
compatible tools such as [Perfetto](https://perfetto.dev/docs/getting-started/other-formats).

## Integration and distribution

- Reuse the framework's maintained scope locations. Keep GPUI-specific capture,
  frame/window/submission correlation and adapter code; delegate generic timeline
  navigation and profiler analysis UI to the selected tool.
- Make the profiler backend explicitly opt-in. Ordinary GPUI use must not need
  the analyzer, start it, collect profiling data or open a profiler connection
  listener. Application release packages may omit the developer tools.
- Ship a matched capture-client/analyzer version, required runtime files and
  upstream license notices in the supported SDK tools bundle. A Cargo feature
  alone does not distribute an analyzer executable.
- Resolve the analyzer relative to the SDK tools bundle, independently of the
  working directory or PATH. The host/developer-tools entry owns explicit
  activation, connection and process cleanup. Report missing/incompatible tools
  clearly; do not make ordinary application startup depend on them.

## Migration and acceptance

The current recorder, JSON/HTML exports, GPU evidence and versioned read-only
Agent queries remain the supported implementation until a replacement path is
qualified. Existing Chrome JSON export alone does not establish a working Tracy
connection or equivalent GPU/Agent evidence. Define adapters or an explicit
migration for those contracts before replacing their implementation.

Acceptance requires a clean supported Windows environment with no separately
installed profiler: unpack the SDK tools, run a real consuming application,
explicitly start capture and open the bundled analyzer through the provided
entry. Verify actual framework scopes, frames, supported GPU evidence, repeated
connect/disconnect and normal shutdown. Verify ordinary use with profiling
disabled and without the tools directory. Record capture overhead and packaged
dependencies with their actual tested scope.

The completed VT01-VT06 delivery and its source/evidence identities remain
historical results. This decision authorizes the next integration direction;
no third-party binary is added and no runtime or packaging behavior changes in
this documentation update. Track implementation in [TRACE-PLAN](../TRACE-PLAN.md).
