# Vesta GPUI

A Rust UI foundation for native desktop tools, with shared layout, text/input,
widgets, winit/Vulkan integration and opt-in structured UI access for authorized
Agent integrations. The maintained family contains eight libraries and two
application examples. No Vesta product, Graph runtime or model account is needed.

The published source baseline is **0.1.0-rc.1**, targeting Windows x86-64 with
Rust 1.89.0 / Edition 2024. The next source candidate is described in the
[rc.2 migration notes](release/0.1.0-rc.2.md). Both are developer-preview scope;
no final 0.1.0 tag or registry package is available. The rc.1 baseline passed
204 independent automated tests and strict checks. See
[validation scope](docs/VALIDATION.md) and each candidate's qualification
sidecar for the evidence applicable to that exact source.

## Run the examples

For a small complete native consumer, start with the
[single-window host recipe](docs/HOSTING.md). Standalone below provides the wider
demonstration/acceptance gallery.

From this repository root, with Node.js 22+ and the pinned Rust toolchain:

```powershell
node tools/sdk/gpui_source.mjs --verify .
cargo run --locked -p vesta-gpui-standalone
```

Standalone demonstrates controls, layout, Markdown/text, collections, input,
accessibility and Workspace integration. Native rendering needs Windows host
fonts and a compatible Vulkan loader/driver. WebView2 is needed when using the
optional native browser carrier.

The backend-neutral static editor-outline example can be run without a window:

```powershell
cargo run --locked -p vesta-gpui --example editor_outline
```

```powershell
cargo build --locked -p vesta-motion --bin vesta-motion
.\target\debug\vesta-motion.exe --tour
```

[Vesta Motion](apps/vesta-motion/README.md) contains six native motion studies.
Its geometry is host preview content; keyframe authoring, save/undo and export
are outside the example's scope. Smaller public examples remain integration
references rather than additional application products.

## UI performance tracing

The opt-in [in-process trace recorder](docs/TRACING.md) is shared by libraries and
hosts. It records bounded layout/text/paint/render CPU-side wall-time intervals
with automatic parent/frame association, then exports an offline frame picker,
recorded call tree and timer totals or Chrome Trace Event JSON. The two native
examples share `--trace-ui=<new-output.html|json>`. Hosts add one frame-entry guard;
business scopes are optional. It requires no system profiler and is disabled by default;
these intervals are not CPU utilization or GPU execution timing.

## Agent integration

The opt-in JSONL interface exposes current UI semantics, paginated snapshots,
changes and controlled commands. Stable owner/version checks and observation
following an action help controllers avoid stale state. Reads and admitted writes
are separate capabilities. Sensitive content, unsupported native surfaces and
unrealized data remain explicit boundaries.

Start with [structured UI access](docs/AGENT-ACCESS.md) and the API inventories:

- [Package support and host boundaries](docs/SUPPORTED-API.md)
- [Core UI](crates/vesta-gpui/SUPPORTED-API.md)
- [Widgets](crates/vesta-gpui-widgets/SUPPORTED-API.md)
- [Native renderer and integration](crates/vesta-gpui-vulkan/SUPPORTED-API.md)
- [Optional native WebView lifecycle and resources](docs/WEBVIEW.md)
- [Workspace model, operations and versioned recovery](docs/WORKSPACE.md)

## Maintain and release

The [architecture map](docs/ARCHITECTURE.md) states dependency direction, state
ownership and host responsibilities. The [refinement programme](docs/REFINEMENT.md)
tracks the remaining API, lifecycle, frame and performance gates; its completion
is separate from release or universal platform support.

The repository is maintained directly as one versioned family. Use its normal
default Cargo target and pinned lockfile; no other product checkout or compiled
artifacts are required. See [ownership](docs/MAINTENANCE.md),
[qualification/package commands](docs/RELEASING.md) and
[candidate notes](release/0.1.0-rc.1.md).

`IMPORT.json` records the preserved import identities. `SOURCE.json` seals a
specific candidate's source bytes; intentional development edits invalidate it
until the next seal. It is an integrity manifest, not a signature or test
result. Registry publication is disabled in the manifests. Browser/WASM
execution, full editor authoring and universal platform/reader compatibility
are not promised by this candidate. Workspace/floating windows, native WebView
and the optional viewport keep their documented limits.

## License

Apache-2.0, with the embedded Material Icons font's original
[license notice](crates/vesta-gpui/tests/fixtures/LICENSE-Material-Icons.txt)
preserved. See [LICENSE](LICENSE).
