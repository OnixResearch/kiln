## Why

Ability containment already routes through verified logic, but small helper predicates used by shell/adapters for wildcard/concrete classification can be moved into verified-logic to reduce duplicated edge-case handling for Aspen and Clankers consumers.

## What Changes

- **1**: Add verified helpers for wildcard segment detection and concrete ability classification.
- **2**: Use them from public validated wrappers and adapter-facing helpers.
- **3**: Keep consumer policy mapping outside ucan.

## Capabilities

### New Capabilities
- `add-verified-ability-helper-predicates`: Specify verified ability/capability helper predicates for wildcard and concrete ability classification.

### Modified Capabilities
- `library`: Adds a focused verified-logic follow-up seam while preserving current production-readiness boundaries.

## Impact

- **Files**: Future implementation is expected to touch `src/token.rs`, `src/verified.rs`, `crates/ucan-core`, `vendor/verified-logic`, documentation, tests, and readiness checks as applicable to this slice.
- **APIs**: No consumer-specific adapters are in scope. Public helper additions must remain generic and consumer-agnostic.
- **Dependencies**: Any new proof logic must come through the vendored `verified-logic` snapshot; no new network/storage backend dependency is implied.
- **Testing**: Validate OpenSpec, run focused cargo tests, verified bridge checks, readiness checks, and full `nix flake check` before archive.
