# Verification evidence

## Result

Kiln now admits the exact Nix derivation before dispatch, records typed build
constraints, product NAR/reference facts and input-closure facts, and renders a
bounded static Nix cache projection. Snix semantics are adapted through the
Apache-2.0 `nix-derivation`, `nix-archive` and `nix-narinfo` crates; no Snix
GPL-3.0 crate is linked into Kiln.

## Evidence

- `cargo test -p kiln-nix-store` (9 passed) passes derivation admission (valid, platform,
  output-name, output-path and malformed negatives), NAR hashing/reference
  scanning/completeness, input closure, cache render/reparse and cache bounds.
- `cargo test -p kiln-nix-build --test composition` (7 passed) passes admitted derivation
  outputs/constraints, pre-process mismatch rejection, preflight failure,
  durable state and rooted-build composition.
- `cargo test -p kiln-adapter-nix` (15 passed) passes direct and composition build paths and
  the exact NAR/fact attachment test.
- `cargo test -p kiln-adapter-mantle` (10 passed) passes native report admission and
  product NAR fact attachment.
- `cargo test -p kiln-core` passes additive compatibility and constraint/store/
  input admission tests; absent new fields keep their previous serialized bytes.
- `kiln-nix-cache render` was exercised end to end on a disposable store and
  emitted `nix-cache-info`, a canonical `.narinfo` and the copied NAR bytes with
  a matching receipt.
- `cargo test --workspace --no-fail-fast` passes all workspace binaries and doctests.
- Workspace Clippy, rustfmt and Cairn validation are recorded below.

## Non-claims

Derivation admission is structural and task-bound where input-modulo hashes are
unavailable. NAR facts prove observed bytes and references, not build
correctness, hermeticity, reproducibility, cache truth or release readiness.
The cache projection is static; it does not serve HTTP, sign with operator keys,
substitute or mutate a store. Reference completeness is explicit and false when
unresolved input-derivation outputs are absent from the candidate set.
