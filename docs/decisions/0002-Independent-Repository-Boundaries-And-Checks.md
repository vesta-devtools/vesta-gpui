# Independent repository boundaries and checks

Status: Accepted decision; implementation and checks recorded separately.
Date: 2026-10-30.

Keep the existing eight-library dependency direction. The base UI library owns
UI contracts, widgets project those contracts, winit owns native platform
adaptation, Vulkan owns rendering, and the workspace model remains independent
of its UI projection. Applications own example data and acceptance orchestration.

The independent import omitted four modules required by the native profile
verifier. Restore their existing validation logic without weakening evidence
checks. A focused CLI regression must cover module loading, accepted synthetic
fixture data and rejected corrupt evidence. Synthetic fixtures are never native
measurements. CI runs the source/tool contracts and the existing Rust checks.

Move the early UI-1 editor outline out of the base library into a runnable
example. Its fixed IDs belong to that example, not the reusable API. Move the
read-only monitor work-area query from Vulkan to the winit platform adapter.
These two preview API removals are explicit in the next-candidate migration
notes. Do not introduce a renderer-to-adapter dependency merely to preserve an
incorrect re-export. No final 0.1 API release has been tagged.

The Win32 monitor query keeps its exact behavior and documented safety argument.
Only that small winit platform module may use unsafe code; all other adapter
modules deny unsafe. The Vulkan exception contracts then cover only its checked
graphics calls. Workspace/window state and native event authority do not move
into the renderer or the base UI library.

Split Standalone's main/float presentation, floating-window events, startup state
and inline test module into cohesive host modules. Keep the same App owner,
typed Action dispatch, input origin, stable IDs, GPU ownership and event ordering.
This is a source reorganization, not a new event loop or window manager. Retain
the existing contract tests and prior acceptance evidence; do not restart the
historical native acceptance programme for moves alone.

CI uses the pinned Rust toolchain, one ordinary Cargo target, bounded jobs and
no GUI. Source integrity checks belong to a new candidate seal after the edits;
the published rc.1 archive and qualification remain immutable. CI compilation
and synthetic tool tests do not certify physical IME, reader speech, monitor
changes, GPU pixels or consuming-product integration. Vesta adoption remains a
separate migration.
