# Vulkan recording and observation ownership

Status: Accepted; local complete Vulkan source/software/affected-native scope qualified.
Date: 2026-10-31.

The full renderer review retains one owner for each real boundary: GPU context,
asynchronous creation/update/recovery, submission and real completion, capture,
CPU preparation, checked command recording, resource caches and observations.
No new renderer facade, scheduler, telemetry task or abstraction is justified.

Remove the private `recording.rs` module that only delegates its whole call to
`checked_pipeline::recording::record`. Both presented and offscreen paths call
that same existing recording authority directly. Move its private single-scene
tuple alias to `scene_views`, beside the single/batch input admission it names.
The checked module remains the only unsafe-permitted shader/draw boundary.
Public signatures and the existing presentation/capture pipeline are unchanged.

RetirementMonitor's caller-supplied observation budget currently uses unchecked
Instant addition. First reproduce Duration::MAX in its existing ownership test;
then reject an unrepresentable deadline with RendererError at retirement_budget.
A rejected or expired observation never changes resource ownership or grants
permission to discard pending GPU work. Fixed internal creation/recovery budgets
remain unchanged. No arbitrary new duration cap or helper abstraction is needed.

Original red02 reproduces the public-budget panic in the existing retirement
ownership contract. Focused01 passes after the checked deadline and recording
cleanup; the original red remains separate from final qualification.

Qualify all current renderer exports, feature branches, actual consumer entry
points, error classification, cache bounds and resource lifetimes. Map software
and incremental native evidence to their exact inputs. The new presented-capture
qualification is defined by decision 0026; earlier controlled faults and image /
scene / timestamp evidence keep their original identities. Source review and a
healthy GPU run cannot certify physical device-loss deadlines, display latency,
other platforms or current immutable product adoption.

Final full01 passes 234 contracts/39 harnesses and the CPU smoke, two compile-fail
docs, all-feature and base strict checks, six Node groups, format and ordinary
standard builds. The source/runtime audit covers 24 production modules in nine
responsibility groups and 18 actual Host entry files. Native01 qualifies both
rejected-then-fresh captures and 128 later submissions, real completion before
rebuild, normal owned-process retirement and executable-lock release. Its
standard fault binary identity remains separate from the restored ordinary
artifacts. See the final checkpoint under `target/research/vulkan-family-20261031/`.
