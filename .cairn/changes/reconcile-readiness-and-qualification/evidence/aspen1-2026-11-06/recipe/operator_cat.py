#!/usr/bin/env python3
"""Operator-only CAT assignment/observation; no product API or host setup changes."""
import argparse
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import sys
import time
BLAKE3 = '/nix/store/0by3mqkn4gk970s8h0ns546jcm8hfl7w-b3sum-1.8.7/bin/b3sum'


def require(ok, message):
    if not ok:
        raise ValueError(message)


def read(path):
    return Path(path).read_text().strip()


def digest(path):
    return subprocess.check_output([BLAKE3, '--no-names', str(path)], text=True).strip()


def digest_bytes(data):
    return subprocess.run([BLAKE3, '--no-names'], input=data, stdout=subprocess.PIPE, check=True).stdout.decode().strip()


def create(path, value):
    with open(path, 'x') as stream:
        json.dump(value, stream, indent=2)
        stream.write('\n')


def stable(path, executable=False):
    p = Path(path)
    require(p.is_absolute() and p.resolve() == p and p.is_file(), f'not a canonical file: {p}')
    s = p.stat()
    require(s.st_uid == 0 and not s.st_mode & 0o022, f'not operator-owned immutable input: {p}')
    require(os.access(p, os.X_OK if executable else os.R_OK), f'input inaccessible: {p}')
    # Prevent replacing an otherwise immutable file through a writable ancestor.
    for parent in p.parents:
        require(not os.access(parent, os.W_OK), f'writable input ancestor: {parent}')
    return str(p)


def runtime(path):
    p = Path(path)
    require(p.is_absolute() and p.resolve() == p and p.is_dir(), 'runtime must be an existing canonical directory')
    s = p.stat()
    require(s.st_uid == os.getuid() and stat.S_IMODE(s.st_mode) == 0o700, 'runtime must be account-owned mode 0700')
    return p


def group(path):
    p = Path(path)
    require(p.parent == Path('/sys/fs/resctrl') and re.fullmatch(r'[A-Za-z0-9_-]+', p.name), 'only explicit top-level resctrl groups admitted')
    require(p.resolve() == p and p.is_dir(), 'resctrl group missing or symlinked')
    require(p.stat().st_uid == 0, 'group directory must remain operator-owned')
    require((p/'tasks').stat().st_uid == os.getuid() and os.access(p/'tasks', os.W_OK), 'group tasks not delegated to this account')
    require(read(p/'mode') == 'exclusive', 'group mode must be exclusive')
    require(not read(p/'cpus_list'), 'group must have no CPU-default assignment')
    return p


def l3(schemata):
    rows = [line.lstrip() for line in schemata.splitlines() if line.lstrip().startswith('L3:')]
    require(len(rows) == 1, 'requires unified L3 CAT, not CDP or absent L3')
    return {int(k): int(v, 16) for k, v in (entry.split('=') for entry in rows[0][3:].split(';') if entry)}


def groups(cfg, empty=False):
    observed = {}
    for role in ('sender', 'receiver'):
        p = group(cfg['groups'][role])
        observed[role] = {'path': str(p), 'mode': read(p/'mode'), 'schemata': read(p/'schemata'), 'cpus_list': read(p/'cpus_list')}
        if empty:
            require(not read(p/'tasks'), f'{role} group not empty before/after measurement')
    a, b = (l3(observed[r]['schemata']) for r in ('sender', 'receiver'))
    require(a.keys() == b.keys() and a, 'CAT domains differ or missing')
    require(all(a[k] and b[k] and not a[k] & b[k] for k in a), 'CAT masks not nonempty/disjoint in every domain')
    if 'group_snapshot' in cfg:
        require(observed == cfg['group_snapshot'], 'group configuration changed')
    return observed


def topology(cpu):
    root = Path(f'/sys/devices/system/cpu/cpu{cpu}')
    require(cpu in os.sched_getaffinity(0), 'CPU outside this account execution affinity')
    facts = {key: read(root/'topology'/key) for key in ('physical_package_id', 'core_id', 'thread_siblings_list')}
    llcs = [p for p in (root/'cache').iterdir() if p.name.startswith('index') and read(p/'level') == '3']
    require(len(llcs) == 1, 'requires exactly one observed L3 domain per CPU')
    facts['llc_shared_cpus'] = read(llcs[0]/'shared_cpu_list')
    return facts


def load(path):
    cfg = json.loads(Path(path).read_text())
    require(os.getuid() != 0 and cfg['uid'] == os.getuid(), 'must use the dedicated unprivileged account')
    runtime(cfg['runtime'])
    return cfg


def prepare(a):
    require(os.getuid() != 0, 'do not run as root')
    run = runtime(a.runtime)
    require(a.sender_group != a.receiver_group, 'groups must differ')
    require(a.sender_cpu != a.receiver_cpu and 0 <= a.sender_cpu < 4096 and 0 <= a.receiver_cpu < 4096, 'invalid CPU pair')
    require(read('/sys/devices/system/cpu/smt/active') == '0', 'SMT must already be disabled')
    topo = {r: topology(c) for r, c in [('sender', a.sender_cpu), ('receiver', a.receiver_cpu)]}
    x, y = topo.values()
    require((x['physical_package_id'], x['core_id']) != (y['physical_package_id'], y['core_id']), 'CPUs share physical core')
    require((x['physical_package_id'], x['llc_shared_cpus']) == (y['physical_package_id'], y['llc_shared_cpus']), 'CPUs do not share observed LLC')
    cfg = {'schema': 'kiln.operator.cat-cohort.v1', 'uid': os.getuid(), 'runtime': str(run), 'topology': topo,
           'groups': {'sender': a.sender_group, 'receiver': a.receiver_group}, 'cpus': {'sender': a.sender_cpu, 'receiver': a.receiver_cpu}}
    for key in ('python', 'script', 'firecracker', 'kernel', 'initrd', 'taskset'):
        cfg[key] = stable(getattr(a, key), key in ('python', 'firecracker', 'taskset'))
    cfg['operator_tool'] = stable(str(Path(__file__).resolve()))
    cfg['input_blake3'] = {key: digest(cfg[key]) for key in ('python', 'script', 'firecracker', 'kernel', 'initrd', 'taskset', 'operator_tool')}
    cfg['group_snapshot'] = groups(cfg, empty=True)
    stable(BLAKE3, executable=True)
    cfg['cohort'] = digest_bytes(json.dumps(cfg, sort_keys=True, separators=(',', ':')).encode())
    (run/'registrations').mkdir(mode=0o700)
    manifest = run/'cohort.json'
    placements = {'pinned_cores': {}}
    for role in ('sender', 'receiver'):
        endpoint = {key: cfg[key] for key in ('firecracker', 'kernel', 'initrd', 'taskset')}
        endpoint['cpus'] = [cfg['cpus'][role]]
        endpoint['command'] = [cfg['python'], cfg['operator_tool'], 'worker', '--manifest', str(manifest), '--role', role]
        placements['pinned_cores'][role] = endpoint
    create(run/'placements.json', placements)
    cfg['placements_blake3'] = digest(run/'placements.json')
    create(manifest, cfg)
    print(json.dumps({'manifest': str(manifest), 'cohort': cfg['cohort']}))


def start_ticks(pid):
    return read(f'/proc/{pid}/stat').rsplit(')', 1)[1].split()[19]


def worker(a):
    cfg = load(a.manifest)
    groups(cfg)
    raw = sys.stdin.buffer.read(65537)
    require(len(raw) <= 65536, 'request too large')
    request = json.loads(raw)
    require(request['role'] == a.role, 'endpoint role mismatch')
    endpoint = json.loads(Path(cfg['runtime'], 'placements.json').read_text())['pinned_cores'][a.role]
    require(request['endpoint'] == endpoint, 'endpoint differs from approved operator cohort')
    # Forward precisely the original JSON, without retaining the report identity key.
    fd = os.memfd_create('kiln-cat-worker-stdin', 0)
    with os.fdopen(os.dup(fd), 'wb') as stream:
        stream.write(raw)
    os.lseek(fd, 0, os.SEEK_SET)
    path = Path(cfg['groups'][a.role])
    with open(path/'tasks', 'w') as stream:
        stream.write(str(os.getpid()))
    require(str(os.getpid()) in read(path/'tasks').split(), 'self assignment was not observed')
    record = {'cohort': cfg['cohort'], 'role': a.role, 'channel': request['channel'], 'pid': os.getpid(), 'start_ticks': start_ticks(os.getpid()),
              'start_ms': request['start_ms'], 'symbol_ms': request['symbol_ms'], 'count': len(request['bits']) if a.role == 'sender' else request['count'],
              'request_blake3': digest_bytes(raw), 'sender_bits_blake3': digest_bytes(request['bits'].encode()) if a.role == 'sender' else None,
              'assigned_group': str(path), 'assigned_at_ns': time.time_ns()}
    create(Path(cfg['runtime'])/'registrations'/f'{os.getpid()}-{record["start_ticks"]}.json', record)
    os.dup2(fd, 0)
    os.close(fd)
    os.execv(cfg['python'], [cfg['python'], cfg['script'], 'worker'])


def descendants(pid):
    todo = [pid]
    found = set()
    while todo:
        current = todo.pop()
        if current in found:
            continue
        found.add(current)
        try:
            todo.extend(int(p) for p in read(f'/proc/{current}/task/{current}/children').split())
        except FileNotFoundError:
            pass
    return found - {pid}


def observe(cfg, registrations, observations):
    snap = groups(cfg)
    for file in (Path(cfg['runtime'])/'registrations').glob('*.json'):
        if file.name not in registrations:
            try:
                registrations[file.name] = json.loads(file.read_text())
            except json.JSONDecodeError:
                continue  # writer has not yet closed the new registration
        reg = registrations[file.name]
        require(reg['cohort'] == cfg['cohort'], 'foreign registration')
        try:
            if start_ticks(reg['pid']) != reg['start_ticks']:
                continue
        except FileNotFoundError:
            continue
        role = reg['role']
        for pid in descendants(reg['pid']):
            try:
                if os.readlink(f'/proc/{pid}/exe') != cfg['firecracker']:
                    continue
                tids = list(Path(f'/proc/{pid}/task').iterdir())
                rows = []
                for tidpath in tids:
                    tid = int(tidpath.name)
                    try:
                        name = read(tidpath/'comm')
                        affinity = sorted(os.sched_getaffinity(tid))
                        members = set(map(int, read(Path(cfg['groups'][role])/'tasks').split()))
                        if not tidpath.exists():
                            continue
                        require(tid in members, f'live VMM task {tid} outside intended group')
                        require(affinity == [cfg['cpus'][role]], f'live VMM task {tid} outside intended CPU')
                        rows.append({'tid': tid, 'name': name, 'cpus': affinity, 'observed_in_group': cfg['groups'][role]})
                    except ProcessLookupError:
                        continue
                # Require the actual vCPU thread, not only worker or VMM main PID.
                if rows and any('vcpu' in r['name'].lower() for r in rows):
                    key = file.name
                    item = observations.setdefault(key, {'first_observed_ns': time.time_ns(), 'samples': 0})
                    item.update({'last_observed_ns': time.time_ns(), 'vmm_pid': pid, 'vmm_start_ticks': start_ticks(pid), 'tasks': rows, 'group_snapshot': snap})
                    item['samples'] += 1
            except (FileNotFoundError, ProcessLookupError):
                continue


def measure(a):
    cfg = load(a.manifest)
    run = Path(cfg['runtime'])
    require(not (run/'observations.json').exists() and not (run/'cat-evidence.json').exists(), 'cohort is single-use')
    require(8 <= a.symbols <= 1024 and 20 <= a.symbol_ms <= 1000 and 1000 <= a.lead_ms <= 30000, 'outside maintained run bounds')
    require(digest(run/'placements.json') == cfg['placements_blake3'], 'placements changed')
    require(all(digest(cfg[key]) == value for key, value in cfg['input_blake3'].items()), 'cohort input changed')
    groups(cfg, empty=True)
    require(read('/sys/devices/system/cpu/smt/active') == '0', 'SMT changed before measurement')
    require({r: topology(cfg['cpus'][r]) for r in ('sender', 'receiver')} == cfg['topology'], 'CPU topology changed')
    require(not list((run/'registrations').iterdir()), 'registration directory must be empty')
    command = [cfg['python'], cfg['script'], 'run', '--config', str(run/'placements.json'), '--output', str(run/'observations.json'),
               '--symbols', str(a.symbols), '--symbol-ms', str(a.symbol_ms), '--lead-ms', str(a.lead_ms)]
    registrations, observations, errors = {}, {}, []
    evidence = {'schema': 'kiln.operator.cat-assignment-observation.v1', 'cohort': cfg['cohort'], 'manifest_blake3': digest(a.manifest),
                'measurement_command': command, 'non_claim': 'Operator sidecar only; not built-in worker CAT metadata, hostile isolation, channel absence or physical-pool evidence.'}
    process = subprocess.Popen(command)
    try:
        while process.poll() is None:
            observe(cfg, registrations, observations)
            time.sleep(0.02)
        require(process.returncode == 0, f'maintained measurement exited {process.returncode}')
        report = json.loads((run/'observations.json').read_text())
        require(len(report['runs']) == 3, 'expected exactly cache/cpu/memory pinned-core runs')
        for result in report['runs']:
            require(result['placement'] == 'pinned_cores', 'unexpected placement')
            for role in ('sender', 'receiver'):
                matches = [(key, reg) for key, reg in registrations.items() if reg['role'] == role and reg['channel'] == result['channel']]
                require(len(matches) == 1, 'missing or duplicate request registration')
                key, reg = matches[0]
                require(key in observations, 'no live VMM/vCPU group observation for request')
                require(reg['count'] == len(result['sender_bits']) and reg['symbol_ms'] == a.symbol_ms, 'request schedule mismatch')
                probe = result[role]['probe']
                require(probe['start_ms'] == reg['start_ms'] and probe['symbol_ms'] == reg['symbol_ms'], 'guest schedule differs from registered request')
                require(probe['role'] == role and probe['channel'] == reg['channel'], 'guest identity differs from registered request')
                require(result[role]['endpoint'] == json.loads((run/'placements.json').read_text())['pinned_cores'][role], 'reported endpoint mismatch')
                if role == 'sender':
                    require(reg['sender_bits_blake3'] == digest_bytes(result['sender_bits'].encode()), 'request bits do not bind to measurement')
        require(len(registrations) == 6 and len(observations) == 6, 'expected six uniquely observed VMM executions')
    except BaseException as error:
        errors.append(f'{type(error).__name__}: {error}')
        if process.poll() is None:
            # Only signal our owned maintained coordinator. Its existing cleanup owns workers/VMMs.
            process.send_signal(signal.SIGINT)
            process.wait()
    finally:
        evidence['measurement_exit_code'] = process.returncode
        evidence['registrations'] = registrations
        evidence['live_vmm_observations'] = observations
        if (run/'observations.json').exists():
            evidence['measurement_blake3'] = digest(run/'observations.json')
        try:
            groups(cfg, empty=True)
            owned = [(reg['pid'], reg['start_ticks']) for reg in registrations.values()]
            owned += [(obs['vmm_pid'], obs['vmm_start_ticks']) for obs in observations.values()]
            for pid, ticks in owned:
                try:
                    require(start_ticks(pid) != ticks, f'owned worker/VMM {pid} remains alive after coordinator exit')
                except FileNotFoundError:
                    pass
            evidence['resctrl_tasks_empty_after'] = True
        except Exception as error:
            errors.append(str(error))
            evidence['resctrl_tasks_empty_after'] = False
        evidence['errors'] = errors
        evidence['verified'] = not errors
        create(run/'cat-evidence.json', evidence)
    require(not errors, '; '.join(errors))
    print(json.dumps({'cohort': cfg['cohort'], 'cat_evidence': str(run/'cat-evidence.json'), 'verified': True}))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    sub = p.add_subparsers(dest='action', required=True)
    prep = sub.add_parser('prepare')
    for key in ('runtime', 'python', 'script', 'firecracker', 'kernel', 'initrd', 'taskset', 'sender-group', 'receiver-group'):
        prep.add_argument('--'+key, required=True)
    for role in ('sender', 'receiver'):
        prep.add_argument('--'+role+'-cpu', required=True, type=int)
    prep.set_defaults(function=prepare)
    w = sub.add_parser('worker')
    w.add_argument('--manifest', required=True)
    w.add_argument('--role', choices=('sender', 'receiver'), required=True)
    w.set_defaults(function=worker)
    m = sub.add_parser('measure')
    m.add_argument('--manifest', required=True)
    m.add_argument('--symbols', type=int, default=64)
    m.add_argument('--symbol-ms', type=int, default=100)
    m.add_argument('--lead-ms', type=int, default=5000)
    m.set_defaults(function=measure)
    a = p.parse_args()
    a.function(a)


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(f'CAT operator failure: {error}', file=sys.stderr)
        sys.exit(1)
