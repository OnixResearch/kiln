# Patchgate Review Adapter Specification

## ADDED Requirements

### Requirement: Run-bound reviewed worker job

r[kiln.patchgate.worker.job] A reviewed worker step MUST build the review intent
from the exact pending native inbox in its operator-provisioned local Pijul repository, invoke one atomic adapter
`run` operation, and declare the published receipt as a `review` report for the
run. The step MUST fail closed when the inbox is behind main, has no pending
change, native state is unavailable, or receipt publication fails. The step MUST
NOT accept, merge, revert, watch, or otherwise mutate source, and a missing
report MUST remain unavailable rather than become a clean review.

#### Scenario: Pending inbox change is reviewed

- GIVEN a reviewed worker checkout with the job script, an accessible target
  repository with `patchgate.json`, its policy, and one pending inbox change
- WHEN the worker step runs
- THEN it MUST invoke the adapter once, publish one receipt, and declare that
  exact receipt as the run's `review` report.

#### Scenario: No eligible change or broken inputs

- GIVEN an inaccessible target, an inbox behind main, an empty inbox, unavailable
  native state, a failed adapter process, or a rejected output destination
- WHEN the worker step runs
- THEN it MUST exit nonzero and MUST NOT upload an approving or empty report.

#### Scenario: Uploaded receipt is inspected

- GIVEN a run with the declared receipt uploaded
- WHEN the Reviews surface admits it
- THEN the receipt MUST retain its exact request, decisions, and native report
  identity while granting no review, merge, gate, or release authority.
