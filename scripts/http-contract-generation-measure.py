#!/usr/bin/env python3
"""Measure the pinned HTTP client generator without changing Kiln's workspace."""

# r[impl kiln.web.http_contract.generation_evaluation]
# r[verify kiln.web.http_contract.generation_evaluation.scenario.measured]
import argparse
from datetime import date
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import tempfile
import time
import tomllib

ROOT = Path(__file__).resolve().parent.parent
CONTRACT = ROOT / "contracts/http-api/openapi.json"
ROOT_LOCK = ROOT / "Cargo.lock"
FIXTURE = ROOT / "crates/kiln-web-service/tests/http_contract.rs"
DEFAULT_SCRATCH = Path.home() / ".cache/kiln/http-contract-generation"
DEFAULT_CARGO = "cargo"
REVISION = "b317a86578098ecabbeb04a0f57dc693032b81cc"
MAX_JOBS = 4
OPERATION_IDS = {
    "/api/projects": "list_projects",
    "/api/runs": "list_runs",
    "/api/runs/{id}": "get_run",
    "/api/runs/{id}/artifacts": "list_artifacts",
    "/api/runs/{id}/execution-binding": "get_execution_binding",
}
MANIFEST = f'''[package]
name = "kiln-progenitor-measurement"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
futures = "0.3"
progenitor-client = {{ git = "https://github.com/oxidecomputer/progenitor", rev = "{REVISION}" }}
reqwest = {{ version = "0.13.4", features = ["json", "query", "stream"] }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"

[build-dependencies]
prettyplease = "0.3.0"
progenitor = {{ git = "https://github.com/oxidecomputer/progenitor", rev = "{REVISION}", default-features = false }}
serde_json = "1.0"
syn = "3.0.2"
'''
BUILD_SCRIPT = '''use std::{env, fs::{self, File}, path::Path};
fn main() {
    let src = "openapi.json";
    println!("cargo:rerun-if-changed={src}");
    let spec = serde_json::from_reader(File::open(src).unwrap()).unwrap();
    let mut generator = progenitor::Generator::default();
    let tokens = generator.generate_tokens(&spec).unwrap();
    let ast = syn::parse2(tokens).unwrap();
    let content = prettyplease::unparse(&ast);
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join("codegen.rs"), content).unwrap();
}
'''


def command(args, checks, expected_success=True):
    started = time.perf_counter()
    result = subprocess.run(args, cwd=ROOT, text=True, capture_output=True, check=False)
    elapsed = round(time.perf_counter() - started, 2)
    checks.append({
        "command": shlex.join(map(str, args)),
        "result": "passed" if result.returncode == 0 else f"failed (exit {result.returncode})",
        "passed": int(result.returncode == 0),
        "failed": int(result.returncode != 0),
        "wall_seconds": elapsed,
    })
    if expected_success and result.returncode:
        raise RuntimeError(f"{shlex.join(map(str, args))}:\n{result.stdout}\n{result.stderr}")
    return result, elapsed


def cargo(program, *args):
    return [program, "--config", "net.git-fetch-with-cli=false", *map(str, args)]


def build(program, manifest, target, jobs, checks):
    return command(cargo(program, "build", "--offline", "--locked", "--manifest-path", manifest,
                         "--target-dir", target, "--jobs", jobs), checks, expected_success=False)


def fixture_size():
    lines = FIXTURE.read_text().splitlines()
    start = next(i for i, line in enumerate(lines) if line.startswith("fn resolve_schema"))
    end = next(i for i in range(start, len(lines)) if lines[i] == "#[test]")
    while not lines[end - 1].strip():
        end -= 1
    return {"path": str(FIXTURE.relative_to(ROOT)),
            "client_validator_range": f"{start + 1}-{end}", "lines": end - start,
            "note": "resolve_schema through classify, excluding fixture setup and tests"}


def dependencies(program, manifest, checks):
    result, _ = command(cargo(program, "metadata", "--offline", "--locked", "--manifest-path",
                              manifest, "--format-version", "1"), checks)
    packages = [package for package in json.loads(result.stdout)["packages"]
                if package["name"] != "kiln-progenitor-measurement"]
    root_packages = tomllib.loads(ROOT_LOCK.read_text())["package"]
    root_names = {(package["name"], package["version"]) for package in root_packages}
    added = [package for package in packages
             if (package["name"], package["version"]) not in root_names]
    manifest_data = tomllib.loads(manifest.read_text())
    direct_names = set(manifest_data["dependencies"]) | set(manifest_data["build-dependencies"])
    licenses = {}
    for package in added:
        licenses.setdefault(package["license"] or "NOASSERTION", []).append(
            f'{package["name"]}@{package["version"]}')
    return {
        "direct": len(direct_names),
        "transitive": len(packages) - len(direct_names),
        "locked_total": len(packages),
        "added_relative_to_root_lock_by_name_and_version": len(added),
        "added_license_inventory": {license_id: sorted(names)
                                    for license_id, names in sorted(licenses.items())},
        "note": "Resolved packages include target-conditional dependencies; not a linked-binary inventory.",
    }


def generated_size(target):
    matches = list(target.glob("debug/build/kiln-progenitor-measurement-*/out/codegen.rs"))
    if len(matches) != 1:
        raise RuntimeError(f"expected one generated codegen.rs, found {len(matches)}")
    content = matches[0].read_bytes()
    return {"generated_rust_lines": content.count(b"\n"), "generated_rust_bytes": len(content)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scratch", type=Path, default=DEFAULT_SCRATCH)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cargo", default=DEFAULT_CARGO,
                        help="Cargo program; run inside `nix develop` or pass the dev-shell wrapper")
    parser.add_argument("--jobs", type=int, default=2)
    args = parser.parse_args()
    if not 1 <= args.jobs <= MAX_JOBS:
        parser.error(f"--jobs must be between 1 and {MAX_JOBS}")
    args.scratch.mkdir(parents=True, exist_ok=True)
    checks = []
    with tempfile.TemporaryDirectory(prefix="progenitor-", dir=args.scratch) as temporary:
        work = Path(temporary)
        (work / "src").mkdir()
        manifest = work / "Cargo.toml"
        manifest.write_text(MANIFEST)
        (work / "build.rs").write_text(BUILD_SCRIPT)
        (work / "src/lib.rs").write_text('include!(concat!(env!("OUT_DIR"), "/codegen.rs"));\n')
        original_bytes = CONTRACT.read_bytes()
        (work / "openapi.json").write_bytes(original_bytes)
        command(cargo(args.cargo, "generate-lockfile", "--manifest-path", manifest), checks)
        original, original_seconds = build(args.cargo, manifest, work / "target-original", args.jobs,
                                           checks)
        error_match = re.search(r'UnexpectedFormat\([^\n]+', original.stderr)
        unchanged = {
            "generated_rust_lines": None,
            "generated_rust_bytes": None,
            "error": error_match.group(0) if error_match else original.stderr.strip(),
            "cold_build_wall_seconds": original_seconds,
            "result": "passed" if original.returncode == 0 else "failed at generation",
        }
        if original.returncode == 0:
            unchanged.update(generated_size(work / "target-original"))
        document = json.loads(original_bytes)
        adaptations = []
        for path, operation_id in OPERATION_IDS.items():
            operation = document["paths"][path]["get"]
            if "operationId" in operation:
                raise RuntimeError(f"the committed document already has operationId on GET {path}")
            operation["operationId"] = operation_id
            adaptations.append(f"GET {path}: operationId={operation_id}")
        (work / "openapi.json").write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
        cold, cold_seconds = build(args.cargo, manifest, work / "target-adapted", args.jobs, checks)
        if cold.returncode:
            raise RuntimeError(f"adapted generation/build failed:\n{cold.stdout}\n{cold.stderr}")
        warm, warm_seconds = build(args.cargo, manifest, work / "target-adapted", args.jobs, checks)
        if warm.returncode:
            raise RuntimeError(f"adapted warm build failed:\n{warm.stdout}\n{warm.stderr}")
        adapted = {
            "source": "scratch copy of unchanged committed document",
            "adaptations": adaptations,
            **generated_size(work / "target-adapted"),
            "cold_build_wall_seconds": cold_seconds,
            "warm_rebuild_wall_seconds": warm_seconds,
            "result": "generation and cold/warm builds passed",
        }
        receipt = {
            "observed_on": date.today().isoformat(), "issue": 73,
            "scope": "Pinned Progenitor generation of unchanged contract and five-operationId scratch copy",
            "generator": {"name": "progenitor", "revision": REVISION, "license": "MPL-2.0",
                          "method": "documented build.rs Generator::generate_tokens -> syn::parse2 -> prettyplease::unparse"},
            "reproduce": f"python3 scripts/http-contract-generation-measure.py --jobs {args.jobs} --output <receipt.json>",
            "scratch_parent": str(args.scratch), "cpu_jobs": args.jobs,
            "handwritten_fixture": fixture_size(), "unchanged_document": unchanged,
            "adapted_scratch_document": adapted, "dependencies": dependencies(args.cargo, manifest, checks),
            "checks": checks,
            "non_claims": [
                "The unchanged committed contract does not generate; five operationId fields were added only in scratch.",
                "Resolved dependency inventories include target-conditional crates, not only linked code.",
                "Single-host cold/warm times are observations and differ from earlier ad-hoc measurements; warm is an unchanged rebuild.",
                "No generated client or Progenitor dependency was added to the Kiln root workspace or production service.",
            ],
        }
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(receipt, indent=2) + "\n")
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
