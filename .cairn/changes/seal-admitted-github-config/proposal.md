## Why

`AdmittedGithubConfig` is a proof of reviewed GitHub CI profile admission, but
its fields are public, so another crate can fabricate admission without running
`admit_config`.

## What Changes

- Make the admitted config, identities, Lattice and transport fields
  crate-private.
- Expose read-only accessors.
- Update hosted connection re-admission to clone the admitted config.
- Add a compile-fail proof check and complete the admitted-proof audit.

## Impact

- **Files**: GitHub profile, hosted connections, consumers, tests and the proof
  specification.
- **Testing**: GitHub adapter and hosted tests, workspace tests, Clippy and Cairn
  validation.
