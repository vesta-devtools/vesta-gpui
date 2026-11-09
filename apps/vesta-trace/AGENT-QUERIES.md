# Agent evidence interface — schema 1

Vesta Trace exposes captured evidence directly. An Agent does not need to read
the native window, inspect a screenshot or execute HTML. The GPUI frontend and
machine queries use the same immutable session and interval/relationship analysis.
This interface is in development under [VT01–VT06](../../docs/TRACE-PLAN.md).
Agent-friendly acquisition and analysis is a required product exit. Keep source
identity, selection, coverage and analysis version attached to a diagnosis, and
follow returned event keys when additional evidence is needed. A rendered chart
alone does not satisfy this contract.

Discover the actual typed contracts:

```text
target/debug/vesta-trace.exe --schema
```

The returned request and response JSON Schemas are generated from the serde
types used by the executable. Open one explicit GPUI JSON capture or offline
HTML report and keep stdin/stdout open for multiple requests:

```text
target/debug/vesta-trace.exe path/to/capture.json --query-stdio
```

Send one JSON object per line; read one response per line. EOF closes normally.
No window, GPU initialization, network or model provider is involved. The input
is read once and stays immutable. Requests have no filesystem or execution tools.
An import/startup failure returns a versioned error with null request identity
and a nonzero process exit. Individual request failures return structured errors
and leave the session available for the next request.

## A bounded diagnosis workflow

1. Read session quality and original byte identity:

```json
{"schema_version":1,"request_id":"quality","query":{"op":"session"}}
```

2. Retrieve slow recorded frame attempts:

```json
{"schema_version":1,"request_id":"slow","query":{"op":"frames","sort":"duration_desc"},"page":{"limit":10}}
```

3. Use a returned frame's actual key, for example process 17 / event 1, to
   request recorded hotspots within that frame:

```json
{"schema_version":1,"request_id":"hot","query":{"op":"hotspots","sort":"self_desc"},"selection":{"frame":{"process":17,"event":1}},"page":{"limit":20}}
```

4. Use a returned timer's exact category/name to obtain individual event keys,
   then inspect an event or its recorded tree:

```json
{"schema_version":1,"request_id":"occurrences","query":{"op":"events","timer":{"category":"fixture","name":"scope2"}},"page":{"limit":20}}
{"schema_version":1,"request_id":"event","query":{"op":"event","key":{"process":17,"event":2}}}
{"schema_version":1,"request_id":"tree","query":{"op":"tree","root":{"process":17,"event":1}},"page":{"limit":100}}
```

5. Inspect immediate recorded edges or raw counters:

```json
{"schema_version":1,"request_id":"parents","query":{"op":"relations","timer":{"category":"fixture","name":"scope2"},"direction":"callers"}}
{"schema_version":1,"request_id":"counters","query":{"op":"counters"},"selection":{"range_ns":[25000,60000]}}
{"schema_version":1,"request_id":"counter-peaks","query":{"op":"counter_series","columns":64},"page":{"limit":3}}
```

Example keys and labels illustrate the protocol; always obtain actual keys from
the opened source. `lanes` enumerates process/capture-local thread keys. Tree
rows retain original parent/frame keys even when range/text filters hide their
parents. Names group captured labels and are not symbol-resolved function IDs.

`counter_series` returns exact observation counts and extrema with original event
keys, plus first/last and min/max representatives of time bins. It uses the same
envelope model as the native counter graph. `columns` is 1..256 (default 64);
start with a small page such as three series to stay within context/output budgets.
Use an optional exact `timer` category/name and the normal range/lane/text/frame
selection. Frame filtering follows recorded frame keys, including when a series
contains both framed and unframed observations. `omitted_observations` describes
query/display decimation, separate from capture loss. Every returned point is an
actual observation; no zero-fill, averages or interpolated samples are generated.
Use a minimum/maximum point's event key with `event` for full evidence drill-down.

## Observed heap evidence — analysis 6

`memory_summary` reports whether the optional heap extension exists. All other
heap operations return `trace_memory_not_attached` when absent. `quality.memory`
contains the inclusive last known instant (`through_ns`), operation/stack counts,
event cap, hook errors, unresolved frames and unknown baseline operations. This
channel covers observed Rust System global-allocator lifetimes; pre-capture,
recorder-excluded, native/driver and GPU heaps are outside the baseline. Live
bytes/counts are observed totals, not process-wide usage or a leak diagnosis.

```jsonl
{"schema_version":1,"request_id":"heap","query":{"op":"memory_summary"}}
{"schema_version":1,"request_id":"live","query":{"op":"memory_allocations","sort":"size_desc","lifetime":{"state":"live_at","time_ns":50000000}},"page":{"limit":20}}
{"schema_version":1,"request_id":"growth","query":{"op":"memory_compare","start_ns":10000000,"end_ns":50000000}}
{"schema_version":1,"request_id":"new","query":{"op":"memory_allocations","lifetime":{"state":"new_survivors","start_ns":10000000,"end_ns":50000000}},"page":{"limit":20}}
{"schema_version":1,"request_id":"stacks","query":{"op":"memory_stack_groups","lifetime":{"state":"live_at","time_ns":50000000}},"page":{"limit":20}}
{"schema_version":1,"request_id":"heap-curve","query":{"op":"memory_series","columns":64},"page":{"limit":1000}}
```

Use times from this source's actual coverage. Time-point and comparison queries
reject instants after `through_ns`; they never extrapolate a flat curve beyond
the known prefix. `memory_compare` returns observed start/end totals plus common,
new-surviving, released and transient lifetime cohorts. `observed_change_bytes`
is an exact signed decimal string. Retrieve each cohort's actual rows with the
corresponding `lifetime.state`: `common`, `new_survivors`, `released`, `transient`.
At instant t all operations with timestamp <= t are applied; a free at t removes
the block. Nanosecond ties preserve operation order in raw/series evidence.

Allocation keys are `{process,allocation}`, where allocation is the original
alloc/successful-realloc operation ID. Address reuse creates a distinct key.
`memory_allocation` follows such a key; its returned `stack` key can be passed to
`memory_stack`. Successful realloc closes the old lifetime and links a new one;
failed realloc preserves the original. Unobserved prior allocations stay unknown.
`sort` is `size_desc` (default), `allocated_asc`, or `lifetime_desc`; unresolved
lifetimes use their observed duration through the coverage end for ordering.

`quality.memory` also reports allocation records with/without stacks, requested
depth, potentially truncated stacks, `stack_observations_dropped`, the stored
frame budget and `symbol_budget_exhausted`. Missing stack/symbol metadata does
not remove the lifetime operation or make the rest of the heap unknown.

`memory_allocations` and `memory_stack_groups` support range/lane/text selection.
Ranges select lifetimes intersecting the half-open interval (including an
allocation/free observed at one timestamp); lanes select the allocation thread,
including named memory-only threads. Text matches allocation addresses or stack
addresses/function/file labels. An optional exact `stack` filter is available on
allocations. CPU frame selection is unsupported because no allocation-to-frame
ownership is invented. Summary and individual allocation/stack lookup do not
accept filters; comparisons use their explicit A–B times and optional lane/text.

`memory_series` returns actual post-operation bytes/counts in operation order.
Its bounded bins retain first/last and min/max byte and count observations; no
interpolation or zero-fill is emitted. `omitted_observations` is decimation,
separate from capture loss. Series accepts only range selection. Page through
the representatives if necessary; the curve is not a process-memory baseline.
All list operations use the existing source/query-bound cursor and work budgets.
Stack frames retain raw hexadecimal addresses, available local function/file/line
metadata and potential truncation. CPU scope names remain recorded labels;
heap stack resolution does not imply system CPU stack sampling.

## GPU evidence

Analysis version 5 adds `frame_timings`, shared with the native **Frames** tab:

```json
{"schema_version":1,"request_id":"cpu-gpu","query":{"op":"frame_timings","sort":"gpu_work_desc"},"page":{"limit":10}}
```

Sort by `cpu_duration_desc` (default), `gpu_work_desc`, or `start_asc`. Normal
range/lane/frame/text selection chooses CPU frame attempts. Text matches the
frame's recorded name/category. Each row returns the whole CPU frame evidence
and GPU submissions joined by its original frame key. `overlap_ns` shows CPU
selection overlap; GPU durations are **whole associated submissions**, even
when their CPU recording contexts lie outside the selected subinterval. A
GPU-context query (`gpu_submissions`) retains its separate original semantics.

`gpu.sampled_work_ns` sums available submission durations, which can overlap
across queues/windows and exceed CPU frame elapsed time. It is not GPU frame
latency or utilization. Null means no measured timing; zero means an actual
zero-duration sample. Inspect `samples`, `pending`, `unavailable` and
`all_recorded_submissions_sampled`; missing records and producer loss prevent a
complete recorded-sample claim. `longest_submission` is an actual `GpuKey` for
`gpu_submission` drill-down. Follow the CPU frame key with `hotspots`, `tree`, or
`gpu_submissions` using `selection.frame` to inspect its individual work.

Statistics describe all selected CPU frames before paging, including whole-frame
CPU percentiles and GPU coverage counts. `source_unframed_gpu_records` and
`statistics.source_dropped_gpu_records` explicitly describe the whole input;
unframed submissions are not assigned to a nearby frame. CPU-only captures keep
their rows with null GPU timing. Deterministic sorting, source/query-bound paging,
request budgets, cancellation and checked totals use the shared query contract.
CPU/GPU clock mapping remains unavailable. Analysis-4 cursors are rejected by
analysis 5; request current schemas and fresh pages after upgrading.

Analysis version 4 includes the optional capture-schema-1 GPU attachment. Original
CPU schema 2 remains unchanged. Use actual returned keys rather than these
illustrative values:

```json
{"schema_version":1,"request_id":"devices","query":{"op":"gpu_clocks"}}
{"schema_version":1,"request_id":"gpu-slow","query":{"op":"gpu_submissions","sort":"duration_desc"},"page":{"limit":10}}
{"schema_version":1,"request_id":"gpu-frame","query":{"op":"gpu_submissions"},"selection":{"frame":{"process":17,"event":1}}}
{"schema_version":1,"request_id":"gpu-detail","query":{"op":"gpu_submission","key":{"process":17,"generation":7,"submission":1}}}
```

`gpu_clocks` describes the whole input and rejects selection filters. Metadata
identifies a renderer profiling generation, queue capabilities and recorded
hardware description. It does not assert a persistent physical-device identity.
Submissions retain their original generation/submission, capture-local window,
thread and optional CPU frame key. Follow that frame using `event`/`tree`.

Range, lane and frame selection use the **CPU recording context**. Text matches
the associated CPU frame label; unframed GPU observations do not match text.
The default order is CPU recording context then generation/submission. Duration
order puts actual samples first, including measured zero; pending/unavailable
observations retain null durations. Pages have the same source/query-bound cursor
and budgets as CPU queries. Single-submission drill-down rejects a page cursor.

`quality.gpu` is null for old CPU-only captures, otherwise it reports samples,
pending/unavailable observations and independently dropped GPU records. A missing
attachment is not evidence that the application did no GPU work. Raw ticks,
period, valid bits and completion upper bound support a measured duration.
Raw `raw_ticks` values are exact base-10 strings in this machine interface,
including values above JavaScript's safe integer range. Convert them with a
lossless integer type when arithmetic is necessary. The source capture retains
its original numeric u64 facts; native and machine detail share their exact values.
Earlier analysis-version-3 cursors are rejected.

The renderer interval includes upload/scene/UI commands and device scheduling
stalls. No calibrated CPU/device clock mapping is currently recorded. CPU
association is neither GPU execution start nor display latency. Do not combine
those clocks into an inferred shared timeline.

## Evidence and limits

Each response includes the complete original file's SHA-256 and byte count,
producer schema/clock/thread domain, analysis version and request identity.
An HTML wrapper is part of that original byte identity; only its data script is
parsed. `quality` retains detail level, dropped/clipped/unframed work, expiry,
nesting diagnostics, optional GPU coverage and unavailable full-function/symbol coverage. Absence of
a recorded event does not prove absence of execution. These facts do not supply
an automatic root cause or prove comparisons across unrelated workloads/builds.
Identifiers and accumulated totals are unsigned 64-bit JSON integers. Clients
must preserve integer precision (for example, Python's JSON integer decoding),
rather than silently rounding them through an IEEE-754 JavaScript number. Event
clock coordinates themselves are admitted within the exact integer range.

Ranges are half-open integer nanoseconds inside session bounds. Inclusive/self
timer totals are clipped; self subtracts the union of direct captured span
children even if those children are filtered out. It includes waits and
uninstrumented work. Inclusive sums overlap across nesting/threads. Frame
percentiles use nearest rank over whole recorded attempts intersecting selection,
not clipped durations, CPU utilization or display latency. They are `null` when
no samples match and numeric zero for a measured zero-duration attempt (analysis
definition version 2; old cursors are invalidated). Counters are raw
observations without interpolation. Relationships follow recorded parent keys,
never temporal overlap or invented cross-thread causality.

Paged operations return `total`, `returned`, `has_more` and `next_cursor`. Send
the exact next cursor with the same query, selection and page size. The cursor
is bound to source and query/analysis fingerprints; another source or filter
returns `trace_cursor_stale`. Stable event keys, rather than row positions,
address evidence. `session` and `lanes` describe the whole source and reject
selection filters. Single-event lookup rejects a nonmatching selection.

Requests are at most 64 KiB, pages 1..1000 rows and responses at most 4 MiB.
Default per-request budget is 5,000,000 event/child visits and 5,000 ms; allowed
maxima are 10,000,000 visits and 10,000 ms. `budget` may lower either. Deadlines
are cooperative: bounded JSON parsing, sorting and serialization are checked
around non-preemptible stages. No hard real-time claim is made. Over-budget,
cancelled or overflowing totals return errors without partial successful data.
An oversized line returns one error and closes, so its tail cannot become a
second request. `cost` reports analysis work/time; transport also enforces the
deadline through serialization. A response-budget failure omits oversized
quality data explicitly and retains source identity.

The legacy `--summary` diagnostic remains available; new clients should use the
versioned query interface. Native navigation, GPU association and consuming-host
capture delivery still have their own exits in the plan.

Consuming hosts own their explicit capture controls and opt-in admission. Those
controls acquire an artifact; this executable then opens that explicit artifact
read-only. The SDK analyzer does not acquire host credentials or author
permissions. The 2026-11-02 `diagnosis01` evidence exercises twelve current queries
against an actual current Builder capture, retaining raw wires and a source-bound
report with its workload and missing-data limits; see the delivery plan.

The maintainer-only [navigation verifier](../../tools/verify/trace_navigation.py)
also compares actual native report intervals with this interface and independent
original-JSON arithmetic. The current two real inputs yield 38 source-bound
query pairs across eighteen native states. Held intent is separate from completed
analysis, so a preview cannot silently rename the interval associated with old
numbers. Use this query protocol for product analysis; the finite native console
receipts are acceptance evidence, not an additional interactive API.
