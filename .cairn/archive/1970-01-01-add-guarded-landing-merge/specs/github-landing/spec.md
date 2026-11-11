# GitHub Landing Delta Specification

## ADDED Requirements

### Requirement: Guarded merge-commit landing

r[kiln.github.landing.merge] Kiln MAY land one admitted landing observation with
a merge-commit request bound to the exact PR head SHA. It MUST first admit the
full PR/check/review/base observation. After the request it MUST read the PR and
target base ref back and return a receipt only when the PR is merged at the exact
head/base and the base head equals the merge commit. A timeout or ambiguous
response MUST reconcile by read-back; an unobservable merge commit MUST remain
unknown. The route MUST NOT close issues, mutate source status or claim atomic
base protection or merge-queue qualification.

#### Scenario: Merge lands and reads back

r[kiln.github.landing.merge.scenario.landed]
- GIVEN an exact ready observation
- WHEN the merge request succeeds
- THEN the receipt MUST bind the candidate head, merge commit, base and base head

#### Scenario: Merge does not land

r[kiln.github.landing.merge.scenario.refused]
- GIVEN a refused merge or no observable merged state
- WHEN landing reconciliation runs
- THEN it MUST fail or remain unknown without claiming a landed result
