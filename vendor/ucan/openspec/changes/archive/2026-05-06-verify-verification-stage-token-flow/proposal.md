## Why

Verification-stage transcript helpers exist, but live token verification can provide stronger audit evidence by guaranteeing that emitted stage facts follow a fixed flow and first-failure mapping from parse through authorization.

## What Changes

- Add or extend a vendored stage-flow primitive for ordered parse, structural, key, signature, envelope/proof, revocation, and authorization stage facts.
- Route live verification stage facts through a UCAN bridge before returning success or failure.
- Keep the truth of parser, crypto, backend, and authorization facts outside the stage-flow admission claim.

## Impact

- **Files**: `vendor/verified-logic`, `src/verified.rs`, verification flow in `src/token.rs`, docs, guard, tests, and this OpenSpec package.
- **APIs**: add a verification-stage token-flow bridge.
- **Testing**: focused sibling tests/Verus where a new primitive is added, UCAN bridge/integration tests, docs/guard updates, OpenSpec validation, workspace checks, and Nix flake check.
