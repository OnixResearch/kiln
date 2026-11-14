#!/usr/bin/env python3
"""Bounded native Bombadil campaigns over real, disposable Kiln examples."""
from __future__ import annotations

# r[impl kiln.web.exploration.browser_property_rail]
# r[impl kiln.web.exploration.browser_property_rail.scenario.isolation]
# r[impl kiln.web.exploration.browser_property_rail.scenario.evidence]

import argparse
import json
import os
from pathlib import Path
import queue
import shlex
import shutil
import signal
import ssl
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request

MAX_RESPONSE_BYTES = 1_048_576
MAX_EXAMPLES = 512
HTTP_TIMEOUT_SECONDS = 10
STARTUP_TIMEOUT_SECONDS = 30
TOOL_SETUP_ALLOWANCE_SECONDS = 60
STOP_TIMEOUT_SECONDS = 10
MAX_CAMPAIGN_SECONDS = 3600
DYNAMIC_LOADER_OVERRIDES = frozenset({"LD_LIBRARY_PATH", "LD_PRELOAD", "LD_AUDIT"})


class AdmissionError(ValueError):
    pass


def example_origin(value: str) -> str:
    """Admit one explicit origin, never credentials, a query or a subpath."""
    parsed = urllib.parse.urlsplit(value)
    if (parsed.scheme not in {"http", "https"} or not parsed.hostname
            or parsed.username is not None or parsed.password is not None
            or parsed.path not in {"", "/", "/examples"}
            or parsed.query or parsed.fragment):
        raise AdmissionError("Target must be an HTTP(S) example origin or its /examples URL")
    if parsed.scheme == "http" and parsed.hostname not in {"127.0.0.1", "localhost", "::1"}:
        raise AdmissionError("Non-loopback examples require HTTPS")
    try:
        port = parsed.port
    except ValueError as cause:
        raise AdmissionError("Invalid target port") from cause
    if (parsed.scheme, port) in {("http", 80), ("https", 443)}:
        port = None
    host = parsed.hostname.lower()
    if ":" in host:
        host = f"[{host}]"
    authority = host if port is None else f"{host}:{port}"
    return f"{parsed.scheme}://{authority}"


def admit_catalog(origin: str, catalog: object, session: object) -> dict:
    """Native examples, not an empty or production endpoint fallback."""
    if not isinstance(session, dict):
        raise AdmissionError("Example session must be an object")
    principal = session.get("principal")
    permissions = session.get("permissions")
    if (not isinstance(principal, dict) or principal.get("id") != "local:operator"
            or principal.get("role") != "operator" or not isinstance(permissions, dict)
            or permissions.get("read") is not True):
        raise AdmissionError("Target is not the synthetic local example session")
    examples = catalog.get("examples") if isinstance(catalog, dict) else None
    if not isinstance(examples, list) or not 1 <= len(examples) <= MAX_EXAMPLES:
        raise AdmissionError("Target lacks a bounded native example catalog")
    admitted = []
    for entry in examples:
        if not isinstance(entry, dict):
            raise AdmissionError("Malformed example catalog entry")
        title, href, detail = (entry.get(key) for key in ("title", "href", "detail"))
        if (not isinstance(title, str) or not title.strip() or len(title) > 512
                or not isinstance(href, str) or len(href) > 8192
                or not isinstance(detail, str) or len(detail) > 8192):
            raise AdmissionError("Malformed example title, route or description")
        route = urllib.parse.urlsplit(href)
        if route.scheme or route.netloc or route.path != "/" or not href.startswith("/"):
            raise AdmissionError("Example catalog route escapes the console")
        admitted.append({"title": title, "href": href, "detail": detail})
    return {"origin": origin, "examples": admitted}


class RefuseRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, new_url):
        raise AdmissionError("Example admission never follows redirects")


def fetch_json(opener, url: str):
    request = urllib.request.Request(url, headers={"Accept": "application/json"})
    try:
        with opener.open(request, timeout=HTTP_TIMEOUT_SECONDS) as response:
            if response.headers.get_content_type() != "application/json":
                raise AdmissionError(f"Expected native JSON at {url}")
            raw = response.read(MAX_RESPONSE_BYTES + 1)
    except urllib.error.HTTPError as cause:
        raise AdmissionError(f"Example endpoint refused {url}: HTTP {cause.code}") from cause
    if len(raw) > MAX_RESPONSE_BYTES:
        raise AdmissionError("Example admission response exceeds the byte bound")
    try:
        return json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as cause:
        raise AdmissionError("Malformed native example JSON") from cause


def executable(value: str) -> str:
    resolved = shutil.which(value)
    if not resolved:
        raise ValueError(f"Executable is unavailable: {value}")
    return str(Path(resolved).absolute())


def stop_owned(process: subprocess.Popen | None) -> dict:
    """Signal only the new session created by this runner."""
    if process is None:
        return {"owned": False}
    result = {"owned": True, "pid": process.pid, "escalated": False}
    if process.poll() is None:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            process.wait(timeout=STOP_TIMEOUT_SECONDS)
        except subprocess.TimeoutExpired:
            result["escalated"] = True
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait(timeout=STOP_TIMEOUT_SECONDS)
    result["exit_code"] = process.returncode
    return result


def start_example(program: str, output: Path, environment: dict) -> tuple:
    startup = queue.Queue()
    log = (output / "example.log").open("wb")
    try:
        process = subprocess.Popen([program, "--port", "0"], stdout=subprocess.PIPE,
                                   stderr=subprocess.STDOUT, env=environment, start_new_session=True)
    except BaseException:
        log.close()
        raise

    def capture():
        try:
            assert process.stdout is not None
            for line in iter(lambda: process.stdout.readline(65536), b""):
                log.write(line)
                log.flush()
                if line.startswith(b"Explore: "):
                    startup.put(line[len(b"Explore: "):].decode("utf-8").strip())
        finally:
            startup.put(None)
            log.close()

    reader = threading.Thread(target=capture, name="kiln-example-output", daemon=True)
    reader.start()
    return process, reader, startup


def campaign_seconds(value: str) -> int:
    try:
        seconds = int(value)
    except ValueError as cause:
        raise argparse.ArgumentTypeError("Time limit must be an integer number of seconds") from cause
    if not 1 <= seconds <= MAX_CAMPAIGN_SECONDS:
        raise argparse.ArgumentTypeError(f"Time limit must be between 1 and {MAX_CAMPAIGN_SECONDS} seconds")
    return seconds


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bombadil", default="bombadil")
    parser.add_argument("--program", default="kiln-website-exploration")
    parser.add_argument("--browser", default=os.environ.get("CHROME", "chromium"))
    parser.add_argument("--browser-arg", action="append", default=[])
    parser.add_argument("--specification", type=Path,
                        default=Path(__file__).resolve().parents[1] / "checks/bombadil/kiln-web-exploration.spec.ts")
    parser.add_argument("--url", help="Attach read-only to an existing native example workspace")
    parser.add_argument("--ca-certificate", type=Path)
    parser.add_argument("--time-limit", type=campaign_seconds, default=90, metavar="SECONDS")
    parser.add_argument("--output-path", type=Path, required=True)
    args = parser.parse_args()
    output = args.output_path.absolute()
    if output.exists() and (not output.is_dir() or any(output.iterdir())):
        parser.error("Output path must be absent or an empty directory; existing artifacts are never overwritten")
    output.mkdir(parents=True, exist_ok=True)
    report = {"schema": "kiln.web-exploration-bombadil.v1", "mode": "attach" if args.url else "owned",
              "started_at": time.time(), "time_limit_seconds": args.time_limit,
              "browser_arguments": args.browser_arg, "outcome": "error",
              "non_claims": ["Bounded property exploration, not exhaustive coverage or CI/provider correctness",
                             "Reset example identities prevent guaranteed replay against a fresh cohort",
                             "No production, release, provider-execution or attached-service mutation authority"]}
    child = tool_process = reader = None
    result = 1
    # Pinned executables carry their runtime closure; a dev shell must not replace it.
    environment = {key: value for key, value in os.environ.items()
                   if key.lower() not in {"http_proxy", "https_proxy", "all_proxy", "no_proxy"}
                   and key not in DYNAMIC_LOADER_OVERRIDES}
    report["removed_loader_variables"] = sorted(DYNAMIC_LOADER_OVERRIDES.intersection(os.environ))
    try:
        if args.url:
            origin = example_origin(args.url)
        else:
            program = executable(args.program)
            child, reader, startup = start_example(program, output, environment)
            report["example_command"] = [program, "--port", "0"]
            try:
                discovered = startup.get(timeout=STARTUP_TIMEOUT_SECONDS)
            except queue.Empty as cause:
                raise ValueError("Owned example did not print its URL before the startup deadline") from cause
            if discovered is None:
                raise ValueError("Owned example exited before reporting its URL; inspect example.log")
            origin = example_origin(discovered)
        report["origin"] = origin
        tls = ssl.create_default_context(cafile=str(args.ca_certificate) if args.ca_certificate else None)
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), RefuseRedirect(),
                                            urllib.request.HTTPSHandler(context=tls))
        context = admit_catalog(origin, fetch_json(opener, origin + "/examples.json"),
                                fetch_json(opener, origin + "/api/session"))
        (output / "web-exploration-context.json").write_text(json.dumps(context, indent=2) + "\n")
        staged_specification = output / "kiln-web-exploration.spec.ts"
        shutil.copyfile(args.specification, staged_specification)
        report["admission"] = {"examples": len(context["examples"]), "tls_hostname_verified": origin.startswith("https:"),
                               "ca_certificate": str(args.ca_certificate.absolute()) if args.ca_certificate else None,
                               "redirects": "refused", "ambient_proxies": "disabled"}
        bombadil, browser = executable(args.bombadil), executable(args.browser)
        report["tool_version"] = subprocess.check_output([bombadil, "--version"], text=True,
                                                         env=environment, timeout=HTTP_TIMEOUT_SECONDS).strip()
        report["browser"] = browser
        if args.browser_arg:
            launcher = output / "chrome-launcher"
            launcher.write_text("#!/bin/sh\nexec " + shlex.join([browser, *args.browser_arg]) + ' "$@"\n')
            launcher.chmod(0o700)
            environment["CHROME"] = str(launcher)
        else:
            environment["CHROME"] = browser
        command = [bombadil, "browser", "test", "--headless", "--exit-on-violation", "--time-limit",
                   f"{args.time_limit}s", "--output-path", str(output / "trace"), origin + "/examples",
                   str(staged_specification)]
        report["command"] = command
        with (output / "bombadil.log").open("wb") as log:
            tool_process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT,
                                            env=environment, start_new_session=True)
            try:
                result = tool_process.wait(timeout=args.time_limit + TOOL_SETUP_ALLOWANCE_SECONDS)
            except subprocess.TimeoutExpired as cause:
                raise ValueError("Bombadil exceeded the campaign plus setup deadline") from cause
        report["native_exit_code"] = result
        report["outcome"] = "pass" if result == 0 else "violation" if result == 2 else "error"
        report["trace"] = str(output / "trace")
    except AdmissionError as cause:
        report["outcome"], report["error"] = "refused", str(cause)
        print(f"Refused: {cause}", file=sys.stderr)
    except KeyboardInterrupt:
        report["error"], result = "Interrupted", 130
    except (OSError, ValueError, subprocess.SubprocessError, urllib.error.URLError) as cause:
        report["error"] = str(cause)
        print(f"Error: {cause}", file=sys.stderr)
    finally:
        report["tool_cleanup"] = stop_owned(tool_process)
        report["example_cleanup"] = stop_owned(child)
        if reader is not None:
            reader.join(timeout=STOP_TIMEOUT_SECONDS)
        if (report["tool_cleanup"].get("escalated") or report["example_cleanup"].get("escalated")
                or (child is not None and child.returncode != 0) or (reader is not None and reader.is_alive())):
            report["outcome"], report["cleanup_error"], result = "error", "Owned process cleanup was not graceful", 1
        report["finished_at"] = time.time()
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"outcome": report["outcome"], "exit_code": result, "report": str(output / "report.json")}))
    return result if result >= 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
