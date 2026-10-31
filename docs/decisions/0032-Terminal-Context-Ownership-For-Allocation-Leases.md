# Terminal context ownership for allocation leases

Status: Accepted; directed native lifetime and capture paths qualified locally.
Date: 2026-10-31.

Decision 0030 centralizes the device allocator. An ownership continuation finds
two remaining last-owner paths: WindowRenderer declares its context before its
allocator-using paint/target fields, and public CaptureReadback retains a GPU
buffer without retaining the context. A fast worker can retire its context
before those producer fields/readbacks drop, leaving the last allocator pool
release on the consumer thread. Arc memory safety is preserved, but the stated
worker retirement ownership is not guaranteed by timing.

Declare the renderer's context last so its device generation outlives all of its
allocation owners. Give CaptureReadback one private context Arc, also last after
its buffer. Keep Debug support without exposing the context. CPU rgba bytes do
not retain GPU ownership. Reuse the existing Context retirement slot, release
marker and worker; add no resource wrapper, worker or public function.

Extend the existing isolated retirement smoke with a first-frame capture.
Retain its public ticket/readback after App/renderers and in-flight frames have
retired. Observe the weak context still live while that readback is held, then
release it on the former event thread and observe normal worker retirement.
Waiting occurs only after the native loop and App have closed. Record the
original missing-lease failure before implementation. Preserve current
off/stages/detailed memory/idle and unrelated native scenes; only affected
capture/retirement paths require directed continuation.

Root remains on its recorded immutable 4790 revision while current Root jobs
finish. SDK source, exact CI and subsequent immutable product adoption remain
explicit separate delivery gates. No original physical/reader/DPI gate follows
from a lifetime probe.

The original red01 reproduces a completed first-frame readback with the context
already gone after App/renderers retire. The unchanged public capture operation
now retains the context while the result is held; green01 releases it on the
former event thread and observes context retirement on the existing worker.
Both runs record their source, standard executable hash, native PID and exit.
Native01 additionally qualifies Main64/Float64 and two public offscreen PNGs;
capture01 rejects both injected presentation failures, accepts the two fresh
next-presented PNGs and completes another Main64/Float64 sequence. All owned
native processes exit; the original failure remains. Evidence is under
`target/research/gpu-allocation-lease-20261031/`. Software, ordinary builds,
exact remote CI and immutable consumer adoption retain distinct receipts.
