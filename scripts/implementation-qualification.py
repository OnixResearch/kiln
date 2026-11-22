#!/usr/bin/env python3
"""Bounded, fail-closed #50 qualification over actual implementation CLIs.

No remote mutation command is exposed. JSON input is the export of the adjacent
Nickel contract, not worker-provided evidence. See docs/implementation-qualification.md.
"""
# r[impl kiln.implementation_qualification.real_stages]
# r[impl kiln.implementation_qualification.scoped_evidence]
# r[impl kiln.implementation_qualification.approval]
import argparse
import concurrent.futures
import hmac
import json
import os
from pathlib import Path
import re
import signal
import sqlite3
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
SCHEMA = "kiln.implementation-qualification.v1"
MAX_BYTES = 16 * 1024 * 1024
MAX_STEPS = 128
MAX_SECONDS = 3600
OMP_REVISION = "e325ffc73ab86e03bdd20a91b77d8e2b3964c906"
REQUIRED = {
    "bug": {"claim", "reserve-plan", "lattice-prepare", "lattice-run", "capture-candidate", "verify"},
    "feature": {"claim", "reserve-plan", "lattice-prepare", "lattice-run", "capture-candidate", "verify"},
    "linked-sources": {"drain", "cairn-intake", "claim"},
    "expired-stale-worker": {"claim", "recover", "capture-candidate"},
    "zero-exit-noop": {"lattice-run", "capture-candidate"},
    "forged-evidence": {"capture-candidate", "verify"},
    "skipped-checks": {"verify"},
    "weakened-policy": {"verify"},
    "source-policy-movement": {"drain", "verify"},
    "target-movement": {"prepare-publication"},
    "restart-possible-effects": {"lattice-crash", "lattice-recover", "recover"},
    "descendant-cancellation": {"lattice-cancel", "lattice-recover"},
    "already-satisfied": {"lattice-run", "capture-candidate", "verify"},
}
# These are the actual bounded consumer routes, not user-supplied shell commands.
STAGES = {
    "drain": ("coordinator", ["drain"]),
    "cairn-intake": ("cairn_intake", []),
    "claim": ("coordinator", ["claim"]),
    "renew": ("coordinator", ["renew"]),
    "cancel": ("coordinator", ["cancel"]),
    "recover": ("coordinator", ["recover"]),
    "reserve-plan": ("coordinator", ["reserve-plan"]),
    "start": ("coordinator", ["start"]),
    "reconcile": ("coordinator", ["reconcile"]),
    "handoff": ("coordinator", ["handoff"]),
    "capture-candidate": ("coordinator", ["capture-candidate"]),
    "verify": ("verifier", ["verify-implementation"]),
    "prepare-verification-profile": ("coordinator", ["prepare-verification-profile"]),
    "prepare-needs": ("coordinator", ["prepare-needs"]),
    "attach-needs": ("coordinator", ["attach-needs"]),
    "admission": ("coordinator", ["admission"]),
    "admission-status": ("coordinator", ["admission-status"]),
    "prepare-publication": ("coordinator", ["prepare-publication"]),
    "status": ("coordinator", ["status"]),
    "metrics": ("coordinator", ["metrics"]),
    "inspect-work": ("coordinator", ["inspect-work"]),
    "lattice-dispatch": ("lattice_dispatch", []),
    "lattice-prepare": ("lattice", ["implementation-attempt", "prepare"]),
    "lattice-run": ("lattice_dispatch", []),
    "lattice-recover": ("lattice_dispatch", ["--recover", "true"]),
    "lattice-cancel": ("lattice_dispatch", []),
    "lattice-crash": ("lattice_dispatch", []),
}


class Missing(RuntimeError):
    pass


def encode(value):
    return json.dumps(value, separators=(",", ":"), ensure_ascii=False).encode()


def load(path):
    path = Path(path)
    if path.stat().st_size > MAX_BYTES:
        raise ValueError(f"input exceeds {MAX_BYTES} bytes: {path}")
    return json.loads(path.read_bytes())


def put(path, value):
    path = Path(path)
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "wb") as stream:
        stream.write(value if isinstance(value, bytes) else encode(value) + b"\n")


def pointer(value, path):
    if not path:
        return value
    if not path.startswith("/"):
        raise ValueError("JSON observation must use an RFC6901 pointer")
    for key in path[1:].split("/"):
        key = key.replace("~1", "/").replace("~0", "~")
        value = value[int(key)] if isinstance(value, list) else value[key]
    return value


def expand(value, captures):
    """Whole-value substitutions preserve number/null/object types; no shell eval."""
    if isinstance(value, str) and value.startswith("@{") and value.endswith("}"):
        name, _, path = value[2:-1].partition("#")
        return pointer(captures[name], path)
    if isinstance(value, dict):
        return {key: expand(item, captures) for key, item in value.items()}
    if isinstance(value, list):
        return [expand(item, captures) for item in value]
    return value


def descendants(pid):
    children = {}
    for status in Path("/proc").glob("[0-9]*/status"):
        try:
            fields = dict(line.split(":", 1) for line in status.read_text().splitlines() if ":" in line)
            children.setdefault(int(fields["PPid"].strip()), []).append(int(status.parent.name))
        except (FileNotFoundError, PermissionError, KeyError, ValueError, ProcessLookupError):
            continue
    found, pending = [], [pid]
    while pending:
        current = pending.pop()
        found.append(current)
        pending.extend(children.get(current, []))
    return found


def incarnation(pid):
    try:
        # comm can contain spaces and parentheses; starttime is field 22.
        return Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()[19]
    except (FileNotFoundError, PermissionError, ProcessLookupError):
        return None


def process_is_mutating(pid, identity):
    try:
        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
        return fields[19] == identity and fields[0] not in ("Z", "X")
    except (FileNotFoundError, ProcessLookupError):
        return False


def sent_requests(output_directory, admitted):
    """Requests Kiln froze for `admitted` before a worker launch.

    Kiln freezes the bytes it sends as `request-<BLAKE3 of those bytes>.json`
    in the transport output directory: the admitted request, plus its budget
    lineage when the attempt holds an admitted reservation. Lattice's intent
    names that digest, never the admitted request's.
    """
    found = []
    for index, path in enumerate(sorted(Path(output_directory).glob("request-*.json"))):
        if index >= MAX_STEPS:
            raise ValueError("frozen request scan exceeds bounded qualification cohort")
        if not path.is_file() or path.stat().st_size > MAX_BYTES:
            continue
        try:
            value = json.loads(path.read_bytes())
        except ValueError:
            continue
        if isinstance(value, dict):
            value.pop("lineage", None)
        if value == admitted:
            found.append(path)
    return found


def retained_worker_barrier(state_directory, request_digests):
    """Discover an actual owner-written intent for one of the sent requests;
    do not duplicate its path hash. Returns the native files and the digest."""
    for index, intent in enumerate(Path(state_directory).glob("*/intent.json")):
        if index >= MAX_STEPS:
            raise ValueError("native intent scan exceeds bounded qualification cohort")
        if intent.stat().st_size != 64:
            continue
        digest = intent.read_bytes().decode(errors="replace")
        if digest not in request_digests:
            continue
        files = {name: intent.parent / filename for name, filename in
                 (("raw_events", "raw-events.jsonl"), ("omp_observation", "omp-observation.json"))}
        if all(path.is_file() and path.stat().st_size <= MAX_BYTES for path in files.values()):
            return files, digest
    return None


def retained_intent_digests(state_directory):
    """Request digests named by Lattice's retained intents in one state root."""
    digests = set()
    for index, intent in enumerate(Path(state_directory).glob("*/intent.json")):
        if index >= MAX_STEPS:
            raise ValueError("native intent scan exceeds bounded qualification cohort")
        if intent.stat().st_size == 64:
            digests.add(intent.read_bytes().decode(errors="replace"))
    return digests


def validate_scenario(scenario):
    """Validate qualification scope before any process or configuration write."""
    name, steps = scenario["name"], scenario["steps"]
    if name not in REQUIRED or not 0 < len(steps) <= MAX_STEPS:
        raise ValueError("unknown scenario or stage count outside 1..128")
    stages = {step["stage"] for step in steps}
    if not stages <= set(STAGES):
        raise ValueError("unsupported native stage; remote effects are deliberately unavailable")
    if not REQUIRED[name] <= stages:
        raise Missing(f"{name}: missing actual stages {sorted(REQUIRED[name] - stages)}")
    if not any(step.get("observations") for step in steps):
        raise Missing(f"{name}: no explicit native-output assertions")
    verifications = [step for step in steps if step["stage"] == "verify"]
    if name in ("bug", "feature", "already-satisfied") and not any(step["exit_code"] == 0 for step in verifications):
        raise Missing(f"{name}: actual successful independent verification required")
    if name in ("bug", "feature") and any(not any(step["stage"] == stage and step["exit_code"] == 0 for step in steps) for stage in REQUIRED[name]):
        raise Missing(f"{name}: every real-path production stage must actually succeed")
    if name == "descendant-cancellation" and any(step["stage"] == "lattice-cancel" and step["exit_code"] != 0 for step in steps):
        raise Missing("failed cancellation is unqualified, never a passing negative-control exit")
    if name == "restart-possible-effects":
        crash_indices = [index for index, step in enumerate(steps) if step["stage"] == "lattice-crash"]
        if len(crash_indices) != 1 or steps[crash_indices[0]]["exit_code"] != -signal.SIGKILL:
            raise Missing("restart scenario requires exactly one real native-effect crash, never a successful zero-exit stand-in")
        if not any(step["stage"] == "lattice-recover" and step["exit_code"] == 0 for step in steps[crash_indices[0] + 1:]):
            raise Missing("restart scenario requires successful native read-only recovery after the crash")
    rejected = {"expired-stale-worker", "zero-exit-noop", "forged-evidence", "skipped-checks", "weakened-policy", "source-policy-movement", "target-movement"}
    if name in rejected and not any(step["exit_code"] != 0 and step.get("stderr_contains") for step in steps if step["stage"] in ("verify", "handoff", "capture-candidate", "prepare-publication")):
        raise Missing(f"{name}: actual admission rejection and exact diagnostic required")
    names = [step["capture"] for step in steps]
    if len(names) != len(set(names)) or any(value in ("inputs", "output", "stage") or not re.fullmatch(r"[A-Za-z0-9_]{1,128}", value) for value in names):
        raise ValueError("stage capture names must be unique and safe")
    for step in steps:
        if type(step["exit_code"]) is not int or type(step["timeout_seconds"]) is not int or not 1 <= step["timeout_seconds"] <= MAX_SECONDS:
            raise ValueError("expected exit and bounded timeout must be integers")
        if not isinstance(step["arguments"], list) or len(step["arguments"]) > 64 or any(not isinstance(arg, str) or "\0" in arg for arg in step["arguments"]):
            raise ValueError("native argv must be a bounded array of NUL-free strings")
        if step["stage"] == "lattice-crash" and (type(step.get("crash_after_seconds")) is not int or not 1 <= step["crash_after_seconds"] <= MAX_SECONDS):
            raise Missing("native-effect crash requires an explicit bounded observation interval")
    return name, steps, stages


class Runner:
    def __init__(self, inputs, output):
        self.inputs = inputs
        self.output = Path(output).resolve()
        self.output.mkdir(mode=0o700, parents=False, exist_ok=False)
        self.programs = {}
        self.frozen = {}
        self.receipt = {"schema": SCHEMA, "scope": "local-only", "scenarios": {},
                        "verified_pr": "not_exercised_requires_operator_approval",
                        "landing": "not_exercised_requires_provider_qualification_and_operator_approval",
                        "source_reconciliation": "not_exercised_requires_integration_and_operator_approval",
                        "live_worker": "not_exercised", "suite": "unqualified"}
        self.receipt["remote_recovery_windows"] = {stage: "not_exercised_requires_point_of_risk_approval"
                                                   for stage in ("branch_push", "pr_creation", "integration", "source_update")}
        self.environment = {"LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "GIT_CONFIG_NOSYSTEM": "1",
                            "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_TERMINAL_PROMPT": "0"}
        self.environment.update(inputs.get("environment", {}))
        put(self.output / "input-configuration.json", inputs)

    def digest(self, path):
        program = self.inputs["programs"]["b3sum"]["path"]
        result = subprocess.run([program, "--", str(path)], capture_output=True, timeout=30,
                                env=self.environment, check=True)
        digest = result.stdout.decode().split()[0]
        if not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise ValueError("b3sum did not return a BLAKE3 identity")
        return digest

    def freeze(self, path, expected, label):
        path = Path(path).resolve(strict=True)
        actual = self.digest(path)
        if actual != expected:
            raise ValueError(f"{label}: identity differs")
        self.frozen[str(path)] = (expected, label)
        return path

    def assert_frozen(self):
        for path, (expected, label) in self.frozen.items():
            try:
                if self.digest(path) != expected:
                    raise Missing(f"{label}: frozen input changed during campaign")
            except (OSError, ValueError, subprocess.SubprocessError) as error:
                raise Missing(f"{label}: frozen input unavailable during campaign: {error}") from error

    def preflight(self, live=False):
        missing = []
        if self.inputs.get("schema") != SCHEMA:
            missing.append(f"input schema must be {SCHEMA}")
        names = {"coordinator", "b3sum"}
        if live:
            names.update({"verifier", "lattice", "cairn_intake", "lattice_dispatch", "setpriv", "omp", "git"})
        for name in sorted(names):
            item = self.inputs.get("programs", {}).get(name)
            if not item:
                missing.append(f"programs.{name}: immutable executable path and reviewed BLAKE3")
                continue
            try:
                path = Path(item["path"]).resolve(strict=True)
                if not str(path).startswith("/nix/store/") or not path.is_file() or not os.access(path, os.X_OK):
                    raise ValueError("must resolve to executable in /nix/store")
                if not re.fullmatch(r"[0-9a-f]{64}", item["blake3"]):
                    raise ValueError("must supply reviewed lowercase BLAKE3")
                self.programs[name] = str(path)
            except (KeyError, OSError, ValueError) as error:
                missing.append(f"programs.{name}: {error}")
        if "b3sum" in self.programs:
            for name, path in self.programs.items():
                try:
                    self.freeze(path, self.inputs["programs"][name]["blake3"], f"programs.{name}")
                except (OSError, ValueError, subprocess.SubprocessError) as error:
                    missing.append(f"programs.{name}: {error}")
        if live:
            for name in ("cohort", "source_baseline", "policy", "toolchain", "provider", "coordinator_uid", "coordinator_gid", "worker_uid", "worker_gid", "verifier_uid", "verifier_gid"):
                if name not in self.inputs:
                    missing.append(f"{name}: actual admitted identity/authority, not a fixture value")
            uids = [self.inputs.get(role + "_uid") for role in ("coordinator", "worker", "verifier")]
            if len(set(uids)) != 3:
                missing.append("distinct existing coordinator_uid, worker_uid and verifier_uid")
            for role in ("coordinator", "worker", "verifier"):
                uid = self.inputs.get(role + "_uid")
                if uid is not None and os.geteuid() not in (0, uid):
                    missing.append(f"cannot execute {role} UID {uid}: run as that UID or authorized root; no implicit sudo")
            for name in ("source_baseline", "policy", "toolchain", "provider", "cohort"):
                item = self.inputs.get(name)
                if not isinstance(item, dict) or not item.get("path") or not item.get("blake3"):
                    missing.append(f"{name}: retained path and expected BLAKE3 required")
                elif "b3sum" in self.programs:
                    try:
                        self.freeze(item["path"], item["blake3"], name)
                    except (OSError, ValueError, subprocess.SubprocessError) as error:
                        missing.append(f"{name}: {error}")
            if isinstance(self.inputs.get("cohort"), dict) and "path" in self.inputs["cohort"]:
                try:
                    cohort_path = Path(self.inputs["cohort"]["path"]).resolve(strict=True)
                    cohort = load(cohort_path)
                    if not str(cohort_path).startswith("/nix/store/") or cohort["schema"] != "onix.kiln.implementation-cohort.v1" or cohort["workerAdapter"] != "omp" or cohort["revisions"]["worker"] != OMP_REVISION:
                        raise ValueError("requires immutable actual onix-core cohort and exact OMP revision")
                    for program, entry in {"coordinator": "coordinator", "verifier": "verifier", "lattice": "lattice", "omp": "worker", "cairn_intake": "cairn_intake", "lattice_dispatch": "dispatch"}.items():
                        declared = cohort["entries"][entry]
                        if self.programs.get(program) != declared["resolved_path"] or self.inputs["programs"][program]["blake3"] != declared["blake3"]:
                            raise ValueError(f"{program} differs from adopted cohort")
                except (KeyError, OSError, ValueError) as error:
                    missing.append(f"cohort: {error}")
            for scenario in self.inputs.get("scenarios", [])[:len(REQUIRED)]:
                for step in scenario.get("steps", [])[:MAX_STEPS]:
                    if step.get("stage") != "verify":
                        continue
                    item = step.get("canonical_profile")
                    if not isinstance(item, dict) or not item.get("path") or not item.get("blake3"):
                        missing.append(f"{scenario.get('name')}: exact canonical CI service profile/node/Cairn mapping required")
                    elif "b3sum" in self.programs:
                        try:
                            self.freeze(item["path"], item["blake3"], f"{scenario.get('name')}: canonical CI profile")
                        except (OSError, ValueError, subprocess.SubprocessError) as error:
                            missing.append(f"{scenario.get('name')}: canonical CI profile: {error}")
            if not self.inputs.get("scenarios"):
                missing.append("scenarios: exact enrolled bug/feature and adverse-case bundles")
        harness = {"path": str(Path(__file__).resolve()), "python": str(Path(sys.executable).resolve())}
        if "b3sum" in self.programs:
            put(self.output / "harness-source.py", Path(__file__).read_bytes())
            harness["blake3"] = self.digest(self.output / "harness-source.py")
            harness["python_blake3"] = self.digest(harness["python"])
            if not harness["python"].startswith("/nix/store/"):
                missing.append("qualification Python interpreter must resolve to an immutable Nix-store runtime")
        put(self.output / "preflight.json", {"missing": missing, "programs": self.programs,
                                             "actual_uid": os.geteuid(), "live_requested": live, "harness": harness})
        if missing:
            raise Missing("; ".join(missing))

    def prepare_campaign_inputs(self):
        # Only these nonsecret protocol inputs are traversable by the worker.
        # Native reports/raw outputs stay in separate private scenario directories.
        self.output.chmod(0o711)
        self.input_root = self.output / "stage-inputs"
        self.native_root = self.output / "native-receipts"
        for path, mode in ((self.input_root, 0o755), (self.native_root, 0o700)):
            path.mkdir(mode=mode)
            if os.geteuid() == 0:
                os.chown(path, self.inputs["verifier_uid"], self.inputs["verifier_gid"])

    def command(self, case, label, argv, expected=0, contains=None, timeout=120, role=None, cancel_after=None, mutation_probe=None, crash=None):
        if not 0 < timeout <= MAX_SECONDS:
            raise ValueError("stage timeout must be 1..3600 seconds")
        directory = self.output / case / label
        directory.mkdir(mode=0o700, parents=True, exist_ok=False)
        argv = list(map(str, argv))
        if role:
            uid, gid = self.inputs[role + "_uid"], self.inputs[role + "_gid"]
            if os.geteuid() != uid:
                argv = [self.programs["setpriv"], "--reuid", str(uid), "--regid", str(gid), "--clear-groups", "--", *argv]
        put(directory / "invocation.json", {"argv": argv, "timeout_seconds": timeout,
                                            "role": role, "expected_exit": expected})
        started = time.monotonic_ns()
        observed, observed_ids, cancellation = [], {}, None
        probe_before = probe_at_cancel = None
        stopped = False
        if mutation_probe is not None:
            if not 1 <= mutation_probe["quiet_seconds"] <= 10:
                raise ValueError("cancellation quiescence interval must be 1..10 seconds")
            probe_before = self.digest(Path(mutation_probe["path"]).resolve(strict=True))
        with (directory / "stdout.raw").open("xb") as stdout, (directory / "stderr.raw").open("xb") as stderr:
            process = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr,
                                       env=self.environment, start_new_session=True)
            try:
                while process.poll() is None:
                    elapsed = (time.monotonic_ns() - started) / 1e9
                    if stdout.tell() + stderr.tell() > MAX_BYTES:
                        cancellation = "output_bound"
                        break
                    if elapsed >= timeout:
                        cancellation = "timeout_unknown_effects"
                        break
                    if cancel_after is not None and elapsed >= cancel_after:
                        observed = descendants(process.pid)
                        observed_ids = {str(pid): incarnation(pid) for pid in observed}
                        if mutation_probe is not None:
                            probe_at_cancel = self.digest(mutation_probe["path"])
                        os.kill(process.pid, signal.SIGTERM)
                        cancellation = "requested_sigterm"
                        cancel_after = None
                    if crash is not None and elapsed >= crash["after_seconds"]:
                        current = descendants(process.pid)
                        observed_ids.update({str(pid): incarnation(pid) for pid in current})
                        if not stopped and len(current) > 1:
                            os.kill(process.pid, signal.SIGSTOP)
                            stopped = True
                        if stopped:
                            # Lattice's intent names the BLAKE3 of the bytes Kiln sent,
                            # which carry the budget lineage of an admitted attempt.
                            sent = {self.digest(path) for path in sent_requests(crash["output_directory"], crash["admitted"])}
                            barrier = retained_worker_barrier(crash["state_directory"], sent)
                            files, sent_digest = barrier if barrier is not None else (None, None)
                            active = [pid for pid, identity in observed_ids.items() if int(pid) != process.pid and identity is not None and process_is_mutating(int(pid), identity)]
                            if files is not None and not active:
                                journal = self.ledger(case, crash["database"], label + "-stopped-before-ack")
                                attempt = journal["attempts"]["state"]["attempts"][crash["attempt"]]
                                if attempt.get("workflow") is not None or attempt["fence"] != crash["fence"]:
                                    raise RuntimeError("native acknowledgement already occurred or fence changed; requested crash window was not demonstrated")
                                put(directory / "crash-effect-barrier.json", {
                                    "parent_stopped_before_ack": True, "request_blake3": sent_digest,
                                    "admitted_request_blake3": crash["admitted_request_blake3"],
                                    "native_files": {key: {"path": str(path), "blake3": self.digest(path)} for key, path in files.items()},
                                    "observed_incarnations": observed_ids,
                                    "effect_status": "observed_native_worker_material_acknowledgement_unknown_recovery_required"})
                                os.kill(process.pid, signal.SIGKILL)
                                cancellation = "injected_crash_after_native_material_before_ack"
                                break
                    time.sleep(0.02)
                if cancellation in ("output_bound", "timeout_unknown_effects"):
                    observed = descendants(process.pid)
                    observed_ids = {str(pid): incarnation(pid) for pid in observed}
                    os.killpg(process.pid, signal.SIGKILL)
                code = process.wait()
                if stdout.tell() + stderr.tell() > MAX_BYTES:
                    cancellation = "output_bound"
            except BaseException:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait()
                raise
        survivors = {pid: identity for pid, identity in observed_ids.items()
                     if identity is not None and incarnation(int(pid)) == identity}
        observation = {"exit_code": code, "elapsed_ns": time.monotonic_ns() - started,
                       "cancellation": cancellation, "observed_process_incarnations": observed_ids,
                       "surviving_observed_incarnations": survivors,
                       "stdout_blake3": self.digest(directory / "stdout.raw"),
                       "stderr_blake3": self.digest(directory / "stderr.raw")}
        put(directory / "observation.json", observation)
        if survivors:
            # Emergency cleanup is not evidence of native cancellation propagation.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        if cancellation in ("output_bound", "timeout_unknown_effects"):
            raise RuntimeError(f"{case}/{label}: {cancellation}; no retry or workspace reuse")
        if cancellation == "requested_sigterm" and (len(observed) < 2 or survivors):
            raise RuntimeError(f"{case}/{label}: no actual descendant observed or descendant remained; workspace quarantined")
        if mutation_probe is not None:
            after = self.digest(mutation_probe["path"])
            time.sleep(mutation_probe["quiet_seconds"])
            settled = self.digest(mutation_probe["path"])
            put(directory / "quiescence.json", {"path": mutation_probe["path"], "before": probe_before,
                "at_cancel": probe_at_cancel, "after": after, "settled": settled,
                "quiet_seconds": mutation_probe["quiet_seconds"]})
            if probe_at_cancel is None or probe_before == probe_at_cancel or after != settled:
                raise RuntimeError(f"{case}/{label}: actual mutation then post-cancellation quiescence not established; no workspace reuse")
        if code != expected:
            raise RuntimeError(f"{case}/{label}: exit {code}, expected {expected}; see raw outputs")
        if contains is not None and contains.encode() not in (directory / "stderr.raw").read_bytes():
            raise RuntimeError(f"{case}/{label}: required rejection reason absent")
        raw = (directory / "stdout.raw").read_bytes()
        return json.loads(raw) if raw.strip() else None

    def ledger(self, case, database, label):
        with sqlite3.connect(f"file:{database}?mode=ro", uri=True) as connection:
            rows = connection.execute("SELECT kind, version, bytes FROM implementation_state ORDER BY kind").fetchall()
        value = {kind: {"version": version, "state": json.loads(raw)} for kind, version, raw in rows}
        put(self.output / case / (label + "-ledger.json"), value)
        return value

    def local_admission(self):
        case = "scripted-local-admission"
        directory = self.output / case
        directory.mkdir(mode=0o700)
        database, queue = directory / "work.sqlite", directory / "queue.sqlite"
        secret = os.urandom(32).hex().encode()
        secret_file, enrollment, policy_file = (directory / name for name in ("secret", "enrollment.json", "policy.json"))
        put(secret_file, secret)
        secret_file.chmod(0o600)
        put(enrollment, {"repositories": ["qualification/disposable"], "actors": ["operator"]})
        profile = ROOT / "fixtures/implementation-qualification/acceptance.json"
        policy = {"repository": {"owner": "qualification", "repository": "disposable", "repository_id": 1,
                                  "allow_fork_pull_requests": False},
                  "project": "qualification", "requested_outcome": "scripted admission only",
                  "acceptance": [{"id": "accepted", "statement": "real independent check required"}],
                  "dependencies": [], "permitted_paths": ["src"], "permitted_effects": ["write_source"],
                  "verification_profile": self.digest(profile), "budget": {"attempts": 3, "minutes_per_attempt": 1},
                  "terminal_policy": "verified_pull_request", "priority": 1}
        put(policy_file, policy)
        with sqlite3.connect(queue) as connection:
            connection.execute("CREATE TABLE implementation_deliveries (delivery_id TEXT PRIMARY KEY, event TEXT NOT NULL, signature TEXT NOT NULL, body BLOB NOT NULL, recorded_at_ms INTEGER NOT NULL)")
        def enqueue(index, timestamp, title):
            body = encode({"action": "edited", "repository": {"id": 1, "name": "disposable", "owner": {"login": "qualification"}},
                           "sender": {"login": "operator"}, "issue": {"number": 1, "title": title, "state": "open", "updated_at": timestamp}})
            with sqlite3.connect(queue) as connection:
                connection.execute("INSERT INTO implementation_deliveries VALUES (?, 'issues', ?, ?, ?)",
                                   (f"00000000-0000-0000-0000-{index:012d}", "sha256=" + hmac.digest(secret, body, "sha256").hex(), body, index))
            put(directory / f"delivery-{index}.json", body)
        common = ["--database", database, "--enrollment", enrollment]
        def cli(label, command, *args, **kwargs):
            return self.command(case, label, [self.programs["coordinator"], command, *common, *args], **kwargs)
        drain_args = ["--queue", queue, "--maximum-deliveries", "8", "--secret-file", secret_file,
                      "--policy", policy_file, "--limit", "8"]
        enqueue(1, "2026-11-11T10:00:00Z", "current")
        enqueue(2, "2026-11-11T10:00:00Z", "current")
        enqueue(3, "2026-11-10T10:00:00Z", "older out of order")
        cli("intake", "drain", *drain_args)
        status = cli("intake-status", "status")
        if status["work"] != 1:
            raise RuntimeError("duplicate/out-of-order intake did not retain exactly one current work")
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            claims = list(pool.map(lambda label: cli(label, "claim", "--lease-ms", "5000"), ["scheduler-a", "scheduler-b"]))
        actual = [claim for claim in claims if claim is not None]
        if len(actual) != 1:
            raise RuntimeError("competing schedulers did not result in exactly one current claim")
        old = actual[0]
        self.ledger(case, database, "claimed")
        cli("restart-recover", "recover")  # A new actual CLI process reads the durable ledger.
        if cli("restart-no-duplicate", "claim", "--lease-ms", "60000") is not None:
            raise RuntimeError("restart created a duplicate current attempt")
        remaining = old["lease_until_ms"] / 1000 - time.time()
        if remaining > 0:
            time.sleep(min(remaining + 0.05, 6))
        cli("expire", "recover")
        replacement = cli("replacement", "claim", "--lease-ms", "60000")
        if replacement is None or replacement["identity"] == old["identity"] or replacement["fence"] <= old["fence"]:
            raise RuntimeError("lease expiry did not fence the replacement")
        cli("stale-renew", "renew", "--attempt", old["identity"], "--fence", old["fence"], "--lease-ms", "60000", expected=2, contains="implementation_lease_expired")
        enqueue(4, "2026-11-12T10:00:00Z", "changed source requirements")
        cli("source-change", "drain", *drain_args)
        cli("superseded-renew", "renew", "--attempt", replacement["identity"], "--fence", replacement["fence"], "--lease-ms", "60000", expected=2, contains="implementation_fence_stale")
        final = cli("final-status", "status")
        if final["verified"] or final["published"] or final["integrated"] or final["source_reconciled"]:
            raise RuntimeError("admission-only fixture manufactured completion")
        self.ledger(case, database, "final")
        self.receipt["scenarios"][case] = {"status": "qualified_local_scripted_admission_scope",
            "observed": ["duplicate/out-of-order", "competing current claim", "process restart", "undispatched lease expiry", "stale fence", "source supersession"],
            "not_observed": ["linked Cairn sources", "actual worker", "descendant cancellation", "CI acceptance", "remote effects"]}
        put(directory / "scenario-receipt.json", self.receipt["scenarios"][case])

    def scenario(self, scenario):
        name, steps, stages = validate_scenario(scenario)
        captures = {"output": str(self.output), "inputs": self.inputs}
        for index, step in enumerate(steps):
            stage = step["stage"]
            if stage not in STAGES:
                raise ValueError(f"unsupported stage: {stage}; remote effects are deliberately unavailable")
            label = f"{index:03d}-{stage}"
            captures["stage"] = {"input": str(self.input_root / f"{name}-{label}.json"),
                                 "prepared_request": str(self.input_root / f"{name}-{label}.prepared.json")}
            self.assert_frozen()
            step = expand(step, captures)
            if step.get("input_template"):
                if stage not in ("drain", "cairn-intake", "reserve-plan", "prepare-needs", "attach-needs", "admission", "lattice-prepare", "prepare-verification-profile"):
                    raise ValueError("only native configuration/preparation commands accept input templates; never handoff, CI or effect evidence")
                template = step["input_template"]
                self.freeze(template["path"], template["blake3"], f"{name}/{label}: protocol input template")
                self.assert_frozen()
                path = Path(captures["stage"]["input"])
                put(path, expand(load(template["path"]), captures))
                if os.geteuid() == 0:
                    os.chown(path, self.inputs["verifier_uid"], self.inputs["verifier_gid"])
                path.chmod(0o444)
                captures["input-" + step["capture"]] = str(path)
            if stage in ("lattice-run", "lattice-recover", "lattice-cancel", "lattice-crash"):
                arguments = step["arguments"]
                request_path = Path(arguments[arguments.index("--worker-request") + 1])
                request = load(request_path)
                if str(Path(request["program"]).resolve(strict=True)) != self.programs["omp"] or request["omp"]["executable_blake3"] != self.inputs["programs"]["omp"]["blake3"] or request["omp"]["revision"] != OMP_REVISION:
                    raise Missing(f"{name}: worker request differs from actual adopted OMP executable/revision")
                put(self.output / name / (label + "-admitted-request.json"), request)
            if stage == "lattice-cancel" and (not step.get("cancel_after_seconds") or not step.get("mutation_probe")):
                raise Missing("cancellation requires an actual signal interval and observed mutation/quiescence probe")
            crash = None
            if stage == "lattice-crash":
                if not step.get("crash_after_seconds") or step["exit_code"] != -signal.SIGKILL:
                    raise Missing("restart injection requires an explicit stop interval and actual SIGKILL exit")
                arguments = step["arguments"]
                transport = load(arguments[arguments.index("--worker-transport-profile") + 1])
                crash = {"after_seconds": step["crash_after_seconds"], "state_directory": transport["state_directory"],
                         "output_directory": transport["output_directory"], "admitted": request,
                         "admitted_request_blake3": self.digest(request_path), "database": arguments[arguments.index("--database") + 1],
                         "attempt": request["envelope"]["attempt_id"], "fence": request["envelope"]["fence"]}
            if stage == "verify":
                canonical = step["canonical_profile"]
                arguments = step["arguments"]
                service_profile = arguments[arguments.index("--profile") + 1]
                if Path(service_profile).resolve(strict=True) != Path(canonical["path"]).resolve(strict=True) or self.digest(service_profile) != canonical["blake3"]:
                    raise Missing("canonical verification must use the exact admitted native CI service profile")
            program, prefix = STAGES[stage]
            self.assert_frozen()
            role = "worker" if stage in ("lattice-run", "lattice-recover", "lattice-cancel", "lattice-crash") else "verifier" if stage in ("verify", "capture-candidate", "prepare-verification-profile", "prepare-needs", "lattice-prepare") else "coordinator"
            result = self.command(name, label, [self.programs[program], *prefix, *step["arguments"]],
                                  expected=step["exit_code"], contains=step.get("stderr_contains"),
                                  timeout=step["timeout_seconds"], role=role,
                                  cancel_after=step.get("cancel_after_seconds") if stage == "lattice-cancel" else None,
                                  mutation_probe=step.get("mutation_probe") if stage == "lattice-cancel" else None,
                                  crash=crash)
            if stage in ("lattice-run", "lattice-recover", "lattice-cancel", "lattice-crash"):
                # The bytes Lattice ran: the frozen sent request a retained intent names.
                arguments = step["arguments"]
                transport = load(arguments[arguments.index("--worker-transport-profile") + 1])
                intents = retained_intent_digests(transport["state_directory"])
                sent = [(path, self.digest(path)) for path in sent_requests(transport["output_directory"], request)]
                sent = [(path, digest) for path, digest in sent if digest in intents]
                if len(sent) > 1:
                    raise RuntimeError("more than one frozen request has a retained Lattice intent")
                put(self.output / name / (label + "-request.json"),
                    {"path": str(sent[0][0]), "blake3": sent[0][1], "request": load(sent[0][0])} if sent
                    else {"path": None, "blake3": None, "request": None,
                          "reason": "no retained Lattice intent names a request frozen for this admitted request"})
            if stage in ("lattice-run", "lattice-recover", "lattice-cancel") and step["exit_code"] == 0 and not isinstance(result, dict):
                raise RuntimeError("native transport did not produce a retained observation object")
            if stage == "lattice-cancel" and (result["result"]["disposition"] != "cancelled" or result["accepted"] or result["integrated"]):
                raise RuntimeError("native worker did not establish cancellation without acceptance/integration")
            if stage in ("lattice-prepare", "prepare-verification-profile", "prepare-needs") and step["exit_code"] == 0:
                arguments = step["arguments"]
                path = Path(arguments[arguments.index("--output") + 1]).resolve(strict=True)
                if path.parent != self.input_root:
                    raise ValueError("native prepared input must remain in the owned input directory")
                path.chmod(0o444)
                captures["prepared-" + step["capture"]] = str(path)
                captures["document-" + step["capture"]] = load(path)
            if stage in ("lattice-run", "lattice-recover") and step["exit_code"] == 0 and result["result"].get("receipt") is not None:
                path = self.native_root / f"{name}-{label}.json"
                put(path, result["result"]["receipt"])
                if os.geteuid() == 0:
                    os.chown(path, self.inputs["verifier_uid"], self.inputs["verifier_gid"])
                captures["receipt-" + step["capture"]] = str(path)
                put(self.output / name / label / "native-receipt-projection.json", {
                    "source": "stdout.raw", "pointer": "/result/receipt", "output": str(path), "blake3": self.digest(path)})
            if stage in ("lattice-run", "lattice-recover") and step["exit_code"] == 0 and result.get("unchanged") is not None:
                artifact = Path(result["artifacts"]["unchanged_receipt"]).resolve(strict=True)
                unchanged = load(artifact)
                raw = artifact.read_bytes()
                if unchanged != result["unchanged"] or unchanged["schema"] != "lattice.unchanged-candidate-observation.v1" or unchanged["disposition"] != "unchanged":
                    raise RuntimeError("native unchanged artifact differs from actual proposal observation")
                path = self.native_root / f"{name}-{label}.unchanged.json"
                put(path, raw)
                if os.geteuid() == 0:
                    os.chown(path, self.inputs["verifier_uid"], self.inputs["verifier_gid"])
                captures["receipt-" + step["capture"]] = str(path)
                put(self.output / name / label / "native-unchanged-provenance.json", {"source": str(artifact), "blake3": self.digest(path)})
            if stage == "verify" and step["exit_code"] == 0:
                if result["schema"] != "kiln.implementation-verification.v1" or not result["run"]["success"] or not result["run"]["terminal"]:
                    raise RuntimeError("canonical Engine did not admit successful exact-candidate verification")
                arguments = step["arguments"]
                authority = load(arguments[arguments.index("--authority") + 1])
                report_path = Path(authority["scratch_root"]) / result["handoff"] / "observations.json"
                # Qualification inspects this only as native retained raw output.
                # It is NEVER imported/deserialized into an acceptance authority.
                report = load(report_path)
                if report["outer_binding"] != result["handoff"] or report["handoff"]["candidate"] != result["candidate"] or not report["checks_executed"]:
                    raise RuntimeError("retained native check artifacts do not bind canonical verification")
                if name == "bug":
                    baseline = [row for row in report["observations"] if row["baseline"]]
                    if len(baseline) != 1 or baseline[0]["status"] != "failed" or not baseline[0]["exit_code"] or baseline[0]["exit_code"] < 0:
                        raise RuntimeError("frozen baseline reproducer did not actually fail")
                if name == "already-satisfied" and not report["already_satisfied"]:
                    raise RuntimeError("already-satisfied outcome was not independently checked")
                put(self.output / name / label / "native-check-observations.raw", report_path.read_bytes())
                if name in ("bug", "feature"):
                    source = load(self.inputs["source_baseline"]["path"])["cases"][name]
                    if result["base"]["native_id"] != source["base"] or report["handoff"]["repository"] != source["repository"]:
                        raise RuntimeError(f"{name}: canonical verification does not bind frozen baseline")
            captures[step["capture"]] = result
            for assertion in step.get("observations", []):
                observed = pointer(result, assertion["pointer"])
                if "equals" in assertion and observed != assertion["equals"]:
                    raise RuntimeError(f"{name}/{label}: {assertion['pointer']} differs from expected observation")
                if assertion.get("nonempty") and not observed:
                    raise RuntimeError(f"{name}/{label}: required observation is empty")
            for artifact in step.get("retain", []):
                path = Path(artifact["path"]).resolve(strict=True)
                if not re.fullmatch(r"[A-Za-z0-9_-]{1,128}", artifact["name"]) or path.stat().st_size > MAX_BYTES:
                    raise ValueError("retained artifact has unsafe name or exceeds the bounded receipt size")
                identity = self.digest(path)
                if artifact.get("blake3") and identity != artifact["blake3"]:
                    raise RuntimeError(f"{name}/{label}: retained native artifact differs")
                put(self.output / name / label / (artifact["name"] + ".raw"), path.read_bytes())
            if stage in ("lattice-run", "lattice-recover", "lattice-cancel") and step["exit_code"] == 0:
                retained = result.get("artifacts", {})
                if "omp_observation" not in retained or "raw_events" not in retained:
                    raise Missing(f"{name}: native worker omitted actual OMP artifact locators")
                for field in ("omp_observation", "raw_events"):
                    path = Path(retained[field]).resolve(strict=True)
                    if path.stat().st_size > MAX_BYTES:
                        raise ValueError("native OMP artifact exceeds bounded receipt size")
                    put(self.output / name / label / ("native-" + field + ".raw"), path.read_bytes())
            if stage == "lattice-run" and step["exit_code"] == 0 and name in ("bug", "feature", "already-satisfied"):
                observed = load(retained["omp_observation"])["Ok"]
                if observed["schema"] != "lattice.omp-jsonl-observation.v1" or observed["terminal"] != "completed" or observed["event_count"] <= 0 or observed["raw_events_blake3"] != self.digest(retained["raw_events"]):
                    raise RuntimeError(f"{name}: actual OMP stream is incomplete or differs from native observation")
                self.receipt.setdefault("actual_omp_runs", {})[name] = observed
            if step.get("database"):
                self.ledger(name, step["database"], label)
        self.receipt["scenarios"][name] = {"status": "qualified_local_real_stage_scope", "stages": sorted(stages),
                                            "provider_input": self.inputs["provider"], "raw_outputs": name}
        put(self.output / name / "scenario-receipt.json", self.receipt["scenarios"][name])

    def finish(self):
        put(self.output / "receipt.json", self.receipt)
        for name, receipt in self.receipt["scenarios"].items():
            path = self.output / name / "scenario-receipt.json"
            if not path.exists():
                put(path, receipt)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["prerequisites", "local-admission", "campaign"])
    parser.add_argument("--inputs", type=Path)
    parser.add_argument("--output", type=Path, required=True, help="new owned directory, never an existing run")
    parser.add_argument("--scope", choices=["local", "campaign"], default="local", help="prerequisite scope; never authorizes execution")
    args = parser.parse_args()
    inputs = load(args.inputs) if args.inputs else {"schema": SCHEMA, "programs": {}}
    runner = Runner(inputs, args.output)
    code = 0
    try:
        runner.preflight(live=args.mode == "campaign" or args.scope == "campaign")
        if args.mode == "local-admission":
            runner.local_admission()
            runner.receipt["suite"] = "partial_local_admission_only"
        elif args.mode == "campaign":
            scenarios = inputs["scenarios"]
            names = [scenario["name"] for scenario in scenarios]
            if len(names) != len(set(names)) or set(names) != set(REQUIRED):
                raise Missing("campaign requires each scenario exactly once: " + ", ".join(sorted(REQUIRED)))
            for scenario in scenarios:
                validate_scenario(scenario)
            runner.prepare_campaign_inputs()
            runner.receipt["scenarios"] = {name: {"status": "not_run_due_to_prior_failure_or_missing_native_evidence"} for name in names}
            for scenario in scenarios:
                try:
                    runner.scenario(scenario)
                except (Missing, RuntimeError, ValueError, OSError, KeyError, TypeError, IndexError, subprocess.SubprocessError) as error:
                    runner.receipt["scenarios"][scenario["name"]] = {"status": "blocked_or_failed", "reason": str(error), "workspace_reuse": "forbidden_until_reconciled"}
                    directory = runner.output / scenario["name"]
                    directory.mkdir(mode=0o700, exist_ok=True)
                    put(directory / "scenario-receipt.json", runner.receipt["scenarios"][scenario["name"]])
                    raise
            runner.receipt["suite"] = "qualified_local_real_stage_scope_only"
            runner.receipt["live_worker"] = "actual_adopted_omp_observed_for_bug_and_feature"
        else:
            runner.receipt["suite"] = "prerequisites_present_no_runtime_exercised"
    except (Missing, RuntimeError, ValueError, OSError, KeyError, TypeError, IndexError, subprocess.SubprocessError) as error:
        runner.receipt["error"] = str(error)
        code = 2
    finally:
        runner.finish()
    print(json.dumps({"receipt": str(runner.output / "receipt.json"), "status": runner.receipt["suite"], "error": runner.receipt.get("error")}))
    return code


if __name__ == "__main__":
    sys.exit(main())
