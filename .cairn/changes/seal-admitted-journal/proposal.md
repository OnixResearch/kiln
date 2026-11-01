## Why

`AdmittedJournal` is described as proof that a journal document passed
admission, but every field is public. Another crate can construct or replace an
admitted journal with a struct literal, so the type proves only that its name
was written, not that `admit_document` or `admit_store` ran.

The admitted-proof audit names this type as a separate sealing change. Sealing
it closes the same construction-proof gap already closed for `VerifiedEvent`.

## What Changes

- Make `AdmittedJournal` fields crate-private.
- Expose read-only `state`, `records`, `schema`, `declared_identity` and
  `review` accessors.
- Add a named `with_review` operation for the shell-supplied review window.
- Update dashboard tests and the shell to use the accessors and operation.
- Add a compile-fail documentation check against external struct construction.

## Impact

- **Files**: `crates/kiln-dashboard/src/journal.rs`, `shell.rs`, external
  dashboard tests and the proof specification.
- **Testing**: dashboard tests, workspace tests, Clippy and Cairn validation.
