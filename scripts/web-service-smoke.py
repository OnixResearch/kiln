#!/usr/bin/env python3
"""Exercise the actual web service and native Lattice with disposable exact Git source.

No remote publication, production activation, lifecycle completion or sandbox claim.
Requires the maintained shell-success Lattice cohort and user namespaces.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shlex
import shutil
import signal
import socket
import ssl
import subprocess
import sys
import time
import urllib.error
import urllib.request

REPO = Path(__file__).resolve().parent.parent
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("kiln_native_pilot", REPO / "scripts/onixos-ci-pilot.py")
pilot = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pilot)
DOCUMENT_BYTES = 8 * 1024 * 1024
WAIT_SECONDS = 90
LOG_CHUNK_BYTES = 32
MAX_LOG_CHUNKS = 16
# Native serve counts lifetime exchanges, not concurrent clients.
NATIVE_MAX_EXCHANGES = 1024


def put(path, value):
    path.write_bytes(value if isinstance(value, bytes) else (json.dumps(value, indent=2) + "\n").encode())
    path.chmod(0o600)


class Exercise:
    def __init__(self, args):
        self.args = args
        self.root = Path(args.root).absolute()
        if self.root == Path("/tmp") or Path("/tmp") in self.root.parents:
            raise ValueError("scratch must be outside the producer's private /tmp")
        self.evidence = Path(args.evidence).absolute()
        self.evidence.mkdir(parents=True, exist_ok=True)
        self.exe = {name: str(Path(shutil.which(name)).resolve(strict=True)) for name in ("nickel", "b3sum", "openssl", "jj", "python3", "bwrap", "bash")}
        self.exe.update(service=str(Path(args.service).resolve(strict=True)), lattice=str(Path(args.lattice).resolve(strict=True)))
        self.exe["bubblewrap"] = self.exe["bwrap"]
        self.source = str(Path(args.lattice_source).resolve(strict=True))
        self.root.mkdir(mode=0o700)
        for name in ("lattice-tmp", "work", "state", "home"):
            (self.root / name).mkdir(mode=0o700)
        self.env = {"HOME": str(self.root / "home"), "XDG_CONFIG_HOME": str(self.root / "home/config"), "XDG_CACHE_HOME": str(self.root / "home/cache"), "LANG": "C.UTF-8", "PATH": os.environ["PATH"], "TMPDIR": str(self.root / "lattice-tmp")}
        self.processes = []
        self.report = {"schema": "kiln.web-service.native-exercise.v1", "checks": {}, "non_claims": ["isolated local evidence is not production deployment", "bounds are not hostile code confinement", "worker artifacts are declarations, not semantic product proofs", "no source trust, release eligibility or lifecycle acceptance"]}
        self.operator = os.urandom(32).hex()
        self.worker = os.urandom(32).hex()
        put(self.root / "operator.token", self.operator.encode())
        put(self.root / "worker.token", self.worker.encode())
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            self.port = listener.getsockname()[1]
        self.origin = f"https://127.0.0.1:{self.port}"
        self.repo = self.root / "source"
        self.context = None

    def command(self, argv, allowed=(0,), **kwargs):
        result = subprocess.run(list(map(str, argv)), cwd=self.root, env=self.env, capture_output=True, timeout=300, **kwargs)
        if len(result.stdout) + len(result.stderr) > DOCUMENT_BYTES:
            raise ValueError("native proof command exceeded output bound")
        if result.returncode not in allowed:
            raise RuntimeError(f"command {argv[0]} exited {result.returncode}: {result.stderr.decode(errors='replace')[:2048]}")
        return result.stdout

    def digest(self, path):
        return self.command([self.exe["b3sum"], path]).decode().split()[0]

    def export(self, builder, inputs, output):
        put(self.root / (output + "-inputs.json"), inputs)
        expression = f'(import {json.dumps(str(REPO / "config" / builder))}) (import {json.dumps(str(self.root / (output + "-inputs.json")))})'
        expression_path = self.root / (output + "-export.ncl")
        put(expression_path, expression.encode())
        raw = self.command([self.exe["nickel"], "export", "--format", "json", expression_path])
        put(self.root / (output + ".json"), raw)
        return json.loads(raw)

    def spawn(self, name, argv):
        log = (self.root / (name + ".log")).open("ab")
        process = subprocess.Popen(list(map(str, argv)), cwd=self.root, env=self.env, stdout=log, stderr=log, start_new_session=True)
        log.close()
        self.processes.append(process)
        return process

    def wait(self, predicate, description, seconds=WAIT_SECONDS):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            value = predicate()
            if value:
                return value
            time.sleep(0.1)
        raise TimeoutError(description)

    def api(self, method, target, body=None, token=None, expected=(200, 201, 202), origin=None):
        raw = None if body is None else json.dumps(body).encode()
        headers = {"Authorization": "Bearer " + (self.operator if token is None else token)}
        if origin is not False:
            headers["Origin"] = self.origin if origin is None else origin
        if raw is not None:
            headers["Content-Type"] = "application/json"
        request = urllib.request.Request(self.origin + target, data=raw, headers=headers, method=method)
        try:
            with urllib.request.urlopen(request, context=self.context, timeout=15) as response:
                status, data = response.status, response.read(DOCUMENT_BYTES + 1)
        except urllib.error.HTTPError as error:
            status, data = error.code, error.read(DOCUMENT_BYTES + 1)
        if status not in expected or len(data) > DOCUMENT_BYTES:
            raise RuntimeError(f"{method} {target}: unexpected status/size {status}: {data[:2048]!r}")
        try:
            return json.loads(data)
        except (ValueError, UnicodeDecodeError):
            return data

    def prepare(self):
        self.command([self.exe["openssl"], "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1", "-subj", "/CN=Kiln isolated exercise CA", "-addext", "basicConstraints=critical,CA:TRUE", "-addext", "keyUsage=critical,keyCertSign,cRLSign", "-keyout", self.root / "ca-key.pem", "-out", self.root / "ca.pem"])
        self.command([self.exe["openssl"], "req", "-new", "-newkey", "rsa:2048", "-nodes", "-subj", "/CN=127.0.0.1", "-keyout", self.root / "key.pem", "-out", self.root / "server.csr"])
        put(self.root / "server.ext", b"basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\nsubjectAltName=IP:127.0.0.1\n")
        self.command([self.exe["openssl"], "x509", "-req", "-in", self.root / "server.csr", "-CA", self.root / "ca.pem", "-CAkey", self.root / "ca-key.pem", "-CAcreateserial", "-days", "1", "-extfile", self.root / "server.ext", "-out", self.root / "certificate.pem"])
        (self.root / "key.pem").chmod(0o600)
        (self.root / "ca-key.pem").chmod(0o600)
        self.context = ssl.create_default_context(cafile=str(self.root / "ca.pem"))
        self.command([self.exe["jj"], "git", "init", "--colocate", self.repo])
        put(self.repo / "source-value.txt", b"exact-source-artifact-v1\n")
        put(self.repo / "ci.py", b'import pathlib,sys,time\nprint("actual command started",flush=True)\npathlib.Path(sys.argv[2]).write_text("started")\nif sys.argv[1]=="slow": time.sleep(40)\nif sys.argv[1]=="fail": print("actual intentional failure",file=sys.stderr); sys.exit(7)\npayload=pathlib.Path("source-value.txt").read_bytes()\npathlib.Path("artifact.txt").write_bytes(payload)\nprint("actual artifact bytes:",payload.decode().strip(),flush=True)\n')
        for args in (["config", "set", "--repo", "user.name", "Kiln isolated exercise"], ["config", "set", "--repo", "user.email", "kiln@example.invalid"], ["file", "track", "root:ci.py", "root:source-value.txt"], ["describe", "-m", "Exact source for isolated web workflow"], ["bookmark", "create", "main", "-r", "@"]):
            self.command([self.exe["jj"], "-R", self.repo, *args])
        self.revision = self.command([self.exe["jj"], "-R", self.repo, "log", "--no-graph", "-r", "main", "-T", "commit_id"]).decode().strip()
        limits = {"max_entries": 128, "max_path_bytes": 1024, "max_file_bytes": 65536, "max_total_bytes": 1048576}
        transport = {"maximum_frame_bytes": 1048576, "maximum_log_bytes": 1048576, "maximum_connections": NATIVE_MAX_EXCHANGES, "read_deadline_milliseconds": 10000, "write_deadline_milliseconds": 10000}
        projects = []
        for mode in ("success", "slow", "fail"):
            steps = [{"node_id": "build", "program": self.exe["python3"], "argv": ["ci.py", mode, str(self.root / ("command-" + mode + ".started"))], "environment": {"PYTHONNOUSERSITE": "1"}, "artifacts": [] if mode == "fail" else ["artifact.txt"]}]
            if mode == "success":
                steps.append({"node_id": "build", "program": self.exe["python3"], "argv": ["-c", "print('second reviewed step', flush=True)"], "environment": {"PYTHONNOUSERSITE": "1"}, "artifacts": []})
            worker = self.export("web-worker.ncl", {"project_id": mode, "repository_path": str(self.repo), "service_origin": self.origin, "token_file": str(self.root / "worker.token"), "tls_ca_path": str(self.root / "ca.pem"), "work_root": str(self.root / "work"), "snapshot_limits": limits, "command_timeout_ms": 60000, "maximum_output_bytes": 65536, "http_timeout_ms": 5000, "lookup_timeout_ms": 15000, "maximum_artifact_bytes": 65536, "maximum_total_artifact_bytes": 65536, "maximum_artifacts": 1, "steps": steps}, "worker-" + mode)
            identity = self.digest(self.root / ("worker-" + mode + ".json"))
            workflow = {"socket_path": str(self.root / "lattice.sock"), "allowed_user_ids": [os.getuid()], "transport_limits": transport, "adapter_limits": {"maximum_request_bytes": 1048576, "maximum_response_bytes": 1048576}, "policy_id": "web-native-" + mode, "provider_profile_id": "lattice-web-native-" + mode, "platform": "linux-x86_64", "nodes": [{"id": "build", "kind": "build", "effect_class": "build", "adapter_kind": "local", "operation_identity": identity}], "edges": [], "controls": {"cancel": True, "pause": False, "resume": False}, "bounds": {"max_nodes": 8, "max_edges": 8, "max_log_references": MAX_LOG_CHUNKS, "max_log_read_bytes": LOG_CHUNK_BYTES, "max_total_log_bytes": 1048576}}
            projects.append({"id": mode, "label": "Native <img src=x onerror=alert(1)> " + mode, "repository_path": str(self.repo), "reference": "refs/heads/main", "source_limits": limits, "workflow": workflow})
        profile = {"service_id": "isolated-web-exercise", "listen_address": "127.0.0.1:" + str(self.port), "origin": self.origin, "tls": {"certificate_path": str(self.root / "certificate.pem"), "private_key_path": str(self.root / "key.pem"), "min_version": "TLS1.3"}, "operator_token_file": str(self.root / "operator.token"), "worker_token_file": str(self.root / "worker.token"), "state_directory": str(self.root / "state"), "projects": projects, "bounds": {"max_request_bytes": 1048576, "max_header_bytes": 16384, "max_connections": 16, "timeout_seconds": 10, "poll_interval_ms": 100, "max_projects": 8, "max_runs": 32, "max_artifact_bytes": 65536, "max_artifacts": 4, "max_run_artifact_bytes": 262144, "max_total_artifact_bytes": 1048576, "max_log_read_bytes": 65536, "max_requests": 256, "max_events_per_run": 128, "max_state_bytes": 8388608, "max_response_bytes": 4194304}}
        self.export("web-service.ncl", profile, "service")
        put(self.root / "lattice.ncl", pilot.ncl({"data_dir": str(self.root / "lattice-store"), "shell": self.exe["bash"], "default_timeout_secs": 90, "max_parallelism": 1}).encode())
        prefix = pilot.lattice_command(self.exe, self.root, "--config", str(self.root / "lattice.ncl"))
        routes = []
        for project in projects:
            mode = project["id"]
            worker_path = self.root / ("worker-" + mode + ".json")
            identity = self.digest(worker_path)
            command = "printf '%s\\n' \"$LATTICE_SOURCE_REVISION\" >> " + shlex.quote(str(self.root / "invocations.log")) + "; exec " + shlex.join([self.exe["service"], "job", "--profile", str(worker_path), "--identity", identity]) + " > " + shlex.quote(str(self.root / ("worker-" + mode + ".stdout.log"))) + " 2> " + shlex.quote(str(self.root / ("worker-" + mode + ".stderr.log")))
            workflow = {"name": "web-" + mode, "command": command, "timeout_seconds": 90, "created": "2026-10-27T00:00:00Z"}
            self.export("live-ci-workflow.ncl", workflow, "workflow-" + mode)
            put(self.root / ("workflow-" + mode + ".ncl"), ("import " + json.dumps(str(self.root / ("workflow-" + mode + ".json")))).encode())
            self.command([*prefix, "import", self.root / ("workflow-" + mode + ".ncl")])
            expression = "let base = import " + json.dumps(self.source + "/examples/orchestrations/llm-review-loop.ncl") + " in base & { objective | force = \"Native web route identity observation\", workflow | force = " + pilot.ncl({"workflow_name": "web-" + mode, "content_hash": identity}) + ", triggers | force = [] }"
            observe = self.root / ("observe-" + mode + ".ncl")
            put(observe, expression.encode())
            observed = json.loads(self.command([*prefix, "orchestration", "validate", observe, "--output", "json"], allowed=(0, 1)))
            revision = observed["workflow_revision"]["content_hash"]
            put(observe, expression.replace(identity, revision, 1).encode())
            self.command([*prefix, "orchestration", "validate", observe, "--output", "json"])
            native = json.loads(self.command([self.exe["service"], "reviewed-request", "--profile", self.root / "service.json", "--project", mode]))
            put(self.root / ("native-" + mode + ".json"), native)
            routes.append({"profile_identity": native["profile_identity"], "graph_identity": native["graph"]["identity"], "workflow_name": "web-" + mode, "workflow_revision": "b3:" + revision, "effect_authority": "route_exact", "source_binding": "shell_environment", "nodes": [{"request_node_id": native["graph"]["nodes"][0]["id"], "operation_identity": native["graph"]["nodes"][0]["operation_identity"], "workflow_node_id": 0}]})
        handler = {"schema": "lattice.workflow-exchange-handler-profile.v1", "socket_path": str(self.root / "lattice.sock"), "transport": {"schema": "lattice.workflow-transport-profile.v1", "limits": transport, "peers": {"allowed_user_ids": [os.getuid()]}, "non_claims": ["transport admission does not prove workflow correctness", "transport peer identity does not prove peer intent", "transport acceptance does not prove an operation executed", "transport response does not prove an external effect succeeded", "transport checks do not establish production deployment"]}, "maximum_active_executions": 8, "routes": sorted(routes, key=lambda route: (route["profile_identity"], route["graph_identity"])), "non_claims": ["handler admission does not prove workflow correctness", "route authority does not prove host confinement", "execution observation does not prove an external effect succeeded", "peer identity does not prove peer intent", "handler checks do not establish release readiness"]}
        expression = "let Contract = import " + json.dumps(self.source + "/contracts/workflow-exchange-handler-profile.ncl") + " in (" + pilot.ncl(handler) + ") | Contract.HandlerProfile"
        put(self.root / "handler.ncl", expression.encode())
        self.command([self.exe["nickel"], "export", "--format", "json", self.root / "handler.ncl"])
        self.spawn("lattice", [*prefix, "workflow-exchange", "serve", "--profile", self.root / "handler.ncl"])
        self.wait(lambda: (self.root / "lattice.sock").exists(), "Lattice socket")
        self.start_service()
        self.report.update(origin=self.origin, source_revision=self.revision, service_profile_blake3=self.digest(self.root / "service.json"), executables={name: {"path": path, "blake3": self.digest(path)} for name, path in self.exe.items()}, lattice_source=self.source)
        print(json.dumps({"ready": True, "origin": self.origin, "root": str(self.root)}), flush=True)

    def start_service(self):
        before = {path.name: (path.stat().st_size, path.stat().st_mtime_ns) for path in (self.root / "state").iterdir()}
        preflight = self.command([self.exe["service"], "check", "--profile", self.root / "service.json"])
        assert self.operator.encode() not in preflight and self.worker.encode() not in preflight
        assert before == {path.name: (path.stat().st_size, path.stat().st_mtime_ns) for path in (self.root / "state").iterdir()}, "preflight modified persistent state"
        self.service = self.spawn("service", [self.exe["service"], "serve", "--profile", self.root / "service.json"])
        def ready():
            try:
                return self.api("GET", "/api/projects")
            except (OSError, urllib.error.URLError):
                if self.service.poll() is not None:
                    raise RuntimeError("service failed: " + (self.root / "service.log").read_text()[-2048:])
                return False
        self.wait(ready, "HTTPS service")
        health = self.api("GET", "/healthz")
        assert health == {"status": "ok"}, health
        self.report["checks"]["startup"] = {"preflight_state_unchanged": True, "health": health}

    def enroll(self, project):
        return self.api("POST", "/api/projects", {"project_id": project, "request_id": "enroll-" + project})

    def terminal(self, run):
        def observed():
            view = self.api("GET", "/api/runs/" + run["id"])
            return view if view["outcome"] not in ("Pending", "Unknown") else False
        return self.wait(observed, "terminal native run")

    def captured_log(self, run_id, stream):
        cursor, output, reads = 0, "", 0
        for _ in range(MAX_LOG_CHUNKS + 1):
            view = self.api("GET", f'/api/runs/{run_id}/logs?node_id=build&stream={stream}&cursor={cursor}&max_bytes={LOG_CHUNK_BYTES}')
            assert view["source"] == "worker-capture" and view["gap"] is False, view
            assert view["cursor"] == cursor and view["next_cursor"] >= cursor, view
            reads += 1
            output += view["text"]
            if view["next_cursor"] == cursor:
                return {**view, "cursor": 0, "text": output, "reads": reads}
            cursor = view["next_cursor"]
        raise AssertionError("captured log pagination exceeded declared chunk bound")

    def execute(self):
        self.prepare()
        if self.args.browser_hold:
            self.wait(lambda: (self.root / "browser-done.json").exists(), "browser evidence marker", seconds=900)
            self.report["browser"] = json.loads((self.root / "browser-done.json").read_bytes())
        self.api("GET", "/api/projects", token="0" * 64, expected=(401,))
        self.api("POST", "/api/projects", {"project_id": "success", "request_id": "worker-cannot-enroll"}, token=self.worker, expected=(403,))
        self.api("POST", "/api/projects", {"project_id": "success", "request_id": "wrong-origin"}, origin="https://not-registered.invalid", expected=(403,))
        self.api("POST", "/api/projects", {"project_id": "success", "request_id": "missing-origin"}, origin=False, expected=(403,))
        legacy = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        legacy.check_hostname = False
        legacy.verify_mode = ssl.CERT_NONE
        legacy.maximum_version = ssl.TLSVersion.TLSv1_2
        try:
            with socket.create_connection(("127.0.0.1", self.port), timeout=5) as raw:
                with legacy.wrap_socket(raw, server_hostname="127.0.0.1"):
                    raise AssertionError("service admitted TLS below 1.3")
        except ssl.SSLError:
            self.report["checks"]["tls"] = "TLS1.3 succeeded; TLS1.2 rejected"
        self.api("POST", "/api/runs", {"project_id": "not-installed", "request_id": "unknown-project"}, expected=(400, 404))
        self.enroll("success")
        request = {"project_id": "success", "request_id": "native-success"}
        run = self.api("POST", "/api/runs", request)
        assert self.api("POST", "/api/runs", request)["id"] == run["id"]
        self.api("POST", "/api/runs", {**request, "project_id": "slow"}, expected=(409,))
        success = self.terminal(run)
        assert success["outcome"] == "Success" and success["source_revision"] == self.revision, success
        logs = self.captured_log(run["id"], "stdout")
        assert "actual artifact bytes: exact-source-artifact-v1" in logs["text"], logs
        assert logs["source"] == "worker-capture" and logs["gap"] is False
        assert "second reviewed step" in logs["text"] and logs["reads"] > 2, logs
        empty = self.captured_log(run["id"], "stderr")
        assert empty["text"] == "" and empty["next_cursor"] == 0 and empty["retained"] is True, empty
        diagnostic = self.api("GET", f'/api/runs/{run["id"]}/logs?node_id=build&stream=diagnostic&cursor=0&max_bytes={LOG_CHUNK_BYTES}')
        assert diagnostic["source"] == "native-provider" and diagnostic["text"] == "completed\n", diagnostic
        artifacts = self.api("GET", f'/api/runs/{run["id"]}/artifacts')["artifacts"]
        assert len(artifacts) == 1
        artifact = artifacts[0]
        downloaded = self.api("GET", artifact["download_path"])
        assert downloaded == b"exact-source-artifact-v1\n", downloaded
        put(self.root / "downloaded.bin", downloaded)
        assert self.digest(self.root / "downloaded.bin") == artifact["blake3"].removeprefix("b3:")
        self.report["checks"]["success"] = {"run": success, "logs": logs, "artifact": artifact}
        self.enroll("fail")
        failure = self.terminal(self.api("POST", "/api/runs", {"project_id": "fail", "request_id": "native-failure"}))
        assert failure["outcome"] == "DomainFailure", failure
        assert self.api("GET", f'/api/runs/{failure["id"]}/artifacts')["artifacts"] == []
        failure_log = self.captured_log(failure["id"], "stderr")
        assert failure_log["source"] == "worker-capture" and "actual intentional failure" in failure_log["text"], failure_log
        self.report["checks"]["failure_log"] = failure_log
        self.report["checks"]["failure"] = failure
        self.enroll("slow")
        slow = self.api("POST", "/api/runs", {"project_id": "slow", "request_id": "native-cancel"})
        def active():
            view = self.api("GET", "/api/runs/" + slow["id"])
            return view if view["execution"] and view["controls"]["cancel"] else False
        active_run = self.wait(active, "cancellable native execution")
        self.wait(lambda: (self.root / "command-slow.started").exists(), "actual source command start before cancellation")
        invocation_count = len((self.root / "invocations.log").read_text().splitlines())
        self.service.terminate()
        self.service.wait(timeout=15)
        self.start_service()
        restored = self.api("GET", "/api/runs/" + slow["id"])
        assert restored["execution"] == active_run["execution"] and restored["attempt_id"] == active_run["attempt_id"]
        assert len((self.root / "invocations.log").read_text().splitlines()) == invocation_count
        control = {"request_id": "cancel-exact", "attempt_id": restored["attempt_id"], "expected_revision": restored["revision"], "expected_sequence": restored["sequence"], "execution": restored["execution"], "action": "cancel"}
        self.api("POST", f'/api/runs/{slow["id"]}/control', {**control, "request_id": "stale", "expected_revision": restored["revision"] + 1}, expected=(409,))
        self.api("POST", f'/api/runs/{slow["id"]}/control', {**control, "request_id": "unsupported", "action": "pause"}, expected=(409,))
        # Refresh exact native sequence after read-only negative checks.
        restored = self.api("GET", "/api/runs/" + slow["id"])
        control.update(expected_revision=restored["revision"], expected_sequence=restored["sequence"])
        self.api("POST", f'/api/runs/{slow["id"]}/control', control)
        cancelled = self.terminal(slow)
        self.report["checks"].update(authentication="invalid token and worker control-plane role rejected", replay="same request returns same run; changed request conflicts", restart={"execution_preserved": True, "invocations_before": invocation_count, "invocations_after": len((self.root / "invocations.log").read_text().splitlines())}, cancellation=cancelled, stale_control="rejected", unsupported_control="rejected")
        assert cancelled["outcome"] == "Cancelled", cancelled
        assert self.api("GET", f'/api/runs/{slow["id"]}/artifacts')["artifacts"] == [], "cancelled work published an artifact"
        self.report["passed"] = True

    def close(self):
        failures = []
        for process in reversed(self.processes):
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait(timeout=5)
                    failures.append("forced process termination")
        for name in ("operator.token", "worker.token", "key.pem", "ca-key.pem"):
            (self.root / name).unlink(missing_ok=True)
        self.report["retirement"] = {"all_processes_exited": all(process.poll() is not None for process in self.processes), "credentials_removed": all(not (self.root / name).exists() for name in ("operator.token", "worker.token", "key.pem", "ca-key.pem")), "failures": failures}
        for pattern in ("service.json", "worker-*.json", "native-*.json", "service.log", "lattice.log", "worker-*.stdout.log", "worker-*.stderr.log", "downloaded.bin", "certificate.pem", "ca.pem"):
            for path in self.root.glob(pattern):
                shutil.copyfile(path, self.evidence / path.name)
        shutil.rmtree(self.root)
        self.report["retirement"]["scratch_removed"] = not self.root.exists()
        put(self.evidence / "verification.json", self.report)
        if failures:
            raise RuntimeError("native resource retirement required force")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--service", required=True)
    parser.add_argument("--lattice", required=True)
    parser.add_argument("--lattice-source", required=True)
    parser.add_argument("--root", required=True, help="New private scratch directory outside /tmp")
    parser.add_argument("--evidence", required=True)
    parser.add_argument("--browser-hold", action="store_true", help="Wait for browser-done.json before native checks")
    args = parser.parse_args()
    os.umask(0o077)
    exercise = Exercise(args)
    try:
        exercise.execute()
    except BaseException as error:
        exercise.report["failure"] = {"kind": type(error).__name__, "diagnostic": str(error)}
        raise
    finally:
        exercise.close()


if __name__ == "__main__":
    main()
