"""Canonical native newc image builder; shares the hosted runner's byte contract."""
import argparse
import json
import pathlib
import stat
import struct
import subprocess


def digest(path):
    return subprocess.check_output(["b3sum", "--no-names", str(path)], text=True).strip()


def regular(path):
    path = pathlib.Path(path)
    if not stat.S_ISREG(path.lstat().st_mode):
        raise ValueError(f"not a regular file: {path}")
    return path.read_bytes()


def static_pie(data):
    if len(data) < 64 or data[:7] != b"\x7fELF\x02\x01\x01":
        raise ValueError("expected little-endian ELF64")
    if struct.unpack_from("<HH", data, 16) != (3, 62):
        raise ValueError("expected x86_64 ET_DYN static PIE")
    offset = struct.unpack_from("<Q", data, 32)[0]
    size, count = struct.unpack_from("<HH", data, 54)
    if size != 56 or count == 0 or offset + size * count > len(data):
        raise ValueError("invalid program header table")
    for index in range(count):
        header = offset + index * size
        kind = struct.unpack_from("<I", data, header)[0]
        if kind == 3:
            raise ValueError("PT_INTERP is forbidden")
        if kind == 2:
            start = struct.unpack_from("<Q", data, header + 8)[0]
            length = struct.unpack_from("<Q", data, header + 32)[0]
            if start + length > len(data) or length % 16:
                raise ValueError("invalid dynamic segment")
            for cursor in range(start, start + length, 16):
                tag = struct.unpack_from("<q", data, cursor)[0]
                if tag == 0:
                    break
                if tag == 1:
                    raise ValueError("DT_NEEDED is forbidden")


def entry(name, data, inode, mode):
    name = name.encode("utf-8") + b"\0"
    fields = [inode, mode, 0, 0, 1, 0, len(data), 0, 0, 0, 0, len(name), 0]
    if any(value > 0xFFFFFFFF for value in fields):
        raise ValueError("newc field overflow")
    header = b"070701" + b"".join(f"{value:08x}".encode() for value in fields) + name
    return header + b"\0" * (-len(header) % 4) + data + b"\0" * (-len(data) % 4)


def image(binary, fixtures):
    executable = regular(binary)
    static_pie(executable)
    entries = {".": (b"", 0o40755), "test": (executable, 0o100755)}
    for fixture in fixtures:
        name = fixture["path"]
        if not name or "\0" in name or name.startswith("/") or any(part in ("", ".", "..") for part in name.split("/")):
            raise ValueError(f"invalid fixture path: {name}")
        if name.split("/", 1)[0] == "TRAILER!!!" or name in entries:
            raise ValueError(f"duplicate or reserved fixture path: {name}")
        data = regular(fixture["source"])
        entries[name] = (data, 0o100444)
        parent = pathlib.PurePosixPath(name).parent
        while str(parent) != ".":
            key = str(parent)
            if key in entries and entries[key][1] != 0o40755:
                raise ValueError(f"fixture collides with file: {key}")
            entries[key] = (b"", 0o40755)
            parent = parent.parent
    result = bytearray()
    for inode, name in enumerate(sorted(entries), 1):
        data, mode = entries[name]
        result.extend(entry(name, data, inode, mode))
    result.extend(entry("TRAILER!!!", b"", len(entries) + 1, 0))
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--fixtures", required=True)
    parser.add_argument("--guest", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    fixtures = json.loads(pathlib.Path(args.fixtures).read_text())
    out = pathlib.Path(args.out)
    out.mkdir(parents=True)
    initrd = out / "initrd.cpio"
    initrd.write_bytes(image(args.binary, fixtures))
    guest = pathlib.Path(args.guest)
    manifest = {
        "schema": "kiln.unikernel.image.v1",
        "guest_kind": "unikernel", "project": "unikraft",
        "version": "a4c494e80141bd402961c7d96150dd218c79db37",
        "kernel": {"path": str(guest / "kernel"), "blake3": digest(guest / "kernel")},
        "sources": json.loads((guest / "sources.json").read_text()),
        "firecracker": json.loads((guest / "manifest.json").read_text())["firecracker"],
        "binary": {"path": str(args.binary), "blake3": digest(args.binary)},
        "fixtures": [{"path": f["path"], "blake3": digest(f["source"])} for f in sorted(fixtures, key=lambda f: f["path"])],
        "image": {"path": str(initrd), "blake3": digest(initrd)},
        "hostile_isolation_claim": False,
    }
    (out / "manifest.json").write_text(json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n")


if __name__ == "__main__":
    main()
