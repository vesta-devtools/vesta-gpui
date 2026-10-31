# Stdio wait budget and process lifetime

Status: Accepted; bounded transport qualification completed 2026-10-31.
Date: 2026-10-31.

## Finding

The existing read-only `wait` operation admits a 1–5,000 ms condition budget,
but each poll calls the ordinary ten-second reply wait. A host that keeps a
reply sender without replying can exceed the caller's budget and report
`host_unavailable`, retiring the stdio transport instead of returning the
existing `wait_timeout`. Poll retries must share one deadline.

The public stdio launchers explicitly start a detached stdin/stdout worker.
Their actual Standalone, native Agent and Builder callers use a process-level
event-loop proxy, not an embedded Agent controller. Blocking stdin cannot be
honestly represented as cancellable with a flag or an unconditional join API.
Keep this process-lifetime boundary and document it. A future replaceable-stream
embedding need would require its own demonstrated admission and shutdown design;
it does not justify another worker/state machine now.

## Decision

Keep the public launchers, wire schema, input bounds, one-request-at-a-time
dispatch and ordinary ten-second host-response policy. Use one private reply
path for ordinary requests and condition polls. A condition poll waits only
until the existing overall wait deadline. Budget expiry returns `wait_timeout`
without retiring transport; actual response-channel disconnection remains
`host_unavailable` and retires it. Preserve per-request channel identity so a
late reply cannot enter a successor request. Check the deadline before dispatch;
retry sleep consumes the same remaining budget.

Dispatch and serialization callbacks are host-owned and must not block. The
reply deadline cannot preempt arbitrary callback work or an OS write to a
blocked output pipe. Do not claim general preemptive cancellation or move host
actions, author authority, event-loop policy or I/O workers into Core.

No new dependency, public type, API export, scheduler, background timer or
configuration state. The existing stdio worker owns stdin/stdout until EOF,
oversized/read/output failure or lost ordinary host response; window close alone
does not cancel a blocked stdin read. Ordinary application process exit remains
the current shutdown boundary.

## Qualification

First reproduce the short-budget defect through the existing polling function.
Protect deadline/retry, late reply isolation and disconnection as one focused
state-machine regression. Exercise the actual public stdio worker in normal
Cargo test child processes: invalid input, bounded wait, a subsequent healthy
request, EOF and oversized termination. Record/retire exact own child handles;
no GUI, author data or operating-system input. Keep the original failed run.

Then run the affected adapter contracts, strict checks, final SDK software
closure and both standard builds. Source integrity, native/physical acceptance,
remote CI and immutable Vesta adoption remain separate facts. GF04 and the
overall refinement goal stay open for their other exits.

`red01` reproduced a 25 ms request waiting 10.0106849 seconds and returning
`host_unavailable`. The final `focused03` passed 15 adapter contracts and strict
checking. `full02` passed 217 contracts / 34 harnesses, the CPU workload smoke,
documentation, all-feature/base strict checks, six Node contracts, formatting and
both standard builds with unchanged inputs. The final public worker test keeps
stdin open during oversized-error retirement, rather than allowing EOF to hide
a missing error exit. Its two actual test children and stdout readers retire
normally; the per-request unit contract also rejects late replies and preserves
context through a not-ready retry. Earlier focused/full scopes remain recorded.

Receipts are under `target/research/stdio-lifecycle-20261031`; production callers
were reviewed without modifying Root. This does not qualify all GF04 interaction
paths, native windows, physical input, remote CI or current Vesta deployment.
