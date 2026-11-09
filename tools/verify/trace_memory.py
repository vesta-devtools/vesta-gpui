"""Independent integer heap oracle and generated-schema wire qualification.

Maintainer-only: reads original operations and exercises the actual JSONL tool.
Never controls a GUI or supplies product analysis results.
"""
import argparse
import hashlib
import json
from pathlib import Path
import queue
import subprocess
import threading
import time

import jsonschema


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Client:
    def __init__(self, executable, arguments, schema, pairs):
        self.process = subprocess.Popen([str(executable), *arguments], stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                        text=True, encoding="utf-8")
        self.lines = queue.Queue()
        self.thread = threading.Thread(target=self.read, daemon=True)
        self.thread.start()
        self.schema = schema
        self.request_validator = jsonschema.validators.validator_for(schema["request"])(schema["request"])
        self.response_validator = jsonschema.validators.validator_for(schema["response"])(schema["response"])
        self.pairs = pairs
        self.sequence = 0

    def read(self):
        for line in self.process.stdout:
            self.lines.put(line)
        self.lines.put(None)

    def request(self, payload, expected_error=None):
        self.sequence += 1
        request = dict(schema_version=1, request_id=f"heap-oracle-{self.sequence}", **payload)
        self.request_validator.validate(request)
        self.process.stdin.write(json.dumps(request) + "\n")
        self.process.stdin.flush()
        line = self.lines.get(timeout=40)
        assert line is not None, "JSONL process exited before its response"
        response = json.loads(line)
        self.response_validator.validate(response)
        assert response["request_id"] == request["request_id"]
        self.pairs.write(json.dumps(dict(request=request, response=response)) + "\n")
        self.pairs.flush()
        if expected_error:
            assert response["result"]["status"] == "error", response
            assert response["result"]["code"] == expected_error, response
        else:
            assert response["result"]["status"] == "ok", response
        return response

    def close(self):
        self.process.stdin.close()
        assert self.process.wait(timeout=35) == 0, self.process.stderr.read()
        self.thread.join(timeout=2)


def schema(executable, option):
    return json.loads(subprocess.check_output([str(executable), option], text=True,
                                             encoding="utf-8", timeout=30))


def acquire(args, pairs):
    c = Client(args.executable, ["--capture-stdio", f"--output-dir={args.output / 'captures'}"],
               schema(args.executable, "--capture-schema"), pairs)
    try:
        c.request(dict(command=dict(op="launch", executable=str(args.host),
                                    arguments=["--hidden", "--observe-seconds=35"])))
        for _ in range(100):
            status = c.request(dict(command=dict(op="status")))["result"]["data"]
            if status["state"] == "idle":
                break
            time.sleep(.03)
        assert status["state"] == "idle" and status["memory_supported"]
        # The endpoint precedes asynchronous renderer startup; qualify steady
        # host work separately from startup-allocation captures.
        time.sleep(2)
        c.request(dict(command=dict(op="start", duration_seconds=2, max_events=100000,
                                    detail="stages", memory=dict(max_events=100000, stack_frames=32))))
        time.sleep(1)
        saved = c.request(dict(command=dict(op="stop")))["result"]["data"]
        source = Path(saved["path"])
        assert digest(source) == saved["source"]["sha256"]
        # Repeated capture with the optional heap channel off remains CPU/GPU only.
        c.request(dict(command=dict(op="start", duration_seconds=1, max_events=100000, detail="stages")))
        time.sleep(.15)
        plain = c.request(dict(command=dict(op="stop")))["result"]["data"]
        assert "gpui_memory" not in json.loads(Path(plain["path"]).read_text(encoding="utf-8"))
        c.request(dict(command=dict(op="detach")))
        return source, plain["path"]
    finally:
        c.close()


def oracle(memory):
    live, allocations, points = {}, {}, []
    unknown_free = unknown_realloc = failures = turnover = 0
    live_bytes = 0
    for event in memory["events"]:
        operation = event["operation"]
        kind = operation["op"]
        prior = None
        if kind in ("free", "realloc"):
            old = operation["old_address"] if kind == "realloc" else operation["address"]
            prior = live.pop(old, None)
            if prior is not None:
                row = allocations[prior]
                assert row["size_bytes"] == operation.get("old_size", operation.get("size"))
                row.update(freed_ns=event["time_ns"], free_thread=event["thread"], end_operation=event["id"])
                live_bytes -= row["size_bytes"]
            elif kind == "free":
                unknown_free += 1
            else:
                unknown_realloc += 1
        if kind == "realloc_failed":
            failures += 1
        if kind in ("alloc", "realloc"):
            assert operation["address"] not in live
            row = dict(allocation=event["id"], address=operation["address"], size_bytes=operation["size"],
                       allocated_ns=event["time_ns"], freed_ns=None, free_thread=None, end_operation=None,
                       thread=event["thread"], stack=event["stack"], reallocated_from=prior,
                       unknown_prior_reallocation=kind == "realloc" and prior is None)
            allocations[event["id"]] = row
            live[operation["address"]] = event["id"]
            turnover += row["size_bytes"]
            live_bytes += row["size_bytes"]
        points.append(dict(operation=event["id"], time_ns=event["time_ns"],
                           observed_live_bytes=live_bytes, observed_live_count=len(live)))
    return allocations, points, dict(unknown_frees=unknown_free, unknown_reallocations=unknown_realloc,
                                    failed_reallocations=failures, observed_allocated_bytes=turnover)


def verify(args, source, pairs):
    original = digest(source)
    document = json.loads(source.read_text(encoding="utf-8"))
    memory = document["gpui_memory"]
    allocations, points, totals = oracle(memory)
    contract = schema(args.executable, "--schema")
    assert contract["analysis_version"] >= 6
    (args.output / "query-schema.json").write_text(json.dumps(contract), encoding="utf-8")
    c = Client(args.executable, [str(source), "--query-stdio"], contract, pairs)

    def ask(query, **kwargs):
        r = c.request(dict(query=query, **kwargs))
        assert r["source"]["sha256"] == original and r["analysis_version"] == contract["analysis_version"]
        return r["result"]["data"]

    def pages(query, **kwargs):
        cursor, rows = None, []
        while True:
            data = ask(query, page=dict(limit=1000, cursor=cursor), **kwargs)
            rows.extend(data["items"])
            cursor = data["page"]["next_cursor"]
            if cursor is None:
                assert len(rows) == data["page"]["total"]
                return rows

    def live(row, at):
        return row["allocated_ns"] <= at and (row["freed_ns"] is None or row["freed_ns"] > at)

    try:
        summary = ask(dict(op="memory_summary"))
        coverage, stats = summary["coverage"], summary["statistics"]
        assert coverage["baseline_known"] is False and coverage["process_wide"] is False
        assert coverage["through_ns"] == memory["elapsed_ns"]
        assert coverage["allocations_with_stacks"] == sum(a["stack"] is not None for a in allocations.values())
        assert coverage["stack_observations_dropped"] == memory.get("stack_observations_dropped", 0)
        if memory.get("stack_frame_budget"):
            assert sum(len(s["frames"]) for s in memory["stacks"]) <= memory["stack_frame_budget"] <= 131072
        assert sum(len((f["function"] or "").encode("utf-8")) + len((f["file"] or "").encode("utf-8")) for s in memory["stacks"] for f in s["frames"]) <= 4*1024*1024 or not memory.get("stack_frame_budget")
        for name in ("unknown_frees", "unknown_reallocations"):
            assert coverage[name] == totals[name]
        for name in ("failed_reallocations", "observed_allocated_bytes"):
            assert stats[name] == totals[name]
        assert stats["allocations"] == len(allocations)
        assert stats["observed_live_bytes_at_end"] == (points[-1]["observed_live_bytes"] if points else 0)
        assert stats["peak_observed_live_bytes"] == max((p["observed_live_bytes"] for p in points), default=0)
        assert stats["peak_observed_live_count"] == max((p["observed_live_count"] for p in points), default=0)
        for sort in ("size_desc", "allocated_asc", "lifetime_desc"):
            rows = pages(dict(op="memory_allocations", sort=sort))
            assert len({r["key"]["allocation"] for r in rows}) == len(allocations)
            for r in rows:
                a = allocations[r["key"]["allocation"]]
                for name in ("address", "size_bytes", "allocated_ns", "freed_ns", "end_operation", "unknown_prior_reallocation"):
                    assert r[name] == a[name], (name, r, a)
                assert r["allocation_lane"]["thread"] == a["thread"]
                assert (r["free_lane"]["thread"] if r["free_lane"] else None) == a["free_thread"]
                assert (r["reallocated_from"]["allocation"] if r["reallocated_from"] else None) == a["reallocated_from"]
            lifetime = lambda a: (a["freed_ns"] if a["freed_ns"] is not None else memory["elapsed_ns"]) - a["allocated_ns"]
            sort_key = {"size_desc": lambda a: (-a["size_bytes"], a["allocation"]),
                        "allocated_asc": lambda a: (a["allocated_ns"], a["allocation"]),
                        "lifetime_desc": lambda a: (-lifetime(a), a["allocation"])}[sort]
            assert [r["key"]["allocation"] for r in rows] == [a["allocation"] for a in sorted(allocations.values(), key=sort_key)]
        start, end = memory["elapsed_ns"] // 4, memory["elapsed_ns"] * 3 // 4
        comparison = ask(dict(op="memory_compare", start_ns=start, end_ns=end))["comparison"]
        predicates = dict(common=lambda a: live(a, start) and live(a, end),
                          new_survivors=lambda a: not live(a, start) and live(a, end),
                          released=lambda a: live(a, start) and not live(a, end),
                          transient=lambda a: start < a["allocated_ns"] <= end and a["freed_ns"] is not None and a["freed_ns"] <= end)
        for name, predicate in predicates.items():
            expected = {a["allocation"] for a in allocations.values() if predicate(a)}
            rows = pages(dict(op="memory_allocations", lifetime=dict(state=name, start_ns=start, end_ns=end)))
            assert {r["key"]["allocation"] for r in rows} == expected
            assert comparison[name]["allocations"] == len(expected)
            assert comparison[name]["bytes"] == sum(allocations[i]["size_bytes"] for i in expected)
        for at in (0, start, end, memory["elapsed_ns"]):
            rows = pages(dict(op="memory_allocations", lifetime=dict(state="live_at", time_ns=at)))
            assert {r["key"]["allocation"] for r in rows} == {a["allocation"] for a in allocations.values() if live(a, at)}
        for columns in (1, 64):
            rows = pages(dict(op="memory_series", columns=columns))
            raw = {p["operation"]: p for p in points}
            assert all(raw[p["operation"]] == p for p in rows)
            assert [p["operation"] for p in rows] == sorted({p["operation"] for p in rows})
            if points:
                assert rows[0] == points[0] and rows[-1] == points[-1]
                for name in ("observed_live_bytes", "observed_live_count"):
                    assert min(p[name] for p in rows) == min(p[name] for p in points)
                    assert max(p[name] for p in rows) == max(p[name] for p in points)
        groups = pages(dict(op="memory_stack_groups"))
        assert sum(g["allocations"] for g in groups) == len(allocations)
        assert sum(g["allocated_bytes"] for g in groups) == totals["observed_allocated_bytes"]
        example = next((a for a in allocations.values() if a["stack"] is not None), None)
        if example:
            process = memory["process_id"]
            detail = ask(dict(op="memory_allocation", key=dict(process=process, allocation=example["allocation"])))["item"]
            stack = ask(dict(op="memory_stack", key=detail["stack"]))["item"]
            raw = next(s for s in memory["stacks"] if s["id"] == example["stack"])
            assert stack["frames"] == raw["frames"] and stack["potentially_truncated"] == raw["potentially_truncated"]
        c.request(dict(query=dict(op="memory_allocations", lifetime=dict(state="live_at", time_ns=memory["elapsed_ns"]+1))), expected_error="trace_memory_coverage")
    finally:
        c.close()
    assert digest(source) == original
    return dict(source=str(source), source_sha256=original, operations=len(points), allocations=len(allocations),
                through_ns=memory["elapsed_ns"], peak_observed_bytes=stats["peak_observed_live_bytes"],
                actual_heap_not_process_wide=True, schemas_validated=True, complete_paged_orders=3,
                baseline_known=False, resolved_frames=coverage["resolved_frames"],
                unresolved_frames=coverage["unresolved_frames"], event_limit_reached=memory["event_limit_reached"])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--executable", type=Path, required=True)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--source", type=Path)
    mode.add_argument("--host", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.executable = args.executable.resolve()
    if args.host:
        args.host = args.host.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    with (args.output / "wire-pairs.jsonl").open("w", encoding="utf-8") as pairs:
        source, plain = (acquire(args, pairs) if args.host else (args.source.resolve(), None))
        receipt = verify(args, source, pairs)
    receipt.update(executable_sha256=digest(args.executable), verifier_sha256=digest(Path(__file__)),
                   plain_repeat_capture=plain, host_sha256=digest(args.host) if args.host else None,
                   wire_sha256=digest(args.output / "wire-pairs.jsonl"))
    (args.output / "qualification.json").write_text(json.dumps(receipt, indent=2), encoding="utf-8")
    print(json.dumps(receipt))


if __name__ == "__main__":
    main()
