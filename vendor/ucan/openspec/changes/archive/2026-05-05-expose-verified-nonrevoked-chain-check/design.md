## Context

`VerifiedChainEntry` and `structural_chain_is_valid` bridge into the vendored structural chain checker. `revocation_reference_is_listed` bridges one finite reference lookup. The vendored snapshot also provides `check_chain_structural_valid_not_revoked`, which composes structural chain validity, one-to-one chain/reference alignment, and finite revocation-set non-membership.

## Design

Add `structural_chain_not_revoked_is_valid(entries, chain_references, revoked_references, now_seconds, tolerance_seconds) -> VerifiedResult<bool>` in `src/verified.rs`.

Implementation builds the same `ucan_chain::ChainEntry` values as the structural bridge, wraps `revoked_references` as `ucan_revocation::RevocationSet`, copies caller-supplied `chain_references`, and calls the verified composed checker.

## Boundaries

The helper is intentionally data-only. It consumes caller-resolved public fields and caller-supplied signature-valid flags/reference bytes. It does not resolve proofs, derive proof references from compact tokens, check revocation backend availability, enforce revocation freshness, evaluate caveats, perform cryptography, access registries, or read the clock.

The production verification path remains `verify_compact_token_with_resolvers_and_revocations`; backend errors still fail closed there. This helper exposes the finite verified composition for advanced callers/tests.

## Risks

- Misuse as a production revocation backend replacement: mitigated by naming/docs and explicit non-goals.
- Canonical OpenSpec archive clobber/EOF whitespace: inspect `openspec/specs/library/spec.md`, normalize EOF, and rerun strict validation plus `git diff --check`.
