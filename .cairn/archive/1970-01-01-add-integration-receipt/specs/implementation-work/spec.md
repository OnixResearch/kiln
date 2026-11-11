# Implementation Work Delta Specification

## ADDED Requirements

### Requirement: Verified integration receipt

r[kiln.implementation.integration] Kiln MUST record integration only from an
independently read-back merge of the exact published PR: exact work/version,
attempt/fence, handoff, verification, candidate, PR identity, merge commit, base
ref and base head. The publication record MUST be at `pull-request-published`.
The receipt MUST move it to `integrated`; repeating the equal receipt MUST be
idempotent and a different receipt MUST conflict. Recording integration MUST NOT
close issues, mutate source status or claim release eligibility.

#### Scenario: Exact landed result

r[kiln.implementation.integration.scenario.exact]
- GIVEN a published PR with an independently observed merge commit and base head
- WHEN the exact integration receipt is recorded
- THEN the publication MUST become `integrated` and retain the receipt

#### Scenario: Mismatched integration

r[kiln.implementation.integration.scenario.refused]
- GIVEN a different PR, candidate, base ref, merge commit, attempt or fence
- WHEN integration admission runs
- THEN it MUST fail without changing publication state
