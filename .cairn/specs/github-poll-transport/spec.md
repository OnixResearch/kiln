# GitHub Poll Transport Specification

## Purpose

Defines the `github-poll-transport` capability.

## Requirements

### Requirement: Authenticated bounded GitHub issue polling transport

r[kiln.github.poll.transport] Kiln MUST perform GitHub issue polling only against
the fixed API origin with an admitted bearer token, no redirect following, no
environment proxy, bounded connect/read deadlines and bounded response bytes.
The transport MUST reject malformed owner/repository/page inputs, filter pull
requests out of the issues feed, validate GitHub timestamps and actor/state
fields, and derive at most one strictly advancing next page from a bounded
`Link` header. Authentication, authorization, not-found and rate-limit responses
MUST remain distinct from a successful empty page. The transport MUST NOT
retry, schedule, admit or dispatch; normalization and deduplication remain in
`poll_implementation_issues` and the work ledger.

#### Scenario: Paged issues are fetched

r[kiln.github.poll.transport.scenario.pages]
- GIVEN an authenticated repository with an issue and a pull request
- WHEN one page is fetched
- THEN the issue MUST be returned, the pull request MUST be filtered
- AND the exact next page MUST be derived from the Link header

#### Scenario: Invalid credentials or payload fail

r[kiln.github.poll.transport.scenario.refused]
- GIVEN a refused token or a malformed response body
- WHEN polling runs
- THEN it MUST fail without returning a page

#### Scenario: Page and response bounds hold

r[kiln.github.poll.transport.scenario.bounds]
- GIVEN a non-advancing, malformed or oversized Link header
- WHEN polling parses pagination
- THEN it MUST fail rather than loop or truncate silently

### Requirement: Bounded polling admission pass

r[kiln.github.poll.admission] The operator shell MAY run one explicit bounded
polling pass that normalizes through `poll_implementation_issues` and admits the
results into the durable implementation ledger. Repeated passes MUST be
idempotent through the work ledger; closed observations MUST NOT cancel,
complete or dispatch work. A ledger failure MUST fail the pass rather than
report partial success. The pass MUST NOT schedule itself, retry, claim,
dispatch or publish.

#### Scenario: Repeated pass creates one work version

r[kiln.github.poll.admission.scenario.repeat]
- GIVEN one open issue returned by authenticated polling
- WHEN the same bounded pass runs twice
- THEN the first pass MUST admit one version and the second MUST report a duplicate
- AND one current work version MUST remain

#### Scenario: Closed observation is not completion

r[kiln.github.poll.admission.scenario.closed]
- GIVEN a closed issue observation
- WHEN the pass runs
- THEN it MUST count the observation without cancelling or completing work

### Requirement: Bounded polling admission pass

r[kiln.github.poll.admission] The operator shell MAY run one explicit bounded
polling pass that normalizes through `poll_implementation_issues` and admits the
results into the durable implementation ledger. Repeated passes MUST be
idempotent through the work ledger; closed observations MUST NOT cancel,
complete or dispatch work. A ledger failure MUST fail the pass rather than
report partial success. The pass MUST NOT schedule itself, retry, claim,
dispatch or publish.

#### Scenario: Repeated pass creates one work version

r[kiln.github.poll.admission.scenario.repeat]
- GIVEN one open issue returned by authenticated polling
- WHEN the same bounded pass runs twice
- THEN the first pass MUST admit one version and the second MUST report a duplicate
- AND one current work version MUST remain

#### Scenario: Closed observation is not completion

r[kiln.github.poll.admission.scenario.closed]
- GIVEN a closed issue observation
- WHEN the pass runs
- THEN it MUST count the observation without cancelling or completing work
