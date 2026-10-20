## Phase 1: Revocation model

- [x] Add `RevocationChecker`, `RevocationError`, `NoRevocations`, and `RevocationList` public types keyed by `ProofReference`. [covers=library.revocation.explicit-checker]
- [x] Add structured `TokenError` variants for revoked tokens and revocation backend errors. [covers=library.revocation.explicit-checker]

## Phase 2: Verification integration

- [x] Add revocation-aware verification entry points and route existing verification through a no-op checker for backwards compatibility. [covers=library.revocation.explicit-checker]
- [x] Ensure leaf tokens and every recursively resolved proof token are checked before verification can succeed. [covers=library.revocation.chain-denial]

## Phase 3: Tests, docs, validation

- [x] Add tests for no-revocation compatibility, leaf revocation denial, parent proof revocation denial, and backend error propagation. [covers=library.revocation.explicit-checker library.revocation.chain-denial]
- [x] Update README/docs/examples to describe explicit revocation support and remaining non-goals. [covers=library.consumer-onboarding]
- [x] Run fmt, targeted/all-target Cargo tests, clippy, no-std/bridge guards, strict OpenSpec validation, `git diff --check`, and `nix flake check`. [covers=library.revocation.explicit-checker library.revocation.chain-denial]
