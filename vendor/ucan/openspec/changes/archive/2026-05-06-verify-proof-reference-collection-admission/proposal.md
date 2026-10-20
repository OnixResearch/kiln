## Why

A parsed `prf` field can be present and list-shaped while the decoded proof-reference collection semantics remain shell-local. The closeable fact is a finite collection admission predicate over already decoded proof-reference byte strings: non-empty entries, bounded count, and an explicit duplicate policy before traversal.

## What Changes

- Add a vendored verified collection-admission primitive over decoded proof-reference bytes.
- Route decoded proof-reference collection facts through a UCAN bridge before proof traversal.
- Keep hash derivation, proof-store truth, backend provenance, and proof semantic correctness outside the verified claim.

## Impact

- **Files**: `vendor/verified-logic`, `src/verified.rs`, proof-reference validation in `src/token.rs`, docs, guard, tests, and this OpenSpec package.
- **APIs**: add a decoded proof-reference collection admission bridge.
- **Testing**: focused sibling tests/Verus where a new primitive is added, UCAN bridge/integration tests, docs/guard updates, OpenSpec validation, workspace checks, and Nix flake check.
