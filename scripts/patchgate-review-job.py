#!/usr/bin/env python3
"""Produce one kiln.patchgate.review.v1 receipt for a pending Pijul inbox change.

Runs inside a Kiln job checkout. Reads only the named local repositories, never
contacts a model unless Patchgate itself decides a change needs one, and writes
the receipt where the worker declares it for upload.
"""
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

PIN = "97c1216fe13000d9dcbb97f70ddd5423180eea2c"
MAX_REQUEST_BYTES = 262144
MAX_REPORT_BYTES = 4194304


def run(argv, cwd=None, env=None):
    result = subprocess.run([str(a) for a in argv], cwd=cwd, env=env,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if result.returncode:
        raise SystemExit(f"{argv[0]} failed ({result.returncode}): "
                         + result.stderr.decode("utf-8", "replace")[:2000])
    return result.stdout


def blake3_file(path):
    return run(["b3sum", "--no-names", path]).decode().strip()


def blake3_bytes(data):
    result = subprocess.run(["b3sum", "--no-names"], input=data,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if result.returncode:
        raise SystemExit("b3sum failed on in-memory bytes")
    return result.stdout.decode().strip()


def external(kind, value):
    return {"contract_version": 1, "provider_namespace": "pijul",
            "native_kind": kind, "native_id": value}


def channel_state(pijul, repo, channel, env):
    raw = run([pijul, "log", "--channel", channel, "--state", "--output-format", "json",
               "--repository", repo], env=env)
    entries = json.loads(raw or b"[]")
    return entries[0]["state"] if entries else None


def status(patchgate, repo, env):
    return json.loads(run([patchgate, "status", "--json"], cwd=repo, env=env))


def snapshot(repo, pijul, change, work, env):
    """Canonical manifest identity of main plus one change.

    Archive bytes carry mtimes and are not reproducible; the snapshot identity is
    instead the BLAKE3 of a sorted path/mode/content-hash manifest of exactly the
    snapshot channel's tracked files.
    """
    copy = work / ("snapshot-" + change[:12])
    shutil.copytree(repo, copy, symlinks=True)
    channel = "snapshot-" + change[:12]
    run([pijul, "fork", "--channel", "main", channel, "--repository", copy], env=env)
    run([pijul, "apply", "--channel", channel, change, "--repository", copy], env=env)
    run([pijul, "channel", "switch", channel], cwd=copy, env=env)
    lines = []
    for path in sorted(copy.rglob("*")):
        relative = path.relative_to(copy)
        if relative.parts[:1] in ((".pijul",), (".git",)):
            continue
        if path.is_symlink():
            lines.append(f"{relative}\0symlink\0{os.readlink(path)}")
        elif path.is_file():
            lines.append(f"{relative}\0{path.stat().st_mode:o}\0{blake3_file(path)}")
    return blake3_bytes("\n".join(lines).encode())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", required=True)
    parser.add_argument("--patchgate", required=True)
    parser.add_argument("--adapter", required=True)
    parser.add_argument("--pijul", required=True)
    parser.add_argument("--out", default="patchgate-review.json")
    parser.add_argument("--home", default="/home/brittonr/git/.patchgate-review-home",
                        help="stable HOME bound into the review profile so identical native states reproduce an identical receipt")
    parser.add_argument("--pipeline", default="patchgate-review-v1")
    args = parser.parse_args()

    target = Path(args.target).resolve()
    home = Path(args.home).resolve()
    home.mkdir(parents=True, exist_ok=True)
    env = {"HOME": str(home), "PATH": os.environ.get("PATH", "/run/current-system/sw/bin")}
    work = Path(tempfile.mkdtemp(prefix="patchgate-work-"))
    try:
        observed = status(args.patchgate, target, env)
        items = observed.get("items") or []
        if observed.get("behind"):
            raise SystemExit("inbox is behind main; refusing to review")
        if not items:
            raise SystemExit("no pending inbox change to review")

        repo = target
        config_text = (repo / "patchgate.json").read_text()
        config_values = json.loads(config_text)["settings"]
        policy_text = (repo / config_values["policy_path"]).read_text()
        main_state = channel_state(args.pijul, repo, "main", env)
        inbox_state = channel_state(args.pijul, repo, "inbox", env)

        ordered, edges, snapshots = [], [], {}
        for index, item in enumerate(items):
            change = item["hash"]
            snapshots[change] = snapshot(repo, args.pijul, change, work, env)
            ordered.append({
                "change": external("change", change),
                "target_revision": external("state", main_state or "empty"),
                "source_snapshot": snapshots[change],
                "pipeline_revision": args.pipeline,
                "collaboration_provider": "patchgate-review-provider",
            })
        for index, item in enumerate(items):
            for need in item.get("needs") or []:
                for earlier, other in enumerate(items):
                    if other["hash"] == need.rstrip("?"):
                        edges.append({"before_index": earlier, "after_index": index})
        candidates = {item["hash"]: item for item in items}

        profile = {
            "source_revision": PIN,
            "report_schema": 1,
            "program": str(Path(args.patchgate).resolve()),
            "executable_identity": blake3_file(Path(args.patchgate).resolve()),
            "repository_root": str(repo),
            "environment": {"HOME": str(home), "PATH": env["PATH"]},
            "provider": {
                "contract_version": 1,
                "profile_id": "patchgate-review-provider",
                "role": "Collaboration",
                "namespace": {"contract_version": 1, "provider_namespace": "patchgate",
                              "native_kind": "review-provider", "native_id": PIN},
                "operations": ["Observe", "Reconcile"],
                "outcomes": [],
                "limits": {"max_request_bytes": MAX_REQUEST_BYTES,
                           "max_result_bytes": MAX_REPORT_BYTES, "max_in_flight": 1},
            },
            "native": {
                "main": {"channel": "main", "state": main_state},
                "inbox": {"channel": "inbox", "state": inbox_state},
                "configuration": {
                    "source": "main",
                    "path": "patchgate.json",
                    "sha256": hashlib.sha256(config_text.encode()).hexdigest(),
                    "values": config_values,
                },
                "policy": {"path": config_values["policy_path"],
                           "sha256": hashlib.sha256(policy_text.encode()).hexdigest()},
            },
            "limits": {
                "max_changes": 32, "max_edges": 256,
                "max_request_bytes": MAX_REQUEST_BYTES,
                "max_report_bytes": MAX_REPORT_BYTES,
                "max_stderr_bytes": 65536,
                "max_executable_bytes": 134217728,
                "timeout_ms": 900000, "poll_interval_ms": 10,
                "teardown_timeout_ms": 5000,
            },
        }
        profile_path = work / "profile.json"
        profile_path.write_text(json.dumps(profile, sort_keys=True, indent=2) + "\n")

        intent = {
            "effect_id": "review-" + (inbox_state or "unknown")[:32],
            "pipeline_revision": args.pipeline,
            "candidate": {
                "id": "candidate-" + (inbox_state or "unknown")[:32],
                "ordered_changes": ordered,
                "dependencies": edges,
                "frozen_sources": [{"change": external("change", change), "snapshot": snapshots[change]}
                                   for change in candidates],
                "policy_id": "patchgate-review-policy-v1",
            },
        }
        intent_path = work / "intent.json"
        intent_path.write_text(json.dumps(intent, sort_keys=True, indent=2) + "\n")

        plan = run([args.adapter, "plan", "--profile", profile_path, "--intent", intent_path])
        (work / "plan.json").write_bytes(plan)
        receipt = run([args.adapter, "review", "--profile", profile_path,
                       "--plan", work / "plan.json"])
        parsed = json.loads(receipt)
        Path(args.out).write_bytes(receipt)
        summary = {
            "receipt": str(Path(args.out)),
            "schema": parsed.get("schema"),
            "decisions": [(r.get("change"), r.get("decision"))
                          for r in parsed.get("observation", {}).get("reviews", [])],
            "authorizes_acceptance": parsed.get("observation", {}).get("authorizes_acceptance"),
        }
        print(json.dumps(summary, indent=2))
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()
