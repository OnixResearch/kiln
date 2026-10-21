#!/usr/bin/env python3
"""Exercise only an explicitly selected, isolated OnixOS CI runtime cohort.

No forge delivery, remote mutation, activation, production service, or implicit
provider fallback. Private credentials are fresh and retired by this process.
"""
import argparse
import copy
import json
import os
from pathlib import Path
import re
import selectors
import shlex
import signal
import socket
import subprocess
import time
from urllib.parse import quote

from onixos_ci_closure import admit_preparation_closure, consumer_output_status, preparation_plan

REPO = Path(__file__).resolve().parent.parent
COMMAND_SECONDS = 360
RECOVERY_SECONDS = 360
POLL_SECONDS = 0.1
WORKER_START_DELAY_SECONDS = 10
REQUEST_SECONDS = 600
RECONCILE_OBSERVATION_RESERVE = 2
DOCUMENT_BYTES = 4 * 1024 * 1024
SERVICE_LOG_BYTES = 32 * 1024 * 1024
REQUIRED_NON_CLAIMS = [
    "operator initiation is not autonomous forge delivery",
    "local restart evidence is not distributed exactly-once execution",
    "a built application is not activation or deployment evidence",
    "exact-cohort evidence is not general runtime or release qualification",
    "prepared external tools are not authenticated consumer construction evidence",
]


def encoded(value):
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def put(path, value):
    raw = value if isinstance(value, bytes) else encoded(value)
    if len(raw) > DOCUMENT_BYTES:
        raise ValueError("runner document exceeds its bound")
    with path.open("xb") as target:
        target.write(raw)


def load(path):
    with path.open("rb") as source:
        raw = source.read(DOCUMENT_BYTES + 1)
    if len(raw) > DOCUMENT_BYTES:
        raise ValueError("runner document exceeds its bound")
    return json.loads(raw)


def ncl(value):
    if isinstance(value, dict):
        if any(not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_-]*", key) for key in value):
            raise ValueError("invalid generated Nickel field")
        return "{ " + ", ".join(key + " = " + ncl(item) for key, item in value.items()) + " }"
    if isinstance(value, list):
        return "[" + ", ".join(map(ncl, value)) + "]"
    return json.dumps(value)


def owned_command(executables, arguments, temporary=None):
    # Separate PID namespaces own descendants even when a host or runner is killed.
    # Shared filesystem/network access deliberately provides no sandbox claim.
    prefix = [executables["bubblewrap"], "--die-with-parent", "--unshare-user", "--unshare-pid",
              "--bind", "/", "/"]
    if temporary is not None:
        prefix += ["--bind", str(temporary), "/tmp"]
    return [*prefix, "--proc", "/proc", "--dev", "/dev", "--", *arguments]


def lattice_command(executables, root, *arguments):
    # The pinned producer writes /tmp/lattice.log, not a configurable log path.
    return owned_command(executables, [executables["lattice"], *arguments], root / "lattice-tmp")


class Exercise:
    def __init__(self, args):
        self.args = args
        self.root = Path(args.root).absolute()
        if self.root == Path("/tmp") or Path("/tmp") in self.root.parents:
            raise ValueError("pilot scratch must be outside the producer's private /tmp")
        self.root.mkdir(mode=0o700)
        self.commands = []
        self.services = []
        self.private_files = []
        self.env = {"HOME": str(self.root), "XDG_CONFIG_HOME": str(self.root / "config"),
                    "XDG_CACHE_HOME": str(self.root / "cache"), "LANG": "C.UTF-8",
                    "PATH": str(Path(args.nickel).parent)}
        self.exe = {"nickel": str(Path(args.nickel).resolve(strict=True))}

    def run(self, argv, *, data=None, allowed=(0,)):
        argv = list(map(str, argv))
        process = subprocess.Popen(argv, cwd=REPO, env=self.env, stdin=subprocess.PIPE if data is not None else subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
        if data is not None:
            process.stdin.write(data)
            process.stdin.close()
        buffers = {"stdout": bytearray(), "stderr": bytearray()}
        selector = selectors.DefaultSelector()
        selector.register(process.stdout, selectors.EVENT_READ, "stdout")
        selector.register(process.stderr, selectors.EVENT_READ, "stderr")
        deadline = time.monotonic() + COMMAND_SECONDS
        try:
            while selector.get_map():
                if time.monotonic() >= deadline:
                    raise TimeoutError(f"bounded command deadline: {argv}")
                for key, _ in selector.select(POLL_SECONDS):
                    chunk = os.read(key.fileobj.fileno(), 4096)
                    if not chunk:
                        selector.unregister(key.fileobj)
                        continue
                    target = buffers[key.data]
                    if len(target) + len(chunk) > DOCUMENT_BYTES:
                        raise ValueError(f"bounded command output exceeded: {argv}")
                    target.extend(chunk)
            process.wait(timeout=5)
        finally:
            selector.close()
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=5)
            process.stdout.close()
            process.stderr.close()
        record = {"argv": argv, "exit_code": process.returncode,
                  **{key: bytes(value).decode(errors="replace") for key, value in buffers.items()}}
        self.commands.append(record)
        put(self.root / f"command-{len(self.commands):03}.json", record)
        if process.returncode not in allowed:
            raise RuntimeError(f"command failed: {argv}: {record['stderr']}")
        return record

    def json_command(self, argv, *, allowed=(0,)):
        return json.loads(self.run(argv, allowed=allowed)["stdout"])

    def export(self, name, expression):
        path = self.root / (name + ".ncl")
        put(path, (expression + "\n").encode())
        return self.json_command([self.exe["nickel"], "export", "--format", "json", path])

    def digest(self, *, path=None, value=None):
        argv = [self.exe["b3sum"], "--no-names"]
        if path is not None:
            argv.append(path)
        result = self.run(argv, data=None if path is not None else encoded(value))["stdout"].strip()
        if not re.fullmatch(r"[0-9a-f]{64}", result):
            raise ValueError("BLAKE3 executable returned an invalid identity")
        return result

    def spawn(self, name, argv, *, temporary=None):
        argv = owned_command(self.exe, argv, temporary)
        log = self.root / (name + ".log")
        output = log.open("xb")
        try:
            process = subprocess.Popen(list(map(str, argv)), cwd=REPO, env=self.env,
                                       stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.STDOUT,
                                       start_new_session=True)
        finally:
            output.close()
        item = {"name": name, "argv": list(map(str, argv)), "process": process, "log": log}
        self.services.append(item)
        return item

    def stop(self, item, *, abrupt=False):
        process = item["process"]
        if process.poll() is None:
            try:
                os.killpg(process.pid, signal.SIGKILL if abrupt else signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=5)

    def until(self, description, predicate, *, interval=POLL_SECONDS):
        deadline = time.monotonic() + RECOVERY_SECONDS
        while time.monotonic() < deadline:
            for item in self.services:
                if item["log"].stat().st_size > SERVICE_LOG_BYTES:
                    raise ValueError(f"service log exceeds bound: {item['name']}")
            result = predicate()
            if result:
                return result
            time.sleep(interval)
        raise TimeoutError(description)

    def pilot(self, *arguments):
        return self.json_command([self.exe["coordinator"], *arguments])

    def consumer_observations(self, plan):
        observations = {}
        for name, path in sorted(plan["consumer_outputs"].items()):
            native = self.run([self.exe["nix"], "path-info", "--json", "--offline", path], allowed=(0, 1))
            observations[name] = {"path": path, "native": native,
                **consumer_output_status(path, json.loads(native["stdout"]),
                    Path(path).exists() or Path(path).is_symlink())}
        return observations

    # r[impl kiln.onixos_pilot.closure.construction]
    def inspect_consumer(self, plan, result):
        if self.args.case == "success":
            outputs = plan["consumer_outputs"]
            closure = self.json_command([self.exe["nix"], "path-info", "--json", "--recursive", outputs["home-activation"]])
            if not set(outputs.values()).issubset(closure):
                raise ValueError("successful launcher does not retain the exact generated consumer closure")
            with (Path(outputs["home-files"]) / ".gitconfig").open("rb") as source:
                content = source.read(DOCUMENT_BYTES + 1)
            identity = load(self.root / "cohort.json")["consumer_build_identity"]
            expected = f"[user]\n  name = Alice Example (Kiln CI {identity})\n".encode()
            if content != expected:
                raise ValueError("generated Git configuration differs from the bound consumer input")
            return {"consumer_artifacts": {name: closure[path] for name, path in outputs.items()},
                    "launcher_closure": closure, "gitconfig": {"text": content.decode(),
                        "blake3": self.digest(path=Path(outputs["home-files"]) / ".gitconfig")}}
        after = self.consumer_observations(plan)
        admit_preparation_closure(plan, [item["path"] for item in after.values() if item["registered"]])
        diagnostics = {}
        if self.args.case == "build-failure":
            actual = bytearray()
            for identity in result["build_observation"]["log_identities"]:
                path = self.root / "worker" / (identity + ".log")
                with path.open("rb") as source:
                    raw = source.read(DOCUMENT_BYTES + 1)
                if len(raw) > DOCUMENT_BYTES:
                    raise ValueError("consumer build diagnostic exceeds its bound")
                actual.extend(raw)
                diagnostics[path.name] = self.digest(path=path)
            if (Path(plan["consumer_derivations"]["home-files"]).name.encode() not in actual
                    or b".gitconfig" not in actual
                    or not any(message in actual for message in (b"File exists", b"Not a directory"))):
                raise ValueError("negative build lacks the real home-file target-conflict diagnostic")
        return {"consumer_outputs_after_failure": after, "build_diagnostic_identities": diagnostics}

    def prepare(self):
        inputs = self.export("admitted-inputs", "(import " + json.dumps(str(REPO / "config/onixos-ci-pilot-inputs.ncl"))
                             + ") (import " + json.dumps(str(Path(self.args.inputs).resolve(strict=True))) + ")")
        for key, value in inputs["executables"].items():
            Path(value).resolve(strict=True)
            # Multicall tools such as nix-store select behavior from argv[0].
            self.exe[key] = str(Path(value).absolute())
        consumer_lock = load(Path(inputs["consumer"]["path"]) / "flake.lock")
        nodes = consumer_lock["nodes"]
        locked_inputs = nodes[consumer_lock["root"]]["inputs"]
        locked_nixpkgs = nodes[locked_inputs["nixpkgs"]]["locked"]
        locked_wasm = nodes[locked_inputs["onix-wasm"]]["locked"]
        if locked_nixpkgs["type"] != "github":
            raise ValueError("pilot requires the consumer's exact GitHub nixpkgs lock")
        expected_nixpkgs = "github:{owner}/{repo}/{rev}?narHash=".format(**locked_nixpkgs)
        expected_nixpkgs += quote(locked_nixpkgs["narHash"], safe="")
        if inputs["nixpkgs_url"] != expected_nixpkgs:
            raise ValueError("selected nixpkgs differs from the consumer's root flake.lock")
        put(self.root / "inputs.json", inputs)
        (self.root / "lattice-tmp").mkdir(mode=0o700)
        executable_facts = {name: {"path": path, "blake3": self.digest(path=path)}
                            for name, path in self.exe.items() if Path(path).is_file()}
        jobset = self.root / "jobset"
        jobset.mkdir(mode=0o700)
        put(jobset / "jobset.nix", (REPO / "runtime/onixos-ci-pilot/jobset.nix").read_bytes())
        projection_identity = self.digest(path=jobset / "jobset.nix")
        build_identity = self.digest(value={"schema": "kiln.onixos-pilot.consumer-build.v1",
            "root": str(self.root), "inputs": inputs, "projection_blake3": projection_identity})
        trees = {name: inputs[name] for name in ("consumer", "wasmSource", "plugin")}
        flake = '{ inputs.nixpkgs.url = ' + json.dumps(inputs["nixpkgs_url"]) + ';\n'
        flake += 'outputs = { nixpkgs, ... }: import ./jobset.nix { inherit nixpkgs; consumerInputs = '
        # Nickel and Nix have different record syntax; all these values are strings.
        flake += "{ " + " ".join(name + " = { " + " ".join(key + " = " + json.dumps(value) + ";" for key, value in tree.items())
                                                   + " };" for name, tree in trees.items()) + " }; buildIdentity = " + json.dumps(build_identity) + "; }; }\n"
        put(jobset / "flake.nix", flake.encode())
        self.run([self.exe["nix"], "flake", "lock", "--offline", str(jobset)])
        source = self.run([self.exe["nix"], "store", "add-path", jobset, "--name", "kiln-onixos-application-jobset"])["stdout"].strip()
        source_identity = self.digest(path=Path(source) / "flake.nix")
        source_roots = [source, *[tree["path"] for tree in trees.values()]]
        source_roots.append(inputs["executables"]["lattice_source"])
        for path in self.exe.values():
            parts = Path(path).parts
            if len(parts) > 3 and parts[:3] == ("/", "nix", "store"):
                source_roots.append(str(Path(*parts[:4])))
        immutable_kiln_source = REPO.parent == Path("/nix/store")
        if immutable_kiln_source:
            source_roots.append(str(REPO))
        tree_facts = self.json_command([self.exe["nix"], "path-info", "--json", *sorted(set(source_roots))])
        cohort = {"schema": "kiln.onixos-pilot.cohort.v1", "executables": executable_facts,
                  "kiln_source_root": str(REPO), "immutable_kiln_source": immutable_kiln_source,
                  "consumer_revision": inputs["consumer_revision"], "sources": tree_facts,
                  "consumer_locked_inputs": {"nixpkgs": locked_nixpkgs, "onix-wasm": locked_wasm},
                  "explicit_producer_substitution": {"wasmSource": inputs["wasmSource"], "plugin": inputs["plugin"],
                      "reason": inputs["producer_substitution_reason"]},
                  "jobset_source": source, "flake_blake3": source_identity,
                  "projection_blake3": projection_identity, "consumer_build_identity": build_identity,
                  "preparation_core_blake3": self.digest(path=REPO / "scripts/onixos_ci_closure.py"),
                  "policy_blake3": self.digest(path=REPO / "config/onixos-ci-pilot.ncl"),
                  "runner_blake3": self.digest(path=Path(__file__)), "case": self.args.case}
        put(self.root / "cohort.json", cohort)
        operation = self.digest(value=cohort)
        policy_inputs = {"root": str(self.root), "source_path": source, "source_identity": source_identity,
                         "case": self.args.case, "operation_identity": operation,
                         "worker_identity": executable_facts["worker"]["blake3"],
                         "store_identity": executable_facts["nix_store"]["blake3"],
                         "plan_identity": operation, "policy_identity": cohort["policy_blake3"], "user_id": os.getuid()}
        policy = self.export("pilot-policy", "(import " + json.dumps(str(REPO / "config/onixos-ci-pilot.ncl")) + ") " + ncl(policy_inputs))
        put(self.root / "eval-selection.json", policy["eval_selection"])
        evaluation = self.json_command([self.exe["worker"], "eval-profile", self.root / "eval-selection.json"])
        put(self.root / "eval-profile.json", evaluation)
        # r[impl kiln.onixos_pilot.closure.preparation]
        # Observe the real consumer boundary, then prepare only external tools.
        prepared = self.run([evaluation["runtime"]["executable_path"], "--option", "experimental-features", "nix-command flakes wasm-builtin",
                             "--option", "allow-import-from-derivation", "false", "--option", "substituters", "",
                             "--workers", "1", "--max-memory-size", "1024", "--show-input-drvs", "--flake", source + "#preparation." + self.args.case])
        native_jobs = [json.loads(line) for line in prepared["stdout"].splitlines() if line.strip()]
        plan = preparation_plan(native_jobs)
        dependencies = plan["dependency_derivations"]
        requisites = self.run([self.exe["nix_store"], "--query", "--requisites", *dependencies])["stdout"].splitlines()
        external_derivation_closure = admit_preparation_closure(plan, requisites)
        realised = self.run([self.exe["nix_store"], "--realise", "--option", "builders", "", *dependencies])
        realised_outputs = realised["stdout"].splitlines()
        closure = self.json_command([self.exe["nix"], "path-info", "--json", "--recursive", *realised_outputs])
        admit_preparation_closure(plan, closure)
        before = self.consumer_observations(plan)
        admit_preparation_closure(plan, [item["path"] for item in before.values() if item["present"]])
        put(self.root / "provisioning.json", {"native_jobs": native_jobs, "preparation_plan": plan,
            "external_derivation_closure": external_derivation_closure,
            "realised_outputs": realised_outputs, "realised_closure": closure,
            "consumer_outputs_before_authentication": before,
            "scope": "Only external tools are prepared; authenticated CI must construct the real launcher, activation script and generated home files"})
        build = policy["build_profile"]
        intent = {"effect_id": "pilot-build-" + operation, "attempt_id": "pilot-attempt-" + operation,
                  "idempotency_key": operation, "candidate_id": "pilot-candidate-" + operation,
                  "pipeline_revision": "pilot-pipeline-" + operation, "plan_identity": operation,
                  "policy_identity": cohort["policy_blake3"], "platform": "x86_64-linux"}
        discovery = {"effect_id": "pilot-discovery-" + operation, "evaluation_id": "pilot-evaluation-" + operation,
                     "project_id": "onixos-home-application", "pipeline_revision_id": intent["pipeline_revision"],
                     "discovery_policy_id": "onixos-explicit-cohort-v1", "provider_profile": evaluation["provider"]["profile_id"],
                     "evaluator_cohort": evaluation["evaluator_cohort"], "build_provider_profile": build["provider_profile_id"],
                     "idempotency_key": operation, "source_identity": source_identity,
                     "source": policy["eval_selection"]["source_binding"]["source"], "source_mode": "Flake",
                     "root_selector": "cases." + self.args.case, "system": "x86_64-linux",
                     "options": {"workers": 1, "max_memory_mebibytes": 1024,
                                 **{key: evaluation["policy"][key] for key in ("include_metadata", "include_input_derivations", "check_cache_status", "partial_admission")}},
                     "bounds": {key: evaluation["limits"][key] for key in ("max_records", "max_jobs", "max_job_errors", "max_diagnostic_bytes",
                                                                                      "max_outputs_per_job", "max_dependencies_per_job", "max_tasks")}}
        source_uri = "path:" + source + "#cases." + self.args.case
        expected_source = {"provider": "radicle", "repository": inputs["source_repository"], "revision": inputs["source_revision"]}
        job = {"schema": "kiln.onixos-pilot.worker-job.v1", "source_uri": source_uri, "expected_source": expected_source,
               "discovery_profile": evaluation, "discovery_plan": discovery, "selected_task": "home-activation",
               "build_profile": build, "build_intent": intent, "state_root": str(self.root / "worker")}
        put(self.root / "job.json", job)
        job_identity = self.digest(path=self.root / "job.json")
        build_plan = {**intent, "provider_profile": build["provider_profile_id"], "required_products": ["out"]}
        put(self.root / "aspen.json", policy["aspen_document"])
        put(self.root / "radicle.json", policy["radicle_profile"])
        runtime = self.pilot("runtime-profile", self.root / "aspen.json")
        public = self.root / "operator.public.json"
        secret = self.root / "operator.secret.json"
        self.private_files.append(secret)
        self.run([self.exe["coordinator"], "keygen", secret, public])
        profile = {"schema": "kiln.onixos-pilot.profile.v1", "allowed_operator_keys": [load(public)], "cohort_identity": operation,
                   "source_uri": source_uri, "source_identity": source_identity, "job_identity": job_identity,
                   "selected_task": "home-activation", "max_request_age_seconds": REQUEST_SECONDS,
                   "max_request_bytes": DOCUMENT_BYTES, "max_journal_bytes": DOCUMENT_BYTES, "max_journal_records": 64,
                   "socket_path": str(self.root / "aspen.sock"), "transport_timeout_seconds": COMMAND_SECONDS,
                   "maximum_frame_bytes": DOCUMENT_BYTES, "runtime_profile": runtime,
                   "discovery_plan": discovery, "build_plan": build_plan}
        put(self.root / "profile.json", profile)
        trigger = {"repository": {"contract_version": 1, "provider_namespace": "radicle", "native_kind": "repository", "native_id": inputs["source_repository"]},
                   "actor": {"contract_version": 1, "provider_namespace": "radicle", "native_kind": "did", "native_id": inputs["source_actor"]},
                   "before": None, "after": {"contract_version": 1, "provider_namespace": "radicle", "native_kind": "revision", "native_id": inputs["source_revision"]},
                   "branch": None, "patch": None, "event": "push"}
        put(self.root / "trigger.json", trigger)
        native = self.pilot("native-workflow", self.root / "radicle.json", self.root / "trigger.json")
        put(self.root / "native-request.json", native)
        return inputs, policy, profile, job_identity, native

    def workflow(self, inputs, policy, job_identity, native, worker_arguments=None):
        name = "onixos-home-application"
        invocation = self.root / "provider-invocations.log"
        command = "set -eu; printf '%s\\n' \"$LATTICE_SOURCE_REVISION\" >> " + shlex.quote(str(invocation)) + "; "
        command += shlex.join([self.exe["sleep"], str(WORKER_START_DELAY_SECONDS)]) + "; exec "
        arguments = worker_arguments if worker_arguments is not None else ["run", str(self.root / "job.json"), job_identity]
        command += shlex.join([self.exe["worker"], *map(str, arguments)])
        command += " > " + shlex.quote(str(self.root / "worker.stdout.log"))
        command += " 2> " + shlex.quote(str(self.root / "worker.stderr.log"))
        workflow = {"name": name, "command": command, "timeout_seconds": policy["workflow_timeout_seconds"], "created": "2026-10-20T00:00:00Z"}
        workflow_path = self.root / "workflow.ncl"
        put(self.root / "workflow-builder.ncl", (REPO / "config/live-ci-workflow.ncl").read_bytes())
        put(self.root / "workflow-inputs.json", workflow)
        put(workflow_path, b'((import "workflow-builder.ncl") (import "workflow-inputs.json")) & { meta.description | force = "Operator-authorized OnixOS application discovery and build" }\n')
        config = {"data_dir": str(self.root / "lattice-store"), "shell": self.exe["shell"],
                  "default_timeout_secs": policy["workflow_timeout_seconds"], "max_parallelism": 1}
        put(self.root / "lattice.ncl", ncl(config).encode())
        prefix = lattice_command(self.exe, self.root, "--config", str(self.root / "lattice.ncl"))
        self.run([*prefix, "import", workflow_path])
        self.run([*prefix, "export", name, self.root / "workflow-export.ncl"])
        observed_path = self.root / "workflow-observe.ncl"
        expression = "let base = import " + json.dumps(inputs["executables"]["lattice_source"] + "/examples/orchestrations/llm-review-loop.ncl") + " in base & { objective | force = \"Read-only workflow identity observation\", workflow | force = "
        expression += ncl({"workflow_name": name, "content_hash": job_identity}) + ", triggers | force = [] }\n"
        put(observed_path, expression.encode())
        observed = self.json_command([*prefix, "orchestration", "validate", observed_path, "--output", "json"], allowed=(0, 1))
        if not observed.get("workflow_revision") or observed.get("workflow_diagnostics"):
            raise ValueError("producer did not observe a valid exact workflow")
        revision = observed["workflow_revision"]["content_hash"]
        # Validate once against the producer's own observed revision, never a duplicated hash algorithm.
        put(self.root / "workflow-validated.ncl", expression.replace(job_identity, revision, 1).encode())
        observed = self.json_command([*prefix, "orchestration", "validate", self.root / "workflow-validated.ncl", "--output", "json"])
        put(self.root / "workflow-observed.json", observed)
        route = {"profile_identity": native["profile_identity"], "graph_identity": native["graph"]["identity"],
                 "workflow_name": name, "workflow_revision": "b3:" + revision,
                 "effect_authority": "route_exact", "source_binding": "shell_environment",
                 "nodes": [{"request_node_id": native["graph"]["nodes"][0]["id"], "operation_identity": native["graph"]["nodes"][0]["operation_identity"], "workflow_node_id": 0}]}
        handler = {"schema": "lattice.workflow-exchange-handler-profile.v1", "socket_path": str(self.root / "lattice.sock"),
                   "transport": {"schema": "lattice.workflow-transport-profile.v1", "limits": policy["radicle_profile"]["transport_limits"],
                                 "peers": {"allowed_user_ids": [os.getuid()]}, "non_claims": [
                                     "transport admission does not prove workflow correctness", "transport peer identity does not prove peer intent",
                                     "transport acceptance does not prove an operation executed", "transport response does not prove an external effect succeeded",
                                     "transport checks do not establish production deployment"]}, "maximum_active_executions": 8,
                   "routes": [route], "non_claims": ["handler admission does not prove workflow correctness", "route authority does not prove host confinement",
                   "execution observation does not prove an external effect succeeded", "peer identity does not prove peer intent", "handler checks do not establish release readiness"]}
        checked = self.export("handler-checked", "let Contract = import " + json.dumps(inputs["executables"]["lattice_source"] + "/contracts/workflow-exchange-handler-profile.ncl") + " in (" + ncl(handler) + ") | Contract.HandlerProfile")
        put(self.root / "handler.json", checked)
        return self.spawn("lattice", [self.exe["lattice"], "--config", self.root / "lattice.ncl",
            "workflow-exchange", "serve", "--profile", self.root / "handler-checked.ncl"],
            temporary=self.root / "lattice-tmp")

    # r[verify kiln.onixos_pilot.runtime] r[verify kiln.onixos_pilot.journal]
    # r[verify kiln.onixos_pilot.failure] r[verify kiln.onixos_pilot.evidence]
    # r[impl kiln.onixos_pilot.closure.evidence] r[verify kiln.onixos_pilot.closure.evidence]
    def execute(self):
        inputs, policy, profile, job_identity, native = self.prepare()
        profile_path = self.root / "profile.json"
        journal = self.root / "journal.json"
        coordinator = self.exe["coordinator"]
        now = int(self.run([coordinator, "now"])["stdout"])
        self.run([coordinator, "prepare-request", profile_path, self.root / "trigger.json", now, now + REQUEST_SECONDS, self.root / "request.json"])
        self.run([coordinator, "sign", self.root / "operator.secret.json", self.root / "request.json", self.root / "signed.json"])
        signature = load(self.root / "signed.json")
        tampered = copy.deepcopy(signature)
        tampered["request"]["source_uri"] += "-substitution"
        put(self.root / "tampered.json", tampered)
        self.run([coordinator, "start", profile_path, self.root / "tampered.json", self.root / "denied.json"], allowed=(1,))
        self.run([coordinator, "prepare-request", profile_path, self.root / "trigger.json", now - REQUEST_SECONDS - 1, now - 1, self.root / "stale-request.json"])
        self.run([coordinator, "sign", self.root / "operator.secret.json", self.root / "stale-request.json", self.root / "stale-signed.json"])
        self.run([coordinator, "start", profile_path, self.root / "stale-signed.json", self.root / "denied.json"], allowed=(1,))
        if (self.root / "denied.json").exists() or (self.root / "worker").exists():
            raise ValueError("denied authentication created journal or worker state")
        lattice = self.workflow(inputs, policy, job_identity, native)
        self.until("Lattice socket", lambda: (self.root / "lattice.sock").is_socket() or self.failed(lattice))
        host_state = self.root / "host-state"
        host_state.mkdir(mode=0o700)
        options = policy["host_options"]
        argv = [self.exe["host"], "--aspen-profile", self.root / "aspen.json", "--radicle-profile", self.root / "radicle.json",
                "--socket", self.root / "aspen.sock", "--extension", self.exe["extension"], "--state-root", host_state,
                "--max-requests", options["maximum_requests"], "--timeout-ms", options["timeout_milliseconds"], "--poll-interval-ms", options["poll_interval_milliseconds"]]
        host = self.spawn("host-first", argv)
        self.until("Aspen socket", lambda: (self.root / "aspen.sock").is_socket() or self.failed(host))
        before_dispatch = self.consumer_observations(load(self.root / "provisioning.json")["preparation_plan"])
        put(self.root / "before-dispatch.json", before_dispatch)
        admit_preparation_closure(load(self.root / "provisioning.json")["preparation_plan"],
            [item["path"] for item in before_dispatch.values() if item["present"]])
        submit = self.spawn("submit", [coordinator, "start", profile_path, self.root / "signed.json", journal])
        self.until("real provider invocation", lambda: (self.root / "provider-invocations.log").exists() or self.failed(submit))
        self.stop(host, abrupt=True)
        submit["process"].wait(timeout=15)
        if submit["process"].returncode:
            raise ValueError("interrupted submit did not retain a replayable uncertain intent")
        pending_journal = load(journal)
        pending_run = pending_journal["request"]["request"]["run_id"]
        pending = self.pilot("replay", profile_path, journal)
        pending_outcome = pending["runs"][pending_run]["outcome"]
        if pending_outcome != "Pending":
            raise ValueError("interrupted intent was not non-terminal during replay")
        duplicate = self.pilot("start", profile_path, self.root / "signed.json", journal)
        original = load(journal)["ingress"]["operation_id"]
        if duplicate["ingress"]["operation_id"] != original:
            raise ValueError("redelivery changed the original operation")
        socket_path = self.root / "aspen.sock"
        if socket_path.is_socket():
            socket_path.unlink()
        result_path = self.root / "worker/worker-result.json"
        restart_observation = {"worker_result_present": result_path.exists(),
            "lattice_process_running": lattice["process"].poll() is None,
            "pending_canonical_outcome": pending_outcome}
        if restart_observation["worker_result_present"] or not restart_observation["lattice_process_running"]:
            raise ValueError("provider was not still executing before the host restart")
        host = self.spawn("host-restarted", argv)
        self.until("recovered Aspen socket", lambda: socket_path.is_socket() or self.failed(host))
        def worker_finished():
            if result_path.exists():
                return True
            errors = self.root / "worker.stderr.log"
            if errors.exists() and errors.stat().st_size:
                with errors.open("rb") as source:
                    diagnostic = source.read(DOCUMENT_BYTES).decode(errors="replace")
                raise RuntimeError("real worker failed before its durable result: " + diagnostic)
            return self.failed(lattice)
        self.until("durable worker result", worker_finished)
        result = load(result_path)
        if self.args.case == "evaluation-failure":
            diagnostic = self.root / "worker/evaluator.stderr.log"
            with diagnostic.open("rb") as source:
                actual = source.read(DOCUMENT_BYTES + 1)
            if (len(actual) > DOCUMENT_BYTES or result["build_plan"] is not None
                    or result["discovery_observation"]["terminal"] != "ProviderError"
                    or b"kiln-onixos-pilot: explicit evaluation failure control" not in actual):
                raise ValueError("negative evaluation lacks its genuine control diagnostic")
        def completed():
            current = self.pilot("reconcile", profile_path, journal, result_path)
            return current if len(current["records"]) > 1 else None
        interval = RECOVERY_SECONDS / (profile["max_journal_records"] - RECONCILE_OBSERVATION_RESERVE)
        completed_journal = self.until("canonical terminal admission", completed, interval=interval)
        with (self.root / "worker.stderr.log").open("rb") as source:
            terminal_error = source.read(DOCUMENT_BYTES + 1)
        if terminal_error:
            raise RuntimeError("worker retained a terminal error after its durable result: " + terminal_error.decode(errors="replace"))
        if result["discovery_observation"]["terminal"] == "Complete":
            released = load(self.root / "worker/discovery-root-release.json")
            if released["gc_roots"]["state"] != "Released":
                raise ValueError("complete discovery did not discharge its authorized root handoff")
        state = self.pilot("replay", profile_path, journal)
        run_id = completed_journal["request"]["request"]["run_id"]
        outcome = state["runs"][run_id]["outcome"]
        expected_outcome = {"success": "Success", "build-failure": "DomainFailure",
                            "evaluation-failure": "ProviderError"}[self.args.case]
        if outcome != expected_outcome:
            raise ValueError(f"canonical outcome conflicts with the selected real case: {outcome}; expected {expected_outcome}")
        terminal_duplicate = self.pilot("start", profile_path, self.root / "signed.json", journal)
        if terminal_duplicate["identity"] != completed_journal["identity"]:
            raise ValueError("terminal redelivery mutated canonical evidence")
        invocations = (self.root / "provider-invocations.log").read_text().splitlines()
        if invocations != [inputs["source_revision"]]:
            raise ValueError("restart or redelivery executed the provider more than once")
        root_directory = self.root / "build-roots"
        roots = {str(path): str(path.resolve(strict=True)) for path in root_directory.rglob("*") if path.is_symlink()}
        if self.args.case == "success":
            result = load(result_path)
            task = next(item for item in result["discovery_observation"]["tasks"] if item["native_attribute"]["native_id"] == "home-activation")
            expected = sorted(item["path"]["native_id"] for item in task["outputs"])
            if sorted(roots.values()) != expected:
                raise ValueError("durable successful products are not exactly rooted")
        construction = self.inspect_consumer(load(self.root / "provisioning.json")["preparation_plan"], result)
        proof = self.observe(profile_path, journal)
        release = []
        if load(result_path)["build_plan"] is not None:
            release = [self.json_command([self.exe["worker"], "release", self.root / "job.json", job_identity]) for _ in range(2)]
            if any(path.is_symlink() for path in root_directory.rglob("*")):
                raise ValueError("explicit durable handoff retained output roots")
        report = {"schema": "kiln.onixos-pilot.verification.v1", "case": self.args.case,
                  "cohort_identity": profile["cohort_identity"], "source_identity": profile["source_identity"],
                  "job_identity": job_identity, "operation_id": original, "run_id": run_id,
                  "canonical_outcome": outcome, "journal_identity": completed_journal["identity"],
                  "restarted_during_provider_execution": not restart_observation["worker_result_present"] and restart_observation["lattice_process_running"],
                  "restart_observation": restart_observation, "pending_replay": pending,
                  "provisioning": load(self.root / "provisioning.json"),
                  "consumer_outputs_before_dispatch": before_dispatch, "consumer_construction": construction,
                  "provider_invocations": invocations, "rooted_products_before_release": roots,
                  "explicit_idempotent_release": release, "ucan_observation": proof,
                  "maintenance_owner": "Kiln maintainers", "non_claims": REQUIRED_NON_CLAIMS}
        put(self.root / "verification.json", report)
        print(json.dumps({key: report[key] for key in ("case", "canonical_outcome", "operation_id", "journal_identity")}, indent=2))

    def failed(self, item):
        if item["process"].poll() is not None:
            raise RuntimeError(f"service exited before readiness: {item['name']} ({item['log']})")
        return False

    # r[verify kiln.onixos_pilot.observation]
    def observe(self, profile, journal):
        with socket.socket() as candidate:
            candidate.bind(("127.0.0.1", 0))
            port = candidate.getsockname()[1]
        template = self.export("http-template", "(import " + json.dumps(str(REPO / "config/fixtures/ucan-observation-positive.ncl")) + ").profile")
        template["origin"] = {"scheme": "https", "host": "127.0.0.1", "port": port}
        key = self.root / "tls.key"
        certificate = self.root / "tls.pem"
        self.private_files.extend([key, self.root / "ucan/credentials.json"])
        self.run([self.exe["openssl"], "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", key, "-out", certificate,
                  "-days", "1", "-subj", "/CN=localhost", "-addext", "subjectAltName=DNS:localhost,IP:127.0.0.1",
                  "-addext", "basicConstraints=critical,CA:FALSE", "-addext", "keyUsage=digitalSignature,keyEncipherment", "-addext", "extendedKeyUsage=serverAuth"])
        template["tls"].update({"certificate_path": str(certificate), "private_key_path": str(key)})
        put(self.root / "http-template.json", template)
        self.run([self.exe["coordinator"], "ucan-init", self.root / "http-template.json", profile, journal, self.root / "ucan"])
        server = self.spawn("observation", [self.exe["coordinator"], "serve", profile, journal, self.root / "ucan/http.json"])
        def listening():
            self.failed(server)
            try:
                with socket.create_connection(("127.0.0.1", port), timeout=1):
                    return True
            except ConnectionRefusedError:
                return False
        self.until("TLS observation listener", listening)
        report = self.pilot("ucan-probe", profile, journal, self.root / "ucan")
        put(self.root / "ucan-observation.json", report)
        return report

    def close(self):
        errors = []
        for item in reversed(self.services):
            try:
                self.stop(item)
            except Exception as error:
                errors.append(f"{item['name']}: {error}")
        for path in self.private_files:
            try:
                path.unlink(missing_ok=True)
            except OSError as error:
                errors.append(f"credential retirement: {error}")
        put(self.root / "retirement.json", {"services": [{"name": item["name"], "argv": item["argv"],
            "exit_code": item["process"].returncode} for item in self.services],
            "private_credentials_retired": all(not path.exists() for path in self.private_files),
            "production_state_or_status_mutated": False, "errors": errors})
        if errors:
            raise RuntimeError("private pilot teardown incomplete: " + "; ".join(errors))


def interrupted(signum, _frame):
    raise InterruptedError(f"runner interrupted by signal {signum}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inputs", required=True, help="Explicit operator Nickel input record")
    parser.add_argument("--nickel", required=True, help="Exact bootstrap Nickel executable")
    parser.add_argument("--root", required=True, help="New private scratch directory; replacement refused")
    parser.add_argument("--case", choices=("success", "build-failure", "evaluation-failure"), required=True)
    args = parser.parse_args()
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGHUP, interrupted)
    os.umask(0o077)
    exercise = Exercise(args)
    try:
        exercise.execute()
    except BaseException as error:
        put(exercise.root / "failure.json", {"kind": type(error).__name__, "diagnostic": str(error)})
        raise
    finally:
        exercise.close()


if __name__ == "__main__":
    main()
