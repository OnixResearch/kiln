# GitHub Poll Transport Delta Specification

## ADDED Requirements

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
