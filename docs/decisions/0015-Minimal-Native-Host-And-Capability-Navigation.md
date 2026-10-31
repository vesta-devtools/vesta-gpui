# Minimal native host and capability navigation

Status: Accepted; bounded native recipe/navigation qualification completed 2026-10-31.
Date: 2026-10-31.

## Findings

The supported-API documents mix current capabilities with dated additions and
historical exit labels. The Widget document places numeric, scroll and collection
capabilities under compatibility. The package matrix omits the existing native
transport/trace and asynchronous renderer-lifetime entry points. These are
navigation/contract omissions, not reasons to delete qualified APIs or rewrite
historical evidence.

The two full native applications demonstrate many features but carry product
presentation and acceptance orchestration. The existing editor-outline example
does not create a window or show native lifetime/input integration. GF06 requires
a small complete native consumer, not another facade or a second scheduler.

## Decision

Add one genuine Cargo example in the existing Standalone package. It uses that
package's current dependencies and default target, without adding another crate,
public API, dependency, feature or application product. This is an integration
recipe, not a renamed executable or a way around an occupied standard program.
Check that the SDK's standard programs are not in use before building/running it.

The private example owns one window and one multiline editor. Reuse asynchronous
`RendererCreation`, `RendererUpdates`, CPU layout/paint/text geometry and the
native text/clipboard adapter. Keep application text/history authoritative, derive
the complete input projection before renderer acquisition, bound width
reconciliation, suspend geometry-dependent input during target/layout changes,
and retain CPU state across a deferred acquisition. Dispose GPU owners before
observing their existing retirement monitor after the event loop returns.

Use the existing opt-in trace launcher and one frame guard. Ordinary idle waits
do not run a frame timer. A finite hidden smoke drives only this example's native
window and logical adapter input, verifies forward/reverse resize and edit/Undo
coherence, and closes normally. It is not OS input, physical IME/reader evidence,
general device-loss recovery or a reason to repeat unrelated accepted scenes.

Give current capabilities and host responsibilities a clear navigation path,
preserving old heading anchors and dated qualification boundaries. Link detailed
contracts rather than copying executable type/parameter inventories. Record
finite remaining API families and their actual review evidence; an export with
no product caller is not automatically redundant in a reusable SDK.

## Qualification

Compile the actual example with ordinary all-target/strict checks. Run the
finite native case from its normal Cargo example path, record exact source,
executable/PID/creation/exit and trace evidence, verify final GPU retirement and
release owned processes/file locks. Verify documentation links and compiled
public API names. Preserve failed runs and current supported/experimental limits.

The example qualifies its stated integration slice, not every Host responsibility
or GF02/GF06 as a whole. Remote CI, publishing and Vesta immutable adoption retain
their separate requirements. No Root production or other Owner's process changes.

Final frozen full01 passed 217 contracts / 35 harnesses (the new example has a
normal zero-test compile harness), the existing CPU workload smoke,
documentation, all-feature/base strict checks, six Node contracts, formatting,
both standard applications and the genuine example build. Actual final example
`6a0900c6…` / native PID 33224 completed five forward/reverse width steps,
logical adapter text plus Core Undo, CPU/painter geometry checks and normal
GPU retirement/exit0. Its complete stage trace contains nine frame attempts and
114 events. Eleven documents / 157 local links passed before final evidence notes.

Preserve native01's pre-launch PowerShell helper failure, native02's hidden
redraw timeout, check02's strict warning and native03's post-close queued redraw
panic. Native03 originally appeared green from exit/log-summary alone; stderr
review rejected it, and later qualification explicitly rejects panic/error
stderr. Closing callbacks now stop work. Nine actual-report/corruption checks
accept the corrected scopes and reject the old failed/incomplete/foreign data.

Evidence is under `target/research/native-host-recipe-20261031`. The native smoke
is explicit logical input, not physical IME, reader, general device recovery,
all-family API review, remote CI or current immutable Vesta adoption.
