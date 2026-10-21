#!/usr/bin/env python3
"""Qualify Kiln self-CI from a dedicated fork of its own source.

The canonical repository is not used. A fresh fork is created from the exact
committed Kiln clone; the publisher and observer run on the declared second
host. The unchanged broker, controller, worker, Nix adapters and status rails
build the real `packages.x86_64-linux.kiln` product and publish its status.
"""
import argparse
import json
import os
from pathlib import Path
import re
import runpy
import subprocess
import sys
import tempfile
import time

REPO = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO / "scripts"))
STATUS = runpy.run_path(str(REPO / "scripts/radicle-ci-live-distinct-status.py"))
BaseExercise = STATUS["Exercise"]
put, load = (STATUS[name] for name in ("put", "load"))
PEER_TIMEOUT = STATUS["PEER_TIMEOUT"]
encoded = STATUS["encoded"]
SELF_CI_NON_CLAIMS = [
    "fork-based self-CI is not canonical publication, delegate or governance change",
    "the real Kiln build is not production rollout, activation or release authority",
    "the publisher and observer hosts are trusted canaries; no hostile-node isolation is claimed",
]


class Exercise(BaseExercise):
    def __init__(self, args):
        base = args.base_revision
        if base is None:
            base = subprocess.run(["git", "-C", str(REPO), "rev-parse", "HEAD"],
                                  capture_output=True, text=True, check=True).stdout.strip()
        if not re.fullmatch(r"[0-9a-f]{40}", base):
            raise ValueError("self-CI base revision must be an exact 40-hex commit")
        self.base_commit = base
        super().__init__(args)

    # r[impl kiln.self_ci.authority] r[verify kiln.self_ci.authority]
    # r[impl kiln.self_ci.source] r[verify kiln.self_ci.source]
    def profile_path(self):
        return REPO / "config/radicle-ci-live-self-ci.ncl"

    def pilot_policy_path(self):
        return REPO / "config/onixos-ci-pilot-self-ci.ncl"

    def profile_inputs(self, tools):
        inputs = super().profile_inputs(tools)
        inputs["self_ci"] = {
            "base_commit": self.base_commit,
            "root_selector": "packages.x86_64-linux",
            "selected_task": "kiln",
            "product_path": "bin/kiln",
            "failure_marker": "self_ci_genuine_failure",
            "unavailable_settle_seconds": 30,
            "source_limits": {
                "max_identity_bytes": 4096,
                "max_reference_bytes": 512,
                "max_entries": 8192,
                "max_path_bytes": 1024,
                "max_file_bytes": 4 * 1024 * 1024,
                "max_total_bytes": 64 * 1024 * 1024,
            },
        }
        return inputs

    def extra_executables(self):
        host = getattr(self.args, "host", None)
        return {"host": str(Path(host).resolve(strict=True))} if host else {}

    def source_files(self, value):
        source = self.root / "project-source"
        if source.exists():
            return source
        subprocess.run(["git", "clone", "--no-local", str(REPO), str(source)],
                       check=True, capture_output=True)
        subprocess.run(["git", "-C", str(source), "checkout", "-B", "ci-canary", self.base_commit],
                       check=True, capture_output=True)
        (source / "self-ci-bootstrap.txt").write_text("Kiln base " + self.base_commit + "\n")
        return source

    def discovery_root_selector(self):
        return self.settings["self_ci"]["root_selector"] if hasattr(self, "settings") else "packages.x86_64-linux"

    def selected_task(self):
        return self.settings["self_ci"]["selected_task"] if hasattr(self, "settings") else "kiln"

    def prepare_nodes(self):
        initial = super().prepare_nodes()
        self.selection["limits"] = dict(self.settings["self_ci"]["source_limits"])
        return initial

    def seed_candidate_cache(self, bindings):
        source = Path(self.passwd_home) / ".cache/nix"
        if not source.is_dir():
            return
        home = Path(bindings["root"]) / "eval-home"
        target = home / ".cache/nix"
        home.mkdir(mode=0o700, parents=True, exist_ok=True)
        (home / ".cache").mkdir(mode=0o700, exist_ok=True)
        target.mkdir(mode=0o700, exist_ok=True)
        subprocess.run(["cp", "-a", str(source) + "/.", str(target)], check=False, capture_output=True)

    def until(self, description, predicate, *, interval=0.2):
        deadline = time.monotonic() + 1800
        while time.monotonic() < deadline:
            for item in self.services:
                if item["log"].stat().st_size > 33 * 1024 * 1024:
                    raise ValueError("service log exceeds bound: " + item["name"])
            result = predicate()
            if result:
                return result
            time.sleep(interval)
        raise TimeoutError(description)

    # r[impl kiln.self_ci.build] r[verify kiln.self_ci.build]
    def commit_source(self, case):
        source = self.remote.source
        if case == "failure":
            target = source / "crates/kiln-provider-api/src/lib.rs"
            original = self.remote.read_file(target)
            content = original + "\nconst SELF_CI_GENUINE_FAILURE: () = self_ci_genuine_failure\n"
        else:
            target = source / "docs/self-ci-canary.md"
            original = self.remote.read_file(target, missing_ok=True)
            content = original + "\nSelf-CI " + case + " " + self.root.name + "\n"
        with tempfile.NamedTemporaryFile(delete=False) as handle:
            handle.write(content.encode())
            temporary = Path(handle.name)
        try:
            self.remote.upload(temporary, target)
        finally:
            temporary.unlink(missing_ok=True)
        self.jj(source, "commit", "-m", "Self-CI " + case + ": " + self.root.name)
        self.jj(source, "bookmark", "set", self.settings["source_branch"], "-r", "@-")
        revision = self.jj(source, "log", "--no-graph", "-r", "@-", "-T", "commit_id")["stdout"].strip()
        if not re.fullmatch(r"[0-9a-f]{40}", revision):
            raise ValueError("self-CI source commit is not exact")
        return {"provider": "radicle", "repository": self.rid, "revision": revision}

    # r[impl kiln.self_ci.build] r[verify kiln.self_ci.build]
    # r[impl kiln.self_ci.recovery] r[verify kiln.self_ci.recovery]
    def run_cases(self):
        reports = []
        for case in ("success", "failure", "unavailable"):
            self.env["RAD_HOME"] = self.nodes["publisher"]["home"]
            source = self.commit_source(case)
            context = self.root / (case + "-source.json")
            put(context, source)
            self.env["RAD_HOME"] = self.nodes["ci"]["home"]
            bindings = self.ci("bindings", self.root / "controller.json", context)
            self.seed_candidate_cache(bindings)
            if case == "unavailable":
                self.stop(self.host, abrupt=True)
                (self.root / "aspen.sock").unlink(missing_ok=True)
            self.env["RAD_HOME"] = self.nodes["publisher"]["home"]
            self.jj(self.root / "project-source", "git", "push", "--remote", "rad", "--bookmark", self.settings["source_branch"])
            self.env["RAD_HOME"] = self.nodes["ci"]["home"]
            ingress = self.root / "ingress" / source["revision"]
            self.until("automatic real broker ingress", lambda: (ingress / "request.json").is_file() or self.failed(self.broker))
            message = load(ingress / "request.json")
            if message["repository"]["id"] != self.rid or message["after"] != source["revision"]:
                raise ValueError("broker delivered a foreign repository/revision")
            journal_path = Path(bindings["root"]) / "ci-journal.json"
            self.until("canonical native source reservation", lambda: journal_path.is_file() or self.failed(self.broker))
            original = load(journal_path)["ingress"]["operation_id"]
            restart = None
            if case == "success":
                self.until("real independent Nix build intent", lambda: (Path(bindings["root"]) / "build-state.json").is_file() or self.failed(self.broker))
                put(ingress / "interrupt", b"explicit owned controller interruption\n")
                self.until("controller SIGKILL evidence", lambda: (ingress / "interrupted.json").is_file() or self.failed(self.broker))
                self.stop(self.host, abrupt=True)
                pending = self.ci("replay", self.root / "controller.json", context)
                restart = {"canonical_outcome": pending["runs"][bindings["run_id"]]["outcome"],
                           "worker_result_present": (Path(bindings["root"]) / "project-result.json").exists(),
                           "independent_lattice_running": self.lattice["process"].poll() is None,
                           "original_operation": original}
                if restart["canonical_outcome"] != "Pending" or restart["worker_result_present"] or not restart["independent_lattice_running"]:
                    raise ValueError("interruption missed actual independent in-flight execution")
                (self.root / "aspen.sock").unlink(missing_ok=True)
                self.host = self.spawn("host-restarted", self.host_argv)
                self.until("recovered Aspen socket", lambda: (self.root / "aspen.sock").is_socket() or self.failed(self.host))
                put(ingress / "resume", b"reconcile original unchanged broker bytes\n")
            if case != "unavailable":
                self.until("bounded controller completion", lambda: (ingress / "complete.json").is_file() or self.failed(self.broker))
            else:
                time.sleep(self.settings["self_ci"]["unavailable_settle_seconds"])
            replies = [json.loads(line) for path in sorted(ingress.glob("attempt-*-stdout.log")) for line in path.read_text().splitlines()]
            journal = self.ci("reconcile", self.root / "controller.json", context)
            state = self.ci("replay", self.root / "controller.json", context)
            expected = {"success": "Success", "failure": "DomainFailure", "unavailable": "Pending"}[case]
            if state["runs"][bindings["run_id"]]["outcome"] != expected or journal["ingress"]["operation_id"] != original:
                raise ValueError("canonical outcome or original operation differs")
            before_invocations = (self.root / "provider-invocations.log").read_text().splitlines()
            duplicate = self.rad("ci", "sync", "--fetch", "--seed", self.nodes["publisher"]["id"], "--timeout", PEER_TIMEOUT, self.rid)
            duplicate_journal = self.ci("reconcile", self.root / "controller.json", context)
            if duplicate_journal["ingress"]["operation_id"] != original or (self.root / "provider-invocations.log").read_text().splitlines() != before_invocations:
                raise ValueError("duplicate native fetch/reconciliation redispatched a worker")
            product = None
            failure_marker = None
            if case == "unavailable":
                if any(reply["response"] == "finished" for reply in replies) or journal["status_intent"] is not None or duplicate_journal["status_intent"] is not None:
                    raise ValueError("unavailable provider manufactured a final reply or status")
                remote = None
                release = None
                roots = {}
            else:
                if journal["status_observation"]["disposition"] != "written" or not any(reply["response"] == "finished" for reply in replies):
                    raise ValueError("actual terminal source result lacks dedicated status/broker completion")
                remote = self.independent_status(case, source, journal)
                roots = {str(path): str(path.resolve(strict=True)) for path in (self.root / "build-roots").rglob("*") if path.is_symlink()}
                if case == "success":
                    if len(roots) != 1:
                        raise ValueError("successful Kiln build did not produce exactly one rooted product")
                    product = next(iter(roots.values()))
                    executable = Path(product) / self.settings["self_ci"]["product_path"]
                    if not executable.is_file() or not os.access(executable, os.X_OK):
                        raise ValueError("rooted product does not contain the expected Kiln executable")
                else:
                    marker = self.settings["self_ci"]["failure_marker"].encode()
                    diagnostics = b"".join(path.read_bytes() for path in Path(bindings["root"]).glob("*.log"))
                    if marker not in diagnostics:
                        raise ValueError("genuine self-CI Rust failure diagnostic absent")
                    if roots:
                        raise ValueError("failed Kiln build unexpectedly rooted an output")
                    failure_marker = marker.decode()
                release = [self.json_command([self.exe["worker"], "project-release", self.root / "project.json", self.project_identity, context]) for _ in range(2)]
                if any(path.is_symlink() for path in (self.root / "build-roots").rglob("*")) or (Path(bindings["root"]) / "source-root").is_symlink():
                    raise ValueError("source/build roots survived explicit idempotent release")
            put(self.root / (case + "-journal.json"), journal)
            reports.append({"case": case, "source": source, "operation_id": original, "canonical_outcome": expected,
                "original_broker_replies": replies, "restart": restart, "remote_status": remote, "rooted_outputs": roots,
                "product": product, "failure_marker": failure_marker, "release": release,
                "native_duplicate_fetch": duplicate, "duplicate_preserved_operation": duplicate_journal["ingress"]["operation_id"]})
            self.after_case(case, reports[-1])
        return reports

    # r[impl kiln.self_ci.evidence] r[verify kiln.self_ci.evidence]
    # r[impl kiln.self_ci.claims] r[verify kiln.self_ci.claims]
    def execute(self):
        super().execute()
        report = load(self.root / "verification.json")
        report["self_ci"] = {
            "schema": "kiln.self-ci.v1",
            "base_commit": self.base_commit,
            "fork_repository": self.rid,
            "root_selector": self.settings["self_ci"]["root_selector"],
            "selected_task": self.settings["self_ci"]["selected_task"],
            "product_path": self.settings["self_ci"]["product_path"],
            "failure_marker": self.settings["self_ci"]["failure_marker"],
            "source_limits": self.settings["self_ci"]["source_limits"],
            "source_placement": "publisher and observer remote; CI, broker, controller and worker local",
            "canonical_repository": "untouched",
        }
        report["non_claims"] = sorted(set(report["non_claims"] + SELF_CI_NON_CLAIMS))
        (self.root / "verification.json").write_bytes(encoded(report))
        cohort = load(self.root / "cohort.json")
        cohort["self_ci"] = report["self_ci"]
        (self.root / "cohort.json").write_bytes(encoded(cohort))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("inputs", "nickel", "root", "worker", "extension", "coordinator", "radicle",
                 "radicle-node", "broker", "broker-tool", "jj", "git", "secretspec", "nixpkgs",
                 "remote-host", "remote-root", "remote-address", "local-address"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--remote-port", required=True, type=int)
    parser.add_argument("--remote-readiness-seconds", required=True, type=int)
    parser.add_argument("--host")
    parser.add_argument("--base-revision")
    args = parser.parse_args()
    os.umask(0o077)
    exercise = Exercise(args)
    try:
        exercise.execute()
    finally:
        exercise.close()


if __name__ == "__main__":
    main()
