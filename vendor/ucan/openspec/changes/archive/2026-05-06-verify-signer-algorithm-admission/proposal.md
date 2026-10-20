## Why

Signer-based issuance still checks the caller-supplied signer algorithm with a shell-local string equality against `EdDSA`. The same exact supported algorithm byte value is already modeled in vendored `verified_logic::ucan_header` for parsed token headers. Routing signer algorithm admission through a verified helper removes one more deterministic adapter fact from the UCAN shell without claiming signer truth or cryptographic correctness.

## What Changes

- Add a sibling `verified_logic::ucan_header` executable helper for exact supported-algorithm byte admission.
- Sync the vendored snapshot into UCAN.
- Route `validate_signer_algorithm` through a UCAN bridge helper before signer-based issuance accepts a signer.
- Update docs and bridge guards to describe the narrow non-cryptographic boundary.

## Capabilities

### Modified Capabilities
- `library`: signer algorithm admission becomes a verified byte-equality bridge for exact `EdDSA` bytes.

## Impact

- **Files**: `verified-logic/src/ucan_header.rs`, vendored snapshot, `src/verified.rs`, `src/token.rs`, docs, bridge guard, OpenSpec archive.
- **APIs**: adds a public helper for supported signer algorithm admission.
- **Dependencies**: no new dependencies.
- **Testing**: sibling header tests/Verus, UCAN targeted and full gates, OpenSpec validation, flake check.
