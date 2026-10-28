#!/usr/bin/env python3
"""Exercise the installed review CLI against isolated real Patchgate/Pijul data."""
# r[impl kiln.patchgate.conformance]
# r[verify kiln.patchgate.conformance] r[verify kiln.patchgate.nonclaims]
# r[verify kiln.patchgate.conformance.scenario.native]
# r[verify kiln.patchgate.profile.scenario.match]

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

REVISION = "97c1216fe13000d9dcbb97f70ddd5423180eea2c"
TIMEOUT_SECONDS = 120
STARTUP_SECONDS = 10
MAX_REQUEST_BYTES = 262144
MAX_REPORT_BYTES = 4194304


def checked(argv, *, cwd=None, env=None, data=None):
    result = subprocess.run(
        [str(value) for value in argv],
        cwd=cwd,
        env=env,
        input=data,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=TIMEOUT_SECONDS,
        check=False,
    )
    if result.returncode:
        raise RuntimeError(
            f"{argv[0]} failed ({result.returncode}): "
            + result.stderr.decode("utf-8", errors="replace")
        )
    return result.stdout


def blake3(data):
    return checked(["b3sum", "--no-names"], data=data).decode().strip()


def external(kind, value):
    return {
        "contract_version": 1,
        "provider_namespace": "pijul",
        "native_kind": kind,
        "native_id": value,
    }


def write_json(path, value):
    path.write_text(json.dumps(value, sort_keys=True, indent=2) + "\n")


def retained_files(repo):
    """Native channel states are checked separately from mutable DB lock files."""
    files = {}
    for path in sorted(repo.rglob("*")):
        relative = path.relative_to(repo)
        if relative.parts[0] == ".pijul" and relative.parts[1:2] != ("patchgate",):
            continue
        if path.is_symlink():
            files[str(relative)] = ["symlink", os.readlink(path)]
        elif path.is_file():
            files[str(relative)] = ["file", path.stat().st_mode, blake3(path.read_bytes())]
        elif path.is_dir():
            files[str(relative)] = ["directory", path.stat().st_mode]
    return files


def exercise(args, root, environment):
    repo = root / "repo"
    repo.mkdir()
    home = root / "home"
    home.mkdir()
    environment = dict(environment, HOME=str(home))

    def pijul(*argv):
        return checked([args.pijul, *argv], cwd=repo, env=environment).decode()

    def record(path, content, message):
        target = repo / path
        fresh = not target.exists()
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content)
        if fresh:
            pijul("add", "--", path)
        return pijul("record", "-a", "-m", message, "--author", "kiln-fixture").split()[-1]

    def channel_state(channel):
        entries = json.loads(pijul("log", "--channel", channel, "--state", "--output-format", "json"))
        return entries[0]["state"] if entries else None

    pijul("init")
    policy = "Accept safe documentation. Escalate protected source changes. Reject credentials.\n"
    values = {
        "policy_path": "POLICY.md",
        "protected": ["src/**"],
        "documentation_paths": ["docs/**"],
        "max_change_chars": 8000,
        "timeout_seconds": 300,
        "check_timeout_seconds": 900,
        "max_review_requests": 16,
        "review_timeout_seconds": 60,
    }
    config_text = json.dumps({"schema_version": 1, "settings": values}, sort_keys=True) + "\n"
    for name, content in {
        "POLICY.md": policy,
        "patchgate.json": config_text,
        "docs/readme.md": "Base documentation.\n",
        "notes/sample.txt": "No credential.\n",
        "src/lib.rs": "pub const VALUE: u8 = 1;\n",
    }.items():
        target = repo / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content)
        pijul("add", "--", name)
    pijul("record", "-a", "-m", "Define trusted fixture", "--author", "kiln-fixture")
    pijul("fork", "--channel", "main", "inbox")
    pijul("channel", "switch", "inbox")
    candidates = {
        "accepted": record("docs/readme.md", "Document the public behavior.\n", "Document behavior"),
        "rejected": record(
            "notes/sample.txt",
            "-----BEGIN " + "OPENSSH PRIVATE KEY-----\nfixture data\n",
            "Candidate containing a supported credential header",
        ),
        "escalated": record("src/lib.rs", "pub const VALUE: u8 = 2;\n", "Change protected source"),
    }
    # Each frozen source is a measured native archive of exactly main plus that
    # change and its native prerequisites, not the dirty inbox working copy.
    snapshots = {}
    for name, change in candidates.items():
        channel = "snapshot-" + name
        pijul("fork", "--channel", "main", channel)
        pijul("apply", "--channel", channel, change)
        archive = root / (name + ".tar.gz")
        pijul("archive", "--channel", channel, "-o", str(archive))
        snapshots[name] = blake3(archive.read_bytes())

    (repo / "docs/readme.md").write_text("Dirty author edits must survive.\n")
    (repo / "untracked.txt").write_text("Untracked author data must survive.\n")
    gate = repo / ".pijul/patchgate"
    gate.mkdir(parents=True, exist_ok=True)
    (gate / "ledger.jsonl").write_bytes(b"")
    (gate / "owner-note").write_text("Existing gate state must survive.\n")
    initial_channels = sorted(pijul("channel").splitlines())
    channels = ["main", "inbox", *("snapshot-" + name for name in candidates)]
    initial_states = {name: channel_state(name) for name in channels}
    before = retained_files(repo)
    profile = {
        "source_revision": REVISION,
        "report_schema": 1,
        "program": str(args.patchgate),
        "executable_identity": blake3(args.patchgate.read_bytes()),
        "repository_root": str(repo),
        # No caller SSH agent or model credentials enter review. All three
        # fixture paths decide deterministically without contacting a model.
        "environment": {"HOME": str(home), "PATH": environment["PATH"]},
        "provider": {
            "contract_version": 1,
            "profile_id": "patchgate-native-fixture",
            "role": "Collaboration",
            "namespace": {
                "contract_version": 1,
                "provider_namespace": "patchgate",
                "native_kind": "review-provider",
                "native_id": REVISION,
            },
            "operations": ["Observe", "Reconcile"],
            "outcomes": [],
            "limits": {
                "max_request_bytes": MAX_REQUEST_BYTES,
                "max_result_bytes": MAX_REPORT_BYTES,
                "max_in_flight": 1,
            },
        },
        "native": {
            "main": {"channel": "main", "state": initial_states["main"]},
            "inbox": {"channel": "inbox", "state": initial_states["inbox"]},
            "configuration": {
                "source": "main",
                "path": "patchgate.json",
                "sha256": hashlib.sha256(config_text.encode()).hexdigest(),
                "values": values,
            },
            "policy": {"path": "POLICY.md", "sha256": hashlib.sha256(policy.encode()).hexdigest()},
        },
        "limits": {
            "max_changes": 32,
            "max_edges": 256,
            "max_request_bytes": MAX_REQUEST_BYTES,
            "max_report_bytes": MAX_REPORT_BYTES,
            "max_stderr_bytes": 65536,
            "max_executable_bytes": 134217728,
            "timeout_ms": 90000,
            "poll_interval_ms": 10,
            "teardown_timeout_ms": 5000,
        },
    }
    profile_path = root / "profile.json"
    write_json(profile_path, profile)
    receipts = {}
    for expected, change in candidates.items():
        change_ref = external("change", change)
        intent = {
            "effect_id": "review-" + expected,
            "candidate": {
                "id": "candidate-" + expected,
                "ordered_changes": [{
                    "change": change_ref,
                    "target_revision": external("state", initial_states["main"] or "empty"),
                    "source_snapshot": snapshots[expected],
                    "pipeline_revision": "native-fixture-pipeline",
                    "collaboration_provider": "patchgate-native-fixture",
                }],
                "dependencies": [],
                "frozen_sources": [{"change": change_ref, "snapshot": snapshots[expected]}],
                "policy_id": "native-fixture-policy",
            },
            "pipeline_revision": "native-fixture-pipeline",
        }
        intent_path, plan_path = root / "intent.json", root / "plan.json"
        write_json(intent_path, intent)
        plan = checked([
            args.adapter, "plan", "--profile", profile_path, "--intent", intent_path,
        ])
        plan_path.write_bytes(plan)
        receipt = json.loads(checked([
            args.adapter, "review", "--profile", profile_path, "--plan", plan_path,
        ]))
        observation = receipt["observation"]
        assert observation["reviews"] == [{"change": change_ref, "decision": expected}], receipt
        assert observation["isolated"] is True and observation["authorizes_acceptance"] is False
        native = json.loads(receipt["native_report"])
        assert native["dry_run"] is True and native["authorizes_acceptance"] is False
        assert native["review_usage"]["requests"] == 0, "fixture unexpectedly called a model"
        assert observation["report_identity"] == blake3(receipt["native_report"].encode())
        receipt_path = root / "receipt.json"
        write_json(receipt_path, receipt)
        restored = json.loads(checked([
            args.adapter, "reconcile", "--profile", profile_path, "--plan", plan_path,
            "--receipt", receipt_path,
        ]))
        assert restored == receipt, "retained evidence reconciliation changed exact evidence"
        if expected == "accepted":
            bad_plan = json.loads(plan)
            bad_plan["effect_id"] = "different-effect"
            bad_plan_path = root / "different-plan.json"
            write_json(bad_plan_path, bad_plan)
            tampered = dict(receipt)
            tampered["native_report"] += "\n"
            tampered_path = root / "tampered-receipt.json"
            write_json(tampered_path, tampered)
            duplicate_intent = json.loads(json.dumps(intent))
            duplicate_intent["candidate"]["frozen_sources"] *= 2
            duplicate_path = root / "duplicate-intent.json"
            write_json(duplicate_path, duplicate_intent)
            for rejected_command in [
                ["reconcile", "--profile", profile_path, "--plan", bad_plan_path, "--receipt", receipt_path],
                ["reconcile", "--profile", profile_path, "--plan", plan_path, "--receipt", tampered_path],
                ["plan", "--profile", profile_path, "--intent", duplicate_path],
            ]:
                refusal = subprocess.run(
                    [str(args.adapter), *map(str, rejected_command)],
                    capture_output=True, timeout=TIMEOUT_SECONDS, check=False,
                )
                assert refusal.returncode != 0 and not refusal.stdout, "invalid evidence produced a receipt"
        current_channels = sorted(pijul("channel").splitlines())
        assert current_channels == initial_channels, (initial_channels, current_channels)
        assert {name: channel_state(name) for name in initial_states} == initial_states
        assert retained_files(repo) == before, "review changed source, work files or gate audit state"
        receipts[expected] = receipt
    return {
        "schema": "kiln.patchgate.native-smoke.v1",
        "source_revision": REVISION,
        "patchgate": str(args.patchgate),
        "patchgate_blake3": profile["executable_identity"],
        "pijul": str(args.pijul),
        "pijul_version": pijul("--version").strip(),
        "adapter": str(args.adapter),
        "adapter_blake3": blake3(args.adapter.read_bytes()),
        "initial_states": initial_states,
        "source_work_and_audit_unchanged": True,
        "reconciled_exact_evidence": True,
        "stale_request_tampered_report_duplicate_source_refused": True,
        "model_requests": 0,
        "cases": receipts,
        "non_claims": [
            "no model accuracy, remote delivery, deployment or release qualification",
            "no source mutation or acceptance authority",
            "no compatibility claim for Kiln's beta.11 Pijul library adapter",
            "fixture source identities hash measured native archives, not a new tree identity profile",
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--adapter", type=Path, required=True)
    parser.add_argument("--patchgate", type=Path, required=True)
    parser.add_argument("--pijul", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    for name in ["adapter", "patchgate", "pijul"]:
        setattr(args, name, getattr(args, name).resolve(strict=True))
    for command in ["b3sum", "ssh-keygen", "ssh-agent", "ssh-add"]:
        if shutil.which(command) is None:
            parser.error(f"required fixture tool is unavailable: {command}")
    with tempfile.TemporaryDirectory(prefix="kpg-") as temporary:
        root = Path(temporary)
        key, socket = root / "key", root / "agent"
        checked(["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", key])
        agent = subprocess.Popen(
            ["ssh-agent", "-D", "-a", str(socket)],
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        try:
            deadline = time.monotonic() + STARTUP_SECONDS
            while not socket.exists():
                if agent.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError("fixture signer did not become ready")
                time.sleep(0.01)
            environment = {"PATH": os.environ["PATH"], "SSH_AUTH_SOCK": str(socket)}
            checked(["ssh-add", "-q", key], env=environment)
            result = exercise(args, root, environment)
        finally:
            agent.terminate()
            agent.wait(timeout=STARTUP_SECONDS)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        write_json(args.output, result)
    print(json.dumps(result, sort_keys=True, indent=2))


if __name__ == "__main__":
    main()
