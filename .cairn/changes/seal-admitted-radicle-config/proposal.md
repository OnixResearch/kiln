## Why

`AdmittedRadicleConfig` is a proof of reviewed Radicle shell-profile admission,
but its fields are public, so another crate can fabricate or replace admission
without running `admit_config`.

## What Changes

- Make the admitted fields crate-private.
- Expose read-only shell, Lattice and transport accessors.
- Update the Radicle binary and pilot consumers to use the accessors.
- Add a compile-fail proof check and update the admitted-proof audit.

## Impact

- **Files**: Radicle profile, binary consumer, Onix pilot consumer, tests and
  proof specification.
- **Testing**: Radicle adapter tests, pilot checks, workspace tests, Clippy and
  Cairn validation.
