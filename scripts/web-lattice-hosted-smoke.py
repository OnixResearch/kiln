#!/usr/bin/env python3
"""Real HTTPS Web -> genuine native Lattice -> authenticated hosted smoke.

Requires prebuilt kiln-hosted, ordinary kiln-web-service, genuine lattice and
its reviewed source cohort (--lattice-source), Nickel, jj, git, openssl, b3sum,
systemd-run and systemctl. Never changes host configuration or waives safety.
The default proves the exact untrusted host refusal. --trusted-execution also
requires real controlled guest success, stdout, live cancellation and cleanup.
Use fresh short private scratch outside /tmp. Only whitelisted summary evidence
is exported; retained private scratch is not a portable or public artifact.
"""
# r[verify kiln.web_hosted.binding] r[verify kiln.web_hosted.admission]
# r[verify kiln.web_hosted.replay] r[verify kiln.web_hosted.adapter]
# r[impl kiln.web_hosted.qualification] r[verify kiln.web_hosted.qualification]
import argparse
import copy
import hashlib
import hmac
import json
import os
from pathlib import Path
import re
import shutil
import signal
import socket
import shlex
import sqlite3
import ssl
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

REPO = Path(__file__).resolve().parent.parent
DOCUMENT_BYTES = 8 * 1024 * 1024
WAIT_SECONDS = 90
COMMAND_SECONDS = 300
STOP_SECONDS = 30
POLL_SECONDS = 0.1
CANCEL_GUEST_SECONDS = 60
CANCEL_TIMEOUT_MS = 120000
NATIVE_TEARDOWN_MILLIS = 60000
JOB_CLEANUP_MILLIS = 45000
ORG = "web-smoke"
PROJECT = "fixture"
POLICY = "fixture-web"
LIMITS = dict(max_entries=128, max_path_bytes=1024, max_file_bytes=65536, max_total_bytes=1048576)


def nickel_literal(value):
    """Render generated JSON-compatible data as a Nickel record literal."""
    if isinstance(value, bool):
        return 'true' if value else 'false'
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        require('%{' not in value, 'generated Nickel string would interpolate')
        return json.dumps(value)
    if isinstance(value, list):
        return '[' + ', '.join(nickel_literal(item) for item in value) + ']'
    if isinstance(value, dict):
        fields = []
        for key, item in value.items():
            require(re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*', key or ''), 'generated Nickel field name invalid')
            fields.append(key + ' = ' + nickel_literal(item))
        return '{ ' + ', '.join(fields) + ' }'
    raise RuntimeError('generated Nickel value unsupported')


def require(value, message):
    if not value:
        raise RuntimeError(message)


def put(path, value):
    raw = value if isinstance(value, bytes) else (json.dumps(value, indent=2) + "\n").encode()
    with path.open("wb") as stream:
        stream.write(raw)
    path.chmod(0o600)


class Exercise:
    def __init__(self, args):
        self.args = args
        self.root = Path(args.root).resolve()
        self.evidence = Path(args.evidence).resolve()
        require(not self.root.exists(), "--root must be fresh")
        require(Path('/tmp') not in self.root.parents and self.root != Path('/tmp'), "scratch must be outside /tmp")
        require(self.evidence != self.root and self.root not in self.evidence.parents, "evidence must be outside private scratch")
        self.root.mkdir(mode=0o700, parents=False)
        self.owned_units = set()
        self.evidence.mkdir(mode=0o700, parents=True, exist_ok=True)
        self.processes = []
        self.generation = 0
        self.report = {"schema": "kiln.web-lattice-hosted-smoke.v1", "status": "running", "checks": {}, "vm_qualification": "pending", "non_claims": ["not production deployment or hostile-isolation qualification", "loopback Iroh does not prove WAN/relay interoperability", "host refusal is not VM execution qualification", "no host maintenance, security waiver, publication or lifecycle acceptance", "prior direct-route receipts are superseded history, not native Lattice evidence"]}
        self.report['hostile_isolation_claim'] = False
        self.exe = {}
        for name in ('nickel', 'jj', 'git', 'openssl', 'b3sum', 'systemd-run', 'systemctl', 'env'):
            found = shutil.which(name)
            require(found, 'required executable unavailable: ' + name)
            self.exe[name] = str(Path(found).parent.resolve(strict=True) / Path(found).name)
        self.exe.update(hosted=str(Path(args.hosted).resolve(strict=True)), web=str(Path(args.web).resolve(strict=True)), lattice=str(Path(args.lattice).resolve(strict=True)))
        self.lattice_source = Path(args.lattice_source).resolve(strict=True)
        self.exe['bash'] = str(Path(shutil.which('bash')).resolve(strict=True))
        self.lattice_generation = 0
        for directory in ('home', 'tmp', 'web-state', 'hosted-state', 'work'):
            (self.root / directory).mkdir(mode=0o700)
        self.env = {"HOME": str(self.root / 'home'), "XDG_CONFIG_HOME": str(self.root / 'home/config'), "XDG_CACHE_HOME": str(self.root / 'home/cache'), "TMPDIR": str(self.root / 'tmp'), "PATH": os.environ['PATH'], "LANG": "C.UTF-8"}
        # A qualified execution host needs its existing user manager; never create one.
        for key in ('XDG_RUNTIME_DIR', 'DBUS_SESSION_BUS_ADDRESS'):
            if key in os.environ:
                self.env[key] = os.environ[key]
        self.operator, self.worker, self.viewer, self.secret = (os.urandom(32).hex() for _ in range(4))
        for name, token in [('operator', self.operator), ('worker', self.worker), ('viewer', self.viewer), ('trigger', self.secret)]:
            put(self.root / (name + '.token'), token.encode())
        with socket.socket() as listener:
            listener.bind(('127.0.0.1', 0))
            self.port = listener.getsockname()[1]
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as listener:
            listener.bind(('127.0.0.1', 0))
            self.hosted_address = '127.0.0.1:' + str(listener.getsockname()[1])
        self.origin = 'https://127.0.0.1:' + str(self.port)
        self.repo = self.root / 'source'
        self.hosted_repo = self.root / 'hosted-source'

    def command(self, argv, allowed=(0,), **kwargs):
        result = subprocess.run(list(map(str, argv)), cwd=self.root, env=self.env, capture_output=True, timeout=COMMAND_SECONDS, **kwargs)
        require(len(result.stdout) + len(result.stderr) <= DOCUMENT_BYTES, 'command output exceeds bound')
        # Never include argv (may contain private paths) or process output in exported errors.
        require(result.returncode in allowed, 'command failed: ' + Path(str(argv[0])).name)
        return result.stdout

    def export(self, builder, inputs, name):
        put(self.root / (name + '-input.json'), inputs)
        put(self.root / (name + '.ncl'), ('(import ' + json.dumps(str(REPO / 'config' / builder)) + ') (import ' + json.dumps(str(self.root / (name + '-input.json'))) + ')').encode())
        raw = self.command([self.exe['nickel'], 'export', '--format', 'json', self.root / (name + '.ncl')])
        put(self.root / (name + '.json'), raw)
        return json.loads(raw)

    def digest(self, value, raw=False):
        argv = [self.exe['b3sum']]
        return self.command(argv if raw else [*argv, value], **({'input': value} if raw else {})).decode().split()[0]

    def spawn(self, name, argv):
        path = self.root / (name + '.log')
        with path.open('wb') as output:
            process = subprocess.Popen(list(map(str, argv)), cwd=self.root, env=self.env, stdout=output, stderr=output, start_new_session=True)
        path.chmod(0o600)
        self.processes.append(process)
        return process

    def stop(self, process):
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=STOP_SECONDS)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=STOP_SECONDS)
                raise RuntimeError('owned child required forced termination')

    def wait(self, predicate, description):
        deadline = time.monotonic() + WAIT_SECONDS
        while time.monotonic() < deadline:
            value = predicate()
            if value:
                return value
            time.sleep(POLL_SECONDS)
        raise RuntimeError('timed out: ' + description)

    def api(self, method, target, body=None, token=None, expected=(200, 201, 202), signed=None):
        raw = body if isinstance(body, bytes) else None if body is None else json.dumps(body).encode()
        headers = {'Authorization': 'Bearer ' + (self.operator if token is None else token), 'Origin': self.origin}
        if signed is not None:
            headers = signed
        if raw is not None:
            headers['Content-Type'] = 'application/json'
        request = urllib.request.Request(self.origin + target, data=raw, headers=headers, method=method)
        try:
            with urllib.request.urlopen(request, context=self.context, timeout=WAIT_SECONDS) as response:
                status, data = response.status, response.read(DOCUMENT_BYTES + 1)
        except urllib.error.HTTPError as error:
            status, data = error.code, error.read(DOCUMENT_BYTES + 1)
        require(status in expected and len(data) <= DOCUMENT_BYTES, 'unexpected HTTPS response: ' + method + ' ' + target + ' status=' + str(status))
        return json.loads(data)

    def network(self, name):
        return dict(key_file=str(self.root / (name + '.key')), bind_address=self.hosted_address if name == 'server' else '127.0.0.1:0', relay_urls=[])

    def start_hosted(self):
        self.generation += 1
        name = 'hosted-' + str(self.generation)
        self.hosted = self.spawn(name, [self.exe['hosted'], 'serve', '--profile', self.root / 'hosted.json'])
        def ready():
            require(self.hosted.poll() is None, 'hosted server exited before readiness')
            for line in (self.root / (name + '.log')).read_bytes().splitlines():
                try:
                    value = json.loads(line)
                    if isinstance(value, dict) and 'id' in value and 'addrs' in value:
                        return value
                except (ValueError, UnicodeDecodeError):
                    pass
            return False
        self.remote = self.wait(ready, 'real hosted Iroh endpoint')
        for name in ('owner', 'lattice-peer', 'stranger'):
            self.export('hosted-client.ncl', dict(network=self.network(name), remote=self.remote, organization=ORG, listen_address='127.0.0.1:0'), name + '-client')

    def rpc(self, action, value, peer='owner', error=None, request_id=None):
        put(self.root / 'request.json', dict(schema='kiln.hosted.v1', organization=ORG, request_id=request_id or str(uuid.uuid4()), action=action, input=value))
        output = self.command([self.exe['hosted'], 'rpc', '--profile', self.root / (peer + '-client.json'), '--request', self.root / 'request.json'], allowed=(0, 1))
        response = json.loads(output)
        if error is not None:
            require(response.get('result') is None and error in response.get('error', ''), 'hosted negative did not return expected refusal: ' + error)
            return response['error']
        require(response.get('error') is None and response.get('result') is not None, 'hosted RPC refused: ' + action)
        return response['result']

    def native_command(self, *args):
        return [self.exe['lattice'], '--config', self.root / 'lattice.ncl', *args]

    def start_lattice(self):
        self.lattice_generation += 1
        socket_path = self.root / 'l.sock'
        if socket_path.exists():
            require(socket_path.is_socket(), 'native socket path is not a socket')
            socket_path.unlink()
        self.lattice = self.spawn('lattice-' + str(self.lattice_generation), self.native_command('workflow-exchange', 'serve', '--profile', self.root / 'handler.ncl'))
        def ready():
            require(self.lattice.poll() is None, 'genuine native Lattice exited')
            return socket_path.is_socket()
        self.wait(ready, 'genuine native workflow-exchange listener')

    def prepare_native(self, web_inputs, transport):
        process_limits = dict(stdin_max_bytes=1048576, stdout_max_bytes=1048576, stderr_max_bytes=1048576, command_agent_timeout_secs=180, poll_interval_millis=10, teardown_grace_millis=NATIVE_TEARDOWN_MILLIS)
        put(self.root / 'lattice-config.json', dict(data_dir=str(self.root / 'lattice-store'), shell=self.exe['bash'], default_timeout_secs=180, max_parallelism=3, process_limits=process_limits))
        put(self.root / 'lattice.ncl', ('import ' + json.dumps(str(self.root / 'lattice-config.json'))).encode())
        self.native_profiles = {}
        for project, policy in zip(web_inputs['projects'], self.hosted_profile['lattice_execution']):
            project_id = project['id']
            helper = dict(schema='kiln.hosted.lattice-job.v1', web=self.web_connection, network=self.network('lattice-peer'), remote=self.remote, organization=ORG, project_id=project_id, repository_id=self.repository_id, node_id='build', policy_id=policy['id'], lookup_timeout_ms=15000, poll_interval_ms=100, maximum_run_ms=180000, cleanup_timeout_ms=JOB_CLEANUP_MILLIS)
            helper_path = self.root / ('job-' + project_id + '.json')
            put(helper_path, helper)
            operation = self.digest(helper_path)
            project['workflow']['nodes'] = [dict(id='build', kind='build', effect_class='build', adapter_kind='remote', operation_identity=operation)]
            policy.update(workflow_policy_id=project['workflow']['policy_id'], node_id='build', operation_identity=operation, web_service=self.web_connection)
            command = "printf '%s\\n' \"$LATTICE_SOURCE_REVISION\" >> " + shlex.quote(str(self.root / ('invocations-' + project_id))) + '; exec ' + shlex.join([self.exe['hosted'], 'lattice-job', '--profile', str(helper_path), '--identity', operation]) + ' 2>' + shlex.quote(str(self.root / ('job-' + project_id + '.stderr')))
            workflow_name = 'native-hosted-' + project_id
            self.export('live-ci-workflow.ncl', dict(name=workflow_name, command=command, timeout_seconds=180, created='2026-11-06T00:00:00Z'), 'workflow-' + project_id)
            workflow_path = self.root / ('workflow-' + project_id + '.ncl')
            put(workflow_path, ('import ' + json.dumps(str(self.root / ('workflow-' + project_id + '.json')))).encode())
            self.command(self.native_command('import', workflow_path))
            observe = self.root / ('observe-' + project_id + '.ncl')
            expression = 'let base = import ' + json.dumps(str(self.lattice_source / 'examples/orchestrations/llm-review-loop.ncl')) + ' in base & { objective | force = "Observe genuine saved hosted workflow", workflow | force = { workflow_name = ' + json.dumps(workflow_name) + ', content_hash = ' + json.dumps(operation) + ' }, triggers | force = [] }'
            put(observe, expression.encode())
            observed = json.loads(self.command(self.native_command('orchestration', 'validate', observe, '--output', 'json'), allowed=(0, 1)))
            require(observed.get('workflow_revision') and not observed.get('workflow_diagnostics'), 'native producer did not observe valid saved workflow')
            revision = observed['workflow_revision']['content_hash']
            put(observe, expression.replace(operation, revision, 1).encode())
            self.command(self.native_command('orchestration', 'validate', observe, '--output', 'json'))
            self.native_profiles[project_id] = dict(workflow_name=workflow_name, workflow_revision='b3:' + revision, operation_identity=operation)
        self.export('web-service.ncl', web_inputs, 'web')
        routes = []
        for project in web_inputs['projects']:
            native = json.loads(self.command([self.exe['web'], 'reviewed-request', '--profile', self.root / 'web.json', '--project', project['id']]))
            put(self.root / ('native-' + project['id'] + '.json'), native)
            facts = self.native_profiles[project['id']]
            route = dict(profile_identity=native['profile_identity'], graph_identity=native['graph']['identity'], workflow_name=facts['workflow_name'], workflow_revision=facts['workflow_revision'], effect_authority='route_exact', source_binding='shell_environment', nodes=[dict(request_node_id=node['id'], operation_identity=node['operation_identity'], workflow_node_id=0) for node in native['graph']['nodes']])
            routes.append(route)
            facts.update(profile_identity=route['profile_identity'], graph_identity=route['graph_identity'])
        handler = dict(schema='lattice.workflow-exchange-handler-profile.v1', socket_path=str(self.root / 'l.sock'), transport=dict(schema='lattice.workflow-transport-profile.v1', limits=transport, peers=dict(allowed_user_ids=[os.getuid()]), non_claims=['transport admission does not prove workflow correctness', 'transport peer identity does not prove peer intent', 'transport acceptance does not prove an operation executed', 'transport response does not prove an external effect succeeded', 'transport checks do not establish production deployment']), maximum_active_executions=3, routes=sorted(routes, key=lambda route: (route['profile_identity'], route['graph_identity'])), non_claims=['handler admission does not prove workflow correctness', 'route authority does not prove host confinement', 'execution observation does not prove an external effect succeeded', 'peer identity does not prove peer intent', 'handler checks do not establish release readiness'])
        put(self.root / 'handler-input.json', handler)
        # Nickel 0.18 panics applying this record contract to an imported JSON
        # thunk. Emit the same typed value as a literal so the producer's
        # contract still validates it without the import-evaluator regression.
        put(self.root / 'handler.ncl', ('let Contract = import ' + json.dumps(str(self.lattice_source / 'contracts/workflow-exchange-handler-profile.ncl')) + ' in (' + nickel_literal(handler) + ') | Contract.HandlerProfile').encode())
        put(self.root / 'handler.json', self.command([self.exe['nickel'], 'export', '--format', 'json', self.root / 'handler.ncl']))
        self.report['native_profiles'] = self.native_profiles
        self.report['handler_profile_blake3'] = self.digest(self.root / 'handler.json')
        self.report['native_process_limits'] = process_limits
        self.start_lattice()

    def native_evidence(self, run_id, project_id):
        bound = self.api('GET', '/api/runs/' + run_id + '/execution-binding', token=self.worker)
        invocation = self.root / ('invocations-' + project_id)
        require(invocation.read_text().splitlines() == [bound['binding']['source']['exact_revision']['native_id']], 'native saved shell invocation missing, duplicated or wrong source')
        with sqlite3.connect((self.root / 'web-state/service.sqlite').as_uri() + '?mode=ro', uri=True) as db:
            stored = json.loads(db.execute('SELECT state FROM service_state WHERE id=1').fetchone()[0])['runs'][run_id]
        facts = dict(self.native_profiles[project_id], execution_binding=bound, execution=bound['execution'], observation=stored['observation'], reviewed_request_scope='template profile and graph only; not a captured dispatched request identity')
        self.report.setdefault('native_executions', {})[run_id] = facts
        return facts

    def start_web(self):
        self.web = self.spawn('web-' + str(self.generation), [self.exe['web'], 'serve', '--profile', self.root / 'web.json', '--access', self.root / 'access.json', '--automation', self.root / 'automation.json'])
        def ready():
            require(self.web.poll() is None, 'ordinary Web service exited before readiness')
            try:
                return self.api('GET', '/healthz') == {'status': 'ok'}
            except (OSError, urllib.error.URLError) as error:
                put(self.root / 'web-ready.err', type(error).__name__.encode())
                return False
        self.wait(ready, 'real TLS Web server')

    def ledger(self):
        with sqlite3.connect((self.root / 'hosted-state/hosted.sqlite').as_uri() + '?mode=ro', uri=True) as db:
            bindings = db.execute('SELECT run_id,identity,attempt,response,web_approval_verified,source_verified,execution_identity,sequence FROM hosted_lattice_bindings ORDER BY run_id').fetchall()
            counts = {table: db.execute('SELECT COUNT(*) FROM ' + table).fetchone()[0] for table in ('hosted_execution', 'finance_reservations', 'finance_usage')}
        return {run: dict(identity=identity, attempt=attempt, response=None if response is None else json.loads(response), web_approval_verified=bool(approval), source_verified=bool(source), execution_identity=execution_identity, sequence=sequence) for run, identity, attempt, response, approval, source, execution_identity, sequence in bindings}, counts

    def binding(self, run_id):
        bound = self.api('GET', '/api/runs/' + run_id + '/execution-binding', token=self.worker)
        require(bound['execution'] is not None and bound['execution']['provider_namespace'] == 'lattice', 'missing genuine native execution')
        return dict(policy_id=POLICY, plan=bound['plan'], binding=bound['binding'], lattice_execution=bound['execution'], node_id='build')

    def fixture(self):
        # The Rust HTTPS worker uses rustls, which rejects a CA certificate served
        # as the server end entity. Issue a leaf from a disposable local CA.
        extensions = self.root / 'tls.ext'
        put(extensions, b'basicConstraints=critical,CA:FALSE\nextendedKeyUsage=serverAuth\nsubjectAltName=IP:127.0.0.1\nsubjectKeyIdentifier=hash\nauthorityKeyIdentifier=keyid:always\n')
        self.command([self.exe['openssl'], 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1', '-subj', '/CN=Kiln disposable local CA', '-addext', 'basicConstraints=critical,CA:TRUE', '-addext', 'keyUsage=critical,keyCertSign,cRLSign', '-keyout', self.root / 'ca.key', '-out', self.root / 'ca.pem'])
        self.command([self.exe['openssl'], 'req', '-new', '-newkey', 'rsa:2048', '-nodes', '-subj', '/CN=127.0.0.1', '-keyout', self.root / 'tls.key', '-out', self.root / 'tls.csr'])
        self.command([self.exe['openssl'], 'x509', '-req', '-in', self.root / 'tls.csr', '-CA', self.root / 'ca.pem', '-CAkey', self.root / 'ca.key', '-CAcreateserial', '-days', '1', '-extfile', extensions, '-out', self.root / 'tls.pem'])
        for name in ('ca.key', 'tls.key'):
            (self.root / name).chmod(0o600)
        self.context = ssl.create_default_context(cafile=str(self.root / 'ca.pem'))
        self.command([self.exe['jj'], 'git', 'init', '--colocate', self.repo])
        for key, value in [('user.name', 'Kiln local smoke'), ('user.email', 'kiln@example.invalid')]:
            self.command([self.exe['jj'], '-R', self.repo, 'config', 'set', '--repo', key, value])
        put(self.repo / 'value.txt', b'approved-exact-source-v1\n')
        for argv in (['file', 'track', 'root:value.txt'], ['describe', '-m', 'Disposable exact source'], ['bookmark', 'create', 'main', '-r', '@'], ['new', 'main']):
            self.command([self.exe['jj'], '-R', self.repo, *argv])
        self.revision = self.command([self.exe['jj'], '-R', self.repo, 'log', '--no-graph', '-r', 'main', '-T', 'commit_id']).decode().strip()
        self.command([self.exe['jj'], 'git', 'clone', '--colocate', self.repo, self.hosted_repo])
        self.repository_id = self.digest(os.fsencode(self.repo.resolve()), raw=True)
        self.peers = {name: self.command([self.exe['hosted'], 'keygen', '--key', self.root / (name + '.key')]).decode().strip() for name in ('server', 'owner', 'lattice-peer', 'stranger')}
        self.hosted_profile = dict(schema='kiln.hosted.v1', service_id='isolated-web-lattice-hosted', state_directory=str(self.root / 'hosted-state'), network=self.network('server'), bootstrap_peer=self.peers['owner'], bootstrap_organization=ORG, operator_peers=[self.peers['owner']], source_profiles=[], operations=None, secrets=None, execution=None, fleet=None, lattice_execution=[])
        put(self.root / 'hosted.json', self.hosted_profile)
        self.start_hosted()
        self.rpc('finance.account.configure', dict(request_id='account', expected_revision=0, currency='usd', cap_minor=100000, request_max_minor=100000, max_concurrent=1))
        price = self.rpc('finance.price.admit', {'class': 'linux', 'currency': 'usd', 'unit_minor': 1, 'unit_ms': 1000, 'max_units': 1000})
        pipeline = {'schema': 'kiln.hosted.pipeline.v1', 'name': 'Exact source smoke', 'triggers': ['manual'], 'jobs': {'build': {'runner': 'linux', 'timeout_ms': 60000, 'steps': [{'kind': 'run', 'run': "read -r value < value.txt; test \"$value\" = approved-exact-source-v1; printf 'real approved guest source verified\\n'"}]}}}
        reviewed = self.rpc('pipelines.review', dict(project=PROJECT, name='checks', expected_revision=0, config=pipeline))
        put(self.root / 'pipeline.json', pipeline)
        compiled = json.loads(self.command([self.exe['hosted'], 'compile', '--input', self.root / 'pipeline.json']))
        require(len(compiled['jobs']) == 1, 'exact-source smoke requires one reviewed job')
        put(self.root / 'compiled.json', compiled)
        projects = [PROJECT]
        if self.args.trusted_execution:
            projects.append('trusted-fixture')
            self.command([self.exe['jj'], '-R', self.repo, 'bookmark', 'create', 'trusted-fixture', '-r', 'main'])
            trusted_pipeline = copy.deepcopy(pipeline)
            trusted_pipeline['name'] = 'Controlled trusted source smoke'
            trusted_pipeline['jobs']['build']['runner'] = 'trusted-linux'
            trusted_pipeline['jobs']['build']['steps'] = [{'kind': 'run', 'run': "read -r value < value.txt && test \"$value\" = approved-exact-source-v1 && printf 'real trusted guest source=%s\\n' \"$value\""}]
            trusted_reviewed = self.rpc('pipelines.review', dict(project='trusted-fixture', name='trusted-checks', expected_revision=0, config=trusted_pipeline))
            put(self.root / 'trusted-pipeline.json', trusted_pipeline)
            trusted_compiled = json.loads(self.command([self.exe['hosted'], 'compile', '--input', self.root / 'trusted-pipeline.json']))
            require(len(trusted_compiled['jobs']) == 1, 'controlled trusted smoke requires one reviewed job')
            put(self.root / 'trusted-compiled.json', trusted_compiled)
            projects.append('cancel-fixture')
            self.command([self.exe['jj'], '-R', self.repo, 'bookmark', 'create', 'cancel-fixture', '-r', 'main'])
            cancel_pipeline = copy.deepcopy(trusted_pipeline)
            cancel_pipeline['name'] = 'Controlled live cancellation smoke'
            cancel_pipeline['jobs']['build']['timeout_ms'] = CANCEL_TIMEOUT_MS
            cancel_pipeline['jobs']['build']['steps'] = [{'kind': 'run', 'run': "read -r value < value.txt && test \"$value\" = approved-exact-source-v1 && printf 'controlled cancellation guest ready\\n' && sleep " + str(CANCEL_GUEST_SECONDS)}]
            cancel_reviewed = self.rpc('pipelines.review', dict(project='cancel-fixture', name='cancel-checks', expected_revision=0, config=cancel_pipeline))
            put(self.root / 'cancel-pipeline.json', cancel_pipeline)
            cancel_compiled = json.loads(self.command([self.exe['hosted'], 'compile', '--input', self.root / 'cancel-pipeline.json']))
            require(len(cancel_compiled['jobs']) == 1, 'controlled cancellation requires one reviewed job')
            self.cancel_node_id = cancel_compiled['jobs'][0]['id']
            put(self.root / 'cancel-compiled.json', cancel_compiled)
        invite = self.rpc('accounts.invite', dict(peer=self.peers['lattice-peer'], role='member', projects=sorted(projects), expires_at_ms=int(time.time() * 1000) + 600000, nonce='native-lattice-job'))
        self.rpc('accounts.accept', {'invitation': invite['invitation']}, peer='lattice-peer')
        self.stop(self.hosted)
        manifest_path = Path(self.args.guest).resolve(strict=True)
        if manifest_path.is_dir():
            manifest_path /= 'manifest.json'
        guest = json.loads(manifest_path.read_bytes())
        image = lambda name: {key: guest[name][key] for key in ('path', 'blake3')}
        tool = lambda name: dict(path=self.exe[name], blake3=self.digest(self.exe[name]))
        runner = dict(kind='firecracker', firecracker=image('firecracker'), kernel=image('kernel'), initrd=image('initrd'), guest_shell=guest['guest_shell'], vcpus=1, memory_mib=512, boot_timeout_ms=30000, source_image_mib=16, untrusted_capable=True, trust_domain='isolated-web-smoke', host_identity='isolated-web-smoke-host')
        tenant = self.root / 'work' / ORG / PROJECT
        tenant.mkdir(mode=0o700, parents=True)
        self.hosted_profile['execution'] = dict(systemd_run=tool('systemd-run'), systemctl=tool('systemctl'), env=tool('env'), manager_runtime_directory='/run/user/' + str(os.getuid()), work_root=str(self.root / 'work'), trusted_projects=[dict(organization=ORG, project=PROJECT)], pricing=[dict(organization=ORG, runner='linux', price_id=price['price_id'], **{'class': 'linux'}, max_units=1000)], runners={'linux': runner}, maximum_concurrent=1, memory_bytes=1073741824, processes=64, cpu_percent=100, maximum_timeout_ms=120000, maximum_output_bytes=262144)
        self.hosted_profile['lattice_execution'] = [dict(id=POLICY, peer=self.peers['lattice-peer'], organization=ORG, web_project=PROJECT, web_repository=self.repository_id, repository_path=str(self.hosted_repo), source_limits=LIMITS, hosted_project=PROJECT, pipeline_id=reviewed['identity'], price_id=price['price_id'], **{'class': 'linux'})]
        if self.args.trusted_execution:
            execution = self.hosted_profile['execution']
            trusted_runner = copy.deepcopy(runner)
            trusted_runner.update(untrusted_capable=False, trust_domain='controlled-trusted-smoke')
            execution['runners']['trusted-linux'] = trusted_runner
            execution['trusted_projects'].append(dict(organization=ORG, project='trusted-fixture'))
            execution['pricing'].append(dict(organization=ORG, runner='trusted-linux', price_id=price['price_id'], **{'class': 'linux'}, max_units=1000))
            (self.root / 'work' / ORG / 'trusted-fixture').mkdir(mode=0o700)
            trusted_policy = copy.deepcopy(self.hosted_profile['lattice_execution'][0])
            trusted_policy.update(id='trusted-hosted', web_project='trusted-fixture', hosted_project='trusted-fixture', pipeline_id=trusted_reviewed['identity'])
            self.hosted_profile['lattice_execution'].append(trusted_policy)
            execution['trusted_projects'].append(dict(organization=ORG, project='cancel-fixture'))
            (self.root / 'work' / ORG / 'cancel-fixture').mkdir(mode=0o700)
            cancel_policy = copy.deepcopy(trusted_policy)
            cancel_policy.update(id='cancel-hosted', web_project='cancel-fixture', hosted_project='cancel-fixture', pipeline_id=cancel_reviewed['identity'])
            self.hosted_profile['lattice_execution'].append(cancel_policy)
        self.web_connection = dict(service_origin=self.origin, tls_ca_path=str(self.root / 'ca.pem'), token_file=str(self.root / 'worker.token'), http_timeout_ms=5000, maximum_response_bytes=1048576)
        transport = dict(maximum_frame_bytes=1048576, maximum_log_bytes=1048576, maximum_connections=10000, read_deadline_milliseconds=10000, write_deadline_milliseconds=10000)
        workflow = dict(socket_path=str(self.root / 'l.sock'), allowed_user_ids=[os.getuid()], transport_limits=transport, adapter_limits=dict(maximum_request_bytes=1048576, maximum_response_bytes=1048576), policy_id='web-lattice-untrusted', provider_profile_id='native-hosted-untrusted', platform='linux-x86_64', nodes=[], edges=[], controls=dict(cancel=True, pause=False, resume=False), bounds=dict(max_nodes=8, max_edges=8, max_log_references=64, max_log_read_bytes=65536, max_total_log_bytes=1048576))
        bounds = dict(max_request_bytes=1048576, max_header_bytes=16384, max_connections=16, timeout_seconds=90, poll_interval_ms=100, max_projects=8, max_runs=32, max_artifact_bytes=65536, max_artifacts=16, max_run_artifact_bytes=262144, max_total_artifact_bytes=1048576, max_log_read_bytes=65536, max_requests=256, max_events_per_run=128, max_state_bytes=8388608, max_response_bytes=4194304)
        web_inputs = dict(service_id='isolated-web-lattice-hosted', listen_address='127.0.0.1:' + str(self.port), origin=self.origin, tls=dict(certificate_path=str(self.root / 'tls.pem'), private_key_path=str(self.root / 'tls.key'), min_version='TLS1.3'), operator_token_file=str(self.root / 'operator.token'), worker_token_file=str(self.root / 'worker.token'), state_directory=str(self.root / 'web-state'), projects=[dict(id=PROJECT, label='Disposable native hosted approval', repository_path=str(self.repo), reference='refs/heads/main', source_limits=LIMITS, workflow=workflow, untrusted_capable=True)], bounds=bounds)
        if self.args.trusted_execution:
            trusted_workflow = copy.deepcopy(workflow)
            trusted_workflow.update(policy_id='web-lattice-trusted', provider_profile_id='native-hosted-trusted')
            trusted_project = copy.deepcopy(web_inputs['projects'][0])
            trusted_project.update(id='trusted-fixture', label='Controlled local trusted source', reference='refs/heads/trusted-fixture', workflow=trusted_workflow, untrusted_capable=False)
            web_inputs['projects'].append(trusted_project)
            cancel_workflow = copy.deepcopy(trusted_workflow)
            cancel_workflow.update(policy_id='web-lattice-cancel', provider_profile_id='native-hosted-cancel')
            cancel_project = copy.deepcopy(trusted_project)
            cancel_project.update(id='cancel-fixture', label='Controlled live cancellation', reference='refs/heads/cancel-fixture', workflow=cancel_workflow)
            web_inputs['projects'].append(cancel_project)
        self.prepare_native(web_inputs, transport)
        self.hosted_profile = self.export('hosted-service.ncl', self.hosted_profile, 'hosted')
        self.start_hosted()
        self.export('web-service.ncl', web_inputs, 'web')
        self.export('web-access.ncl', dict(viewers=[dict(id='observer', label='Read only', token_file=str(self.root / 'viewer.token'))]), 'access')
        self.export('web-automation.ncl', dict(triggers=[dict(id='fork', project_id=PROJECT, reference='refs/heads/main', repository=dict(owner='fixture', repository='source', repository_id=4242, allow_fork_pull_requests=False, untrusted_pull_requests='approve'), secret_file=str(self.root / 'trigger.token'), enabled=True, push=False, pull_request=True, pull_request_base='main')], schedules=[], notifications=[], max_records=1000, status_limit=100, max_event_bytes=1048576, max_tick_effects=4), 'automation')
        self.start_web()
        self.api('POST', '/api/projects', dict(project_id=PROJECT, request_id='enroll'))
        self.report['inputs'] = dict(source_commit=self.revision, hosted_peer=self.peers['server'], lattice_peer=self.peers['lattice-peer'], source_checkout_distinct=self.repo != self.hosted_repo, guest_manifest_blake3=self.digest(manifest_path), lattice_source=str(self.lattice_source), binaries={name: self.digest(self.exe[name]) for name in ('hosted', 'web', 'lattice')})

    def event(self, revision, number=1, action='opened'):
        base = dict(id=4242, name='source', full_name='fixture/source', owner=dict(login='fixture'))
        fork = dict(id=4243, name='source', full_name='fork/source', owner=dict(login='fork'))
        payload = dict(action=action, repository=base, number=number, pull_request=dict(number=number, state='open', head=dict(ref='main', sha=revision, repo=fork), base=dict(ref='main', sha=self.revision, repo=base)))
        raw = json.dumps(payload, separators=(',', ':')).encode()
        response = self.api('POST', '/api/triggers/fork', raw, signed={'X-GitHub-Delivery': str(uuid.uuid4()), 'X-GitHub-Event': 'pull_request', 'X-Hub-Signature-256': 'sha256=' + hmac.new(self.secret.encode(), raw, hashlib.sha256).hexdigest()})
        require(response.get('run_id'), 'signed fork event did not create run')
        return response['run_id']

    def approve(self, run, commit, request, token=None, expected=(200,)):
        return self.api('POST', '/api/runs/' + run + '/approve', dict(request_id=request, source_commit=commit), token=token, expected=expected)

    def check(self, name, value):
        self.report['checks'][name] = value
        self.save()

    def save(self):
        put(self.evidence / 'summary.json', self.report)

    def execute(self):
        self.fixture()
        run = self.event(self.revision)
        blocked = self.api('GET', '/api/runs/' + run)
        require(blocked['phase'] == 'awaiting_approval' and blocked.get('execution') is None, 'fork must remain blocked')
        self.api('GET', '/api/runs/' + run + '/execution-binding', token=self.viewer, expected=(403,))
        projected = self.api('GET', '/api/runs/' + run + '/execution-binding', token=self.worker)
        require(projected['phase'] == 'awaiting_approval' and projected['execution'] is None, 'read-only waiting binding invented native acceptance')
        require(not (self.root / ('invocations-' + PROJECT)).exists(), 'unapproved event invoked the native shell')
        require(self.ledger() == ({}, dict(hosted_execution=0, finance_reservations=0, finance_usage=0)), 'unapproved event crossed hosted admission')
        self.approve(run, self.revision, 'viewer-denied', token=self.viewer, expected=(403,))
        self.approve(run, self.revision, 'worker-denied', token=self.worker, expected=(403,))
        self.approve(run, '0' * 40, 'stale-denied', expected=(409,))
        self.approve(run, self.revision, 'bad-token', token='0' * 64, expected=(401,))
        require(not self.ledger()[0], 'unauthorized approval reached hosted')
        self.check('blocked_and_unauthorized', dict(run_id=run, phase=blocked['phase'], no_hosted_binding=True, stale_commit_denied=True, viewer_denied=True, worker_denied=True, unknown_token_denied=True))
        approved = self.approve(run, self.revision, 'real-operator-approval')
        original = self.binding(run)
        def handoff():
            observed = self.ledger()
            return observed if observed[0].get(run, {}).get('response') is not None else False
        bindings, effects = self.wait(handoff, 'durable hosted handoff result')
        def terminal_handoff():
            view = self.api('GET', '/api/runs/' + run)
            return view if view['phase'] == 'terminal' else False
        native_terminal = self.wait(terminal_handoff, 'native shell observed helper completion after hosted result')
        require(native_terminal['outcome'] == 'DomainFailure', 'native shell must report actual refused helper exit as failure')
        self.native_evidence(run, PROJECT)
        response = bindings[run]['response']
        self.report['observed_handoff'] = dict(run_id=run, binding=bindings[run], effects=effects)
        self.save()
        require(bindings[run]['web_approval_verified'] and bindings[run]['source_verified'], 'hosted did not durably verify exact approval and independently acquired source')
        require('Err' in response and response['Err'].startswith('untrusted host preflight refused: host kernel CVE-2026-98432 is '), 'expected actual kernel safety refusal, not arbitrary provider failure')
        require(os.uname().release.startswith('6.18.50') and 'release 6.18.50' in response['Err'], 'refusal proof requires the specified Linux 6.18.50 host')
        require(effects == dict(hosted_execution=0, finance_reservations=0, finance_usage=0), 'host refusal must precede execution/financial effects')
        self.check('approved_handoff', dict(run_id=run, hosted_binding=bindings[run], effects=effects, web_phase=approved['phase'], source_verification_evidence='durable hosted web_approval_verified and source_verified ledger facts'))
        replay = self.approve(run, self.revision, 'real-operator-approval')
        require(replay == approved and self.ledger() == (bindings, effects), 'approval replay changed attempt or response')
        self.check('approval_replay', dict(same_response=True, unchanged_ledger=True))
        # Move fixture source through jj only. Older blocked runs are superseded.
        old = self.event(self.revision, number=2)
        put(self.repo / 'value.txt', b'approved-exact-source-v2\n')
        for argv in (['describe', '-m', 'Moved disposable head'], ['bookmark', 'set', 'main', '-r', '@'], ['new', 'main']):
            self.command([self.exe['jj'], '-R', self.repo, *argv])
        moved = self.command([self.exe['jj'], '-R', self.repo, 'log', '--no-graph', '-r', 'main', '-T', 'commit_id']).decode().strip()
        self.command([self.exe['jj'], '-R', self.hosted_repo, 'git', 'fetch', '--remote', 'origin'])
        newer = self.event(moved, number=2, action='synchronize')
        self.approve(old, self.revision, 'superseded-denied', expected=(409,))
        self.approve(newer, self.revision, 'moved-needs-new-approval', expected=(409,))
        require(self.api('GET', '/api/runs/' + newer)['phase'] == 'awaiting_approval', 'moved source reused old approval')
        require(self.ledger() == (bindings, effects), 'moved/unapproved run executed')
        self.check('moved_source', dict(old_run=old, new_run=newer, new_commit=moved, superseded_denied=True, fresh_approval_required=True))
        # The helper has exited; CLI probes now acquire its dedicated key.
        self.rpc('execution.lattice_run', original, peer='stranger', error='membership')
        self.rpc('execution.lattice_run', original, peer='owner', error='current Web approval authority policy denies request')
        require(self.ledger() == (bindings, effects), 'unauthorized peer left execution effects')
        self.rpc('execution.lattice_get', original, peer='lattice-peer', error=response.get('Err'))
        for _ in range(2):
            self.rpc('execution.lattice_run', original, peer='lattice-peer', error=response.get('Err'), request_id='original-native-replay')
        require(self.ledger() == (bindings, effects), 'authenticated execution replay created another attempt')
        for label, mutate in [
            ('gate', lambda request: request['binding']['untrusted'].update(approval=None)),
            ('operation', lambda request: request['plan']['nodes'][0].update(operation_identity='0' * 64)),
            ('node', lambda request: request.update(node_id='other')),
            ('source', lambda request: request['binding']['source'].update(canonical_tree='0' * 64)),
        ]:
            probe = copy.deepcopy(original)
            mutate(probe)
            expected = 'Lattice workflow policy, node, operation or execution rejected' if label == 'node' else 'Web run binding conflicts with retained input'
            denied = self.rpc('execution.lattice_run', probe, peer='lattice-peer', error=expected)
            require(self.ledger() == (bindings, effects), 'tampered replay changed retained admission or effects')
            self.check('tampered_' + label, dict(refusal=denied, unchanged_ledger=True))
        callback_probe = copy.deepcopy(original)
        callback_probe['binding']['run_id'] = newer
        for action in ('execution.lattice_get', 'execution.lattice_cancel'):
            pending = self.rpc(action, callback_probe, peer='lattice-peer')
            require(pending['state'] == 'unknown' and pending['sequence'] == 0 and pending['result'] is None, 'unregistered control invented a backend execution')
            require(not pending['admission_registered'] and not pending['cancellation_requested'] and not pending['web_approval_verified'] and not pending['source_verified'], 'unregistered control claimed retained cancellation or verification')
            require(self.ledger() == (bindings, effects), 'unregistered control allocated binding or execution state')
        self.check('unregistered_control', dict(unknown_without_receipt=True, no_orphan_cancellation_intent=True, unchanged_ledger=True))
        self.rpc('execution.lattice_run', callback_probe, peer='lattice-peer', error='authoritative Web execution binding differs from Lattice request')
        refused_bindings, refused_effects = self.ledger()
        require(refused_effects == effects and set(refused_bindings) == set(bindings) | {newer} and all(refused_bindings[key] == value for key, value in bindings.items()), 'callback refusal changed execution/finance or previous bindings')
        refused = refused_bindings[newer]
        require(not refused['web_approval_verified'] and not refused['source_verified'] and refused['response'].get('Err') == 'authoritative Web execution binding differs from Lattice request', 'callback refusal metadata falsely claimed verification')
        self.check('authoritative_callback', dict(waiting_moved_run_cannot_reuse_approved_payload=True, metadata_only_refusal_retained=True, no_execution_or_financial_effects=True))
        self.check('peer_and_binding_admission', dict(unenrolled_peer_denied=True, enrolled_unpinned_owner_denied=True, pinned_peer_replay_unchanged=True))
        all_bindings, all_effects = self.ledger()
        require(all_effects == effects, 'negative probes created execution or financial effects')
        self.stop(self.web)
        self.stop(self.lattice)
        self.stop(self.hosted)
        self.start_hosted()
        self.start_lattice()
        self.start_web()
        recovered = self.api('GET', '/api/runs/' + run)
        require(recovered['id'] == run and self.ledger() == (all_bindings, all_effects), 'restart reexecuted original or changed retained refusal')
        require(self.approve(run, self.revision, 'real-operator-approval') == approved, 'restart lost approval replay')
        require(self.ledger() == (all_bindings, all_effects), 'restart replay changed hosted state')
        self.native_evidence(run, PROJECT)
        self.check('restart', dict(original_run_observed=True, approval_replay_preserved=True, no_reexecution=True, hosted_binding_count=len(all_bindings), effects=all_effects))
        if self.args.trusted_execution:
            self.exercise_trusted()
            self.exercise_cancellation()
        self.report['status'] = 'passed_native_trusted_and_untrusted_refusal' if self.args.trusted_execution else 'passed_admission_vm_qualification_pending'

    def exercise_trusted(self):
        before_bindings, before_effects = self.ledger()
        self.api('POST', '/api/projects', dict(project_id='trusted-fixture', request_id='enroll-trusted'))
        submitted = self.api('POST', '/api/runs', dict(project_id='trusted-fixture', request_id='trusted-exact-source'))
        run_id = submitted['id']
        def terminal():
            observed = self.api('GET', '/api/runs/' + run_id)
            return observed if observed['outcome'] not in ('Pending', 'Unknown') else False
        view = self.wait(terminal, 'actual controlled trusted microVM terminal result')
        self.native_evidence(run_id, 'trusted-fixture')
        self.report['observed_trusted_run'] = {key: view[key] for key in ('id', 'source_revision', 'phase', 'outcome', 'nodes', 'error_code')}
        self.save()
        bindings, effects = self.ledger()
        bound = bindings.get(run_id)
        with sqlite3.connect((self.root / 'hosted-state/hosted.sqlite').as_uri() + '?mode=ro', uri=True) as db:
            row = None if bound is None else db.execute('SELECT state,result,request_identity FROM hosted_execution WHERE org=? AND project=? AND attempt=?', (ORG, 'trusted-fixture', bound['attempt'])).fetchone()
        receipt = {} if row is None or row[1] is None else json.loads(row[1])
        diagnostics = []
        def retain_diagnostics(value):
            if isinstance(value, dict):
                fields = {key: value[key] for key in ('state', 'reason', 'cleaned') if key in value and isinstance(value[key], (str, bool))}
                if 'reason' in fields:
                    reason = str(fields['reason']).replace(str(self.root), '[private-scratch]')
                    for peer in self.peers.values():
                        reason = reason.replace(peer, '[peer]')
                    fields['reason'] = reason[:2048]
                if fields:
                    diagnostics.append(fields)
                for nested in value.values():
                    retain_diagnostics(nested)
            elif isinstance(value, list):
                for nested in value:
                    retain_diagnostics(nested)
        retain_diagnostics(receipt)
        self.report['observed_trusted_run']['hosted_diagnostics'] = diagnostics
        self.save()
        require(view['source_revision'] == self.revision and view['outcome'] == 'Success' and view['phase'] == 'terminal', 'controlled exact trusted source did not really succeed')
        trusted_node = 'build'
        require(len(view['nodes']) == 1 and view['nodes'][0]['id'] == trusted_node and view['nodes'][0]['state'] == 'Completed', 'native outer node result was not Completed')
        expected_stdout = 'real trusted guest source=approved-exact-source-v1\n'
        log_target = '/api/runs/' + run_id + '/logs?' + urllib.parse.urlencode(dict(node_id=trusted_node, stream='stdout', cursor=0, max_bytes=65536))
        logs = self.api('GET', log_target)
        require(logs.get('text') == expected_stdout and logs.get('gap') is False, 'HTTPS hosted stdout did not preserve exact source-bound guest bytes')
        require(set(bindings) == set(before_bindings) | {run_id} and all(bindings[key] == value for key, value in before_bindings.items()), 'trusted run changed untrusted/negative admission records')
        require(bound['web_approval_verified'] and bound['source_verified'] and bound['sequence'] > 0, 'trusted hosted source/authority or observation sequence absent')
        require(row is not None and row[0] == 'succeeded' and row[2] == bound['execution_identity'], 'trusted Web handoff differs from exact real hosted execution')
        require(receipt.get('cleaned') is True and receipt.get('state') == 'succeeded' and len(receipt.get('jobs', [])) == 1, 'trusted hosted receipt did not prove successful cleanup')
        require(receipt['jobs'][0].get('state') == 'succeeded' and receipt['jobs'][0].get('microvm') is not None, 'trusted result lacks actual microVM job receipt')
        require(effects == {key: value + 1 for key, value in before_effects.items()}, 'trusted run must own exactly one execution, reservation and usage receipt')
        replay = self.api('POST', '/api/runs', dict(project_id='trusted-fixture', request_id='trusted-exact-source'))
        require(replay['id'] == run_id and self.ledger() == (bindings, effects), 'trusted HTTPS replay created another execution')
        self.stop(self.web)
        self.stop(self.lattice)
        self.stop(self.hosted)
        self.start_hosted()
        self.start_lattice()
        self.start_web()
        restored = self.api('GET', '/api/runs/' + run_id)
        restored_logs = self.api('GET', log_target)
        require(restored['outcome'] == 'Success' and restored['nodes'] == view['nodes'] and restored_logs.get('text') == expected_stdout, 'trusted outcome/node/log observation did not survive restart')
        require(self.ledger() == (bindings, effects), 'trusted restart observation reexecuted or changed retained sequence')
        self.native_evidence(run_id, 'trusted-fixture')
        self.check('trusted_execution', dict(run_id=run_id, source_commit=self.revision, outcome=view['outcome'], node_states={node['id']: node['state'] for node in view['nodes']}, stdout=logs['text'], cleaned=True, hosted_execution_identity=bound['execution_identity'], sequence=bound['sequence'], exactly_one_execution=True, request_replay_unchanged=True, restart_outcome_and_logs_preserved=True, untrusted_vm_qualification='pending', non_claim='controlled trusted local execution is not untrusted VM qualification'))

    def unit_properties(self, unit):
        require(unit.startswith('kiln-hosted-') and unit.endswith('.service') and len(unit) == len('kiln-hosted-') + 32 + len('.service') and all(c in '0123456789abcdef' for c in unit[len('kiln-hosted-'):-len('.service')]), 'invalid owned execution unit')
        raw = self.command([self.exe['systemctl'], '--user', 'show', unit, '--property=ActiveState,SubState,MainPID'], allowed=(0, 1))
        return dict(line.split('=', 1) for line in raw.decode().splitlines() if '=' in line)

    def live_cancel_guest(self, run_id):
        bindings, _ = self.ledger()
        bound = bindings.get(run_id)
        if bound is None:
            return False
        owned_unit = 'kiln-hosted-' + bound['execution_identity'][:32] + '.service'
        self.owned_units.add(owned_unit)
        with sqlite3.connect((self.root / 'hosted-state/hosted.sqlite').as_uri() + '?mode=ro', uri=True) as db:
            row = db.execute('SELECT e.unit,e.state,j.lifecycle,a.pid,a.start_ticks FROM hosted_execution e JOIN hosted_microvm_job j ON j.org=e.org AND j.attempt=e.attempt JOIN hosted_vm_active a ON a.org=j.org AND a.attempt=j.attempt AND a.job=j.job WHERE e.org=? AND e.project=? AND e.attempt=? AND j.job=?', (ORG, 'cancel-fixture', bound['attempt'], self.cancel_node_id)).fetchone()
        if row is None:
            return False
        unit, state, lifecycle, pid, start_ticks = row
        require(unit == owned_unit, 'cancellation unit differs from bound execution')
        if state != 'running' or lifecycle != 'starting':
            return False
        properties = self.unit_properties(unit)
        if properties.get('ActiveState') != 'active' or properties.get('SubState') != 'running' or properties.get('MainPID') != str(pid):
            return False
        try:
            process = Path('/proc') / str(pid)
            fields = (process / 'stat').read_text().rsplit(')', 1)[1].split()
            if fields[0] in ('Z', 'X', 'x') or fields[19] != start_ticks:
                return False
            runner = self.hosted_profile['execution']['runners']['trusted-linux']
            if (process / 'exe').resolve(strict=True) != Path(runner['firecracker']['path']).resolve(strict=True):
                return False
        except (OSError, ValueError, IndexError):
            return False
        return dict(unit=unit, attempt=bound['attempt'], live_vmm=True)

    def exercise_cancellation(self):
        before_bindings, before_effects = self.ledger()
        self.api('POST', '/api/projects', dict(project_id='cancel-fixture', request_id='enroll-cancel'))
        submitted = self.api('POST', '/api/runs', dict(project_id='cancel-fixture', request_id='cancel-live-source'))
        run_id = submitted['id']
        live = self.wait(lambda: self.live_cancel_guest(run_id), 'owned controlled VMM observed live after confinement')
        view = self.api('GET', '/api/runs/' + run_id)
        require(view['source_revision'] == self.revision and view['outcome'] == 'Pending' and view['controls']['cancel'] and view.get('execution') is not None, 'live cancellation run must be source-bound and controllable')
        control = dict(request_id='cancel-live-https', action='cancel', attempt_id=view['attempt_id'], expected_revision=view['revision'], expected_sequence=view['sequence'], execution=view['execution'])
        require(self.live_cancel_guest(run_id), 'controlled VMM stopped before HTTPS cancellation')
        accepted = self.api('POST', '/api/runs/' + run_id + '/control', control, expected=(200,))
        require(accepted['id'] == run_id, 'HTTPS cancellation accepted a different run')
        def terminal():
            observed = self.api('GET', '/api/runs/' + run_id)
            return observed if observed['outcome'] not in ('Pending', 'Unknown') else False
        result = self.wait(terminal, 'truthful cancelled Web outcome')
        require(result['outcome'] == 'Cancelled' and result['phase'] == 'terminal', 'live HTTPS cancellation did not map to Cancelled')
        require(len(result['nodes']) == 1 and result['nodes'][0]['id'] == 'build' and result['nodes'][0]['state'] == 'Cancelled', 'native outer node did not map to Cancelled')
        def cleaned():
            with sqlite3.connect((self.root / 'hosted-state/hosted.sqlite').as_uri() + '?mode=ro', uri=True) as db:
                row = db.execute('SELECT state,result FROM hosted_execution WHERE org=? AND project=? AND attempt=?', (ORG, 'cancel-fixture', live['attempt'])).fetchone()
                journal = db.execute('SELECT lifecycle FROM hosted_microvm_job WHERE org=? AND attempt=? AND job=?', (ORG, live['attempt'], self.cancel_node_id)).fetchone()
                active = db.execute('SELECT COUNT(*) FROM hosted_vm_active WHERE org=? AND attempt=?', (ORG, live['attempt'])).fetchone()[0]
            if row is None or row[1] is None:
                return False
            receipt = json.loads(row[1])
            require(row[0] == 'cancelled' and receipt.get('state') == 'cancelled' and receipt.get('cleaned') is True, 'hosted cancellation did not prove cancelled clean result')
            require(len(receipt.get('jobs', [])) == 1 and receipt['jobs'][0].get('state') == 'cancelled' and receipt['jobs'][0].get('microvm') is not None, 'cancelled job lacks actual microVM receipt')
            return receipt if journal == ('destroyed',) and active == 0 and self.unit_properties(live['unit']).get('ActiveState') not in ('active', 'activating', 'deactivating') else False
        self.wait(cleaned, 'destroyed guest, cleaned hosted cancellation and stopped owned unit')
        self.native_evidence(run_id, 'cancel-fixture')
        self.wait(lambda: self.ledger()[0].get(run_id, {}).get('response') is not None, 'durable cancelled hosted handoff response')
        bindings, effects = self.ledger()
        bound = bindings[run_id]
        require(bound['web_approval_verified'] and bound['source_verified'], 'cancelled job lacked exact source and authority verification')
        require(set(bindings) == set(before_bindings) | {run_id} and all(bindings[key] == value for key, value in before_bindings.items()), 'cancellation changed previous bindings')
        require(effects == {key: value + 1 for key, value in before_effects.items()}, 'cancelled run must have exactly one execution/reservation/usage')
        require(self.api('POST', '/api/runs/' + run_id + '/control', control) == accepted, 'HTTPS cancel replay changed retained response')
        require(self.api('POST', '/api/runs', dict(project_id='cancel-fixture', request_id='cancel-live-source'))['id'] == run_id and self.ledger() == (bindings, effects), 'cancellation replay created execution or financial effects')
        self.stop(self.web)
        self.stop(self.lattice)
        self.stop(self.hosted)
        self.start_hosted()
        self.start_lattice()
        self.start_web()
        restored = self.api('GET', '/api/runs/' + run_id)
        require(restored['outcome'] == 'Cancelled' and restored['nodes'] == result['nodes'], 'cancelled outcome/node lost after restart')
        require(self.api('POST', '/api/runs/' + run_id + '/control', control) == accepted and self.ledger() == (bindings, effects), 'restart/cancel replay created execution or financial effects')
        self.native_evidence(run_id, 'cancel-fixture')
        require(self.unit_properties(live['unit']).get('ActiveState') not in ('active', 'activating', 'deactivating'), 'restart revived cancelled owned unit')
        self.check('trusted_live_cancellation', dict(run_id=run_id, source_commit=self.revision, node_id='build', hosted_job_id=self.cancel_node_id, live_vmm_observed=True, confinement_and_pid_lifetime_verified=True, https_cancel_accepted=True, outcome='Cancelled', node_state='Cancelled', cleaned=True, journal_lifecycle='destroyed', no_running_owned_unit=True, exactly_one_execution_reservation_usage=True, replay_and_restart_unchanged=True, untrusted_vm_qualification='pending'))

    def close(self):
        failures = []
        database = self.root / 'hosted-state/hosted.sqlite'
        def discover_owned_units():
            if database.exists():
                try:
                    with sqlite3.connect(database.as_uri() + '?mode=ro', uri=True) as db:
                        self.owned_units.update(row[0] for row in db.execute('SELECT unit FROM hosted_execution WHERE org=?', (ORG,)) if row[0])
                except Exception as error:
                    failures.append(type(error).__name__)
        discover_owned_units()
        for unit in self.owned_units:
            try:
                if self.unit_properties(unit).get('ActiveState') in ('active', 'activating', 'deactivating'):
                    self.command([self.exe['systemctl'], '--user', 'stop', unit])
                require(self.unit_properties(unit).get('ActiveState') not in ('active', 'activating', 'deactivating'), 'owned smoke unit remains running after teardown')
            except Exception as error:
                failures.append(type(error).__name__)
        for process in reversed(self.processes):
            try:
                self.stop(process)
            except Exception as error:
                failures.append(type(error).__name__)
        # A request accepted just before teardown may have spawned its exact
        # owned unit while the service was draining; check after children stop.
        discover_owned_units()
        for unit in self.owned_units:
            try:
                if self.unit_properties(unit).get('ActiveState') in ('active', 'activating', 'deactivating'):
                    self.command([self.exe['systemctl'], '--user', 'stop', unit])
                require(self.unit_properties(unit).get('ActiveState') not in ('active', 'activating', 'deactivating'), 'owned smoke unit survived service shutdown')
            except Exception as error:
                failures.append(type(error).__name__)
        secret_paths = [self.root / (name + '.token') for name in ('operator', 'worker', 'viewer', 'trigger')]
        secret_paths += [self.root / (name + '.key') for name in ('server', 'owner', 'lattice-peer', 'stranger', 'tls', 'ca')]
        secret_paths += [self.root / 'tls.csr', self.root / 'tls.srl']
        for path in secret_paths:
            try:
                path.unlink(missing_ok=True)
            except OSError as error:
                failures.append(type(error).__name__)
        self.report['secret_removal'] = dict(scope='only smoke-created root bearer tokens, Iroh private keys and TLS private key; not a claim of sanitized retained scratch', removed=all(not path.exists() for path in secret_paths), files_checked=len(secret_paths))
        self.report['teardown'] = dict(all_owned_children_stopped=all(process.poll() is not None for process in self.processes), owned_units_checked=len(self.owned_units), errors=failures, private_scratch_retained=True)
        if failures:
            self.report['status'] = 'failed'
            self.report['teardown_failure'] = dict(kind='TeardownFailure', message='owned teardown required force or failed')
            self.report.setdefault('failure', self.report['teardown_failure'])
        require(not failures, 'owned teardown required force or failed')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--hosted', required=True)
    parser.add_argument('--web', required=True, help='ordinary kiln-web-service binary')
    parser.add_argument('--lattice', required=True, help='genuine native Lattice executable')
    parser.add_argument('--lattice-source', required=True, help='exact reviewed source cohort for native contracts/templates')
    parser.add_argument('--guest', required=True, help='existing guest output directory or manifest.json')
    parser.add_argument('--root', required=True, help='fresh short private scratch outside /tmp (e.g. target/kwh-c); parent must exist; guest AF_UNIX paths must fit 108 bytes')
    parser.add_argument('--evidence', required=True, help='retained whitelist-only summary directory outside scratch')
    parser.add_argument('--trusted-execution', action='store_true', help='also prove controlled trusted Firecracker success and live HTTPS cancellation; never waives untrusted preflight')
    args = parser.parse_args()
    os.umask(0o077)
    def interrupt(signum, frame):
        raise KeyboardInterrupt('smoke interrupted')
    signal.signal(signal.SIGTERM, interrupt)
    signal.signal(signal.SIGINT, interrupt)
    exercise = None
    try:
        exercise = Exercise(args)
        exercise.execute()
    except BaseException as error:
        if exercise is not None:
            exercise.report['status'] = 'failed'
            exercise.report['failure'] = dict(kind=type(error).__name__, message=str(error))
        raise
    finally:
        if exercise is not None:
            try:
                exercise.close()
            finally:
                exercise.save()
    print(json.dumps(dict(status=exercise.report['status'], evidence=str(exercise.evidence / 'summary.json'), vm_qualification=exercise.report['vm_qualification'])))


if __name__ == '__main__':
    main()
