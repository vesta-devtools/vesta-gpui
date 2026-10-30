# Independent repository ownership

Decision date: 2026-10-29.

This repository maintains the eight GPUI libraries and the Standalone and Motion
examples as one versioned family. The initial source import is identified by
`IMPORT.json`. The original import receipt and source archive are preserved by
the maintainers; they are historical records, not a second live source tree.

The former product-workspace extraction command is retired here.
`tools/sdk/gpui_source.mjs` now seals or verifies this repository's source only.
It cannot import from another checkout, filter out product dependencies or
substitute files during packaging. Source files, notices and lock resolution are
maintained directly in this repository.

Use this project's ordinary Cargo `target` directory. Do not share another
project's compiled target, copy executables around a file lock, or inherit an
external `CARGO_TARGET_DIR`. Package/runtime qualification and the consuming
application's integration qualification are separate results.

All eight packages keep the same release version. A consuming application must
select one immutable Git revision from the
[public repository](https://github.com/vesta-devtools/vesta-gpui) for the family.
During local development it may use a deliberate path override; this
does not constitute a published release. Changing the Vesta application's
workspace, lockfile or package ownership is a separately coordinated migration.

Future commits require an explicitly chosen public author address. Account
verification, repository creation, public push, release tagging and crates.io
publication remain separate actions. Package manifests currently disable
registry publication; a source release does not imply registry availability.

## Ownership inside the repository

Use [ARCHITECTURE.md](ARCHITECTURE.md) for the current responsibility and state
map, and [REFINEMENT.md](REFINEMENT.md) for open work. A boundary change requires
a decision and the resolved architecture check. Public API additions must state
their caller, necessity, owner, lifecycle and failures.

- `vesta-gpui`: stable UI identity, layout, text, input and semantic contracts.
- `vesta-gpui-widgets`: reusable UI projections and typed interaction helpers.
- `vesta-gpui-winit`: native events, IME, accessibility and monitor geometry.
- `vesta-gpui-vulkan`: rendering, capture, GPU resource ownership and retirement.
- `vesta-workspace`: the renderer-independent pane/tab/split model.
- `vesta-gpui-workspace`: its UI projection; it does not own another model.
- `vesta-viewport`: optional preview scene/camera/geometry contracts.
- `vesta-gpui-webview`: optional native browser ownership and local assets.

Demonstration pages and fixed demo IDs belong in examples or applications.
Standalone keeps one App owner and shared Action/input authority. Its startup
state, main/float presentation, float event adapter and acceptance tests live in
separate modules; a physical file move does not establish a second state model.

`.github/workflows/checks.yml` runs source/tool and Rust software checks on
Windows with the pinned toolchain. It creates no GUI and grants no new native
acceptance. Existing contract tests guard library dependency and unsafe-code
boundaries. The four native profile verifier modules are maintained here with
their CLI; importing them from a private product checkout is not supported.
