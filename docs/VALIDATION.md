# Validation scope

## Current candidate

The published baseline is `0.1.0-rc.1`; the development tree prepares
`0.1.0-rc.2` with explicit [preview API migrations](../release/0.1.0-rc.2.md).
Package API versions remain `0.1.0`. This is a Windows native 2D developer preview, with
optional experimental Workspace, native WebView and viewport capabilities.

Source integrity, Rust compilation, automated contract tests, native operation
and consuming-application integration are separate evidence. A successful source
seal or compiler run cannot certify physical IME input, screen-reader speech,
cross-monitor DPI changes or the absence of all visual defects.

See [the release procedure](RELEASING.md) for the exact independent commands.
Actual local logs are written under `target/qualification/0.1.0-rc.1`; the
distribution sidecar records their outcomes and the exact source identity.
Absence of an outcome means it has not been qualified. The source is hosted at
[vesta-devtools/vesta-gpui](https://github.com/vesta-devtools/vesta-gpui).
There is no final 0.1.0 tag or crates.io publication. Adoption by a consuming
product is separate from the SDK's own qualification.

The repository now includes a Windows software-check workflow and a focused
native-profile CLI regression. Local fixtures exercise evidence validation and
module loading; they do not provide new native observations. A workflow must
actually run successfully before it can be reported as passing.

## On-demand stage capture — 2026-10-31

The shared native recorder now supports explicit start, stats and a frozen
stop-to-worker export handoff. Native launch capture selects stage detail;
the base start API preserves detailed scopes. Three high-rate measurement/shaping
hooks remain available in detailed mode rather than being deleted.

The final pinned Rust 1.89.0 batch passed all 210 workspace tests, documentation,
all-feature strict checks, renderer-base strict checks, both Node tool contracts,
formatting and both standard example builds. Extended existing contracts verify
detail filtering, retained frame/parent semantics, and background export while a
successor capture is active. Production inputs remained unchanged during the
batch. A two-second hidden native Motion run recorded 100 CPU frame attempts /
2,000 events, with zero loss, unfinished scopes or nesting errors; it is stage
mode, not a repeat of the older detailed capture or an Agent latency result.
Local command/source receipts are under
`target/research/agent-frame-trace-20261030/sdk01`. This outcome paragraph is a
post-qualification documentation-only update. Consumer adoption and native Agent
verification remain separate.

## Frame capture successor — 2026-10-30

ADR-0004 extends the initial timer recorder with frame identity, inherited call
parents, per-frame analysis, an offline report and the shared native launch path.
Its qualification is recorded separately from the first timer prototype below;
old passing counts are not transferred to changed code. The final pinned Rust
1.89.0 workspace all-target/all-feature run passed 210 tests, 0 failures and 0
ignored across 30 harnesses. Doctests, all-feature strict checks, renderer-base
strict checks, both Node tool contracts, formatting and both standard example
builds passed. Extended contracts protect frame/parent inheritance, unframed
worker isolation, clipped/overlapping child accounting, escaped offline export,
post-deadline completion labels and non-overwriting post-run file export.

The headless JSON and HTML captures each contain ten CPU frame attempts and 70
events. A standard-build native Motion process, hidden with a two-second observer,
captured 94 CPU frame attempts / 11,235 events; 81 attempts contain the successful
CPU present-submission marker. No events were dropped, unfinished or incorrectly
nested. Frame 15 was selected in the actual generated report and its renderer
children expanded in a browser. Its 132 recorded scopes include layout, text,
paint, input/accessibility projection and Vulkan phases. The observer exited
normally; the preview tab/server were closed. Local receipts are under
`target/research/frame-trace-20261030/run02`.

An earlier run failed the added late-completion assertion while its library had
already been compiled before the deadline fix; it is retained as a failed,
source-changing attempt, not a qualified frozen build. The final run's production
source guard remained unchanged. This section was updated afterwards to record
actual outcomes. Consumer deployment, physical presentation, GPU execution timing
and the original Agent divider defect remain separate; Standalone's new frame
entries were built/tested, not claimed as additional native interaction scenes.

## First timer prototype software checks — 2026-10-30 (historical)

The additive trace implementation passed the pinned Rust 1.89.0 workspace
all-target/all-feature suite: 208 passed, 0 failed, 0 ignored across 30 harnesses.
Three new public capture contracts and one abandoned-storage lifetime contract
protect lifecycle, bounds, thread/generation isolation and non-content export;
the existing production paint test additionally checks renderer phase bridging
without enabling the separate CPU profile API. Documentation checks and strict
all-feature and renderer-base checks passed. Both source/tool Node contracts and
format checks passed.

The real headless example exported 70 events, 0 losses and 0 unfinished scopes,
including host width-step/skip markers and automatic layout/text/paint/hit stages.
It uses synthetic width steps, not physical input. There is no native frame-time,
GPU execution or live Agent performance qualification in this result. Local
command/source receipts are retained at
`target/research/shared-ui-trace-20261030/run03`; earlier interrupted/failed attempts
are retained separately. Final source integrity and standard example builds are
recorded in the same parent evidence directory. This new work is not certified
by the earlier public CI run or deployed into an already running product.

## Historical evidence

Links described as historical context in the imported API inventories refer to
the earlier source-workspace acceptance programme. The original reports remain
with the maintainers at their recorded dates and hashes. They are not bundled
here, cannot be fetched from a private product repository by public users, and
are not independent verification of this candidate. The import receipt hash in
`IMPORT.json` preserves the exact original inventory identity.

Those reports include text/focus/menu, Markdown, layout, basic controls, numeric
editing, scrolling, collections, image/icon resources, toolbar/breadcrumb,
temporary editors, suggestions and command-center work. API behavior is described
in each package's current `SUPPORTED-API.md` and its implementation/tests.
The old product's Handoff and broad widget roadmap are not release requirements
or promises of unimplemented product functionality in this repository.

The earlier product accepted some remaining manual interaction checks by an
explicit human waiver. That decision is retained as historical acceptance; it
does not create new physical measurements or implement unsupported capabilities.
Known software defects, independent build failures and unsupported APIs cannot
be waived by source packaging.
