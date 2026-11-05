## Why

Read-only worker data mounts are currently limited to `/srv` and
`/var/lib/kiln`. A real Patchgate target may live under another reviewed data
family such as `/home`, so the sandboxed step cannot use the installed target
without moving operator-owned repositories.

## What Changes

- Add optional `execution.read_only_data_mount_roots` to the isolation policy.
- Default roots remain `/srv` and `/var/lib/kiln`.
- Explicit roots may extend only within `/srv`, `/home`, `/mnt`, `/opt`, and
  `/var/lib/kiln`; roots must be absolute, clean, unique, and bounded.
- Keep each individual data mount canonical, read-only, symlink-free, and
  outside the writable checkout.
- Expose the field through `config/web-worker.ncl` and document the authority.

## Impact

- **Files**: `crates/kiln-web-service/src/execution.rs`,
  `crates/kiln-web-service/src/worker/process.rs`, `config/web-worker.ncl`,
  `config/fixtures/web-worker-patchgate.ncl`, `docs/operator.md`.
- **Testing**: default and extended-root admission, foreign/relative/duplicate
  root rejection, and read-only bind generation.
- **Authority**: explicit roots are operator-reviewed and still read-only. A
  profile can expose a reviewed data family, not arbitrary host paths.
