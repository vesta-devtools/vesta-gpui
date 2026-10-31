# Device-scoped Vulkan memory ownership

Status: Accepted; device ownership implemented and measured; delivery follows GF05/GF06.
Date: 2026-10-31.

The native two-window collection workload observes about 1.2 GiB of private
process commitment after warmup, even with the recorder disabled. It stays flat
during the measured quiet interval; this is not evidence of a leak. The current
Painter, ScenePainter, framebuffer attachment, capture readback and offscreen
target paths each construct independent default Vulkano memory allocators.
Pinned Vulkano 0.35.2 chooses 256 MiB blocks on large memory heaps. Attribute the
observed cost with an equivalent-work experiment instead of assuming all memory
belongs to the framework or to the driver.

Use one existing StandardMemoryAllocator per GpuContext/device generation.
Painters, scene renderers, attachments and captures borrow its Arc and retain
their own actual allocations/subbuffers. Preserve Vulkano's default type filters
and allocation policy; do not invent a pool service or arbitrary block-size knob.
Surface recovery reuses that device owner; Device recovery creates a fresh one.

Extend the existing worker retirement owner with the allocator Arc. The event
thread must never become its last GPU owner merely because a context is dropped;
the existing release marker and future retirement ordering remain authoritative.
No new public SDK API, memory cache, runtime or Graph/backend policy is added.

Retain the previous same-binary off/stages/detailed matrix and noisy raw samples.
Compare the new balanced native matrix with identical virtual subtree signatures
and work counts, explicit fonts/source/executable identities, ordinary idle
counters and exact-PID process sampling. Check error/cancellation/recovery and
presented/offscreen capture paths affected by the shared lifetime. A lower
process private commitment is not a GPU VRAM measurement or universal bound.
Complete relevant software, standard artifacts, native cleanup, full SDK checks
and CI; immutable Vesta adoption remains a separate explicit delivery gate.

The original balanced six-run baseline and six-run device-owned matrix retain
1,536 equivalent measured frames. A further eight-run allocator-only ABBA
experiment binds 13 registered font inputs and 1,024 equivalent measured frames.
All runs use the one normal standard path, preserve their own raw source and
process identities, and exit normally. The allocator-only comparison and its
limits are recorded in `target/research/native-collection-cost-20261031/ownership-abba/`.
Software, affected native recovery/capture and immutable consumer delivery keep
their own explicit qualifications; measurement alone does not close the programme.
