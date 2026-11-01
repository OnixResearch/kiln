# Verification evidence

## Result

`AdmittedGithubConfig` fields are crate-private. External consumers read the
service config, identities, Lattice and transport through accessors. Hosted
connection re-admission clones the admitted config and runs `admit_config`
again before use. A compile-fail doc test rejects external construction.

## Evidence

- `cargo check --workspace --all-targets` passed after accessor migration.
- GitHub adapter and hosted test suites passed, including the compile-fail doc
  test.
- Full workspace tests passed.
- Workspace Clippy with `-D warnings`, rustfmt and `cairn validate` passed.
- The admitted-proof audit now records every `Admitted*` proof type as sealed.

## Non-claims

The seal proves external construction and mutation are unavailable; it does not
prove in-crate admission discipline or service-config semantic correctness.
Accessors return borrowed values, so callers may clone them.
