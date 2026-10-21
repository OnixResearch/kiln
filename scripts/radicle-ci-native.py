#!/usr/bin/env python3
"""Repeat the isolated real Radicle-producer/source/worker/status CI rail.

Only fixture repositories/keys are created. No production node, publication,
activation or release authority is exercised. Existing bounded process/TLS
helpers are shared with the maintained OnixOS pilot, not copied.
"""
import argparse
import json
import os
from pathlib import Path
import runpy
import shlex
import signal
import sys

REPO = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO / "scripts"))
PILOT = runpy.run_path(str(REPO / "scripts/onixos-ci-pilot.py"))
BaseExercise, put, load, ncl = (PILOT[name] for name in ("Exercise", "put", "load", "ncl"))
FIXTURE_CALLBACK_MILLISECONDS = 240000
FIXTURE_OBSERVATION_POLLS = 120
FIXTURE_OBSERVATION_INTERVAL_MILLISECONDS = 1000
FIXTURE_HOST_REQUESTS = 256
FIXTURE_BUILD_DELAY_SECONDS = 3


class Exercise(BaseExercise):
    def ci(self, *arguments):
        return self.json_command([self.exe["coordinator"], *arguments])

    def executable(self, name, arguments):
        path = self.root / name
        put(path, ("#!" + self.exe["shell"] + "\nexec " + shlex.join(map(str, arguments)) + ' "$@"\n').encode())
        path.chmod(0o700)
        return str(path)

    def source_files(self, value):
        source = self.root / "project-source"
        if source.exists():
            (source / "value.txt").write_text(value + "\n")
            return source
        source.mkdir(mode=0o700)
        put(source / "value.txt", (value + "\n").encode())
        shell = '"${pkgs.bash}/bin/bash"'
        sleep = '${pkgs.coreutils}/bin/sleep'
        build = (sleep + " " + str(FIXTURE_BUILD_DELAY_SECONDS) + "; read value < ${./value.txt}; "
                 'if [ "$value" = fail ]; then echo source-requested-genuine-build-failure >&2; exit 19; fi; '
                 'printf \'%s\\n\' "$value" > "$out"')
        expression = '{ inputs.nixpkgs = { flake = false; url = ' + json.dumps("path:" + str(Path(self.args.nixpkgs).resolve(strict=True))) + '; }; '
        expression += 'outputs = { self, nixpkgs }: let pkgs = import nixpkgs { system = "x86_64-linux"; }; in { jobs.app = builtins.derivation { name = "kiln-radicle-source-ci"; system = "x86_64-linux"; builder = '
        expression += shell + '; args = [ "-c" ' + json.dumps(build) + ' ]; }; }; }\n'
        put(source / "flake.nix", expression.encode())
        self.run([self.exe["nix"], "flake", "lock", "--offline", "--extra-experimental-features", "nix-command flakes", "path:" + str(source)])
        return source

    def prepare(self):
        inputs = self.export("admitted-inputs", "(import " + json.dumps(str(REPO / "config/onixos-ci-pilot-inputs.ncl"))
                             + ") (import " + json.dumps(str(Path(self.args.inputs).resolve(strict=True))) + ")")
        for key, value in inputs["executables"].items():
            Path(value).resolve(strict=True)
            self.exe[key] = str(Path(value).absolute())
        self.exe["legacy_coordinator"] = self.exe["coordinator"]
        self.exe["real_worker"] = str(Path(self.args.worker).resolve(strict=True))
        self.exe["extension"] = str(Path(self.args.extension).resolve(strict=True))
        self.exe["raw_coordinator"] = str(Path(self.args.coordinator).resolve(strict=True))
        self.exe["fixture"] = str(Path(self.args.source_fixture).resolve(strict=True))
        self.exe["broker_tool"] = str(Path(self.args.broker_tool).resolve(strict=True))
        self.exe["radicle"] = str(Path(self.args.radicle).resolve(strict=True))
        self.exe["secretspec_fixture"] = str(Path(self.args.secretspec_fixture).resolve(strict=True))
        self.exe["coordinator"] = self.executable("coordinator-fixture", [self.exe["raw_coordinator"], "--native-fixtures"])
        (self.root / "lattice-tmp").mkdir(mode=0o700)
        (self.root / "rad-home").mkdir(mode=0o700)
        (self.root / "candidates").mkdir(mode=0o700)
        self.env["RAD_HOME"] = str(self.root / "rad-home")
        source = self.source_files("exact-event-source-" + self.root.name)
        bootstrap = self.json_command([self.exe["fixture"], "init", self.root / "source-fixture", source])
        self.run([self.exe["radicle"], "auth", "--alias", "kiln-isolated-ci-fixture", "--stdin"], data=b"\n")
        self.run([self.exe["radicle"], "init", "--no-confirm", "--no-seed", "--private", "--name", bootstrap["project_name"],
                  "--description", "Isolated Kiln native CI proof", "--default-branch", "main", bootstrap["worktree"]])
        rid = self.run([self.exe["radicle"], "inspect", "--rid", bootstrap["worktree"]])["stdout"].strip()
        native = self.json_command([self.exe["fixture"], "advance", self.root / "source-fixture", source, self.root / "rad-home", rid])
        facts = {name: {"path": path, "blake3": self.digest(path=path)} for name, path in self.exe.items() if Path(path).is_file()}
        operation = self.digest(value={"root": str(self.root), "executables": facts, "source_policy": native["selection"]})
        template_source = "/nix/store/00000000000000000000000000000000-project-template"
        policy_inputs = {"root": str(self.root), "source_path": template_source, "source_identity": operation, "case": "success",
                         "operation_identity": operation, "worker_identity": facts["real_worker"]["blake3"],
                         "store_identity": facts["nix_store"]["blake3"], "plan_identity": operation, "policy_identity": operation, "user_id": os.getuid()}
        policy = self.export("runtime-policy", "((import " + json.dumps(str(REPO / "config/onixos-ci-pilot.ncl")) + ") " + ncl(policy_inputs)
                             + ') & { eval_selection.source_binding.root_selector | force = "jobs" }')
        put(self.root / "eval-selection.json", policy["eval_selection"])
        evaluation = self.json_command([self.exe["real_worker"], "eval-profile", self.root / "eval-selection.json"])
        build = policy["build_profile"]
        intent = {"effect_id": "project-template", "attempt_id": "project-template", "idempotency_key": operation,
                  "candidate_id": "project-template", "pipeline_revision": "radicle-source-ci", "plan_identity": operation, "policy_identity": operation, "platform": "x86_64-linux"}
        discovery = {"effect_id": "project-template", "evaluation_id": "project-template", "project_id": "radicle-source-ci", "pipeline_revision_id": intent["pipeline_revision"],
                     "discovery_policy_id": "onix-explicit-project-v1", "provider_profile": evaluation["provider"]["profile_id"], "evaluator_cohort": evaluation["evaluator_cohort"],
                     "build_provider_profile": build["provider_profile_id"], "idempotency_key": operation, "source_identity": operation,
                     "source": policy["eval_selection"]["source_binding"]["source"], "source_mode": "Flake", "root_selector": "jobs", "system": "x86_64-linux",
                     "options": {"workers": 1, "max_memory_mebibytes": 1024, **{key: evaluation["policy"][key] for key in ("include_metadata", "include_input_derivations", "check_cache_status", "partial_admission")}},
                     "bounds": {key: evaluation["limits"][key] for key in ("max_records", "max_jobs", "max_job_errors", "max_diagnostic_bytes", "max_outputs_per_job", "max_dependencies_per_job", "max_tasks")}}
        worker_inputs = {"source": native["selection"], "state_root": str(self.root / "candidates"), "discovery_profile": evaluation, "discovery_plan": discovery,
                         "selected_task": "app", "build_profile": build, "build_intent": intent}
        project = self.export("project-profile", "(import " + json.dumps(str(REPO / "config/radicle-ci-worker.ncl")) + ") " + ncl(worker_inputs))
        put(self.root / "project.json", project)
        project_identity = self.digest(path=self.root / "project.json")
        controller_inputs = {"worker_profile_path": str(self.root / "project.json"), "worker_profile_identity": project_identity, "socket_path": str(self.root / "aspen.sock"),
                             "status_storage_root": str(self.root / "rad-home/storage"), "status": {"allowed_rids": [native["source"]["repository"]],
                             "signer_identity": {"contract_version": 1, "provider_namespace": "radicle", "native_kind": "node", "native_id": "z6Mkon3Necd6NkkyfoGoHxid2znGc59LU3K7mubaRcFbLfLX"},
                             "signer": {"project_root": str(self.root), "project": "fixture", "profile": "fixture", "secret_key": "RADICLE_SIGNER_SEED", "provider": "fixture-primary"}}}
        controller = self.export("controller-profile", "(import " + json.dumps(str(REPO / "config/radicle-ci.ncl")) + ") " + ncl(controller_inputs))
        controller["runtime_profile"] = policy["aspen_document"]
        controller["runtime_profile"]["bounds"]["callback_timeout_millis"] = FIXTURE_CALLBACK_MILLISECONDS
        controller.update({"maximum_polls": FIXTURE_OBSERVATION_POLLS, "poll_interval_milliseconds": FIXTURE_OBSERVATION_INTERVAL_MILLISECONDS})
        controller["status_profile"]["executable"].update({"path": self.exe["secretspec_fixture"], "blake3": facts["secretspec_fixture"]["blake3"]})
        put(self.root / "controller.json", controller)
        put(self.root / "aspen.json", controller["runtime_profile"])
        put(self.root / "radicle.json", policy["radicle_profile"])
        self.producer(native["source"], native["selection"], native["project_name"])
        request = self.json_command([self.exe["legacy_coordinator"], "native-workflow", self.root / "radicle.json", self.trigger_path])
        self.exe["worker"] = self.exe["real_worker"]
        lattice = self.workflow(inputs, policy, project_identity, request,
                                worker_arguments=["project-run", self.root / "project.json", project_identity])
        self.until("Lattice socket", lambda: (self.root / "lattice.sock").is_socket() or self.failed(lattice))
        state = self.root / "host-state"
        state.mkdir(mode=0o700)
        options = policy["host_options"]
        host_argv = [self.exe["host"], "--aspen-profile", self.root / "aspen.json", "--radicle-profile", self.root / "radicle.json", "--socket", self.root / "aspen.sock",
                     "--extension", self.exe["extension"], "--state-root", state, "--max-requests", FIXTURE_HOST_REQUESTS,
                     "--timeout-ms", options["timeout_milliseconds"], "--poll-interval-ms", options["poll_interval_milliseconds"]]
        host = self.spawn("host-first", host_argv)
        self.until("Aspen socket", lambda: (self.root / "aspen.sock").is_socket() or self.failed(host))
        put(self.root / "cohort.json", {"schema": "kiln.radicle-ci.native-cohort.v1", "executables": facts, "operation_identity": operation,
            "project_profile_identity": project_identity, "source_selection": native["selection"], "runtime_profile": controller["runtime_profile"],
            "consumer_tools": {"nixpkgs_source": str(Path(self.args.nixpkgs).resolve(strict=True)), "lock": load(self.root / "project-source/flake.lock")},
            "fixture_authority": "new isolated native source profile and fixed fixture status key only; no production mutation"})
        return native, project_identity, host, host_argv, lattice

    def producer(self, source, selection, project_name):
        record = self.run([self.exe["broker_tool"], "trigger", "--message", "--repo", project_name, "--node", selection["signer_node_id"], "--commit", source["revision"], "--ref", "main"])
        message = json.loads(record["stdout"])
        if message["repository"]["id"] != source["repository"] or message["commits"][0] != source["revision"]:
            raise ValueError("actual producer selected another repository/revision")
        self.message_path = self.root / (source["revision"] + "-broker-message.json")
        self.trigger_path = self.root / (source["revision"] + "-trigger.json")
        put(self.message_path, message)
        ref = lambda kind, value: {"contract_version": 1, "provider_namespace": "radicle", "native_kind": kind, "native_id": value}
        trigger = {"repository": ref("repository", source["repository"]), "actor": ref("actor", "broker"), "before": None,
                   "after": ref("revision", source["revision"]), "branch": "main", "patch": None, "event": "push"}
        put(self.trigger_path, trigger)
        return message

    def broker_process(self, name):
        command = shlex.join([self.exe["coordinator"], "--profile", str(self.root / "controller.json")]) + " < " + shlex.quote(str(self.message_path))
        script = self.root / (name + ".sh")
        put(script, ("#!" + self.exe["shell"] + "\nexec " + command + "\n").encode())
        return self.spawn(name, [self.exe["shell"], script])

    def uncertain_broker(self, native):
        source = self.source_files("pending-without-provider-" + self.root.name)
        native = self.json_command([self.exe["fixture"], "advance", self.root / "source-fixture", source,
                                    self.root / "rad-home", native["source"]["repository"]])
        self.producer(native["source"], native["selection"], native["project_name"])
        profile = load(self.root / "controller.json")
        profile.update({"socket_path": str(self.root / "absent-provider.sock"), "maximum_polls": 1})
        profile_path = self.root / "uncertain-controller.json"
        context = self.root / "uncertain-source.json"
        put(profile_path, profile)
        put(context, native["source"])
        replies = []
        original = None
        for _ in range(2):
            reply = self.run([self.exe["coordinator"], "--profile", profile_path],
                             data=self.message_path.read_bytes(), allowed=(1,))
            lines = [json.loads(line) for line in reply["stdout"].splitlines()]
            journal = self.ci("reconcile", profile_path, context)
            state = self.ci("replay", profile_path, context)
            bindings = self.ci("bindings", profile_path, context)
            if lines != [{"response": "triggered", "run_id": {"id": bindings["run_id"]}}]:
                raise ValueError("uncertain broker emitted a final result instead of only Triggered")
            if state["runs"][bindings["run_id"]]["outcome"] != "Pending" or journal["status_intent"] is not None:
                raise ValueError("bounded uncertainty manufactured canonical completion or final status")
            if original is not None and journal["ingress"]["operation_id"] != original:
                raise ValueError("uncertain redelivery replaced the original operation")
            original = journal["ingress"]["operation_id"]
            replies.append(reply)
        put(self.root / "uncertain-journal.json", journal)
        return {"source": native["source"], "operation_id": original, "canonical_outcome": "Pending",
                "broker_replies": replies, "status_intent": journal["status_intent"]}

    # r[verify kiln.radicle_ci.evidence] r[verify kiln.radicle_ci.recovery]
    def execute(self):
        native, profile_identity, host, host_argv, lattice = self.prepare()
        reports = []
        for number, case in enumerate(("success", "failure")):
            if number:
                source_files = self.source_files("fail")
                native = self.json_command([self.exe["fixture"], "advance", self.root / "source-fixture", source_files, self.root / "rad-home", native["source"]["repository"]])
                self.producer(native["source"], native["selection"], native["project_name"])
            context = self.root / (case + "-source.json")
            put(context, native["source"])
            bindings = self.ci("bindings", self.root / "controller.json", context)
            output = self.run([self.exe["nix"], "eval", "--raw", "--offline", "--extra-experimental-features", "nix-command flakes", "path:" + str(self.root / "project-source") + "#jobs.app.outPath"])["stdout"]
            before = self.run([self.exe["nix_store"], "--check-validity", output], allowed=(0, 1))
            if Path(output).exists() or before["exit_code"] == 0: raise ValueError("consumer output was already present before dispatch")
            broker = self.broker_process(case + "-broker-first")
            invocation = self.root / "provider-invocations.log"
            self.until("real fixed project worker invocation", lambda: invocation.exists() and len(invocation.read_text().splitlines()) == number + 1 or self.failed(broker))
            journal_path = Path(bindings["root"]) / "ci-journal.json"
            original = load(journal_path)["ingress"]["operation_id"]
            if number == 0:
                self.until("real Nix build intent", lambda: (Path(bindings["root"]) / "build-state.json").is_file() or self.failed(broker))
                self.stop(host, abrupt=True)
                self.stop(broker, abrupt=True)
                pending = self.ci("replay", self.root / "controller.json", context)
                if pending["runs"][bindings["run_id"]]["outcome"] != "Pending" or lattice["process"].poll() is not None:
                    raise ValueError("restart did not preserve a pending independent real execution")
                if (Path(bindings["root"]) / "project-result.json").exists(): raise ValueError("worker already completed before restart")
                (self.root / "aspen.sock").unlink(missing_ok=True)
                host = self.spawn("host-restarted", host_argv)
                self.until("recovered Aspen socket", lambda: (self.root / "aspen.sock").is_socket() or self.failed(host))
                broker = self.broker_process("success-broker-recovered")
            self.until("broker terminal result", lambda: broker["process"].poll() is not None or False)
            if broker["process"].returncode: raise ValueError("broker failed: " + broker["log"].read_text())
            journal = self.ci("reconcile", self.root / "controller.json", context)
            state = self.ci("replay", self.root / "controller.json", context)
            expected = "Success" if case == "success" else "DomainFailure"
            if state["runs"][bindings["run_id"]]["outcome"] != expected: raise ValueError("canonical outcome disagrees with actual build")
            if journal["status_observation"]["disposition"] != "written": raise ValueError("status publication remains uncertain")
            written = journal["status_observation"]["observation"]
            if written["revision"]["native_id"] != native["source"]["revision"]: raise ValueError("status revision differs")
            if journal["ingress"]["operation_id"] != original: raise ValueError("restart changed original operation")
            replayed = self.ci("start", self.root / "controller.json", self.trigger_path)
            if replayed["identity"] != journal["identity"]: raise ValueError("duplicate changed terminal evidence")
            reconciled = self.ci("reconcile", self.root / "controller.json", context)
            if reconciled["status_observation"] != journal["status_observation"]: raise ValueError("status reconciliation changed exact object/head")
            result = load(Path(bindings["root"]) / "project-result.json")
            task = next(task for task in result["worker"]["discovery_observation"]["tasks"] if task["native_attribute"]["native_id"] == "app")
            if task["outputs"][0]["path"]["native_id"] != output: raise ValueError("actual discovery changed the preflight output")
            roots = {str(path): str(path.resolve(strict=True)) for path in (self.root / "build-roots").rglob("*") if path.is_symlink()}
            if case == "success":
                if list(roots.values()) != [output] or Path(output).read_text() != "exact-event-source-" + self.root.name + "\n": raise ValueError("exact event bytes/output root handoff mismatch")
                observation = self.observe(self.root / "controller.json", context)
            else:
                diagnostics = b"".join(path.read_bytes() for path in Path(bindings["root"]).glob("*.log"))
                if b"source-requested-genuine-build-failure" not in diagnostics: raise ValueError("genuine Nix failure diagnostic absent")
                observation = None
            release = [self.json_command([self.exe["real_worker"], "project-release", self.root / "project.json", profile_identity, context]) for _ in range(2)]
            if any(path.is_symlink() for path in (self.root / "build-roots").rglob("*")): raise ValueError("build output roots retained after explicit release")
            if (Path(bindings["root"]) / "source-root").is_symlink(): raise ValueError("source root retained after explicit release")
            put(self.root / (case + "-journal.json"), journal)
            reports.append({"case": case, "source": native["source"], "operation_id": original, "canonical_outcome": expected, "status": written,
                            "output_absent_before_dispatch": before, "rooted_outputs": roots, "release": release, "ucan_observation": observation})
        uncertainty = self.uncertain_broker(native)
        invocations = (self.root / "provider-invocations.log").read_text().splitlines()
        if invocations != [report["source"]["revision"] for report in reports]: raise ValueError("duplicate/restart redispatched an operation")
        report = {"schema": "kiln.radicle-ci.native-verification.v1", "cases": reports, "provider_invocations": invocations,
                  "workflow_observation": load(self.root / "workflow-observed.json"), "project_profile_identity": profile_identity,
                  "uncertainty": uncertainty,
                  "maintenance_owner": "Kiln maintainers", "non_claims": ["isolated signed-source fixture, not production source trust", "real cibtool producer command, not a live remote Radicle delivery", "no remote status synchronization, activation, deployment or release authority", "bounded execution is not sandboxing or hermeticity"]}
        put(self.root / "verification.json", report)
        print(json.dumps(report, indent=2))

    def close(self):
        self.private_files.extend(path for path in (self.root / "rad-home/keys").rglob("*") if path.is_file())
        super().close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("inputs", "nickel", "root", "worker", "extension", "coordinator", "source-fixture", "broker-tool", "secretspec-fixture", "radicle", "nixpkgs"):
        parser.add_argument("--" + name, required=True)
    args = parser.parse_args()
    signal.signal(signal.SIGTERM, PILOT["interrupted"])
    signal.signal(signal.SIGHUP, PILOT["interrupted"])
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
