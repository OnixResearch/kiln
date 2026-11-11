## Decisions

The guard is keyed by canonical work key plus exact `work_identity`, so a new
source revision (which has a different identity) can still be claimed while the
verified version stays terminal. The error is handled like other ineligible
claims by `claim_next`, allowing independent work to be selected.

## Non-claims

A refusal does not publish, land or close the source. It only prevents duplicate
implementation attempts for the verified version.
