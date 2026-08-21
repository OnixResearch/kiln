# Mutate Radicle COBs with single-writer authority

## Why

Kiln can observe Radicle patches and issues, but it cannot act on them. Review decisions and CI-driven patch updates stay manual.

Collaboration mutation is the highest-authority source operation. It needs preview-first plans, fresh expected heads, one explicitly resolved signer, and strict rejection of everything else.

This change builds on the SecretSpec-resolved signer and cohort pins from `publish-radicle-ci-status`.

## What Changes

- Add a typed Nickel collaboration profile with RID and object-type allowlists, declared target identities, bounds, timeouts, and non-claims.
- Implement the concrete mutation transport over the pinned `radicle-cob` cohort.
- Require a fresh expected-head preflight immediately before signing; reject any moved head.
- Keep single-writer expected-head authority: Kiln's resolved signer must have authored every prior head.
- Preserve preview before dispatch, explicit denial without a signer, and unknown outcomes after ambiguous writes.
- Add native positive and negative fixtures created by the pinned cohort.
- Keep merge authority, peer trust, history rewriting, and issue or patch creation outside this adapter.

## Dependencies

- Depends on `publish-radicle-ci-status` for the cohort pin, SecretSigner resolution shell, profile scaffolding, and conformance patterns.
- Uses the accepted `source-collaboration` contracts from `establish-neutral-ci-control-plane`.

## Impact

- **Adapters**: adds a bounded collaboration mutation transport to `kiln-adapter-radicle`.
- **Configuration**: adds a typed collaboration profile with declared targets and signer policy.
- **Testing**: adds stale-head, ambient-signer, foreign-target, replay, timeout, disconnect, and reconciliation fixtures.
- **Documentation**: records authority flow, recovery, limits, and non-claims.

## Out of Scope

- Do not create, close, or merge patches; do not rewrite history.
- Do not claim actor trust or peer convergence.
- Do not widen beyond single-writer policy in this change.
