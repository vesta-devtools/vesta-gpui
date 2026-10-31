# Presented capture and submission outcome

Status: Accepted; local source/software/affected-native qualification complete.
Date: 2026-10-31.

## Finding and ownership

Continue the complete Vulkan-family review in GF02/GF04/GF05. A recorded
next-presented capture currently survives the handled OutOfDate branch. That
branch returns no submitted frame, but the next completion poll may publish its
old pixels before a later accepted presentation. This is a source lead until a
controlled native reproduction is preserved.

The original red01 now reproduces this in both actual hidden Main and Float:
each rejected first presentation returns a successful `next_presented` PNG.
The second request and all 128 later workload submissions succeed; the owned
native process exits normally. The failure is capture outcome, not inability
to recover the renderer. Original source/artifact/process receipts are retained.

The renderer owns submission outcome, the staging buffer and completion. The
Host owns snapshot identity and publication. Neither may invent success on
behalf of the other. Keep CaptureState as the sole capture producer and
Submission as the sole owner of submitted work and its real completion fence.

## Decision

If presentation reports OutOfDate after capture recording, resolve that capture
with the same typed error. Do not publish or rebind its pixels, requeue the old
buffer, retire a healthy renderer, or release submitted GPU resources early.
The existing retry/reconfiguration path remains responsible for a later frame;
the caller may make a fresh capture request after that recovery.

Use the existing fault-injection feature and hidden/nonactive frame probe to
qualify the boundary through the existing public stdio worker. An explicit
private `capture` value of the existing probe-case option requests one injected
presentation outcome and one fresh successful request per Main/Float window.
There is no new flag, public API, application, thread, target or renderer.
The fixture prepares no physical input and makes no physical display claim.
Injection follows an actual healthy GPU submission/presentation and preserves
its real completion fence; it qualifies the controlled error path only.
The existing I/O worker records fixture-only typed outcomes in the bounded
trace. The maintained frame-report verifier requires rejected-then-successful
outcomes for each window as well as all later real workload submissions. The
native regression therefore remains reproducible from maintained source; it
does not depend solely on an untracked controller's assertions.

Preserve the original failed result, then prove typed failure, fresh successful
readback, continued Main/Float submission, normal retirement and frozen source /
artifact / process identities. Run relevant software checks before full SDK
closure and restore the ordinary standard build after fault qualification.
This finding alone does not close the complete Vulkan family, physical gates,
current product adoption or the refinement programme.

Final native01 returns typed `present` failures for the first Main/Float
requests, genuine next-presented PNGs for the fresh requests, and all later
Main64/Float64 submissions. The owned standard fault build exits normally;
completion fences and descendants retire. Seven report checks accept the real
report and reject outcome/source/completeness substitutions. Full01 passes all
234 contracts, docs/feature/strict/tool/format checks and restores both ordinary
standard builds. The final family checkpoint also includes decision 0027 and
the complete ownership audit; no physical or product-adoption gate is inherited.
