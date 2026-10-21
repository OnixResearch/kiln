#!/usr/bin/env python3
"""Qualify distinct-host Radicle status dissemination with local authority.

The publisher and observer nodes run on the declared second host. The local CI
node, controller, status creation, signing and checkpoint synchronization stay
local. The remote observer fetches the status over the tailnet and reduces it in
its own storage with the pinned read-only coordinator command.
"""
import argparse
import ipaddress
import json
import os
from pathlib import Path
import runpy
import sys
import time

REPO = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO / "scripts"))
DISTINCT = runpy.run_path(str(REPO / "scripts/radicle-ci-live-distinct-host.py"))
BaseExercise = DISTINCT["Exercise"]
put, load = (DISTINCT[name] for name in ("put", "load"))
PEER_TIMEOUT = DISTINCT["PEER_TIMEOUT"]
encoded = DISTINCT["LIVE"]["PILOT"]["encoded"]
# r[impl kiln.radicle_ci.distinct_status.claims] r[verify kiln.radicle_ci.distinct_status.claims]
STATUS_NON_CLAIMS = [
    "status object creation and signing remained local; dissemination and readback were distinct-host",
    "the remote observer is a trusted canary identity; no hostile-node isolation is claimed",
]


class Exercise(BaseExercise):
    def __init__(self, args):
        local = ipaddress.ip_address(args.local_address)
        if local not in ipaddress.ip_network("100.64.0.0/10"):
            raise ValueError("local CI address must be a tailnet 100.64/10 address")
        self.observer_remote_ready = False
        self.remote_observer_pid = None
        self.status_transport = {}
        super().__init__(args)

    # r[impl kiln.radicle_ci.distinct_status.authority] r[verify kiln.radicle_ci.distinct_status.authority]
    def profile_path(self):
        return REPO / "config/radicle-ci-live-distinct-status.ncl"

    def profile_inputs(self, tools):
        inputs = super().profile_inputs(tools)
        inputs["local_address"] = self.args.local_address
        return inputs

    # r[impl kiln.radicle_ci.distinct_status.transport] r[verify kiln.radicle_ci.distinct_status.transport]
    def prepare_nodes(self):
        initial = super().prepare_nodes()
        remote = self.settings["remote"]
        ci = self.nodes["ci"]
        self.stop(ci["service"])
        control = Path(ci["home"]) / "node/control.sock"
        deadline = time.monotonic() + 10
        while control.exists() and time.monotonic() < deadline:
            time.sleep(0.1)
        if control.exists():
            control.unlink()
        home = Path(ci["home"])
        config = load(home / "config.json")
        config["node"]["listen"] = [f"0.0.0.0:{ci['port']}"]
        config["node"]["externalAddresses"] = [f"{self.args.local_address}:{ci['port']}"]
        (home / "config.json").write_bytes(encoded(config))
        ci["service"] = self.spawn("node-ci-tailnet", [self.exe["radicle_node"], "--force"])
        self.until("local CI tailnet node", lambda: control.is_socket() or self.failed(ci["service"]))
        def control_ready():
            self.failed(ci["service"])
            previous = self.env.get("RAD_HOME")
            self.env["RAD_HOME"] = ci["home"]
            try:
                record = self.run([self.exe["radicle"], "node", "status"], allowed=(0, 1))
            finally:
                if previous is None:
                    self.env.pop("RAD_HOME", None)
                else:
                    self.env["RAD_HOME"] = previous
            return record["exit_code"] == 0
        self.until("local CI tailnet control socket", control_ready)
        self.rad("ci", "node", "connect", self.nodes["publisher"]["id"] + "@" + remote["address"] + ":" + str(self.remote_port))
        self.rad("ci", "seed", "--scope", "all", "--from", self.nodes["publisher"]["id"], "--timeout", PEER_TIMEOUT, self.rid)
        observer = self.nodes["observer"]
        self.stop(observer["service"])
        observer_home = Path(observer["home"])
        observer_port = self.remote.free_port(int(observer["port"]))
        observer_config = load(observer_home / "config.json")
        observer_config["node"]["listen"] = [f"127.0.0.1:{observer_port}"]
        observer_config["node"]["externalAddresses"] = []
        (observer_home / "config.json").write_bytes(encoded(observer_config))
        self.remote.observer = self.remote.root / "rad-observer"
        self.remote.observer_log = self.remote.root / "remote-observer.log"
        self.remote.copy_tree(observer_home, self.remote.observer, exclude_node=True)
        for key in (observer_home / "keys").rglob("*"):
            if key.is_file():
                key.unlink()
        self.remote_observer_pid = self.remote.spawn_at(self.remote.observer, self.remote.observer_log)
        self.remote.wait_at(self.remote.observer, self.remote_observer_pid)
        self.remote.rad_at(self.remote.observer, "node", "connect",
                           ci["id"] + "@" + self.args.local_address + ":" + str(ci["port"]))
        self.remote.rad_at(self.remote.observer, "seed", "--scope", "all", "--from", ci["id"],
                           "--timeout", PEER_TIMEOUT, self.rid)
        self.observer_remote_ready = True
        self.status_transport = {
            "ci_listen": f"0.0.0.0:{ci['port']}",
            "ci_external": f"{self.args.local_address}:{ci['port']}",
            "observer_home": str(self.remote.observer),
            "observer_listen": f"127.0.0.1:{observer_port}",
            "status_authority": "local CI node",
        }
        return initial

    def rad(self, role, *arguments):
        if getattr(self, "observer_remote_ready", False) and role == "observer":
            return self.remote.rad_at(self.remote.observer, *arguments)
        return super().rad(role, *arguments)

    # r[impl kiln.radicle_ci.distinct_status.readback] r[verify kiln.radicle_ci.distinct_status.readback]
    def independent_status(self, case, source, journal):
        written = journal["status_observation"]["observation"]
        plan = self.root / (case + "-status-plan.json")
        put(plan, journal["status_intent"])
        profile = self.root / "status-profile.json"
        remote_profile = self.remote.root / "status-profile.json"
        remote_plan = self.remote.root / plan.name
        self.remote.upload(profile, remote_profile)
        self.remote.upload(plan, remote_plan)
        storage = self.remote.observer / "storage"
        absent = self.remote.reconcile(remote_profile, storage, remote_plan)
        if absent is not None:
            raise ValueError("unannounced dedicated status was already visible in remote observer storage")
        self.jj(self.status_checkout, "git", "fetch", "--remote", "origin")
        self.jj(self.status_checkout, "git", "fetch", "--remote", "publisher")
        self.jj(self.status_checkout, "bookmark", "set", self.settings["status_checkpoint_branch"], "-r", source["revision"])
        self.jj(self.status_checkout, "git", "push", "--remote", "origin", "--bookmark", self.settings["status_checkpoint_branch"])
        self.rad("observer", "sync", "--fetch", "--seed", self.nodes["ci"]["id"], "--timeout", PEER_TIMEOUT, self.rid)
        remote = self.remote.reconcile(remote_profile, storage, remote_plan)
        if remote != written:
            raise ValueError("remote observer status reduction differs from the exact local object/head")
        put(self.root / (case + "-remote-status.json"), {
            "before_sign_sync": absent,
            "after_native_fetch": remote,
            "observer_host": self.settings["remote"]["host"],
            "observer_storage": str(storage),
            "readback": "pinned status-reconcile on remote observer storage",
        })
        return remote

    # r[impl kiln.radicle_ci.distinct_status.evidence] r[verify kiln.radicle_ci.distinct_status.evidence]
    def execute(self):
        super().execute()
        report = load(self.root / "verification.json")
        report["distinct_host"]["status_boundary"] = (
            "observer node and storage remote; dissemination and readback distinct-host; "
            "status creation and signing local"
        )
        report["distinct_status"] = {
            "schema": "kiln.radicle-ci.distinct-status.v1",
            "observer_host": self.settings["remote"]["host"],
            "observer_home": str(self.remote.observer),
            "observer_storage": str(self.remote.observer / "storage"),
            "observer_id": self.nodes["observer"]["id"],
            "local_ci_address": f"{self.args.local_address}:{self.nodes['ci']['port']}",
            "transport": "remote observer outbound direct tailnet TCP to the local CI node",
            "readback": "pinned status-reconcile on remote observer storage",
            "authority": "status object creation, signing and checkpoint synchronization local",
            "transport_evidence": self.status_transport,
        }
        non_claims = [claim for claim in report["non_claims"]
                      if "status dissemination remained same-host" not in claim]
        report["non_claims"] = sorted(set(non_claims + STATUS_NON_CLAIMS))
        (self.root / "verification.json").write_bytes(encoded(report))
        cohort = load(self.root / "cohort.json")
        cohort["distinct_status"] = report["distinct_status"]
        cohort["topology"]["observer"] = {
            **cohort["topology"]["observer"],
            "host": self.settings["remote"]["host"],
            "placement": "remote",
            "storage": str(self.remote.observer / "storage"),
        }
        (self.root / "cohort.json").write_bytes(encoded(cohort))
        (self.root / "remote-observer.log").write_bytes(self.remote.read_file(self.remote.observer_log).encode())

    # r[impl kiln.radicle_ci.distinct_status.evidence] r[verify kiln.radicle_ci.distinct_status.evidence]
    def close(self):
        errors = []
        try:
            if getattr(self, "remote", None) is not None and getattr(self, "remote_observer_pid", None):
                try:
                    self.remote.stop_node(self.remote_observer_pid)
                except Exception as error:  # noqa: BLE001
                    errors.append(f"observer stop: {error}")
                try:
                    (self.root / "remote-observer.log").write_bytes(
                        self.remote.read_file(self.remote.observer_log).encode())
                    (self.root / "remote-observer-config.json").write_bytes(
                        self.remote.read_file(self.remote.observer / "config.json").encode())
                    (self.root / "remote-observer-retirement.json").write_text(json.dumps({
                        "schema": "kiln.radicle-ci.distinct-status-retirement.v1",
                        "host": self.settings["remote"]["host"],
                        "root": str(self.remote.root),
                        "observer_home": str(self.remote.observer),
                        "observer_stopped": not self.remote.node_alive(self.remote_observer_pid),
                        "errors": errors,
                    }, indent=2, sort_keys=True) + "\n")
                except Exception as error:  # noqa: BLE001
                    errors.append(f"observer evidence: {error}")
        finally:
            super().close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("inputs", "nickel", "root", "worker", "extension", "coordinator", "radicle",
                 "radicle-node", "broker", "broker-tool", "jj", "git", "secretspec", "nixpkgs",
                 "remote-host", "remote-root", "remote-address", "local-address"):
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
