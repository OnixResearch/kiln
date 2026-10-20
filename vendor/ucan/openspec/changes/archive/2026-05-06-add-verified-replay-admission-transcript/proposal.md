## Why

Replay admission is now a generic caller-supplied hook, but the shell currently treats the replay decision as ordinary Rust orchestration. A narrow verified transcript can prove the finite decision shape without claiming durable nonce-store or backend truth.

## What Changes

- **1**: Add a vendored verified-logic replay admission transcript predicate for duplicate, stale, malformed, denied, and backend outcomes.
- **2**: Route `verify_invocation_with_replay` through the verified transcript before returning replay-related invocation denials.
- **3**: Keep storage freshness, crash recovery, request identity provenance, and consumer envelope semantics outside the verified claim.

## Capabilities

### New Capabilities
- `add-verified-replay-admission-transcript`: Specify verified replay-admission transcript and fail-closed invocation outcome semantics.

### Modified Capabilities
- `library`: Adds a focused verified-logic follow-up seam while preserving current production-readiness boundaries.

## Impact

- **Files**: Future implementation is expected to touch `src/token.rs`, `src/verified.rs`, `crates/ucan-core`, `vendor/verified-logic`, documentation, tests, and readiness checks as applicable to this slice.
- **APIs**: No consumer-specific adapters are in scope. Public helper additions must remain generic and consumer-agnostic.
- **Dependencies**: Any new proof logic must come through the vendored `verified-logic` snapshot; no new network/storage backend dependency is implied.
- **Testing**: Validate OpenSpec, run focused cargo tests, verified bridge checks, readiness checks, and full `nix flake check` before archive.
