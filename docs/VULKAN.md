# Vulkan renderer ownership and public contracts

Scope: the current `vesta-gpui-vulkan` source family, including its optional
`viewport` and explicit `fault-injection` branches. Qualification remains in
[the refinement ledger](REFINEMENT.md); this document defines responsibilities,
not universal platform or driver guarantees. [Host composition](HOSTING.md)
and [the candidate API inventory](../crates/vesta-gpui-vulkan/SUPPORTED-API.md)
retain their own support and evidence limits.

## Production ownership

| Owner | Modules | Responsibility |
| --- | --- | --- |
| Window renderer | `lib` | Window/swapchain state and one render attempt; delegates construction, preparation, recording, submission and retirement |
| Context and failures | `gpu_context`, `gpu_failure`, `renderer_error` | Shared logical device generation, first typed DeviceLost, structured failure classification |
| Factories and recovery | `creation`, `reconfiguration`, `recovery` | Native surface preparation, existing worker, one bounded factory job per recovery seed, result delivery and cancellation |
| Submission and retirement | `submission`, `retirement`, `frame_retry` | Acquired/submitted owners, real completion, worker handoff, observation and window-local retry deadlines |
| CPU paint and resources | `preparation`, `painter`, `glyph_atlas`, `image` | Exact device-free paint preparation, checked image fallback, immutable bounded caches and typed upload batches |
| GPU recording | `checked_pipeline`, `checked_pipeline/recording`, `targets` | Embedded validated shaders, checked draw calls, target attachments and the shared presented/offscreen command path |
| Capture | `capture`, `capture/offscreen` | One pending readback per renderer; staging buffer, outcome and completion; independent offscreen target |
| Observations | `frame_profile`, `gpu_profile` | Opt-in CPU stage totals and bounded consuming GPU timestamp observations; no scheduler or telemetry worker |
| Optional scene projection | `scene_views`, `viewport`, `viewport/labels` | Bounded view admission, safe immutable scene projection and prepared numeric labels |

The old private `recording.rs` delegating wrapper is removed. The existing
checked recording function is the only recording authority. The private
single-scene tuple alias lives with single/batch scene admission. No public
facade, event loop, task runtime, application state or author model is added.

## Device memory ownership

One private `StandardMemoryAllocator` belongs to each `GpuContext` device
generation. Painters, scene rendering, framebuffer attachments, readback and
offscreen targets borrow that Arc and own their actual allocations. Surface
recovery retains the device allocator; Device recovery constructs a fresh one.
The existing worker retirement owner retains the allocator until the context
producer releases its fields. There is no separate pool service, per-window
allocator or public allocator configuration. Vulkano 0.35.2 default block/type
policy is preserved. [Decision 0030](decisions/0030-Device-Scoped-Vulkan-Memory-Ownership.md)
records the measured ownership change. Private process commitment is distinct
from GPU VRAM and allocator retention is not itself a leak.

## Construction, updates and recovery

`RendererCreation::start` prepares instance/surface capabilities on the native
event thread and hands device/renderer work to the existing GPU worker.
`with_gpu` shares an existing healthy context. Poll `RecoveryPoll` and retain
the creation handle until `worker_finished`; cancellation or an observation
timeout does not mean the worker or a driver call has stopped. Late resources
are retired on that worker. A `RecoverySeed` retains instance/physical-device /
queue-family/worker identity, never the lost logical device or author state.

`WindowRenderer::new` / `with_gpu` and `GpuContext::new` remain synchronous
compatibility entry points. A live interactive Host should use asynchronous
creation. Current SDK Standalone and Agent do so; Root Builder main/Workspace
windows also do so, while its catalog popup still uses the compatibility path.
These Root callers are inspected inputs, not evidence of adoption of this SDK
revision or completion of their budget/fault gates.

`RendererUpdates` owns one asynchronous reconfiguration. The Host passes its
current `TargetConfiguration`, retains its context/monitor before handing off
owners, polls completion and publishes only the current native-window/target
configuration. A stale completed configuration may be reused as an owner but
cannot make current input ready. `blocked` concerns target readiness;
`presented` acknowledges a current successful frame. Reset for recovery only
after capturing the recovery seed and respecting `worker_finished`.

`RecoveryJob::start` / `restart` use explicit `RecoveryTarget` owners and surface
or device scope. A rejected handoff returns every target in `RecoveryRejected`;
the caller retains/retries them. One seed's existing worker serializes factory
work and retirement. Busy or timed-out observation grants no permission to start
another competing factory. Recovery may replace GPU resources; text, document,
workspace and other durable CPU state remain with the Host.

## A render attempt

The Host first prepares coherent CPU layout, paint, hit and input data. It calls
one of `render`, the legacy optional-scene variant, explicit-target-mode variant
or bounded scene-view variants. `Ok(Some(RenderOutput))` means a successful
submit/present call, not observed physical display. `Ok(None)` means no new
published frame: zero size, a retry deadline, pending GPU completion or target
configuration, or unavailable/outdated swapchain image. Schedule the returned
`FrameRetry` or factory deadline and keep the Host's existing CPU state.

The renderer checks the preceding real completion, prepares CPU paint, acquires
with zero timeout, records and installs an owned `Submission` before flushing.
There is at most one in-flight submission per renderer. Normal rendering does
not wait for a fence on the UI thread. `finish=true` is an explicit blocking
compatibility/measurement path and is unsuitable for ordinary interactive frames.
Its observation timeout preserves unfinished owners.

`RendererError` supplies `code`, `stage`, typed `RendererFault`, message and
optional element identity. Classification follows typed causes, never text.
DeviceLost is shared and sticky across a context; a surface failure remains
local. Ordinary rendering errors latch the affected renderer. New scene-batch
admission errors (`native_vulkan_scene_batch_rejected` / `frame_scene_mode`)
reject caller input without retiring the renderer. Legacy single-scene methods
retain their earlier error policy and viewport clamp; do not infer the newer
batch rejection guarantee for them.

The acquired image stays owned if recording fails. Submitted commands and their
keep-alives stay owned through observed completion, including handled OutOfDate.
Completion dependency panics become structured errors while borrowing owners;
they do not establish DeviceLost or fake completion. A Host watchdog bounds
observation, never driver execution. Retry delay grows from 1 to 16 ms; successful
progress resets it. The Host owns wakeups, visibility and recovery policy.

## Paint, images and optional scenes

`prepare_paint` is the production CPU painter with no Device. The caller supplies
fonts, text/rasterizer contexts, image resolution, viewport, options and an
explicit maximum image dimension. `PreparedPaint::output`, `batch_count` and
`vertex_count` describe CPU batches before atlas/upload merging, not GPU draw
counts. The native/offscreen paths use that same preparation.

Image failures are local structured `ImageDiagnostic`s and fallback geometry;
allocation, mapping and submission failures remain renderer failures. An image
must have nonzero valid RGBA dimensions, fit the supplied device dimension and
stay within 32 MiB. The immutable texture LRU retains at most 4096 entries and
32 MiB of pixel payload per painter. Pointer hits avoid hashing; content hashes
also require complete pixel equality. The glyph atlas retains up to four
1024-square pages and 4096 entries, recycles between frames and replaces dirty
GPU pages. Submitted commands retain old descriptors. These are cache bounds,
not a total application-memory guarantee; input trees, geometry, temporary
buffers and the number of windows remain caller-controlled.

Optional `SceneView` uses physical target pixels. `validate_scene_views` and both
render/capture batch paths share admission: at most `MAX_SCENE_VIEWS` (32), valid
target-contained viewports, representable clips, finite camera projections and
pairwise-disjoint effective visible clips. Touching edges are allowed; empty
clipped views draw nothing. Clipping preserves viewport aspect. Explicit
`TargetMode::Scene` denotes depth/scene capacity and can carry UI-only content;
actual scene content on a UI-only target is rejected.

Scene data is borrowed from `vesta-viewport`, never a second scene model. Mesh
indices/counts, vertex-color cardinality/finite values and material policy are
checked before draw; the Host supplies geometry appropriate for the current
display/lighting policy.
Viewport input additionally checks finite position/normal values in the
same mesh packer and translation/color/range in the object packer; its corrected
failure stage is `viewport_vertices`. The [Viewport review](VIEWPORT.md) records
its affected native scene qualification and the original clipping failure.
Opaque depth precedes sorted translucent surfaces. Numeric labels require the Host's immutable
ten-cell RGBA digit atlas and explicit valid style/capacity. Missing atlas means
no labels. One cached atlas is retained, each edge at most 8192; this separate
scene resource can be larger than the 2D image-cache budget. Uploads precede
render passes; shared depth/MSAA attachments remain command-owned during rebuild.

## Capture outcome and identity

`CaptureRegion` admits nonempty target-contained regions, each edge at most 4096
and area at most 4,194,304 pixels. Only one queued or recorded capture is allowed
per renderer. `capture_next_frame` requests a future successfully submitted
presentation; a recorded capture whose presentation reports OutOfDate fails with
that same error. It cannot publish old pixels under a later successful frame.
Request a fresh capture after ordinary recovery. `cancel_capture`, fatal failure
and renderer closure resolve their owned requests with errors.

Offscreen variants allocate an independent target, use the same paint/recording
path and optional scene admission, and apply the capture budget to that entire
target as well as the crop. They never claim to read the native compositor or
child browser surfaces. `CaptureReadback::offscreen` and `frame` retain their
renderer scope; the Host separately validates source instance/window/revision,
sensitivity, current resources and successful presentation identity.
An offscreen `frame` is the current presentation count plus one; it is not a
unique capture ID and does not increment the successful presentation count.

Only actual GPU completion releases a successful readback. Wait for
`CaptureTicket` and map `CaptureReadback::rgba` on a worker; native color order
is normalized to RGBA8/sRGB. Timeout consumes the waiting ticket, not GPU
ownership. The renderer's completion/retry path or Host cancellation still owns
pending work. PNG encoding and protocol responses remain Host-owned.

## Observation and shutdown

CPU profiling exposes the last successful renderer call's nine stage intervals,
UI counters and explicit-finish flag; it clears at a new attempted frame.
GPU profiling is default-off. `GpuProfileKey` identifies generation and command
recording attempt, independently of Host author identity or physical display.
Timestamps bracket upload/scene/UI commands and may include device scheduling
stalls. Raw ticks, capability/availability and ambiguous-wrap rejection remain
explicit; CPU and GPU clocks are not aligned by an invented offset.

`GpuProfileStream` clones consume the same queue of at most 256 observations.
Busy/poisoned telemetry returns no drain; contention or capacity drops increment
the dropped count. No telemetry worker or redraw is created. Retain the stream
through retirement to collect late completed samples.

Drop/handoff all Host renderers, creation/update/recovery tickets and GPU context
owners before observing `RetirementMonitor` off the active event loop. Its
`pending` includes worker activity and pending retirement. `observe` returns a
stored error or observation timeout; an unrepresentable caller deadline is a
`retirement_budget` error. None authorizes releasing unfinished GPU owners.
The existing worker owns retired resources and never joins the UI thread.

An externally held `CaptureReadback` is also a GPU owner. Its private context
lease outlives its staging buffer, so the result can safely outlive its renderer
without transferring the final device allocator release to the consumer thread.
Drop that result before expecting complete context retirement; `rgba()` returns
ordinary CPU bytes and adds no GPU lease. The renderer likewise drops its
allocation owners before its terminal context field. Both paths reuse the
existing worker and retirement monitor; see
[decision 0032](decisions/0032-Terminal-Context-Ownership-For-Allocation-Leases.md).

## Evidence limits

The current decisions are [0026](decisions/0026-Presented-Capture-And-Submission-Outcome.md)
and [0027](decisions/0027-Vulkan-Recording-And-Observation-Ownership.md). Original
capture and deadline reds remain in `target/research/vulkan-family-20261031/`.
Final full01 passes 234 contracts/39 harnesses plus the CPU smoke, two
compile-fail docs, all/base strict, six tool groups, format and ordinary standard
builds. Actual native01 uses the standard fault build in hidden/nonactive
Main/Float: each rejected request fails, each fresh one returns a real PNG,
and all 128 later workload steps submit. Seven report checks accept the actual
trace and reject substituted failure/source/completeness. Real completion before
rebuild, normal process/descendant exit and executable locks are verified; the
ordinary standard build is restored afterward. The final checkpoint signs this
local source family, preserving the original reds and a malformed empty process
observation as a rejected receipt. Earlier shader/image/scene/timestamp /
creation/recovery/retirement qualifications retain their recorded inputs. This
review does not turn controlled notifications into physical device-loss tests,
sign new input/reader/DPI/display-latency gates, close Viewport's separate family
review or claim current immutable Vesta adoption.
