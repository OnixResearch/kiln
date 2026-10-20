## Phase 1: Specification

- [x] Create focused OpenSpec package for compact-token framing bridge scope.

## Phase 2: Implementation

- [x] Sync vendored verified-logic compact framing primitive into UCAN.
- [x] Add UCAN verified bridge API and tests for accepted/rejected compact/signing-input framing.
- [x] Route parse-time compact token framing through the verified bridge without moving codec/hash/crypto boundaries.
- [x] Update public docs and bridge guards for the new compact framing bridge.

## Phase 3: Verification

- [x] Run Rust, OpenSpec, bridge/no-std, whitespace, and Nix validation gates.
- [x] Archive this OpenSpec change after validation and commit/push the completed slice.
