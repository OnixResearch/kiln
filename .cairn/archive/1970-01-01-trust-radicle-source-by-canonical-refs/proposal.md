## Why

Issue #19: Kiln accepts Radicle source only when a single pinned signer node's
signed refs name the requested revision. In Radicle a branch is canonical when a
threshold of the repository's delegates agree on it; with several delegates and a
threshold above one, a single-node check can accept a revision the delegates have
not agreed on, or reject one they have. The vendored heartwood canonical module
makes the correct computation available.

## What Changes

- Add a `canonical` mode to the Radicle source request, beside the unchanged
  single-signer mode.
- In canonical mode: read the reviewed identity document revision, verify every
  delegate's namespaced signed refs with the existing signature/RID/parent/namespace
  checks, require each observing delegate's signed refs to embed identical
  identity-document bytes, compute the canonical head with heartwood's own
  implementation for the configured reference, and accept the requested revision
  only when it equals that head.
- Extend the source receipt with optional canonical evidence: identity commit and
  root, threshold, delegate DIDs, reference, canonical object and each delegate's
  verified observation.
- Keep single-signer requests, observations and receipts byte-identical: the new
  fields are optional and omitted when unused.
- Add the contract admission rules, Nickel contract, positive and negative
  fixtures, and native 2-of-3, insufficient-vote and diverging-tip tests.

## Impact

- **Files**: `crates/kiln-radicle-source-contract` (model, admission),
  `crates/kiln-adapter-radicle/src/source` (canonical module, profile, protocol
  visibility, shell), `crates/kiln-adapter-radicle/tests/source_canonical_native.rs`,
  `contracts/radicle-source-contract.ncl`,
  `fixtures/radicle-source-contract/{canonical-base,positive/canonical-receipt,negative/canonical-*}.ncl`,
  `flake.nix`, `docs/radicle-source-contract.md`, `Cargo.toml`/`Cargo.lock`.
- **Testing**: contract unit tests, adapter unit and native fixture tests
  (`source-native-fixtures`), Nickel export checks, clippy, formatting, and the
  workspace check. Single-signer fixture bytes are compared by the existing tests.
