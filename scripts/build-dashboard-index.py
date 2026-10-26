#!/usr/bin/env python3
"""Build the locked, self-contained Ratzilla index assets; never install tools."""

import argparse
import base64
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent
CLIENT = ROOT / "web/dashboard-index"
DESTINATION = ROOT / "crates/kiln-dashboard/src/render/generated"
TARGET = "wasm32-unknown-unknown"
WASM_BINDGEN_VERSION = "0.2.121"
ESBUILD_VERSION = "0.27.2"
ASSETS = ("rats-index.js", "rats-index.wasm.b64", "rats-index-notices.txt")
# These registry archives omit their repository-root licenses. Retain the MIT
# option from the exact published source revisions; never fetch at build time.
SUPPLEMENTAL_NOTICES = {
    **{(name, "1.0.0"): ("beamterm-1.0.0-LICENSE",
       "https://raw.githubusercontent.com/junkdog/beamterm/fd8066e840ebf4d7ad26dbfcc0ac5f4b7b34b7e3/LICENSE")
       for name in ("beamterm-core", "beamterm-data", "beamterm-renderer", "beamterm-unicode")},
    ("ratzilla", "0.3.1"): ("ratzilla-0.3.1-LICENSE-MIT",
        "https://raw.githubusercontent.com/ratatui/ratzilla/5d1bff3a9d503ab875008bb10bd7fa9103053c9b/LICENSE-MIT"),
    ("rustc-std-workspace-core", "1.0.1"): ("rustc-std-workspace-core-1.0.1-LICENSE-MIT",
        "https://raw.githubusercontent.com/rust-lang/rust/82c24eccadc216cac4365ce83ac6a36ef72ff1a3/LICENSE-MIT"),
}


def capture(command, *, env=None):
    return subprocess.check_output(command, cwd=ROOT, env=env, text=True).strip()


def run(command, *, env=None):
    subprocess.run(command, cwd=ROOT, env=env, check=True)


def require_version(command, expected):
    observed = capture([command, "--version"])
    if observed not in (expected, f"wasm-bindgen {expected}"):
        raise RuntimeError(f"{command}: need version {expected}, found {observed}")


def package_notices(package):
    """Collect original license/NOTICE files, including pinned archive omissions."""
    manifest = Path(package["manifest_path"])
    if package["name"] == "kiln-dashboard-index" and package["source"] is None:
        paths = [ROOT / "LICENSE-MIT", ROOT / "LICENSE-APACHE"]
        source_root = ROOT
    elif package["name"] == "rat-table" and package["source"] is None:
        source_root = ROOT / "vendor/subwayrat"
        paths = [source_root / "LICENSE", source_root / "LICENSING.md"]
    else:
        source_root = manifest.parent
        paths = []
        explicit = package.get("license_file")
        if explicit:
            path = source_root / explicit
            if not path.is_file():
                raise RuntimeError(f"Missing declared license file for {package['name']}: {explicit}")
            paths.append(path)
        for path in source_root.iterdir():
            upper = path.name.upper()
            relevant = upper.startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT"))
            if relevant:
                if path.is_file():
                    paths.append(path)
                elif path.is_dir():
                    paths.extend(child for child in path.rglob("*") if child.is_file())
    if not paths:
        supplemental = SUPPLEMENTAL_NOTICES.get((package["name"], package["version"]))
        if supplemental:
            filename, upstream = supplemental
            path = CLIENT / "notices" / filename
            return [(upstream, path.read_text(encoding="utf-8"))]
        raise RuntimeError(f"No distributed license/NOTICE files found for {package['name']} {package['version']}")
    notices = []
    for path in sorted(set(paths), key=lambda path: path.relative_to(source_root).as_posix()):
        if not path.is_file():
            raise RuntimeError(f"Missing notice for {package['name']}: {path.name}")
        # Fail rather than silently discard or replace unknown notice encodings.
        notices.append((path.relative_to(source_root).as_posix(), path.read_text(encoding="utf-8")))
    return notices


def collect_notices(cargo, env):
    metadata = json.loads(capture([
        cargo, "metadata", "--locked", "--format-version", "1",
        "--filter-platform", TARGET, "--manifest-path", str(CLIENT / "Cargo.toml"),
    ], env=env))
    root_id = next(package["id"] for package in metadata["packages"]
                   if package["name"] == "kiln-dashboard-index" and package["source"] is None)
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    pending = [root_id]
    reached = set()
    while pending:
        identity = pending.pop()
        if identity in reached:
            continue
        reached.add(identity)
        pending.extend(dependency["pkg"] for dependency in nodes[identity]["deps"])
    packages = sorted((package for package in metadata["packages"] if package["id"] in reached),
                      key=lambda package: (package["name"], package["version"], package["source"] or ""))
    text = [
        "Kiln offline repository/history index — dependency notices\n",
        "This is the locked wasm32 metadata dependency cohort (including build tools, proc macros and optional edges),\n"
        "not a claim that every package is linked into the final WASM. Original license/NOTICE texts follow.\n"
        "Registry packages come from the public crates.io registry; local package source is identified below.\n",
    ]
    for package in packages:
        source = package["source"] or ("Kiln browser workspace" if package["name"] == "kiln-dashboard-index"
                                         else "Subwayrat selected source at 2e52b3150819a2365aaefd3dcf8bbd2a2fa2e901 (Kiln wrapper manifest)")
        text.append(f"\n{'=' * 72}\n{package['name']} {package['version']}\nSource: {source}\nDeclared license: {package.get('license') or 'see license files'}\n")
        for name, content in package_notices(package):
            text.append(f"\n--- {name} ---\n{content.rstrip()}\n")
    return "".join(text).encode("utf-8"), packages


def build(args, temporary):
    lock_path = CLIENT / "Cargo.lock"
    if not lock_path.is_file():
        raise RuntimeError("Missing isolated web/dashboard-index/Cargo.lock; generate and review it before building assets")
    lock = tomllib.loads(lock_path.read_text(encoding="utf-8"))
    versions = {package["version"] for package in lock["package"] if package["name"] == "wasm-bindgen"}
    if versions != {WASM_BINDGEN_VERSION}:
        raise RuntimeError(f"Browser lock must contain wasm-bindgen {WASM_BINDGEN_VERSION}; found {sorted(versions)}")
    require_version(args.wasm_bindgen, WASM_BINDGEN_VERSION)
    require_version(args.esbuild, ESBUILD_VERSION)
    env = os.environ.copy()
    flags = (env.get("CARGO_ENCODED_RUSTFLAGS", "").split("\x1f")
             if env.get("CARGO_ENCODED_RUSTFLAGS") else shlex.split(env.get("RUSTFLAGS", "")))
    cargo_home = Path(env.get("CARGO_HOME", str(Path.home() / ".cargo"))).resolve()
    sysroot = capture([args.rustc, "--print", "sysroot"])
    for source, replacement in ((str(ROOT), "kiln"), (str(temporary), "kiln-dashboard-build"),
                                (str(cargo_home), "cargo"), (sysroot, "rustc-sysroot")):
        flags.append(f"--remap-path-prefix={source}={replacement}")
    env.pop("RUSTFLAGS", None)
    env.update({"CARGO_ENCODED_RUSTFLAGS": "\x1f".join(flags), "SOURCE_DATE_EPOCH": "0",
                "CARGO_INCREMENTAL": "0", "TZ": "UTC", "LC_ALL": "C", "RUSTC": args.rustc})
    notices, packages = collect_notices(args.cargo, env)
    registry_roots = set()
    for package in packages:
        if package["source"] and package["source"].startswith("registry+"):
            source = Path(package["manifest_path"]).parent
            # Nix's cargoDepsCopy and Cargo's registry cache use different roots;
            # normalize the actual metadata paths, including symlink spellings.
            for path in sorted({str(source), str(source.resolve())}):
                registry_roots.add(path)
                flags.append(f"--remap-path-prefix={path}=registry/{package['name']}-{package['version']}")
    env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(flags)
    run([args.cargo, "build", "--locked", "--release", "--target", TARGET,
         "--manifest-path", str(CLIENT / "Cargo.toml"), "--target-dir", str(temporary / "target")], env=env)
    bindgen = temporary / "bindgen"
    run([args.wasm_bindgen, "--target", "web", "--no-typescript", "--out-dir", str(bindgen),
         "--out-name", "kiln_dashboard_index",
         str(temporary / f"target/{TARGET}/release/kiln_dashboard_index.wasm")], env=env)
    shutil.copyfile(CLIENT / "loader.js", temporary / "loader.js")
    js_path = temporary / "rats-index.js"
    run([args.esbuild, str(temporary / "loader.js"), "--bundle", "--format=esm", "--platform=browser",
         "--target=es2020", "--charset=ascii", "--minify", "--legal-comments=none", f"--outfile={js_path}"], env=env)
    wasm = (bindgen / "kiln_dashboard_index_bg.wasm").read_bytes()
    result = {"rats-index.js": js_path.read_bytes(), "rats-index.wasm.b64": base64.b64encode(wasm),
              "rats-index-notices.txt": notices}
    for name, content in result.items():
        for forbidden in (str(ROOT).encode(), str(temporary).encode(), str(cargo_home).encode(), sysroot.encode(),
                          *(path.encode() for path in sorted(registry_roots))):
            if forbidden in wasm:
                raise RuntimeError("Absolute build/source path leaked into raw WASM")
            if forbidden in content:
                raise RuntimeError(f"Absolute build/source path leaked into {name}")
    return result


# r[impl kiln.dashboard.rats.provenance.scenario.drift]
# r[verify kiln.dashboard.rats.provenance.scenario.drift]
def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="compare freshly rebuilt assets without modifying checked-in files")
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--wasm-bindgen", default="wasm-bindgen")
    parser.add_argument("--esbuild", default="esbuild")
    args = parser.parse_args()
    try:
        with tempfile.TemporaryDirectory(prefix="kiln-dashboard-index-") as directory:
            assets = build(args, Path(directory))
        if args.check:
            drift = [name for name in ASSETS if not (DESTINATION / name).is_file()
                     or (DESTINATION / name).read_bytes() != assets[name]]
            if drift:
                raise RuntimeError("Dashboard index asset drift: " + ", ".join(drift))
            print("Verified locked dashboard index JS, WASM and dependency notices")
        else:
            DESTINATION.mkdir(parents=True, exist_ok=True)
            for name in ASSETS:
                (DESTINATION / name).write_bytes(assets[name])
            print("Generated locked dashboard index JS, WASM and dependency notices")
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"dashboard index generation failed: {error}\n")


if __name__ == "__main__":
    main()
