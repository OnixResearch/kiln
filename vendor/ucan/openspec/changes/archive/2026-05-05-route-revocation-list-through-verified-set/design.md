## Context

Revocation support is intentionally shell-side and caller-injected. `RevocationList` is a deterministic in-memory helper, not a registry or persistence layer. Its lookup can still be grounded in the verified finite revocation-set primitive because both operate over caller-provided proof-reference bytes.

## Decision

Add `revocation_reference_is_listed(revoked_references, reference)` in `src/verified.rs`. The helper constructs `verified_logic::ucan_revocation::RevocationSet { entries }` and calls `ucan_revocation::is_revoked`. `RevocationList::is_revoked` will convert its private `ProofReference` values to byte vectors and delegate to the bridge.

## Non-Goals

- No revocation registry, freshness protocol, network/database/filesystem lookup, replay store, or process-global checker.
- No change to `RevocationChecker` backend error handling; caller backends still fail closed.
- No claim about cryptographic proof-reference derivation beyond existing BLAKE3 shell behavior.

## Risks

The bridge allocates a temporary vector for the in-memory helper. That is acceptable for std-shell verification and preserves the no-std core boundary.
