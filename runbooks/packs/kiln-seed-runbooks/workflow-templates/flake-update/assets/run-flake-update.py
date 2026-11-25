#!/usr/bin/env python3
"""Reviewed seed runbook script: flake-update.

Runs `nix flake update` (bounded by update_scope) and optional verification
(`nix flake check` / `cargo check`) inside the candidate checkout, emits a
bounded provenance receipt JSON, and stops prior to any publication or push.
"""

from __future__ import annotations

import json
import os
import pathlib
import shutil
import subprocess
import sys


VALID_SCOPES = {"all", "nixpkgs", "cairn", "toolchain"}


def safe_relative_path(raw: str) -> pathlib.Path:
    if not raw or len(raw) > 128 or "\x00" in raw:
        raise ValueError("invalid flake_dir")
    p = pathlib.PurePosixPath(raw)
    if p.is_absolute() or any(part in ("..", "") or part.startswith("-") for part in p.parts):
        raise ValueError("unsafe relative flake_dir")
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


def run_cmd(argv: list[str], cwd: pathlib.Path, timeout: int = 300) -> dict:
    try:
        proc = subprocess.run(
            argv,
            cwd=str(cwd),
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
        return {
            "command": argv,
            "status": "pass" if proc.returncode == 0 else "fail",
            "exit_code": proc.returncode,
            "stdout_tail": proc.stdout[-2048:],
            "stderr_tail": proc.stderr[-2048:],
        }
    except FileNotFoundError:
        return {
            "command": argv,
            "status": "unavailable",
            "exit_code": None,
            "stdout_tail": "",
            "stderr_tail": f"{argv[0]} not found in PATH",
        }
    except subprocess.TimeoutExpired:
        return {
            "command": argv,
            "status": "fail",
            "exit_code": None,
            "stdout_tail": "",
            "stderr_tail": "command timed out",
        }


def main() -> int:
    flake_dir_raw = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("LATTICE_PARAM_FLAKE_DIR", ".")
    scope = sys.argv[2] if len(sys.argv) > 2 else os.environ.get("LATTICE_PARAM_UPDATE_SCOPE", "all")
    run_check_raw = sys.argv[3] if len(sys.argv) > 3 else os.environ.get("LATTICE_PARAM_RUN_CHECK", "true")

    if scope not in VALID_SCOPES:
        print(f"invalid update_scope: {scope}", file=sys.stderr)
        return 2

    try:
        rel_dir = safe_relative_path(flake_dir_raw)
        run_check = parse_bool(run_check_raw)
        work_root = pathlib.Path(os.environ.get("KILN_WORK_DIR", ".")).resolve()
        receipt_path = resolve_receipt_path(work_root, "flake-update-receipt.json")
    except (ValueError, OSError) as err:
        print(f"unsafe flake-update configuration: {err}", file=sys.stderr)
        return 2

    target_dir = (work_root / rel_dir).resolve()
    if work_root != target_dir and work_root not in target_dir.parents:
        print("flake_dir escapes work root", file=sys.stderr)
        return 2

    flake_nix = target_dir / "flake.nix"
    if not flake_nix.is_file():
        print(f"flake.nix missing at {target_dir}", file=sys.stderr)
        return 1

    nix_bin = shutil.which("nix")
    steps: list[dict] = []
    if nix_bin is None:
        steps.append({
            "command": ["nix", "flake", "update"],
            "status": "unavailable",
            "exit_code": None,
            "stdout_tail": "",
            "stderr_tail": "nix executable unavailable",
        })
    else:
        cmd = [nix_bin, "flake", "update", "--option", "builders", ""]
        if scope != "all":
            cmd.append(scope)
        update_res = run_cmd(cmd, target_dir)
        steps.append(update_res)

        if update_res["status"] == "pass" and run_check:
            check_res = run_cmd(
                [nix_bin, "flake", "check", "--no-build", "--option", "builders", ""],
                target_dir,
            )
            steps.append(check_res)

    has_fail = any(s["status"] == "fail" for s in steps)
    has_unavailable = any(s["status"] == "unavailable" for s in steps)
    has_pass = any(s["status"] == "pass" for s in steps)
    overall_status = "fail" if has_fail else ("incomplete" if (has_unavailable or not has_pass) else "pass")
    overall_ok = overall_status == "pass"

    receipt = {
        "schema": "kiln.runbook.flake-update-receipt.v1",
        "runbook_id": "flake-update",
        "mode": "mutating",
        "approval_gate": "kiln_publication",
        "grants_authority": False,
        "source_repository": os.environ.get("LATTICE_SOURCE_REPOSITORY", ""),
        "source_revision": os.environ.get("LATTICE_SOURCE_REVISION", ""),
        "flake_dir": str(rel_dir),
        "update_scope": scope,
        "run_check": run_check,
        "overall_status": overall_status,
        "steps": steps,
        "non_claims": [
            "Worker execution prepares candidate updates only and holds no push or PR credentials.",
            "Publication requires explicit operator approval and Kiln's separately credentialed publisher.",
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
