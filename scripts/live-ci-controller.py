#!/usr/bin/env python3
"""Own only the isolated, operator-authorized live CI controller deployment.

Bootstrap takes immutable realized store paths, never a source build or secret
argument. Native service and peer inputs are supplied separately by their owners.
Runtime units are transient, unprivileged, and independent of the Lattice worker.
"""

import argparse
import copy
import json
import os
from pathlib import Path
import re
import secrets
import shlex
import subprocess
import time

ROOT = "/var/tmp/kiln-live-ci-20261018"
USER = "kiln-live-controller"
UNIT_PREFIX = "kiln-live-controller"
PORT = 8787
INGRESS_READY_SECONDS = 60
LISTENER_READY_SECONDS = 30
READY_POLL_SECONDS = 1


def run(argv, **kwargs):
    return subprocess.run(argv, check=True, **kwargs)


def remote(host, command, data=None):
    return run(["ssh", "-o", "BatchMode=yes", host, command], input=data,
               stdout=subprocess.PIPE).stdout


def quote(value):
    return shlex.quote(str(value))


def put(host, relative, content):
    path = f"{ROOT}/{relative}"
    remote(host, f"set -eu; umask 077; cat > {quote(path)}; chown {USER}:{USER} {quote(path)}",
           content if isinstance(content, bytes) else content.encode())


def settings(host):
    return json.loads(remote(host, f"cat {ROOT}/deployment.json"))


def save_settings(host, value):
    put(host, "deployment.json", json.dumps(value, indent=2) + "\n")


def start_unit(host, name, command):
    unit = f"{UNIT_PREFIX}-{name}"
    remote(host, f"set -eu; umask 077; touch {ROOT}/{name}.log; "
           f"chmod 0600 {ROOT}/{name}.log; chown {USER}:{USER} {ROOT}/{name}.log")
    remote(host, " ".join(map(quote, [
        "systemd-run", f"--unit={unit}", f"--uid={USER}", f"--gid={USER}",
        f"--working-directory={ROOT}", "--collect",
        "--property=UMask=0077", "--property=NoNewPrivileges=yes",
        f"--property=Environment=HOME={ROOT}",
        f"--property=StandardOutput=append:{ROOT}/{name}.log",
        f"--property=StandardError=append:{ROOT}/{name}.log", *command,
    ])))
    return unit + ".service"


def bootstrap(args):
    for path in (args.package, args.secretspec):
        if not re.fullmatch(r"/nix/store/[a-z0-9]{32}-[^/]+", path):
            raise ValueError("Supply an immutable realized Nix store output")
    # Validate the protected handoff directory before obtaining credentials.
    handoff = Path(args.webhook_file)
    parent = handoff.parent.stat()
    if parent.st_uid != os.getuid() or parent.st_mode & 0o077:
        raise ValueError("Webhook handoff parent must be owned by caller, mode 0700")
    if handoff.exists() or handoff.is_symlink():
        raise FileExistsError(handoff)
    remote(args.host, f"set -eu; test ! -e {ROOT}; "
           f"! getent passwd {USER}; "
           f"test -z \"$(ss -H -ltn 'sport = :{PORT}')\"; "
           f"useradd --system --user-group --home-dir {ROOT} --shell /run/current-system/sw/bin/nologin {USER}; "
           f"install -d -m 0700 -o {USER} -g {USER} {ROOT} {ROOT}/secrets {ROOT}/secrets/.secrets "
           f"{ROOT}/secrets/.secrets/kiln-live {ROOT}/secrets/.secrets/kiln-live/default; "
           f"id {USER}")
    run(["nix-copy-closure", "--to", args.host, args.package, args.secretspec])
    token = run(["gh", "auth", "token"], stdout=subprocess.PIPE).stdout.strip()
    if not token or b"\n" in token:
        raise ValueError("GitHub credential capture did not produce one token")
    webhook = secrets.token_hex(32).encode()
    fd = os.open(handoff, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "wb") as output:
        output.write(webhook)
    put(args.host, "secrets/.secrets/kiln-live/default/STATUS_TOKEN", token)
    put(args.host, "secrets/.secrets/kiln-live/default/WEBHOOK_SECRET", webhook)
    put(args.host, "secrets/secretspec.toml", '''[project]
name = "kiln-live"
revision = "1.0"

[providers]
local = "file:./.secrets"

[profiles.default]
WEBHOOK_SECRET = { description = "Isolated validation webhook HMAC", providers = ["local"] }
STATUS_TOKEN = { description = "Temporary operator-authorized GitHub status credential", providers = ["local"] }
''')
    data = {"root": ROOT, "user": USER, "package": args.package,
            "secretspec": args.secretspec + "/bin/secretspec", "port": PORT}
    save_settings(args.host, data)
    public = remote(args.host, " ".join(map(quote, [
        "runuser", "-u", USER, "--", args.package + "/bin/kiln-adapter-iroh",
        "keygen", "--key", ROOT + "/controller.key",
    ]))).decode().strip()
    put(args.host, "controller-public.txt", public + "\n")
    # Exercise the real provider, compare privately, and print only digests.
    check = (f"set -eu; umask 077; cd {ROOT}/secrets; "
             f"runuser -u {USER} -- {quote(data['secretspec'])} get WEBHOOK_SECRET "
             f"--profile default --provider local --reason live-ci-provider-proof > {ROOT}/resolved-secret; "
             f"test \"$(cat {ROOT}/resolved-secret)\" = \"$(cat {ROOT}/secrets/.secrets/kiln-live/default/WEBHOOK_SECRET)\"; "
             f"rm {ROOT}/resolved-secret; "
             f"b3sum {quote(data['secretspec'])} {ROOT}/secrets/.secrets/kiln-live/default/WEBHOOK_SECRET; "
             f"id {USER}; stat -c '%a %U %n' {ROOT} {ROOT}/controller.key "
             f"{ROOT}/secrets/.secrets/kiln-live/default/STATUS_TOKEN")
    print(remote(args.host, check).decode(), end="")
    print(json.dumps({"controller_public": public, "webhook_file": str(handoff), **data}))


def configure_proxy(args):
    data = settings(args.host)
    native = json.loads(Path(args.native).read_text())
    endpoint = json.loads(Path(args.endpoint).read_text())
    uid = int(remote(args.host, f"id -u {USER}"))
    transport = copy.deepcopy(native["transport"])
    transport["peers"]["allowed_user_ids"] = [uid]
    profile = {
        "schema": "kiln.iroh-workflow-profile.v1", "iroh_version": "0.95.1",
        "alpn": "kiln.workflow.v1", "socket_path": ROOT + "/iroh.sock",
        "bind_address": args.bind, "allowed_peers": [endpoint["id"]],
        "relay_urls": args.relay, "transport": transport,
        "contract": native["contract"], "exchange_timeout_ms": args.exchange_timeout_ms,
    }
    # The adapter itself validates EndpointAddr identity during normal startup.
    put(args.host, "proxy.json", json.dumps(profile, indent=2) + "\n")
    put(args.host, "worker-endpoint.json", json.dumps(endpoint) + "\n")
    command = [data["package"] + "/bin/kiln-adapter-iroh", "validate",
               "--profile", ROOT + "/proxy.json"]
    print(remote(args.host, " ".join(map(quote, ["runuser", "-u", USER, "--", *command]))).decode(), end="")


def start(args):
    data = settings(args.host)
    if args.component == "proxy":
        command = [data["package"] + "/bin/kiln-adapter-iroh", "proxy",
                   "--profile", ROOT + "/proxy.json", "--key", ROOT + "/controller.key",
                   "--remote", ROOT + "/worker-endpoint.json"]
    elif args.component == "github":
        command = [data["package"] + "/bin/kiln-adapter-github", "--profile", ROOT + "/github-ci.json"]
    else:
        if not args.cloudflared or not args.cloudflared.startswith("/nix/store/"):
            raise ValueError("HTTPS ingress requires a realized --cloudflared store output")
        run(["nix-copy-closure", "--to", args.host, args.cloudflared])
        command = [args.cloudflared + "/bin/cloudflared", "tunnel", "--no-autoupdate",
                   "--url", f"http://127.0.0.1:{PORT}", "--protocol", "http2",
                   "--metrics", "127.0.0.1:0"]
        data["cloudflared"] = args.cloudflared
        save_settings(args.host, data)
        log_offset = int(remote(args.host, f"if test -f {ROOT}/https.log; then "
                                f"stat -c %s {ROOT}/https.log; else echo 0; fi"))
    print(start_unit(args.host, args.component, command))
    if args.component == "https":
        # Bounded readiness wait inspects only this owned unit's private log.
        for _ in range(INGRESS_READY_SECONDS):
            log = remote(args.host, f"cat {ROOT}/https.log")[log_offset:].decode()
            match = re.search(r"https://[a-z0-9-]+\.trycloudflare\.com", log)
            if match:
                print(match.group())
                return
            time.sleep(READY_POLL_SECONDS)
        raise RuntimeError("Owned ingress has not announced its public URL; inspect private https.log")
    if args.component == "proxy":
        for _ in range(LISTENER_READY_SECONDS):
            result = remote(args.host, f"if test -S {ROOT}/iroh.sock; then echo ready; fi")
            if result.strip() == b"ready":
                print("private proxy socket ready")
                return
            time.sleep(READY_POLL_SECONDS)
        raise RuntimeError("Proxy socket not ready; inspect private proxy.log")
    if args.component == "github":
        for _ in range(LISTENER_READY_SECONDS):
            result = remote(args.host,
                            f"if systemctl is-active --quiet {UNIT_PREFIX}-github.service "
                            f"&& test -n \"$(ss -H -ltn 'src 127.0.0.1 sport = :{PORT}')\"; "
                            "then echo ready; fi")
            if result.strip() == b"ready":
                print("loopback webhook listener ready")
                return
            time.sleep(READY_POLL_SECONDS)
        raise RuntimeError("GitHub listener not ready; inspect private github.log")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="root@aspen1.local")
    commands = parser.add_subparsers(dest="action", required=True)
    bootstrap_parser = commands.add_parser("bootstrap")
    bootstrap_parser.add_argument("--package", required=True)
    bootstrap_parser.add_argument("--secretspec", required=True)
    bootstrap_parser.add_argument("--webhook-file", required=True)
    configure = commands.add_parser("configure-proxy")
    configure.add_argument("--native", required=True)
    configure.add_argument("--endpoint", required=True)
    configure.add_argument("--bind", required=True)
    configure.add_argument("--relay", action="append", default=[])
    configure.add_argument("--exchange-timeout-ms", type=int, required=True)
    stage = commands.add_parser("stage-github")
    stage.add_argument("--profile", required=True)
    start_parser = commands.add_parser("start")
    start_parser.add_argument("component", choices=["proxy", "github", "https"])
    start_parser.add_argument("--cloudflared")
    stop = commands.add_parser("stop")
    stop.add_argument("component", choices=["proxy", "github", "https"])
    commands.add_parser("cleanup-credentials")
    args = parser.parse_args()
    if args.action == "bootstrap":
        bootstrap(args)
    elif args.action == "configure-proxy":
        configure_proxy(args)
    elif args.action == "stage-github":
        put(args.host, "github-ci.json", Path(args.profile).read_bytes())
        print(remote(args.host, f"b3sum {ROOT}/github-ci.json").decode(), end="")
    elif args.action == "start":
        start(args)
    elif args.action == "stop":
        remote(args.host, f"systemctl stop {UNIT_PREFIX}-{args.component}.service")
    else:
        remote(args.host, f"set -eu; systemctl stop {UNIT_PREFIX}-github.service "
               f"|| test \"$(systemctl show {UNIT_PREFIX}-github.service --property=LoadState --value)\" = not-found; "
               f"rm -f {ROOT}/secrets/.secrets/kiln-live/default/STATUS_TOKEN "
               f"{ROOT}/secrets/.secrets/kiln-live/default/WEBHOOK_SECRET {ROOT}/resolved-secret")
        print("Controller runtime credential copies retired; remove the private local webhook handoff file separately.")


if __name__ == "__main__":
    main()
