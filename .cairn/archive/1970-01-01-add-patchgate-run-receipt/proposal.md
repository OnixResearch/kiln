## Why

`kiln-patchgate-review` currently exposes separate `plan` and `review`
commands. Retaining the result requires a shell redirect, which is not atomic,
and a reviewed worker step needs an additional upload step. A single command
that admits the intent, performs the isolated review, and publishes exactly one
receipt gives worker profiles a stable report path without exposing the private
native report in command output.

## What Changes

- Add `kiln-patchgate-review run --profile PROFILE --intent INTENT --output
  RECEIPT`; it builds and validates the plan, performs the same isolated review,
  renders `kiln.patchgate.review.v1`, and publishes the receipt atomically.
- Publish through a same-directory temporary file with file and directory
  synchronization; reject a symlink or non-regular destination.
- Print only a bounded summary containing the receipt path, identities and
  per-change decisions. The private native report remains inside the receipt.
- Document the worker profile `reports = [{ kind = "review", path =
  "patchgate-review.json" }]` retention path.

## Impact

- **Files**: `crates/kiln-adapter-patchgate/src/main.rs`,
  `crates/kiln-adapter-patchgate/Cargo.toml`, `docs/operator.md`.
- **Testing**: atomic publication positive and symlink-rejection tests, an
  end-to-end fake-producer run smoke, and the focused crate test/build checks.
- **Authority**: no new review, merge, gate or release authority. Automatic
  intent construction from live Pijul state and run upload remain separate
  integration work.
