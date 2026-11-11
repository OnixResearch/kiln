# Verification evidence

## Result

`AdmittedAspenRuntimeProfile` fields are crate-private. External consumers read
the reviewed runtime profile and identity through accessors; the Aspen
extension, Radicle CI, Radicle binary and pilot compile against that boundary.
A compile-fail doc test records that external struct construction is rejected.

## Evidence

- `cargo check --workspace --all-targets` passed after accessor migration.
- Aspen adapter, extension, Radicle CI and pilot test suites passed; the full
  workspace suite also passed.
- The compile-fail doc test passed.
- `cargo fmt -- --check` and `cargo clippy --workspace --all-targets -- -D
  warnings` passed.
- `cairn validate` passed with zero issues.

## Non-claims

The seal proves external construction and mutation are unavailable; it does not
prove in-crate admission discipline or profile semantic correctness. Accessors
expose borrowed values, so callers may clone them.
