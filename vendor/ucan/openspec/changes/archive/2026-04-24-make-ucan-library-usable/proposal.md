## Why

The repo currently exposes a validated `resource=...` / `ability=...` capability document plus a handful of verified primitives, but it still stops short of the end-to-end UCAN flows that consumers actually need. A caller cannot issue a token, parse a received token, verify a signature against the issuer DID, or walk a proof chain without hand-assembling those steps outside the library.

That makes the crate more like a collection of low-level helpers than a usable UCAN library. Consumers would need to invent their own token format plumbing, error handling, proof traversal, and ergonomic API surface, which defeats the point of depending on `ucan`.

## What Changes

- Add a first-class token model and deterministic structural validation boundary so `ucan-core` can represent real UCAN claims instead of only a two-field capability document.
- Add a consumer-facing `ucan` API for building, signing, parsing, and verifying tokens through stable entry points instead of exposing only raw primitives.
- Add proof-chain verification that composes issuer/audience linkage, temporal narrowing, and capability attenuation through the existing `verified-logic` bridge.
- Add runnable docs/examples so a fresh consumer can issue and verify a token without reading internal modules.

## Non-Goals

- Support every DID method or signing algorithm in the first usable release
- Add revocation registries, persistence layers, or network proof fetching
- Implement invocation-specific or caveat-heavy UCAN features beyond the minimum delegation flow needed for a usable library
- Reimplement verified UCAN semantics in `ucan`; semantic checks should keep flowing through `vendor/verified-logic`

## Capabilities

### New Capabilities
- `ucan-token-round-trip`: Consumers can issue, serialize, parse, and verify tokens through the public library API.
- `ucan-delegation-chain-verification`: Consumers can validate proof chains and receive the effective delegation result.
- `ucan-consumer-onboarding`: The repo documents the supported end-to-end flow with runnable examples.

### Modified Capabilities
- `ucan-core-structural-validation`: The no-std core grows from validating standalone capability documents to validating real token claim structures.
- `verified-logic-integration`: The std shell uses verified primitives as the semantic engine behind the high-level token API instead of exposing them mostly in isolation.

## Impact

- **Files**: `crates/ucan-core/**`, `src/**`, tests/examples, `README.md`, and OpenSpec specs/tasks/design artifacts
- **APIs**: Adds first-class token issuance, parsing, verification, and delegation-chain APIs; may deprecate or narrow helper-only surfaces that are no longer the primary entry point
- **Dependencies**: May add codec or token-serialization dependencies that preserve the `no_std` / `std` boundary
- **Testing**: Needs positive and negative unit/integration coverage for token round-trips, proof traversal, signature failures, malformed tokens, and expired/over-broad delegations
