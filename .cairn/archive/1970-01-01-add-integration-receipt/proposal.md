# Add verified integration receipts

## Why

Issue #47 requires verified-integrated and source-status reconciliation outcomes
to remain distinct. The ledger could publish a PR but not record independently
observed landing.

## What Changes

- Add `IntegrationReceipt` binding the published PR, candidate, merge commit,
  base ref/head, work/attempt/fence and handoff/verification.
- Add `integrated` publication phase and idempotent ledger recording.
- Add `land` CLI that matches the prepared publication, performs the guarded
  merge, reads back, and records the receipt.
- Keep source issue/Cairn status separate.

## Impact

- **Files**: implementation publication/attempt, adapter landing, implementer
  shell/CLI, docs and specification.
- **Testing**: exact integration admission, mismatch refusal and grounded landing
  read-back tests.
- **Authority**: one guarded merge and local receipt. No source-status, release
  or lifecycle mutation.
