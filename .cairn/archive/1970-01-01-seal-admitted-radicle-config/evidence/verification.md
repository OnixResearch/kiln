# Verification evidence

## Result

`AdmittedRadicleConfig` fields are crate-private. External consumers read the
reviewed shell, Lattice and transport profiles through accessors; the Radicle
binary and Onix pilot compile against that boundary. A compile-fail doc test
records that external struct construction is rejected.

## Evidence

- `cargo check -p kiln-adapter-radicle -p kiln-onixos-pilot --all-targets`
  passed.
- `cargo check --workspace --all-targets` passed.
- Radicle, pilot and dashboard test suites passed, including the compile-fail
  doc test; the full workspace suite also passed.
- `cargo clippy --workspace --all-targets -- -D warnings`, rustfmt and
  `cairn validate` passed.

## Non-claims

The seal proves external construction and mutation are unavailable; it does not
prove in-crate admission discipline or profile semantic correctness. Accessors
expose borrowed profiles, so callers may clone values.
