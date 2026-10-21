#!/usr/bin/env python3
"""Qualify only a fresh trusted same-host native Radicle CI canary.

No existing node, production repository, public peer, activation or publication.
The broker supplies every protocol request; the adapter only records bytes and
performs the explicitly requested interruption/recovery of the original input.
"""
import argparse
import json
import os
from pathlib import Path
import pwd
import re
import runpy
import selectors
import shlex
import signal
import socket
import subprocess
import sys
import time

REPO = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO / "scripts"))
PILOT = runpy.run_path(str(REPO / "scripts/onixos-ci-pilot.py"))
BaseExercise, put, load, ncl, owned_command = (
    PILOT[name] for name in ("Exercise", "put", "load", "ncl", "owned_command"))
from radicle_ci_live_signer import prepare_signer

DOCUMENT_BYTES = 4 * 1024 * 1024
RECOVERY_SECONDS = 360
POLL_SECONDS = 0.1
PEER_TIMEOUT = "20s"
NON_CLAIMS = [
    "same-host native loopback TCP is not WAN or distinct-host qualification",
    "trusted canary source and shared-user workers do not establish hostile-worker confinement",
    "a real Nix build is not activation, deployment, release authority or arbitrary reproducibility",
    "upstream event deduplication is distinct from canonical controller reconciliation",
    "private dedicated test identities grant no production repository or governance authority",
]


def adapter(root, coordinator, profile):
    """Transparent bounded recorder; recovery resends only original broker bytes."""
    raw = sys.stdin.buffer.read(DOCUMENT_BYTES + 1)
    if len(raw) > DOCUMENT_BYTES:
        raise ValueError("broker request exceeds its bound")
    message = json.loads(raw)
    revision = message["after"]
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("broker revision is not exact")
    directory = Path(root) / "ingress" / revision
    directory.mkdir(mode=0o700)
    put(directory / "request.json", raw)
    for attempt in range(2):
        process = subprocess.Popen([coordinator, "--profile", profile], stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        process.stdin.write(raw)
        process.stdin.close()
        selector = selectors.DefaultSelector()
        selector.register(process.stdout, selectors.EVENT_READ, "stdout")
        selector.register(process.stderr, selectors.EVENT_READ, "stderr")
        buffers = {"stdout": bytearray(), "stderr": bytearray()}
        interrupted = False
        deadline = time.monotonic() + RECOVERY_SECONDS
        try:
            while selector.get_map():
                if time.monotonic() >= deadline:
                    raise TimeoutError("bounded controller adapter deadline")
                if attempt == 0 and (directory / "interrupt").exists() and not interrupted:
                    process.kill()
                    interrupted = True
                for key, _ in selector.select(POLL_SECONDS):
                    chunk = os.read(key.fileobj.fileno(), 4096)
                    if not chunk:
                        selector.unregister(key.fileobj)
                        continue
                    if len(buffers[key.data]) + len(chunk) > DOCUMENT_BYTES:
                        raise ValueError("controller output exceeds its bound")
                    buffers[key.data].extend(chunk)
                    stream = sys.stdout.buffer if key.data == "stdout" else sys.stderr.buffer
                    stream.write(chunk)
                    stream.flush()
            process.wait(timeout=5)
        finally:
            selector.close()
            if process.poll() is None:
                process.kill()
                process.wait(timeout=5)
            process.stdout.close()
            process.stderr.close()
        for name, value in buffers.items():
            put(directory / f"attempt-{attempt}-{name}.log", bytes(value))
        put(directory / f"attempt-{attempt}.json", {"exit_code": process.returncode,
            "interrupted": interrupted, "original_input_reused": True})
        if not interrupted:
            put(directory / "complete.json", {"exit_code": process.returncode, "attempts": attempt + 1})
            return process.returncode
        put(directory / "interrupted.json", {"exit_code": process.returncode})
        deadline = time.monotonic() + RECOVERY_SECONDS
        while not (directory / "resume").exists():
            if time.monotonic() >= deadline:
                raise TimeoutError("explicit controller recovery was not requested")
            time.sleep(POLL_SECONDS)
    raise RuntimeError("controller recovery bound exceeded")


class Exercise(BaseExercise):
    def __init__(self, args):
        root = Path(args.root)
        if (not root.is_absolute() or root.parts[:3] != ("/", "var", "tmp")
                or len(root.parts) < 4 or any(part in (".", "..") for part in args.root.split("/"))):
            raise ValueError("live canary requires a fresh canonical /var/tmp descendant")
        super().__init__(args)
        self.exe.update(load(Path(args.inputs))["executables"])
        self.passwd_home = pwd.getpwuid(os.getuid()).pw_dir
        (self.root / "passwd-home").mkdir(mode=0o700)
        self.nodes = {}

    def isolated(self, argv):
        # SecretSpec clears env, so its passwd-home fallback must also be owned.
        # Preserve the checkout read-only after masking the operator's whole home.
        command = owned_command(self.exe, list(map(str, argv)))
        separator = command.index("--")
        command[separator:separator] = ["--bind", str(self.root / "passwd-home"), self.passwd_home,
                                        "--ro-bind", str(REPO), str(REPO), "--chdir", str(self.root)]
        return command

    def run(self, argv, **kwargs):
        return super().run(self.isolated(argv), **kwargs)

    def spawn(self, name, argv, **kwargs):
        return super().spawn(name, self.isolated(argv), **kwargs)

    def ci(self, *arguments):
        return self.json_command([self.exe["coordinator"], *arguments])

    def jj(self, directory, *arguments):
        return self.run([self.exe["jj"], "--config", 'user.name="Kiln Canary"', "--config",
                         'user.email="canary@kiln.invalid"', "--config", 'signing.behavior="drop"',
                         "--config", "git.subprocess=true", "--config", "git.executable=" + json.dumps(self.exe["git"]),
                         "-R", directory, *arguments])

    def rad(self, role, *arguments):
        previous = self.env.get("RAD_HOME")
        self.env["RAD_HOME"] = self.nodes[role]["home"]
        try:
            return self.run([self.exe["radicle"], *arguments])
        finally:
            if previous is None:
                self.env.pop("RAD_HOME", None)
            else:
                self.env["RAD_HOME"] = previous

    def source_files(self, value):
        source = self.root / "project-source"
        if source.exists():
            (source / "value.txt").write_text(value + "\n")
            return source
        source.mkdir(mode=0o700)
        put(source / "value.txt", (value + "\n").encode())
        build = ('${pkgs.coreutils}/bin/sleep ' + str(self.settings["build_delay_seconds"])
                 + '; read value < ${./value.txt}; if [ "$value" = fail ]; then '
                 + 'echo source-requested-genuine-build-failure >&2; exit 19; fi; '
                 + 'printf \'%s\\n\' "$value" > "$out"')
        expression = ('{ inputs.nixpkgs = { flake = false; url = '
                      + json.dumps("path:" + str(Path(self.args.nixpkgs).resolve(strict=True)))
                      + '; }; outputs = { self, nixpkgs }: let pkgs = import nixpkgs { system = "x86_64-linux"; }; '
                      + 'in { jobs.app = builtins.derivation { name = "kiln-radicle-live-ci"; '
                      + 'system = "x86_64-linux"; builder = "${pkgs.bash}/bin/bash"; args = [ "-c" '
                      + json.dumps(build) + ' ]; }; }; }\n')
        put(source / "flake.nix", expression.encode())
        self.run([self.exe["nix"], "flake", "lock", "--offline", "--extra-experimental-features", "nix-command flakes", "path:" + str(source)])
        return source

    def commit_source(self, value):
        source = self.source_files(value)
        self.jj(source, "commit", "-m", "Dedicated trusted canary: " + value)
        self.jj(source, "bookmark", "set", self.settings["source_branch"], "-r", "@-")
        revision = self.jj(source, "log", "--no-graph", "-r", "@-", "-T", "commit_id")["stdout"].strip()
        if not re.fullmatch(r"[0-9a-f]{40}", revision):
            raise ValueError("source commit is not exact")
        return {"provider": "radicle", "repository": self.rid, "revision": revision}

    def prepare_nodes(self):
        put(self.root / ".gitconfig", b"[init]\n\tdefaultBranch = ci-canary\n")
        for role in ("publisher", "ci", "observer"):
            home = self.root / ("rad-" + role)
            self.env["RAD_HOME"] = str(home)
            self.run([self.exe["radicle"], "auth", "--alias", "kiln-canary-" + role, "--stdin"], data=b"\n")
            self.private_files.extend(path for path in (home / "keys").rglob("*") if path.is_file())
            node_id = self.run([self.exe["radicle"], "self", "--did"])["stdout"].strip().removeprefix("did:key:")
            with socket.socket() as candidate:
                candidate.bind((self.settings["listen_host"], 0))
                port = candidate.getsockname()[1]
            config = load(home / "config.json")
            config["preferredSeeds"] = []
            config["node"].update({"listen": [f"127.0.0.1:{port}"], "connect": [],
                "peers": {"type": "static"}, "externalAddresses": [], "network": self.settings["network"], "relay": "never"})
            (home / "config.json").write_bytes(PILOT["encoded"](config))
            service = self.spawn("node-" + role, [self.exe["radicle_node"], "--force"])
            self.until("owned native node", lambda h=home, s=service: (h / "node/control.sock").is_socket() or self.failed(s))
            self.nodes[role] = {"home": str(home), "id": node_id, "port": port, "service": service}
        if len({node["id"] for node in self.nodes.values()}) != 3:
            raise ValueError("native source, status and observer identities are not distinct")
        source = self.source_files("bootstrap-" + self.root.name)
        self.env["RAD_HOME"] = self.nodes["publisher"]["home"]
        self.run([self.exe["jj"], "git", "init", "--colocate", source])
        self.jj(source, "commit", "-m", "Initialize dedicated trusted CI canary")
        self.jj(source, "bookmark", "set", self.settings["source_branch"], "-r", "@-")
        self.run([self.exe["radicle"], "init", "--no-confirm", "--no-seed", "--public", "--name", "kiln-live-canary",
                  "--description", "Dedicated isolated native CI canary", "--default-branch", self.settings["source_branch"], source])
        self.rid = self.run([self.exe["radicle"], "inspect", "--rid", source])["stdout"].strip()
        self.jj(source, "bookmark", "track", self.settings["source_branch"] + "@rad")
        self.rad("publisher", "seed", "--scope", "all", self.rid)
        self.rad("ci", "node", "connect", self.nodes["publisher"]["id"] + "@127.0.0.1:" + str(self.nodes["publisher"]["port"]))
        self.rad("ci", "seed", "--scope", "all", "--from", self.nodes["publisher"]["id"], "--timeout", PEER_TIMEOUT, self.rid)
        repository = Path(self.nodes["ci"]["home"]) / "storage" / self.rid.removeprefix("rad:")
        if not repository.is_dir():
            raise ValueError("native peer did not acquire the declared source repository")
        self.selection = {"repository_path": str(repository), "repository_id": self.rid,
            "signer_node_id": self.nodes["publisher"]["id"], "signed_reference": "refs/heads/" + self.settings["source_branch"],
            "profile_id": "radicle-source-v1", "limits": {"max_identity_bytes": 4096, "max_reference_bytes": 512,
            "max_entries": 512, "max_path_bytes": 1024, "max_file_bytes": DOCUMENT_BYTES, "max_total_bytes": DOCUMENT_BYTES * 4}}
        signer_root = self.root / "signer"
        self.private_files.extend(prepare_signer(signer_root, Path(self.nodes["ci"]["home"]) / "keys/radicle"))
        self.env["RAD_HOME"] = self.nodes["ci"]["home"]
        self.status_checkout = self.root / "status-checkpoint"
        # Explicit helper syntax preserves the case-sensitive multibase RID.
        self.run([self.exe["jj"], "--config", "git.subprocess=true", "--config", "git.executable=" + json.dumps(self.exe["git"]),
                  "git", "clone", "rad::rad://" + self.rid.removeprefix("rad:"), self.status_checkout])
        status_url = "rad::rad://" + self.rid.removeprefix("rad:") + "/" + self.nodes["ci"]["id"]
        self.jj(self.status_checkout, "git", "remote", "set-url", "origin", "--fetch", status_url, "--push", status_url)
        self.jj(self.status_checkout, "git", "remote", "add", "publisher",
                "rad::rad://" + self.rid.removeprefix("rad:") + "/" + self.nodes["publisher"]["id"])
        # Broker 0.31's LocalRefsAnnounced observer requires a previous sigrefs
        # commit. Initialize this declared namespace before subscribing, just as
        # the source namespace and baseline fetch precede live event admission.
        initial_revision = self.jj(source, "log", "--no-graph", "-r", "@-", "-T", "commit_id")["stdout"].strip()
        self.jj(self.status_checkout, "bookmark", "set", self.settings["status_checkpoint_branch"], "-r", initial_revision)
        self.jj(self.status_checkout, "git", "push", "--remote", "origin", "--bookmark", self.settings["status_checkpoint_branch"])
        self.rad("observer", "node", "connect", self.nodes["ci"]["id"] + "@127.0.0.1:" + str(self.nodes["ci"]["port"]))
        self.rad("observer", "seed", "--scope", "all", "--from", self.nodes["ci"]["id"], "--timeout", PEER_TIMEOUT, self.rid)
        return {"provider": "radicle", "repository": self.rid,
                "revision": self.jj(source, "log", "--no-graph", "-r", "@-", "-T", "commit_id")["stdout"].strip()}

    def prepare_runtime(self, inputs, initial):
        facts = {name: {"path": path, "blake3": self.digest(path=path)} for name, path in self.exe.items() if Path(path).is_file()}
        operation = self.digest(value={"root": str(self.root), "executables": facts, "source_policy": self.selection})
        policy_inputs = {"root": str(self.root), "source_path": "/nix/store/00000000000000000000000000000000-project-template",
            "source_identity": operation, "case": "success", "operation_identity": operation, "worker_identity": facts["worker"]["blake3"],
            "store_identity": facts["nix_store"]["blake3"], "plan_identity": operation, "policy_identity": operation, "user_id": os.getuid()}
        policy = self.export("runtime-policy", "((import " + json.dumps(str(REPO / "config/onixos-ci-pilot.ncl")) + ") " + ncl(policy_inputs)
                             + ') & { eval_selection.source_binding.root_selector | force = "jobs" }')
        put(self.root / "eval-selection.json", policy["eval_selection"])
        evaluation = self.json_command([self.exe["worker"], "eval-profile", self.root / "eval-selection.json"])
        build = policy["build_profile"]
        intent = {"effect_id": "project-template", "attempt_id": "project-template", "idempotency_key": operation, "candidate_id": "project-template",
                  "pipeline_revision": "radicle-source-ci", "plan_identity": operation, "policy_identity": operation, "platform": "x86_64-linux"}
        discovery = {"effect_id": "project-template", "evaluation_id": "project-template", "project_id": "radicle-source-ci",
            "pipeline_revision_id": intent["pipeline_revision"], "discovery_policy_id": "onix-explicit-project-v1", "provider_profile": evaluation["provider"]["profile_id"],
            "evaluator_cohort": evaluation["evaluator_cohort"], "build_provider_profile": build["provider_profile_id"], "idempotency_key": operation,
            "source_identity": operation, "source": policy["eval_selection"]["source_binding"]["source"], "source_mode": "Flake", "root_selector": "jobs", "system": "x86_64-linux",
            "options": {"workers": 1, "max_memory_mebibytes": 1024, **{key: evaluation["policy"][key] for key in ("include_metadata", "include_input_derivations", "check_cache_status", "partial_admission")}},
            "bounds": {key: evaluation["limits"][key] for key in ("max_records", "max_jobs", "max_job_errors", "max_diagnostic_bytes", "max_outputs_per_job", "max_dependencies_per_job", "max_tasks")}}
        project = self.export("project-profile", "(import " + json.dumps(str(REPO / "config/radicle-ci-worker.ncl")) + ") " + ncl({"source": self.selection,
            "state_root": str(self.root / "candidates"), "discovery_profile": evaluation, "discovery_plan": discovery, "selected_task": "app", "build_profile": build, "build_intent": intent}))
        put(self.root / "project.json", project)
        self.project_identity = self.digest(path=self.root / "project.json")
        controller_inputs = {"worker_profile_path": str(self.root / "project.json"), "worker_profile_identity": self.project_identity,
            "socket_path": str(self.root / "aspen.sock"), "status_storage_root": str(Path(self.nodes["ci"]["home"]) / "storage"),
            "status": {"allowed_rids": [self.rid], "signer_identity": self.reference("node", self.nodes["ci"]["id"]),
            "signer": {"project_root": str(self.root / "signer"), "project": "kiln-canary", "profile": "production", "secret_key": "RADICLE_SIGNER_SEED", "provider": "dotenv"}}}
        controller = self.export("controller-profile", "(import " + json.dumps(str(REPO / "config/radicle-ci.ncl")) + ") " + ncl(controller_inputs))
        controller["runtime_profile"] = policy["aspen_document"]
        controller["runtime_profile"]["bounds"]["callback_timeout_millis"] = self.settings["callback_timeout_milliseconds"]
        controller.update({"maximum_polls": self.settings["maximum_polls"], "poll_interval_milliseconds": self.settings["poll_interval_milliseconds"]})
        put(self.root / "controller.json", controller)
        put(self.root / "status-profile.json", controller["status_profile"])
        put(self.root / "aspen.json", controller["runtime_profile"])
        put(self.root / "radicle.json", policy["radicle_profile"])
        # Configuration-only route lowering of the measured bootstrap source.
        # This document is never delivered to the broker/controller or dispatched.
        template = {"repository": self.reference("repository", self.rid), "actor": self.reference("actor", "broker"), "before": None,
                    "after": self.reference("revision", initial["revision"]), "branch": self.settings["source_branch"], "patch": None, "event": "push"}
        put(self.root / "route-selection.json", template)
        request = self.json_command([self.exe["legacy_coordinator"], "native-workflow", self.root / "radicle.json", self.root / "route-selection.json"])
        lattice = self.workflow(inputs, policy, self.project_identity, request, worker_arguments=["project-run", self.root / "project.json", self.project_identity])
        self.until("Lattice socket", lambda: (self.root / "lattice.sock").is_socket() or self.failed(lattice))
        state = self.root / "host-state"
        state.mkdir(mode=0o700)
        options = policy["host_options"]
        self.host_argv = [self.exe["host"], "--aspen-profile", self.root / "aspen.json", "--radicle-profile", self.root / "radicle.json", "--socket", self.root / "aspen.sock",
            "--extension", self.exe["extension"], "--state-root", state, "--max-requests", str(self.settings["host_maximum_requests"]),
            "--timeout-ms", options["timeout_milliseconds"], "--poll-interval-ms", options["poll_interval_milliseconds"]]
        self.host = self.spawn("host-first", self.host_argv)
        self.until("Aspen socket", lambda: (self.root / "aspen.sock").is_socket() or self.failed(self.host))
        put(self.root / "cohort.json", {"schema": "kiln.radicle-ci.live-cohort.v1", "executables": facts, "operation_identity": operation,
            "settings": self.settings, "topology": {role: {key: value for key, value in node.items() if key != "service"} for role, node in self.nodes.items()},
            "source_selection": self.selection, "project_profile_identity": self.project_identity, "runtime_profile": controller["runtime_profile"],
            "consumer_tools": {"nixpkgs_source": str(Path(self.args.nixpkgs).resolve(strict=True)), "lock": load(self.root / "project-source/flake.lock")},
            "passwd_home_isolation": {"host_path": self.passwd_home, "owned_backing": str(self.root / "passwd-home"), "checkout_read_only": True}, "non_claims": NON_CLAIMS})
        return lattice

    @staticmethod
    def reference(kind, value):
        return {"contract_version": 1, "provider_namespace": "radicle", "native_kind": kind, "native_id": value}

    def prepare_broker(self):
        (self.root / "ingress").mkdir(mode=0o700)
        launcher = self.root / "adapter"
        put(launcher, ("#!" + self.exe["python"] + "\nimport os\nos.execv(" + repr(self.exe["python"]) + ", "
            + repr([self.exe["python"], str(Path(__file__).resolve()), "--adapter", str(self.root), self.exe["coordinator"], str(self.root / "controller.json")]) + ")\n").encode())
        launcher.chmod(0o700)
        config = {"db": str(self.root / "broker.db"), "max_run_time": "360s", "queue_len_interval": "60s", "concurrent_adapters": 1,
            "update_job_cobs": False, "adapters": {"kiln": {"command": str(launcher), "env": self.env}},
            "triggers": [{"adapter": "kiln", "filters": [{"And": [{"Repository": self.rid}, {"Node": self.nodes["publisher"]["id"]},
                {"Branch": self.settings["source_branch"]}, {"Or": ["BranchCreated", "BranchUpdated"]}]}]}]}
        put(self.root / "broker.json", config)
        self.json_command([self.exe["broker"], "--config", self.root / "broker.json", "config"])
        events = self.spawn("native-events", [self.exe["radicle"], "node", "events", "--timeout", "360s"])
        broker = self.spawn("native-broker", [self.exe["broker"], "--config", self.root / "broker.json", "--log-level", "trace", "process-events"])
        self.until("real broker event subscription", lambda: b"created node event source" in broker["log"].read_bytes() or self.failed(broker))
        return broker, events

    def independent_status(self, case, source, journal):
        written = journal["status_observation"]["observation"]
        plan = self.root / (case + "-status-plan.json")
        put(plan, journal["status_intent"])
        observer = Path(self.nodes["observer"]["home"]) / "storage"
        absent = self.ci("status-reconcile", self.root / "status-profile.json", observer, plan)
        if absent is not None:
            raise ValueError("unannounced dedicated status was already visible remotely")
        self.jj(self.status_checkout, "git", "fetch", "--remote", "origin")
        self.jj(self.status_checkout, "git", "fetch", "--remote", "publisher")
        self.jj(self.status_checkout, "bookmark", "set", self.settings["status_checkpoint_branch"], "-r", source["revision"])
        self.jj(self.status_checkout, "git", "push", "--remote", "origin", "--bookmark", self.settings["status_checkpoint_branch"])
        self.rad("observer", "sync", "--fetch", "--seed", self.nodes["ci"]["id"], "--timeout", PEER_TIMEOUT, self.rid)
        remote = self.ci("status-reconcile", self.root / "status-profile.json", observer, plan)
        if remote != written:
            raise ValueError("independently fetched status differs from exact local object/head")
        put(self.root / (case + "-remote-status.json"), {"before_sign_sync": absent, "after_native_fetch": remote})
        return remote

    # r[impl kiln.radicle_ci.live.authority] r[verify kiln.radicle_ci.live.authority]
    # r[impl kiln.radicle_ci.live.ingress] r[verify kiln.radicle_ci.live.ingress]
    # r[impl kiln.radicle_ci.live.status] r[verify kiln.radicle_ci.live.status]
    # r[impl kiln.radicle_ci.live.recovery] r[verify kiln.radicle_ci.live.recovery]
    # r[impl kiln.radicle_ci.live.failure] r[verify kiln.radicle_ci.live.failure]
    # r[impl kiln.radicle_ci.live.evidence] r[verify kiln.radicle_ci.live.evidence]
    def execute(self):
        inputs = self.export("admitted-inputs", "(import " + json.dumps(str(REPO / "config/onixos-ci-pilot-inputs.ncl")) + ") (import " + json.dumps(str(Path(self.args.inputs).resolve(strict=True))) + ")")
        tools = {key: str(Path(getattr(self.args, key)).absolute()) for key in ("radicle", "radicle_node", "broker", "broker_tool", "jj", "git", "secretspec")}
        self.settings = self.export("canary-settings", "(import " + json.dumps(str(REPO / "config/radicle-ci-live.ncl")) + ") " + ncl({"root": str(self.root),
            "source_branch": "ci-canary", "status_checkpoint_branch": "kiln-status-checkpoint", "tools": tools}))
        self.exe.update(tools)
        self.exe["legacy_coordinator"] = self.exe["coordinator"]
        self.exe.update({name: str(Path(getattr(self.args, name)).resolve(strict=True)) for name in ("coordinator", "worker", "extension")})
        self.exe["python"] = str(Path(sys.executable).resolve(strict=True))
        for path in self.exe.values():
            Path(path).resolve(strict=True)
        self.env.update({"TMPDIR": str(self.root), "PATH": ":".join(dict.fromkeys(str(Path(self.exe[name]).parent) for name in ("git", "radicle", "shell", "sleep", "jj")))})
        (self.root / "lattice-tmp").mkdir(mode=0o700)
        (self.root / "candidates").mkdir(mode=0o700)
        initial = self.prepare_nodes()
        self.env["RAD_HOME"] = self.nodes["ci"]["home"]
        lattice = self.prepare_runtime(inputs, initial)
        broker, events = self.prepare_broker()
        reports = []
        for number, case in enumerate(("success", "failure", "unavailable")):
            self.env["RAD_HOME"] = self.nodes["publisher"]["home"]
            source = self.commit_source("fail" if case == "failure" else case + "-" + self.root.name)
            context = self.root / (case + "-source.json")
            put(context, source)
            self.env["RAD_HOME"] = self.nodes["ci"]["home"]
            bindings = self.ci("bindings", self.root / "controller.json", context)
            output = self.run([self.exe["nix"], "eval", "--raw", "--offline", "--extra-experimental-features", "nix-command flakes", "path:" + str(self.root / "project-source") + "#jobs.app.outPath"])["stdout"]
            before = self.run([self.exe["nix_store"], "--check-validity", output], allowed=(0, 1))
            if Path(output).exists() or before["exit_code"] == 0:
                raise ValueError("consumer output already present before real peer dispatch")
            if case == "unavailable":
                self.stop(self.host, abrupt=True)
                (self.root / "aspen.sock").unlink(missing_ok=True)
            self.env["RAD_HOME"] = self.nodes["publisher"]["home"]
            self.jj(self.root / "project-source", "git", "push", "--remote", "rad", "--bookmark", self.settings["source_branch"])
            self.env["RAD_HOME"] = self.nodes["ci"]["home"]
            ingress = self.root / "ingress" / source["revision"]
            self.until("automatic real broker ingress", lambda: (ingress / "request.json").is_file() or self.failed(broker))
            message = load(ingress / "request.json")
            if message["repository"]["id"] != self.rid or message["after"] != source["revision"]:
                raise ValueError("broker delivered a foreign repository/revision")
            journal_path = Path(bindings["root"]) / "ci-journal.json"
            self.until("canonical native source reservation", lambda: journal_path.is_file() or self.failed(broker))
            original = load(journal_path)["ingress"]["operation_id"]
            restart = None
            if case == "success":
                self.until("real independent Nix build intent", lambda: (Path(bindings["root"]) / "build-state.json").is_file() or self.failed(broker))
                put(ingress / "interrupt", b"explicit owned controller interruption\n")
                self.until("controller SIGKILL evidence", lambda: (ingress / "interrupted.json").is_file() or self.failed(broker))
                self.stop(self.host, abrupt=True)
                pending = self.ci("replay", self.root / "controller.json", context)
                restart = {"canonical_outcome": pending["runs"][bindings["run_id"]]["outcome"], "worker_result_present": (Path(bindings["root"]) / "project-result.json").exists(),
                           "independent_lattice_running": lattice["process"].poll() is None, "original_operation": original}
                if restart["canonical_outcome"] != "Pending" or restart["worker_result_present"] or not restart["independent_lattice_running"]:
                    raise ValueError("interruption missed actual independent in-flight execution")
                (self.root / "aspen.sock").unlink(missing_ok=True)
                self.host = self.spawn("host-restarted", self.host_argv)
                self.until("recovered Aspen socket", lambda: (self.root / "aspen.sock").is_socket() or self.failed(self.host))
                put(ingress / "resume", b"reconcile original unchanged broker bytes\n")
            self.until("bounded controller completion", lambda: (ingress / "complete.json").is_file() or self.failed(broker))
            replies = [json.loads(line) for path in sorted(ingress.glob("attempt-*-stdout.log")) for line in path.read_text().splitlines()]
            journal = self.ci("reconcile", self.root / "controller.json", context)
            state = self.ci("replay", self.root / "controller.json", context)
            expected = {"success": "Success", "failure": "DomainFailure", "unavailable": "Pending"}[case]
            if state["runs"][bindings["run_id"]]["outcome"] != expected or journal["ingress"]["operation_id"] != original:
                raise ValueError("canonical outcome or original operation differs")
            # A real duplicate fetch must not be promoted to controller recovery.
            before_invocations = (self.root / "provider-invocations.log").read_text().splitlines()
            duplicate = self.rad("ci", "sync", "--fetch", "--seed", self.nodes["publisher"]["id"], "--timeout", PEER_TIMEOUT, self.rid)
            duplicate_journal = self.ci("reconcile", self.root / "controller.json", context)
            if duplicate_journal["ingress"]["operation_id"] != original or (self.root / "provider-invocations.log").read_text().splitlines() != before_invocations:
                raise ValueError("duplicate native fetch/reconciliation redispatched a worker")
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
                    if list(roots.values()) != [output] or Path(output).read_text() != "success-" + self.root.name + "\n":
                        raise ValueError("native source bytes or exact output root handoff differ")
                else:
                    diagnostics = b"".join(path.read_bytes() for path in Path(bindings["root"]).glob("*.log"))
                    if b"source-requested-genuine-build-failure" not in diagnostics:
                        raise ValueError("genuine source-controlled Nix failure diagnostic absent")
                release = [self.json_command([self.exe["worker"], "project-release", self.root / "project.json", self.project_identity, context]) for _ in range(2)]
                if any(path.is_symlink() for path in (self.root / "build-roots").rglob("*")) or (Path(bindings["root"]) / "source-root").is_symlink():
                    raise ValueError("source/build roots survived explicit idempotent release")
            put(self.root / (case + "-journal.json"), journal)
            reports.append({"case": case, "source": source, "operation_id": original, "canonical_outcome": expected, "original_broker_replies": replies,
                "restart": restart, "output_absent_before_dispatch": before, "remote_status": remote, "rooted_outputs": roots, "release": release,
                "native_duplicate_fetch": duplicate, "duplicate_preserved_operation": duplicate_journal["ingress"]["operation_id"]})
        invocations = (self.root / "provider-invocations.log").read_text().splitlines()
        if invocations != [report["source"]["revision"] for report in reports[:2]]:
            raise ValueError("duplicate/restart/unavailable provider altered real worker invocation count")
        audit = self.root / "passwd-home/.local/state/secretspec/audit.log"
        if not audit.is_file():
            raise ValueError("real isolated SecretSpec provider audit evidence absent")
        report = {"schema": "kiln.radicle-ci.live-verification.v1", "repository": self.rid, "cases": reports, "provider_invocations": invocations,
            "project_profile_identity": self.project_identity, "workflow_observation": load(self.root / "workflow-observed.json"),
            "real_secretspec_audit": str(audit), "maintenance_owner": "Kiln maintainers", "non_claims": NON_CLAIMS}
        put(self.root / "verification.json", report)
        print(json.dumps({"repository": self.rid, "cases": [{"case": row["case"], "revision": row["source"]["revision"], "outcome": row["canonical_outcome"]} for row in reports]}, indent=2))

    def close(self):
        # Stop owned execution before using the existing exact-result release
        # contract; even a rejected qualification must not leave completed roots.
        releases = []
        try:
            for item in reversed(self.services):
                self.stop(item)
            if hasattr(self, "project_identity"):
                for path in sorted((self.root / "candidates").glob("*/project-result.json")):
                    context = path.parent / "teardown-source.json"
                    put(context, load(path)["source_intent"]["source"])
                    releases.append(self.json_command([self.exe["worker"], "project-release", self.root / "project.json", self.project_identity, context]))
            put(self.root / "teardown-releases.json", releases)
        finally:
            super().close()


def main():
    if len(sys.argv) == 5 and sys.argv[1] == "--adapter":
        raise SystemExit(adapter(*sys.argv[2:]))
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("inputs", "nickel", "root", "worker", "extension", "coordinator", "radicle", "radicle-node", "broker", "broker-tool", "jj", "git", "secretspec", "nixpkgs"):
        parser.add_argument("--" + name, required=True)
    args = parser.parse_args()
    os.umask(0o077)
    exercise = Exercise(args)
    try:
        exercise.execute()
    finally:
        exercise.close()


if __name__ == "__main__":
    main()
