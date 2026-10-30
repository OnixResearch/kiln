## Why

Issue #19 (and its dependants #18, #20, #21) needs Radicle's canonical-reference
computation. The published `radicle` 0.25.1 crate is exactly the reviewed
heartwood revision `71a91042d96bb994c4cd4311d85f5cd1a6311b4e`, but it cannot be a
workspace dependency: it links the `sqlite` crate, whose native `sqlite3` link
collides with the workspace's rusqlite/libsqlite3-sys (Cargo allows one
`links = "sqlite3"` package). It also does not compile as published. The chosen
approach is to vendor only its canonical module.

## What Changes

- Add `vendor/heartwood-canonical`, a standalone crate carrying the nine
  `git::canonical` source files and the DID wrapper from the pinned revision,
  verbatim except for two fixture-dependent test modules removed with recorded
  anchors, plus hand-written shims for the module paths those files use.
- Add `vendor/heartwood-canonical.source.json` with the upstream commit, upstream
  and derived per-file SHA-256 digests, stripped test modules, shim inventory and
  non-claims.
- Add `scripts/import-heartwood-canonical.sh`, which refreshes only the
  upstream-derived files, verifies every derived digest, and refuses to run when
  the shims are absent.
- Add a `checks.x86_64-linux.heartwood-canonical` rail that builds and tests the
  vendored crate in its own workspace.
- Record the license in `LICENSES.md`.

## Impact

- **Files**: `vendor/heartwood-canonical/**`,
  `vendor/heartwood-canonical.source.json`,
  `scripts/import-heartwood-canonical.sh`, `flake.nix`, `LICENSES.md`.
- **Testing**: vendored crate tests (23 retained upstream tests), clippy with
  warnings denied, the new flake check, and the existing workspace rails to show
  the dependency graph is unchanged. No Kiln crate consumes the module yet.
