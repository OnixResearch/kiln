## Why

`ucan-core` owns deterministic no_std claim and shape validation, but those validators are not themselves machine-checked here. Mirroring the finite validators as verified-logic predicates can make the shell/core boundary more auditable while keeping I/O and runtime parsing outside the proof.

## What Changes

- **1**: Add verified admission predicates for DID shape, claim required fields, list bounds, proof-reference bounds, and temporal order.
- **2**: Route ucan-core structural validation decisions through those predicates or parity tests.
- **3**: Preserve no_std and parser/runtime boundaries.

## Capabilities

### New Capabilities
- `add-verified-core-structural-validators`: Specify verified mirrors for no_std ucan-core structural validators.

### Modified Capabilities
- `library`: Adds a focused verified-logic follow-up seam while preserving current production-readiness boundaries.

## Impact

- **Files**: Future implementation is expected to touch `src/token.rs`, `src/verified.rs`, `crates/ucan-core`, `vendor/verified-logic`, documentation, tests, and readiness checks as applicable to this slice.
- **APIs**: No consumer-specific adapters are in scope. Public helper additions must remain generic and consumer-agnostic.
- **Dependencies**: Any new proof logic must come through the vendored `verified-logic` snapshot; no new network/storage backend dependency is implied.
- **Testing**: Validate OpenSpec, run focused cargo tests, verified bridge checks, readiness checks, and full `nix flake check` before archive.
