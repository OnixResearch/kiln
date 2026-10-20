## Why

The vendored verified-logic snapshot now includes a composed UCAN invocation-chain authorization checker. The `ucan` shell already exposes structural-chain, non-revoked-chain, and concrete-invocation bridge helpers separately, but advanced callers cannot yet ask one data-only helper to validate a resolved chain, aligned references, revocation set, leaf authorization CID binding, leaf capability index, invoker/audience match, resource refinement, and ability equality using the single verified composition.

## What Changes

- Add a public helper that wraps `verified_logic::ucan_invocation::check_invocation_chain_authorized`.
- Accept caller-supplied resolved public chain entries, aligned proof-reference bytes, finite revoked-reference bytes, the invocation authorization reference, invoker DID, request resource/ability, leaf capability index, and deterministic time inputs.
- Document that this helper is advanced/data-only and does not replace production verification, caveat policy, wildcard containment, proof resolution, cryptography, revocation backend/freshness checks, networking, persistence, or ambient clocks.

## Capabilities

### New Capabilities
- `library.verified-bridges.invocation-chain`: Advanced callers can compose resolved chain authorization through the vendored verified invocation-chain checker.

### Modified Capabilities
- `library.public-api`: Public verified bridge docs and re-exports include the invocation-chain helper.

## Impact

- **Files**: `src/verified.rs`, `src/lib.rs`, `README.md`, `docs/public-api.md`, `scripts/check-ucan-bridge.sh`, and OpenSpec library specs.
- **APIs**: Adds a public verified bridge helper.
- **Dependencies**: No new dependencies.
- **Testing**: Focused bridge unit test, existing bridge guard, cargo fmt/test/clippy, no-std guard, OpenSpec validation, diff check, and Nix flake check.
