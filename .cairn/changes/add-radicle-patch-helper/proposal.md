## Why

Issues #18/#20 need the upstream Radicle patch reducer. Linking the `radicle`
crate directly conflicts with Kiln's `rusqlite` because both link native
SQLite. A separate process is the compatible boundary.

## What Changes

- Add `tools/radicle-patch`, a separate workspace around `radicle` 0.25.1.
- Add a bounded adapter invocation that runs the helper and parses one reduced
  patch JSON projection.
- Keep the helper read-only; no mutation or Kiln authority.

## Impact

- **Files**: `tools/radicle-patch/**`,
  `crates/kiln-adapter-radicle/src/patch_read.rs`.
- **Testing**: helper build and bounded-output tests.
- **Authority**: read-only projection only.
