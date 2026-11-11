# Implementation Work Specification

## ADDED Requirements

### Requirement: Bounded GitHub issue polling recovery

r[kiln.implementation.polling] Polling recovery MUST use a bounded provider port
and MUST normalize polled issues into the same versioned work contract as
webhook intake, including enrollment, text bounds, exact content-bound source
identities, closed observations and deterministic ordering by update sequence and
issue number. Duplicate records across pages MUST keep the highest sequence.
Exceeding the page or record bound with more records remaining MUST fail rather
than silently truncating recovery. Polling MUST NOT create execution authority or
bypass ledger admission.

#### Scenario: Pages normalize deterministically

r[kiln.implementation.polling.scenario.pages]
- GIVEN two pages containing open and closed issues
- WHEN polling recovery runs
- THEN each issue MUST produce one work or closed observation
- AND observations MUST be ordered by update sequence and number

#### Scenario: Boundaries fail closed

r[kiln.implementation.polling.scenario.bound]
- GIVEN an unenrolled actor, an oversized record, or more pages than the reviewed bound
- WHEN polling recovery runs
- THEN it MUST fail without returning a partial recovery set
