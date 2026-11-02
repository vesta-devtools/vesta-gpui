"""Check a finite native navigation run against queries and original CPU facts.

This is a maintainer evidence verifier, not a second product analysis engine.
Only the explicit standard analyzer is executed; no window or OS input is used.
Python integers preserve original u64 identities and accumulated totals.
"""
import argparse
import collections
import datetime
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import sys

MAX_SOURCE = 256 * 1024 * 1024
MAX_LOG = 16 * 1024 * 1024
MAX_REPLY = 4 * 1024 * 1024
STAGES = ["initial", "range_preview", "forward_range", "reverse_range",
          "anchored_zoom", "pointer_pan", "wheel_pan", "fit", "resize_cancel"]


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(65536), b""):
            h.update(block)
    return h.hexdigest()


def nanoseconds(microseconds):
    value = float(microseconds) * 1000.0
    assert math.isfinite(value) and 0 <= value <= (1 << 53) - 1
    whole = math.floor(value)
    return whole + (1 if value - whole >= 0.5 else 0)


def original_facts(document):
    raw = document["traceEvents"]
    assert len(raw) <= 1_000_000
    events, children = [], collections.defaultdict(list)
    for item in raw:
        event = dict(item)
        event["start"] = nanoseconds(item["ts"])
        event["end"] = event["start"] + (nanoseconds(item["dur"]) if item["ph"] == "X" else 0)
        event["key"] = (item["pid"], item["args"]["event_id"])
        events.append(event)
        if item["ph"] == "X" and item["args"]["parent_id"] is not None:
            children[(item["pid"], item["args"]["parent_id"])].append(event)
    assert len({e["key"] for e in events}) == len(events)
    return events, children


def reference(events, children, bounds):
    start, end = bounds
    timers, frames = {}, []
    for event in events:
        if event["ph"] != "X":
            continue
        a, b = event["start"], event["end"]
        visible = start <= a < end if a == b else a < end and start < b
        if not visible:
            continue
        if event["args"]["frame"]:
            frames.append(b - a)
        left, right = max(start, a), min(end, b)
        inclusive = max(0, right - left)
        intervals = sorted((max(left, c["start"]), min(right, c["end"]))
                           for c in children[event["key"]]
                           if max(left, c["start"]) < min(right, c["end"]))
        covered, previous = 0, left
        for x, y in intervals:
            covered += max(0, y - max(x, previous))
            previous = max(previous, y)
        key = (event["cat"], event["name"])
        totals = timers.setdefault(key, [0, 0, 0])
        totals[0] += 1
        totals[1] += inclusive
        totals[2] += inclusive - covered
    ordered = sorted(timers.items(), key=lambda item: (-item[1][1], item[0]))
    rows = [{"timer": {"category": key[0], "name": key[1]}, "calls": value[0],
             "inclusive_ns": value[1], "self_ns": value[2]} for key, value in ordered]
    frames.sort()
    percentiles = [frames[max(0, (len(frames) * p + 99) // 100 - 1)] if frames else 0
                   for p in (50, 95, 99)]
    return rows, len(frames), percentiles


def verify(args):
    source, log, analyzer, out = (Path(p).resolve() for p in
                                 (args.source, args.log, args.analyzer, args.output))
    assert source.is_file() and source.stat().st_size <= MAX_SOURCE
    assert log.is_file() and log.stat().st_size <= MAX_LOG
    assert analyzer.is_file() and (os.name != "nt" or analyzer.suffix.lower() == ".exe")
    assert digest(analyzer) == args.expected_analyzer_sha256, "analyzer identity changed"
    source_sha = digest(source)
    snapshots = [json.loads(line.removeprefix("trace_navigation_evidence "))
                 for line in log.read_text(encoding="utf-8-sig").splitlines()
                 if line.startswith("trace_navigation_evidence ")]
    assert [s["name"] for s in snapshots] == STAGES
    assert [s["step"] for s in snapshots] == list(range(len(STAGES)))
    assert all(s["schema_version"] == 1 and s["source_sha256"] == source_sha for s in snapshots)
    assert snapshots[1]["range_ns"] == snapshots[0]["range_ns"]
    assert snapshots[1]["intent_range_ns"] != snapshots[1]["range_ns"]
    assert snapshots[2]["range_ns"] == snapshots[1]["intent_range_ns"]
    assert all(s["range_ns"] == snapshots[3]["range_ns"] for s in snapshots[4:7])
    assert all(s["range_ns"] == snapshots[0]["range_ns"] for s in snapshots[7:])
    assert all(s["view_ns"] == snapshots[0]["view_ns"] for s in snapshots[7:])
    full = snapshots[0]["range_ns"]
    assert all(full[0] <= s["view_ns"][0] < s["view_ns"][1] <= full[1] for s in snapshots)
    events, children = original_facts(json.loads(source.read_text(encoding="utf-8-sig")))
    out.mkdir(parents=True, exist_ok=False)
    receipt = {"status": "running", "controller_pid": os.getpid(), "controller_executable": sys.executable, "analyzer": str(analyzer),
               "analyzer_sha256": digest(analyzer), "source": str(source), "source_sha256": source_sha,
               "native_log": str(log), "native_log_sha256": digest(log),
               "verifier_sha256": digest(Path(__file__)),
               "analysis_version": snapshots[0]["analysis_version"],
               "started_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()}

    def save():
        (out / "qualification.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")

    with (out / "stderr.log").open("xb") as errors, (out / "wire.jsonl").open("x", encoding="utf-8") as wire:
        child = subprocess.Popen([str(analyzer), str(source), "--query-stdio"], stdin=subprocess.PIPE,
                                 stdout=subprocess.PIPE, stderr=errors,
                                 creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        receipt["pid"] = child.pid
        save()
        query_count = 0

        def query(name, operation, selection=None, limit=8):
            nonlocal query_count
            request = {"schema_version": 1, "request_id": name, "query": operation,
                       "page": {"limit": limit}, "budget": {"max_work": 5_000_000, "deadline_ms": 5000}}
            if selection is not None:
                request["selection"] = {"range_ns": selection}
            child.stdin.write((json.dumps(request) + "\n").encode("utf-8"))
            child.stdin.flush()
            line = child.stdout.readline(MAX_REPLY + 1)
            assert line.endswith(b"\n") and len(line) <= MAX_REPLY, "missing/oversized query response"
            response = json.loads(line)
            wire.write('{"request":' + json.dumps(request) + ',"response":' + line.decode("utf-8").strip() + '}\n')
            wire.flush()
            query_count += 1
            assert response["schema_version"] == 1 and response["request_id"] == name
            assert response["analysis_version"] == receipt["analysis_version"]
            assert response["source"]["sha256"] == source_sha and response["result"]["status"] == "ok"
            return response["result"]["data"]

        try:
            session = query("source-quality", {"op": "session"})
            assert session["events"] == len(events) and session["bounds_ns"] == full
            assert session["frames"] == sum(e["ph"] == "X" and e["args"]["frame"] for e in events)
            checks = []
            for snapshot in snapshots:
                stage = snapshot["name"]
                assert snapshot["analysis_version"] == receipt["analysis_version"]
                rows, count, percentiles = reference(events, children, snapshot["range_ns"])
                assert snapshot["timers"] == rows[:8], stage + ": native timers differ from original arithmetic"
                assert snapshot["timers_total"] == len(rows) and snapshot["frames_count"] == count
                assert snapshot["percentile_ns"] == percentiles
                hotspots = query(stage + "-hotspots", {"op": "hotspots", "sort": "inclusive_desc"}, snapshot["range_ns"])
                assert hotspots["items"] == rows[:8] and hotspots["page"]["total"] == len(rows), stage
                frames = query(stage + "-frames", {"op": "frames", "sort": "duration_desc"}, snapshot["range_ns"], 1)
                stats = frames["statistics"]
                assert stats["count"] == count
                assert [stats[key] for key in ("p50_ns", "p95_ns", "p99_ns")] == (percentiles if count else [None] * 3)
                checks.append({"stage": stage, "range_ns": snapshot["range_ns"], "timers": len(rows), "frames": count})
            receipt.update(status="passed", stages=checks, query_pairs=query_count,
                           original_arithmetic_matches_native_and_queries=True)
        except BaseException as error:
            receipt.update(status="failed", error=repr(error), query_pairs=query_count)
            raise
        finally:
            try:
                child.stdin.close()
            except BrokenPipeError:
                pass
            receipt["exit_code"] = child.wait()
            receipt.update(source_unchanged=digest(source) == source_sha,
                           analyzer_unchanged=digest(analyzer) == args.expected_analyzer_sha256,
                           finished_utc=datetime.datetime.now(datetime.timezone.utc).isoformat())
            receipt["scope"] = ("Finite actual native navigation snapshots agree with persistent typed machine queries "
                                "and independent original JSON interval/child-union/frame arithmetic. No GUI/OS input, "
                                "Reader, calibrated GPU clock, workload root cause or comparative performance claim.")
            save()
        assert receipt["exit_code"] == 0 and receipt["source_unchanged"] and receipt["analyzer_unchanged"]
    return {"status": receipt["status"], "pid": receipt["pid"], "query_pairs": query_count,
            "native_stages": len(snapshots), "source_sha256": source_sha}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", help="original GPUI JSON capture, unchanged")
    parser.add_argument("log", help="completed --navigation-capture stdout log")
    parser.add_argument("--analyzer", required=True, help="explicit standard vesta-trace executable")
    parser.add_argument("--expected-analyzer-sha256", required=True)
    parser.add_argument("--output", required=True, help="new private evidence directory")
    try:
        print(json.dumps(verify(parser.parse_args())))
    except Exception as error:
        print(json.dumps({"status": "failed", "error": str(error)}), file=sys.stderr)
        sys.exit(1)
