"""Prepare real dotenv-backed SecretSpec credentials for a fresh native canary."""

import base64
import binascii
import os
from pathlib import Path
import stat
import struct


MAX_NATIVE_KEY_BYTES = 4096
ED25519_PUBLIC_BYTES = 32
ED25519_KEYPAIR_BYTES = 64
OPENSSH_BLOCK_BYTES = 8
MANIFEST = b'''[project]
name = "kiln-canary"
revision = "1.0"
require_reason = true

[profiles.production]
RADICLE_SIGNER_SEED = { description = "Dedicated native canary status signer", required = true }
'''


def prepare_signer(project_root: Path, key_path: Path) -> list[Path]:
    """Create a private manifest and .env; return credential paths to retire.

    Only the fresh node's unencrypted OpenSSH Ed25519 identity is accepted.
    Repeated public-key fields, checkints, framing and padding are checked
    structurally; deriving the public key from the seed remains the production
    resolver's responsibility, including its separately pinned signer admission.
    Neither existing credentials nor a manifest are overwritten. Both the native
    key and the newly created .env are returned for retirement.
    """
    project_root.mkdir(mode=0o700, exist_ok=True)
    root_fd = os.open(project_root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    created = []
    try:
        root_info = os.fstat(root_fd)
        if root_info.st_uid != os.getuid() or stat.S_IMODE(root_info.st_mode) != 0o700:
            raise ValueError("Signer project must be an owned mode-0700 directory")

        key_fd = os.open(key_path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(key_fd, "rb") as key_file:
            info = os.fstat(key_file.fileno())
            if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid()
                    or stat.S_IMODE(info.st_mode) != 0o600 or info.st_nlink != 1
                    or not 0 < info.st_size <= MAX_NATIVE_KEY_BYTES):
                raise ValueError("Native key must be an owned, single-link mode-0600 bounded file")
            armored = key_file.read(MAX_NATIVE_KEY_BYTES + 1)
            if len(armored) != info.st_size:
                raise ValueError("Native key changed or exceeded its byte bound")

        lines = armored.split(b"\n")
        if (len(lines) < 4 or lines[0] != b"-----BEGIN OPENSSH PRIVATE KEY-----"
                or lines[-2:] != [b"-----END OPENSSH PRIVATE KEY-----", b""]
                or any(not line for line in lines[1:-2])):
            raise ValueError("Native key is not the expected OpenSSH PEM encoding")
        try:
            data = base64.b64decode(b"".join(lines[1:-2]), validate=True)
        except binascii.Error:
            raise ValueError("Native key has invalid OpenSSH base64") from None
        magic = b"openssh-key-v1\0"
        if not data.startswith(magic):
            raise ValueError("Native key lacks OpenSSH framing")
        offset = len(magic)

        # OpenSSH uses only big-endian uint32 and length-prefixed strings here.
        # These local readers operate solely on this bounded native key blob.
        def uint32():
            nonlocal offset
            if offset + 4 > len(data):
                raise ValueError("Native key has truncated OpenSSH framing")
            value = struct.unpack_from(">I", data, offset)[0]
            offset += 4
            return value

        def string():
            nonlocal offset
            size = uint32()
            end = offset + size
            if end > len(data):
                raise ValueError("Native key has a truncated OpenSSH string")
            value = data[offset:end]
            offset = end
            return value

        if string() != b"none" or string() != b"none" or string() != b"":
            raise ValueError("Native canary key must be unencrypted without a KDF")
        if uint32() != 1:
            raise ValueError("Native canary key must contain exactly one identity")
        public_blob = string()
        private_blob = string()
        if offset != len(data):
            raise ValueError("Native key has trailing OpenSSH container data")

        data, offset = public_blob, 0
        if string() != b"ssh-ed25519":
            raise ValueError("Native canary key must be Ed25519")
        public_key = string()
        if len(public_key) != ED25519_PUBLIC_BYTES or offset != len(data):
            raise ValueError("Native key has malformed OpenSSH public data")

        data, offset = private_blob, 0
        if len(data) % OPENSSH_BLOCK_BYTES or uint32() != uint32():
            raise ValueError("Native key has invalid OpenSSH alignment or checkints")
        if string() != b"ssh-ed25519" or string() != public_key:
            raise ValueError("Native key has inconsistent OpenSSH public fields")
        keypair = string()
        if len(keypair) != ED25519_KEYPAIR_BYTES or keypair[ED25519_PUBLIC_BYTES:] != public_key:
            raise ValueError("Native key has inconsistent OpenSSH keypair fields")
        string()  # The native node alias is an opaque OpenSSH comment.
        padding = data[offset:]
        if (len(padding) >= OPENSSH_BLOCK_BYTES
                or padding != bytes(range(1, len(padding) + 1))):
            raise ValueError("Native key has invalid OpenSSH padding")
        seed = keypair[:ED25519_PUBLIC_BYTES]
        if not any(seed):
            raise ValueError("Native key has a zero signer seed")
        credential = b"RADICLE_SIGNER_SEED=" + seed.hex().encode("ascii") + b"\n"

        # SecretSpec 0.19.0's bare dotenv provider reads .env next to its
        # manifest, with flat key names, independent of project/profile/env.
        for name, content in (("secretspec.toml", MANIFEST), (".env", credential)):
            fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                         0o600, dir_fd=root_fd)
            created.append(name)
            with os.fdopen(fd, "wb") as output:
                os.fchmod(output.fileno(), 0o600)
                output.write(content)
        return [key_path, project_root / ".env"]
    except BaseException:
        for name in reversed(created):
            os.unlink(name, dir_fd=root_fd)
        raise
    finally:
        os.close(root_fd)
