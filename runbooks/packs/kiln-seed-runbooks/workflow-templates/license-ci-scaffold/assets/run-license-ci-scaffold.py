#!/usr/bin/env python3
"""Reviewed seed runbook script: license-ci-scaffold.

Scaffolds a reviewed LICENSE file and GitHub Actions CI workflow from pinned
bundle assets, runs local verification when available, writes a provenance
receipt, and stops before any remote push or PR creation.
"""

from __future__ import annotations

import json
import os
import pathlib
import shutil
import subprocess
import sys


VALID_LICENSES = {"apache-2.0", "mit", "mit-or-apache-2.0"}
VALID_CI_TEMPLATES = {"rust-cargo", "nix-flake", "rust-and-nix"}


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


def validate_scaffold_targets(work_root: pathlib.Path) -> None:
    license_dest = work_root / "LICENSE"
    if license_dest.is_symlink() or (license_dest.exists() and license_dest.is_dir()):
        raise ValueError(f"unsafe LICENSE target: {license_dest}")
    if work_root not in license_dest.resolve().parents:
        raise ValueError("LICENSE target escapes work root")

    dot_github = work_root / ".github"
    if dot_github.is_symlink() or (dot_github.exists() and not dot_github.is_dir()):
        raise ValueError(f"unsafe .github directory: {dot_github}")
    if work_root not in dot_github.resolve().parents:
        raise ValueError(".github directory escapes work root")

    workflows_dir = dot_github / "workflows"
    if workflows_dir.is_symlink() or (workflows_dir.exists() and not workflows_dir.is_dir()):
        raise ValueError(f"unsafe .github/workflows directory: {workflows_dir}")
    if work_root not in workflows_dir.resolve().parents:
        raise ValueError(".github/workflows directory escapes work root")

    ci_dest = workflows_dir / "ci.yml"
    if ci_dest.is_symlink() or (ci_dest.exists() and ci_dest.is_dir()):
        raise ValueError(f"unsafe ci.yml target: {ci_dest}")
    if work_root not in ci_dest.resolve().parents:
        raise ValueError("ci.yml target escapes work root")


def write_scaffold_file(
    work_root: pathlib.Path,
    rel_dirs: tuple[str, ...],
    filename: str,
    text: str,
    overwrite: bool,
) -> bool:
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    dir_flags = os.O_RDONLY | os.O_DIRECTORY | nofollow
    cur_fd = os.open(work_root, dir_flags)
    cur_path = work_root
    try:
        for part in rel_dirs:
            cur_path = cur_path / part
            if cur_path.is_symlink():
                raise ValueError(f"refusing symlink directory: {cur_path}")
            try:
                os.mkdir(part, 0o755, dir_fd=cur_fd)
            except FileExistsError:
                pass
            next_fd = os.open(part, dir_flags, dir_fd=cur_fd)
            os.close(cur_fd)
            cur_fd = next_fd
            if work_root not in cur_path.resolve().parents:
                raise ValueError(f"scaffold directory escapes work root: {cur_path}")

        target = cur_path / filename
        if target.is_symlink():
            raise ValueError(f"refusing symlink target: {target}")
        if work_root not in target.resolve().parents:
            raise ValueError(f"scaffold target escapes work root: {target}")

        if overwrite:
            fd = os.open(filename, os.O_WRONLY | os.O_CREAT | os.O_TRUNC | nofollow, 0o644, dir_fd=cur_fd)
        else:
            try:
                fd = os.open(filename, os.O_WRONLY | os.O_CREAT | os.O_EXCL | nofollow, 0o644, dir_fd=cur_fd)
            except FileExistsError:
                return False
        try:
            data = text.encode("utf-8")
            view = memoryview(data)
            written = 0
            while written < len(data):
                written += os.write(fd, view[written:])
        finally:
            os.close(fd)
        return True
    finally:
        os.close(cur_fd)


def main() -> int:
    license_spdx = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("LATTICE_PARAM_LICENSE_SPDX", "mit-or-apache-2.0")
    ci_template = sys.argv[2] if len(sys.argv) > 2 else os.environ.get("LATTICE_PARAM_CI_TEMPLATE", "rust-and-nix")
    overwrite_raw = sys.argv[3] if len(sys.argv) > 3 else os.environ.get("LATTICE_PARAM_OVERWRITE_EXISTING", "false")

    if license_spdx not in VALID_LICENSES:
        print(f"invalid license_spdx: {license_spdx}", file=sys.stderr)
        return 2
    if ci_template not in VALID_CI_TEMPLATES:
        print(f"invalid ci_template: {ci_template}", file=sys.stderr)
        return 2

    try:
        overwrite = parse_bool(overwrite_raw)
        assets_dir = pathlib.Path(__file__).resolve().parent
        work_root = pathlib.Path(os.environ.get("KILN_WORK_DIR", ".")).resolve()
        receipt_path = resolve_receipt_path(work_root, "license-ci-scaffold-receipt.json")
        validate_scaffold_targets(work_root)
    except (ValueError, OSError) as err:
        print(f"unsafe scaffold configuration: {err}", file=sys.stderr)
        return 2

    if license_spdx == "mit":
        license_text = (assets_dir / "LICENSE-MIT.txt").read_text(encoding="utf-8")
    elif license_spdx == "apache-2.0":
        license_text = (assets_dir / "LICENSE-APACHE-2.0.txt").read_text(encoding="utf-8")
    else:
        license_text = (
            (assets_dir / "LICENSE-APACHE-2.0.txt").read_text(encoding="utf-8").rstrip()
            + "\n\n---\n\n"
            + (assets_dir / "LICENSE-MIT.txt").read_text(encoding="utf-8")
        )

    ci_asset_map = {
        "rust-cargo": "ci-rust-cargo.yml",
        "nix-flake": "ci-nix-flake.yml",
        "rust-and-nix": "ci-rust-and-nix.yml",
    }
    ci_text = (assets_dir / ci_asset_map[ci_template]).read_text(encoding="utf-8")

    written_files: list[str] = []
    skipped_files: list[str] = []

    try:
        if write_scaffold_file(work_root, (), "LICENSE", license_text, overwrite):
            written_files.append("LICENSE")
        else:
            skipped_files.append("LICENSE")

        if write_scaffold_file(work_root, (".github", "workflows"), "ci.yml", ci_text, overwrite):
            written_files.append(".github/workflows/ci.yml")
        else:
            skipped_files.append(".github/workflows/ci.yml")
    except (ValueError, OSError) as err:
        print(f"unsafe scaffold target: {err}", file=sys.stderr)
        return 2

    checks: list[dict] = []
    cargo_bin = shutil.which("cargo")
    if (work_root / "Cargo.toml").is_file():
        if cargo_bin is None:
            checks.append({
                "check": "cargo check --locked",
                "status": "unavailable",
                "exit_code": None,
                "summary": "cargo executable not found on PATH",
            })
        else:
            try:
                proc = subprocess.run(
                    [cargo_bin, "check", "--locked"],
                    cwd=str(work_root),
                    capture_output=True,
                    text=True,
                    timeout=180,
                    check=False,
                )
                checks.append({
                    "check": "cargo check --locked",
                    "status": "pass" if proc.returncode == 0 else "fail",
                    "exit_code": proc.returncode,
                })
            except FileNotFoundError:
                checks.append({
                    "check": "cargo check --locked",
                    "status": "unavailable",
                    "exit_code": None,
                    "summary": "cargo executable not found on PATH",
                })
            except subprocess.TimeoutExpired:
                checks.append({
                    "check": "cargo check --locked",
                    "status": "fail",
                    "exit_code": None,
                    "summary": "cargo check timed out",
                })
    else:
        checks.append({
            "check": "cargo check --locked",
            "status": "skipped",
            "exit_code": None,
            "summary": "Cargo.toml absent in target directory",
        })

    has_fail = any(c["status"] == "fail" for c in checks)
    has_unavailable = any(c["status"] == "unavailable" for c in checks)
    has_pass = any(c["status"] == "pass" for c in checks)
    overall_status = "fail" if has_fail else ("incomplete" if (has_unavailable or not has_pass) else "pass")
    overall_ok = overall_status == "pass"
    receipt = {
        "schema": "kiln.runbook.license-ci-scaffold-receipt.v1",
        "runbook_id": "license-ci-scaffold",
        "mode": "mutating",
        "approval_gate": "kiln_publication",
        "grants_authority": False,
        "source_repository": os.environ.get("LATTICE_SOURCE_REPOSITORY", ""),
        "source_revision": os.environ.get("LATTICE_SOURCE_REVISION", ""),
        "license_spdx": license_spdx,
        "ci_template": ci_template,
        "overwrite_existing": overwrite,
        "written_files": written_files,
        "skipped_files": skipped_files,
        "checks": checks,
        "overall_status": overall_status,
        "non_claims": [
            "Worker step writes workspace files only; no branch push or pull request is created without approval and Kiln's publisher.",
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
