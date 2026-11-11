## Decisions

Integration is a distinct local state after a provider read-back, not a worker
claim or merge request success. The receipt binds the exact publication plan and
observed merge/base identities. The CLI requires the prepared publication and
landing plans to agree on repository, PR number, candidate head and base branch
before calling the landing provider.

The attempt is already verified, so integration does not create a new claim or
resume execution. Repeating an equal receipt is idempotent; a different PR,
candidate or merge/base result conflicts.

## Non-claims

The receipt does not prove atomic base protection, merge-queue semantics,
source-status changes or release eligibility. It does not close issues or run
Cairn lifecycle operations.
