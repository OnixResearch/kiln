## Why

Proof-store backends are caller-injected and cannot be proven honest by this crate, but the shell can verify the finite transcript shape around each proof-store request and response before using returned proof material.

## What Changes

- Add a vendored proof-store result transcript primitive for requested reference, success/failure, returned compact-token presence, and first-failure ordering.
- Route live proof-store result facts through a UCAN bridge before returned proof material participates in traversal.
- Keep backend truth, freshness, availability, storage/network behavior, and returned proof semantic correctness outside the verified claim.

## Impact

- **Files**: `vendor/verified-logic`, `src/verified.rs`, proof traversal/resolution code in `src/token.rs`, docs, guard, tests, and this OpenSpec package.
- **APIs**: add a proof-store result transcript bridge.
- **Testing**: focused sibling tests/Verus where a new primitive is added, UCAN bridge/integration tests, docs/guard updates, OpenSpec validation, workspace checks, and Nix flake check.
