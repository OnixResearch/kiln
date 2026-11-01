# Verification evidence

## Result

`AdmittedJournal` fields are crate-private. External crates can read admitted
state, records, schema, declared identity and review through accessors, and can
attach a review window only through `with_review`. The shell now uses that
operation. A compile-fail doc test records that external struct construction is
rejected.

## Evidence

- `cargo test -p kiln-dashboard`: 42 dashboard tests passed, including the
  accessor and compile-fail proof checks.
- `cargo test --workspace --no-fail-fast`: all workspace test binaries and doc
  tests passed.
- `cargo check --workspace --all-targets` and workspace-wide Clippy with
  `-D warnings` passed after updating the external dashboard tests.
- `cargo fmt` passed.
- The accepted admitted-proof audit is modified so `AdmittedJournal` is recorded
  as sealed and the remaining three proof types stay named.
- `cairn validate` passed with zero issues.

## Non-claims

The seal proves external construction and mutation are unavailable; it does not
prove that in-crate callers construct only after admission, nor that journal
contents are semantically correct. `with_review` can replace an attached review
window by design.
