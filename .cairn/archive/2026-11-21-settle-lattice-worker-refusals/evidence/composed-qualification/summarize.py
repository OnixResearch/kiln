#!/usr/bin/env python3
"""Summarize one qualify.sh output root into per-scenario observations."""
import json
import os
import sys

out = sys.argv[1]


def text(path):
    try:
        with open(path) as handle:
            return handle.read().strip()
    except FileNotFoundError:
        return None


def parsed(value):
    if value is None:
        return None
    lines = [line for line in value.splitlines() if line.strip()]
    result = []
    for line in lines:
        try:
            result.append(json.loads(line))
        except json.JSONDecodeError:
            result.append(line)
    return result


def compact(observation):
    if not isinstance(observation, dict):
        return observation
    keep = {}
    for key in ("schema", "status", "reason", "dispatched", "accepted", "integrated", "request_identity",
                "execution_binding", "cancellation_requested", "termination_observed", "not_started"):
        if key in observation:
            keep[key] = observation[key]
    result = observation.get("result")
    if isinstance(result, dict):
        keep["result"] = {k: result.get(k) for k in ("disposition", "stdout_blake3", "receipt") if k in result}
    elif "result" in observation:
        keep["result"] = result
    if "envelope" in observation:
        keep["envelope_attempt"] = observation["envelope"].get("attempt_id")
        keep["envelope_fence"] = observation["envelope"].get("fence")
    return keep


summary = {}
for scenario in sorted(os.listdir(out)):
    directory = os.path.join(out, scenario)
    if not os.path.isdir(directory):
        continue
    calls = {}
    for name in sorted(os.listdir(directory)):
        if name.endswith(".exit"):
            label = name[:-5]
            stdout = parsed(text(os.path.join(directory, label + ".stdout")))
            calls[label] = {
                "exit": int(text(os.path.join(directory, name))),
                "stdout": [compact(item) for item in stdout or []],
                "stderr": (text(os.path.join(directory, label + ".stderr")) or "")[-600:],
            }
    attempt = None
    try:
        attempt = json.loads(text(os.path.join(directory, "kiln-attempt.json")) or "null")
    except json.JSONDecodeError:
        attempt = text(os.path.join(directory, "kiln-attempt.json"))
    observation = None
    for root, _, files in os.walk(os.path.join(directory, "state")):
        if "omp-observation.json" in files:
            observation = json.loads(text(os.path.join(root, "omp-observation.json")))
    summary[scenario] = {
        "calls": calls,
        "endpoint_requests": int(text(os.path.join(directory, "endpoint.count")) or 0),
        "kiln_attempt": {k: attempt.get(k) for k in ("phase", "fence", "dispatch", "dispatch_request_binding", "execution", "workflow", "failure", "cause")}
        if isinstance(attempt, dict) else attempt,
        "lattice_state_files": (text(os.path.join(directory, "state.files")) or "").splitlines(),
        "omp_observation": observation,
        "prepared_worker_profile": json.loads(text(os.path.join(directory, "request.json")))["envelope"]["worker_profile"]
        if text(os.path.join(directory, "request.json")) else None,
    }
json.dump(summary, sys.stdout, indent=1, sort_keys=True)
