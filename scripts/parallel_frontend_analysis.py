#!/usr/bin/env python3
"""Pure aggregation of retained parallel frontend JSONL (CLI only does file I/O)."""
# r[impl kiln.build.parallel_frontend_evaluation]
import argparse
import collections
import json
import math
import statistics
from pathlib import Path

SCHEMA = "kiln.parallel_frontend.job.v1"
MIN_SUCCESS = 3
MAX_RAW_BYTES = 32 * 1024 * 1024
MAX_ROWS = 10000
CONTROLS = ("toolchain", "rustc_vv", "crate", "mode", "threads", "concurrency", "cpu_quota",
            "memory_max", "cargo_jobs", "source_revision", "host_cpu_count", "host_memory_bytes")


def number(value, name, positive=False):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value < (0 if not positive else 1e-300):
        raise ValueError(f"invalid {name}")
    return value


def distribution(values):
    if not values:
        return {"n": 0, "median": None, "p95": None, "min": None, "max": None}
    ordered = sorted(values)
    return {"n": len(ordered), "median": statistics.median(ordered),
            "p95": ordered[math.ceil(.95 * len(ordered)) - 1],
            "min": ordered[0], "max": ordered[-1]}


def analyze(rows):
    groups = collections.defaultdict(list)
    for row in rows:
        if row.get("schema") != SCHEMA:
            raise ValueError("unknown row schema")
        for field in CONTROLS:
            if field not in row:
                raise ValueError(f"missing {field}")
        if row.get("status") not in ("success", "failure"):
            raise ValueError("invalid status")
        if row["mode"] not in ("check", "build") or row["concurrency"] < 1:
            raise ValueError("invalid mode or concurrency")
        number(row["quota_cpus"], "quota_cpus", positive=True)
        for field in ("wall_s", "end_to_end_s", "queue_wait_s", "cpu_seconds", "peak_bytes", "peak_rss_bytes", "oom_kills", "batch_makespan_s"):
            if row.get(field) is not None:
                number(row[field], field, positive=field in ("wall_s", "batch_makespan_s"))
        if row["status"] == "success" and (row.get("wall_s") is None or row.get("batch_makespan_s") is None):
            raise ValueError("successful job needs wall and batch timing")
        identity = {field: row[field] for field in CONTROLS}
        groups[json.dumps(identity, sort_keys=True)].append(row)
    cells = []
    for encoded, records in sorted(groups.items()):
        identity = json.loads(encoded)
        successes = [row for row in records if row["status"] == "success"]
        batches = collections.defaultdict(list)
        for row in records:
            batches[row["batch_id"]].append(row)
        throughput_batches = []
        for batch in batches.values():
            lengths = {row.get("batch_makespan_s") for row in batch}
            if len(batch) != identity["concurrency"] or len(lengths) != 1 or None in lengths:
                continue
            throughput_batches.append(60 * sum(row["status"] == "success" for row in batch) / next(iter(lengths)))
        complete_batches = [batch for batch in batches.values()
                            if len(batch) == identity["concurrency"]
                            and len({row.get("batch_makespan_s") for row in batch}) == 1
                            and batch[0].get("batch_makespan_s") is not None]
        total_span = sum(batch[0]["batch_makespan_s"] for batch in complete_batches)
        measured_successes = sum(row["status"] == "success" for batch in complete_batches for row in batch)
        utilization = [row["cpu_seconds"] / (row["wall_s"] * row["quota_cpus"])
                       for row in successes if row.get("cpu_seconds") is not None]
        hashes = sorted(((row.get("repeat"), row["artifact_hash"]) for row in successes
                         if row.get("artifact_hash") is not None), key=lambda pair: pair[0])
        hash_pair = hashes[:2] if len(hashes) >= 2 and hashes[0][0] != hashes[1][0] else []
        cells.append({"identity": identity, "n": len(records), "successes": len(successes),
                      "failures": len(records) - len(successes),
                      "ooms": sum((row.get("oom_kills") or 0) for row in records),
                      "wall_s": distribution([row["wall_s"] for row in successes]),
                      "end_to_end_s": distribution([row["end_to_end_s"] for row in successes if row.get("end_to_end_s") is not None]),
                      "cpu_seconds_mean": statistics.mean(v) if (v := [row["cpu_seconds"] for row in successes if row.get("cpu_seconds") is not None]) else None,
                      "cpu_utilization_mean": statistics.mean(utilization) if utilization else None,
                      "peak_bytes": distribution([row["peak_bytes"] for row in successes if row.get("peak_bytes") is not None]),
                      "peak_rss_bytes": distribution([row["peak_rss_bytes"] for row in successes if row.get("peak_rss_bytes") is not None]),
                      "queue_wait_s": distribution([row["queue_wait_s"] for row in records if row.get("queue_wait_s") is not None]),
                      "throughput_per_min": 60 * measured_successes / total_span if total_span else None,
                      "batch_throughput_per_min": distribution(throughput_batches),
                      "insufficient": len(successes) < MIN_SUCCESS or len(complete_batches) < 2,
                      "resource_complete": all(row.get("cpu_seconds") is not None and row.get("peak_bytes") is not None and row.get("peak_rss_bytes") is not None and row.get("oom_kills") is not None and row.get("queue_wait_s") is not None for row in records),
                      "artifact_hash_pair": hash_pair,
                      "artifact_hash_equal": hash_pair[0][1] == hash_pair[1][1] if hash_pair else None})
    recommendations = []
    cohorts = collections.defaultdict(list)
    for cell in cells:
        ident = cell["identity"]
        if ident["threads"] is None:
            continue
        cohort = json.dumps({key: value for key, value in ident.items() if key != "threads"}, sort_keys=True)
        cohorts[cohort].append(cell)
    for encoded, cohort in sorted(cohorts.items()):
        key = json.loads(encoded)
        baseline = next((c for c in cohort if c["identity"]["threads"] == 1), None)
        eligible = [c for c in cohort if not c["insufficient"] and c["resource_complete"]
                    and c["throughput_per_min"] is not None]
        winner = max(eligible, key=lambda c: c["throughput_per_min"], default=None)
        if not baseline or baseline not in eligible or not winner or winner is baseline:
            recommendation = None
            reason = "missing sufficient baseline/candidate or threads=1 maximizes throughput"
        elif winner["batch_throughput_per_min"]["min"] > baseline["batch_throughput_per_min"]["max"]:
            recommendation = winner["identity"]["threads"]
            reason = "winner exceeds threads=1 beyond observed per-batch throughput spread"
        else:
            recommendation = None
            reason = "observed throughput spreads overlap threads=1"
        recommendations.append({"cohort": key, "thread_count": recommendation,
                                "status": "recommended" if recommendation is not None else "no recommendation",
                                "reason": reason})
    return {"schema": "kiln.parallel_frontend.analysis.v1", "cells": cells,
            "recommendations": recommendations,
            "method": "nearest-rank p95; successes only for performance; failed jobs retained; batch throughput = successes / sum measured batch makespans"}


def load_raw(path):
    path = Path(path)
    if path.stat().st_size > MAX_RAW_BYTES:
        raise ValueError("raw JSONL exceeds size bound")
    with path.open(encoding="utf-8") as stream:
        rows = [json.loads(line) for line in stream if line.strip()]
    if len(rows) > MAX_ROWS:
        raise ValueError("too many rows")
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("raw", type=Path)
    args = parser.parse_args()
    print(json.dumps(analyze(load_raw(args.raw)), indent=2))


if __name__ == "__main__":
    main()
