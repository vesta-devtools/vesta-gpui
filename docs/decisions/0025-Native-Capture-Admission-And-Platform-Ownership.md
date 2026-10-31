# Native capture admission and platform ownership

Status: Accepted; local full Native source/software/native scope qualified 2026-10-31.
Date: 2026-10-31.

## Scope and findings to reproduce

Continue the finite GF02 Native adapter review and its GF04/GF05 capture/resource
boundaries. Map the actual 17 production modules, public contracts and existing
Standalone/Agent/Builder callers. Preserve earlier text/input/stdio evidence.
No new platform, event loop, dispatcher, author model or background service.

Standalone still duplicates the shared capture region and sensitivity rules.
Both implementations currently check redacted bounds against the requested
logical crop, although floor/ceil physical readback covers complete pixels
outside it. Reproduce a crop whose outward-rounded last pixel intersects an
adjacent redacted control at fractional scale. Separately preserve evidence if
f32 intermediate multiplication loses a conservative crop edge. These are
source leads until the original runtime reds are recorded.

Red01 reproduced both shared failures (right edge 4002 instead of 4003 and an
accepted rounded pixel over redacted bounds) and Standalone reaching renderer
admission for the same sensitive crop. Focused01 preserves an incomplete private
schema-constant rename after a guarded transform failed. Focused02 preserves a
fixture that incorrectly expected the original unnormalized Rect: Core clipping
already reconstructs its width from stored edges. Keep the original physical
edge assertion; compare `logical` to the established clipped Rect contract.

## Decision

`UiCaptureGeometry` remains the one capture geometry/admission owner. Preserve
its public fields/signatures and the existing 0.25–4 scale, 4096 edge and 4M
pixel readback limits. Reuse Core Rect/Viewport validity. Preserve the current
target-extent convention used by the renderer: ceil of the f32 viewport/scale
product. Explicitly reject nonrepresentable physical extents rather than rely
on saturating integer casts. This does not redefine native surface dimensions.

Calculate inner crop floor/ceil edges in f64 from the current Core logical
edges, then clamp to those target extents before checked u32 conversion. Keep
`logical` as the requested clipped Rect for existing binding comparisons.
Sensitivity admission instead compares redacted visible bounds with the actual
covered physical pixels, using f64 comparisons. This deliberately rejects an
adjacent sensitive region touched only by a rounded readback pixel. Preserve
half-open touching-edge behavior and the incomplete-snapshot rejection.

Make Standalone use `UiCaptureRequest::geometry` for initial and subsequent
sensitivity/geometry admission and `UiCaptureGeometry::new` for offscreen target
budgeting. Convert its checked output to the existing renderer CaptureRegion;
delete the private parallel region/requested-rect/sensitivity implementations.
Store the existing request/geometry in its private binding, following the actual
consumer contract. Renderer allocation/completion/PNG encoding remain Host-owned.
Presented crops budget only the crop; offscreen targets budget the whole target.

Geometry admission is not instance/window/revision authorization: Hosts validate
the requested source before relayout, then reuse geometry checks against later
snapshots to cancel invalid pending captures. Context leases are optional stale
automation preconditions, never author permission. Make these limits and native
service lifetimes explicit. The stdio worker calls serialization on its I/O
thread; a Host may do bounded capture waiting/encoding there. It cannot enforce
a deadline inside a blocking callback or stdout write. Dispatch must enqueue
without blocking the UI path; no new worker/runtime or cancellation fiction.

Name the existing 64 source-step and 256 expanded UiInputStep limits once and
project those same constants to the existing discovery JSON. Expanded steps
are not emitted event count: key presses add modifier/key bookends. Keep pinned
winit serde as the named-key authority. Preserve legacy stateless IME publication
and RetryRender's Host-qualified F5 intent, documenting both rather than adding
a replacement generic service. `changes_ui` is an ordinary invalidation hint;
Hosts must still dispatch/reconcile native lifecycle events when it returns false.

## Qualification

An explicit existing `--ui-query-stdio` / `--ui-control-stdio` launch may also
enable the same public worker during the existing hidden/nonactive frame probe.
The probe alone still starts no stdin worker. This fixes flag composition so an
owned controller can verify actual offscreen capture and normal Main/Float
retirement without a visible window, OS input, new flag, example or executable.
Only query/capture operations are used for this qualification; local fixture
preparation and control-mode admission are not physical gestures.

Full01 passed all software/build checks, then red02 preserved actual standard
startup rejecting the explicit stdio/probe combination before renderer creation.
Complete the composition in the argument validator as well as startup. One
private flag parser supplies existing query/control intent to initial state,
startup and probe admission; control wins when both existing opt-ins are present.
Final qualification must use the completed source, not full01's earlier green.

Preserve original pure geometry/sensitivity reds and the existing Standalone
composite failure before changes. Tests protect conservative pixel admission,
redaction across the Core/native boundary, failure limits and pending binding
revalidation; no pixel snapshots or generic getter tests. Qualify the full Native
surface and real callers, then focused packages, full SDK/docs/feature/strict/
tool/format checks and ordinary standard builds. Run only affected existing
hidden/nonactive integration and public capture/stdio scenarios, with actual
source/artifact/process identity and normal retirement. Do not inherit physical
input, reader, DPI, speed, Vulkan full-family or immutable product adoption exits.

Final full02 passes 234 contracts/39 harnesses and CPU smoke, two compile-fail
docs, all/base strict, six Node groups, format and ordinary standard builds.
The actual standard executable completes hidden/nonactive Main64/Float64 and
two public offscreen readbacks, each a validated 10x10 PNG. Seven protocol
requests are limited to windows/window_state/snapshot/capture; normal exit,
observed child retirement and both executable locks are verified. Original
red01/red02 and intermediate qualification failures remain under
`target/research/native-family-20261031/`; no earlier green substitutes for the
final unchanged runtime inputs. Remaining programme/adoption gates stay open.
