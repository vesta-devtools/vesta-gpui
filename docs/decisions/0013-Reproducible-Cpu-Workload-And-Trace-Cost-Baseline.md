# Reproducible CPU workload and trace-cost baseline

Status: Accepted; bounded CPU implementation qualified 2026-10-31. GF05 remains active.
Date: 2026-10-31.

## Finding

The recorder has functional and native capture evidence, but no comparable
disabled/stage/detailed cost baseline. A successful export does not establish
frame cost, idle behavior or bounded collection work. GF05 needs measurements
whose work and output remain the same across trace modes.

## Decision

Keep one recorder and its existing public ownership/budget/export contract. Add
one bounded Cargo benchmark to the existing Standalone package, whose existing
dependencies already span Core, Widgets and device-free production paint
preparation. Libraries gain no reverse dependency, public API, worker, benchmark
runtime or configuration feature. The normal applications keep their standard
entry points. The benchmark uses Cargo's default target/debug/deps through the
explicit dev profile; it never bypasses an occupied GUI executable.

The benchmark exercises four deterministic public-API workloads: static trees,
continuous-width text reconciliation, actual editor updates and a 100,000-item
virtual collection with stable item identities and a bounded visible projection.
Build, retained layout, paint, hit/action routes and production CPU paint
preparation remain real calls. Record the fixed viewport/sequence, font sources,
warm-up/sample counts and artifact identity. No GPU/context/window or author model.

Compare disabled, stage and detailed collection with the same compiled artifact,
fresh equivalent fixture states and balanced repeated ordering. Retain per-frame
wall times and actual geometry/work signatures. Export after stopping, outside
the timed frame. Reject missing/dropped/unfinished/invalid capture data and
different work rather than advertise a faster incomplete run. Report dispersion
and machine/source identity; do not promise universal time thresholds or mistake
wall time for CPU utilization, native submission or display latency.

Also isolate disabled hook cost against an empty-loop control, using the same
artifact and raw repetitions. Measure bounded exhaustion/stop/idle behavior
without changing application scheduling. Existing high-risk lifecycle contracts
remain authoritative; do not add a test per timer or snapshot of exact pixels.

Two unchanged-artifact runs found the collection detailed-mode median near
0.217 ms versus 0.143 ms disabled; the representative frame records 262 cached
text-shaping calls. Every recorded entry currently checks the session under its
mutex, then immediately takes the same mutex and checks the deadline again to
append. Route entries through one private coordinator and retain the authoritative
append-time check. Keep the global-owner to session lock order, detail filtering,
loss accounting and expired-owner release. No thread-local cache, generation
state, second recorder or public interface is justified. Compare the same work
afterward; a source simplification alone does not establish a measured speedup.

## Qualification

The normal all-target test command runs a small correctness smoke, not timing
thresholds. The explicit benchmark produces new non-overwriting evidence and a
strict verifier. Preserve failed runs. Optimize only a measured bottleneck, then
rerun comparable inputs. The benchmark is a headless CPU slice: native workload,
Builder shared capture ownership, resource/idle review and the rest of GF05 stay
open until separately evidenced. The continuing refinement goal is unchanged.

Local `target/research/ui-workloads-20261031/full02` passed 215 existing Rust
contracts across 33 harnesses, the four-workload/three-mode smoke, documentation,
all-feature/base strict checks, six Node tool contracts, formatting and both
standard builds with unchanged source. Six measured runs retain 6,912 frames /
288 complete traces with matching work and event structure. The earlier Cargo
no-argument benchmark failure and the research controller's initial bad import
remain recorded. [The procedure](../PERFORMANCE.md) reports all observed timing
limitations; there is no new native, physical input, remote CI or Vesta adoption.
