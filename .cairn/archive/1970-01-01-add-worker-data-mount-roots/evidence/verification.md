# Verification evidence

## Result

Worker data mounts now accept an optional reviewed root list. Omitting it keeps
the existing `/srv` and `/var/lib/kiln` defaults. An explicit list may extend the
roots within `/srv`, `/home`, `/mnt`, `/opt`, and `/var/lib/kiln`; each mount is
still canonical, directory-only, symlink-free, outside the checkout, and bound
read-only.

## Evidence

- Focused execution tests pass for default roots, an extended `/home` root, and
  foreign/relative/duplicate root rejection.
- All 167 web-service library tests pass (1 ignored).
- All 81 engine integration tests pass.
- The worker profile and Patchgate fixture typecheck, format, and export with
  the optional roots field.
- `cargo fmt --check -p kiln-web-service` passes.
- `cairn validate --root .` reports zero issues.

## Non-claims

Selecting `/home` or `/opt` can expose sensitive mounted data to the sandbox.
Root selection adds no write, network, environment, credential, or arbitrary
path authority. Unavailable isolation still fails closed without account-authority
fallback. This change does not prove live worker execution of the Patchgate step.
