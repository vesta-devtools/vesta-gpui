# Recovery retry observation states

Status: Accepted; directed cancellation and busy-timeout observations qualified.
Date: 2026-10-31.

The allocator cancellation qualification preserves a real false failure in
`target/research/native-collection-cost-20261031/cancel01/`: cancellation pauses,
the worker finishes, explicit retry succeeds, both windows resume with preserved
CPU state and all GPU owners retire, but the smoke report requires a busy retry
rejection. The existing boolean conflates an unobserved gate with a worker that
correctly finished before observation. The 150 ms fixture delay cannot guarantee
event-loop scheduling and must not be extended to hide this race.

Replace that private observation boolean with explicit Unobserved, BusyRejected
and WorkerFinished states. Retain the real retry admission implementation and
its worker-finished guard. A cancellation run may qualify an already-finished
worker, with no claim that it exercised busy rejection. Preserve the separate
timeout fixture's requirement to observe a genuinely rejected busy retry and
qualify that path alongside cancellation. Every retry fixture still requires a
pause, exactly one accepted explicit retry, restored identity/input and worker
retirement. A retry accepted after the worker finishes is correct even if an
earlier read saw it busy.

Use the existing native smoke and recovery interfaces. No new public API,
test-only worker handshake, arbitrary delay, renderer or production retry policy
is introduced. Keep prior qualified capture/device/surface scenes and repeat
only the affected observation paths. Software, immutable consumer delivery and
the full programme retain their own gates.
