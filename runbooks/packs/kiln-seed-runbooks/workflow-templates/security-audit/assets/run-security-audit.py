#!/usr/bin/env python3
"""Reviewed seed runbook script: security-audit.

Executes read-only security and policy checks (cairn validate, cargo-audit,
octet) against a target checkout and writes a structured Provenance summary
receipt (`kiln.runbook.security-audit-receipt.v1`). Unavailable tools or
offline advisory databases are reported explicitly as `unavailable`, never
fabricated as `pass`.
"""

from __future__ import annotations

import json
import os
import pathlib
import shutil
import subprocess
import sys


VALID_SCOPES = {"all", "cairn", "cargo-audit", "octet"}


def safe_relative_path(raw: str) -> pathlib.Path:
    if not raw or len(raw) > 128 or "\x00" in raw:
        raise ValueError("invalid crate_root")
    p = pathlib.PurePosixPath(raw)
    if p.is_absolute() or any(part in ("..", "") or part.startswith("-") for part in p.parts):
        raise ValueError("unsafe relative crate_root")
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


def run_check(tool: str, argv: list[str], cwd: pathlib.Path) -> dict:
    try:
        proc = subprocess.run(
            argv,
            cwd=str(cwd),
            capture_output=True,
            text=True,
            timeout=180,
            check=False,
        )
        return {
            "tool": tool,
            "status": "pass" if proc.returncode == 0 else "fail",
            "exit_code": proc.returncode,
            "summary": (proc.stdout or proc.stderr or "").strip()[-1024:],
        }
    except FileNotFoundError:
        return {
            "tool": tool,
            "status": "unavailable",
            "exit_code": None,
            "summary": f"{argv[0]} not found on PATH",
        }
    except subprocess.TimeoutExpired:
        return {
            "tool": tool,
            "status": "fail",
            "exit_code": None,
            "summary": f"{tool} timed out",
        }


def check_cairn(target_dir: pathlib.Path) -> dict:
    cairn_bin = shutil.which("cairn")
    if cairn_bin is None:
        return {
            "tool": "cairn",
            "status": "unavailable",
            "exit_code": None,
            "summary": "cairn executable not found on PATH",
        }
    cmd = [cairn_bin, "validate", "--root", str(target_dir)]
    policy = target_dir / "cairn-policy" / "generated" / "cairn-policy.json"
    if policy.is_file():
        cmd.extend(["--policy", str(policy)])
    return run_check("cairn", cmd, target_dir)


def check_cargo_audit(target_dir: pathlib.Path) -> dict:
    lockfile = target_dir / "Cargo.lock"
    if not lockfile.is_file():
        return {
            "tool": "cargo-audit",
            "status": "skipped",
            "exit_code": None,
            "summary": "Cargo.lock absent in target directory",
        }
    cargo_audit = shutil.which("cargo-audit")
    cargo_bin = shutil.which("cargo")
    if cargo_audit is not None:
        cmd = [cargo_audit, "audit", "--json", "--file", str(lockfile)]
    elif cargo_bin is not None:
        probe = subprocess.run(
            [cargo_bin, "audit", "--version"],
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )
        if probe.returncode != 0:
            return {
                "tool": "cargo-audit",
                "status": "unavailable",
                "exit_code": None,
                "summary": "cargo-audit subcommand unavailable",
            }
        cmd = [cargo_bin, "audit", "--json", "--file", str(lockfile)]
    else:
        return {
            "tool": "cargo-audit",
            "status": "unavailable",
            "exit_code": None,
            "summary": "cargo-audit executable unavailable",
        }

    advisory_db = os.environ.get("CARGO_AUDIT_DB", "")
    if advisory_db:
        cmd.extend(["--db", advisory_db, "--no-fetch"])
    elif not (pathlib.Path.home() / ".cargo" / "advisory-db").exists():
        return {
            "tool": "cargo-audit",
            "status": "unavailable",
            "exit_code": None,
            "summary": "offline advisory-db not provisioned (refusing unverified network fetch)",
        }
    else:
        cmd.append("--no-fetch")

    res = run_check("cargo-audit", cmd, target_dir)
    if res["status"] == "pass" and res["summary"]:
        try:
            parsed = json.loads(res["summary"])
            vulns = len(parsed.get("vulnerabilities", {}).get("list", []))
            res["findings_count"] = vulns
        except json.JSONDecodeError:
            pass
    return res


def check_octet(target_dir: pathlib.Path) -> dict:
    octet_bin = shutil.which("octet-standards")
    if octet_bin is None:
        return {
            "tool": "octet",
            "status": "unavailable",
            "exit_code": None,
            "summary": "octet-standards executable not found on PATH",
        }
    artifacts_dir = os.environ.get("OCTET_POLICY_ARTIFACTS_DIR", "")
    registry_path = os.environ.get("OCTET_POLICY_REGISTRY", "")
    if artifacts_dir and pathlib.Path(artifacts_dir).is_dir():
        cmd = [
            octet_bin,
            "check",
            "--policy-artifacts-dir",
            artifacts_dir,
            "--source",
            str(target_dir),
            "--output",
            "json",
        ]
    elif registry_path and pathlib.Path(registry_path).is_file():
        cmd = [
            octet_bin,
            "check",
            "--registry",
            registry_path,
            "--source",
            str(target_dir),
            "--output",
            "json",
        ]
    else:
        return {
            "tool": "octet",
            "status": "unavailable",
            "exit_code": None,
            "summary": "octet-standards policy artifacts/registry not configured (set OCTET_POLICY_ARTIFACTS_DIR or OCTET_POLICY_REGISTRY)",
        }
    return run_check("octet", cmd, target_dir)


def main() -> int:
    crate_root_raw = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("LATTICE_PARAM_CRATE_ROOT", ".")
    scope = sys.argv[2] if len(sys.argv) > 2 else os.environ.get("LATTICE_PARAM_AUDIT_SCOPE", "all")
    strict_raw = sys.argv[3] if len(sys.argv) > 3 else os.environ.get("LATTICE_PARAM_STRICT", "true")

    if scope not in VALID_SCOPES:
        print(f"invalid audit_scope: {scope}", file=sys.stderr)
        return 2

    try:
        rel_root = safe_relative_path(crate_root_raw)
        strict = parse_bool(strict_raw)
        work_root = pathlib.Path(os.environ.get("KILN_WORK_DIR", ".")).resolve()
        receipt_path = resolve_receipt_path(work_root, "security-audit-receipt.json")
    except (ValueError, OSError) as err:
        print(f"unsafe security-audit configuration: {err}", file=sys.stderr)
        return 2

    target_dir = (work_root / rel_root).resolve()
    if work_root != target_dir and work_root not in target_dir.parents:
        print("crate_root escapes work root", file=sys.stderr)
        return 2

    checks: list[dict] = []
    if scope in ("all", "cairn"):
        checks.append(check_cairn(target_dir))
    if scope in ("all", "cargo-audit"):
        checks.append(check_cargo_audit(target_dir))
    if scope in ("all", "octet"):
        checks.append(check_octet(target_dir))

    has_fail = any(c["status"] == "fail" for c in checks)
    has_unavailable = any(c["status"] == "unavailable" for c in checks)
    overall_status = "fail" if has_fail else ("incomplete" if has_unavailable else "pass")

    receipt = {
        "schema": "kiln.runbook.security-audit-receipt.v1",
        "runbook_id": "security-audit",
        "mode": "read_only",
        "grants_authority": False,
        "source_repository": os.environ.get("LATTICE_SOURCE_REPOSITORY", ""),
        "source_revision": os.environ.get("LATTICE_SOURCE_REVISION", ""),
        "crate_root": str(rel_root),
        "audit_scope": scope,
        "strict": strict,
        "overall_status": overall_status,
        "checks": checks,
        "non_claims": [
            "Read-only audit summary receipt records observed check outputs only; it grants no merge, approval, or release authority.",
            "Unavailable or skipped checks are never treated as passing.",
        ],
    }
    try:
        write_text_nofollow(receipt_path, json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    except (ValueError, OSError) as err:
        print(f"failed writing receipt safely: {err}", file=sys.stderr)
        return 2
    print(json.dumps(receipt, sort_keys=True))

    if has_fail or (strict and has_unavailable):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
