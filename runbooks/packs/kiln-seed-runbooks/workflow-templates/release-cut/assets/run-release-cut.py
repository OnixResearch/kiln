#!/usr/bin/env python3
"""Reviewed seed runbook script: release-cut.

Prepares a release candidate summary (reads Cargo.toml version, computes
candidate version/tag, verifies git cleanliness unless allow_dirty=true,
runs cargo check/test when Cargo.toml is present), writes a release receipt
JSON, and halts at the mandatory approval gate before any tag/push/release.
"""

from __future__ import annotations

import json
import os
import pathlib
import re
import shutil
import subprocess
import sys


VALID_CHANNELS = {"alpha", "beta", "rc", "stable"}
VALID_BUMPS = {"patch", "minor", "major"}
VERSION_RE = re.compile(r'^version\s*=\s*"(\d+)\.(\d+)\.(\d+)(?:-[^"]+)?"', re.MULTILINE)


def safe_relative_path(raw: str) -> pathlib.Path:
    if not raw or len(raw) > 128 or "\x00" in raw:
        raise ValueError("invalid crate_manifest")
    p = pathlib.PurePosixPath(raw)
    if p.is_absolute() or any(part in ("..", "") or part.startswith("-") for part in p.parts):
        raise ValueError("unsafe relative crate_manifest")
    return pathlib.Path(".") if raw == "." else pathlib.Path(*p.parts)


def parse_bool(raw: str) -> bool:
    if raw == "true":
        return True
    if raw == "false":
        return False
    raise ValueError(f"invalid boolean parameter: {raw}")


def resolve_receipt_path(work_root: pathlib.Path, default_name: str) -> pathlib.Path:
    raw = os.environ.get("KILN_RECEIPT_PATH") or default_name
    if "\x00" in raw:
        raise ValueError("invalid KILN_RECEIPT_PATH")
    candidate = pathlib.Path(raw)
    if candidate.is_absolute():
        if candidate.is_symlink() or candidate.parent.is_symlink():
            raise ValueError(f"refusing symlinked receipt path: {candidate}")
        if candidate.exists() and candidate.is_dir():
            raise ValueError(f"receipt path is a directory: {candidate}")
        return candidate
    if ".." in candidate.parts or not candidate.parts or candidate == pathlib.Path("."):
        raise ValueError(f"unsafe relative receipt path: {raw}")
    cur = work_root
    for idx, part in enumerate(candidate.parts):
        cur = cur / part
        if cur.is_symlink():
            raise ValueError(f"refusing symlink in receipt path: {cur}")
        if idx < len(candidate.parts) - 1 and cur.exists() and not cur.is_dir():
            raise ValueError(f"non-directory component in receipt path: {cur}")
    resolved = cur.resolve()
    work_resolved = work_root.resolve()
    if resolved == work_resolved or work_resolved not in resolved.parents:
        raise ValueError(f"receipt path escapes work root: {raw}")
    if cur.exists() and cur.is_dir():
        raise ValueError(f"receipt path is a directory: {cur}")
    return cur


def write_text_nofollow(path: pathlib.Path, content: str) -> None:
    if path.is_symlink() or path.parent.is_symlink():
        raise ValueError(f"refusing symlinked path: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.parent.is_symlink():
        raise ValueError(f"refusing symlinked parent directory: {path.parent}")
    flags = os.O_WRONLY | os.O_CREAT | os.O_TRUNC | getattr(os, "O_NOFOLLOW", 0)
    fd = os.open(path, flags, 0o644)
    try:
        data = content.encode("utf-8")
        view = memoryview(data)
        written = 0
        while written < len(data):
            written += os.write(fd, view[written:])
    finally:
        os.close(fd)


def bump_version(major: int, minor: int, patch: int, bump: str, channel: str) -> str:
    if bump == "major":
        major, minor, patch = major + 1, 0, 0
    elif bump == "minor":
        minor, patch = minor + 1, 0
    else:
        patch += 1
    base = f"{major}.{minor}.{patch}"
    return base if channel == "stable" else f"{base}-{channel}.1"


def main() -> int:
    manifest_raw = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("LATTICE_PARAM_CRATE_MANIFEST", "Cargo.toml")
    channel = sys.argv[2] if len(sys.argv) > 2 else os.environ.get("LATTICE_PARAM_RELEASE_CHANNEL", "rc")
    bump = sys.argv[3] if len(sys.argv) > 3 else os.environ.get("LATTICE_PARAM_BUMP_LEVEL", "patch")
    allow_dirty_raw = sys.argv[4] if len(sys.argv) > 4 else os.environ.get("LATTICE_PARAM_ALLOW_DIRTY", "false")

    if channel not in VALID_CHANNELS:
        print(f"invalid release_channel: {channel}", file=sys.stderr)
        return 2
    if bump not in VALID_BUMPS:
        print(f"invalid bump_level: {bump}", file=sys.stderr)
        return 2

    try:
        rel_manifest = safe_relative_path(manifest_raw)
        allow_dirty = parse_bool(allow_dirty_raw)
        work_root = pathlib.Path(os.environ.get("KILN_WORK_DIR", ".")).resolve()
        receipt_path = resolve_receipt_path(work_root, "release-cut-receipt.json")
    except (ValueError, OSError) as err:
        print(f"unsafe release-cut configuration: {err}", file=sys.stderr)
        return 2

    manifest_path = (work_root / rel_manifest).resolve()
    if work_root not in manifest_path.parents and manifest_path != work_root:
        print("crate_manifest escapes work root", file=sys.stderr)
        return 2
    if not manifest_path.is_file():
        print(f"crate manifest missing: {manifest_path}", file=sys.stderr)
        return 1

    text = manifest_path.read_text(encoding="utf-8")
    match = VERSION_RE.search(text)
    if not match:
        print("could not locate semver version in crate manifest", file=sys.stderr)
        return 1
    current_version = match.group(0).split('"')[1]
    candidate_version = bump_version(
        int(match.group(1)),
        int(match.group(2)),
        int(match.group(3)),
        bump,
        channel,
    )

    checks: list[dict] = []
    git_bin = shutil.which("git")
    dirty: bool | None = False
    git_available_and_ok = True
    if (work_root / ".git").exists():
        if git_bin is None:
            dirty = None
            git_available_and_ok = False
            checks.append({
                "check": "git status --porcelain",
                "status": "unavailable",
                "exit_code": None,
                "summary": "git executable not found on PATH",
            })
        else:
            try:
                proc = subprocess.run(
                    [git_bin, "status", "--porcelain"],
                    cwd=str(work_root),
                    capture_output=True,
                    text=True,
                    timeout=30,
                    check=False,
                )
                if proc.returncode != 0:
                    dirty = None
                    git_available_and_ok = False
                    checks.append({
                        "check": "git status --porcelain",
                        "status": "fail",
                        "exit_code": proc.returncode,
                    })
                else:
                    dirty = bool(proc.stdout.strip())
                    checks.append({
                        "check": "git status --porcelain",
                        "status": "pass" if (not dirty or allow_dirty) else "fail",
                        "exit_code": proc.returncode,
                    })
            except FileNotFoundError:
                dirty = None
                git_available_and_ok = False
                checks.append({
                    "check": "git status --porcelain",
                    "status": "unavailable",
                    "exit_code": None,
                    "summary": "git executable not found on PATH",
                })
            except subprocess.TimeoutExpired:
                dirty = None
                git_available_and_ok = False
                checks.append({
                    "check": "git status --porcelain",
                    "status": "fail",
                    "exit_code": None,
                    "summary": "git status timed out",
                })

    clean_ok = git_available_and_ok and (dirty is False or allow_dirty)
    cargo_bin = shutil.which("cargo")
    if clean_ok:
        if cargo_bin is None:
            checks.append({
                "check": "cargo test --locked",
                "status": "unavailable",
                "exit_code": None,
                "summary": "cargo executable not found on PATH",
            })
        else:
            try:
                proc = subprocess.run(
                    [cargo_bin, "test", "--locked", "--manifest-path", str(manifest_path)],
                    cwd=str(work_root),
                    capture_output=True,
                    text=True,
                    timeout=300,
                    check=False,
                )
                checks.append({
                    "check": "cargo test --locked",
                    "status": "pass" if proc.returncode == 0 else "fail",
                    "exit_code": proc.returncode,
                })
            except FileNotFoundError:
                checks.append({
                    "check": "cargo test --locked",
                    "status": "unavailable",
                    "exit_code": None,
                    "summary": "cargo executable not found on PATH",
                })
            except subprocess.TimeoutExpired:
                checks.append({
                    "check": "cargo test --locked",
                    "status": "fail",
                    "exit_code": None,
                    "summary": "cargo test timed out",
                })

    has_fail = any(c["status"] == "fail" for c in checks) or (
        not clean_ok and not any(c["status"] == "unavailable" for c in checks)
    )
    has_unavailable = any(c["status"] == "unavailable" for c in checks)
    has_pass = any(c["status"] == "pass" for c in checks)
    overall_status = "fail" if has_fail else ("incomplete" if (has_unavailable or not has_pass) else "pass")
    overall_ok = overall_status == "pass"
    receipt = {
        "schema": "kiln.runbook.release-cut-receipt.v1",
        "runbook_id": "release-cut",
        "mode": "mutating",
        "approval_gate": "kiln_publication",
        "grants_authority": False,
        "source_repository": os.environ.get("LATTICE_SOURCE_REPOSITORY", ""),
        "source_revision": os.environ.get("LATTICE_SOURCE_REVISION", ""),
        "crate_manifest": str(rel_manifest),
        "current_version": current_version,
        "candidate_version": candidate_version,
        "candidate_tag": f"v{candidate_version}",
        "release_channel": channel,
        "bump_level": bump,
        "working_tree_dirty": dirty,
        "allow_dirty": allow_dirty,
        "checks": checks,
        "overall_status": overall_status,
        "non_claims": [
            "Worker step prepares a release candidate receipt only and creates no Git tag, push, or GitHub release.",
            "Tag creation and publication require explicit approval and Kiln's separately credentialed publisher.",
        ],
    }
    try:
        write_text_nofollow(receipt_path, json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    except (ValueError, OSError) as err:
        print(f"failed writing receipt safely: {err}", file=sys.stderr)
        return 2
    print(json.dumps(receipt, sort_keys=True))
    return 0 if overall_ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
