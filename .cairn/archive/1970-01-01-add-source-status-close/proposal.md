# Close integrated GitHub sources

## Why

Issue #47 requires source-status reconciliation to be distinct from candidate
verification, publication and integration. The ledger could record integration
but not close the originating issue.

## What Changes

- Add source-status plan/receipt admission tied to an integrated publication.
- Add an idempotent GitHub issue-close client with read-back.
- Add ledger source-status state and `close-source` operator command.
- Keep issue closure separate from release or Cairn lifecycle completion.

## Impact

- **Files**: implementation publication/attempt, GitHub source-status client,
  implementer shell/CLI, docs and specifications.
- **Testing**: close-after-integration admission, pre-integration refusal,
  open-close-readback and already-closed idempotence.
- **Authority**: one exact issue close. No comments, other edits, release or
  lifecycle effects.
