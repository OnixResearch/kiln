#!/usr/bin/env python3
"""Exercise only the explicitly authorized, isolated live GitHub/Iroh CI rail.

No fabricated events or local API/source fixtures. GitHub creates and redelivers
its own signed requests; all terminal claims require exact remote status IDs.
"""
import argparse
import json
from pathlib import Path
import shlex
import subprocess
import time

REPO_ROOT = Path(__file__).resolve().parent.parent
CONTROLLER_ROOT = "/var/tmp/kiln-live-ci-20261018"
WORKER_ROOT = CONTROLLER_ROOT + "/worker"
PACKAGE = "/nix/store/pns6mna0zim4y9yp8cj5932dgssc29s4-kiln-0.1.0"
PYTHON = "/nix/store/b5bpi6zfajzzrwwpgba2q6li3nnya4bs-python3-3.14.7/bin/python3"
REPOSITORY = "OnixResearch/kiln"
REPOSITORY_ID = 1421187104
WAIT_SECONDS = 600
COMMAND_TIMEOUT_SECONDS = 120
POLL_SECONDS = 2
MAXIMUM_LOG_BYTES = 1048576


def run(argv, *, cwd=REPO_ROOT, data=None):
    result = subprocess.run(list(map(str, argv)), cwd=cwd, input=data,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            timeout=COMMAND_TIMEOUT_SECONDS)
    if result.returncode:
        raise RuntimeError(f"Command failed ({result.returncode}): {argv!r}\n"
                           + result.stderr.decode(errors="replace"))
    return result.stdout


def api(route, method="GET", value=None):
    argv = ["gh", "api", "--method", method, f"repos/{REPOSITORY}/{route}"]
    if value is not None:
        argv += ["--input", "-"]
    raw = run(argv, data=None if value is None else json.dumps(value).encode())
    return json.loads(raw) if raw.strip() else None


def remote(host, argv):
    return run(["ssh", "-o", "BatchMode=yes", host, shlex.join(list(map(str, argv)))])


def remote_python(host, code, *args):
    return json.loads(remote(host, [PYTHON, "-c", code, *args]))


def records(args):
    return remote_python(args.controller,
        "import json,sqlite3,sys; "
        "c=sqlite3.connect('file:'+sys.argv[1]+'?mode=ro',uri=True); "
        "print(json.dumps([json.loads(r[0]) for r in c.execute('SELECT record FROM github_runs')]))",
        CONTROLLER_ROOT + "/github-shell.sqlite")


def record_for(args, revision):
    matches = [value for value in records(args)
               if value["plan"]["source"]["revision"]["native_id"] == revision]
    if len(matches) > 1:
        raise RuntimeError("More than one canonical run for this exact push")
    return matches[0] if matches else None


def worker_logs(args):
    return remote_python(args.worker, '''import json,pathlib,sys
root=pathlib.Path(sys.argv[1]); result={}
for path in sorted(root.glob('probe.*.log')):
    with path.open('rb') as source: raw=source.read(int(sys.argv[2])+1)
    if len(raw)>int(sys.argv[2]): raise ValueError('probe log exceeds evidence bound')
    exit_path=path.with_suffix(path.suffix+'.exit')
    inputs_path=path.with_suffix(path.suffix+'.inputs')
    result[path.name]={'output':raw.decode(), 'exit_code':int(exit_path.read_text()) if exit_path.exists() else None,
                       'input_identities':inputs_path.read_text() if inputs_path.exists() else None}
print(json.dumps(result))''', WORKER_ROOT + "/probe-target", MAXIMUM_LOG_BYTES)


def until(description, observe):
    deadline = time.monotonic() + WAIT_SECONDS
    while time.monotonic() < deadline:
        result = observe()
        if result:
            return result
        time.sleep(POLL_SECONDS)
    raise TimeoutError(description)


def delivery(args, revision, *, redelivery=False):
    for item in api(f"hooks/{args.hook_id}/deliveries?per_page=30"):
        if item.get("event") != "push":
            continue
        if redelivery and not item["redelivery"]:
            continue
        detail = api(f"hooks/{args.hook_id}/deliveries/{item['id']}")
        payload = detail["request"]["payload"]
        if isinstance(payload, str):
            payload = json.loads(payload)
        if payload.get("after") != revision:
            continue
        if payload["repository"]["id"] != REPOSITORY_ID:
            raise RuntimeError("Forge delivery substituted the admitted repository")
        response = detail["response"]
        if detail["status_code"] == 0:
            return None
        if detail["status_code"] != 202:
            raise RuntimeError(f"Real webhook rejected: {response!r}")
        acknowledgement = response["payload"]
        if isinstance(acknowledgement, str):
            acknowledgement = json.loads(acknowledgement)
        return {"id": detail["id"], "guid": detail["guid"],
                "delivered_at": detail["delivered_at"], "event": detail["event"],
                "redelivery": detail["redelivery"], "ref": payload["ref"],
                "revision": revision, "response_status": 202,
                "acknowledgement": acknowledgement}
    return None


def status(args, identity):
    return json.loads(remote(args.controller, ["runuser", "-u", "kiln-live-controller", "--",
        PACKAGE + "/bin/kiln-adapter-github", "status", "--profile",
        CONTROLLER_ROOT + "/github-ci.json", "--run", identity]))


# r[impl kiln.live_ci.outcomes] r[verify kiln.live_ci.outcomes]
def terminal(args, identity, expected):
    local = status(args, identity)
    if local["phase"] != "terminal":
        return None
    if local["outcome"] != expected:
        raise RuntimeError(f"Canonical outcome mismatch: {local!r}")
    state = "failure" if expected == "DomainFailure" else "success"
    exact = [item for item in local["statuses"]
             if item["plan"]["state"] == state and item["phase"] == "published"]
    if len(exact) != 1:
        return None
    intent = exact[0]
    remote_statuses = api(f"commits/{local['source']['revision']['native_id']}/statuses?per_page=100")
    match = [item for item in remote_statuses if item["id"] == intent["publication"]["status_id"]]
    if not match:
        return None
    observed = match[0]
    expected_description = f"Kiln run {identity} state {state}"
    if (observed["state"], observed["context"], observed["description"]) != (
            state, intent["plan"]["context"], expected_description):
        raise RuntimeError("Published intent does not match its real remote status")
    return {"local": local, "remote_status": {key: observed[key] for key in
            ("id", "node_id", "state", "context", "description", "url", "created_at")}}


def host_script(script, host, *args):
    return run(["python3", REPO_ROOT / "scripts" / script, "--host", host, *args]).decode()


# r[impl kiln.live_ci.recovery]
def interrupt(args, revision, before_logs):
    def running():
        record = record_for(args, revision)
        logs = worker_logs(args)
        new = set(logs) - set(before_logs)
        if record and record["phase"] == "active" and record.get("observation") and len(new) == 1:
            log = logs[next(iter(new))]
            if "behavior probe started" in log["output"] and log["exit_code"] is None:
                return {"record": record, "logs": logs}
        return None
    before = until("durable native acceptance and running real probe", running)
    unit = "kiln-live-worker-lattice.service"
    executor_pid = remote(args.worker, ["systemctl", "show", unit, "--property=MainPID", "--value"]).decode().strip()
    stopped = [host_script("live-ci-controller.py", args.controller, "stop", "github"),
               host_script("live-ci-controller.py", args.controller, "stop", "proxy"),
               host_script("live-ci-worker.py", args.worker, "stop", "iroh")]
    during = worker_logs(args)
    surviving_pid = remote(args.worker, ["systemctl", "show", unit, "--property=MainPID", "--value"]).decode().strip()
    if executor_pid == "0" or executor_pid != surviving_pid:
        raise RuntimeError("Interruption did not preserve the original Lattice executor")
    active_logs = set(before["logs"]) - set(before_logs)
    if any(during[name]["exit_code"] is not None for name in active_logs):
        raise RuntimeError("The real worker probe did not remain active across interruption")
    # r[verify kiln.live_ci.recovery]
    started = [host_script("live-ci-worker.py", args.worker, "start", "iroh"),
               host_script("live-ci-controller.py", args.controller, "start", "proxy"),
               host_script("live-ci-controller.py", args.controller, "start", "github")]
    return {"before": before, "during": during, "executor_pid": executor_pid,
            "surviving_executor_pid": surviving_pid, "stop_commands": stopped, "start_commands": started}


def exercise(args):
    checkout = Path(args.checkout).resolve()
    if not args.branch.startswith("kiln-live-ci-validation-"):
        raise ValueError("Use a dedicated operator-authorized validation bookmark")
    probe = checkout / "crates/kiln-standalone/examples/live_ci_probe.rs"
    if args.case != "pass":
        run(["jj", "new", "--message", f"Live CI {args.case} validation"], cwd=checkout)
        text = probe.read_text()
        old, new = ("Some(Outcome::DomainFailure)", "Some(Outcome::Success)")
        if args.case == "recovery":
            old, new = new, old
        if text.count(old) != 1:
            raise ValueError("Exact reviewed probe mutation is absent or ambiguous")
        probe.write_text(text.replace(old, new))
        run(["jj", "bookmark", "set", args.branch, "--revision", "@"], cwd=checkout)
    revision = run(["jj", "log", "--no-graph", "--revisions", "@", "--template", "commit_id"], cwd=checkout).decode().strip()
    before_logs = worker_logs(args)
    result = {"schema": "kiln.live-ci-case.v1", "case": args.case, "revision": revision,
              "branch": args.branch, "hook_id": args.hook_id, "verdict": "INCOMPLETE"}
    try:
        # r[impl kiln.live_ci.composition] r[verify kiln.live_ci.composition]
        run(["jj", "git", "push", "--bookmark", args.branch], cwd=checkout)
        admitted = until("genuine GitHub delivery acknowledgement", lambda: delivery(args, revision))
        result["delivery"] = admitted
        identity = admitted["acknowledgement"]["run_identity"]
        if args.case == "recovery":
            result["interruption"] = interrupt(args, revision, before_logs)
        expected = "DomainFailure" if args.case == "fail" else "Success"
        result["terminal"] = until("matching canonical and Published real GitHub terminal status",
                                   lambda: terminal(args, identity, expected))
        final = record_for(args, revision)
        if final["run_identity"] != identity or not final["observation"]["terminal"]:
            raise RuntimeError("Terminal native observation is missing or substituted")
        result["record"] = final
        after_logs = worker_logs(args)
        new_logs = set(after_logs) - set(before_logs)
        if len(new_logs) != 1:
            raise RuntimeError("Expected exactly one real worker probe execution")
        actual_log = after_logs[next(iter(new_logs))]
        expected_code = 101 if args.case == "fail" else 0
        if actual_log["exit_code"] != expected_code:
            raise RuntimeError(f"Probe exit differs from expected real assertion result: {actual_log!r}")
        result["worker_probe"] = {name: after_logs[name] for name in new_logs}
        if args.case == "recovery":
            prior = result["interruption"]["before"]["record"]["observation"]["execution"]
            if prior != final["observation"]["execution"]:
                raise RuntimeError("Recovery changed the originally accepted execution")
        if args.case == "fail":
            api(f"hooks/{args.hook_id}/deliveries/{admitted['id']}/attempts", "POST")
            def redelivered():
                value = delivery(args, revision, redelivery=True)
                return value if value and value["redelivery"] and value["acknowledgement"].get("duplicate") else None
            replay = until("genuine duplicate GitHub redelivery", redelivered)
            # r[verify kiln.live_ci.outcomes]
            if replay["acknowledgement"]["run_identity"] != identity or worker_logs(args) != after_logs:
                raise RuntimeError("Replay changed the canonical run or executed new work")
            result["replay"] = replay
        result["verdict"] = "PASS"
    except BaseException as error:
        result["error"] = str(error)
        raise
    finally:
        Path(args.output).write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps({"case": args.case, "revision": revision, "verdict": result["verdict"], "evidence": args.output}), flush=True)


# r[impl kiln.live_ci.authority]
def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    create = commands.add_parser("create-hook")
    create.add_argument("--url", required=True)
    create.add_argument("--webhook-file", required=True)
    create.add_argument("--output", required=True)
    case = commands.add_parser("run")
    for name in ("checkout", "branch", "hook-id", "output"):
        case.add_argument("--" + name, required=True)
    case.add_argument("--case", choices=("pass", "fail", "recovery"), required=True)
    case.add_argument("--controller", default="root@aspen1.local")
    case.add_argument("--worker", default="root@localhost")
    retire = commands.add_parser("retire-hook")
    retire.add_argument("--hook-id", required=True)
    args = parser.parse_args()
    if args.command == "create-hook":
        secret_path = Path(args.webhook_file)
        if secret_path.stat().st_mode & 0o077:
            raise ValueError("Webhook handoff file must be private")
        hook = api("hooks", "POST", {"name": "web", "active": True, "events": ["push"],
            "config": {"url": args.url, "content_type": "json", "insecure_ssl": "0", "secret": secret_path.read_text()}})
        receipt = {"id": hook["id"], "url": hook["config"]["url"], "events": hook["events"],
                   "active": hook["active"], "created_at": hook["created_at"]}
        Path(args.output).write_text(json.dumps(receipt, indent=2) + "\n")
        print(json.dumps(receipt))
    elif args.command == "retire-hook":
        api(f"hooks/{args.hook_id}", "DELETE")
        print(json.dumps({"retired_hook_id": args.hook_id}))
    else:
        exercise(args)


if __name__ == "__main__":
    main()
