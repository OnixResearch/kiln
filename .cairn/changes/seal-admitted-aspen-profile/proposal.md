## Why

`AdmittedAspenRuntimeProfile` is a proof of reviewed Aspen runtime-profile
admission, but its fields are public, so another crate can fabricate admission
without running `admit_profile`.

## What Changes

- Make the admitted profile and identity fields crate-private.
- Expose read-only profile and identity accessors.
- Update external consumers to use the accessors.
- Add a compile-fail proof check and update the admitted-proof audit.

## Impact

- **Files**: Aspen profile, extension, Radicle CI and pilot consumers, tests and
  the proof specification.
- **Testing**: Aspen adapter and extension tests, Radicle CI tests, workspace
  tests, Clippy and Cairn validation.
