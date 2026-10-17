#!/usr/bin/env python3
"""Prepare and run only the isolated, operator-authorized live CI worker.

Inputs are reviewed immutable executables, exact job bytes, native identities,
and an enrolled controller key. Secrets travel over protected stdin, never argv.
The independently managed bridge can restart without touching the Lattice store.
"""
import argparse
import json
from pathlib import Path
import re
import shlex
import subprocess
import time

ROOT = "/var/tmp/kiln-live-ci-20261018/worker"
USER = "kiln-live-build"
UNITS = {"lattice": "kiln-live-worker-lattice", "iroh": "kiln-live-worker-iroh"}
REPO = Path(__file__).resolve().parent.parent
READINESS_ATTEMPTS = 30
READINESS_POLL_SECONDS = 1
MAXIMUM_ACTIVE_EXECUTIONS = 4


def run(argv, **kwargs):
    return subprocess.run(argv, check=True, **kwargs)


def quote(value):
    return shlex.quote(str(value))


def remote(host, command, data=None):
    return run(["ssh", "-o", "BatchMode=yes", host, command], input=data,
               stdout=subprocess.PIPE).stdout


def put(host, name, content):
    path = f"{ROOT}/{name}"
    remote(host, f"umask 077; cat > {quote(path)}; chown {USER}:{USER} {quote(path)}",
           content if isinstance(content, bytes) else content.encode())


def worker(host, argv):
    return remote(host, "umask 077; " + " ".join(map(quote, ["runuser", "-u", USER, "--", *argv])))


def settings(host):
    return json.loads(remote(host, f"cat {ROOT}/deployment.json"))


def save(host, value):
    put(host, "deployment.json", json.dumps(value, indent=2) + "\n")


def immutable(path):
    resolved = str(Path(path).resolve(strict=True))
    if not re.fullmatch(r"/nix/store/[a-z0-9]{32}-[^/]+(?:/[^\x00]+)?", resolved):
        raise ValueError("Expected an existing immutable Nix store path")
    return resolved


def bootstrap(args):
    package, lattice, nickel, producer, secret, shell = map(immutable, (
        args.package, args.lattice, args.nickel, args.producer_source, args.secretspec, args.shell))
    # Reuse only this exact, previously provisioned service identity and private root.
    remote(args.host, f"set -eu; "
           f"if ! getent passwd {USER} >/dev/null; then "
           f"test ! -e {ROOT}; useradd --system --user-group --home-dir {ROOT} "
           f"--shell /run/current-system/sw/bin/nologin {USER}; fi; "
           f"test \"$(getent passwd {USER} | cut -d: -f6)\" = {ROOT}; "
           f"if test -e {ROOT}; then test ! -L {ROOT}; test \"$(stat -c %U {ROOT})\" = {USER}; fi; "
           f"install -d -m 0700 -o {USER} -g {USER} {ROOT} {ROOT}/secrets "
           f"{ROOT}/secrets/.secrets {ROOT}/secrets/.secrets/kiln-live "
           f"{ROOT}/secrets/.secrets/kiln-live/default {ROOT}/probe-target {ROOT}/store")
    put(args.host, "secrets/secretspec.toml", '''[project]
name = "kiln-live"
revision = "1.0"
[providers]
local = "file:./.secrets"
[profiles.default]
SOURCE_TOKEN = { description = "Temporary operator-authorized exact GitHub source credential", providers = ["local"] }
''')
    token = run(["gh", "auth", "token"], stdout=subprocess.PIPE).stdout.strip()
    if not token or b"\n" in token:
        raise ValueError("GitHub credential capture must return exactly one token")
    put(args.host, "secrets/.secrets/kiln-live/default/SOURCE_TOKEN", token)
    # Exercise the real provider and compare without emitting either secret value.
    result = remote(args.host, f"cd {ROOT}/secrets && runuser -u {USER} -- "
                    f"{quote(secret)} get SOURCE_TOKEN --profile default --provider local")
    if result.strip() != token:
        raise ValueError("SecretSpec file provider did not resolve the supplied source token")
    key = ROOT + "/iroh.key"
    exists = remote(args.host, f"if test -e {key}; then echo exists; fi").strip()
    if not exists:
        public = worker(args.host, [package + "/bin/kiln-adapter-iroh", "keygen", "--key", key]).decode().strip()
        put(args.host, "worker-public.txt", public + "\n")
    data = {"package": package, "lattice": lattice, "nickel": nickel,
            "producer_source": producer, "secretspec": secret, "shell": shell, "root": ROOT, "user": USER}
    save(args.host, data)
    print(remote(args.host, f"id {USER}; hostname; b3sum {quote(secret)}; stat -c '%a %U %n' "
                 f"{ROOT} {key} {ROOT}/secrets/.secrets/kiln-live/default/SOURCE_TOKEN").decode(), end="")


def select_producer(args):
    data = settings(args.host)
    state = remote(args.host, "systemctl show " + UNITS["lattice"] + ".service -p ActiveState --value").strip()
    if state not in (b"inactive", b"failed"):
        raise RuntimeError("Stop only the owned Lattice unit before changing its immutable producer pin")
    data.update({"lattice": immutable(args.lattice), "producer_source": immutable(args.producer_source)})
    save(args.host, data)
    print(json.dumps({"lattice": data["lattice"], "producer_source": data["producer_source"]}))


def prepare(args):
    data = settings(args.host)
    if not re.fullmatch(r"[0-9a-f]{64}", args.identity):
        raise ValueError("Job identity must be its exact BLAKE3 64-hex digest")
    raw = Path(args.job).read_bytes()
    actual = run(["b3sum", "--no-names"], input=raw, stdout=subprocess.PIPE).stdout.decode().strip()
    if actual != args.identity:
        raise ValueError("Reviewed job bytes changed")
    put(args.host, "github-job.json", raw)
    put(args.host, "workflow-builder.ncl", (REPO / "config/live-ci-workflow.ncl").read_bytes())
    inputs = {"name": args.name, "command": "exec " + " ".join(map(quote, [
        data["package"] + "/bin/kiln-github-job", "--profile", ROOT + "/github-job.json",
        "--identity", args.identity])), "timeout_seconds": args.timeout_seconds,
        "created": args.created}
    put(args.host, "workflow-inputs.json", json.dumps(inputs))
    put(args.host, "workflow.ncl", 'let build = import "workflow-builder.ncl" in build (import "workflow-inputs.json")\n')
    config = {"data_dir": ROOT + "/store", "shell": data["shell"],
              "default_timeout_secs": args.timeout_seconds, "max_parallelism": 1}
    put(args.host, "lattice.ncl", "{\n" + ",\n".join(
        key + " = " + json.dumps(value) for key, value in config.items()) + "\n}\n")
    worker(args.host, [data["nickel"], "export", "--format", "json", ROOT + "/lattice.ncl"])
    print(worker(args.host, [data["lattice"], "--config", ROOT + "/lattice.ncl", "import", ROOT + "/workflow.ncl"]).decode(), end="")
    # Export is producer-owned; observation resolves the saved revision from the store.
    worker(args.host, [data["lattice"], "--config", ROOT + "/lattice.ncl", "export", args.name, ROOT + "/workflow-export.ncl"])
    observe = ('let base = import ' + json.dumps(data["producer_source"] + '/examples/orchestrations/llm-review-loop.ncl') + ' in\n'
               'base & { objective | force = "Read-only saved workflow identity observation; never start",\n'
               'workflow | force = { workflow_name = ' + json.dumps(args.name) + ', content_hash = "' + args.identity + '" }, triggers | force = [] }\n')
    put(args.host, "workflow-observe.ncl", observe)
    argv = ["runuser", "-u", USER, "--", data["lattice"], "--config", ROOT + "/lattice.ncl",
            "orchestration", "validate", ROOT + "/workflow-observe.ncl", "--output", "json"]
    result = subprocess.run(["ssh", "-o", "BatchMode=yes", args.host, " ".join(map(quote, argv))],
                            stdout=subprocess.PIPE, check=False)
    observed = result.stdout
    report = json.loads(observed)
    if result.returncode not in (0, 1) or not report.get("workflow_revision") or report.get("workflow_diagnostics"):
        raise ValueError("Producer did not observe a valid saved workflow")
    # Revalidate using only the producer's observed hash, never the initial probe value.
    observe = observe.replace(args.identity, report["workflow_revision"]["content_hash"], 1)
    put(args.host, "workflow-observe.ncl", observe)
    observed = remote(args.host, " ".join(map(quote, argv)))
    put(args.host, "workflow-observed.json", observed)
    data.update({"workflow_name": args.name, "job_identity": args.identity})
    save(args.host, data)
    print(observed.decode(), end="")


def configure(args):
    data = settings(args.host)
    native = json.loads(Path(args.native).read_text())
    uid = int(remote(args.host, f"id -u {USER}"))
    inputs = {"native": native, "uid": uid, "root": ROOT, "bind": args.bind,
              "controller_id": args.controller_id, "exchange_timeout_ms": args.exchange_timeout_ms}
    put(args.host, "iroh-workflow.ncl", (REPO / "config/iroh-workflow.ncl").read_bytes())
    put(args.host, "worker-builder.ncl", (REPO / "config/live-ci-worker.ncl").read_bytes())
    put(args.host, "worker-inputs.json", json.dumps(inputs))
    put(args.host, "worker.ncl", 'let build = import "worker-builder.ncl" in build (import "worker-inputs.json")\n')
    exported = worker(args.host, [data["nickel"], "export", "--format", "json", ROOT + "/worker.ncl"])
    put(args.host, "worker.json", exported)
    print(worker(args.host, [data["package"] + "/bin/kiln-adapter-iroh", "validate", "--profile", ROOT + "/worker.json"]).decode(), end="")
    if args.revision is None:
        return
    if args.lattice_connections is None:
        raise ValueError("Final route requires explicit finite --lattice-connections lifetime budget")
    observed = json.loads(remote(args.host, f"cat {ROOT}/workflow-observed.json"))
    if args.revision != "b3:" + observed["workflow_revision"]["content_hash"]:
        raise ValueError("Revision differs from retained producer observation")
    if len(native["nodes"]) != 1:
        raise ValueError("This composition maps exactly one reviewed shell operation")
    node = native["nodes"][0]
    transport = json.loads(exported)["transport"]
    # Lattice counts lifetime exchanges; Iroh counts concurrent exchanges.
    transport["limits"]["maximum_connections"] = args.lattice_connections
    handler = {"schema": "lattice.workflow-exchange-handler-profile.v1", "socket_path": ROOT + "/lattice.sock",
               "transport": transport, "maximum_active_executions": MAXIMUM_ACTIVE_EXECUTIONS,
               "routes": [{"profile_identity": native["profile_identity"], "graph_identity": native["graph_identity"],
                           "workflow_name": data["workflow_name"], "workflow_revision": args.revision,
                           "effect_authority": "route_exact", "source_binding": "shell_environment",
                           "nodes": [{"request_node_id": node["id"],
                                      "operation_identity": node["operation_identity"], "workflow_node_id": 0}]}],
               "non_claims": ["handler admission does not prove workflow correctness", "route authority does not prove host confinement",
                              "execution observation does not prove an external effect succeeded", "peer identity does not prove peer intent",
                              "handler checks do not establish release readiness"]}
    put(args.host, "handler-inputs.json", json.dumps(handler))
    put(args.host, "handler.ncl", 'let Contract = import ' + json.dumps(data["producer_source"] + '/contracts/workflow-exchange-handler-profile.ncl') + ' in (import "handler-inputs.json") | Contract.HandlerProfile\n')
    checked = worker(args.host, [data["nickel"], "export", "--format", "json", ROOT + "/handler.ncl"])
    put(args.host, "handler.json", checked)


def lattice_listeners(host):
    lines = remote(host, "ss -H -xlnp").decode().splitlines()
    return [line for line in lines if ROOT + "/lattice.sock" in line.split()]


def stop(args):
    unit = UNITS[args.component] + ".service"
    state = remote(args.host, "systemctl show " + unit + " -p ActiveState --value").strip()
    if state not in (b"inactive", b"failed"):
        remote(args.host, "systemctl stop " + unit)
    if args.component == "lattice":
        if lattice_listeners(args.host):
            raise RuntimeError("Owned native endpoint still has a listener; refusing removal")
        # SIGTERM can leave Lattice's pathname behind. Remove only our stopped socket.
        path = ROOT + "/lattice.sock"
        remote(args.host, f"set -eu; test ! -L {ROOT}; "
               f"test \"$(stat -c %U {ROOT})\" = {USER}; "
               f"if test -e {path} || test -L {path}; then "
               f"test ! -L {path}; test -S {path}; "
               f"test \"$(stat -c %U {path})\" = {USER}; "
               f"runuser -u {USER} -- rm -- {path}; fi")


def start(args):
    data = settings(args.host)
    state = remote(args.host, "systemctl show " + UNITS[args.component] + ".service -p ActiveState --value").strip()
    if state not in (b"inactive", b"failed"):
        raise RuntimeError("Owned component is already active; stop it explicitly before restarting")
    if args.component == "lattice":
        if remote(args.host, f"if test -e {ROOT}/lattice.sock || test -L {ROOT}/lattice.sock; then echo exists; fi").strip():
            raise RuntimeError("Owned native endpoint remains; run stop lattice before restarting")
        command = [data["lattice"], "--config", ROOT + "/lattice.ncl", "workflow-exchange", "serve", "--profile", ROOT + "/handler.ncl"]
    else:
        # Each start gets one startup line; never mix old endpoints with a new bind.
        put(args.host, "iroh.log", "")
        command = [data["package"] + "/bin/kiln-adapter-iroh", "worker", "--profile", ROOT + "/worker.json", "--key", ROOT + "/iroh.key"]
    remote(args.host, " ".join(map(quote, ["systemd-run", "--unit=" + UNITS[args.component], "--uid=" + USER,
        "--gid=" + USER, "--working-directory=" + ROOT, "--collect", "--property=UMask=0077",
        "--property=NoNewPrivileges=yes", "--property=Environment=HOME=" + ROOT,
        "--property=Environment=PATH=/run/current-system/sw/bin", "--property=StandardOutput=append:" + ROOT + "/" + args.component + ".log",
        "--property=StandardError=append:" + ROOT + "/" + args.component + ".err", *command])))
    for _ in range(READINESS_ATTEMPTS):
        if args.component == "lattice":
            pid = remote(args.host, "systemctl show " + UNITS[args.component] + ".service -p MainPID --value").decode().strip()
            if pid == "0":
                raise RuntimeError("Owned Lattice process exited before readiness; inspect its private .log/.err files")
            if any(f"pid={pid}," in line for line in lattice_listeners(args.host)):
                print(UNITS[args.component] + ".service socket ready")
                return
        else:
            raw = remote(args.host, f"cat {ROOT}/iroh.log").strip()
            if raw:
                endpoint = json.loads(raw.splitlines()[0])
                put(args.host, "worker-endpoint.json", json.dumps(endpoint) + "\n")
                print(json.dumps(endpoint))
                return
        time.sleep(READINESS_POLL_SECONDS)
    raise RuntimeError("Owned runtime not ready; inspect its private .log/.err files")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="root@localhost")
    sub = parser.add_subparsers(dest="action", required=True)
    boot = sub.add_parser("bootstrap")
    for name in ("package", "lattice", "nickel", "producer-source", "secretspec", "shell"):
        boot.add_argument("--" + name, required=True)
    select = sub.add_parser("select-producer")
    select.set_defaults(action="select_producer")
    select.add_argument("--lattice", required=True)
    select.add_argument("--producer-source", required=True)
    prep = sub.add_parser("prepare")
    for name in ("job", "identity", "name", "created"):
        prep.add_argument("--" + name, required=True)
    prep.add_argument("--timeout-seconds", type=int, required=True)
    config = sub.add_parser("configure")
    for name in ("native", "controller-id", "bind"):
        config.add_argument("--" + name, required=True)
    config.add_argument("--exchange-timeout-ms", type=int, required=True)
    config.add_argument("--revision", help="Observed saved revision; omit to configure only the bridge")
    config.add_argument("--lattice-connections", type=int, help="Independent native server lifetime connection budget")
    for name in ("start", "stop"):
        p = sub.add_parser(name)
        p.add_argument("component", choices=UNITS)
    sub.add_parser("status")
    sub.add_parser("retire-credentials")
    args = parser.parse_args()
    if args.action == "stop":
        stop(args)
    elif args.action == "status":
        print(remote(args.host, "systemctl show " + " ".join(u + ".service" for u in UNITS.values()) + " -p Id -p ActiveState -p SubState -p MainPID").decode(), end="")
    elif args.action == "retire-credentials":
        remote(args.host, f"rm -f {ROOT}/secrets/.secrets/kiln-live/default/SOURCE_TOKEN")
        print("Worker temporary source credential retired")
    else:
        globals()[args.action](args)


if __name__ == "__main__":
    main()
