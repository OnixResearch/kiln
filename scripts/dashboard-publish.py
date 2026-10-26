#!/usr/bin/env python3
"""Generate the real offline dashboard and atomically deliver only admitted HTML."""
# r[impl kiln.dashboard.repositories.delivery]
# r[impl kiln.dashboard.repositories.delivery.scenario.failure]

import argparse
from html.parser import HTMLParser
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import sys
import tempfile
import time

MAX_CONFIG_BYTES = 65536
MAX_CATALOG_BYTES = 1048576
MAX_REPORT_BYTES = 67108864
MAX_TIMEOUT_SECONDS = 3600
MAX_INTERVAL_MS = 86400000
MAX_PATH_CHARACTERS = 4096


class PublicationError(ValueError):
    """A deliberately content-free, safe-to-log publication failure."""


def closed(value, keys, label):
    if not isinstance(value, dict) or set(value) != set(keys):
        raise PublicationError(f"invalid {label} fields")


def integer(value, maximum, label):
    if type(value) is not int or not 1 <= value <= maximum:
        raise PublicationError(f"invalid {label}")


def path_text(value):
    return (isinstance(value, str) and 0 < len(value) <= MAX_PATH_CHARACTERS
            and not any(ord(char) < 32 or ord(char) == 127 for char in value))


def admit_config(value):
    """Pure, closed, versioned admission; no files or credentials are resolved."""
    closed(value, {"schema_version", "dashboard_executable", "catalog", "profile",
                   "local_report", "destination", "max_report_bytes",
                   "subprocess_timeout_seconds", "interval_ms"}, "configuration")
    if type(value["schema_version"]) is not int or value["schema_version"] != 1:
        raise PublicationError("unsupported configuration version")
    for key in ("dashboard_executable", "catalog", "profile", "local_report"):
        if not path_text(value[key]):
            raise PublicationError(f"invalid {key}")
    integer(value["max_report_bytes"], MAX_REPORT_BYTES, "max_report_bytes")
    integer(value["subprocess_timeout_seconds"], MAX_TIMEOUT_SECONDS,
            "subprocess_timeout_seconds")
    integer(value["interval_ms"], MAX_INTERVAL_MS, "interval_ms")
    destination = value["destination"]
    if not isinstance(destination, dict):
        raise PublicationError("invalid destination")
    kind = destination.get("kind")
    if kind == "local":
        closed(destination, {"kind", "path"}, "destination")
    elif kind == "ssh":
        closed(destination, {"kind", "host", "path"}, "destination")
        host = destination["host"]
        if (not isinstance(host, str) or len(host) > 253
                or re.fullmatch(r"(?:[A-Za-z0-9_][A-Za-z0-9_.-]*@)?[A-Za-z0-9_][A-Za-z0-9_.-]*", host) is None):
            raise PublicationError("invalid SSH host")
    else:
        raise PublicationError("unsupported destination kind")
    target = destination["path"]
    if (not path_text(target) or not target.startswith("/")
            or target.rsplit("/", 1)[-1] in {"", ".", ".."}):
        raise PublicationError("destination must be an absolute report file path")
    return value


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise PublicationError("duplicate JSON field")
        result[key] = value
    return result


def read_json(path, maximum, label):
    with path.open("rb") as source:
        data = source.read(maximum + 1)
    if len(data) > maximum:
        raise PublicationError(f"{label} exceeds byte bound")
    try:
        return json.loads(data, object_pairs_hook=unique_object)
    except (UnicodeError, json.JSONDecodeError, RecursionError) as error:
        raise PublicationError(f"invalid {label} JSON") from error


class OfflineHTML(HTMLParser):
    """Admit complete HTML framing and reject external document resources.

    Inline scripts/styles belong to the configured trusted dashboard executable;
    this is resource/framing admission, not a JavaScript security sandbox.
    """

    def __init__(self, allow_embedded):
        super().__init__(convert_charrefs=True)
        self.allow_embedded = allow_embedded
        self.framing = []
        self.doctype = False
        self.in_style = False

    def handle_decl(self, decl):
        if decl.lower() != "doctype html" or self.doctype:
            raise PublicationError("report has invalid HTML declaration")
        self.doctype = True

    def handle_starttag(self, tag, attrs):
        if tag in {"html", "head", "body"}:
            self.framing.append(tag)
        if tag in {"base", "link", "frame", "object", "embed", "form"}:
            raise PublicationError("report is not self-contained")
        if tag == "iframe":
            attributes = dict(attrs)
            document = attributes.get("srcdoc")
            if (not self.allow_embedded or not isinstance(document, str)
                    or attributes.get("sandbox") != "allow-scripts"):
                raise PublicationError("report has an unsupported embedded document")
            admit_report(document.encode("utf-8"), MAX_REPORT_BYTES, allow_embedded=False)
        for name, value in attrs:
            if name in {"src", "srcset", "action", "formaction", "poster", "background", "data"}:
                raise PublicationError("report has an external resource attribute")
            if name in {"href", "xlink:href"} and not (value or "").startswith("#"):
                raise PublicationError("report has a nonlocal link")
            if name == "style" and re.search(r"url\s*\(|@import", value or "", re.I):
                raise PublicationError("report has an external style resource")
        attributes = dict(attrs)
        if tag == "meta" and attributes.get("http-equiv", "").lower() == "refresh":
            if re.fullmatch(r"[0-9]+", attributes.get("content", "")) is None:
                raise PublicationError("report has an external refresh")
        self.in_style = tag == "style" or self.in_style

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        self.handle_endtag(tag)

    def handle_endtag(self, tag):
        if tag in {"html", "head", "body"}:
            self.framing.append("/" + tag)
        if tag == "style":
            self.in_style = False

    def handle_data(self, data):
        if self.in_style and re.search(r"url\s*\(|@import", data, re.I):
            raise PublicationError("report has an external style resource")


def admit_report(data, maximum, *, allow_embedded=True):
    """Pure report admission, before either last-good destination is touched."""
    if not data or len(data) > maximum:
        raise PublicationError("report is empty or exceeds max_report_bytes")
    try:
        text = data.decode("utf-8", errors="strict")
    except UnicodeError as error:
        raise PublicationError("report is not UTF-8 HTML") from error
    if ("\x00" in text or not text.lstrip().lower().startswith("<!doctype html>")
            or not text.rstrip().lower().endswith("</html>")):
        raise PublicationError("report is incomplete HTML")
    parser = OfflineHTML(allow_embedded)
    parser.feed(text)
    parser.close()
    if not parser.doctype or parser.framing != ["html", "head", "/head", "body", "/body", "/html"]:
        raise PublicationError("report has invalid HTML framing")
    return data


def absolute(base, value):
    path = Path(value)
    return Path(os.path.abspath(path if path.is_absolute() else base / path))


def source_paths(configuration, config_path):
    """Shell-side path resolution and source collision protection."""
    base = config_path.parent
    paths = {key: absolute(base, configuration[key])
             for key in ("dashboard_executable", "catalog", "profile", "local_report")}
    protected = [config_path, paths["dashboard_executable"], paths["catalog"], paths["profile"]]
    catalog = read_json(paths["catalog"], MAX_CATALOG_BYTES, "catalog")
    try:
        for project in catalog["projects"]:
            for history in project["histories"]:
                source = absolute(paths["catalog"].parent, history["input"]["path"])
                protected.append(source)
                if history["input"]["kind"] == "store":
                    protected.extend(Path(str(source) + suffix)
                                     for suffix in ("-wal", "-shm", "-journal"))
    except (KeyError, TypeError, ValueError) as error:
        raise PublicationError("invalid catalog source paths") from error
    targets = [paths["local_report"], Path(configuration["destination"]["path"])]
    # Conservatively protect the same absolute source path even for SSH hosts.
    for target in targets:
        for source in protected:
            if target.resolve() == source.resolve() or (
                    target.exists() and source.exists() and os.path.samefile(target, source)):
                raise PublicationError("report destination collides with a source input")
    if targets[0].resolve() == targets[1].resolve():
        raise PublicationError("last-good and delivery destinations must be distinct")
    local_targets = targets if configuration["destination"]["kind"] == "local" else targets[:1]
    for target in local_targets:
        if not target.parent.is_dir() or target.is_symlink() or (target.exists() and not target.is_file()):
            raise PublicationError("report requires an existing parent and a regular-file destination")
    for key in ("catalog", "profile", "dashboard_executable"):
        if not paths[key].is_file():
            raise PublicationError(f"{key} is not a regular file")
    if not os.access(paths["dashboard_executable"], os.X_OK):
        raise PublicationError("dashboard executable is not executable")
    return paths


def checked_process(argv, timeout, label):
    """No child output is logged: it can contain raw histories or credentials."""
    with subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                          stderr=subprocess.DEVNULL, start_new_session=True) as process:
        try:
            code = process.wait(timeout=timeout)
        except BaseException as error:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()
            if isinstance(error, subprocess.TimeoutExpired):
                raise PublicationError(f"{label} timed out") from error
            raise
    if code:
        raise PublicationError(f"{label} failed (exit {code}); prior report retained")


def write_candidate(parent, data):
    fd, name = tempfile.mkstemp(prefix=".kiln-report-", suffix=".html", dir=parent)
    path = Path(name)
    try:
        with os.fdopen(fd, "wb") as target:
            target.write(data)
            target.flush()
            os.fsync(target.fileno())
    except BaseException:
        path.unlink(missing_ok=True)
        raise
    return path


def deliver(configuration, candidate, data):
    destination = configuration["destination"]
    if destination["kind"] == "local":
        target = Path(destination["path"])
        staged = write_candidate(target.parent, data)
        try:
            os.replace(staged, target)
        finally:
            staged.unlink(missing_ok=True)
    else:
        timeout = configuration["subprocess_timeout_seconds"]
        # --protect-args sends the destination path via rsync's protocol rather
        # than interpolating it into the remote shell. Default rsync temp+rename
        # is intentional: never use --inplace or --append here.
        ssh = ("ssh -T -o BatchMode=yes -o StrictHostKeyChecking=yes "
               f"-o ConnectTimeout={timeout}")
        checked_process(["rsync", "--protect-args", "--checksum", "--no-times",
                         "--chmod=F600", f"--timeout={timeout}", "--rsh", ssh,
                         "--", str(candidate), destination["host"] + ":" + destination["path"]],
                        timeout, "report transfer")


def publish(configuration, config_path, watch=False):
    paths = source_paths(configuration, config_path)
    fd, name = tempfile.mkstemp(prefix=".kiln-generation-", suffix=".html",
                                dir=paths["local_report"].parent)
    os.close(fd)
    generated = Path(name)
    candidate = None
    try:
        argv = [str(paths["dashboard_executable"]), "--catalog", str(paths["catalog"]),
                "--profile", str(paths["profile"]), "--format", "html", "--output", str(generated)]
        if watch:
            argv.extend(["--refresh-seconds", str((configuration["interval_ms"] + 999) // 1000)])
        checked_process(argv, configuration["subprocess_timeout_seconds"], "dashboard generation")
        # O_NOFOLLOW prevents a misbehaving generator from replacing the candidate
        # with a link to a journal, database, or credential file.
        descriptor = os.open(generated, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(descriptor, "rb") as source:
            if not stat.S_ISREG(os.fstat(source.fileno()).st_mode):
                raise PublicationError("generated report is not a regular file")
            data = source.read(configuration["max_report_bytes"] + 1)
        admit_report(data, configuration["max_report_bytes"])
        candidate = write_candidate(paths["local_report"].parent, data)
        deliver(configuration, candidate, data)
        os.replace(candidate, paths["local_report"])
    finally:
        generated.unlink(missing_ok=True)
        if candidate is not None:
            candidate.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", required=True, type=Path)
    parser.add_argument("--watch", action="store_true", help="repeat once per configured cadence, without retries inside a tick")
    args = parser.parse_args()
    config_path = args.config.absolute()
    try:
        configuration = admit_config(read_json(config_path, MAX_CONFIG_BYTES, "configuration"))
    except (OSError, ValueError) as error:
        message = str(error) if isinstance(error, PublicationError) else "cannot read configuration"
        print("dashboard publication failed: " + message, file=sys.stderr)
        return 1
    try:
        while True:
            started = time.monotonic()
            try:
                publish(configuration, config_path, args.watch)
                print("dashboard report delivered; no CI freshness or success claim", flush=True)
            except (OSError, ValueError) as error:
                message = str(error) if isinstance(error, PublicationError) else "file/process operation failed; prior report retained where not committed"
                print("dashboard publication failed: " + message, file=sys.stderr, flush=True)
                if not args.watch:
                    return 1
            if not args.watch:
                return 0
            # Missed ticks do not queue retries or accumulate catch-up work.
            time.sleep(max(0, configuration["interval_ms"] / 1000 - (time.monotonic() - started)))
    except KeyboardInterrupt:
        return 130


if __name__ == "__main__":
    sys.exit(main())
