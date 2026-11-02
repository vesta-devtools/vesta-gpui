# Transactional native trace work and cancellation

Status: Accepted; scoped implementation and finite consumer delivery qualified, 2026-11-03.
Date: 2026-11-02.

The native host currently conflates import, analysis and finite image export in
one busy flag. Navigation against the previous session can replace a pending
import; a failed task discards the last report, and Escape has no explicit work
cancellation contract. Refine this private host state, retaining the same one
bounded worker, cooperative token and generation fence. No scheduler, framework
runtime or second analysis engine is added.

Use explicit idle/import/analysis/capture activity. Import owns its requested
path until completion, explicit cancellation or a newer import; old-source data
commands cannot cancel it. Global search editing remains possible and applies
to the newly admitted source when import completes. Busy work offers an explicit
Cancel action; Escape cancels work unless composition owns the key.

Keep the last validated immutable session/report. Before analysis-affecting view
changes, keep one source-bound view checkpoint, not a per-frame history. Failure
or cancellation restores the completed view and retains the search draft;
another Enter may apply it. Successful publication consumes the checkpoint.
Import failure leaves the previous source explicit in the status. A successful
source load commits a consistent canonical report before optional desired-panel
or draft-filter analysis; cancelling that follow-up never restores another file.
When import supersedes a pending analysis, preserve the requested global panel
before restoring the last validated source checkpoint. Failure/cancellation
still restores that checkpoint; successful load applies the saved panel intent
to the newly committed canonical source. A checkpoint is not the user's latest
pending panel choice.

Cancel sets the current cooperative token, drops an unsent request and advances
the generation before returning control. Old success/failure messages cannot
alter session, report, status or activity. Import start cancels pointer preview
even when reading fails. Closing during pending work retains normal worker/GPU
retirement and the existing finite acceptance deadline.

Qualify actual missing/malformed input, cancellation with an observed stale
reply, replacement/coalescing, deferred search and normal close while work is
pending. Preserve failure receipts and unchanged original evidence. Synthetic
input timings and command-origin fixtures do not prove workload performance,
physical input, Reader speech, DPI or calibrated CPU/GPU timing.

The seven-state real-worker fixture and directed affected regressions are
recorded in [TRACE-PLAN](../TRACE-PLAN.md). The existing one bounded worker and
generation fence remain authoritative; this adds no public library or protocol.
