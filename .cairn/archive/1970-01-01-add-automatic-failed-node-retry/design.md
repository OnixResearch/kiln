## Decisions

The automatic retry reuses the existing `failed` rerun path rather than adding a
new candidate builder. That keeps source binding, failed/downstream selection,
untrusted gate retention and request replay identical to the manual operation.

The retry ordinal is stored on the child (`auto_retry`). The next attempt is
enqueued only when `ordinal < retry_failed`, so an always-failing lineage stops
at the reviewed bound instead of retrying forever. The request identity derives
from the parent run and the next ordinal, and an existing child with the same
parent and ordinal suppresses a duplicate.

Automatic retry runs during `poll`, after observation refresh and queue dispatch,
so a run that becomes terminal during dispatch is still considered. Enqueue
refusals (retention bounds, missing snapshots, unknown nodes, superseded
untrusted state) are not poll failures: the parent keeps its failed outcome and
the observation loop continues.

The child is a partial-node run by construction (`selected_nodes` is non-empty).
The existing required-CI selector requires `full_run && clean`, so a passing
child cannot satisfy required CI. The run view exposes direct retry children and
the UI labels a failed parent with a passing child as "failed, then passed on
retry" without changing the parent badge.

## Non-claims

Automatic retry records observations and enqueues work. It does not quarantine
tests, retry cancelled or unknown runs, change manual rerun semantics, prove a
test is flaky in general, or grant CI, gate, merge or release authority. The
flaky view continues to report pass/fail counts for the same exact test
identity, with no new statistical claim.
