# Reconcile landing after base advancement

## Why

Landing recovery required base head equality with the merge commit. Later base
merges make that equality stale and leave a previously landed result
unrecoverable.

## What Changes

- Read the current base head.
- If it differs from the merge commit, use GitHub compare to require an
  ahead/identical relationship with the merge commit as merge base.
- Keep divergent/unrelated bases unknown.
- Add docs and fixture coverage.

## Impact

- **Files**: `crates/kiln-adapter-github/src/landing.rs`, docs and specification.
- **Testing**: base-advanced recovery and unchanged no-second-merge recovery.
- **Authority**: read-only ancestry evidence only; no merge, queue or base-atomicity claim.
