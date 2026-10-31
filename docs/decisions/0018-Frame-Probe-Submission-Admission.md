# Frame probe submission admission

Status: Accepted; local implementation and qualification completed.
Date: 2026-10-31.

## Evidence and scope

Workspace-model/full03 passed all software checks and ordinary builds. Its
native01 then exited normally and completed both 64-step sequences, but the
unchanged report verifier correctly rejected Main step zero: one renderer
attempt returned before readiness, without a successful submission. The trace
and rejection remain original evidence. Native asynchronous resize/readiness
may legally defer a frame; the probe advanced its accepted-step counter anyway.

This is a GF03/GF06 fixture admission defect encountered during the model's
native caller migration. It is not evidence of an atomic-model failure, a
renderer fault or permission to weaken the existing successful-frame exit.

## Decision

Reuse the probe's existing per-window presented snapshot stamp. For ordinary
inputs/documents/focus, only a new published presentation qualifies a step.
Record every draw initially as `reflow.attempt`, and rename its existing span
to `reflow.step` only after admission. An unsubmitted attempt keeps the same
logical index and uses the existing warm-up/redraw path to retry. Its complete
trace remains visible, with an await-submission marker. Keep the existing
100-second overall deadline, event budget and deliberately deferred/budget
case contracts. Do not add a second renderer state or scheduling abstraction.

The verifier must still require 128 accepted steps, one CPU frame/renderer
attempt per ordinary qualified step and an actual submission. It must also
validate unqualified attempts: retained trace, no successful submission,
known ordinary case, complete geometry preparation and a later accepted step
with the same window/index. Report their count rather than discard them.
The original native01 must remain rejected.

To prove this deterministically, add one private inputs-probe-only retry-once
flag behind the existing GPU fault feature. Inject through the existing
renderer acquire hook once, record that injection, and require at least one
unqualified attempt followed by the complete ordinary workload. Use the same
standard executable path, dispose the process normally, restore the ordinary
build and qualify that build too. No new example, native input, renderer API or
production-loop retry policy. Preserve source/artifact/PID bindings and old
qualified scenes; this is a directed revalidation for a demonstrated defect.

## Local result

Workspace-model/native02, PID 32680 / actual standard-path feature artifact
`6c8bcbcc…`, observed exactly one injection and one retained unsubmitted attempt,
then all 128 ordinary qualified submissions. The same path was restored with a
normal build; native03 PID 39856 / `9babff9f…` completed all 128 submissions with
zero injections. Both exited0 with empty child-retirement observations and no
observer error; no OS input was generated. Bare probe flags and use of the
fault option in the ordinary executable both returned before native creation.

Thirteen final report/corruption checks accept those two actual captures and
reject the original Main0 submission failure, missing steps/submissions/await
markers, changed retry identity, hidden submitted/deferred frames, duplicate
injection and wrong native PID. The final verifier-only injection consistency
check follows full06 and has its own exact source hash in that receipt; no Rust
input changed. The Source seal and current ordinary artifact/process bindings
are recorded in the product handoff. This closes this fixture admission defect,
not the remaining Builder/consumer/renderer programme exits or physical gates.
