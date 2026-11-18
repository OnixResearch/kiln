# GitHub source status delta

## MODIFIED Requirements

### Requirement: Idempotent GitHub source closure

r[kiln.github.source_status] The GitHub source-status client MUST read only the
exact admitted issue identity and MUST NOT send a close mutation: GitHub's issue
update has no qualified source-version guard, so a close could act on an issue
that was edited or reopened after admission. An issue already observed closed
MUST be returned idempotently, and an open issue MUST be refused for operator
reconciliation. The client MUST NOT comment, reopen, edit other fields, mutate
pull requests or follow redirects or proxies.

#### Scenario: An open source is left for reconciliation

r[kiln.github.source_status.scenario.open]
- GIVEN an open exact source issue
- WHEN closure runs
- THEN it MUST be refused after one read, and no request MUST mutate the issue

#### Scenario: Closed source is reused

r[kiln.github.source_status.scenario.closed]
- GIVEN an already closed source issue
- WHEN closure runs
- THEN no mutation MUST occur and the closed state MUST be returned
