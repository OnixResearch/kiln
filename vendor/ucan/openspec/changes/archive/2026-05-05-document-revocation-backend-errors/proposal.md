## Why

Production revocation checks often depend on caller-owned stores or services that can be temporarily unavailable. Consumers need runnable guidance that these backend failures fail closed as structured errors, not as implicit allow decisions.

## What Changes

- Extend the revocation example with a deterministic failing `RevocationChecker`.
- Update docs to call out backend-failure handling in the revocation onboarding path.

## Impact

- **Files**: `examples/revocation.rs`, README, public API docs, library OpenSpec.
- **APIs**: No new API surface.
- **Testing**: Run the revocation example and standard Rust/OpenSpec/Nix gates.
