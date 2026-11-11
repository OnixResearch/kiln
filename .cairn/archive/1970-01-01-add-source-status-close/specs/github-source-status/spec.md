# GitHub Source Status Delta Specification

## ADDED Requirements

### Requirement: Idempotent GitHub source closure

r[kiln.github.source_status] The GitHub source-status client MUST close only the
exact admitted issue identity and only when it is open. An already closed issue
MUST be returned idempotently. After a PATCH close the client MUST read the issue
back and return `closed` only when observed. It MUST NOT comment, reopen, edit
other fields, mutate pull requests or follow redirects/proxies.

#### Scenario: Open source is closed

r[kiln.github.source_status.scenario.close]
- GIVEN an open exact source issue
- WHEN closure runs
- THEN one close mutation MUST be followed by a closed read-back

#### Scenario: Closed source is reused

r[kiln.github.source_status.scenario.closed]
- GIVEN an already closed source issue
- WHEN closure runs
- THEN no mutation MUST occur and the closed state MUST be returned
