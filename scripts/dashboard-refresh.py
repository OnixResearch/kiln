#!/usr/bin/env python3
"""Observe bounded canonical CI journals and publish one finite dashboard refresh."""
# r[impl kiln.dashboard.live.collection]
# r[impl kiln.dashboard.live.collection.scenario.admission]
# r[impl kiln.dashboard.live.supervision.scenario.cadence]
# r[impl kiln.dashboard.live.recovery.scenario.restart]

import argparse
from contextlib import contextmanager
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import sys
import tempfile

SPEC = importlib.util.spec_from_file_location(
    "dashboard_publish", Path(__file__).with_name("dashboard-publish.py"))
publisher = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publisher)
RefreshError = publisher.PublicationError

MAX_CANDIDATES = 1024
MAX_HISTORIES = 128
MAX_JOURNAL_BYTES = 16777216
MAX_TOTAL_JOURNAL_BYTES = 268435456
MAX_LABEL_CHARACTERS = 1024
MAX_IDENTIFIER_CHARACTERS = 128
JOURNAL_SCHEMA = "kiln.radicle-ci.journal.v1"
CANDIDATE = re.compile(r"[0-9a-f]{64}")
JOURNAL_FIELDS = {"schema", "profile_identity", "source", "source_intent", "trigger",
                  "ingress", "records", "native_observations", "worker_result",
                  "status_intent", "status_observation", "identity"}


def label(value):
    return (isinstance(value, str) and 0 < len(value.encode("utf-8")) <= MAX_LABEL_CHARACTERS
            and bool(value.strip())
            and not any(ord(char) < 32 or 127 <= ord(char) <= 159 for char in value))


def admit_config(value):
    """Pure, closed configuration admission; limits include total scan work."""
    publisher.closed(value, {"schema_version", "candidate_root", "catalog_output",
                             "publication_config", "project_id", "project_label",
                             "max_candidates", "max_histories", "max_journal_bytes"},
                     "refresh configuration")
    if type(value["schema_version"]) is not int or value["schema_version"] != 1:
        raise RefreshError("unsupported refresh configuration version")
    for key in ("candidate_root", "catalog_output", "publication_config"):
        path = value[key]
        if (not publisher.path_text(path) or not path.startswith("/")
                or any(part in {".", "..", ""} for part in path.split("/")[1:])):
            raise RefreshError(f"invalid absolute {key}")
    if (not isinstance(value["project_id"], str)
            or len(value["project_id"]) > MAX_IDENTIFIER_CHARACTERS
            or re.fullmatch(r"[A-Za-z0-9._-]+", value["project_id"]) is None):
        raise RefreshError("invalid project_id")
    if not label(value["project_label"]):
        raise RefreshError("invalid project_label")
    for key, maximum in (("max_candidates", MAX_CANDIDATES),
                         ("max_histories", MAX_HISTORIES),
                         ("max_journal_bytes", MAX_JOURNAL_BYTES)):
        publisher.integer(value[key], maximum, key)
    if value["max_histories"] > value["max_candidates"]:
        raise RefreshError("history bound exceeds candidate bound")
    if value["max_candidates"] * value["max_journal_bytes"] > MAX_TOTAL_JOURNAL_BYTES:
        raise RefreshError("candidate byte budget exceeds total bound")
    return value


def admit_journal(value):
    """Admit metadata only; the renderer independently admits canonical records.

    These labels do not authenticate a source or verify the CiJournal seal.
    Empty records are not a completed run and are not a usable observation.
    """
    publisher.closed(value, JOURNAL_FIELDS, "CI journal")
    if value["schema"] != JOURNAL_SCHEMA:
        raise RefreshError("unsupported CI journal schema")
    publisher.closed(value["source"], {"provider", "repository", "revision"}, "source labels")
    source = value["source"]
    if (not all(label(source[key]) for key in source)
            or not label(source["provider"] + ":" + source["repository"])):
        raise RefreshError("invalid source labels")
    if not isinstance(value["records"], list) or not value["records"]:
        raise RefreshError("CI journal has no canonical observations")
    if not all(isinstance(record, dict) for record in value["records"]):
        raise RefreshError("invalid canonical record shape")
    return source


def plan_catalog(configuration, observations, max_rendered_bytes):
    """Pure deterministic observed-update order, not newest-source truth.

    An observation is (candidate identity, original absolute journal path,
    observed mtime_ns, admitted declared source labels). Never infer an outcome.
    """
    if not observations:
        raise RefreshError("no canonical journal observations")
    if len(observations) > configuration["max_candidates"]:
        raise RefreshError("candidate bound exceeded")
    identities = [item[0] for item in observations]
    if len(set(identities)) != len(identities):
        raise RefreshError("duplicate candidate observation")
    ordered = sorted(observations, key=lambda item: (-item[2], item[0]))
    histories = [{"id": identity, "label": "Observed candidate " + identity,
                  "source": source["provider"] + ":" + source["repository"],
                  "revision": source["revision"],
                  "input": {"kind": "journal", "path": str(path)}}
                 for identity, path, _, source in ordered[:configuration["max_histories"]]]
    return {"schema_version": 1,
            "bounds": {"max_catalog_bytes": publisher.MAX_CATALOG_BYTES,
                       "max_projects": 1, "max_histories": configuration["max_histories"],
                       "max_input_bytes": (configuration["max_histories"]
                                           * configuration["max_journal_bytes"]),
                       "max_rendered_bytes": max_rendered_bytes},
            "projects": [{"id": configuration["project_id"],
                          "label": configuration["project_label"], "histories": histories}]}


def no_link_directories(path):
    """Reject symlink directory components, including the declared root itself."""
    for component in reversed((path, *path.parents)):
        info = component.lstat()
        if not stat.S_ISDIR(info.st_mode):
            raise RefreshError("path requires real directory components")


def output_path(path):
    no_link_directories(path.parent)
    parent = path.parent.stat()
    if parent.st_uid != os.geteuid() or parent.st_mode & 0o022:
        raise RefreshError("output requires an owned non-shared parent")
    try:
        info = path.lstat()
    except FileNotFoundError:
        return
    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_uid != os.geteuid():
        raise RefreshError("output requires an owned unlinked regular file")


def read_json(path, maximum, description):
    no_link_directories(path.parent)
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, "rb") as source:
        before = os.fstat(source.fileno())
        if not stat.S_ISREG(before.st_mode):
            raise RefreshError(f"{description} requires a regular file")
        if before.st_size > maximum:
            raise RefreshError(f"{description} exceeds byte bound")
        data = source.read(maximum + 1)
        after = os.fstat(source.fileno())
    if len(data) > maximum:
        raise RefreshError(f"{description} exceeds byte bound")
    if fingerprint(before) != fingerprint(after):
        raise RefreshError(f"{description} changed during observation")
    try:
        value = json.loads(data, object_pairs_hook=publisher.unique_object,
                           parse_constant=lambda _: invalid_constant())
    except (UnicodeError, json.JSONDecodeError, RecursionError) as error:
        raise RefreshError(f"invalid {description} JSON") from error
    return value, fingerprint(after)


def invalid_constant():
    raise RefreshError("non-finite JSON number")


def fingerprint(info):
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def collides(left, right):
    return (left.resolve() == right.resolve()
            or (left.exists() and right.exists() and os.path.samefile(left, right)))


def prepare_paths(configuration, config_path, publication):
    """Reject output aliases and any writes beneath canonical candidate state."""
    root = Path(configuration["candidate_root"])
    no_link_directories(root)
    publication_path = Path(configuration["publication_config"])
    paths = {key: publisher.absolute(publication_path.parent, publication[key])
             for key in ("dashboard_executable", "catalog", "profile", "local_report")}
    catalog = Path(configuration["catalog_output"])
    if paths["catalog"] != catalog:
        raise RefreshError("publication catalog differs from catalog_output")
    lock = Path(str(catalog) + ".lock")
    targets = [catalog, lock, paths["local_report"], Path(publication["destination"]["path"])]
    protected = [config_path, publication_path, paths["profile"], paths["dashboard_executable"],
                 Path(__file__).resolve(), Path(publisher.__file__).resolve()]
    for index, target in enumerate(targets):
        if target.resolve().is_relative_to(root.resolve()):
            raise RefreshError("output collides with canonical candidate state")
        if any(collides(target, source) for source in protected + targets[:index]):
            raise RefreshError("output collides with an input or output")
    local_targets = targets if publication["destination"]["kind"] == "local" else targets[:-1]
    for target in local_targets:
        output_path(target)
    # Bound profile input before the renderer reads it. Publisher remains the
    # authority for profile semantics, report admission, generation and transfer.
    read_json(paths["profile"], publisher.MAX_CONFIG_BYTES, "dashboard profile")
    if not paths["dashboard_executable"].is_file() or not os.access(paths["dashboard_executable"], os.X_OK):
        raise RefreshError("dashboard executable is not executable")
    return root, catalog, lock


@contextmanager
def cycle_lock(path):
    """Persistent inode, nonblocking flock, no queue and no unlink race."""
    output_path(path)
    descriptor = os.open(path, os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW | os.O_NONBLOCK, 0o600)
    try:
        info = os.fstat(descriptor)
        if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.geteuid()
                or info.st_nlink != 1 or info.st_mode & 0o077):
            raise RefreshError("invalid owned refresh lock")
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise RefreshError("another refresh cycle owns the lock") from error
        yield
    finally:
        os.close(descriptor)


def collect(configuration, root):
    """Bound *all* immediate entries, even names that are not candidates."""
    observations = []
    snapshots = []
    with os.scandir(root) as entries:
        for count, entry in enumerate(entries, 1):
            if count > configuration["max_candidates"]:
                raise RefreshError("candidate directory entry bound exceeded")
            if entry.is_symlink():
                raise RefreshError("symlink candidate entry rejected")
            if CANDIDATE.fullmatch(entry.name) is None:
                continue
            if not entry.is_dir(follow_symlinks=False):
                raise RefreshError("candidate identity is not a directory")
            path = root / entry.name / "ci-journal.json"
            try:
                value, snapshot = read_json(path, configuration["max_journal_bytes"], "CI journal")
            except FileNotFoundError:
                # A reservation without its first journal is not a run.
                if not path.parent.is_dir():
                    raise RefreshError("candidate disappeared during observation")
                continue
            source = admit_journal(value)
            observations.append((entry.name, path, snapshot[3], source))
            snapshots.append((path, snapshot))
    return observations, snapshots


def atomic_catalog(path, data):
    descriptor, name = tempfile.mkstemp(prefix=".kiln-catalog-", suffix=".json", dir=path.parent)
    temporary = Path(name)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
        output_path(path)
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def refresh(configuration, config_path):
    """One serialized observation/publication cycle; never dispatches or loops."""
    configuration = admit_config(configuration)
    publication_path = Path(configuration["publication_config"])
    publication, _ = read_json(publication_path, publisher.MAX_CONFIG_BYTES, "publication configuration")
    publication = publisher.admit_config(publication)
    root, catalog_path, lock_path = prepare_paths(configuration, config_path, publication)
    with cycle_lock(lock_path):
        observations, snapshots = collect(configuration, root)
        catalog = plan_catalog(configuration, observations, publication["max_report_bytes"])
        data = (json.dumps(catalog, ensure_ascii=True, separators=(",", ":")) + "\n").encode("utf-8")
        if len(data) > publisher.MAX_CATALOG_BYTES:
            raise RefreshError("generated catalog exceeds byte bound")
        # Recheck every observation, not merely the retained history window.
        for path, snapshot in snapshots:
            if fingerprint(path.lstat()) != snapshot or path.is_symlink():
                raise RefreshError("CI journal changed before publication")
        atomic_catalog(catalog_path, data)
        # publish() is finite. watch only emits finite HTML refresh metadata;
        # calling the publisher CLI with --watch would instead loop forever.
        publisher.publish(publication, publication_path, watch=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", required=True, type=Path)
    args = parser.parse_args()
    config_path = args.config.absolute()
    try:
        configuration, _ = read_json(config_path, publisher.MAX_CONFIG_BYTES, "refresh configuration")
        refresh(configuration, config_path)
    except (OSError, ValueError) as error:
        message = str(error) if isinstance(error, RefreshError) else "file/process operation failed"
        print("dashboard refresh failed: " + message, file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        return 130
    print("dashboard report delivered; observed update order is not newest-source completeness")
    return 0


if __name__ == "__main__":
    sys.exit(main())
