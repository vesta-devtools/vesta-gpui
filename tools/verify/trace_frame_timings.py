"""Verify frame_timings against original JSON facts, Python integer arithmetic and schemas.

Maintainer oracle only: not a second product analysis engine. Never drives a GUI.
Writes source/executable identities, actual wire pairs and a bounded receipt.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import threading
import time

import jsonschema


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def ns(value):
    scaled = float(value) * 1000
    assert math.isfinite(scaled) and scaled >= 0
    whole = math.floor(scaled)
    return whole + int(scaled - whole >= .5)


def key(event):
    return event["pid"], event["args"]["event_id"]


def identity(pair):
    return {"process": pair[0], "event": pair[1]}


def oracle(document):
    frames = [e for e in document["traceEvents"] if e["args"]["frame"]]
    assert len(frames) <= 20000, "independent verifier frame budget exceeded"
    capture = document.get("gpui_capture", {})
    grouped = {}
    unframed = 0
    for record in capture.get("submissions", []):
        if record["outcome"]["state"] == "sample":
            clock = next(c for c in capture["clocks"] if c["generation"] == record["generation"])
            ticks = record["outcome"]["raw_ticks"]
            delta = (ticks[1]-ticks[0]) & ((1 << clock["valid_bits"])-1)
            assert math.isclose(delta*clock["period_ns"], record["outcome"]["elapsed_ns"], rel_tol=1e-15, abs_tol=1), "raw GPU timestamps disagree with duration"
        if record["frame_id"] is None:
            unframed += 1
        else:
            grouped.setdefault((capture["process_id"], record["frame_id"]), []).append(record)
    rows = {}
    for frame in frames:
        records = grouped.get(key(frame), [])
        samples = [r for r in records if r["outcome"]["state"] == "sample"]
        longest = min(samples, key=lambda r: (-r["outcome"]["elapsed_ns"], r["generation"], r["submission"])) if samples else None
        rows[key(frame)] = {"records": len(records), "samples": len(samples),
            "pending": sum(r["outcome"]["state"] in ("recording", "pending_completion") for r in records),
            "unavailable": sum(r["outcome"]["state"] == "unavailable" for r in records),
            "sampled_work_ns": sum(r["outcome"]["elapsed_ns"] for r in samples) if samples else None,
            "longest_sample_ns": longest["outcome"]["elapsed_ns"] if longest else None,
            "longest_submission": {"process": capture["process_id"], "generation": longest["generation"], "submission": longest["submission"]} if longest else None,
            "all_recorded_submissions_sampled": bool(records) and len(samples) == len(records) and capture["dropped_records"] == 0}
    return frames, rows, unframed


def verify(analyzer, source, output):
    assert source.stat().st_size <= 256 * 1024 * 1024
    document = json.loads(source.read_text(encoding="utf-8-sig"))
    frames, expected, unframed = oracle(document)
    frame_by_key = {key(frame): frame for frame in frames}
    schemas = json.loads(subprocess.check_output([str(analyzer), "--schema"], timeout=15))
    request_validator = jsonschema.Draft7Validator(schemas["request"])
    response_validator = jsonschema.Draft7Validator(schemas["response"])
    source_sha = digest(source)
    output.mkdir(parents=True, exist_ok=False)
    started = time.monotonic()
    wire = []
    stderr = (output / "stderr.log").open("xb")
    process = subprocess.Popen([str(analyzer), str(source), "--query-stdio"], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=stderr, text=True,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
    def ask(operation, selection=None, page=None):
        request = {"schema_version": 1, "request_id": f"frame-{len(wire)}", "query": operation,
                   "page": page or {"limit": 1000}}
        if selection is not None:
            request["selection"] = selection
        request_validator.validate(request)
        # Time out a child that stops responding, not the independent Python
        # oracle/schema work between requests while the child waits on stdin.
        deadline = threading.Timer(60, process.terminate)
        deadline.start()
        try:
            process.stdin.write(json.dumps(request) + "\n")
            process.stdin.flush()
            line = process.stdout.readline(4 * 1024 * 1024 + 1)
        finally:
            deadline.cancel()
        assert len(line) <= 4 * 1024 * 1024 and line.endswith("\n"), "bounded response missing"
        response = json.loads(line)
        response_validator.validate(response)
        wire.append({"request": request, "response": response})
        assert response["request_id"] == request["request_id"]
        assert response["source"]["sha256"] == source_sha
        assert response["analysis_version"] == schemas["analysis_version"]
        assert response["result"]["status"] == "ok", response
        return response["result"]["data"]
    def check_rows(data, selection=None):
        assert data["source_unframed_gpu_records"] == unframed
        assert data["gpu_attached"] == ("gpui_capture" in document)
        for row in data["items"]:
            observed = row["frame"]
            pair = observed["key"]["process"], observed["key"]["event"]
            frame = frame_by_key[pair]
            start, duration = ns(frame["ts"]), ns(frame["dur"])
            assert observed["start_ns"] == start and observed["duration_ns"] == duration
            assert observed["end_ns"] == start + duration
            if selection:
                begin, end = selection["range_ns"]
                assert observed["overlap_ns"] == max(0, min(end, start+duration)-max(start, begin))
            assert row["gpu"] == expected[pair], (pair, row["gpu"], expected[pair])
    try:
        for sort in ("cpu_duration_desc", "gpu_work_desc", "start_asc"):
            if sort == "cpu_duration_desc":
                ordered = sorted(frames, key=lambda f: (-ns(f["dur"]), key(f)))
            elif sort == "start_asc":
                ordered = sorted(frames, key=lambda f: (ns(f["ts"]), key(f)))
            else:
                ordered = sorted(frames, key=lambda f: (expected[key(f)]["sampled_work_ns"] is None,
                    -(expected[key(f)]["sampled_work_ns"] or 0), key(f)))
            observed = []
            page = None
            while True:
                data = ask({"op": "frame_timings", "sort": sort}, page=page)
                check_rows(data)
                stats = data["statistics"]
                assert stats["cpu"]["count"] == len(frames)
                durations = sorted(ns(f["dur"]) for f in frames)
                worst = min(frames,key=lambda f:(-ns(f["dur"]),key(f))) if frames else None
                assert stats["cpu"]["worst"] == (identity(key(worst)) if worst else None)
                for percentile in (50, 95, 99):
                    want = durations[(len(durations)*percentile+99)//100-1] if durations else None
                    assert stats["cpu"][f"p{percentile}_ns"] == want
                assert stats["frames_with_gpu_records"] == sum(r["records"] > 0 for r in expected.values())
                assert stats["frames_with_gpu_samples"] == sum(r["samples"] > 0 for r in expected.values())
                assert stats["frames_with_all_recorded_submissions_sampled"] == sum(r["all_recorded_submissions_sampled"] for r in expected.values())
                for field in ("samples", "pending", "unavailable"):
                    assert stats[f"gpu_{field}"] == sum(r[field] for r in expected.values())
                observed.extend((r["frame"]["key"]["process"], r["frame"]["key"]["event"]) for r in data["items"])
                if data["page"]["next_cursor"] is None:
                    break
                page = {"limit": 1000, "cursor": data["page"]["next_cursor"]}
            assert observed == [key(f) for f in ordered]
        if frames:
            frame = max(frames, key=lambda f: ns(f["dur"]))
            start, duration = ns(frame["ts"]), ns(frame["dur"])
            if duration >= 3:
                selection = {"range_ns": [start+duration//3, start+2*duration//3], "frame": identity(key(frame))}
                selected = ask({"op": "frame_timings"}, selection=selection)
                check_rows(selected, selection)
                assert selected["page"]["total"] == 1
            longest = expected[key(frame)]["longest_submission"]
            if longest:
                gpu = ask({"op": "gpu_submission", "key": longest})
                assert gpu["item"]["frame"] == identity(key(frame))
                assert gpu["item"]["elapsed_ns"] == expected[key(frame)]["longest_sample_ns"]
        process.stdin.close()
        assert process.wait(timeout=5) == 0
        assert digest(source) == source_sha
        (output / "wire.json").write_text(json.dumps(wire, indent=2)+"\n", encoding="utf-8")
        receipt = {"status": "passed", "analysis_version": schemas["analysis_version"], "analyzer": str(analyzer),
            "analyzer_sha256": digest(analyzer), "source": str(source), "source_sha256": source_sha,
            "verifier_sha256": digest(Path(__file__)), "frames": len(frames), "wire_pairs": len(wire),
            "schema_checked_messages": 2*len(wire), "all_rows_and_orders_verified": True,
            "whole_gpu_work_and_cpu_overlap_verified": True, "wall_seconds": time.monotonic()-started}
        (output / "qualification.json").write_text(json.dumps(receipt, indent=2)+"\n", encoding="utf-8")
        return receipt
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
        stderr.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--analyzer", type=Path, required=True)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    print(json.dumps(verify(arguments.analyzer.resolve(), arguments.source.resolve(), arguments.output.resolve())))
