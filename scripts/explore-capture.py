#!/usr/bin/env python3
"""Bounded retained UI capture for named exploration cases.

Drives a supplied Chromium executable through the Chrome DevTools Protocol pipe
using only the Python standard library. This is not CI, provider, review or
deployment evidence.
"""
# r[impl kiln.web.exploration.cases.capture] Readiness is asserted before capture; outputs stay within named limits and carry provenance.
from __future__ import annotations

import argparse
import base64
import json
import os
import select
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request
from pathlib import Path

MAX_SCREENSHOTS = 1
MAX_SEMANTIC_BYTES = 262144
MAX_MANIFEST_BYTES = 131072
MAX_AX_NODES = 256
MAX_OUTPUT_FILES = 8
NAVIGATION_TIMEOUT_SECONDS = 30
READINESS_TIMEOUT_SECONDS = 15
CAPTURE_TIMEOUT_SECONDS = 120
MIN_VIEWPORT_WIDTH = 320
MIN_VIEWPORT_HEIGHT = 240
MAX_VIEWPORT_WIDTH = 3840
MAX_VIEWPORT_HEIGHT = 2160
MAX_DEVTOOLS_MESSAGE_BYTES = 64 * 1024 * 1024
DEVTOOLS_COMMAND_FD = 3
DEVTOOLS_REPLY_FD = 4
SUBJECT_PRESENT_EXPRESSION = (
    "(id => document.body.textContent.includes(id) || Array.from(document.querySelectorAll('*'))"
    ".some(el => Array.from(el.attributes).some(attribute => attribute.value.includes(id))))(%s)"
)


class CaptureRefused(Exception):
    pass


class CDP:
    """Chromium `--remote-debugging-pipe`: commands on fd 3, NUL-terminated JSON replies on fd 4."""

    def __init__(self, command_fd, reply_fd):
        self.command_fd = command_fd
        self.reply_fd = reply_fd
        self.buffer = bytearray()
        self.next_id = 1
        self.network_failures = []

    def send(self, message):
        data = json.dumps(message).encode("utf-8") + b"\0"
        while data:
            written = os.write(self.command_fd, data)
            data = data[written:]

    def read_message(self, timeout):
        deadline = time.monotonic() + timeout
        while b"\0" not in self.buffer:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("timed out waiting for a DevTools message")
            ready, _, _ = select.select([self.reply_fd], [], [], min(0.1, remaining))
            if ready:
                chunk = os.read(self.reply_fd, 65536)
                if not chunk:
                    raise EOFError("Chromium closed the DevTools pipe")
                self.buffer.extend(chunk)
                if len(self.buffer) > MAX_DEVTOOLS_MESSAGE_BYTES:
                    raise CaptureRefused("DevTools message exceeds byte limit")
        body, _, rest = bytes(self.buffer).partition(b"\0")
        self.buffer = bytearray(rest)
        return json.loads(body.decode("utf-8"))

    def command(self, method, params=None, session_id=None, timeout=10):
        command_id = self.next_id
        self.next_id += 1
        message = {"id": command_id, "method": method}
        if params is not None:
            message["params"] = params
        if session_id is not None:
            message["sessionId"] = session_id
        self.send(message)
        deadline = time.monotonic() + timeout
        while True:
            if time.monotonic() > deadline:
                raise TimeoutError(f"timed out waiting for {method}")
            message = self.read_message(max(0.1, deadline - time.monotonic()))
            if message.get("method") == "Network.loadingFailed":
                self.network_failures.append(message.get("params", {}))
            if message.get("id") == command_id:
                if "error" in message:
                    raise RuntimeError(f"{method} failed: {message['error']}")
                return message.get("result", {})


def parse_viewport(value):
    parts = value.lower().split("x")
    if len(parts) != 2:
        raise ValueError("viewport must be WIDTHxHEIGHT")
    width, height = int(parts[0]), int(parts[1])
    if width < MIN_VIEWPORT_WIDTH or height < MIN_VIEWPORT_HEIGHT or width > MAX_VIEWPORT_WIDTH or height > MAX_VIEWPORT_HEIGHT:
        raise ValueError("viewport outside bounded range")
    return width, height


def fetch_resolution(origin, case_id, recipe_revision, workspace):
    url = origin.rstrip("/") + "/examples/cases/" + urllib.parse.quote(case_id, safe="")
    query = urllib.parse.urlencode({"recipe": recipe_revision, "workspace": workspace})
    try:
        with urllib.request.urlopen(url + "?" + query, timeout=10) as response:
            payload = json.loads(response.read(MAX_MANIFEST_BYTES + 1))
    except urllib.error.HTTPError as refusal:
        raise CaptureRefused(f"case resolution returned HTTP {refusal.code}") from None
    if not isinstance(payload, dict):
        raise CaptureRefused("malformed case resolution")
    if payload.get("case_id") != case_id:
        raise CaptureRefused("case resolution identity mismatch")
    if (payload.get("recipe") or {}).get("revision") != recipe_revision:
        raise CaptureRefused("recipe revision mismatch")
    if payload.get("workspace") != workspace:
        raise CaptureRefused("workspace identity mismatch")
    return payload


def evaluate(cdp, session_id, expression):
    result = cdp.command("Runtime.evaluate", {
        "expression": expression,
        "returnByValue": True,
        "awaitPromise": True,
    }, session_id=session_id, timeout=10)
    remote = result.get("result", {})
    if remote.get("subtype") == "error":
        raise CaptureRefused(f"JavaScript evaluation failed: {remote.get('description')}")
    return remote.get("value")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--browser", required=True)
    parser.add_argument("--origin", required=True)
    parser.add_argument("--case", required=True)
    parser.add_argument("--workspace", required=True)
    parser.add_argument("--recipe-revision", required=True)
    parser.add_argument("--out-dir", required=True)
    parser.add_argument("--viewport", default="1280x800")
    parser.add_argument("--theme", default="system", choices=["light", "dark", "system"])
    parser.add_argument("--density", default="comfortable", choices=["comfortable", "compact"])
    parser.add_argument("--timeout", type=int, default=CAPTURE_TIMEOUT_SECONDS)
    args = parser.parse_args()

    try:
        viewport = parse_viewport(args.viewport)
    except ValueError as cause:
        raise CaptureRefused(str(cause))

    if len(args.recipe_revision) != 64:
        raise CaptureRefused("recipe revision must be a 64-character BLAKE3 hex digest")

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    if len(list(out_dir.iterdir())) > MAX_OUTPUT_FILES:
        raise CaptureRefused("output directory exceeds bounded file count")

    resolution = fetch_resolution(args.origin, args.case, args.recipe_revision, args.workspace)
    subjects = resolution.get("subjects") or {}
    route = resolution.get("route")
    if not isinstance(route, str) or not route.startswith("/"):
        raise CaptureRefused("case resolution did not provide a console route")
    page_url = args.origin.rstrip("/") + route
    separator = "&" if "?" in page_url else "?"
    page_url = f"{page_url}{separator}theme={urllib.parse.quote(args.theme)}&density={urllib.parse.quote(args.density)}"

    started = time.monotonic()
    profile = tempfile.TemporaryDirectory(prefix="kiln-explore-capture-")
    command_read, command_write = os.pipe()
    reply_read, reply_write = os.pipe()
    child_fds = (os.dup(command_read), os.dup(reply_write))

    def bind_devtools_pipe():
        os.dup2(child_fds[0], DEVTOOLS_COMMAND_FD)
        os.dup2(child_fds[1], DEVTOOLS_REPLY_FD)

    process = subprocess.Popen(
        [args.browser, "--headless", "--remote-debugging-pipe", f"--user-data-dir={profile.name}",
         "--disable-gpu", "--no-first-run", "--no-default-browser-check",
         "--disable-dev-shm-usage", "--no-sandbox", "about:blank"],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        pass_fds=(DEVTOOLS_COMMAND_FD, DEVTOOLS_REPLY_FD),
        preexec_fn=bind_devtools_pipe,
    )
    for descriptor in (command_read, reply_write, *child_fds):
        os.close(descriptor)
    cdp = CDP(command_write, reply_read)
    try:
        version = cdp.command("Browser.getVersion", timeout=10)
        browser_version = version.get("product") or version.get("revision") or "unknown"
        target = cdp.command("Target.createTarget", {"url": "about:blank"}, timeout=10)
        session_id = cdp.command("Target.attachToTarget", {
            "targetId": target["targetId"], "flatten": True
        }, timeout=10)["sessionId"]
        cdp.command("Runtime.enable", session_id=session_id, timeout=10)
        cdp.command("Page.enable", session_id=session_id, timeout=10)
        cdp.command("Network.enable", session_id=session_id, timeout=10)
        cdp.command("Accessibility.enable", session_id=session_id, timeout=10)
        cdp.command("Emulation.setDeviceMetricsOverride", {
            "width": viewport[0], "height": viewport[1], "deviceScaleFactor": 1, "mobile": False,
        }, session_id=session_id, timeout=10)

        cdp.command("Page.navigate", {"url": page_url}, session_id=session_id, timeout=10)
        # Wait for load event.
        deadline = time.monotonic() + NAVIGATION_TIMEOUT_SECONDS
        loaded = False
        while time.monotonic() < deadline:
            try:
                message = cdp.read_message(0.5)
            except TimeoutError:
                continue
            if message.get("method") == "Page.loadEventFired":
                loaded = True
                break
            if message.get("method") == "Network.loadingFailed":
                cdp.network_failures.append(message.get("params", {}))
        if not loaded:
            raise CaptureRefused("page did not finish loading")
        if cdp.network_failures:
            raise CaptureRefused("a required network resource failed to load")

        # Poll readiness until all conditions hold or timeout.
        readiness_deadline = time.monotonic() + READINESS_TIMEOUT_SECONDS
        while True:
            if cdp.network_failures:
                raise CaptureRefused("a required network resource failed to load")
            ready_state = evaluate(cdp, session_id, "document.readyState")
            has_main = evaluate(cdp, session_id, "!!document.querySelector('main')")
            # The console shortens identities in visible text and keeps exact values in
            # text or attributes; a subject counts once its exact value is in the document.
            subjects_present = all(
                bool(evaluate(cdp, session_id, SUBJECT_PRESENT_EXPRESSION % json.dumps(str(subject_id))))
                for subject_id in subjects.values()
            )
            busy = evaluate(cdp, session_id, "!!document.querySelector('[aria-busy=true]')")
            fonts_status = evaluate(cdp, session_id, "document.fonts ? document.fonts.status : 'unsupported'")
            font_error = evaluate(cdp, session_id, "Array.from(document.fonts || []).some(f => f.status === 'error')")
            unmet = [name for name, held in (
                ("document complete", ready_state == "complete"), ("main landmark", has_main),
                ("resolved subjects", subjects_present), ("no busy region", not busy),
                ("fonts loaded", fonts_status == "loaded" and not font_error),
            ) if not held]
            if not unmet:
                break
            if time.monotonic() > readiness_deadline:
                raise CaptureRefused("page did not reach a ready state: " + ", ".join(unmet))
            time.sleep(0.5)
        dpr = evaluate(cdp, session_id, "window.devicePixelRatio")
        observed_viewport = evaluate(cdp, session_id, "[window.innerWidth, window.innerHeight]")
        if observed_viewport != [viewport[0], viewport[1]]:
            raise CaptureRefused(f"observed viewport {observed_viewport} differs from requested {list(viewport)}")
        observed_theme = evaluate(cdp, session_id, "document.documentElement.dataset.theme || document.documentElement.className || ''")
        observed_density = evaluate(cdp, session_id, "document.documentElement.dataset.density || ''")

        screenshot = cdp.command("Page.captureScreenshot", {"format": "png"}, session_id=session_id, timeout=30)
        screenshot_bytes = base64.b64decode(screenshot["data"])
        screenshot_path = out_dir / "screenshot.png"
        screenshot_path.write_bytes(screenshot_bytes)

        ax_tree = cdp.command("Accessibility.getFullAXTree", session_id=session_id, timeout=30)
        ax_nodes = (ax_tree.get("nodes") or [])[:MAX_AX_NODES]

        semantic = {
            "title": evaluate(cdp, session_id, "document.title"),
            "url": evaluate(cdp, session_id, "location.href"),
            "headings": evaluate(cdp, session_id, "Array.from(document.querySelectorAll('h1,h2,h3')).map(e=>e.textContent.trim()).filter(Boolean).slice(0,32)"),
            "button_labels": evaluate(cdp, session_id, "Array.from(document.querySelectorAll('button,[role=button]')).map(e=>e.textContent.trim()).filter(Boolean).slice(0,64)"),
            "link_hrefs": evaluate(cdp, session_id, "Array.from(document.querySelectorAll('a[href]')).map(e=>e.getAttribute('href')).filter(Boolean).slice(0,128)"),
            "accessibility_nodes": ax_nodes,
            "ready": True,
        }
        semantic_bytes = json.dumps(semantic).encode("utf-8")
        if len(semantic_bytes) > MAX_SEMANTIC_BYTES:
            raise CaptureRefused("semantic observation exceeds byte limit")
        (out_dir / "semantic.json").write_bytes(semantic_bytes)

        fonts = evaluate(cdp, session_id, "Array.from(document.fonts).map(f => ({family: f.family, status: f.status}))")
        resources = evaluate(cdp, session_id, "Array.from(document.querySelectorAll('link[rel=stylesheet],script[src],img[src]')).map(e => e.href || e.src).filter(Boolean).slice(0,128)")

        manifest = {
            "source": resolution.get("source_revision") or args.origin,
            "workspace": args.workspace,
            "case_id": args.case,
            "recipe_revision": args.recipe_revision,
            "recipe": resolution.get("recipe"),
            "resolved_subjects": subjects,
            "route": route,
            "browser_version": browser_version,
            "tool_version": "explore-capture/2",
            "viewport": {"width": viewport[0], "height": viewport[1], "device_pixel_ratio": dpr},
            "appearance": {
                "requested_theme": args.theme,
                "observed_theme": observed_theme,
                "requested_density": args.density,
                "observed_density": observed_density,
            },
            "fonts": fonts,
            "resources": resources,
            "readiness": {
                "ready_state": ready_state,
                "has_main": has_main,
                "fonts_status": fonts_status,
                "network_failures": len(cdp.network_failures),
            },
            "limits": {
                "max_screenshots": MAX_SCREENSHOTS,
                "max_semantic_bytes": MAX_SEMANTIC_BYTES,
                "max_manifest_bytes": MAX_MANIFEST_BYTES,
                "max_ax_nodes": MAX_AX_NODES,
                "navigation_timeout_seconds": NAVIGATION_TIMEOUT_SECONDS,
                "readiness_timeout_seconds": READINESS_TIMEOUT_SECONDS,
            },
            "synthetic": True,
            "authority": "none",
        }
        manifest_bytes = json.dumps(manifest, indent=2).encode("utf-8")
        if len(manifest_bytes) > MAX_MANIFEST_BYTES:
            raise CaptureRefused("manifest exceeds byte limit")
        (out_dir / "manifest.json").write_bytes(manifest_bytes)
    finally:
        for descriptor in (command_write, reply_read):
            os.close(descriptor)
        try:
            process.terminate()
        except ProcessLookupError:
            pass
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=10)
        profile.cleanup()

    if time.monotonic() - started > args.timeout:
        raise CaptureRefused("capture exceeded time limit")
    print(f"captured {args.case} to {out_dir}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except CaptureRefused as cause:
        print(f"capture refused: {cause}", file=sys.stderr)
        raise SystemExit(2)
    except Exception as cause:  # pragma: no cover
        print(f"capture failed: {cause}", file=sys.stderr)
        raise SystemExit(1)
