#!/usr/bin/env python3
"""Measure the guest kernel driver surface against a reviewed deny list.

This is a measurement over pinned build inputs, not an isolation claim. It
marks a class `builtin` when the kernel configuration compiles any matching
symbol in, `loadable_module_present` when the guest module closure ships a
matching module, and `absent` otherwise. A class with `enforce = true` fails
the check in either non-absent state; measured-only classes are recorded.
"""

import argparse
import json
import sys
from pathlib import Path

NON_CLAIMS = [
    "an absent class means the guest image neither compiles it in nor ships a loadable module",
    "the measurement does not prove isolation, confinement, or release readiness",
    "runtime module loading remains denied by modules_disabled=1; this inventory is about the image",
    "builtin measured-only classes are recorded misses, not accepted decisions",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--denylist", required=True, type=Path)
    parser.add_argument("--config", required=True, type=Path)
    parser.add_argument("--modules", required=True, type=Path)
    parser.add_argument("--kernel-version", required=True)
    parser.add_argument("--config-blake3", required=True)
    parser.add_argument("--nixpkgs-revision", required=True)
    parser.add_argument("--vmm", required=True, choices=["firecracker", "cloud-hypervisor"])
    parser.add_argument("--out", required=True, type=Path)
    return parser.parse_args()


def kernel_config(path: Path) -> dict[str, str]:
    raw = path.read_bytes()
    values: dict[str, str] = {}
    for line in raw.decode("utf-8").splitlines():
        if not line.startswith("CONFIG_") or "=" not in line:
            continue
        name, value = line.split("=", 1)
        values[name] = value.strip().strip('"')
    return values


def closure_modules(root: Path) -> list[str]:
    if not root.is_dir():
        return []
    names = []
    for module in root.rglob("*.ko*"):
        name = module.name
        for suffix in (".xz", ".gz", ".zst"):
            if name.endswith(suffix):
                name = name[: -len(suffix)]
                break
        if name.endswith(".ko"):
            name = name[:-3]
        names.append(name)
    return sorted(set(names))


def classify(entry: dict, config: dict[str, str], modules: list[str]) -> dict:
    builtin = []
    kernel_modules = []
    for symbol, value in config.items():
        if symbol not in entry["symbols"]:
            continue
        if value == "y":
            builtin.append(symbol)
        elif value == "m":
            kernel_modules.append(symbol)
    fragments = [fragment.lower() for fragment in entry["module_fragments"]]
    shipped = [
        module
        for module in modules
        if any(fragment in module.lower() for fragment in fragments)
    ]
    if builtin:
        verdict = "builtin"
    elif shipped:
        verdict = "loadable_module_present"
    else:
        verdict = "absent"
    return {
        "name": entry["name"],
        "enforce": entry["enforce"],
        "verdict": verdict,
        "builtin_symbols": sorted(builtin),
        "kernel_module_symbols": sorted(kernel_modules),
        "closure_modules": shipped,
    }


def main() -> int:
    args = parse_args()
    deny = json.loads(args.denylist.read_text())
    if deny.get("schema") != "kiln.microvm.guest-surface.v1":
        print("deny list schema is unsupported", file=sys.stderr)
        return 2
    config = kernel_config(args.config)
    config_digest = args.config_blake3
    modules = closure_modules(args.modules)
    classes = [classify(entry, config, modules) for entry in deny["classes"]]
    violations = [
        row
        for row in classes
        if row["enforce"] and row["verdict"] != "absent"
    ]
    inventory = {
        "schema": "kiln.microvm.guest-surface.v1",
        "vmm": args.vmm,
        "nixpkgs_revision": args.nixpkgs_revision,
        "kernel_version": args.kernel_version,
        "kernel_config_blake3": config_digest,
        "closure_module_count": len(modules),
        "classes": classes,
        "deny_list_verdict": "fail" if violations else "pass",
        "violations": [row["name"] for row in violations],
        "non_claims": NON_CLAIMS,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(inventory, indent=2, sort_keys=True) + "\n")
    if violations:
        print(
            "deny-listed driver classes are present in the guest image: "
            + ", ".join(row["name"] for row in violations),
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
