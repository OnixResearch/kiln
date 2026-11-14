#!/usr/bin/env python3
"""Acquire already-built public TryNix runtime bytes; never evaluate/build Nix.

The fixed source/engine cohort is intentional. Bytes are read-only public GETs,
full SHA256 pins below are checked before use, and the resulting local manifest
must still be selected by the operator's exact preview descriptor. This does not
produce CI/build evidence or grant any artifact/cache signing authority.
"""
import argparse
import base64
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import shutil
import urllib.request

SOURCE = "7e7e793594cb58efb48279b17b4e15bbc6fbe91b"
MODULES = "js.2665d2d35976"
BASE = "https://trynix.dev/"
LIMIT = 256 * 1024 * 1024
PINS = {
    f"{MODULES}/boot.js": "7b5af29e58591d41af4c25c95f5ddda4b0da43ac00baf29f874822261b105755",
    "qemu/out.js": "64ba604a4ad0ad2ddebb0940a9812e43f0f8a6bd72fdb88bf3d46666a19cc6b7",
    "qemu/qemu-system-x86_64.wasm": "8e0f4f1735289bf0e112dcdb25de1b89ed80e2c854e518d70b21642668ffcc62",
    "qemu/qemu-system-x86_64.worker.js": "8efab02a9cb5ee63ced22e9853bce74cbb895d15a5fb3d5139fef855c50f62ea",
    "qemu/vm.state": base64.b64decode("vWIMu/r3rr8jXYlhFeamdmE828asv70O6xEdROndeeo=").hex(),
    "guest/bzImage": base64.b64decode("pemVqhxy+olj/osRr1pWEr998RxeiNHv/eWeQiSQI0k=").hex(),
    "guest/initramfs.cpio.gz": base64.b64decode("5ewD3Gf+9zaN1EObAXI0VXOxXvi1Tl3cw7f1eNyytlU=").hex(),
    "guest/machine.json": base64.b64decode("QMWubLHzHx4cSizdmz6pED3kDXK+seGgIJobVw5eKao=").hex(),
}
EXISTING = {
    "qemu/out.js": "/nix/store/s42af61g189b5akd863752yjml7dysgs-out.js",
    "qemu/qemu-system-x86_64.wasm": "/nix/store/f791ksb1l9fdk2z5lficipridzn6harr-qemu-system-x86_64.wasm",
    "qemu/qemu-system-x86_64.worker.js": "/nix/store/d6bwpqqgxvrxwwk9b0p3vsydbrwm8pr7-qemu-system-x86_64.worker.js",
}
IMPORTS = re.compile(r'''(?:\bfrom\s*|\bimport\s*\(\s*|new URL\(\s*)["'](\.{1,2}/[^"']+)["']''')

def normal(base, relative):
    parts = []
    for part in (PurePosixPath(base).parent / relative).parts:
        if part == "..":
            if not parts:
                raise ValueError("upstream import escaped runtime root")
            parts.pop()
        elif part != ".":
            parts.append(part)
    name = "/".join(parts)
    if not name.startswith((MODULES + "/", "vendor/", "qemu/", "guest/")):
        raise ValueError("unexpected upstream runtime dependency: " + name)
    return name

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    total = 0
    files = {}
    pending = list(PINS) + [f"{MODULES}/nar.js", "guest/bios-256k.bin", "guest/vgabios-stdvga.bin", "guest/kvmvapic.bin", "guest/linuxboot_dma.bin", "vendor/xterm-pty.js", "vendor/ghostty-web.js", "vendor/ghostty-vt.wasm"]
    try:
        while pending:
            name = pending.pop()
            if name in files:
                continue
            if len(files) >= 256:
                raise ValueError("engine file budget exhausted")
            target = args.output / name
            target.parent.mkdir(parents=True, exist_ok=True)
            local = Path(EXISTING.get(name, "/nonexistent-kiln-preview"))
            source = local.open("rb") if local.is_file() else urllib.request.urlopen(BASE + name, timeout=30)
            digest = hashlib.sha256()
            size = 0
            with source, target.open("xb") as output:
                while chunk := source.read(65536):
                    total += len(chunk)
                    size += len(chunk)
                    if total > LIMIT:
                        raise ValueError("engine byte budget exhausted")
                    digest.update(chunk)
                    output.write(chunk)
            sha = digest.hexdigest()
            if name in PINS and sha != PINS[name]:
                raise ValueError("immutable upstream cohort unavailable/hash mismatch: " + name)
            files[name] = {"path": name, "bytes": size, "sha256": sha}
            if name.endswith(".js") and name.startswith((MODULES + "/", "vendor/")):
                for dep in IMPORTS.findall(target.read_text()):
                    # Browser-only vendor shims are real files too. No bare/npm,
                    # node: imports or remote module URL is followed.
                    pending.append(normal(name, dep))
        manifest = {"protocol": "trynix-startvm-v1", "source_revision": SOURCE, "module_dir": MODULES, "files": [files[name] for name in sorted(files)]}
        (args.output / "engine-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        print(json.dumps({"tree": str(args.output), "manifest": str(args.output / "engine-manifest.json"), "source": SOURCE, "bytes": total, "files": len(files)}))
    except BaseException:
        shutil.rmtree(args.output)
        raise

if __name__ == "__main__":
    main()
