#!/usr/bin/env python3
"""Exercise the actual web service and native Lattice with disposable exact Git source.

No remote publication, production activation, lifecycle completion or sandbox claim.
Requires the maintained shell-success Lattice cohort and user namespaces.
"""
import argparse
import base64
from collections import Counter
import copy
import hashlib
import hmac
import http.server
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
import threading
import urllib.error
import urllib.request
import uuid

REPO = Path(__file__).resolve().parent.parent
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("kiln_native_pilot", REPO / "scripts/onixos-ci-pilot.py")
pilot = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pilot)
DOCUMENT_BYTES = 8 * 1024 * 1024
WAIT_SECONDS = 90
LOG_CHUNK_BYTES = 32
MAX_LOG_CHUNKS = 64
# Native serve counts lifetime exchanges, not concurrent clients.
NATIVE_MAX_EXCHANGES = 4096


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
        self.viewer = os.urandom(32).hex()
        self.trigger_secret = os.urandom(32).hex()
        self.notification_secret = os.urandom(32).hex()
        self.notification_server = None
        self.notification_thread = None
        self.notifications = []
        put(self.root / "operator.token", self.operator.encode())
        put(self.root / "worker.token", self.worker.encode())
        for name, value in (("viewer.token", self.viewer), ("trigger.token", self.trigger_secret), ("notification.token", self.notification_secret)):
            put(self.root / name, value.encode())
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

    def api(self, method, target, body=None, token=None, expected=(200, 201, 202), origin=None, anonymous=False, extra_headers=None):
        raw = body if isinstance(body, bytes) else None if body is None else json.dumps(body).encode()
        headers = {} if anonymous else {"Authorization": "Bearer " + (self.operator if token is None else token)}
        if origin is not False:
            headers["Origin"] = self.origin if origin is None else origin
        if raw is not None:
            headers["Content-Type"] = "application/json"
        headers.update(extra_headers or {})
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
        put(self.repo / "ci.py", '''import ast,inspect,json,os,pathlib,subprocess,sys,time,trace,unittest,xml.etree.ElementTree as ET
print("actual command started", flush=True)
pathlib.Path(sys.argv[2]).write_text(str(os.getpid()))
if sys.argv[1] == "slow":
    print("actual stderr before exit", file=sys.stderr, flush=True)
    time.sleep(40)
payload = pathlib.Path("source-value.txt").read_bytes()
class SourceTest(unittest.TestCase):
    def test_source(self):
        self.assertEqual(payload, b"exact-source-artifact-v1\\n")
        self.assertFalse(pathlib.Path(sys.argv[2]).with_suffix(".unstable").exists(), "Controlled repeated-observation failure")
result = unittest.TestResult()
unittest.defaultTestLoader.loadTestsFromTestCase(SourceTest).run(result)
suite = ET.Element("testsuite", name="source", tests=str(result.testsRun), failures=str(len(result.failures)), errors=str(len(result.errors)))
case = ET.SubElement(suite, "testcase", name="test_source", classname="SourceTest", file="ci.py")
for _, detail in result.failures: ET.SubElement(case, "failure", message="source assertion failed").text = detail
ET.ElementTree(suite).write("junit.xml", encoding="utf-8")
def classify(value):
    if value.endswith(b"v1\\n"):
        return "original"
    return "changed"
counter = trace.Trace(count=True, trace=False)
counter.runfunc(classify, payload)
tree = ast.parse(pathlib.Path(__file__).read_text())
function = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "classify")
lines = sorted(n.lineno for n in ast.walk(function) if isinstance(n, ast.stmt) and n is not function)
observed = {line: hits for (name, line), hits in counter.results().counts.items() if pathlib.Path(name).resolve() == pathlib.Path(__file__).resolve()}
hits = sum(observed.get(line, 0) > 0 for line in lines)
pathlib.Path("lcov.info").write_text("TN:source\\nSF:ci.py\\n" + "".join(f"DA:{line},{observed.get(line, 0)}\\n" for line in lines) + f"LF:{len(lines)}\\nLH:{hits}\\nend_of_record\\n")
coverage = ET.Element("coverage", {"lines-valid":str(len(lines)), "lines-covered":str(hits), "line-rate":str(hits / len(lines))})
package = ET.SubElement(ET.SubElement(coverage, "packages"), "package", name="source")
covered = ET.SubElement(ET.SubElement(ET.SubElement(package, "classes"), "class", name="SourceTest", filename="ci.py"), "lines")
for line in lines: ET.SubElement(covered, "line", number=str(line), hits=str(observed.get(line, 0)))
ET.ElementTree(coverage).write("coverage.xml", encoding="utf-8")
digest = subprocess.check_output([sys.argv[3], "source-value.txt"], text=True).split()[0]
for kind in ("gate", "review", "provenance"):
    document = {"schema":"kiln.native-exercise." + kind + ".v1", "candidate_file_blake3":digest, "verdict":"PASS" if result.wasSuccessful() else "FAIL", "tests_observed":result.testsRun, "non_claims":["Disposable command evidence; not Cairn acceptance or release authority"]}
    pathlib.Path(kind + ".json").write_text(json.dumps(document))
if sys.argv[1] == "fail":
    print("actual intentional failure", file=sys.stderr, flush=True)
    sys.exit(7)
if not result.wasSuccessful(): sys.exit(1)
pathlib.Path("artifact.txt").write_bytes(payload)
print("actual artifact bytes:", payload.decode().strip(), flush=True)
'''.encode())
        for args in (["config", "set", "--repo", "user.name", "Kiln isolated exercise"], ["config", "set", "--repo", "user.email", "kiln@example.invalid"], ["file", "track", "root:ci.py", "root:source-value.txt"], ["describe", "-m", "Exact source for isolated web workflow"], ["bookmark", "create", "main", "-r", "@"]):
            self.command([self.exe["jj"], "-R", self.repo, *args])
        self.revision = self.command([self.exe["jj"], "-R", self.repo, "log", "--no-graph", "-r", "main", "-T", "commit_id"]).decode().strip()
        limits = {"max_entries": 128, "max_path_bytes": 1024, "max_file_bytes": 65536, "max_total_bytes": 1048576}
        transport = {"maximum_frame_bytes": 1048576, "maximum_log_bytes": 1048576, "maximum_connections": NATIVE_MAX_EXCHANGES, "read_deadline_milliseconds": 10000, "write_deadline_milliseconds": 10000}
        projects = []
        for mode in ("success", "slow", "fail"):
            steps = [{"node_id": "build", "program": self.exe["python3"], "argv": ["ci.py", mode, str(self.root / ("command-" + mode + ".started")), self.exe["b3sum"]], "environment": {"PYTHONNOUSERSITE": "1"}, "artifacts": [] if mode == "fail" else ["artifact.txt"]}]
            if mode == "success":
                steps[0]["reports"] = [{"kind": kind, "path": path} for kind, path in (("junit", "junit.xml"), ("cobertura", "coverage.xml"), ("lcov", "lcov.info"), ("gate", "gate.json"), ("review", "review.json"), ("provenance", "provenance.json"))]
                steps.append({"node_id": "build", "program": self.exe["python3"], "argv": ["-c", "print('second reviewed step', flush=True)"], "environment": {"PYTHONNOUSERSITE": "1"}, "artifacts": []})
            worker = self.export("web-worker.ncl", {"project_id": mode, "repository_path": str(self.repo), "service_origin": self.origin, "token_file": str(self.root / "worker.token"), "tls_ca_path": str(self.root / "ca.pem"), "work_root": str(self.root / "work"), "snapshot_limits": limits, "command_timeout_ms": 60000, "maximum_output_bytes": 65536, "http_timeout_ms": 5000, "lookup_timeout_ms": 15000, "maximum_artifact_bytes": 65536, "maximum_total_artifact_bytes": 65536, "maximum_artifacts": 1, "steps": steps}, "worker-" + mode)
            identity = self.digest(self.root / ("worker-" + mode + ".json"))
            workflow = {"socket_path": str(self.root / "lattice.sock"), "allowed_user_ids": [os.getuid()], "transport_limits": transport, "adapter_limits": {"maximum_request_bytes": 1048576, "maximum_response_bytes": 1048576}, "policy_id": "web-native-" + mode, "provider_profile_id": "lattice-web-native-" + mode, "platform": "linux-x86_64", "nodes": [{"id": "build", "kind": "build", "effect_class": "build", "adapter_kind": "local", "operation_identity": identity}], "edges": [], "controls": {"cancel": True, "pause": False, "resume": False}, "bounds": {"max_nodes": 8, "max_edges": 8, "max_log_references": MAX_LOG_CHUNKS, "max_log_read_bytes": LOG_CHUNK_BYTES, "max_total_log_bytes": 1048576}}
            if mode == "success":
                verify = {key: copy.deepcopy(value) for key, value in worker.items() if key != "schema"}
                verify["steps"] = [{"node_id": "verify", "program": self.exe["python3"], "argv": ["-c", "print('actual dependent verification', flush=True)"], "environment": {"PYTHONNOUSERSITE": "1"}, "artifacts": []}]
                self.export("web-worker.ncl", verify, "worker-success-verify")
                workflow["nodes"].append({"id": "verify", "kind": "test", "effect_class": "test", "adapter_kind": "local", "operation_identity": self.digest(self.root / "worker-success-verify.json")})
                workflow["edges"] = [{"source": "build", "target": "verify"}]
            projects.append({"id": mode, "label": "Native <img src=x onerror=alert(1)> " + mode, "repository_path": str(self.repo), "reference": "refs/heads/main", "source_limits": limits, "workflow": workflow})
        profile = {"service_id": "isolated-web-exercise", "listen_address": "127.0.0.1:" + str(self.port), "origin": self.origin, "tls": {"certificate_path": str(self.root / "certificate.pem"), "private_key_path": str(self.root / "key.pem"), "min_version": "TLS1.3"}, "operator_token_file": str(self.root / "operator.token"), "worker_token_file": str(self.root / "worker.token"), "state_directory": str(self.root / "state"), "projects": projects, "bounds": {"max_request_bytes": 1048576, "max_header_bytes": 16384, "max_connections": 16, "timeout_seconds": 10, "poll_interval_ms": 100, "max_projects": 8, "max_runs": 32, "max_artifact_bytes": 65536, "max_artifacts": 16, "max_run_artifact_bytes": 262144, "max_total_artifact_bytes": 1048576, "max_log_read_bytes": 65536, "max_requests": 256, "max_events_per_run": 128, "max_state_bytes": 8388608, "max_response_bytes": 4194304}}
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
            portable = self.export("live-ci-workflow.ncl", workflow, "workflow-" + mode)
            if mode == "success":
                verify_path = self.root / "worker-success-verify.json"
                verify_identity = self.digest(verify_path)
                verify_command = shlex.join([self.exe["service"], "job", "--profile", str(verify_path), "--identity", verify_identity])
                node = copy.deepcopy(portable["nodes"][0])
                node.update(id=1, label="dependent-verification", x=1)
                for port in node["input_ports"] + node["output_ports"]:
                    port["id"] += 4
                config = copy.deepcopy(portable["configs"][0])
                config.update(node_id=1, command=verify_command)
                portable["nodes"].append(node)
                portable["configs"].append(config)
                portable["edges"] = [{"source": 1, "target": 4}]
                put(self.root / ("workflow-" + mode + ".json"), portable)
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
            routes.append({"profile_identity": native["profile_identity"], "graph_identity": native["graph"]["identity"], "workflow_name": "web-" + mode, "workflow_revision": "b3:" + revision, "effect_authority": "route_exact", "source_binding": "shell_environment", "nodes": [{"request_node_id": node["id"], "operation_identity": node["operation_identity"], "workflow_node_id": index} for index, node in enumerate(native["graph"]["nodes"])]})
        handler = {"schema": "lattice.workflow-exchange-handler-profile.v1", "socket_path": str(self.root / "lattice.sock"), "transport": {"schema": "lattice.workflow-transport-profile.v1", "limits": transport, "peers": {"allowed_user_ids": [os.getuid()]}, "non_claims": ["transport admission does not prove workflow correctness", "transport peer identity does not prove peer intent", "transport acceptance does not prove an operation executed", "transport response does not prove an external effect succeeded", "transport checks do not establish production deployment"]}, "maximum_active_executions": 8, "routes": sorted(routes, key=lambda route: (route["profile_identity"], route["graph_identity"])), "non_claims": ["handler admission does not prove workflow correctness", "route authority does not prove host confinement", "execution observation does not prove an external effect succeeded", "peer identity does not prove peer intent", "handler checks do not establish release readiness"]}
        expression = "let Contract = import " + json.dumps(self.source + "/contracts/workflow-exchange-handler-profile.ncl") + " in (" + pilot.ncl(handler) + ") | Contract.HandlerProfile"
        put(self.root / "handler.ncl", expression.encode())
        self.command([self.exe["nickel"], "export", "--format", "json", self.root / "handler.ncl"])
        self.lattice = self.spawn("lattice", [*prefix, "workflow-exchange", "serve", "--profile", self.root / "handler.ncl"])
        self.wait(lambda: (self.root / "lattice.sock").exists(), "Lattice socket")
        self.prepare_policies()
        self.start_service()
        self.report.update(origin=self.origin, source_revision=self.revision, service_profile_blake3=self.digest(self.root / "service.json"), executables={name: {"path": path, "blake3": self.digest(path)} for name, path in self.exe.items()}, lattice_source=self.source)
        print(json.dumps({"ready": True, "origin": self.origin, "root": str(self.root)}), flush=True)

    def prepare_policies(self):
        self.export("web-access.ncl", {"viewers": [{"id": "native-reader", "label": "Native read-only observer", "token_file": str(self.root / "viewer.token")}]}, "access")
        exercise = self
        class Receiver(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def do_POST(self):
                size = int(self.headers.get("Content-Length", "0"))
                if self.headers.get("Authorization") != "Bearer " + exercise.notification_secret or not 0 < size <= DOCUMENT_BYTES or len(exercise.notifications) >= MAX_LOG_CHUNKS:
                    self.send_response(403)
                    self.send_header("Content-Length", "0")
                    self.end_headers()
                    return
                body = json.loads(self.rfile.read(size))
                exercise.notifications.append({"path": self.path, "body": body})
                if self.path == "/ambiguous":
                    self.connection.shutdown(socket.SHUT_RDWR)
                    self.connection.close()
                    return
                self.send_response(204)
                self.send_header("Content-Length", "0")
                self.end_headers()

        self.notification_server = http.server.HTTPServer(("127.0.0.1", 0), Receiver)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.minimum_version = ssl.TLSVersion.TLSv1_3
        context.load_cert_chain(self.root / "certificate.pem", self.root / "key.pem")
        self.notification_server.socket = context.wrap_socket(self.notification_server.socket, server_side=True)
        self.notification_thread = threading.Thread(target=self.notification_server.serve_forever, daemon=True)
        self.notification_thread.start()
        destination = {"project_id": "success", "enabled": True, "on_success": True, "on_failure": True, "host": "127.0.0.1", "port": self.notification_server.server_port, "addresses": ["127.0.0.1"], "bearer_secret_file": str(self.root / "notification.token"), "ca_certificate_file": str(self.root / "ca.pem"), "allow_loopback": True, "allow_private": False, "timeout_ms": 5000, "max_request_bytes": 65536, "max_response_bytes": 65536, "max_resolver_candidates": 4}
        notifications = [{**destination, "id": "completion", "path": "/completion"}, {**destination, "id": "ambiguous", "path": "/ambiguous"}, {**destination, "id": "failure", "project_id": "fail", "path": "/failure"}]
        self.export("web-automation.ncl", {"triggers": [{"id": "source-event", "project_id": "success", "reference": "refs/heads/main", "repository": {"owner": "native-exercise", "repository": "source", "repository_id": 4242, "allow_fork_pull_requests": False}, "secret_file": str(self.root / "trigger.token"), "enabled": True, "push": True, "pull_request": True, "pull_request_base": "main"}], "schedules": [{"id": "periodic", "project_id": "success", "reference": "refs/heads/main", "enabled": False, "interval_ms": 3600000, "min_interval_ms": 1000, "max_interval_ms": 31536000000, "next_due_ms": None}], "notifications": notifications, "max_records": 1000, "status_limit": 100, "max_event_bytes": 1048576, "max_tick_effects": 4}, "automation")

    def start_service(self):
        before = {path.name: (path.stat().st_size, path.stat().st_mtime_ns) for path in (self.root / "state").iterdir()}
        policy = ["--profile", self.root / "service.json", "--access", self.root / "access.json", "--automation", self.root / "automation.json"]
        preflight = self.command([self.exe["service"], "check", *policy])
        assert self.operator.encode() not in preflight and self.worker.encode() not in preflight
        assert before == {path.name: (path.stat().st_size, path.stat().st_mtime_ns) for path in (self.root / "state").iterdir()}, "preflight modified persistent state"
        self.service = self.spawn("service", [self.exe["service"], "serve", *policy])
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

    def inspect_success(self, run, logs):
        assert run["edges"] == [{"source": "build", "target": "verify"}], run
        nodes = {node["id"]: node for node in run["nodes"]}
        assert set(nodes) == {"build", "verify"}, nodes
        for node in nodes.values():
            assert node["state"] == "Completed" and node["completed_at_ms"] is not None, node
            if node["started_at_ms"] is None:
                assert node["duration_ms"] is None, node
            else:
                assert node["duration_ms"] == node["completed_at_ms"] - node["started_at_ms"], node
        assert run["created_at_ms"] <= run["started_at_ms"] <= run["completed_at_ms"] and run["duration_ms"] >= 0, run
        reports = self.api("GET", f'/api/runs/{run["id"]}/reports')
        normalized = {record["kind"]: record for record in reports["reports"]}
        assert normalized["junit"]["report"]["tests"]["cases"][0]["status"] == "passed", reports
        assert reports["summary"]["tests"] == {"total": 1, "passed": 1, "failed": 0, "errors": 0, "skipped": 0}, reports
        for kind in ("cobertura", "lcov"):
            coverage = normalized[kind]["report"]["coverage"]
            assert coverage["lines_found"] == 3 and coverage["lines_hit"] == 2, coverage
        for kind in ("gate", "review", "provenance"):
            record = normalized[kind]
            evidence = record["report"]["evidence"]
            assert evidence["document"]["verdict"] == "PASS" and evidence["accepted_gate"] is False, evidence
            raw = self.api("GET", record["artifact"]["download_path"])
            assert raw["verdict"] == "PASS" and raw["schema"] == "kiln.native-exercise." + kind + ".v1", raw
        evidence = self.api("GET", f'/api/runs/{run["id"]}/evidence')
        assert evidence["execution"]["attempt_id"] == run["attempt_id"] and len(evidence["reports"]) == 3, evidence
        metadata = self.api("GET", f'/api/runs/{run["id"]}/logs/download?node_id=build&stream=stdout&metadata=true')
        downloaded = self.api("GET", metadata["download_path"])
        assert downloaded == logs["text"].encode() and metadata["bytes"] == len(downloaded), metadata
        put(self.root / "downloaded-log.bin", downloaded)
        assert metadata["blake3"] == self.digest(self.root / "downloaded-log.bin")
        found = self.api("GET", f'/api/runs/{run["id"]}/logs?node_id=build&stream=stdout&cursor=0&max_bytes={LOG_CHUNK_BYTES}&q=actual%20artifact')
        assert found["matches"] and found["text"].startswith("actual artifact bytes:"), found
        self.api("GET", f'/api/runs/{run["id"]}/logs?node_id=build&stream=stdout&cursor=999999999&max_bytes={LOG_CHUNK_BYTES}', expected=(400,))
        self.report["checks"].update(pipeline=run, reports=reports, evidence=evidence, log_search=found, retained_log=metadata)

    def inspect_active(self, run):
        def streamed():
            view = self.api("GET", f'/api/runs/{run["id"]}/logs?node_id=build&stream=stdout&cursor=0&max_bytes={LOG_CHUNK_BYTES}')
            return view if "actual command started" in view["text"] else False
        first = self.wait(streamed, "retained output before command exit")
        running = self.api("GET", "/api/runs/" + run["id"])
        assert running["outcome"] == "Pending" and running["completed_at_ms"] is None, running
        missing = self.api("GET", f'/api/runs/{run["id"]}/reports')
        assert missing["summary"]["tests"] is None and missing["summary"]["coverage"] is None, missing
        raw = b'<!DOCTYPE testsuite [<!ENTITY x SYSTEM "file:///etc/passwd">]><testsuite><testcase name="bad">&x;</testcase></testsuite>'
        put(self.root / "rejected.xml", raw)
        upload = {"run_id": run["id"], "source_revision": run["source_revision"], "attempt_id": run["attempt_id"], "execution": run["execution"], "node_id": "build", "kind": "junit", "name": "rejected.xml", "body_base64": base64.b64encode(raw).decode(), "blake3": self.digest(self.root / "rejected.xml"), "unavailable": None}
        self.api("POST", f'/api/runs/{run["id"]}/reports', upload, token=self.worker, expected=(400,))
        self.api("POST", f'/api/runs/{run["id"]}/reports', {**upload, "source_revision": "0" * 40}, token=self.worker, expected=(409,))
        assert self.api("GET", f'/api/runs/{run["id"]}/reports')["reports"] == []
        self.report["checks"]["preexit_stream"] = {"log": first, "run": running, "malformed_and_stale_reports_rejected": True}

    def complete_dashboard_checks(self, success, failure):
        viewer = self.api("GET", "/api/session", token=self.viewer)
        assert viewer["principal"]["id"] == "viewer:native-reader" and viewer["permissions"]["operate"] is False, viewer
        self.api("GET", "/api/overview", token=self.viewer)
        self.api("POST", "/api/runs", {"project_id": "success", "request_id": "viewer-denied"}, token=self.viewer, expected=(403,))
        self.api("POST", "/api/automation", {"request_id": "viewer-config-denied", "kind": "trigger", "id": "source-event", "enabled": False}, token=self.viewer, expected=(403,))
        self.api("GET", "/api/session", anonymous=True, extra_headers={"X-Kiln-Principal": "tailscale:47256370424925063", "X-Kiln-Role": "operator"}, expected=(401, 403))
        self.api("GET", "/api/runs/not-a-retained-run", expected=(404,))
        self.api("GET", "/api/runs?cursor=not-a-cursor", expected=(400,))
        self.api("POST", "/api/runs", {"project_id": "success", "request_id": "unreachable-source", "source_revision": "f" * 40}, expected=(400, 409))
        refs = self.api("GET", "/api/projects/success/refs")
        assert refs["references"] == [{"reference": "refs/heads/main", "source_revision": self.revision}], refs
        retry = self.terminal(self.api("POST", f'/api/runs/{failure["id"]}/rerun', {"request_id": "retry-failed-node", "mode": "failed"}))
        assert retry["outcome"] == "DomainFailure" and retry["selected_nodes"] == ["build"] and retry["parent_run_id"] == failure["id"], retry
        assert retry["attempt_id"] != failure["attempt_id"] and retry["source_revision"] == self.revision, retry
        self.command([self.exe["jj"], "-R", self.repo, "new", "main", "-m", "Changed exact source assertion"])
        put(self.repo / "source-value.txt", b"exact-source-artifact-v2\n")
        self.command([self.exe["jj"], "-R", self.repo, "bookmark", "set", "main", "-r", "@"])
        revision = self.command([self.exe["jj"], "-R", self.repo, "log", "--no-graph", "-r", "main", "-T", "commit_id"]).decode().strip()
        latest = self.terminal(self.api("POST", f'/api/runs/{success["id"]}/rerun', {"request_id": "run-latest-source", "mode": "latest"}))
        assert latest["source_revision"] == revision and revision != self.revision and latest["outcome"] == "DomainFailure", latest
        marker = self.root / "command-success.unstable"
        put(marker, b"controlled native repeated-observation failure")
        unstable = self.terminal(self.api("POST", f'/api/runs/{success["id"]}/rerun', {"request_id": "same-source-observation-failure", "mode": "exact"}))
        marker.unlink()
        assert unstable["source_revision"] == self.revision and unstable["outcome"] == "DomainFailure", unstable
        exact = self.terminal(self.api("POST", f'/api/runs/{success["id"]}/rerun', {"request_id": "rerun-exact-source", "mode": "exact"}))
        assert exact["source_revision"] == self.revision and exact["outcome"] == "Success" and exact["attempt_id"] != success["attempt_id"], exact
        comparison = self.api("GET", f'/api/runs/{latest["id"]}/compare?other={success["id"]}')
        changed = next(case for case in comparison["test_changes"] if case["name"] == "test_source")
        assert changed["current"] == "failed" and changed["other"] == "passed", comparison
        assert comparison["flaky_history"] == [], comparison
        instability = self.api("GET", f'/api/runs/{exact["id"]}/compare?other={unstable["id"]}')
        assert any(case["name"] == "test_source" and case["source_revision"] == self.revision and case["passed"] >= 1 and case["failed"] >= 1 for case in instability["flaky_history"]), instability
        self.report["checks"]["instability"] = instability
        self.api("GET", f'/api/runs/{success["id"]}/compare?other={failure["id"]}', expected=(409,))
        first = self.api("GET", "/api/runs?project_id=success&limit=1&order=asc")
        assert first["runs"][0]["id"] == success["id"] and first["next_cursor"], first
        second = self.api("GET", "/api/runs?project_id=success&limit=1&order=asc&cursor=" + first["next_cursor"])
        assert second["runs"][0]["ordinal"] > first["runs"][0]["ordinal"], second
        self.api("GET", "/api/runs?project_id=fail&limit=1&order=asc&cursor=" + first["next_cursor"], expected=(400,))
        filtered = self.api("GET", "/api/runs?project_id=success&outcome=failed&source_revision=" + revision)
        assert filtered["runs"] and all(run["source_revision"] == revision and run["outcome"] == "DomainFailure" for run in filtered["runs"]), filtered
        self.report["checks"].update(viewer=viewer, failed_retry=retry, latest=latest, exact=exact, comparison=comparison, pagination={"first": first, "second": second}, filters=filtered)
        self.exercise_automation(revision)
        overview = self.api("GET", "/api/overview")
        assert any(row["run_id"] == success["id"] and row["duration_ms"] == success["duration_ms"] for row in overview["duration_trend"]), overview
        activity = self.api("GET", "/api/activity?limit=100")
        activity_pages = [activity]
        cursor = activity["next_cursor"]
        for _ in range((activity["total"] + 99) // 100 - 1):
            if cursor is None:
                break
            page = self.api("GET", "/api/activity?limit=100&cursor=" + cursor)
            activity_pages.append(page)
            cursor = page["next_cursor"]
        assert cursor is None, "activity pagination exceeded its reported total"
        denied = next((item for page in activity_pages for item in page["items"] if item["kind"] == "audit" and item["audit"]["actor_id"] == "viewer:native-reader" and item["audit"]["status"] == 403), None)
        assert denied is not None, "attributable viewer denial missing from retained activity"
        self.report["checks"].update(overview=overview, activity={"pages": activity_pages, "viewer_denial": denied})
        self.lattice.terminate()
        self.lattice.wait(timeout=15)
        unavailable = self.api("POST", "/api/runs", {"project_id": "fail", "request_id": "provider-unavailable"})
        unavailable = self.api("GET", "/api/runs/" + unavailable["id"])
        operations = self.api("GET", "/api/operations")
        assert next(provider for provider in operations["providers"] if provider["project_id"] == "fail")["status"] == "unavailable", operations
        limit = json.loads((self.root / "service.json").read_bytes())["bounds"]["max_runs"]
        warning_at = (limit * 80 + 99) // 100
        total = self.api("GET", "/api/runs?limit=1")["total"]
        for index in range(total, warning_at):
            self.api("POST", "/api/runs", {"project_id": "fail", "request_id": "retention-observation-" + str(index)})
        operations = self.api("GET", "/api/operations")
        assert any(warning["resource"] == "runs" and warning["used"] >= warning_at for warning in operations["warnings"]), operations
        self.report["checks"].update(provider_unavailable=unavailable, operations=operations)

    def exercise_automation(self, revision):
        repository = {"id": 4242, "name": "source", "full_name": "native-exercise/source", "owner": {"login": "native-exercise"}}
        def delivery(event, payload, expected=(202,), delivery_id=None, signature=None):
            body = json.dumps(payload, separators=(",", ":")).encode()
            headers = {"X-GitHub-Delivery": delivery_id or str(uuid.uuid4()), "X-GitHub-Event": event, "X-Hub-Signature-256": signature or "sha256=" + hmac.new(self.trigger_secret.encode(), body, hashlib.sha256).hexdigest()}
            return self.api("POST", "/api/triggers/source-event", body, expected=expected, origin=False, anonymous=True, extra_headers=headers)
        payload = {"repository": repository, "ref": "refs/heads/main", "after": revision, "deleted": False}
        replay_id = str(uuid.uuid4())
        push = delivery("push", payload, delivery_id=replay_id)
        assert push["run_id"], push
        delivery("push", payload, delivery_id=replay_id, expected=(409,))
        delivery("push", payload, signature="sha256=" + "0" * 64, expected=(403,))
        delivery("push", {**payload, "ref": "refs/heads/not-admitted"}, expected=(403,))
        delivery("push", {**payload, "after": self.revision}, expected=(409,))
        pr = delivery("pull_request", {"action": "opened", "repository": repository, "number": 1, "pull_request": {"number": 1, "state": "open", "head": {"ref": "main", "sha": revision, "repo": repository}, "base": {"ref": "main", "sha": revision, "repo": repository}}})
        assert pr["run_id"] and pr["run_id"] != push["run_id"], pr
        self.terminal({"id": push["run_id"]})
        self.terminal({"id": pr["run_id"]})
        due = int(time.time() * 1000) + 1000
        self.api("POST", "/api/automation", {"request_id": "enable-periodic", "kind": "schedule", "id": "periodic", "enabled": True, "interval_ms": 3600000, "next_due_ms": due})
        def scheduled():
            view = self.api("GET", "/api/automation")
            return next((record for record in view["sources"] if record["kind"] == "schedule" and record["run_id"]), False)
        schedule = self.wait(scheduled, "actual due schedule dispatch")
        self.terminal({"id": schedule["run_id"]})
        self.api("POST", "/api/automation", {"request_id": "disable-periodic", "kind": "schedule", "id": "periodic", "enabled": False})
        def delivered():
            view = self.api("GET", "/api/automation")
            records = [record for record in view["deliveries"] if record["run_id"] == push["run_id"]]
            return view if any(record["status"] == "delivered" for record in records) and any(record["status"] == "ambiguous" for record in records) else False
        observed = self.wait(delivered, "actual HTTPS notification and ambiguous response")
        assert any(record["path"] == "/failure" for record in self.notifications), self.notifications
        self.service.terminate()
        self.service.wait(timeout=15)
        self.start_service()
        after = self.api("GET", "/api/automation")
        received = Counter(record["body"]["idempotency_key"]["native_id"] for record in self.notifications)
        assert all(count == 1 for count in received.values()) and len([record for record in after["sources"] if record["kind"] == "schedule"]) == 1, after
        delivery("push", payload, delivery_id=replay_id, expected=(409,))
        self.report["checks"]["automation"] = {"push": push, "pull_request": pr, "schedule": schedule, "observed": observed, "restarted": after, "https_received": self.notifications}

    def execute(self):
        self.prepare()
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
        artifact = next(artifact for artifact in artifacts if artifact["name"] == "artifact.txt")
        downloaded = self.api("GET", artifact["download_path"])
        assert downloaded == b"exact-source-artifact-v1\n", downloaded
        put(self.root / "downloaded.bin", downloaded)
        assert self.digest(self.root / "downloaded.bin") == artifact["blake3"].removeprefix("b3:")
        self.report["checks"]["success"] = {"run": success, "logs": logs, "artifact": artifact}
        self.inspect_success(success, logs)
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
        self.inspect_active(active_run)
        first_pid = (self.root / "command-slow.started").read_text()
        queued = self.api("POST", "/api/runs", {"project_id": "slow", "request_id": "durably-queued"})
        assert queued["phase"] == "queued" and queued["execution"] is None, queued
        queue = self.api("GET", "/api/operations")["queue"]
        assert any(row["run_id"] == queued["id"] and row["position"] == 1 for row in queue), queue
        invocation_count = len((self.root / "invocations.log").read_text().splitlines())
        self.service.terminate()
        self.service.wait(timeout=15)
        self.start_service()
        restored = self.api("GET", "/api/runs/" + slow["id"])
        assert restored["execution"] == active_run["execution"] and restored["attempt_id"] == active_run["attempt_id"]
        restored_queue = self.api("GET", "/api/runs/" + queued["id"])
        assert restored_queue["phase"] == "queued" and restored_queue["attempt_id"] == queued["attempt_id"], restored_queue
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
        def advanced():
            view = self.api("GET", "/api/runs/" + queued["id"])
            return view if view["execution"] and view["phase"] == "active" and (self.root / "command-slow.started").read_text() != first_pid else False
        advanced_run = self.wait(advanced, "queued command starts exactly after prior cancellation")
        advanced_run = self.api("GET", "/api/runs/" + queued["id"])
        self.api("POST", f'/api/runs/{queued["id"]}/control', {"request_id": "cancel-advanced", "attempt_id": advanced_run["attempt_id"], "expected_revision": advanced_run["revision"], "expected_sequence": advanced_run["sequence"], "execution": advanced_run["execution"], "action": "cancel"})
        assert self.terminal(queued)["outcome"] == "Cancelled"
        self.report["checks"]["queue"] = {"queued": queued, "restored": restored_queue, "advanced": advanced_run}
        self.complete_dashboard_checks(success, failure)
        if self.args.browser_hold:
            put(self.root / "browser-ready.json", {"origin": self.origin, "success_run": success["id"], "latest_run": self.report["checks"]["latest"]["id"]})
            print(json.dumps({"browser_ready": True, "origin": self.origin, "root": str(self.root)}), flush=True)
            self.wait(lambda: (self.root / "browser-done.json").exists(), "browser evidence marker", seconds=900)
            self.report["browser"] = json.loads((self.root / "browser-done.json").read_bytes())
        self.report["passed"] = True

    def close(self):
        failures = []
        if self.notification_server is not None:
            self.notification_server.shutdown()
            self.notification_server.server_close()
            self.notification_thread.join(timeout=5)
        for process in reversed(self.processes):
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait(timeout=5)
                    failures.append("forced process termination")
        secrets = ("operator.token", "worker.token", "viewer.token", "trigger.token", "notification.token", "key.pem", "ca-key.pem")
        for name in secrets:
            (self.root / name).unlink(missing_ok=True)
        self.report["retirement"] = {"all_processes_exited": all(process.poll() is not None for process in self.processes), "credentials_removed": all(not (self.root / name).exists() for name in secrets), "failures": failures}
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
