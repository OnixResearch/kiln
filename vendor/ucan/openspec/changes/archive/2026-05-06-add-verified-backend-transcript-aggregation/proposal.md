## Why

Caller-injected resolver, proof-store, and revocation checker backends remain runtime trust boundaries, but the shell can still verify the finite transcript combiner that decides whether backend stages collectively admit verification or fail closed.

## What Changes

- **1**: Extend backend transcript predicates to cover key resolution, proof loading, revocation checks, and replay admission consistently.
- **2**: Verify first-failure selection and fail-closed mapping into structured errors.
- **3**: Keep backend truth, availability, freshness, and storage/network behavior outside the claim.

## Capabilities

### New Capabilities
- `add-verified-backend-transcript-aggregation`: Specify verified backend transcript aggregation for resolver, proof-store, and revocation stages.

### Modified Capabilities
- `library`: Adds a focused verified-logic follow-up seam while preserving current production-readiness boundaries.

## Impact

- **Files**: Future implementation is expected to touch `src/token.rs`, `src/verified.rs`, `crates/ucan-core`, `vendor/verified-logic`, documentation, tests, and readiness checks as applicable to this slice.
- **APIs**: No consumer-specific adapters are in scope. Public helper additions must remain generic and consumer-agnostic.
- **Dependencies**: Any new proof logic must come through the vendored `verified-logic` snapshot; no new network/storage backend dependency is implied.
- **Testing**: Validate OpenSpec, run focused cargo tests, verified bridge checks, readiness checks, and full `nix flake check` before archive.
