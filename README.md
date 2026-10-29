# Vesta GPUI

A Rust UI foundation for native desktop tools, with shared layout, text/input,
widgets, winit/Vulkan integration and opt-in structured UI access for authorized
Agent integrations. The maintained family contains eight libraries and two
application examples. No Vesta product, Graph runtime or model account is needed.

The first source candidate is **0.1.0-rc.1**, targeting Windows x86-64 with
Rust 1.89.0 / Edition 2024. It is a developer preview. Local independent
compilation, 204 automated tests and strict checks have passed; this is not a
final public release or registry package. See [validation scope](docs/VALIDATION.md)
and the distribution qualification sidecar for the exact evidence.

## Run the examples

From this repository root, with Node.js 22+ and the pinned Rust toolchain:

```powershell
node tools/sdk/gpui_source.mjs --verify .
cargo run --locked -p vesta-gpui-standalone
```

Standalone demonstrates controls, layout, Markdown/text, collections, input,
accessibility and Workspace integration. Native rendering needs Windows host
fonts and a compatible Vulkan loader/driver. WebView2 is needed when using the
optional native browser carrier.

```powershell
cargo build --locked -p vesta-motion --bin vesta-motion
.\target\debug\vesta-motion.exe --tour
```

[Vesta Motion](apps/vesta-motion/README.md) contains six native motion studies.
Its geometry is host preview content; keyframe authoring, save/undo and export
are outside the example's scope. Smaller public examples remain integration
references rather than additional application products.

## Agent integration

The opt-in JSONL interface exposes current UI semantics, paginated snapshots,
changes and controlled commands. Stable owner/version checks and observation
following an action help controllers avoid stale state. Reads and admitted writes
are separate capabilities. Sensitive content, unsupported native surfaces and
unrealized data remain explicit boundaries.

Start with [structured UI access](docs/AGENT-ACCESS.md) and the API inventories:

- [Core UI](crates/vesta-gpui/SUPPORTED-API.md)
- [Widgets](crates/vesta-gpui-widgets/SUPPORTED-API.md)
- [Native renderer and integration](crates/vesta-gpui-vulkan/SUPPORTED-API.md)

## Maintain and release

The repository is maintained directly as one versioned family. Use its normal
default Cargo target and pinned lockfile; no other product checkout or compiled
artifacts are required. See [ownership](docs/MAINTENANCE.md),
[qualification/package commands](docs/RELEASING.md) and
[candidate notes](release/0.1.0-rc.1.md).

`IMPORT.json` records the preserved import identities. `SOURCE.json` seals the
current source bytes; it is an integrity manifest, not a signature or test
result. Registry publication is disabled in the manifests. Browser/WASM
execution, full editor authoring and universal platform/reader compatibility
are not promised by this candidate. Workspace/floating windows, native WebView
and the optional viewport keep their documented limits.

## License

Apache-2.0, with the embedded Material Icons font's original
[license notice](crates/vesta-gpui/tests/fixtures/LICENSE-Material-Icons.txt)
preserved. See [LICENSE](LICENSE).
