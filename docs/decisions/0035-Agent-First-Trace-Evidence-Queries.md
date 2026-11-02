# Agent-first trace evidence queries

Status: Accepted; finite implementation and real-capture Agent workflow qualified, 2026-11-03.
Date: 2026-11-01.
Extends: [0034](0034-Native-Trace-Analysis-Application.md).

The user identifies Agent-friendly acquisition and analysis as Vesta Trace's
primary differentiator. A native timeline alone does not satisfy delivery.
Prioritize a versioned, read-only machine interface alongside the GPUI viewer.
Both consume one immutable session and the same interval/relationship analysis;
neither screenshots nor HTML presentation are machine contracts.

The application exposes its actual typed request and response schemas and a
bounded JSONL stdio query mode. One explicit input is loaded once at startup;
queries cannot open files, execute code, mutate capture data or drive desktop
input. No model provider, credential, network service or second runtime is added.
An Agent can discover coverage, enumerate lanes and slow frames, inspect events
and recorded parent/frame links, request filtered interval hotspots and immediate
callers/callees, and obtain raw counter observations. Labels group recorded work;
they are not resolved function/callsite identities or complete function sampling.

Every response identifies the exact original input bytes by SHA-256, format,
producer schema, CPU clock and units. Event/process/capture-local thread keys
are preserved. Range boundaries are integer nanoseconds and half-open. Frame
durations describe whole recorded frame attempts intersecting the query, while
timer work is clipped. Inclusive totals may overlap. Self time subtracts the
union of direct recorded span children, including filtered-out children, and
still includes waits and uninstrumented work.

Use explicit request identity, deterministic ordering, page limits and cursors
bound to source and query fingerprints. Disclose completeness, loss, clipping,
nesting validity, unframed work, filtering, result truncation and unsupported
GPU/symbol/cross-thread information. Facts are never promoted to an automatic
root-cause claim. Stable event keys permit evidence drill-down without dumping
an entire session into a model context.

Input, output and cooperative work/deadline budgets bound individual requests.
Cancellation or budget failure returns a structured error, not partial totals
that look complete. JSON parsing and bounded sorting/serialization are checked
around their non-preemptible stages; do not describe these as hard real-time
deadlines. Stdio remains machine-only, one response per request, with clean EOF.

Keep this within VT01/03/04/06, preserving all existing native, GPU and host
integration exits. Validate source/query-bound paging, corruption, loss, budget
and cancellation contracts plus a real-capture Agent workflow independently.
