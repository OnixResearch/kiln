# Add guarded merge-commit landing

## Why

Issue #48 requires an actual landing provider that binds exact candidate, checks,
approvals and base evidence, reconciles lost responses and independently reads
back the landed result. The repository had observation only.

## What Changes

- Add a merge-commit POST bound to the exact PR head.
- Re-admit the full observation before the effect.
- Read back the merged PR and target base ref; return a receipt only when the
  merge commit is observed at the expected base.
- Reconcile timeouts/ambiguity by read-back; keep unobservable results unknown.
- Explicitly avoid source-status mutation and atomic base/merge-queue claims.

## Impact

- **Files**: `crates/kiln-adapter-github/src/landing.rs`, docs and specification.
- **Testing**: successful merge/read-back and refused/unlanded reconciliation.
- **Authority**: one guarded merge-commit effect only. No issue close, approval,
  source-status, merge-queue or atomic-base claim.
