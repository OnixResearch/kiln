## Why

Revocation backends are caller-injected and external, but fail-closed revocation checking can be tightened with a finite transcript predicate over each queried proof reference and backend result.

## What Changes

- Add a vendored revocation-check transcript primitive for checked reference, success, revoked/not-revoked, backend error, and first-failure facts.
- Route live revocation-check facts through a UCAN bridge before treating a token/proof as non-revoked.
- Keep registry truth, freshness, external authority, and backend availability outside the verified claim.

## Impact

- **Files**: `vendor/verified-logic`, `src/verified.rs`, revocation checks in `src/token.rs`, docs, guard, tests, and this OpenSpec package.
- **APIs**: add a revocation-check transcript bridge.
- **Testing**: focused sibling tests/Verus where a new primitive is added, UCAN bridge/integration tests, docs/guard updates, OpenSpec validation, workspace checks, and Nix flake check.
