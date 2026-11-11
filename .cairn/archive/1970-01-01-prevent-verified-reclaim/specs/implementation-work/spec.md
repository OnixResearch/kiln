# Implementation Work Delta Specification

## ADDED Requirements

### Requirement: Verified work version is not reclaimable

r[kiln.implementation.verified_claim] A work identity/revision with a retained
`verified` attempt MUST NOT be claimed again, even when the reviewed attempt
budget has remaining attempts. A different source version MUST remain separately
claimable under canonical-key supersession rules. A refused reclaim MUST NOT
mutate the verified attempt or claim state.

#### Scenario: Verified work is refused

r[kiln.implementation.verified_claim.scenario.refused]
- GIVEN work with a verified attempt and remaining budget
- WHEN another claim is attempted for the same work version
- THEN it MUST fail without a new attempt
