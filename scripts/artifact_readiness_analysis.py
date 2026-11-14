#!/usr/bin/env python3
"""Independent, standard-library-only analysis of retained readiness JSONL."""
# r[impl kiln.readiness.benchmark.scenario.evidence]
import argparse
import collections
import json
import math
import statistics
from pathlib import Path

MAX_RAW_BYTES = 32 * 1024 * 1024
MAX_RECORDS = 10000
MIN_P95 = 20
MIN_P99 = 100


def distribution(values):
    if not values:
        return {"n": 0}
    values = sorted(values)
    result = {"n": len(values), "median_ns": statistics.median(values),
              "min_ns": values[0], "max_ns": values[-1],
              "ecdf": [[value, (i + 1) / len(values)] for i, value in enumerate(values)]}
    for name, quantile, minimum in [("p95_ns", .95, MIN_P95), ("p99_ns", .99, MIN_P99)]:
        if len(values) >= minimum:
            result[name] = values[math.ceil(quantile * len(values)) - 1]
    return result


def analyze(records):
    cells = {}
    for row in records:
        if row.get("schema") != "kiln.readiness.sample.v1":
            raise ValueError("unknown sample schema")
        if row.get("status") not in ("complete", "failed", "incomplete", "absent"):
            raise ValueError("invalid attempt status")
        workload = row["workload"]
        key = json.dumps({field: row[field] for field in (
            "provider", "profile", "runner_identity", "source_identity", "cache_state", "delay_phase", "delay_ms")}
            | {"workload": workload, "profile_identity": row.get("profile_identity", row["profile"])}, sort_keys=True)
        cell = cells.setdefault(key, {"identity": json.loads(key), "attempts": 0,
                                     "statuses": collections.Counter(), "phases": {},
                                     "errors": [], "output_identities": set(), "attribution_errors": []})
        cell["attempts"] += 1
        cell["statuses"][row["status"]] += 1
        if row.get("error"):
            cell["errors"].append(row["error"])
        if row.get("output_identity"):
            cell["output_identities"].add(row["output_identity"])
        # Repeated disjoint occurrences of one phase are aggregated within an attempt.
        # Inclusive parent phases have distinct names; never sum across phase names.
        attempt = {}
        for observation in row.get("observations", []):
            name = observation["phase"]
            target = attempt.setdefault(name, {})
            for metric in ("elapsed_ns", "user_cpu_ns", "system_cpu_ns"):
                value = observation.get(metric)
                if value is not None:
                    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value < 0:
                        raise ValueError("invalid timing value")
                    target[metric] = target.get(metric, 0) + value
            injected = observation.get("injected_delay_ms", 0)
            if injected:
                if name != row["delay_phase"] or injected != row["delay_ms"] or observation["elapsed_ns"] < injected * 1_000_000:
                    cell["attribution_errors"].append({"index": row["index"], "phase": name})
        if row["status"] == "complete" and row["delay_ms"] and row["delay_phase"] in attempt:
            matching = [o for o in row["observations"] if o["phase"] == row["delay_phase"]]
            if not matching or any(o.get("injected_delay_ms") != row["delay_ms"] for o in matching):
                cell["attribution_errors"].append({"index": row["index"], "phase": row["delay_phase"], "error": "requested delay not observed"})
        if row["status"] == "complete":
            for name, metrics in attempt.items():
                phase = cell["phases"].setdefault(name, {})
                for metric, value in metrics.items():
                    phase.setdefault(metric, []).append(value)
    result = []
    for cell in cells.values():
        cell["statuses"] = dict(cell["statuses"])
        cell["output_identities"] = sorted(cell["output_identities"])
        cell["phases"] = {name: {metric: distribution(values) for metric, values in metrics.items()}
                          for name, metrics in cell["phases"].items()}
        result.append(cell)
    return {"schema": "kiln.readiness.analysis.v1", "cells": result,
            "optimization_decision": "insufficient-evidence",
            "reason": "This baseline has no reviewed candidate optimization, matched semantic work, or complexity/retention evidence; no speedup or go claim is authorized.",
            "quantile_method": "nearest-rank; p95 >=20 complete attempts; p99 >=100; no failed or absent times imputed",
            "phase_aggregation": "sum repeated same-name phases per attempt; distinct inclusive parent phases are not additive"}


def compare(baseline, candidate, policy, review):
    """A proposed optimization is no-go unless ALL predeclared evidence passes."""
    reasons = []
    minimum = policy["minimum_samples"]
    if type(minimum) is not int or not 30 <= minimum <= 100:
        raise ValueError("minimum optimization evidence is 30..100 samples per cell")
    if policy["minimum_p95_saving_fraction"] != .2 or policy["minimum_p95_saving_ms"] != 5 or policy["maximum_retained_byte_growth"] != 0:
        raise ValueError("predeclared decision thresholds cannot be weakened")
    left = {json.dumps(c["identity"], sort_keys=True): c for c in baseline["cells"]}
    right = {json.dumps(c["identity"], sort_keys=True): c for c in candidate["cells"]}
    if left.keys() != right.keys() or not left:
        reasons.append("comparable cohort identities differ or are empty")
    shifts = []
    if any(cell["statuses"].get(status, 0) for cell in baseline["cells"] + candidate["cells"] for status in ("failed", "incomplete")):
        reasons.append("failed or incomplete attempts cannot be excluded from the decision")
    for key in left.keys() & right.keys():
        a, b = left[key], right[key]
        if a["identity"]["cache_state"] != "warm-local-hit":
            continue
        if a["statuses"].get("complete", 0) < minimum or b["statuses"].get("complete", 0) < minimum:
            reasons.append("insufficient complete samples")
            continue
        if any(cell["statuses"].get(state, 0) for cell in (a, b) for state in ("failed", "incomplete", "absent")):
            reasons.append("failure or incomplete attempt in cohort")
        if a["output_identities"] != b["output_identities"] or len(a["output_identities"]) != 1:
            reasons.append("output semantic identities differ or are missing")
        if a["attribution_errors"] or b["attribution_errors"]:
            reasons.append("invalid phase attribution")
        aa = a["phases"].get("materialization", {}).get("elapsed_ns", {})
        bb = b["phases"].get("materialization", {}).get("elapsed_ns", {})
        if "p95_ns" not in aa or "p95_ns" not in bb:
            reasons.append("materialization p95 absent")
            continue
        saving = aa["p95_ns"] - bb["p95_ns"]
        fraction = saving / aa["p95_ns"] if aa["p95_ns"] else 0
        shifts.append({"identity": a["identity"], "p95_saving_ns": saving, "p95_saving_fraction": fraction,
                       "median_shift_ns": bb["median_ns"] - aa["median_ns"],
                       "baseline_ecdf": aa["ecdf"], "candidate_ecdf": bb["ecdf"]})
        if saving < policy["minimum_p95_saving_ms"] * 1_000_000 or fraction < policy["minimum_p95_saving_fraction"]:
            reasons.append("savings below predeclared absolute or relative threshold")
    if not shifts:
        reasons.append("no comparable materialization cell")
    if review.get("complexity_review_accepted") is not True or review.get("same_semantic_work") is not True:
        reasons.append("complexity review or same-work evidence missing")
    growth = review.get("retained_byte_growth")
    if type(growth) not in (int, float) or not math.isfinite(growth) or growth > policy["maximum_retained_byte_growth"]:
        reasons.append("retention budget exceeded or unmeasured")
    return {"optimization_decision": "no-go" if reasons else "go", "reasons": sorted(set(reasons)), "shifts": shifts}


def load_raw(path):
    if path.stat().st_size > MAX_RAW_BYTES:
        raise ValueError("raw samples exceed bounded analysis input")
    records = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
    if len(records) > MAX_RECORDS:
        raise ValueError("too many raw samples")
    return records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("raw", type=Path)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--policy", type=Path, help="retained exported cohort JSON")
    parser.add_argument("--review", type=Path, help="independently reviewed complexity/same-work/retention JSON")
    args = parser.parse_args()
    result = analyze(load_raw(args.raw))
    if args.candidate:
        if not args.policy or not args.review:
            parser.error("candidate comparison requires predeclared policy and explicit review")
        result["comparison"] = compare(result, analyze(load_raw(args.candidate)),
                                       json.loads(args.policy.read_text())["decision"], json.loads(args.review.read_text()))
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
