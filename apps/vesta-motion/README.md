# Vesta Motion — GPUI application example

This source SDK includes the complete Motion application and its public GPUI dependencies. Build from the extracted SDK root on Windows with the pinned Rust toolchain, a compatible Vulkan driver/loader and system fonts (Segoe UI, Segoe UI Symbol, Microsoft YaHei). No Vesta product checkout, engine, Graph asset or model account is needed.

```powershell
cargo build --locked -p vesta-motion --bin vesta-motion
.\target\debug\vesta-motion.exe
.\target\debug\vesta-motion.exe --tour
```

Use **1–6** to switch between Orbit Study, Wave Field, Orbital Sphere, Double Helix, Kinetic Bloom and Prism Spiral. **T** starts the demo reel and **Space** toggles playback. Drag parameters or the timeline, change palette or toggle layer visibility. Each study retains its own parameters; Reset affects only the selected study.

The workspace, buttons, sliders, lists, focus/input and custom timeline graphics use public Vesta GPUI APIs. Geometry and animation are host-owned display previews. Keyframes and curves are read-only; author save/undo, object manipulation and export are outside this example. Read the local [source guide](GPUI-GUIDE.md), [view](src/view.rs), [state](src/model.rs) and [native host](src/app.rs).

The retained [interface concept](design/interface-concept.png) is a design reference.
It is not a native screenshot or evidence that the illustrated authoring features
are implemented.

## Verification

```powershell
cargo test --locked -p vesta-motion
cargo clippy --locked -p vesta-motion --all-targets -- -D warnings
.\target\debug\vesta-motion.exe --smoke --hidden --study=prism-spiral --capture-dir=target/motion-smoke
```

Smoke exercises all six public gallery actions and playback/seek/slider controls, then repeats them after a real minimum-window resize. Three unmodified native GPU framebuffer captures include both layouts. It exits normally after 105 successful frames; errors and timeouts fail the run. `--hidden` creates no visible/focused window. This is source-controlled input, not physical input qualification.

The default smoke budget is 40 seconds. An explicitly slower software Vulkan worker can pass `--smoke-timeout-seconds=300` (40–300, only with `--smoke`); successful frame/control requirements stay identical. Normal interactive startup has no deadline. Software Vulkan CI evidence does not certify hardware performance, IME, DPI or screen readers.

The SDK root `SOURCE.json` identifies the exact included source and applications. Source compilation, runtime checks, publication and platform qualification retain their separate results.
