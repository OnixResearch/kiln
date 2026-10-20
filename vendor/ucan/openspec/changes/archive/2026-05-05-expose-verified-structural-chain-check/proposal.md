## Why

The shell now exposes narrow verified UCAN bridge helpers for proof links, proof depth, revocation sets, concrete invocation admission, and receipt execution. Vendored `verified-logic` also contains a composed structural chain checker, but callers cannot currently exercise that verified composition without depending on the vendored crate internals.

## What Changes

- Add a public shell bridge for caller-supplied resolved chain entries.
- Route the bridge through `verified_logic::ucan_chain::check_chain_structural_valid`.
- Document that the bridge is structural-only and does not replace shell proof resolution, cryptographic verification, revocation, caveat policy, wildcard containment, or production security checks.

## Impact

- Files: `src/verified.rs`, `src/lib.rs`, docs, README, OpenSpec, bridge guard.
- APIs: new validated `VerifiedChainEntry` input type and structural chain bridge helper.
- Verification: focused bridge tests plus the standard UCAN gates.
