#!/usr/bin/env python3
"""Qualify a distinct-host Radicle publisher against the local CI authority.

Only the publisher node and its source worktree move to the declared SSH host.
The CI node, broker, controller, project worker, Aspen host, Lattice and observer
stay local. SSH provisions files and owned processes; Radicle TCP carries the
source delivery. Status dissemination remains the separate same-host observer
boundary.
"""
import argparse
import ipaddress
import json
import os
from pathlib import Path
import re
import runpy
import shlex
import subprocess
import sys
import tempfile
import time

REPO = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO / "scripts"))
LIVE = runpy.run_path(str(REPO / "scripts/radicle-ci-live.py"))
BaseExercise = LIVE["Exercise"]
put, load = (LIVE[name] for name in ("put", "load"))
PEER_TIMEOUT = LIVE["PEER_TIMEOUT"]
# r[impl kiln.radicle_ci.distinct_host.claims] r[verify kiln.radicle_ci.distinct_host.claims]
DISTINCT_HOST_NON_CLAIMS = [
    "two owned hosts on a private tailnet are not WAN or hostile-network qualification",
    "the remote publisher host is trusted and is not a hostile-worker boundary",
    "status dissemination remained same-host in this qualification",
    "distinct-host source delivery is not production deployment, activation or release authority",
]


def quote(value):
    return shlex.quote(str(value))


class RemoteError(RuntimeError):
    pass


class RemoteHost:
    """Bounded SSH orchestration for the single declared canary publisher."""

    def __init__(self, exercise, destination, root, address, readiness):
        self.exercise = exercise
        self.destination = destination
        self.root = Path(root)
        self.address = address
        self.readiness = readiness
        self.home = self.root / "rad-publisher"
        self.source = self.root / "project-source"
        self.log = self.root / "remote-publisher.log"
        self.tmp = self.root / "tmp"
        self.commands = []
        self.pid = None
        self.node_path = exercise.exe["radicle_node"]
        self.rad_path = exercise.exe["radicle"]
        self.jj_path = exercise.exe["jj"]
        self.git_path = exercise.exe["git"]
        self.path_env = ":".join(dict.fromkeys(
            str(Path(path).parent) for path in (self.rad_path, self.jj_path, self.git_path)
        )) + ":/run/current-system/sw/bin"
        self.ssh = ["ssh", "-o", "BatchMode=yes", self.destination]

    def _script(self, lines):
        return "set -eu\n" + "\n".join(lines) + "\n"

    def _record(self, script, process):
        self.commands.append({
            "script": script[-4000:],
            "exit_code": process.returncode,
            "stdout": (process.stdout or "")[-4000:],
            "stderr": (process.stderr or "")[-4000:],
        })

    def run_script(self, lines, timeout=120, check=True):
        script = self._script(lines)
        process = subprocess.run(
            [*self.ssh, "bash", "-s"], input=script, text=True,
            capture_output=True, timeout=timeout,
        )
        self._record(script, process)
        if check and process.returncode != 0:
            raise RemoteError(
                f"remote command failed ({process.returncode}): {process.stderr.strip()[-1000:]}"
            )
        return self.commands[-1]

    def upload(self, local, remote):
        data = Path(local).read_bytes()
        process = subprocess.run(
            [*self.ssh, "cat > " + quote(remote)], input=data, capture_output=True, timeout=120,
        )
        self.commands.append({
            "script": f"upload {local} -> {remote} ({len(data)} bytes)",
            "exit_code": process.returncode,
            "stdout": "",
            "stderr": process.stderr.decode(errors="replace")[-1000:],
        })
        if process.returncode != 0:
            raise RemoteError("remote file upload failed")
        self.run_script([f"chmod 600 {quote(remote)}"], check=True)

    def read_file(self, remote, limit=2_000_000):
        record = self.run_script([f"head -c {limit} {quote(remote)}"], check=False)
        return record["stdout"]

    def check_root(self):
        root = quote(self.root)
        self.run_script([
            f'case {root} in /var/tmp/[!/]*) ;; *) echo bad-root >&2; exit 64;; esac',
            f'test "$(dirname {root})" = /var/tmp',
            f'test ! -e {root}',
        ])

    def create_root(self):
        root = quote(self.root)
        self.run_script([
            f'test ! -e {root}',
            f'mkdir -m 700 {root}',
            f'test "$(stat -c "%u %a" {root})" = "$(id -u) 700"',
            f'mkdir -m 700 {quote(self.tmp)}',
        ])

    def free_port(self, preferred):
        lines = [f"for p in $(seq {int(preferred)} {int(preferred) + 100}); do",
                 '  if ! ss -H -ltn | grep -qF ":$p "; then echo "$p"; exit 0; fi',
                 "done", "exit 1"]
        record = self.run_script(lines)
        return int(record["stdout"].strip().splitlines()[-1])

    def copy_tree(self, local, remote, exclude_node=False):
        local = Path(local)
        remote = Path(remote)
        with tempfile.NamedTemporaryFile(delete=False) as handle:
            archive = Path(handle.name)
            handle.close()
        try:
            command = ["tar", "-C", str(local.parent)]
            if exclude_node:
                command += ["--exclude=node"]
            command += ["-cf", str(archive), local.name]
            subprocess.run(command, check=True, capture_output=True)
            remote_archive = self.root / "transfer.tar"
            self.upload(archive, remote_archive)
            self.run_script([
                f"rm -rf {quote(remote)}",
                f"mkdir -p {quote(remote.parent)}",
                f"tar -C {quote(remote.parent)} -xf {quote(remote_archive)}",
                f"rm -f {quote(remote_archive)}",
            ])
        finally:
            archive.unlink(missing_ok=True)

    def spawn_node(self):
        script = [
            f"cd {quote(self.home)}",
            f"env -i HOME={quote(self.home)} RAD_HOME={quote(self.home)} TMPDIR={quote(self.tmp)} "
            f"PATH={quote(self.path_env)} nohup {quote(self.node_path)} --force "
            f"> {quote(self.log)} 2>&1 < /dev/null &",
            "echo $!",
        ]
        record = self.run_script(script, timeout=30)
        self.pid = int(record["stdout"].strip().splitlines()[-1])
        return self.pid

    def node_alive(self, pid):
        record = self.run_script([f"kill -0 {int(pid)} 2>/dev/null"], check=False)
        return record["exit_code"] == 0

    def wait_node(self, pid):
        deadline = time.monotonic() + self.readiness
        while time.monotonic() < deadline:
            ready = self.run_script([
                f"test -S {quote(self.home / 'node/control.sock')}",
                f"kill -0 {int(pid)}",
            ], check=False)
            if ready["exit_code"] == 0:
                return
            time.sleep(0.5)
        log = self.read_file(self.log, limit=20000)
        raise RemoteError(f"remote publisher did not become ready: {log[-2000:]}")

    def stop_node(self, pid):
        if pid is None:
            return
        self.run_script([
            f"if kill -0 {int(pid)} 2>/dev/null; then kill -TERM {int(pid)}; fi",
            "for i in $(seq 1 100); do",
            f"  if ! kill -0 {int(pid)} 2>/dev/null; then exit 0; fi",
            "  sleep 0.1",
            "done",
            f"kill -KILL {int(pid)} 2>/dev/null || true",
            "for i in $(seq 1 50); do",
            f"  if ! kill -0 {int(pid)} 2>/dev/null; then exit 0; fi",
            "  sleep 0.1",
            "done",
            "exit 1",
        ], check=False)

    def remove_root(self):
        root = quote(self.root)
        self.run_script([
            f'case {root} in /var/tmp/[!/]*) ;; *) echo bad-root >&2; exit 64;; esac',
            f"rm -rf -- {root}",
            f"test ! -e {root}",
        ])

    def jj(self, directory, *arguments):
        args = ["--config", 'user.name="Kiln Canary"', "--config", 'user.email="canary@kiln.invalid"',
                "--config", 'signing.behavior="drop"', "--config", "git.subprocess=true",
                "--config", "git.executable=" + json.dumps(self.git_path),
                "-R", str(directory), *map(str, arguments)]
        return self.run_script([
            f"cd {quote(directory)}",
            f"env -i HOME={quote(self.home)} RAD_HOME={quote(self.home)} TMPDIR={quote(self.tmp)} "
            f"PATH={quote(self.path_env)} {quote(self.jj_path)} " + " ".join(quote(arg) for arg in args),
        ])

    def rad(self, *arguments):
        return self.run_script([
            f"env -i HOME={quote(self.home)} RAD_HOME={quote(self.home)} TMPDIR={quote(self.tmp)} "
            f"PATH={quote(self.path_env)} {quote(self.rad_path)} " + " ".join(quote(arg) for arg in arguments),
        ])

    def listen_evidence(self, port):
        return self.run_script([f"ss -H -ltn | grep -F {quote(':' + str(port) + ' ')} || true"], check=False)["stdout"].strip()


class Exercise(BaseExercise):
    def __init__(self, args):
        local_root = Path(args.root).absolute()
        remote_root = Path(args.remote_root).absolute()
        if local_root != remote_root:
            raise ValueError("distinct-host canary uses the same /var/tmp path on both hosts")
        address = ipaddress.ip_address(args.remote_address)
        if address not in ipaddress.ip_network("100.64.0.0/10"):
            raise ValueError("remote address must be a tailnet 100.64/10 address")
        self.remote = None
        self.remote_ready = False
        self.remote_created = False
        self.remote_publisher_pid = None
        self.transport_evidence = {}
        super().__init__(args)

    # r[impl kiln.radicle_ci.distinct_host.authority] r[verify kiln.radicle_ci.distinct_host.authority]
    def profile_path(self):
        return REPO / "config/radicle-ci-live-distinct-host.ncl"

    def profile_inputs(self, tools):
        inputs = super().profile_inputs(tools)
        inputs["remote"] = {
            "host": self.args.remote_host,
            "root": str(Path(self.args.remote_root).absolute()),
            "address": self.args.remote_address,
            "port": self.args.remote_port,
            "readiness_seconds": self.args.remote_readiness_seconds,
        }
        return inputs

    # r[impl kiln.radicle_ci.distinct_host.transport] r[verify kiln.radicle_ci.distinct_host.transport]
    def prepare_nodes(self):
        remote = self.settings["remote"]
        if remote["root"] != str(self.root):
            raise ValueError("admitted remote root differs from the exercised path")
        self.remote = RemoteHost(self, remote["host"], remote["root"], remote["address"], remote["readiness_seconds"])
        self.remote.check_root()
        initial = super().prepare_nodes()
        self.remote.create_root()
        self.remote_created = True
        port = self.remote.free_port(int(remote["port"]))
        self.remote_port = port
        publisher = self.nodes["publisher"]
        self.stop(publisher["service"])
        home = Path(publisher["home"])
        config = load(home / "config.json")
        config["node"]["listen"] = [f"0.0.0.0:{port}"]
        config["node"]["externalAddresses"] = [f"{remote['address']}:{port}"]
        (home / "config.json").write_bytes(LIVE["PILOT"]["encoded"](config))
        self.remote.copy_tree(home, self.remote.home, exclude_node=True)
        for key in (home / "keys").rglob("*"):
            if key.is_file():
                key.unlink()
        self.remote_publisher_pid = self.remote.spawn_node()
        self.remote.wait_node(self.remote_publisher_pid)
        self.remote.rad("seed", "--scope", "all", self.rid)
        ci = self.nodes["ci"]
        self.stop(ci["service"])
        ci["service"] = self.spawn("node-ci-restarted", [self.exe["radicle_node"], "--force"])
        self.until("restarted local CI node", lambda: (Path(ci["home"]) / "node/control.sock").is_socket() or self.failed(ci["service"]))
        self.rad("observer", "node", "connect", ci["id"] + "@127.0.0.1:" + str(ci["port"]))
        self.rad("observer", "seed", "--scope", "all", "--from", ci["id"], "--timeout", PEER_TIMEOUT, self.rid)
        self.rad("ci", "node", "connect", self.nodes["publisher"]["id"] + "@" + remote["address"] + ":" + str(port))
        self.rad("ci", "seed", "--scope", "all", "--from", self.nodes["publisher"]["id"], "--timeout", PEER_TIMEOUT, self.rid)
        repository = Path(ci["home"]) / "storage" / self.rid.removeprefix("rad:")
        if not repository.is_dir():
            raise ValueError("local CI node did not retain the canary repository after restart")
        self.env["RAD_HOME"] = ci["home"]
        self.remote.copy_tree(self.root / "project-source", self.remote.source)
        self.remote_ready = True
        listen = self.remote.listen_evidence(port)
        if str(port) not in listen:
            raise ValueError("remote publisher did not bind the declared port")
        self.transport_evidence = {
            "remote_listen": listen,
            "remote_address": remote["address"],
            "remote_port": port,
            "local_ci_address": "127.0.0.1:" + str(ci["port"]),
            "source_placement": "publisher node and source worktree remote; CI/broker/controller/worker/observer local",
        }
        return initial

    # r[impl kiln.radicle_ci.distinct_host.transport] r[verify kiln.radicle_ci.distinct_host.transport]
    def jj(self, directory, *arguments):
        if self.remote_ready and Path(directory) == self.root / "project-source":
            return self.remote.jj(Path(directory), *arguments)
        return super().jj(directory, *arguments)

    def rad(self, role, *arguments):
        if self.remote_ready and role == "publisher":
            return self.remote.rad(*arguments)
        return super().rad(role, *arguments)

    # r[impl kiln.radicle_ci.distinct_host.transport] r[verify kiln.radicle_ci.distinct_host.transport]
    def commit_source(self, value):
        source = self.source_files(value)
        self.remote.upload(source / "value.txt", self.remote.source / "value.txt")
        self.jj(self.remote.source, "commit", "-m", "Dedicated trusted canary: " + value)
        self.jj(self.remote.source, "bookmark", "set", self.settings["source_branch"], "-r", "@-")
        revision = self.jj(self.remote.source, "log", "--no-graph", "-r", "@-", "-T", "commit_id")["stdout"].strip()
        if not re.fullmatch(r"[0-9a-f]{40}", revision):
            raise ValueError("remote source commit is not exact")
        return {"provider": "radicle", "repository": self.rid, "revision": revision}

    # r[impl kiln.radicle_ci.distinct_host.recovery] r[verify kiln.radicle_ci.distinct_host.recovery]
    def after_case(self, case, report):
        if case != "success":
            return
        stopped = self.remote_publisher_pid
        self.remote.stop_node(stopped)
        socket_absent = not self.remote.run_script(
            [f"test -S {quote(self.remote.home / 'node/control.sock')}"], check=False
        )["exit_code"] == 0
        self.remote_publisher_pid = self.remote.spawn_node()
        self.remote.wait_node(self.remote_publisher_pid)
        self.remote.rad("seed", "--scope", "all", self.rid)
        remote = self.settings["remote"]
        self.rad("ci", "node", "connect", self.nodes["publisher"]["id"] + "@" + remote["address"] + ":" + str(self.remote_port))
        self.rad("ci", "seed", "--scope", "all", "--from", self.nodes["publisher"]["id"], "--timeout", PEER_TIMEOUT, self.rid)
        context = self.root / (case + "-source.json")
        journal = self.ci("reconcile", self.root / "controller.json", context)
        state = self.ci("replay", self.root / "controller.json", context)
        runs = state.get("runs") or {}
        if len(runs) != 1:
            raise ValueError("remote restart changed the canonical run set")
        run = next(iter(runs.values()))
        if (journal["ingress"]["operation_id"] != report["operation_id"]
                or run["outcome"] != report["canonical_outcome"]):
            raise ValueError("remote restart did not preserve the original operation/outcome")
        report["remote_restart"] = {
            "stopped_pid": stopped,
            "restarted_pid": self.remote_publisher_pid,
            "socket_absent_after_stop": socket_absent,
            "operation_preserved": True,
            "canonical_outcome": run["outcome"],
        }
        self.transport_evidence["local_peer_after_delivery"] = subprocess.run(
            ["ss", "-H", "-tn"], capture_output=True, text=True
        ).stdout.count(remote["address"] + ":" + str(self.remote_port))

    # r[impl kiln.radicle_ci.distinct_host.status] r[verify kiln.radicle_ci.distinct_host.status]
    # r[impl kiln.radicle_ci.distinct_host.evidence] r[verify kiln.radicle_ci.distinct_host.evidence]
    def execute(self):
        super().execute()
        report = load(self.root / "verification.json")
        report["distinct_host"] = {
            "schema": "kiln.radicle-ci.distinct-host.v1",
            "remote_host": self.settings["remote"]["host"],
            "remote_address": self.settings["remote"]["address"],
            "remote_port": self.remote_port,
            "shared_path_on_distinct_filesystems": str(self.root),
            "publisher_id": self.nodes["publisher"]["id"],
            "ci_id": self.nodes["ci"]["id"],
            "observer_id": self.nodes["observer"]["id"],
            "transport": "direct Radicle TCP over the private tailnet; SSH orchestration only",
            "status_boundary": "same-host independent observer; not distinct-host status dissemination",
            "transport_evidence": self.transport_evidence,
            "remote_commands": self.remote.commands[-100:],
        }
        report["non_claims"] = sorted(set(report["non_claims"] + DISTINCT_HOST_NON_CLAIMS))
        (self.root / "verification.json").write_bytes(LIVE["PILOT"]["encoded"](report))
        cohort = load(self.root / "cohort.json")
        cohort["distinct_host"] = report["distinct_host"]
        cohort["topology"]["publisher"] = {
            **cohort["topology"]["publisher"],
            "host": self.settings["remote"]["host"],
            "address": self.settings["remote"]["address"],
            "port": self.remote_port,
            "placement": "remote",
        }
        cohort["settings"]["remote"] = self.settings["remote"]
        (self.root / "cohort.json").write_bytes(LIVE["PILOT"]["encoded"](cohort))
        (self.root / "remote-publisher.log").write_bytes(self.remote.read_file(self.remote.log).encode())
        print(json.dumps({"repository": self.rid, "remote_host": report["distinct_host"]["remote_host"],
                          "remote_address": report["distinct_host"]["remote_address"],
                          "remote_port": self.remote_port}, indent=2))

    # r[impl kiln.radicle_ci.distinct_host.evidence] r[verify kiln.radicle_ci.distinct_host.evidence]
    def close(self):
        errors = []
        try:
            if self.remote is not None:
                try:
                    self.remote.stop_node(self.remote_publisher_pid)
                except Exception as error:  # noqa: BLE001
                    errors.append(f"stop: {error}")
                try:
                    (self.root / "remote-publisher.log").write_bytes(
                        self.remote.read_file(self.remote.log).encode())
                    (self.root / "remote-publisher-config.json").write_bytes(
                        self.remote.read_file(self.remote.home / "config.json").encode())
                    (self.root / "remote-commands.json").write_bytes(
                        json.dumps(self.remote.commands[-200:], indent=2, sort_keys=True).encode() + b"\n")
                except Exception as error:  # noqa: BLE001
                    errors.append(f"evidence: {error}")
                removed = False
                try:
                    if self.remote_created:
                        self.remote.remove_root()
                        removed = self.remote.run_script(
                            [f"test ! -e {quote(self.remote.root)}"], check=False
                        )["exit_code"] == 0
                except Exception as error:  # noqa: BLE001
                    errors.append(f"remove: {error}")
                try:
                    node_stopped = (self.remote_publisher_pid is None
                                    or not self.remote.node_alive(self.remote_publisher_pid))
                except Exception as error:  # noqa: BLE001
                    errors.append(f"liveness: {error}")
                    node_stopped = False
                put(self.root / "remote-retirement.json", {
                    "schema": "kiln.radicle-ci.distinct-host-retirement.v1",
                    "host": self.settings["remote"]["host"],
                    "root": str(self.remote.root),
                    "node_stopped": node_stopped,
                    "root_removed": removed,
                    "errors": errors,
                })
        finally:
            super().close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("inputs", "nickel", "root", "worker", "extension", "coordinator", "radicle",
                 "radicle-node", "broker", "broker-tool", "jj", "git", "secretspec", "nixpkgs",
                 "remote-host", "remote-root", "remote-address"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--remote-port", required=True, type=int)
    parser.add_argument("--remote-readiness-seconds", required=True, type=int)
    args = parser.parse_args()
    os.umask(0o077)
    exercise = Exercise(args)
    try:
        exercise.execute()
    finally:
        exercise.close()


if __name__ == "__main__":
    main()
