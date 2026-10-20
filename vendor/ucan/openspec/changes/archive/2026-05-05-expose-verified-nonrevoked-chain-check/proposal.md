## Why

The std shell already exposes a structural chain bridge and finite revocation-set helpers, but there is no single public bridge for the vendored verified primitive that composes structural chain validity with caller-supplied proof-reference non-revocation.

## What Changes

- Add a focused public bridge helper around `verified_logic::ucan_revocation::check_chain_structural_valid_not_revoked`.
- Accept only resolved public chain entries, aligned caller-supplied proof-reference bytes, and finite caller-supplied revoked-reference bytes.
- Add tests, docs, and guard coverage.

## Non-Goals

- No built-in revocation registry, backend, network/database/filesystem lookup, replay store, persistence, proof resolution, DID/key resolution, cryptographic signature verification, caveat policy, or ambient clock reads.
- No replacement for the shell's fail-closed caller-injected `RevocationChecker` verification path.

## Verification

Run focused bridge tests, bridge/no-std guards, strict OpenSpec validation, full Cargo fmt/test/clippy gates, `git diff --check`, and Nix flake checks.
