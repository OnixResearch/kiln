## Why

Capability attenuation already routes resource and ability containment through vendored verified primitives, but the shell still preserves opaque caveats with a local nested equality scan. That check is finite, exact, and high-ROI: every parent caveat identifier must be present in the child before a delegated capability is accepted. Routing the already-serialized caveat identifiers through verified logic removes another shell-local semantic branch while keeping caveat payload parsing and identifier construction provenance explicit runtime boundaries.

## What Changes

- Add a vendored verified-logic predicate for finite parent-caveat identifier preservation.
- Expose a UCAN bridge for parent/child caveat identifier sets.
- Route the shell attenuation check through that bridge after constructing the existing length-prefixed caveat identifiers.
- Document that this verifies identifier set preservation only, not opaque caveat semantics, payload parsing, identifier construction injectivity, or backend provenance.

## Impact

- Files: `verified-logic/src`, `vendor/verified-logic`, `src/verified.rs`, `src/token.rs`, docs, bridge guard script.
- APIs: new public bridge type/function for caveat-preservation admission.
- Testing: sibling Rust/Verus checks, UCAN focused tests, workspace tests, clippy, OpenSpec validation, bridge guards, Nix flake check.
